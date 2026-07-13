use anyhow::Result;
use reqwest::Method;
use serde_json::Value;

pub(crate) use crate::support::raw_source_transport::rest_url;
use crate::support::raw_source_transport::{request_json, request_text};

pub(crate) async fn raw_http_json_url(url: &str) -> Result<Value> {
    request_json(
        reqwest::Client::new().get(url),
        url,
        "failed to read http response body",
        "invalid JSON response",
        true,
        true,
    )
    .await
}

pub(crate) async fn raw_http_text_url(url: &str) -> Result<String> {
    send_text(reqwest::Client::new().get(url), url).await
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
    request_json(
        request,
        &url,
        "failed to read http response body",
        "invalid JSON response",
        true,
        false,
    )
    .await
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
    request_text(request, label, "failed to read http response body", true).await
}
