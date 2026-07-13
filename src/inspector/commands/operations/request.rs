use anyhow::{Context as _, Result, bail};
use serde_json::{Map, Value, json};

use crate::{source_routing::NodeOperationRequest, support::args::Args};

use super::delivery;
use super::spec::{OperationDomain, OperationExclusiveGroup, OperationMethod};
use super::storage;

#[derive(Debug, Clone)]
pub(crate) struct RuntimeOperationRequest {
    pub(super) domain: String,
    pub(super) method: OperationMethod,
    node_request: Option<NodeOperationRequest>,
    pub(super) args: Value,
    pub(super) label: String,
}

impl RuntimeOperationRequest {
    pub(crate) fn from_call(method: OperationMethod, args: Value, label: &str) -> Result<Self> {
        let node_request = if node_domain(method.domain()) {
            Some(NodeOperationRequest::from_bridge_args(&Args::new(
                args.clone(),
            )?)?)
        } else {
            None
        };
        Ok(Self {
            domain: method.domain().as_str().to_owned(),
            method,
            node_request,
            args,
            label: label.to_owned(),
        })
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

    pub(super) fn node_request(&self) -> Result<&NodeOperationRequest> {
        self.node_request
            .as_ref()
            .context("typed node operation request is required")
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
    if let Some(domain) = object_string(object, "domain")
        && domain != method.domain().as_str()
    {
        bail!(
            "runtime operation domain `{domain}` does not match method `{}`",
            method.as_str()
        );
    }
    let node_request = if node_domain(method.domain()) {
        Some(NodeOperationRequest::from_value(&value)?)
    } else {
        None
    };
    let args = object
        .get("args")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let request = RuntimeOperationRequest {
        domain: method.domain().as_str().to_owned(),
        method,
        node_request,
        args,
        label: object_string(object, "label").unwrap_or_else(|| method.label().to_owned()),
    };
    validate_node_request(&request)?;
    Ok(request)
}

fn object_string(object: &Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn node_domain(domain: OperationDomain) -> bool {
    matches!(domain, OperationDomain::Storage | OperationDomain::Delivery)
}

fn validate_node_request(request: &RuntimeOperationRequest) -> Result<()> {
    match request.method.domain() {
        OperationDomain::Storage => storage::validate(request),
        OperationDomain::Delivery => delivery::validate(request),
        _ => Ok(()),
    }
}

pub(super) fn runtime_operation_backend(request: &RuntimeOperationRequest) -> String {
    if let Some(node_request) = &request.node_request {
        return node_request.source_mode().to_owned();
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

pub(super) fn runtime_operation_context(request: &RuntimeOperationRequest) -> Value {
    let mut context = Map::new();
    if let Some(node_request) = &request.node_request {
        context.insert("source".to_owned(), json!(node_request.source_mode()));
        if let Some(endpoint) = node_request
            .input("rest_endpoint")
            .or_else(|| node_request.input("rpc_endpoint"))
        {
            context.insert("endpoint".to_owned(), json!(endpoint));
        }
        if node_request.mutating_enabled() {
            context.insert("mutatingEnabled".to_owned(), json!(true));
        }
    }
    match request.method.domain() {
        OperationDomain::Storage => storage::add_operation_context(request, &mut context),
        OperationDomain::Delivery => delivery::add_operation_context(request, &mut context),
        _ => {}
    }
    Value::Object(context)
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};
    use serde_json::json;

    use super::*;

    #[test]
    fn runtime_operation_request_parses_typed_storage_source_and_context() -> Result<()> {
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageDownloadToUrl",
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://storage.local/api" }
            },
            "mutating_enabled": true,
            "payload": {
                "cid": "cid-a",
                "path": "/tmp/cid-a.bin",
                "local_only": false
            },
            "label": "Download"
        }))?;

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
    fn runtime_operation_context_keeps_delivery_context_typed() -> Result<()> {
        let request = runtime_operation_request_from_value(json!({
            "domain": "delivery",
            "method": "deliverySend",
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://delivery.local" }
            },
            "mutating_enabled": true,
            "payload": { "topic": "/topic", "payload": "hello" }
        }))?;

        if runtime_operation_context(&request)
            != json!({
                "endpoint": "http://delivery.local",
                "source": "rest",
                "mutatingEnabled": true,
                "contentTopic": "/topic",
                "bytes": "5"
            })
        {
            bail!("unexpected delivery context");
        }
        Ok(())
    }
}
