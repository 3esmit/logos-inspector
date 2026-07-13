use std::{collections::HashSet, path::Path, time::Duration};

use anyhow::{Context as _, Result, bail};
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

pub(super) async fn module_upload_bytes(
    filename: &str,
    bytes: &[u8],
    block_size: u64,
) -> Result<Value> {
    let filename = filename.to_owned();
    let bytes = bytes.to_vec();
    blocking_module_call("Storage module payload upload", move || {
        module_upload_bytes_blocking(&filename, &bytes, block_size)
    })
    .await
}

fn module_upload_bytes_blocking(filename: &str, bytes: &[u8], block_size: u64) -> Result<Value> {
    let block_size = i64::try_from(block_size).context("storage upload block size is too large")?;
    let safe_filename = Path::new(filename)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .context("storage upload filename is invalid")?;
    let staged = crate::modules::logos_core::stage_shared_file(safe_filename, bytes)?;
    let path = staged
        .path()
        .to_str()
        .context("temporary storage upload path is not UTF-8")?
        .to_owned();

    crate::modules::logos_core::require_module_method(
        super::layer::module_id(),
        "uploadUrl",
        "uploadUrl(QString,int)",
    )?;
    crate::modules::logos_core::require_module_method(
        super::layer::module_id(),
        "manifests",
        "manifests()",
    )?;
    let manifests_before = crate::source_routing::shared::module_bridge::call_value(
        super::layer::module_id(),
        "manifests",
        &[],
    )?;
    let baseline_cids = manifest_cids(&manifests_before);
    let session = crate::source_routing::shared::module_bridge::call_value(
        super::layer::module_id(),
        "uploadUrl",
        &[json!(path), json!(block_size)],
    )?;
    let session_id = session
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("storage_module.uploadUrl returned no session ID")?
        .to_owned();
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    let cid = loop {
        let manifests = crate::source_routing::shared::module_bridge::call_value(
            super::layer::module_id(),
            "manifests",
            &[],
        )?;
        if let Some(cid) = new_manifest_cid(&manifests, safe_filename, bytes.len(), &baseline_cids)
        {
            break cid;
        }
        if std::time::Instant::now() >= deadline {
            bail!("timed out waiting for storage_module upload session {session_id}");
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    Ok(json!({
        "cid": cid,
        "filename": safe_filename,
        "bytes": bytes.len(),
        "endpoint": "logoscore call storage_module.uploadUrl",
        "completion": "manifest_poll",
        "sessionId": session_id,
    }))
}

fn manifest_cids(manifests: &Value) -> HashSet<String> {
    manifests
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|manifest| manifest.get("cid").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .collect()
}

fn new_manifest_cid(
    manifests: &Value,
    filename: &str,
    bytes: usize,
    baseline_cids: &HashSet<String>,
) -> Option<String> {
    let bytes = u64::try_from(bytes).ok()?;
    manifests.as_array()?.iter().find_map(|manifest| {
        let cid = manifest.get("cid")?.as_str()?.trim();
        let candidate_filename = manifest.get("filename")?.as_str()?;
        let candidate_bytes = manifest
            .get("datasetSize")
            .or_else(|| manifest.get("dataset_size"))?
            .as_u64()?;
        (candidate_filename == filename
            && candidate_bytes == bytes
            && !cid.is_empty()
            && !baseline_cids.contains(cid))
        .then(|| cid.to_owned())
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_poll_correlates_new_upload_by_filename_and_size() -> Result<()> {
        let baseline = HashSet::from(["cid-old".to_owned()]);
        let manifests = json!([
            {"cid":"cid-old","filename":"backup.json","datasetSize":12},
            {"cid":"cid-wrong-size","filename":"backup.json","datasetSize":13},
            {"cid":"cid-new","filename":"backup.json","datasetSize":12}
        ]);

        let cid = new_manifest_cid(&manifests, "backup.json", 12, &baseline);

        anyhow::ensure!(
            cid.as_deref() == Some("cid-new"),
            "manifest correlation drift"
        );
        Ok(())
    }
}
