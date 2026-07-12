use anyhow::{Context as _, Result, bail};
use reqwest::Response;
use serde_json::{Value, json};

use super::http_response::{
    expect_success_response, expect_success_response_bounded, read_response_bytes,
    read_response_json, read_response_json_bounded, read_response_text,
};

pub(crate) fn rest_url(endpoint: &str, path: &str) -> String {
    let endpoint = endpoint.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    format!("{endpoint}/{path}")
}

pub(crate) fn json_rpc_body(method: &str, params: Value) -> Result<Value> {
    if method.trim().is_empty() {
        bail!("rpc method is required");
    }
    let params = match params {
        Value::Array(_) | Value::Object(_) => params,
        Value::Null => Value::Array(vec![]),
        other => bail!("rpc params must be array or object, got {other}"),
    };
    Ok(json!({
        "jsonrpc": "2.0",
        "id": 1_u64,
        "method": method,
        "params": params,
    }))
}

pub(crate) fn json_rpc_optional_result(response: &Value, method: &str) -> Result<Value> {
    json_rpc_result_value(response, method).cloned()
}

pub(crate) fn json_rpc_required_result(response: &Value, method: &str) -> Result<Value> {
    let value = json_rpc_result_value(response, method)?;
    if value.is_null() {
        bail!("{method} returned no result");
    }
    Ok(value.clone())
}

fn json_rpc_result_value<'a>(response: &'a Value, method: &str) -> Result<&'a Value> {
    if let Some(error) = response.get("error") {
        bail!("{method} returned JSON-RPC error: {error}");
    }
    response
        .get("result")
        .with_context(|| format!("{method} returned no result"))
}

pub(crate) async fn request_json(
    request: reqwest::RequestBuilder,
    label: &str,
    body_context: &'static str,
    invalid_context: &'static str,
    allow_no_content: bool,
    empty_as_null: bool,
) -> Result<Value> {
    read_response_json(
        request,
        label,
        body_context,
        invalid_context,
        allow_no_content,
        empty_as_null,
    )
    .await
}

pub(crate) async fn request_json_bounded(
    request: reqwest::RequestBuilder,
    label: &str,
    body_context: &'static str,
    invalid_context: &'static str,
    allow_no_content: bool,
    empty_as_null: bool,
    max_bytes: usize,
) -> Result<Value> {
    read_response_json_bounded(
        request,
        label,
        body_context,
        invalid_context,
        allow_no_content,
        empty_as_null,
        max_bytes,
    )
    .await
}

pub(crate) async fn request_text(
    request: reqwest::RequestBuilder,
    label: &str,
    body_context: &'static str,
    allow_no_content: bool,
) -> Result<String> {
    read_response_text(request, label, body_context, allow_no_content).await
}

pub(crate) async fn request_bytes(
    request: reqwest::RequestBuilder,
    label: &str,
    body_context: &'static str,
) -> Result<Vec<u8>> {
    read_response_bytes(request, label, body_context).await
}

pub(crate) async fn request_success(
    request: reqwest::RequestBuilder,
    call_label: &str,
    response_label: &str,
    error_body_context: &'static str,
) -> Result<Response> {
    let response = request
        .send()
        .await
        .with_context(|| format!("failed to call {call_label}"))?;
    expect_success_response(response, response_label, error_body_context).await
}

pub(crate) async fn request_success_bounded(
    request: reqwest::RequestBuilder,
    call_label: &str,
    response_label: &str,
    error_body_context: &'static str,
    max_error_bytes: usize,
) -> Result<Response> {
    let response = request
        .send()
        .await
        .with_context(|| format!("failed to call {call_label}"))?;
    expect_success_response_bounded(
        response,
        response_label,
        error_body_context,
        max_error_bytes,
    )
    .await
}

#[cfg(test)]
mod tests {
    use anyhow::ensure;

    use super::*;

    #[test]
    fn rest_url_trims_endpoint_and_path_slashes() {
        assert_eq!(
            rest_url("http://127.0.0.1:8080/", "/data/cid"),
            "http://127.0.0.1:8080/data/cid"
        );
    }

    #[test]
    fn json_rpc_body_normalizes_null_params_to_empty_array() -> Result<()> {
        let body = json_rpc_body("getStatus", Value::Null)?;

        ensure!(
            body == json!({
                "jsonrpc": "2.0",
                "id": 1_u64,
                "method": "getStatus",
                "params": [],
            }),
            "unexpected JSON-RPC body"
        );
        Ok(())
    }

    #[test]
    fn json_rpc_body_rejects_scalar_params() {
        let error = json_rpc_body("getStatus", json!("bad")).err();

        assert!(
            error
                .as_ref()
                .is_some_and(|error| error.to_string().contains("rpc params must be array"))
        );
    }

    #[test]
    fn json_rpc_required_result_rejects_null_result() {
        let error = json_rpc_required_result(&json!({ "result": null }), "method").err();

        assert!(
            error
                .as_ref()
                .is_some_and(|error| error.to_string().contains("returned no result"))
        );
    }

    #[test]
    fn json_rpc_optional_result_keeps_null_result() -> Result<()> {
        let value = json_rpc_optional_result(&json!({ "result": null }), "method")?;

        ensure!(value == Value::Null, "unexpected optional JSON-RPC result");
        Ok(())
    }

    #[test]
    fn json_rpc_result_reports_error_payload() {
        let error = json_rpc_optional_result(
            &json!({ "error": { "code": -1, "message": "nope" } }),
            "method",
        )
        .err();

        assert!(
            error
                .as_ref()
                .is_some_and(|error| error.to_string().contains("JSON-RPC error"))
        );
    }
}
