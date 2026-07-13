use serde_json::Value;

use crate::{
    AccountReport, AccountTransactionSummary, IndexerBlockReport, TransactionSummary,
    source_routing::{
        adapter::{
            AdapterConnectionType, AdapterInputPolicy, SourceAdapterPolicy, SourceModePolicy,
        },
        core::adapters::{self as module_adapters, INDEXER_MODULE},
    },
};

use super::{
    ChannelSourceTarget,
    layer::{
        ExecutionZoneReadResult, blocking_module_call, capability_error,
        managed_config as shared_managed_config, map_read_error, optional_u64,
    },
};

pub(crate) const MODULE_ID: &str = INDEXER_MODULE;

const RPC_INPUTS: &[AdapterInputPolicy] = &[AdapterInputPolicy {
    key: "rpc_endpoint",
    label: "RPC URL",
    required: true,
}];
const CAPABILITIES: &[&str] = &[
    "execution_zone.head.read",
    "execution_zone.blocks.read",
    "execution_zone.blocks.by_hash.read",
    "execution_zone.transactions.read",
    "execution_zone.accounts.historical.read",
    "execution_zone.accounts.activity.read",
    "execution_zone.transfers.read",
];

pub(crate) const SOURCE_MODES: &[SourceModePolicy] = &[
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
        label_key: "indexer_module",
        label: "Indexer module",
        source_label: "Indexer module",
        summary: "Use the Channel-owned Indexer module",
        implemented: true,
        adapter: SourceAdapterPolicy {
            connector_id: MODULE_ID,
            connection_type: AdapterConnectionType::Module,
            target: "module",
            module_id: Some(MODULE_ID),
            inputs: &[],
            capabilities: CAPABILITIES,
            supports_cid_probe: false,
            supports_mutating_diagnostics: false,
            capability_scopes: &[],
            endpoint_role: None,
        },
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IndexerAdapter<'a> {
    Rpc { endpoint: &'a str },
    Module,
}

impl<'a> IndexerAdapter<'a> {
    pub(crate) fn connect(target: &'a ChannelSourceTarget) -> ExecutionZoneReadResult<Self> {
        match target {
            ChannelSourceTarget::Rpc { endpoint } => Ok(Self::Rpc { endpoint }),
            ChannelSourceTarget::Module { module_id } if module_id == MODULE_ID => Ok(Self::Module),
            ChannelSourceTarget::Module { .. } => Err(capability_error()),
        }
    }

    pub(crate) async fn health(self) -> ExecutionZoneReadResult<()> {
        match self {
            Self::Rpc { endpoint } => crate::lez::indexer_health(endpoint)
                .await
                .map(|_| ())
                .map_err(map_read_error),
            Self::Module => module_health().await.map(|_| ()).map_err(map_read_error),
        }
    }

    pub(crate) async fn reported_head_id(self) -> ExecutionZoneReadResult<Option<u64>> {
        match self {
            Self::Rpc { endpoint } => crate::lez::indexer_finalized_block_id(endpoint)
                .await
                .map_err(map_read_error),
            Self::Module => module_finalized_head()
                .await
                .map_err(map_read_error)
                .and_then(optional_u64),
        }
    }

    pub(crate) async fn head(self) -> ExecutionZoneReadResult<Option<IndexerBlockReport>> {
        let Some(block_id) = self.reported_head_id().await? else {
            return Ok(None);
        };
        self.block_by_id(block_id).await
    }

    pub(crate) async fn blocks(
        self,
        before: Option<u64>,
        limit: u64,
    ) -> ExecutionZoneReadResult<Vec<IndexerBlockReport>> {
        match self {
            Self::Rpc { endpoint } => crate::lez::indexer_blocks(endpoint, before, limit)
                .await
                .map_err(map_read_error),
            Self::Module => module_blocks(before, limit).await.map_err(map_read_error),
        }
    }

    pub(crate) async fn block_by_id(
        self,
        block_id: u64,
    ) -> ExecutionZoneReadResult<Option<IndexerBlockReport>> {
        match self {
            Self::Rpc { endpoint } => crate::lez::indexer_block_by_id(endpoint, block_id)
                .await
                .map_err(map_read_error),
            Self::Module => module_block_by_id(block_id).await.map_err(map_read_error),
        }
    }

    pub(crate) async fn block_by_hash(
        self,
        block_hash: &str,
    ) -> ExecutionZoneReadResult<Option<IndexerBlockReport>> {
        match self {
            Self::Rpc { endpoint } => crate::lez::indexer_block_by_hash(endpoint, block_hash)
                .await
                .map_err(map_read_error),
            Self::Module => module_block_by_hash(block_hash.to_owned())
                .await
                .map_err(map_read_error),
        }
    }

    pub(crate) async fn transaction(
        self,
        transaction_id: &str,
    ) -> ExecutionZoneReadResult<Option<TransactionSummary>> {
        match self {
            Self::Rpc { endpoint } => crate::lez::indexer_transaction(endpoint, transaction_id)
                .await
                .map_err(map_read_error),
            Self::Module => module_transaction(transaction_id.to_owned())
                .await
                .map_err(map_read_error),
        }
    }

    pub(crate) async fn account_at_block(
        self,
        account_id: &str,
        block_id: u64,
    ) -> ExecutionZoneReadResult<AccountReport> {
        match self {
            Self::Rpc { endpoint } => {
                crate::lez::indexer_account_at_block(endpoint, account_id, block_id)
                    .await
                    .map_err(map_read_error)
            }
            Self::Module => module_account_at_block(account_id.to_owned(), block_id)
                .await
                .map_err(map_read_error),
        }
    }

    pub(crate) async fn account_activity(
        self,
        account_id: &str,
        offset: usize,
        limit: usize,
    ) -> ExecutionZoneReadResult<Vec<AccountTransactionSummary>> {
        match self {
            Self::Rpc { endpoint } => {
                crate::lez::account_transactions_by_account(endpoint, account_id, offset, limit)
                    .await
                    .map_err(map_read_error)
            }
            Self::Module => module_account_activity(account_id.to_owned(), offset, limit)
                .await
                .map_err(map_read_error),
        }
    }
}

#[must_use]
pub(crate) fn managed_config(
    network_id: &str,
    data_dir: &str,
    endpoint: Option<&str>,
    port: Option<u16>,
) -> Value {
    shared_managed_config("indexer", network_id, data_dir, endpoint, port)
}

async fn module_health() -> anyhow::Result<Value> {
    blocking_module_call(
        "Execution Zone Indexer health",
        module_adapters::module::indexer_health,
    )
    .await
}

async fn module_finalized_head() -> anyhow::Result<Value> {
    blocking_module_call(
        "Execution Zone Indexer finalized head",
        module_adapters::indexer_finalized_head,
    )
    .await
}

async fn module_blocks(before: Option<u64>, limit: u64) -> anyhow::Result<Vec<IndexerBlockReport>> {
    blocking_module_call("Execution Zone Indexer blocks", move || {
        module_adapters::indexer_blocks(before, limit)
    })
    .await
}

async fn module_block_by_id(block_id: u64) -> anyhow::Result<Option<IndexerBlockReport>> {
    blocking_module_call("Execution Zone Indexer block", move || {
        module_adapters::indexer_block_by_id(block_id)
    })
    .await
}

async fn module_block_by_hash(block_hash: String) -> anyhow::Result<Option<IndexerBlockReport>> {
    blocking_module_call("Execution Zone Indexer block", move || {
        module_adapters::indexer_block_by_hash(&block_hash)
    })
    .await
}

async fn module_transaction(transaction_id: String) -> anyhow::Result<Option<TransactionSummary>> {
    blocking_module_call("Execution Zone Indexer transaction", move || {
        module_adapters::indexer_transaction(&transaction_id)
    })
    .await
}

async fn module_account_at_block(
    account_id: String,
    block_id: u64,
) -> anyhow::Result<AccountReport> {
    blocking_module_call("Execution Zone Indexer account", move || {
        module_adapters::indexer_account_at_block(&account_id, block_id)
    })
    .await
}

async fn module_account_activity(
    account_id: String,
    offset: usize,
    limit: usize,
) -> anyhow::Result<Vec<AccountTransactionSummary>> {
    blocking_module_call("Execution Zone Indexer activity", move || {
        module_adapters::account_transactions_by_account(&account_id, offset, limit)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_routing::channel_sources::layer::ExecutionZoneReadErrorKind;

    #[test]
    fn adapter_accepts_rpc_and_owned_module_only() {
        let rpc = ChannelSourceTarget::Rpc {
            endpoint: "http://node".to_owned(),
        };
        let module = ChannelSourceTarget::Module {
            module_id: MODULE_ID.to_owned(),
        };
        let other_module = ChannelSourceTarget::Module {
            module_id: "other".to_owned(),
        };

        assert!(matches!(
            IndexerAdapter::connect(&rpc),
            Ok(IndexerAdapter::Rpc { .. })
        ));
        assert_eq!(IndexerAdapter::connect(&module), Ok(IndexerAdapter::Module));
        assert_eq!(
            IndexerAdapter::connect(&other_module).map_err(|error| error.kind),
            Err(ExecutionZoneReadErrorKind::Capability)
        );
    }
}
