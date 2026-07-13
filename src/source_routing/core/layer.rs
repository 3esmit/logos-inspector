use std::num::NonZeroUsize;

use anyhow::Result;
use reqwest::{Client, Response};
use serde_json::{Value, json};

use super::adapters::{self, BLOCKCHAIN_MODULE};
use crate::{
    blockchain::{BlockchainLiveBlocksReport, BlockchainNodeReport, channels::ChannelScanReport},
    modules::ModuleReport,
    modules::logos_core::{LogoscoreCliRuntime, ModuleTransportKind, SharedModuleTransport},
    rpc::RawRpcReport,
    source_routing::adapter::{
        AdapterConnectionType, AdapterInputPolicy, ManagedModuleCallSpec, ManagedNodeAction,
        ManagedNodeContract, SourceAdapterPolicy, SourceModePolicy,
    },
};

static MANAGED_CONTRACT: ManagedNodeContract = ManagedNodeContract::new(
    BLOCKCHAIN_MODULE,
    ensure_managed_module,
    call_managed_module,
    managed_call_spec,
    None,
    None,
);

#[must_use]
pub(crate) const fn managed_contract() -> &'static ManagedNodeContract {
    &MANAGED_CONTRACT
}

#[must_use]
pub(crate) const fn module_id() -> &'static str {
    BLOCKCHAIN_MODULE
}

pub(crate) fn ensure_managed_module(runtime: &LogoscoreCliRuntime) -> Result<()> {
    runtime.ensure_module_loaded(module_id())
}

pub(crate) fn call_managed_module(
    runtime: &LogoscoreCliRuntime,
    method: &str,
    signature: &str,
    args: &[String],
) -> Result<Value> {
    runtime.call_checked(module_id(), method, signature, args)
}

pub(crate) async fn diagnostic_report(
    module_transport: &SharedModuleTransport,
    transport: ModuleTransportKind,
    address: Option<&str>,
) -> ModuleReport {
    crate::modules::blockchain_module_report(module_transport, transport, address).await
}

#[must_use]
pub(crate) fn managed_call_spec(
    action: ManagedNodeAction,
    config_path: &str,
) -> Option<ManagedModuleCallSpec> {
    match action {
        ManagedNodeAction::Start => Some(ManagedModuleCallSpec::new(
            "start",
            "start(QString,QString)",
            vec![config_path.to_owned(), String::new()],
        )),
        ManagedNodeAction::Stop => Some(ManagedModuleCallSpec::new("stop", "stop()", Vec::new())),
        ManagedNodeAction::Initialize | ManagedNodeAction::Destroy => None,
    }
}

#[must_use]
pub(crate) fn managed_config(
    network_id: &str,
    data_dir: &str,
    endpoint: Option<&str>,
    port: Option<u16>,
) -> Value {
    json!({
        "network_id": network_id,
        "node": "bedrock",
        "data_dir": data_dir,
        "endpoint": endpoint,
        "port": port,
    })
}

const RPC_INPUTS: &[AdapterInputPolicy] = &[AdapterInputPolicy {
    key: "rpc_endpoint",
    label: "RPC URL",
    required: true,
}];

const BEDROCK_CAPABILITIES: &[&str] = &[
    "l1.blocks.read",
    "l1.transactions.read",
    "l1.channels.read",
    "l1.wallet_balance.read",
    "l1.live_blocks.observe",
];

pub(crate) const BEDROCK_SOURCE_MODES: &[SourceModePolicy] = &[
    SourceModePolicy {
        key: "rpc",
        aliases: &[
            "rpc",
            "direct-rpc",
            "direct rpc",
            "standalone",
            "standalone-rpc",
            "standalone rpc",
        ],
        effective: "rpc",
        label_key: "direct_rpc",
        label: "Direct RPC",
        source_label: "Direct RPC",
        summary: "Use configured standalone RPC endpoint",
        implemented: true,
        adapter: SourceAdapterPolicy {
            connector_id: "direct_l1_rpc",
            connection_type: AdapterConnectionType::Rpc,
            target: "rpc_endpoint",
            module_id: None,
            inputs: RPC_INPUTS,
            capabilities: BEDROCK_CAPABILITIES,
            supports_cid_probe: false,
            supports_mutating_diagnostics: false,
            capability_scopes: &["l1"],
            endpoint_role: Some("node_url"),
        },
    },
    SourceModePolicy {
        key: "module",
        aliases: &["module", "basecamp", "basecamp-module", "basecamp module"],
        effective: "module",
        label_key: "basecamp_module",
        label: "Basecamp module",
        source_label: "Basecamp module",
        summary: "Use the host-provided Bedrock module API",
        implemented: true,
        adapter: SourceAdapterPolicy {
            connector_id: BLOCKCHAIN_MODULE,
            connection_type: AdapterConnectionType::Module,
            target: "module",
            module_id: Some(BLOCKCHAIN_MODULE),
            inputs: &[],
            capabilities: BEDROCK_CAPABILITIES,
            supports_cid_probe: false,
            supports_mutating_diagnostics: true,
            capability_scopes: &["l1", "wallet.l1"],
            endpoint_role: None,
        },
    },
    SourceModePolicy {
        key: "logoscore_cli",
        aliases: &["logoscore_cli", "logoscore-cli", "logoscore cli"],
        effective: "module",
        label_key: "logoscore_cli",
        label: "LogosCore CLI",
        source_label: "LogosCore CLI (Bedrock)",
        summary: "Call blockchain_module with logoscore call",
        implemented: true,
        adapter: SourceAdapterPolicy {
            connector_id: "logoscore_cli_blockchain_module",
            connection_type: AdapterConnectionType::LogoscoreCli,
            target: "module",
            module_id: Some(BLOCKCHAIN_MODULE),
            inputs: &[],
            capabilities: BEDROCK_CAPABILITIES,
            supports_cid_probe: false,
            supports_mutating_diagnostics: true,
            capability_scopes: &["l1", "wallet.l1"],
            endpoint_role: None,
        },
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BedrockAdapter<'a> {
    Rpc { endpoint: &'a str },
    Module { transport: ModuleTransportKind },
}

impl<'a> BedrockAdapter<'a> {
    #[must_use]
    pub(crate) const fn rpc(endpoint: &'a str) -> Self {
        Self::Rpc { endpoint }
    }

    #[must_use]
    pub(crate) const fn module(transport: ModuleTransportKind) -> Self {
        Self::Module { transport }
    }
}

pub(crate) async fn node_report(
    adapter: BedrockAdapter<'_>,
    module_transport: &SharedModuleTransport,
) -> Result<BlockchainNodeReport> {
    match adapter {
        BedrockAdapter::Rpc { endpoint } => {
            Ok(crate::blockchain::blockchain_node_report(endpoint).await)
        }
        BedrockAdapter::Module { transport } => {
            Ok(adapters::blockchain_node_report(module_transport, transport).await)
        }
    }
}

pub(crate) async fn blocks(
    adapter: BedrockAdapter<'_>,
    slot_from: u64,
    slot_to: u64,
    limit: Option<u64>,
    module_transport: &SharedModuleTransport,
) -> Result<Value> {
    match adapter {
        BedrockAdapter::Rpc { endpoint } => match limit {
            Some(limit) => {
                crate::blockchain::blockchain_recent_blocks(endpoint, slot_from, slot_to, limit)
                    .await
            }
            None => crate::blockchain::blockchain_blocks(endpoint, slot_from, slot_to).await,
        },
        BedrockAdapter::Module { transport } => match limit {
            Some(limit) => {
                adapters::blockchain_recent_blocks(
                    module_transport,
                    transport,
                    slot_from,
                    slot_to,
                    limit,
                )
                .await
            }
            None => {
                adapters::blockchain_blocks(module_transport, transport, slot_from, slot_to).await
            }
        },
    }
}

pub(crate) async fn live_blocks(
    adapter: BedrockAdapter<'_>,
    slot_from: u64,
    slot_to: u64,
    limit: u64,
    module_transport: &SharedModuleTransport,
) -> Result<BlockchainLiveBlocksReport> {
    match adapter {
        BedrockAdapter::Rpc { endpoint } => {
            crate::blockchain::blockchain_live_blocks_snapshot(endpoint, slot_from, slot_to, limit)
                .await
        }
        BedrockAdapter::Module { transport } => {
            adapters::blockchain_live_blocks_snapshot(
                module_transport,
                transport,
                slot_from,
                slot_to,
                limit,
            )
            .await
        }
    }
}

pub(crate) async fn block(
    adapter: BedrockAdapter<'_>,
    block_id: &str,
    module_transport: &SharedModuleTransport,
) -> Result<Value> {
    match adapter {
        BedrockAdapter::Rpc { endpoint } => {
            crate::blockchain::blockchain_block(endpoint, block_id).await
        }
        BedrockAdapter::Module { transport } => {
            adapters::blockchain_block(module_transport, transport, block_id).await
        }
    }
}

pub(crate) async fn transaction(
    adapter: BedrockAdapter<'_>,
    transaction_id: &str,
    module_transport: &SharedModuleTransport,
) -> Result<Value> {
    match adapter {
        BedrockAdapter::Rpc { endpoint } => {
            crate::blockchain::blockchain_transaction(endpoint, transaction_id).await
        }
        BedrockAdapter::Module { transport } => {
            adapters::blockchain_transaction(module_transport, transport, transaction_id).await
        }
    }
}

pub(crate) async fn channel_scan(
    endpoint: &str,
    slot_from: u64,
    slot_to: u64,
) -> Result<ChannelScanReport> {
    crate::blockchain::channels::channel_scan(endpoint, slot_from, slot_to).await
}

pub(crate) async fn channel_state(endpoint: &str, channel_id: &str) -> Result<Value> {
    crate::blockchain::channels::channel_state(endpoint, channel_id).await
}

pub(crate) async fn raw_rpc(endpoint: &str, method: &str, params: Value) -> Result<RawRpcReport> {
    crate::rpc::raw_rpc_report(endpoint, method, params).await
}

pub(crate) async fn wallet_balance(
    endpoint: &str,
    public_key: &str,
    tip: Option<&str>,
) -> Result<Value> {
    let mut path = format!("/wallet/{public_key}/balance");
    if let Some(tip) = tip {
        path.push_str("?tip=");
        path.push_str(tip);
    }
    crate::rpc::raw_http_json(endpoint, &path).await
}

pub(crate) async fn catalog_chain_info(
    client: &Client,
    endpoint: &str,
    max_bytes: usize,
) -> Result<Value> {
    crate::blockchain::bedrock::blockchain_cryptarchia_info_bounded(client, endpoint, max_bytes)
        .await
}

pub(crate) async fn catalog_time_info(
    client: &Client,
    endpoint: &str,
    max_bytes: usize,
) -> Result<Value> {
    crate::blockchain::bedrock::blockchain_time_info_bounded(client, endpoint, max_bytes).await
}

pub(crate) async fn catalog_finalized_blocks_response(
    client: &Client,
    endpoint: &str,
    slot_from: u64,
    slot_to: u64,
    blocks_limit: NonZeroUsize,
    max_error_bytes: usize,
) -> Result<Response> {
    crate::blockchain::bedrock::blockchain_finalized_blocks_response(
        client,
        endpoint,
        slot_from,
        slot_to,
        blocks_limit,
        max_error_bytes,
    )
    .await
}

pub(crate) async fn catalog_block(
    client: &Client,
    endpoint: &str,
    block_id: &str,
    max_bytes: usize,
) -> Result<Option<Value>> {
    crate::blockchain::bedrock::blockchain_block_bounded(client, endpoint, block_id, max_bytes)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_routing::adapter::{
        ManagedNodeAction,
        contract_tests::{assert_layer_contract, assert_managed_module_contract},
    };

    #[test]
    fn bedrock_adapters_satisfy_shared_seam_contract() {
        assert_layer_contract("bedrock", BEDROCK_SOURCE_MODES);
    }

    #[test]
    fn bedrock_managed_calls_satisfy_shared_contract() {
        assert_managed_module_contract(
            "bedrock",
            managed_contract(),
            &[ManagedNodeAction::Start, ManagedNodeAction::Stop],
        );
    }

    #[test]
    fn bedrock_adapter_initializers_only_take_required_input() {
        assert_eq!(
            BedrockAdapter::rpc("http://bedrock"),
            BedrockAdapter::Rpc {
                endpoint: "http://bedrock"
            }
        );
        assert_eq!(
            BedrockAdapter::module(ModuleTransportKind::Module),
            BedrockAdapter::Module {
                transport: ModuleTransportKind::Module
            }
        );
    }
}
