use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::{
    source_routing::{SourceFamily, effective_source_mode, source_mode_is_token},
    support::args::Args,
};

pub(crate) const DELIVERY_MODULE: &str = "delivery_module";
pub(crate) const STORAGE_MODULE: &str = "storage_module";

pub(crate) struct ModuleCallArgs {
    pub(crate) values: Vec<Value>,
}

pub(crate) fn storage_args(
    args: &Args,
    uses_mutating_flag: bool,
    action_label: &str,
) -> Result<Option<ModuleCallArgs>> {
    let Some(source) = args
        .optional_string(0)
        .filter(|source| is_storage_source_token(source))
    else {
        return Ok(None);
    };
    if !is_storage_module_source_token(source) {
        return Ok(None);
    }
    if uses_mutating_flag {
        require_mutating(args, 2, action_label)?;
    }
    let start_index = if uses_mutating_flag { 3 } else { 2 };
    Ok(Some(ModuleCallArgs {
        values: args.iter().skip(start_index).cloned().collect(),
    }))
}

pub(crate) fn delivery_message_args(
    args: &Args,
    action_label: &str,
) -> Result<Option<ModuleCallArgs>> {
    let Some(source) = args
        .optional_string(0)
        .filter(|source| is_delivery_source_token(source))
    else {
        return Ok(None);
    };
    if !is_delivery_module_source_token(source) {
        return Ok(None);
    }
    require_mutating(args, 2, action_label)?;
    let values = args.iter().skip(3).cloned().collect::<Vec<_>>();
    if values.is_empty() {
        bail!("delivery module message arguments are required");
    }
    Ok(Some(ModuleCallArgs { values }))
}

pub(crate) fn delivery_lifecycle_args(args: &Args, action_label: &str) -> Result<Vec<Value>> {
    let start_index = if let Some(source) = args
        .optional_string(0)
        .filter(|source| is_delivery_source_token(source))
    {
        if !is_delivery_module_source_token(source) {
            bail!("delivery node lifecycle actions require delivery module source");
        }
        require_mutating(args, 2, action_label)?;
        3
    } else {
        require_mutating(args, 0, action_label)?;
        0
    };
    Ok(args.iter().skip(start_index).cloned().collect())
}

pub(crate) fn is_storage_module_source(args: &Args) -> bool {
    args.optional_string(0)
        .map(is_storage_module_source_token)
        .unwrap_or(false)
}

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

fn is_delivery_source_token(value: &str) -> bool {
    source_mode_is_token(SourceFamily::Delivery, value)
}

fn is_delivery_module_source_token(value: &str) -> bool {
    effective_source_mode(SourceFamily::Delivery, value) == "module"
}

fn is_storage_source_token(value: &str) -> bool {
    source_mode_is_token(SourceFamily::Storage, value)
}

fn is_storage_module_source_token(value: &str) -> bool {
    effective_source_mode(SourceFamily::Storage, value) == "module"
}

fn require_mutating(args: &Args, index: usize, label: &str) -> Result<()> {
    if args.optional_bool(index) {
        return Ok(());
    }
    bail!("{label} requires mutating diagnostics to be enabled")
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
