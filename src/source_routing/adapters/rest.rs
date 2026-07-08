use std::path::Path;

use anyhow::{Context as _, Result};
use reqwest::{Method, Url, header};
use serde_json::{Value, json};
use tokio_util::io::ReaderStream;

use crate::{parse_json_body, read_response_bytes, read_response_text};

use super::super::selection::DeliveryStoreQuery;

pub(crate) async fn storage_rest_upload(
    endpoint: &str,
    path: &str,
    block_size: u64,
) -> Result<Value> {
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
        .post(rest_url(endpoint, &format!("/data?blockSize={block_size}")))
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
    let text = send_text(request, "storage upload").await?;
    Ok(json!({
        "cid": text.trim(),
        "path": path,
        "bytes": bytes,
        "endpoint": endpoint,
    }))
}

pub(crate) async fn storage_rest_upload_bytes(
    endpoint: &str,
    filename: &str,
    bytes: &[u8],
    block_size: u64,
) -> Result<Value> {
    let text = send_text(
        reqwest::Client::new()
            .post(rest_url(endpoint, &format!("/data?blockSize={block_size}")))
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

pub(crate) async fn storage_rest_download_bytes(
    endpoint: &str,
    cid: &str,
    local_only: bool,
) -> Result<Vec<u8>> {
    let route = if local_only {
        format!("/data/{cid}")
    } else {
        format!("/data/{cid}/network/stream")
    };
    read_response_bytes(
        reqwest::Client::new().get(rest_url(endpoint, &route)),
        &rest_url(endpoint, &route),
        "failed to read storage backup download body",
    )
    .await
}

pub(crate) fn delivery_store_query_url(
    endpoint: &str,
    store_query: DeliveryStoreQuery<'_>,
) -> Result<Url> {
    let mut url = Url::parse(&rest_url(endpoint, "/store/v3/messages"))
        .context("invalid Delivery REST endpoint")?;
    {
        let mut query = url.query_pairs_mut();
        if let Some(peer_addr) = store_query.peer_addr {
            query.append_pair("peerAddr", peer_addr);
        }
        if let Some(content_topics) = store_query.content_topics {
            query.append_pair("contentTopics", content_topics);
        }
        if let Some(pubsub_topic) = store_query.pubsub_topic {
            query.append_pair("pubsubTopic", pubsub_topic);
        }
        if let Some(cursor) = store_query.cursor {
            query.append_pair("cursor", cursor);
        }
        query.append_pair(
            "includeData",
            if store_query.include_data {
                "true"
            } else {
                "false"
            },
        );
        query.append_pair("pageSize", &store_query.page_size.to_string());
        query.append_pair(
            "ascending",
            if store_query.ascending {
                "true"
            } else {
                "false"
            },
        );
    }
    Ok(url)
}

pub(crate) async fn raw_http_json_url(url: &str) -> Result<Value> {
    let text = send_text(reqwest::Client::new().get(url), url).await?;
    parse_json_body(&text, "invalid JSON response", true)
}

pub(crate) async fn rest_json_request(
    method: Method,
    endpoint: &str,
    path: &str,
    body: Option<Value>,
) -> Result<Value> {
    let url = rest_url(endpoint, path);
    let mut request = reqwest::Client::new().request(method, &url);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let text = send_text(request, &url).await?;
    parse_json_body(&text, "invalid JSON response", false)
}

pub(crate) async fn rest_empty_request(
    method: Method,
    endpoint: &str,
    path: &str,
    body: Option<Value>,
) -> Result<()> {
    let url = rest_url(endpoint, path);
    let mut request = reqwest::Client::new().request(method, &url);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let _ = send_text(request, &url).await?;
    Ok(())
}

pub(crate) async fn send_text(request: reqwest::RequestBuilder, label: &str) -> Result<String> {
    read_response_text(request, label, "failed to read http response body", true).await
}

pub(crate) fn rest_url(endpoint: &str, path: &str) -> String {
    let endpoint = endpoint.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    format!("{endpoint}/{path}")
}
