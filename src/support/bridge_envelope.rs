use std::fmt;

use anyhow::{Result, anyhow};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, serde::Serialize)]
struct BridgeResponse {
    ok: bool,
    value: Value,
    text: String,
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_details: Option<Value>,
}

#[derive(Debug)]
struct StructuredBridgeError {
    message: String,
    details: Value,
}

impl fmt::Display for StructuredBridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for StructuredBridgeError {}

pub(crate) fn structured_bridge_error(
    message: impl Into<String>,
    details: impl Serialize,
) -> Result<anyhow::Error> {
    let details = serde_json::to_value(details)?;
    Ok(anyhow!(StructuredBridgeError {
        message: message.into(),
        details,
    }))
}

#[must_use]
pub fn bridge_error_response_json(error: impl Into<String>) -> String {
    serialize_bridge_response(BridgeResponse {
        ok: false,
        value: Value::Null,
        text: String::new(),
        error: error.into(),
        error_details: None,
    })
}

#[must_use]
pub(crate) fn bridge_response_json(result: Result<Value>) -> String {
    match result {
        Ok(value) => {
            let text = format_bridge_value(&value);
            serialize_bridge_response(BridgeResponse {
                ok: true,
                value,
                text,
                error: String::new(),
                error_details: None,
            })
        }
        Err(error) => {
            let error_details = error
                .downcast_ref::<StructuredBridgeError>()
                .map(|error| error.details.clone());
            serialize_bridge_response(BridgeResponse {
                ok: false,
                value: Value::Null,
                text: String::new(),
                error: format!("{error:#}"),
                error_details,
            })
        }
    }
}

fn serialize_bridge_response(response: BridgeResponse) -> String {
    match serde_json::to_string(&response) {
        Ok(value) => value,
        Err(error) => json!({
            "ok": false,
            "value": null,
            "text": "",
            "error": format!("failed to serialize bridge response: {error}"),
        })
        .to_string(),
    }
}

fn format_bridge_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        value => serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{Context as _, Result, bail};
    use serde_json::{Value, json};

    use super::*;

    #[test]
    fn bridge_response_text_uses_plain_string_values() -> Result<()> {
        let response = response_value(bridge_response_json(Ok(json!("ready"))))?;

        if response.get("ok").and_then(Value::as_bool) != Some(true) {
            bail!("expected ok response: {response}");
        }
        if response.get("text").and_then(Value::as_str) != Some("ready") {
            bail!("unexpected bridge text: {response}");
        }
        Ok(())
    }

    #[test]
    fn bridge_response_text_pretty_prints_structured_values() -> Result<()> {
        let value = json!({ "status": "ready" });
        let response = response_value(bridge_response_json(Ok(value.clone())))?;
        let expected = serde_json::to_string_pretty(&value)?;

        if response.get("text").and_then(Value::as_str) != Some(expected.as_str()) {
            bail!("unexpected structured bridge text: {response}");
        }
        Ok(())
    }

    #[test]
    fn bridge_response_formats_error_chains() -> Result<()> {
        let error = anyhow::anyhow!("inner").context("outer");
        let response = response_value(bridge_response_json(Err(error)))?;

        if response.get("ok").and_then(Value::as_bool) != Some(false) {
            bail!("expected error response: {response}");
        }
        if response.get("error").and_then(Value::as_str) != Some("outer: inner") {
            bail!("unexpected bridge error: {response}");
        }
        if response.get("error_details").is_some() {
            bail!("legacy bridge error unexpectedly has structured details: {response}");
        }
        Ok(())
    }

    #[test]
    fn bridge_response_includes_typed_error_details() -> Result<()> {
        let error = structured_bridge_error(
            "context changed",
            json!({
                "code": "stale_context",
                "recovery": "refresh_context",
            }),
        )?;
        let response = response_value(bridge_response_json(Err(error)))?;

        if response.get("ok").and_then(Value::as_bool) != Some(false)
            || response.get("error").and_then(Value::as_str) != Some("context changed")
            || response
                .pointer("/error_details/code")
                .and_then(Value::as_str)
                != Some("stale_context")
            || response
                .pointer("/error_details/recovery")
                .and_then(Value::as_str)
                != Some("refresh_context")
        {
            bail!("unexpected structured bridge error: {response}");
        }
        Ok(())
    }

    #[test]
    fn bridge_error_response_uses_supplied_error_text() -> Result<()> {
        let response = response_value(bridge_error_response_json("missing handle"))?;

        if response.get("ok").and_then(Value::as_bool) != Some(false)
            || !response.get("value").is_some_and(Value::is_null)
            || response.get("text").and_then(Value::as_str) != Some("")
            || response.get("error").and_then(Value::as_str) != Some("missing handle")
            || response.get("error_details").is_some()
        {
            bail!("unexpected bridge error response: {response}");
        }
        Ok(())
    }

    fn response_value(response: String) -> Result<Value> {
        serde_json::from_str(&response).context("response should be JSON")
    }
}
