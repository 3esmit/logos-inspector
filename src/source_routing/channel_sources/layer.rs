use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::{
    AccountReport, AccountTransactionSummary, IndexerBlockReport, ProgramIdEntry,
    TransactionSummary,
    lez::BlockSummary,
    source_routing::{
        adapter::{
            AdapterConnectionType, AdapterInputPolicy, AdapterLayer, SourceAdapterPolicy,
            SourceModePolicy, sealed,
        },
        core::adapters::{self as module_adapters, INDEXER_MODULE, LEZ_CORE_MODULE},
    },
};

use super::ChannelSourceRole;

const RPC_INPUTS: &[AdapterInputPolicy] = &[AdapterInputPolicy {
    key: "rpc_endpoint",
    label: "RPC URL",
    required: true,
}];
const SEQUENCER_CAPABILITIES: &[&str] = &[
    "execution_zone.head.read",
    "execution_zone.blocks.read",
    "execution_zone.transactions.read",
    "execution_zone.accounts.current.read",
    "execution_zone.programs.read",
];
const INDEXER_CAPABILITIES: &[&str] = &[
    "execution_zone.head.read",
    "execution_zone.blocks.read",
    "execution_zone.blocks.by_hash.read",
    "execution_zone.transactions.read",
    "execution_zone.accounts.historical.read",
    "execution_zone.accounts.activity.read",
    "execution_zone.commitments.proof.read",
    "execution_zone.transfers.read",
];

pub(crate) const SEQUENCER_SOURCE_MODES: &[SourceModePolicy] = &[
    SourceModePolicy {
        key: "rpc",
        aliases: &["rpc"],
        effective: "rpc",
        label_key: "sequencer_rpc",
        label: "Sequencer RPC",
        source_label: "Sequencer RPC",
        summary: "Inspect provisional Channel state through Sequencer RPC",
        implemented: true,
        adapter: SourceAdapterPolicy {
            connector_id: "direct_sequencer_rpc",
            connection_type: AdapterConnectionType::Rpc,
            target: "rpc_endpoint",
            module_id: None,
            inputs: RPC_INPUTS,
            capabilities: SEQUENCER_CAPABILITIES,
            supports_cid_probe: false,
            supports_mutating_diagnostics: false,
        },
    },
    SourceModePolicy {
        key: "module",
        aliases: &["module"],
        effective: "module",
        label_key: "sequencer_module",
        label: "Sequencer module",
        source_label: "Sequencer module",
        summary: "Use the Channel-owned Sequencer module",
        implemented: false,
        adapter: SourceAdapterPolicy {
            connector_id: LEZ_CORE_MODULE,
            connection_type: AdapterConnectionType::Module,
            target: "module",
            module_id: Some(LEZ_CORE_MODULE),
            inputs: &[],
            capabilities: &[],
            supports_cid_probe: false,
            supports_mutating_diagnostics: false,
        },
    },
];

pub(crate) const INDEXER_SOURCE_MODES: &[SourceModePolicy] = &[
    SourceModePolicy {
        key: "rpc",
        aliases: &["rpc"],
        effective: "rpc",
        label_key: "indexer_rpc",
        label: "Indexer RPC",
        source_label: "Indexer RPC",
        summary: "Inspect finalized Channel history through Indexer RPC",
        implemented: true,
        adapter: SourceAdapterPolicy {
            connector_id: "direct_indexer_rpc",
            connection_type: AdapterConnectionType::Rpc,
            target: "rpc_endpoint",
            module_id: None,
            inputs: RPC_INPUTS,
            capabilities: INDEXER_CAPABILITIES,
            supports_cid_probe: false,
            supports_mutating_diagnostics: false,
        },
    },
    SourceModePolicy {
        key: "module",
        aliases: &["module"],
        effective: "module",
        label_key: "indexer_module",
        label: "Indexer module",
        source_label: "Indexer module",
        summary: "Use the Channel-owned Indexer module",
        implemented: true,
        adapter: SourceAdapterPolicy {
            connector_id: INDEXER_MODULE,
            connection_type: AdapterConnectionType::Module,
            target: "module",
            module_id: Some(INDEXER_MODULE),
            inputs: &[],
            capabilities: INDEXER_CAPABILITIES,
            supports_cid_probe: false,
            supports_mutating_diagnostics: false,
        },
    },
];

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SequencerAdapterLayer;

impl sealed::Sealed for SequencerAdapterLayer {}

impl AdapterLayer for SequencerAdapterLayer {
    fn key(&self) -> &'static str {
        "execution_zone.sequencer"
    }

    fn modes(&self) -> &'static [SourceModePolicy] {
        SEQUENCER_SOURCE_MODES
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct IndexerAdapterLayer;

impl sealed::Sealed for IndexerAdapterLayer {}

impl AdapterLayer for IndexerAdapterLayer {
    fn key(&self) -> &'static str {
        "execution_zone.indexer"
    }

    fn modes(&self) -> &'static [SourceModePolicy] {
        INDEXER_SOURCE_MODES
    }
}

#[must_use]
pub(crate) fn source_modes_for_role(role: ChannelSourceRole) -> &'static [SourceModePolicy] {
    match role {
        ChannelSourceRole::Sequencer => SequencerAdapterLayer.modes(),
        ChannelSourceRole::Indexer => IndexerAdapterLayer.modes(),
    }
}

#[must_use]
pub(crate) const fn module_id_for_role(role: ChannelSourceRole) -> &'static str {
    match role {
        ChannelSourceRole::Sequencer => LEZ_CORE_MODULE,
        ChannelSourceRole::Indexer => INDEXER_MODULE,
    }
}

#[must_use]
pub(crate) const fn managed_sequencer_program() -> &'static str {
    "sequencer_service"
}

pub(crate) async fn sequencer_health(endpoint: &str) -> Result<()> {
    crate::lez::sequencer_health(endpoint).await
}

pub(crate) async fn sequencer_channel_id(endpoint: &str) -> Result<String> {
    crate::lez::sequencer_channel_id(endpoint).await
}

pub(crate) async fn sequencer_last_block_id(endpoint: &str) -> Result<u64> {
    crate::lez::last_sequencer_block_id(endpoint).await
}

pub(crate) async fn sequencer_block(endpoint: &str, block_id: u64) -> Result<Option<BlockSummary>> {
    crate::lez::sequencer_block(endpoint, block_id).await
}

pub(crate) async fn sequencer_blocks(
    endpoint: &str,
    before: Option<u64>,
    limit: u64,
) -> Result<Vec<BlockSummary>> {
    crate::lez::sequencer_blocks(endpoint, before, limit).await
}

pub(crate) async fn sequencer_transaction(
    endpoint: &str,
    transaction_id: &str,
) -> Result<Option<TransactionSummary>> {
    crate::lez::sequencer_transaction(endpoint, transaction_id).await
}

pub(crate) async fn sequencer_account(endpoint: &str, account_id: &str) -> Result<AccountReport> {
    crate::lez::sequencer_account(endpoint, account_id).await
}

pub(crate) async fn sequencer_program_ids(endpoint: &str) -> Result<Vec<ProgramIdEntry>> {
    crate::lez::sequencer_program_ids(endpoint).await
}

pub(crate) async fn sequencer_commitment_proof(
    endpoint: &str,
    commitment_hex: &str,
) -> Result<Option<(u64, Vec<String>)>> {
    crate::lez::sequencer_commitment_proof(endpoint, commitment_hex).await
}

pub(crate) async fn sequencer_account_nonces(
    endpoint: &str,
    account_ids: &[String],
) -> Result<Vec<String>> {
    crate::lez::sequencer_account_nonces(endpoint, account_ids).await
}

pub(crate) async fn indexer_health(endpoint: &str) -> Result<Value> {
    crate::lez::indexer_health(endpoint).await
}

pub(crate) async fn indexer_finalized_block_id(endpoint: &str) -> Result<Option<u64>> {
    crate::lez::indexer_finalized_block_id(endpoint).await
}

pub(crate) async fn indexer_blocks(
    endpoint: &str,
    before: Option<u64>,
    limit: u64,
) -> Result<Vec<IndexerBlockReport>> {
    crate::lez::indexer_blocks(endpoint, before, limit).await
}

pub(crate) async fn indexer_block_by_id(
    endpoint: &str,
    block_id: u64,
) -> Result<Option<IndexerBlockReport>> {
    crate::lez::indexer_block_by_id(endpoint, block_id).await
}

pub(crate) async fn indexer_block_by_hash(
    endpoint: &str,
    block_hash: &str,
) -> Result<Option<IndexerBlockReport>> {
    crate::lez::indexer_block_by_hash(endpoint, block_hash).await
}

pub(crate) async fn indexer_transaction(
    endpoint: &str,
    transaction_id: &str,
) -> Result<Option<TransactionSummary>> {
    crate::lez::indexer_transaction(endpoint, transaction_id).await
}

pub(crate) async fn indexer_account_at_block(
    endpoint: &str,
    account_id: &str,
    block_id: u64,
) -> Result<AccountReport> {
    crate::lez::indexer_account_at_block(endpoint, account_id, block_id).await
}

pub(crate) async fn indexer_account_activity(
    endpoint: &str,
    account_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<AccountTransactionSummary>> {
    crate::lez::account_transactions_by_account(endpoint, account_id, offset, limit).await
}

pub(crate) async fn module_indexer_health() -> Result<Value> {
    blocking_module_call(
        "Execution Zone Indexer health",
        module_adapters::module::indexer_health,
    )
    .await
}

pub(crate) async fn module_indexer_finalized_head() -> Result<Value> {
    blocking_module_call(
        "Execution Zone Indexer finalized head",
        module_adapters::indexer_finalized_head,
    )
    .await
}

pub(crate) async fn module_indexer_blocks(
    before: Option<u64>,
    limit: u64,
) -> Result<Vec<IndexerBlockReport>> {
    blocking_module_call("Execution Zone Indexer blocks", move || {
        module_adapters::indexer_blocks(before, limit)
    })
    .await
}

pub(crate) async fn module_indexer_block_by_id(
    block_id: u64,
) -> Result<Option<IndexerBlockReport>> {
    blocking_module_call("Execution Zone Indexer block", move || {
        module_adapters::indexer_block_by_id(block_id)
    })
    .await
}

pub(crate) async fn module_indexer_block_by_hash(
    block_hash: String,
) -> Result<Option<IndexerBlockReport>> {
    blocking_module_call("Execution Zone Indexer block", move || {
        module_adapters::indexer_block_by_hash(&block_hash)
    })
    .await
}

pub(crate) async fn module_indexer_transaction(
    transaction_id: String,
) -> Result<Option<TransactionSummary>> {
    blocking_module_call("Execution Zone Indexer transaction", move || {
        module_adapters::indexer_transaction(&transaction_id)
    })
    .await
}

pub(crate) async fn module_indexer_account_at_block(
    account_id: String,
    block_id: u64,
) -> Result<AccountReport> {
    blocking_module_call("Execution Zone Indexer account", move || {
        module_adapters::indexer_account_at_block(&account_id, block_id)
    })
    .await
}

pub(crate) async fn module_indexer_account_activity(
    account_id: String,
    offset: usize,
    limit: usize,
) -> Result<Vec<AccountTransactionSummary>> {
    blocking_module_call("Execution Zone Indexer activity", move || {
        module_adapters::account_transactions_by_account(&account_id, offset, limit)
    })
    .await
}

pub(crate) fn deploy_program(
    profile: Value,
    program_path: &str,
) -> Result<crate::wallet::LocalWalletDeployReport> {
    crate::wallet::local_wallet_deploy_program(profile, program_path)
}

pub(crate) async fn submit_instruction(
    profile: Value,
    request: Value,
) -> Result<crate::wallet::LocalWalletInstructionReport> {
    crate::wallet::local_wallet_instruction_submit(profile, request).await
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
    fn execution_zone_adapters_satisfy_shared_seam_contract() {
        assert_layer_contract(&SequencerAdapterLayer);
        assert_layer_contract(&IndexerAdapterLayer);
    }

    #[test]
    fn module_ids_are_owned_by_execution_zone_role() {
        assert_eq!(module_id_for_role(ChannelSourceRole::Sequencer), "lez_core");
        assert_eq!(
            module_id_for_role(ChannelSourceRole::Indexer),
            "lez_indexer_module"
        );
    }
}
