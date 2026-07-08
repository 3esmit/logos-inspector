use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::support::{http_response::read_response_json, json_value::value_to_string};

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

pub async fn logos_node_cryptarchia_info(endpoint: &str) -> Result<Value> {
    raw_http_json(endpoint, "/cryptarchia/info")
        .await
        .map(normalize_cryptarchia_info)
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

pub(crate) fn normalize_cryptarchia_info(raw: Value) -> Value {
    let source = raw
        .get("cryptarchia_info")
        .filter(|value| value.is_object())
        .unwrap_or(&raw);
    let mut info = Map::new();

    if let Some(lib) = first_present(source, &["lib", "lib_hash"]) {
        insert_value(&mut info, "lib", lib.clone());
        insert_value(&mut info, "lib_hash", lib.clone());
    }
    if let Some(lib_slot) = first_u64(source, &["lib_slot", "lib_height"]) {
        insert_value(&mut info, "lib_slot", json!(lib_slot));
    }
    if let Some(tip) = first_present(source, &["tip", "tip_hash", "hash"]) {
        insert_value(&mut info, "tip", tip.clone());
        insert_value(&mut info, "tip_hash", tip.clone());
    }
    if let Some(slot) = first_u64(source, &["slot", "tip_slot", "height"]) {
        insert_value(&mut info, "slot", json!(slot));
    }
    if let Some(height) = first_u64(source, &["height", "slot"]) {
        insert_value(&mut info, "height", json!(height));
    }

    if let Some(mode) = raw
        .get("mode")
        .or_else(|| source.get("mode"))
        .and_then(mode_text)
    {
        insert_value(&mut info, "mode", json!(mode));
    }

    let mut normalized = info.clone();
    normalized.insert("cryptarchia_info".to_owned(), Value::Object(info));
    normalized.insert("raw".to_owned(), raw);
    Value::Object(normalized)
}

fn insert_value(map: &mut Map<String, Value>, key: &str, value: Value) {
    map.insert(key.to_owned(), value);
}

fn first_present<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| value.get(*key))
}

fn first_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|value| value.trim().parse().ok()))
        })
    })
}

fn mode_text(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(_) | Value::Bool(_) => Some(value_to_string(value)),
        Value::Object(object) => {
            let mut entries = object.iter();
            let (key, value) = entries.next()?;
            if entries.next().is_some() {
                return Some(value_to_string(value));
            }
            if value.is_null() {
                return Some(key.clone());
            }
            Some(value_to_string(value))
        }
        Value::Null | Value::Array(_) => None,
    }
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
    fn normalize_cryptarchia_info_accepts_nested_mode_object() {
        let raw = json!({
            "cryptarchia_info": {
                "lib": "lib-hash",
                "lib_slot": "20",
                "tip": "tip-hash",
                "slot": 30
            },
            "mode": { "Started": "Online" }
        });

        let normalized = normalize_cryptarchia_info(raw.clone());

        assert_eq!(
            normalized.pointer("/cryptarchia_info/lib"),
            Some(&json!("lib-hash"))
        );
        assert_eq!(
            normalized.pointer("/cryptarchia_info/lib_slot"),
            Some(&json!(20))
        );
        assert_eq!(
            normalized.pointer("/cryptarchia_info/tip"),
            Some(&json!("tip-hash"))
        );
        assert_eq!(
            normalized.pointer("/cryptarchia_info/slot"),
            Some(&json!(30))
        );
        assert_eq!(
            normalized.pointer("/cryptarchia_info/height"),
            Some(&json!(30))
        );
        assert_eq!(
            normalized.pointer("/cryptarchia_info/mode"),
            Some(&json!("Online"))
        );
        assert_eq!(normalized.get("mode"), Some(&json!("Online")));
        assert_eq!(normalized.get("raw"), Some(&raw));
    }

    #[test]
    fn normalize_cryptarchia_info_accepts_flat_string_mode() {
        let raw = json!({
            "lib_hash": "lib-hash",
            "lib_slot": 8,
            "tip_hash": "tip-hash",
            "height": "13",
            "mode": "Running"
        });

        let normalized = normalize_cryptarchia_info(raw);

        assert_eq!(
            normalized.pointer("/cryptarchia_info/lib"),
            Some(&json!("lib-hash"))
        );
        assert_eq!(
            normalized.pointer("/cryptarchia_info/tip"),
            Some(&json!("tip-hash"))
        );
        assert_eq!(
            normalized.pointer("/cryptarchia_info/slot"),
            Some(&json!(13))
        );
        assert_eq!(
            normalized.pointer("/cryptarchia_info/height"),
            Some(&json!(13))
        );
        assert_eq!(
            normalized.pointer("/cryptarchia_info/mode"),
            Some(&json!("Running"))
        );
    }

    #[test]
    fn normalize_cryptarchia_info_accepts_numeric_mode() {
        let normalized = normalize_cryptarchia_info(json!({ "mode": 2 }));

        assert_eq!(
            normalized.pointer("/cryptarchia_info/mode"),
            Some(&json!("2"))
        );
    }

    #[test]
    fn normalize_cryptarchia_info_tolerates_missing_optional_fields() {
        let normalized = normalize_cryptarchia_info(json!({}));

        assert_eq!(
            normalized.pointer("/cryptarchia_info"),
            Some(&Value::Object(Map::new()))
        );
        assert!(normalized.get("raw").is_some());
    }
}
