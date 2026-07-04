use anyhow::{Context as _, Result, bail};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize)]
pub struct RawRpcReport {
    pub endpoint: String,
    pub method: String,
    pub response: Value,
}

pub async fn raw_json_rpc(endpoint: &str, method: &str, params: Value) -> Result<Value> {
    if method.trim().is_empty() {
        bail!("rpc method is required");
    }
    let params = match params {
        Value::Array(_) | Value::Object(_) => params,
        Value::Null => Value::Array(vec![]),
        other => bail!("rpc params must be array or object, got {other}"),
    };
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1_u64,
        "method": method,
        "params": params,
    });
    let response = reqwest::Client::new()
        .post(endpoint)
        .json(&body)
        .send()
        .await
        .with_context(|| format!("failed to call {endpoint}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .context("failed to read rpc response body")?;
    if !status.is_success() {
        bail!("rpc HTTP {status}: {}", response_excerpt(&text));
    }
    let json: Value = serde_json::from_str(&text)
        .with_context(|| format!("invalid JSON-RPC response: {}", response_excerpt(&text)))?;
    Ok(json)
}

pub async fn raw_json_rpc_result(endpoint: &str, method: &str, params: Value) -> Result<Value> {
    let value = raw_json_rpc_optional_result(endpoint, method, params).await?;
    if value.is_null() {
        bail!("{method} returned no result");
    }
    Ok(value)
}

pub async fn raw_json_rpc_optional_result(
    endpoint: &str,
    method: &str,
    params: Value,
) -> Result<Value> {
    let response = raw_json_rpc(endpoint, method, params).await?;
    json_rpc_result_value(&response, method).cloned()
}

pub async fn logos_node_cryptarchia_info(endpoint: &str) -> Result<Value> {
    raw_http_json(endpoint, "/cryptarchia/info").await
}

pub async fn raw_http_json(endpoint: &str, path: &str) -> Result<Value> {
    let endpoint = endpoint.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    let url = format!("{endpoint}/{path}");
    let response = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .with_context(|| format!("failed to call {url}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .context("failed to read http response body")?;
    if !status.is_success() {
        bail!(
            "http call `{url}` failed with status {status}: {}",
            response_excerpt(&text)
        );
    }
    let json: Value = serde_json::from_str(&text)
        .with_context(|| format!("invalid JSON response: {}", response_excerpt(&text)))?;
    Ok(json)
}

pub async fn raw_rpc_report(endpoint: &str, method: &str, params: Value) -> Result<RawRpcReport> {
    Ok(RawRpcReport {
        endpoint: endpoint.to_owned(),
        method: method.to_owned(),
        response: raw_json_rpc(endpoint, method, params).await?,
    })
}

pub(crate) fn response_excerpt(text: &str) -> String {
    text.chars().take(400).collect()
}

pub(crate) fn json_rpc_result<'a>(response: &'a Value, method: &str) -> Result<Option<&'a Value>> {
    let value = json_rpc_result_value(response, method)?;
    Ok((!value.is_null()).then_some(value))
}

fn json_rpc_result_value<'a>(response: &'a Value, method: &str) -> Result<&'a Value> {
    if let Some(error) = response.get("error") {
        bail!("{method} returned JSON-RPC error: {error}");
    }
    response
        .get("result")
        .with_context(|| format!("{method} returned no result"))
}
