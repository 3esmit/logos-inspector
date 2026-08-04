use std::time::Duration;

use anyhow::{Result, bail};
use serde::Serialize;
use serde_json::Value;
use tokio::time::sleep;

use crate::support::raw_source_transport::{
    json_rpc_body, json_rpc_optional_result, request_json, rest_url,
};

const JSON_RPC_TIMEOUT: Duration = Duration::from_secs(8);
const JSON_RPC_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const JSON_RPC_ATTEMPTS: usize = 2;
const JSON_RPC_RETRY_DELAY: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Serialize)]
pub struct RawRpcReport {
    pub endpoint: String,
    pub method: String,
    pub response: Value,
}

pub async fn raw_json_rpc(endpoint: &str, method: &str, params: Value) -> Result<Value> {
    let body = json_rpc_body(method, params)?;
    request_json_with_retry(
        || reqwest_request_client().map(|client| client.post(endpoint).json(&body)),
        endpoint,
        "failed to read rpc response body",
        "invalid JSON-RPC response",
        false,
        false,
    )
    .await
}

pub async fn raw_json_rpc_optional_result(
    endpoint: &str,
    method: &str,
    params: Value,
) -> Result<Value> {
    let response = raw_json_rpc(endpoint, method, params).await?;
    json_rpc_optional_result(&response, method)
}

pub async fn raw_http_json(endpoint: &str, path: &str) -> Result<Value> {
    let url = rest_url(endpoint, path);
    request_json_with_retry(
        || reqwest_request_client().map(|client| client.get(&url)),
        &url,
        "failed to read http response body",
        "invalid JSON response",
        false,
        false,
    )
    .await
}

pub async fn raw_rpc_report(endpoint: &str, method: &str, params: Value) -> Result<RawRpcReport> {
    Ok(RawRpcReport {
        endpoint: endpoint.to_owned(),
        method: method.to_owned(),
        response: raw_json_rpc(endpoint, method, params).await?,
    })
}

fn reqwest_request_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(JSON_RPC_CONNECT_TIMEOUT)
        .timeout(JSON_RPC_TIMEOUT)
        .build()
        .map_err(Into::into)
}

async fn request_json_with_retry<F>(
    mut request: F,
    label: &str,
    body_context: &'static str,
    invalid_context: &'static str,
    allow_no_content: bool,
    empty_as_null: bool,
) -> Result<Value>
where
    F: FnMut() -> Result<reqwest::RequestBuilder>,
{
    let mut last_error = None;
    for attempt in 0..JSON_RPC_ATTEMPTS {
        match request() {
            Ok(request) => {
                match request_json(
                    request,
                    label,
                    body_context,
                    invalid_context,
                    allow_no_content,
                    empty_as_null,
                )
                .await
                {
                    Ok(response) => return Ok(response),
                    Err(error)
                        if attempt + 1 < JSON_RPC_ATTEMPTS
                            && is_transient_transport_error(&error) =>
                    {
                        last_error = Some(error);
                        sleep(JSON_RPC_RETRY_DELAY).await;
                    }
                    Err(error) => return Err(error),
                }
            }
            Err(error) => return Err(error),
        }
    }
    match last_error {
        Some(error) => Err(error),
        None => bail!("request retry loop completed without a result"),
    }
}

fn is_transient_transport_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<reqwest::Error>()
            .is_some_and(|error| error.is_connect() || error.is_timeout())
    })
}
