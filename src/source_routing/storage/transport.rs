use std::path::Path;

use anyhow::{Context as _, Result};
use reqwest::{Method, Response, header};
use serde_json::{Value, json};
use tokio_util::io::ReaderStream;

use crate::{
    source_routing::shared::{http, module_bridge},
    support::raw_source_transport::{request_bytes, request_success},
};

pub(super) async fn module_call(method: &'static str, args: Vec<Value>) -> Result<Value> {
    blocking_module_call("Storage module call", move || {
        module_bridge::call_value(super::layer::module_id(), method, &args)
    })
    .await
}

pub(super) async fn module_dispatch(
    method: &'static str,
    args: Vec<Value>,
    context: Vec<(&'static str, String)>,
) -> Result<Value> {
    let value = module_call(method, args).await?;
    Ok(module_bridge::dispatch_result(
        super::layer::module_id(),
        method,
        value,
        &context,
    ))
}

pub(super) async fn manifests(endpoint: &str) -> Result<Value> {
    crate::rpc::raw_http_json(endpoint, "/data").await
}

pub(super) async fn manifest(endpoint: &str, cid: &str) -> Result<Value> {
    crate::rpc::raw_http_json(endpoint, &format!("/data/{cid}/network/manifest")).await
}

pub(super) async fn exists(endpoint: &str, cid: &str) -> Result<Value> {
    crate::rpc::raw_http_json(endpoint, &format!("/data/{cid}/exists")).await
}

pub(super) async fn probe_value(endpoint: &str, path: &str) -> Result<Value> {
    let url = http::rest_url(endpoint, path);
    let text = http::raw_http_text_url(&url).await?;
    Ok(parse_probe_text(&text))
}

pub(super) async fn probe_metrics(endpoint: &str) -> Result<String> {
    http::raw_http_text_url(endpoint).await
}

pub(super) async fn fetch(endpoint: &str, cid: &str) -> Result<Value> {
    http::rest_json_request(
        Method::POST,
        endpoint,
        &format!("/data/{cid}/network"),
        None,
    )
    .await
}

pub(super) async fn upload(endpoint: &str, path: &str, block_size: u64) -> Result<Value> {
    let file = tokio::fs::File::open(path)
        .await
        .with_context(|| format!("failed to open upload file `{path}`"))?;
    let bytes = file
        .metadata()
        .await
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let filename = Path::new(path)
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty());
    let body = reqwest::Body::wrap_stream(ReaderStream::new(file));
    let mut request = reqwest::Client::new()
        .post(http::rest_url(
            endpoint,
            &format!("/data?blockSize={block_size}"),
        ))
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(body);
    if let Some(filename) = filename {
        request = request.header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"{}\"",
                filename.replace(['\\', '"'], "_")
            ),
        );
    }
    let text = http::send_text(request, "storage upload").await?;
    Ok(json!({
        "cid": text.trim(),
        "path": path,
        "bytes": bytes,
        "endpoint": endpoint,
    }))
}

pub(super) async fn upload_bytes(
    endpoint: &str,
    filename: &str,
    bytes: &[u8],
    block_size: u64,
) -> Result<Value> {
    let text = http::send_text(
        reqwest::Client::new()
            .post(http::rest_url(
                endpoint,
                &format!("/data?blockSize={block_size}"),
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .header(
                header::CONTENT_DISPOSITION,
                format!(
                    "attachment; filename=\"{}\"",
                    filename.replace(['\\', '"'], "_")
                ),
            )
            .body(bytes.to_vec()),
        "storage settings backup upload",
    )
    .await?;
    Ok(json!({
        "cid": text.trim(),
        "filename": filename,
        "bytes": bytes.len(),
        "endpoint": endpoint,
    }))
}

pub(super) async fn download_bytes(endpoint: &str, cid: &str, local_only: bool) -> Result<Vec<u8>> {
    let route = download_route(cid, local_only);
    let url = http::rest_url(endpoint, &route);
    request_bytes(
        reqwest::Client::new().get(&url),
        &url,
        "failed to read storage backup download body",
    )
    .await
}

pub(super) async fn download_response(
    endpoint: &str,
    cid: &str,
    local_only: bool,
) -> Result<Response> {
    let url = http::rest_url(endpoint, &download_route(cid, local_only));
    request_success(
        reqwest::Client::new().get(&url),
        &url,
        "storage download",
        "failed to read storage download error body",
    )
    .await
}

pub(super) async fn remove(endpoint: &str, cid: &str) -> Result<Value> {
    http::rest_empty_request(Method::DELETE, endpoint, &format!("/data/{cid}"), None).await?;
    Ok(json!({
        "removed": true,
        "cid": cid,
        "endpoint": endpoint,
    }))
}

fn download_route(cid: &str, local_only: bool) -> String {
    if local_only {
        format!("/data/{cid}")
    } else {
        format!("/data/{cid}/network/stream")
    }
}

async fn blocking_module_call<T, F>(label: &'static str, call: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(call)
        .await
        .with_context(|| format!("{label} worker failed"))?
}

fn parse_probe_text(text: &str) -> Value {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Value::Null;
    }
    serde_json::from_str(trimmed).unwrap_or_else(|_| Value::String(trimmed.to_owned()))
}
