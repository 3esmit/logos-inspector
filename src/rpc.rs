use std::time::Duration;

use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

use crate::support::raw_source_transport::{
    json_rpc_body, json_rpc_optional_result, json_rpc_required_result, request_json, rest_url,
};

const JSON_RPC_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, Serialize)]
pub struct RawRpcReport {
    pub endpoint: String,
    pub method: String,
    pub response: Value,
}

pub async fn raw_json_rpc(endpoint: &str, method: &str, params: Value) -> Result<Value> {
    let body = json_rpc_body(method, params)?;
    request_json(
        reqwest::Client::new()
            .post(endpoint)
            .timeout(JSON_RPC_TIMEOUT)
            .json(&body),
        endpoint,
        "failed to read rpc response body",
        "invalid JSON-RPC response",
        false,
        false,
    )
    .await
}

pub async fn raw_json_rpc_result(endpoint: &str, method: &str, params: Value) -> Result<Value> {
    let response = raw_json_rpc(endpoint, method, params).await?;
    json_rpc_required_result(&response, method)
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
    request_json(
        reqwest::Client::new().get(&url),
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
