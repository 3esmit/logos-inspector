use anyhow::{Context as _, Result, bail};
use serde_json::{Map, Value, json};

use crate::{
    modules::logos_core::ModuleTransportKind,
    source_routing::{
        CoreEndpointMode, DeliverySourceMode, NodeOperationRequest, StorageSourceMode,
    },
    support::args::Args,
};

use super::identity::ClientRequestId;
use super::spec::{
    OperationCommand, OperationDefinition, OperationDomain, OperationExclusiveGroup,
    OperationMethod, OperationPolicyDefinition, operation_definition,
};
use super::storage;
use super::{blockchain, delivery};

#[derive(Debug, Clone)]
pub(crate) struct RuntimeOperationRequest {
    definition: OperationDefinition,
    node_request: Option<NodeOperationRequest>,
    client_request_id: Option<ClientRequestId>,
    pub(super) args: Value,
    pub(super) label: String,
}

impl RuntimeOperationRequest {
    pub(crate) fn from_call(method: OperationMethod, args: Value, label: &str) -> Result<Self> {
        let definition = operation_definition(method)
            .with_context(|| format!("runtime operation definition is missing for {method:?}"))?;
        let node_request = if node_domain(definition.domain()) {
            Some(NodeOperationRequest::from_bridge_args(&Args::new(
                args.clone(),
            )?)?)
        } else {
            None
        };
        let request = Self {
            definition,
            node_request,
            client_request_id: None,
            args,
            label: label.to_owned(),
        };
        validate_node_request(&request)?;
        Ok(request)
    }

    pub(crate) fn method_name(&self) -> &'static str {
        self.definition.name()
    }

    pub(super) fn method(&self) -> OperationMethod {
        self.definition.method()
    }

    pub(super) fn domain_name(&self) -> &'static str {
        self.definition.domain().as_str()
    }

    pub(super) fn command(&self) -> OperationCommand {
        self.definition.command()
    }

    pub(super) fn policy_definition(&self) -> OperationPolicyDefinition {
        self.definition.policy()
    }

    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    pub(super) fn client_request_id(&self) -> Option<&ClientRequestId> {
        self.client_request_id.as_ref()
    }

    pub(crate) fn cancellable(&self) -> bool {
        self.definition.is_cancellable()
    }

    pub(crate) fn exclusive_group(&self) -> Option<OperationExclusiveGroup> {
        self.definition.exclusive_group()
    }

    pub(super) fn node_request(&self) -> Result<&NodeOperationRequest> {
        self.node_request
            .as_ref()
            .context("typed node operation request is required")
    }

    pub(super) fn requested_module_transport(&self) -> Result<Option<ModuleTransportKind>> {
        let transport = match self.definition.domain() {
            OperationDomain::Storage => {
                match StorageSourceMode::from_token(self.node_request()?.source_mode()) {
                    StorageSourceMode::Module => Some(ModuleTransportKind::Module),
                    StorageSourceMode::LogoscoreCli => Some(ModuleTransportKind::LogoscoreCli),
                    StorageSourceMode::Rest
                    | StorageSourceMode::Metrics
                    | StorageSourceMode::Unsupported => None,
                }
            }
            OperationDomain::Delivery => {
                match DeliverySourceMode::from_token(self.node_request()?.source_mode()) {
                    DeliverySourceMode::Module => Some(ModuleTransportKind::Module),
                    DeliverySourceMode::LogoscoreCli => Some(ModuleTransportKind::LogoscoreCli),
                    DeliverySourceMode::Rest
                    | DeliverySourceMode::Metrics
                    | DeliverySourceMode::NetworkMonitor
                    | DeliverySourceMode::Unsupported => None,
                }
            }
            OperationDomain::Blockchain => {
                let args = Args::new(self.args.clone())?;
                match args.source_endpoint(0, "node endpoint")?.mode {
                    CoreEndpointMode::Module => Some(ModuleTransportKind::Module),
                    CoreEndpointMode::LogoscoreCli => Some(ModuleTransportKind::LogoscoreCli),
                    CoreEndpointMode::Rpc => None,
                }
            }
            OperationDomain::LocalNodes | OperationDomain::Wallet | OperationDomain::Execution => {
                None
            }
        };
        Ok(transport)
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
    let definition = operation_definition(method)
        .with_context(|| format!("runtime operation definition is missing for {method:?}"))?;
    if let Some(domain) = object_string(object, "domain")
        && domain != definition.domain().as_str()
    {
        bail!(
            "runtime operation domain `{domain}` does not match method `{}`",
            definition.name()
        );
    }
    let node_request = if node_domain(definition.domain()) {
        Some(NodeOperationRequest::from_value(&value)?)
    } else {
        None
    };
    let args = object
        .get("args")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let request = RuntimeOperationRequest {
        definition,
        node_request,
        client_request_id: optional_id(object, "clientRequestId", ClientRequestId::parse)?,
        args,
        label: object_string(object, "label").unwrap_or_else(|| definition.label().to_owned()),
    };
    validate_node_request(&request)?;
    Ok(request)
}

fn optional_id<T>(
    object: &Map<String, Value>,
    key: &str,
    parse: impl FnOnce(&str) -> Result<T>,
) -> Result<Option<T>> {
    let Some(value) = object.get(key) else {
        return Ok(None);
    };
    let value = value
        .as_str()
        .with_context(|| format!("runtime operation {key} must be a string"))?;
    parse(value).map(Some)
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
    match request.command() {
        OperationCommand::Storage(command) => storage::validate(command, request),
        OperationCommand::Delivery(command) => delivery::validate(command, request),
        OperationCommand::Blockchain(command) => blockchain::validate(command, request),
        _ => Ok(()),
    }
}

pub(super) fn runtime_operation_backend(request: &RuntimeOperationRequest) -> String {
    if let Some(node_request) = &request.node_request {
        return node_request.source_mode().to_owned();
    }
    if matches!(request.command(), OperationCommand::Blockchain(_)) {
        return Args::new(request.args.clone())
            .and_then(|args| {
                args.source_endpoint(0, "node endpoint")
                    .map(|source| source.mode.as_str().to_owned())
            })
            .unwrap_or_else(|_| "direct".to_owned());
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

pub(super) fn runtime_operation_pending_module(
    request: &RuntimeOperationRequest,
) -> Option<&'static str> {
    let node_request = request.node_request.as_ref()?;
    match request.definition.domain() {
        OperationDomain::Storage
            if matches!(
                StorageSourceMode::from_token(node_request.source_mode()),
                StorageSourceMode::Module | StorageSourceMode::LogoscoreCli
            ) =>
        {
            Some(crate::source_routing::storage_layer::managed_contract().module_id())
        }
        OperationDomain::Delivery
            if matches!(
                DeliverySourceMode::from_token(node_request.source_mode()),
                DeliverySourceMode::Module | DeliverySourceMode::LogoscoreCli
            ) =>
        {
            Some(crate::source_routing::messaging_layer::managed_contract().module_id())
        }
        OperationDomain::Storage | OperationDomain::Delivery => None,
        OperationDomain::LocalNodes
        | OperationDomain::Wallet
        | OperationDomain::Blockchain
        | OperationDomain::Execution => None,
    }
}

pub(super) fn runtime_operation_context(request: &RuntimeOperationRequest) -> Result<Value> {
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
    match request.command() {
        OperationCommand::Storage(command) => {
            storage::add_operation_context(command, request, &mut context)?;
        }
        OperationCommand::Delivery(command) => {
            delivery::add_operation_context(command, request, &mut context)?;
        }
        OperationCommand::Blockchain(command) => {
            blockchain::add_operation_context(command, request, &mut context)?;
        }
        _ => {}
    }
    Ok(Value::Object(context))
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
            "clientRequestId": "storage-client-1",
            "label": "Download"
        }))?;

        if runtime_operation_context(&request)?
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
        if request.client_request_id().map(ClientRequestId::as_str) != Some("storage-client-1") {
            bail!("client request identity was not parsed independently");
        }
        Ok(())
    }

    #[test]
    fn backup_catalog_upload_context_keeps_catalog_identity() -> Result<()> {
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageUploadBackupCatalogEntry",
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://storage.local/api" }
            },
            "mutating_enabled": true,
            "payload": {
                "backup_catalog_id": "backup-1",
                "block_size": 65536
            }
        }))?;

        if runtime_operation_context(&request)?
            != json!({
                "endpoint": "http://storage.local/api",
                "source": "rest",
                "mutatingEnabled": true,
                "backupCatalogId": "backup-1"
            })
        {
            bail!("unexpected backup catalog upload context");
        }
        Ok(())
    }

    #[test]
    fn backup_catalog_download_context_keeps_adapter_and_scope_identity() -> Result<()> {
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageDownloadBackupCatalogEntry",
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://storage.local/api" }
            },
            "mutating_enabled": false,
            "payload": { "cid": "cid-remote", "local_only": false }
        }))?;

        if runtime_operation_context(&request)?
            != json!({
                "endpoint": "http://storage.local/api",
                "source": "rest",
                "cid": "cid-remote",
                "downloadScope": "network"
            })
        {
            bail!("unexpected backup catalog download context");
        }
        if runtime_operation_backend(&request) != "rest" {
            bail!("backup catalog download backend identity drifted");
        }
        Ok(())
    }

    #[test]
    fn payload_upload_context_keeps_filename_identity() -> Result<()> {
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageUploadPayload",
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://storage.local/api" }
            },
            "mutating_enabled": true,
            "payload": {
                "filename": "shared-idl.json",
                "payload": { "kind": "shared-idl" },
                "block_size": 65536
            }
        }))?;

        if runtime_operation_context(&request)?
            != json!({
                "endpoint": "http://storage.local/api",
                "source": "rest",
                "mutatingEnabled": true,
                "filename": "shared-idl.json"
            })
        {
            bail!("unexpected payload upload context");
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

        if runtime_operation_context(&request)?
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

    #[test]
    fn pending_module_detection_normalizes_storage_and_delivery_aliases() -> Result<()> {
        let storage = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageUploadUrl",
            "adapter": { "source_mode": "basecamp-module", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "path": "/tmp/storage-alias" }
        }))?;
        let delivery = runtime_operation_request_from_value(json!({
            "domain": "delivery",
            "method": "deliverySend",
            "adapter": { "source_mode": "logoscore-cli", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "topic": "/topic", "payload": "hello" }
        }))?;

        anyhow::ensure!(
            runtime_operation_pending_module(&storage) == Some("storage_module")
                && runtime_operation_pending_module(&delivery) == Some("delivery_module"),
            "source aliases did not preserve pending module identity"
        );
        Ok(())
    }

    #[test]
    fn requested_module_transport_preserves_storage_adapter_identity() -> Result<()> {
        let cases = [
            ("module", Some(ModuleTransportKind::Module)),
            ("logoscore_cli", Some(ModuleTransportKind::LogoscoreCli)),
            ("rest", None),
        ];

        for (source_mode, expected) in cases {
            let inputs = if source_mode == "rest" {
                json!({ "rest_endpoint": "http://storage.local/api" })
            } else {
                json!({})
            };
            let request = RuntimeOperationRequest::from_call(
                OperationMethod::StorageManifests,
                json!([{
                    "adapter": { "source_mode": source_mode, "inputs": inputs },
                    "payload": {}
                }]),
                "Storage manifests",
            )?;

            if request.requested_module_transport()? != expected {
                bail!("storage source `{source_mode}` lost transport identity");
            }
        }
        Ok(())
    }

    #[test]
    fn requested_module_transport_preserves_delivery_adapter_identity() -> Result<()> {
        let cases = [
            ("module", Some(ModuleTransportKind::Module)),
            ("logoscore_cli", Some(ModuleTransportKind::LogoscoreCli)),
            ("rest", None),
        ];

        for (source_mode, expected) in cases {
            let inputs = if source_mode == "rest" {
                json!({ "rest_endpoint": "http://delivery.local" })
            } else {
                json!({})
            };
            let request = RuntimeOperationRequest::from_call(
                OperationMethod::DeliverySubscribe,
                json!([{
                    "adapter": { "source_mode": source_mode, "inputs": inputs },
                    "mutating_enabled": true,
                    "payload": { "topic": "/topic" }
                }]),
                "Delivery subscribe",
            )?;

            if request.requested_module_transport()? != expected {
                bail!("delivery source `{source_mode}` lost transport identity");
            }
        }
        Ok(())
    }

    #[test]
    fn requested_module_transport_preserves_blockchain_adapter_identity() -> Result<()> {
        let cases = [
            (json!(["module"]), Some(ModuleTransportKind::Module)),
            (
                json!(["logoscore_cli"]),
                Some(ModuleTransportKind::LogoscoreCli),
            ),
            (json!(["http://blockchain.local"]), None),
        ];

        for (args, expected) in cases {
            let request = RuntimeOperationRequest::from_call(
                OperationMethod::BlockchainNode,
                args,
                "Blockchain node",
            )?;

            if request.requested_module_transport()? != expected {
                bail!("blockchain source lost transport identity");
            }
        }
        Ok(())
    }

    #[test]
    fn blockchain_operation_context_freezes_source_range_and_target_identity() -> Result<()> {
        let cases = [
            (
                "blockchainNode",
                json!(["rpc", "http://blockchain.local"]),
                json!({
                    "source": "rpc",
                    "endpoint": "http://blockchain.local"
                }),
            ),
            (
                "blockchainBlocks",
                json!(["module", 10, 20, 5]),
                json!({
                    "source": "module",
                    "slotFrom": 10,
                    "slotTo": 20,
                    "slotRange": "10:20",
                    "limit": 5
                }),
            ),
            (
                "blockchainLiveBlocks",
                json!(["logoscore_cli", 30, 40]),
                json!({
                    "source": "logoscore_cli",
                    "slotFrom": 30,
                    "slotTo": 40,
                    "slotRange": "30:40",
                    "limit": 50
                }),
            ),
            (
                "blockchainBlock",
                json!(["module", "block-a"]),
                json!({
                    "source": "module",
                    "blockId": "block-a"
                }),
            ),
            (
                "blockchainTransaction",
                json!(["rpc", "http://blockchain.local", "tx-a"]),
                json!({
                    "source": "rpc",
                    "endpoint": "http://blockchain.local",
                    "transactionId": "tx-a"
                }),
            ),
        ];

        for (method, args, expected_context) in cases {
            let request = runtime_operation_request_from_value(json!({
                "domain": "blockchain",
                "method": method,
                "args": args,
                "clientRequestId": format!("client-{method}")
            }))?;
            if runtime_operation_context(&request)? != expected_context {
                bail!("blockchain operation `{method}` lost context identity");
            }
            let expected_backend = expected_context
                .get("source")
                .and_then(Value::as_str)
                .context("expected source")?;
            if runtime_operation_backend(&request) != expected_backend {
                bail!("blockchain operation `{method}` lost backend identity");
            }
        }
        Ok(())
    }

    #[test]
    fn blockchain_operation_request_rejects_incomplete_range_before_admission() {
        let result = runtime_operation_request_from_value(json!({
            "domain": "blockchain",
            "method": "blockchainBlocks",
            "args": ["module", 10]
        }));
        assert!(result.is_err());
    }

    #[test]
    fn runtime_operation_request_rejects_unknown_method() -> Result<()> {
        let result = runtime_operation_request_from_value(json!({
            "domain": "wallet",
            "method": "unknownWalletMethod",
            "args": []
        }));

        let Err(error) = result else {
            bail!("unknown operation method should fail");
        };
        if !error
            .to_string()
            .contains("unknown runtime operation method `unknownWalletMethod`")
        {
            bail!("unexpected unknown-method error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn runtime_operation_request_rejects_domain_method_mismatch() -> Result<()> {
        let result = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "localWalletAccounts",
            "args": ["default"]
        }));

        let Err(error) = result else {
            bail!("domain and operation command mismatch should fail");
        };
        if !error
            .to_string()
            .contains("domain `storage` does not match method `localWalletAccounts`")
        {
            bail!("unexpected domain-method error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn runtime_operation_call_rejects_invalid_node_request() -> Result<()> {
        let result = RuntimeOperationRequest::from_call(
            OperationMethod::StorageFetch,
            json!([{
                "adapter": {
                    "source_mode": "rest",
                    "inputs": { "rest_endpoint": "http://storage.local/api" }
                },
                "mutating_enabled": false,
                "payload": { "cid": "cid-a" }
            }]),
            "Storage fetch",
        );

        let Err(error) = result else {
            bail!("invalid node operation call should fail during request construction");
        };
        if !error
            .to_string()
            .contains("requires mutating diagnostics to be enabled")
        {
            bail!("unexpected node request error: {error:#}");
        }
        Ok(())
    }
}
