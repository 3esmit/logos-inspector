use anyhow::Result;
use serde_json::{Value, json};

pub(crate) fn call_value(module: &str, method: &str, values: &[Value]) -> Result<Value> {
    let args = values.iter().map(module_arg_text).collect::<Vec<_>>();
    crate::source_routing::core::adapters::module::call_value(module, method, &args)
}

pub(crate) fn dispatch_result(
    module: &str,
    method: &str,
    value: Value,
    context: &[(&str, String)],
) -> Value {
    let mut result = json!({
        "module": module,
        "method": method,
        "dispatched": true,
        "value": value,
    });
    if let Some(object) = result.as_object_mut() {
        if let Some(session_id) = dispatch_session_id(object.get("value").unwrap_or(&Value::Null)) {
            object.insert("sessionId".to_owned(), json!(session_id));
        }
        if let Some(request_id) = dispatch_request_id(object.get("value").unwrap_or(&Value::Null)) {
            object.insert("requestId".to_owned(), json!(request_id));
        }
        for (key, value) in context {
            if !value.trim().is_empty() {
                object.insert((*key).to_owned(), json!(value));
            }
        }
    }
    result
}

fn module_arg_text(value: &Value) -> String {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| value.to_string())
}

fn dispatch_session_id(value: &Value) -> Option<String> {
    if let Some(text) = value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(text.to_owned());
    }
    let object = value.as_object()?;
    for key in ["sessionId", "session_id", "operationId", "operation_id"] {
        if let Some(text) = object
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(text.to_owned());
        }
    }
    None
}

fn dispatch_request_id(value: &Value) -> Option<String> {
    if let Some(text) = value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(text.to_owned());
    }
    let object = value.as_object()?;
    for key in ["requestId", "request_id"] {
        if let Some(text) = object
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(text.to_owned());
        }
    }
    None
}
