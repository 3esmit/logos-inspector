use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use serde::Serialize;
use serde_json::{Value, json};

use crate::support::http_response::read_response_json;

const JSON_RPC_TIMEOUT: Duration = Duration::from_secs(8);

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
    read_response_json(
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

pub async fn raw_http_json(endpoint: &str, path: &str) -> Result<Value> {
    let endpoint = endpoint.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    let url = format!("{endpoint}/{path}");
    read_response_json(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_rpc_result_reports_error_payload() {
        let response = json!({
            "error": { "code": -1, "message": "nope" }
        });

        let error = json_rpc_result(&response, "method").err();

        assert!(
            error
                .as_ref()
                .is_some_and(|error| error.to_string().contains("JSON-RPC error"))
        );
    }

    #[test]
    fn json_rpc_result_reports_missing_result() {
        let response = json!({});

        let error = json_rpc_result(&response, "method").err();

        assert!(
            error
                .as_ref()
                .is_some_and(|error| error.to_string().contains("returned no result"))
        );
    }

    #[test]
    fn json_rpc_result_maps_null_to_none() {
        let response = json!({ "result": null });

        let result = json_rpc_result(&response, "method");

        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn json_rpc_result_maps_non_null_result() {
        let response = json!({ "result": { "ok": true } });

        let result = json_rpc_result(&response, "method");

        assert!(matches!(result, Ok(Some(value)) if value == &json!({ "ok": true })));
    }
}
