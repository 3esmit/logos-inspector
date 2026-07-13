use serde_json::Value;

use crate::{
    AccountReport, ProgramIdEntry, TransactionSummary,
    lez::BlockSummary,
    source_routing::{
        adapter::{
            AdapterConnectionType, AdapterInputPolicy, SourceAdapterPolicy, SourceModePolicy,
        },
        core::adapters::LEZ_CORE_MODULE,
    },
};

use super::{
    ChannelSourceTarget,
    layer::{
        ExecutionZoneReadResult, capability_error, managed_config as shared_managed_config,
        map_read_error,
    },
};

pub(crate) const MODULE_ID: &str = LEZ_CORE_MODULE;
pub(crate) const MANAGED_PROGRAM: &str = "sequencer_service";

const RPC_INPUTS: &[AdapterInputPolicy] = &[AdapterInputPolicy {
    key: "rpc_endpoint",
    label: "RPC URL",
    required: true,
}];
const CAPABILITIES: &[&str] = &[
    "execution_zone.head.read",
    "execution_zone.blocks.read",
    "execution_zone.transactions.read",
    "execution_zone.accounts.current.read",
    "execution_zone.accounts.nonces.read",
    "execution_zone.commitments.proof.read",
    "execution_zone.programs.read",
];

pub(crate) const SOURCE_MODES: &[SourceModePolicy] = &[
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
            capabilities: CAPABILITIES,
            supports_cid_probe: false,
            supports_mutating_diagnostics: false,
            capability_scopes: &[],
            endpoint_role: None,
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
            connector_id: MODULE_ID,
            connection_type: AdapterConnectionType::Module,
            target: "module",
            module_id: Some(MODULE_ID),
            inputs: &[],
            capabilities: &[],
            supports_cid_probe: false,
            supports_mutating_diagnostics: false,
            capability_scopes: &["wallet.l2"],
            endpoint_role: None,
        },
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SequencerAdapter<'a> {
    endpoint: &'a str,
}

impl<'a> SequencerAdapter<'a> {
    pub(crate) fn connect(target: &'a ChannelSourceTarget) -> ExecutionZoneReadResult<Self> {
        match target {
            ChannelSourceTarget::Rpc { endpoint } => Ok(Self { endpoint }),
            ChannelSourceTarget::Module { .. } => Err(capability_error()),
        }
    }

    pub(crate) async fn health(self) -> ExecutionZoneReadResult<()> {
        crate::lez::sequencer_health(self.endpoint)
            .await
            .map_err(map_read_error)
    }

    pub(crate) async fn channel_id(self) -> ExecutionZoneReadResult<String> {
        crate::lez::sequencer_channel_id(self.endpoint)
            .await
            .map_err(map_read_error)
    }

    pub(crate) async fn reported_head_id(self) -> ExecutionZoneReadResult<u64> {
        crate::lez::last_sequencer_block_id(self.endpoint)
            .await
            .map_err(map_read_error)
    }

    pub(crate) async fn head(self) -> ExecutionZoneReadResult<Option<BlockSummary>> {
        let block_id = self.reported_head_id().await?;
        self.block_by_id(block_id).await
    }

    pub(crate) async fn blocks(
        self,
        before: Option<u64>,
        limit: u64,
    ) -> ExecutionZoneReadResult<Vec<BlockSummary>> {
        crate::lez::sequencer_blocks(self.endpoint, before, limit)
            .await
            .map_err(map_read_error)
    }

    pub(crate) async fn block_by_id(
        self,
        block_id: u64,
    ) -> ExecutionZoneReadResult<Option<BlockSummary>> {
        crate::lez::sequencer_block(self.endpoint, block_id)
            .await
            .map_err(map_read_error)
    }

    pub(crate) async fn transaction(
        self,
        transaction_id: &str,
    ) -> ExecutionZoneReadResult<Option<TransactionSummary>> {
        crate::lez::sequencer_transaction(self.endpoint, transaction_id)
            .await
            .map_err(map_read_error)
    }

    pub(crate) async fn current_account(
        self,
        account_id: &str,
    ) -> ExecutionZoneReadResult<AccountReport> {
        crate::lez::sequencer_account(self.endpoint, account_id)
            .await
            .map_err(map_read_error)
    }

    pub(crate) async fn programs(self) -> ExecutionZoneReadResult<Vec<ProgramIdEntry>> {
        crate::lez::sequencer_program_ids(self.endpoint)
            .await
            .map_err(map_read_error)
    }

    pub(crate) async fn commitment_proof(
        self,
        commitment_hex: &str,
    ) -> ExecutionZoneReadResult<Option<(u64, Vec<String>)>> {
        crate::lez::sequencer_commitment_proof(self.endpoint, commitment_hex)
            .await
            .map_err(map_read_error)
    }

    pub(crate) async fn account_nonces(
        self,
        account_ids: &[String],
    ) -> ExecutionZoneReadResult<Vec<String>> {
        crate::lez::sequencer_account_nonces(self.endpoint, account_ids)
            .await
            .map_err(map_read_error)
    }
}

#[must_use]
pub(crate) fn managed_config(
    network_id: &str,
    data_dir: &str,
    endpoint: Option<&str>,
    port: Option<u16>,
) -> Value {
    shared_managed_config("sequencer", network_id, data_dir, endpoint, port)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_routing::channel_sources::layer::ExecutionZoneReadErrorKind;

    #[test]
    fn adapter_accepts_only_sequencer_transport() {
        let rpc = ChannelSourceTarget::Rpc {
            endpoint: "http://node".to_owned(),
        };
        let module = ChannelSourceTarget::Module {
            module_id: MODULE_ID.to_owned(),
        };

        assert!(SequencerAdapter::connect(&rpc).is_ok());
        assert_eq!(
            SequencerAdapter::connect(&module).map_err(|error| error.kind),
            Err(ExecutionZoneReadErrorKind::Capability)
        );
    }
}
