use anyhow::{Context as _, Result};
use serde_json::{Value, json};

use crate::source_routing::{SourceArgsNormalization, normalized_source_args};

use super::spec::{OperationDomain, OperationExclusiveGroup, OperationMethod};
use super::storage;

#[derive(Debug, Clone, Default)]
pub(super) struct OperationSourceSelection {
    source_mode: String,
    endpoint: String,
    module: String,
    mutating_enabled: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeOperationRequest {
    pub(super) domain: String,
    pub(super) method: OperationMethod,
    pub(super) source: OperationSourceSelection,
    pub(super) args: Value,
    pub(super) label: String,
}

impl RuntimeOperationRequest {
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

pub(crate) fn runtime_operation_request_from_value(
    value: Value,
) -> Result<RuntimeOperationRequest> {
    let object = value
        .as_object()
        .context("runtime operation request must be a JSON object")?;
    let method = object_string(object, "method")
        .filter(|value| !value.is_empty())
        .context("runtime operation method is required")?;
    let method = OperationMethod::from_str(&method)
        .with_context(|| format!("unknown runtime operation method `{method}`"))?;
    let domain =
        object_string(object, "domain").unwrap_or_else(|| method.domain().as_str().to_owned());
    let args = object
        .get("args")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let label = object_string(object, "label").unwrap_or_else(|| method.label().to_owned());
    let source = OperationSourceSelection::from_object(object);
    let mut request = RuntimeOperationRequest {
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

pub(super) fn runtime_operation_backend(request: &RuntimeOperationRequest) -> String {
    request.source.backend_from_args(&request.args)
}

pub(super) fn runtime_operation_context(request: &RuntimeOperationRequest) -> Value {
    let mut context = serde_json::Map::new();
    request.source.add_context_fields(&mut context);
    if request.method.domain() == OperationDomain::Storage {
        storage::add_operation_context(request, &mut context);
    }
    Value::Object(context)
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};
    use serde_json::json;

    use super::*;

    #[test]
    fn runtime_operation_request_normalizes_storage_source_and_context() -> Result<()> {
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageDownloadToUrl",
            "sourceMode": "rest",
            "endpoint": "http://storage.local/api",
            "mutatingEnabled": true,
            "args": ["cid-a", "/tmp/cid-a.bin", false],
            "label": "Download"
        }))?;

        if request.args
            != json!([
                "rest",
                "http://storage.local/api",
                true,
                "cid-a",
                "/tmp/cid-a.bin",
                false
            ])
        {
            bail!("unexpected normalized args: {:?}", request.args);
        }
        if runtime_operation_context(&request)
            != json!({
                "endpoint": "http://storage.local/api",
                "source": "network",
                "mutatingEnabled": true,
                "cid": "cid-a",
                "path": "/tmp/cid-a.bin"
            })
        {
            bail!("unexpected runtime operation context");
        }
        Ok(())
    }

    #[test]
    fn runtime_operation_context_keeps_non_storage_context_generic() -> Result<()> {
        let request = runtime_operation_request_from_value(json!({
            "domain": "delivery",
            "method": "deliverySend",
            "sourceMode": "rest",
            "endpoint": "http://delivery.local",
            "mutatingEnabled": true,
            "args": ["/topic", "hello"]
        }))?;

        if runtime_operation_context(&request)
            != json!({
                "endpoint": "http://delivery.local",
                "source": "rest",
                "mutatingEnabled": true
            })
        {
            bail!("unexpected delivery context");
        }
        Ok(())
    }
}
