use anyhow::{Context as _, Result};
use serde_json::{Value, json};

use crate::{
    inspector::methods::{OperationDomain, OperationMethod, operation_uses_mutating_flag},
    source_routing::{
        Args, CoreSourceMode, SourceFamily, default_endpoint_for_domain,
        default_source_mode_for_domain, source_mode_is_token, storage_rest_source,
    },
};

#[derive(Debug, Clone)]
pub(crate) struct NodeOperationRequest {
    pub(super) domain: String,
    pub(super) source_mode: String,
    pub(super) endpoint: String,
    pub(super) module: String,
    pub(super) method: String,
    pub(super) args: Value,
    pub(super) mutating_enabled: bool,
    pub(super) label: String,
}

impl NodeOperationRequest {
    pub(crate) fn from_call(domain: &str, method: &str, args: Value, label: &str) -> Self {
        Self {
            domain: domain.to_owned(),
            source_mode: String::new(),
            endpoint: String::new(),
            module: String::new(),
            method: method.to_owned(),
            args,
            mutating_enabled: false,
            label: label.to_owned(),
        }
    }

    #[cfg(test)]
    pub(crate) fn args(&self) -> &Value {
        &self.args
    }
}

pub(crate) fn node_operation_request_from_value(value: Value) -> Result<NodeOperationRequest> {
    let object = value
        .as_object()
        .context("node operation request must be a JSON object")?;
    let method = object_string(object, "method")
        .filter(|value| !value.is_empty())
        .context("node operation method is required")?;
    let domain = object_string(object, "domain").unwrap_or_else(|| node_operation_domain(&method));
    let args = object
        .get("args")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let label = object_string(object, "label").unwrap_or_else(|| default_operation_label(&method));
    let mut request = NodeOperationRequest {
        domain,
        source_mode: object_string(object, "sourceMode").unwrap_or_default(),
        endpoint: object_string(object, "endpoint").unwrap_or_default(),
        module: object_string(object, "module").unwrap_or_default(),
        method,
        args,
        mutating_enabled: object
            .get("mutatingEnabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        label,
    };
    request.args = normalized_node_operation_args(&request);
    Ok(request)
}

fn object_string(object: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn node_operation_domain(method: &str) -> String {
    OperationDomain::from_method(method).as_str().to_owned()
}

fn default_operation_label(method: &str) -> String {
    OperationMethod::from_str(method).map_or_else(String::new, |method| method.label().to_owned())
}

pub(super) fn node_operation_backend(request: &NodeOperationRequest) -> String {
    if !request.source_mode.is_empty() {
        return request.source_mode.clone();
    }
    if !request.module.is_empty() {
        return request.module.clone();
    }
    if !request.endpoint.is_empty() {
        return request.endpoint.clone();
    }
    request
        .args
        .as_array()
        .and_then(|values| values.first())
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("direct")
        .to_owned()
}

pub(super) fn node_operation_context(request: &NodeOperationRequest) -> Value {
    let mut context = serde_json::Map::new();
    if !request.endpoint.is_empty() {
        context.insert("endpoint".to_owned(), json!(request.endpoint));
    }
    if !request.source_mode.is_empty() {
        context.insert("source".to_owned(), json!(request.source_mode));
    }
    if request.mutating_enabled {
        context.insert("mutatingEnabled".to_owned(), json!(true));
    }
    if request.domain == "storage"
        && let Ok(args) = Args::new(request.args.clone())
        && let Ok(source) = storage_rest_source(&args)
    {
        context.insert("endpoint".to_owned(), json!(source.endpoint));
        match request.method.as_str() {
            "storageDownloadToUrl" => {
                if let Some(cid) = args.optional_string(source.next_index + 1) {
                    context.insert("cid".to_owned(), json!(cid));
                }
                if let Some(path) = args.optional_string(source.next_index + 2) {
                    context.insert("path".to_owned(), json!(path));
                }
                context.insert(
                    "source".to_owned(),
                    json!(if args.optional_bool(source.next_index + 3) {
                        "local"
                    } else {
                        "network"
                    }),
                );
            }
            "storageUploadUrl" => {
                if let Some(path) = args.optional_string(source.next_index + 1) {
                    context.insert("path".to_owned(), json!(path));
                }
            }
            "storageFetch" | "storageRemove" => {
                if let Some(cid) = args.optional_string(source.next_index + 1) {
                    context.insert("cid".to_owned(), json!(cid));
                }
            }
            "storageDownloadManifest" => {
                let cid_index = if matches!(args.value(source.next_index), Some(Value::Bool(_))) {
                    source.next_index + 1
                } else {
                    source.next_index
                };
                if let Some(cid) = args.optional_string(cid_index) {
                    context.insert("cid".to_owned(), json!(cid));
                }
            }
            _ => {}
        }
    }
    Value::Object(context)
}

fn normalized_node_operation_args(request: &NodeOperationRequest) -> Value {
    if request.source_mode.is_empty() && request.endpoint.is_empty() {
        return request.args.clone();
    }
    let Some(values) = request.args.as_array() else {
        return request.args.clone();
    };
    if node_operation_args_have_source(request, values) {
        return request.args.clone();
    }
    let mode = if request.source_mode.is_empty() {
        default_source_mode_for_domain(&request.domain).to_owned()
    } else {
        request.source_mode.clone()
    };
    let endpoint = if request.endpoint.is_empty() {
        default_endpoint_for_domain(&request.domain).to_owned()
    } else {
        request.endpoint.clone()
    };
    let mut normalized = vec![json!(mode), json!(endpoint)];
    if operation_uses_mutating_flag(&request.method) {
        normalized.push(json!(request.mutating_enabled));
    }
    let payload_start = if storage_or_delivery_domain(&request.domain)
        && values
            .first()
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|first| {
                first == endpoint || first.starts_with("http://") || first.starts_with("https://")
            }) {
        1
    } else {
        0
    };
    normalized.extend(values.iter().skip(payload_start).cloned());
    Value::Array(normalized)
}

fn node_operation_args_have_source(request: &NodeOperationRequest, values: &[Value]) -> bool {
    let Some(first) = values
        .first()
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    if storage_or_delivery_domain(&request.domain) {
        return source_mode_is_token(SourceFamily::Storage, first)
            || source_mode_is_token(SourceFamily::Delivery, first);
    }
    first == request.endpoint
        || first.starts_with("http://")
        || first.starts_with("https://")
        || CoreSourceMode::from_token(first).is_some()
}

fn storage_or_delivery_domain(domain: &str) -> bool {
    OperationDomain::from_method(domain).is_storage_or_delivery()
}
