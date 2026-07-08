use anyhow::{Context as _, Result};
use serde_json::{Value, json};

use crate::source_routing::{
    Args, SourceArgsNormalization, normalized_source_args, storage_rest_source,
};

use super::spec::{OperationDomain, OperationExclusiveGroup, OperationMethod};

#[derive(Debug, Clone, Default)]
pub(super) struct OperationSourceSelection {
    source_mode: String,
    endpoint: String,
    module: String,
    mutating_enabled: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct NodeOperationRequest {
    pub(super) domain: String,
    pub(super) method: OperationMethod,
    pub(super) source: OperationSourceSelection,
    pub(super) args: Value,
    pub(super) label: String,
}

impl NodeOperationRequest {
    pub(crate) fn from_call(method: OperationMethod, args: Value, label: &str) -> Self {
        Self {
            domain: method.domain().as_str().to_owned(),
            method,
            source: OperationSourceSelection::default(),
            args,
            label: label.to_owned(),
        }
    }

    pub(crate) fn method_name(&self) -> &'static str {
        self.method.as_str()
    }

    pub(super) fn method(&self) -> OperationMethod {
        self.method
    }

    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    pub(crate) fn cancellable(&self) -> bool {
        self.method.cancellable()
    }

    pub(crate) fn exclusive_group(&self) -> Option<OperationExclusiveGroup> {
        self.method.exclusive_group()
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
    let method = OperationMethod::from_str(&method)
        .with_context(|| format!("unknown node operation method `{method}`"))?;
    let domain =
        object_string(object, "domain").unwrap_or_else(|| method.domain().as_str().to_owned());
    let args = object
        .get("args")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let label = object_string(object, "label").unwrap_or_else(|| method.label().to_owned());
    let source = OperationSourceSelection::from_object(object);
    let mut request = NodeOperationRequest {
        domain,
        method,
        source,
        args,
        label,
    };
    request.args = request
        .source
        .normalized_args(&request.domain, request.method, &request.args);
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

impl OperationSourceSelection {
    fn from_object(object: &serde_json::Map<String, Value>) -> Self {
        Self {
            source_mode: object_string(object, "sourceMode").unwrap_or_default(),
            endpoint: object_string(object, "endpoint").unwrap_or_default(),
            module: object_string(object, "module").unwrap_or_default(),
            mutating_enabled: object
                .get("mutatingEnabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }
    }

    fn normalized_args(&self, domain: &str, method: OperationMethod, args: &Value) -> Value {
        normalized_source_args(SourceArgsNormalization {
            domain,
            source_mode: &self.source_mode,
            endpoint: &self.endpoint,
            args,
            inserts_mutating_flag: method.uses_mutating_flag(),
            mutating_enabled: self.mutating_enabled,
        })
    }

    fn backend_from_args(&self, args: &Value) -> String {
        if !self.source_mode.is_empty() {
            return self.source_mode.clone();
        }
        if !self.module.is_empty() {
            return self.module.clone();
        }
        if !self.endpoint.is_empty() {
            return self.endpoint.clone();
        }
        args.as_array()
            .and_then(|values| values.first())
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("direct")
            .to_owned()
    }

    fn add_context_fields(&self, context: &mut serde_json::Map<String, Value>) {
        if !self.endpoint.is_empty() {
            context.insert("endpoint".to_owned(), json!(self.endpoint));
        }
        if !self.source_mode.is_empty() {
            context.insert("source".to_owned(), json!(self.source_mode));
        }
        if self.mutating_enabled {
            context.insert("mutatingEnabled".to_owned(), json!(true));
        }
    }
}

pub(super) fn node_operation_backend(request: &NodeOperationRequest) -> String {
    request.source.backend_from_args(&request.args)
}

pub(super) fn node_operation_context(request: &NodeOperationRequest) -> Value {
    let mut context = serde_json::Map::new();
    request.source.add_context_fields(&mut context);
    if request.domain == OperationDomain::Storage.as_str()
        && let Ok(args) = Args::new(request.args.clone())
        && let Ok(source) = storage_rest_source(&args)
    {
        context.insert("endpoint".to_owned(), json!(source.endpoint));
        match request.method_name() {
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
