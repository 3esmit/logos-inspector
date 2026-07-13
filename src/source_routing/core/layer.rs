use std::num::NonZeroUsize;

use anyhow::{Context as _, Result};
use reqwest::{Client, Response};
use serde_json::Value;

use super::adapters::{self, BLOCKCHAIN_MODULE};
use crate::{
    blockchain::{BlockchainLiveBlocksReport, BlockchainNodeReport, channels::ChannelScanReport},
    modules::ModuleReport,
    modules::logos_core::LogoscoreCliRuntime,
    rpc::RawRpcReport,
};

#[must_use]
pub(crate) const fn module_id() -> &'static str {
    BLOCKCHAIN_MODULE
}

pub(crate) fn ensure_managed_module(runtime: &LogoscoreCliRuntime) -> Result<()> {
    crate::source_routing::adapter::ensure_managed_module(runtime, module_id())
}

pub(crate) fn call_managed_module(
    runtime: &LogoscoreCliRuntime,
    method: &str,
    signature: &str,
    args: &[String],
) -> Result<Value> {
    crate::source_routing::adapter::call_managed_module(
        runtime,
        module_id(),
        method,
        signature,
        args,
    )
}

#[must_use]
pub(crate) fn diagnostic_report(address: Option<&str>) -> ModuleReport {
    crate::modules::blockchain_module_report(address)
}
use crate::source_routing::adapter::{
    AdapterConnectionType, AdapterInputPolicy, AdapterLayer, SourceAdapterPolicy, SourceModePolicy,
    sealed,
};

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
        },
    },
    SourceModePolicy {
        key: "module",
        aliases: &["module", "basecamp", "basecamp-module", "basecamp module"],
        effective: "module",
        label_key: "basecamp_module",
        label: "Basecamp module",
        source_label: "Basecamp module",
        summary: "Use the installed Bedrock module through logoscore",
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
        },
    },
];

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct BedrockAdapterLayer;

impl sealed::Sealed for BedrockAdapterLayer {}

impl AdapterLayer for BedrockAdapterLayer {
    fn key(&self) -> &'static str {
        "bedrock"
    }

    fn modes(&self) -> &'static [SourceModePolicy] {
        BEDROCK_SOURCE_MODES
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BedrockAdapter<'a> {
    Rpc { endpoint: &'a str },
    Module,
}

impl<'a> BedrockAdapter<'a> {
    #[must_use]
    pub(crate) const fn rpc(endpoint: &'a str) -> Self {
        Self::Rpc { endpoint }
    }

    #[must_use]
    pub(crate) const fn module() -> Self {
        Self::Module
    }
}

pub(crate) async fn node_report(adapter: BedrockAdapter<'_>) -> Result<BlockchainNodeReport> {
    match adapter {
        BedrockAdapter::Rpc { endpoint } => {
            Ok(crate::blockchain::blockchain_node_report(endpoint).await)
        }
        BedrockAdapter::Module => {
            blocking_module_call("Bedrock node report", || {
                Ok(adapters::blockchain_node_report())
            })
            .await
        }
    }
}

pub(crate) async fn blocks(
    adapter: BedrockAdapter<'_>,
    slot_from: u64,
    slot_to: u64,
    limit: Option<u64>,
) -> Result<Value> {
    match adapter {
        BedrockAdapter::Rpc { endpoint } => match limit {
            Some(limit) => {
                crate::blockchain::blockchain_recent_blocks(endpoint, slot_from, slot_to, limit)
                    .await
            }
            None => crate::blockchain::blockchain_blocks(endpoint, slot_from, slot_to).await,
        },
        BedrockAdapter::Module => {
            blocking_module_call("Bedrock blocks", move || match limit {
                Some(limit) => adapters::blockchain_recent_blocks(slot_from, slot_to, limit),
                None => adapters::blockchain_blocks(slot_from, slot_to),
            })
            .await
        }
    }
}

pub(crate) async fn live_blocks(
    adapter: BedrockAdapter<'_>,
    slot_from: u64,
    slot_to: u64,
    limit: u64,
) -> Result<BlockchainLiveBlocksReport> {
    match adapter {
        BedrockAdapter::Rpc { endpoint } => {
            crate::blockchain::blockchain_live_blocks_snapshot(endpoint, slot_from, slot_to, limit)
                .await
        }
        BedrockAdapter::Module => {
            blocking_module_call("Bedrock live blocks", move || {
                adapters::blockchain_live_blocks_snapshot(slot_from, slot_to, limit)
            })
            .await
        }
    }
}

pub(crate) async fn block(adapter: BedrockAdapter<'_>, block_id: &str) -> Result<Value> {
    match adapter {
        BedrockAdapter::Rpc { endpoint } => {
            crate::blockchain::blockchain_block(endpoint, block_id).await
        }
        BedrockAdapter::Module => {
            let block_id = block_id.to_owned();
            blocking_module_call("Bedrock block", move || {
                adapters::blockchain_block(&block_id)
            })
            .await
        }
    }
}

pub(crate) async fn transaction(
    adapter: BedrockAdapter<'_>,
    transaction_id: &str,
) -> Result<Value> {
    match adapter {
        BedrockAdapter::Rpc { endpoint } => {
            crate::blockchain::blockchain_transaction(endpoint, transaction_id).await
        }
        BedrockAdapter::Module => {
            let transaction_id = transaction_id.to_owned();
            blocking_module_call("Bedrock transaction", move || {
                adapters::blockchain_transaction(&transaction_id)
            })
            .await
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

async fn blocking_module_call<T, F>(label: &'static str, call: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(call)
        .await
        .with_context(|| format!("{label} worker failed"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_routing::adapter::contract_tests::assert_layer_contract;

    #[test]
    fn bedrock_adapters_satisfy_shared_seam_contract() {
        assert_layer_contract(&BedrockAdapterLayer);
    }

    #[test]
    fn bedrock_adapter_initializers_only_take_required_input() {
        assert_eq!(
            BedrockAdapter::rpc("http://bedrock"),
            BedrockAdapter::Rpc {
                endpoint: "http://bedrock"
            }
        );
        assert_eq!(BedrockAdapter::module(), BedrockAdapter::Module);
    }
}
