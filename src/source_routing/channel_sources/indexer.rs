use std::sync::Arc;

use serde_json::Value;

use crate::{
    AccountReport, AccountTransactionSummary, IndexerBlockReport, TransactionSummary,
    modules::logos_core::{ModuleTransportKind, SharedModuleTransport},
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
        ExecutionZoneReadResult, capability_error, managed_config as shared_managed_config,
        map_read_error, optional_u64,
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

#[derive(Clone)]
pub(crate) enum IndexerAdapter<'a> {
    Rpc {
        endpoint: &'a str,
    },
    Module {
        transport: SharedModuleTransport,
        transport_kind: ModuleTransportKind,
    },
}

impl<'a> IndexerAdapter<'a> {
    pub(crate) fn connect(
        target: &'a ChannelSourceTarget,
        transport: &SharedModuleTransport,
        transport_kind: ModuleTransportKind,
    ) -> ExecutionZoneReadResult<Self> {
        match target {
            ChannelSourceTarget::Rpc { endpoint } => Ok(Self::Rpc { endpoint }),
            ChannelSourceTarget::Module { module_id } if module_id == MODULE_ID => {
                Ok(Self::Module {
                    transport: Arc::clone(transport),
                    transport_kind,
                })
            }
            ChannelSourceTarget::Module { .. } => Err(capability_error()),
        }
    }

    pub(crate) async fn health(&self) -> ExecutionZoneReadResult<()> {
        match self {
            Self::Rpc { endpoint } => crate::lez::indexer_health(endpoint)
                .await
                .map(|_| ())
                .map_err(map_read_error),
            Self::Module {
                transport,
                transport_kind,
            } => module_adapters::indexer_health(transport, *transport_kind)
                .await
                .map(|_| ())
                .map_err(map_read_error),
        }
    }

    pub(crate) async fn reported_head_id(&self) -> ExecutionZoneReadResult<Option<u64>> {
        match self {
            Self::Rpc { endpoint } => crate::lez::indexer_finalized_block_id(endpoint)
                .await
                .map_err(map_read_error),
            Self::Module {
                transport,
                transport_kind,
            } => module_adapters::indexer_finalized_head(transport, *transport_kind)
                .await
                .map_err(map_read_error)
                .and_then(optional_u64),
        }
    }

    pub(crate) async fn head(&self) -> ExecutionZoneReadResult<Option<IndexerBlockReport>> {
        let Some(block_id) = self.reported_head_id().await? else {
            return Ok(None);
        };
        self.block_by_id(block_id).await
    }

    pub(crate) async fn blocks(
        &self,
        before: Option<u64>,
        limit: u64,
    ) -> ExecutionZoneReadResult<Vec<IndexerBlockReport>> {
        match self {
            Self::Rpc { endpoint } => crate::lez::indexer_blocks(endpoint, before, limit)
                .await
                .map_err(map_read_error),
            Self::Module {
                transport,
                transport_kind,
            } => module_adapters::indexer_blocks(transport, *transport_kind, before, limit)
                .await
                .map_err(map_read_error),
        }
    }

    pub(crate) async fn block_by_id(
        &self,
        block_id: u64,
    ) -> ExecutionZoneReadResult<Option<IndexerBlockReport>> {
        match self {
            Self::Rpc { endpoint } => crate::lez::indexer_block_by_id(endpoint, block_id)
                .await
                .map_err(map_read_error),
            Self::Module {
                transport,
                transport_kind,
            } => module_adapters::indexer_block_by_id(transport, *transport_kind, block_id)
                .await
                .map_err(map_read_error),
        }
    }

    pub(crate) async fn block_by_hash(
        &self,
        block_hash: &str,
    ) -> ExecutionZoneReadResult<Option<IndexerBlockReport>> {
        match self {
            Self::Rpc { endpoint } => crate::lez::indexer_block_by_hash(endpoint, block_hash)
                .await
                .map_err(map_read_error),
            Self::Module {
                transport,
                transport_kind,
            } => module_adapters::indexer_block_by_hash(transport, *transport_kind, block_hash)
                .await
                .map_err(map_read_error),
        }
    }

    pub(crate) async fn transaction(
        &self,
        transaction_id: &str,
    ) -> ExecutionZoneReadResult<Option<TransactionSummary>> {
        match self {
            Self::Rpc { endpoint } => crate::lez::indexer_transaction(endpoint, transaction_id)
                .await
                .map_err(map_read_error),
            Self::Module {
                transport,
                transport_kind,
            } => module_adapters::indexer_transaction(transport, *transport_kind, transaction_id)
                .await
                .map_err(map_read_error),
        }
    }

    pub(crate) async fn account_at_block(
        &self,
        account_id: &str,
        block_id: u64,
    ) -> ExecutionZoneReadResult<AccountReport> {
        match self {
            Self::Rpc { endpoint } => {
                crate::lez::indexer_account_at_block(endpoint, account_id, block_id)
                    .await
                    .map_err(map_read_error)
            }
            Self::Module {
                transport,
                transport_kind,
            } => module_adapters::indexer_account_at_block(
                transport,
                *transport_kind,
                account_id,
                block_id,
            )
            .await
            .map_err(map_read_error),
        }
    }

    pub(crate) async fn account_activity(
        &self,
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
            Self::Module {
                transport,
                transport_kind,
            } => module_adapters::account_transactions_by_account(
                transport,
                *transport_kind,
                account_id,
                offset,
                limit,
            )
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

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use anyhow::{Context as _, Result, bail};
    use serde_json::json;

    use super::*;
    use crate::{
        modules::logos_core::{ModuleCall, ModuleCallFuture, ModuleCallReply, ModuleTransport},
        source_routing::channel_sources::layer::ExecutionZoneReadErrorKind,
    };

    struct RecordingModuleTransport {
        kind: ModuleTransportKind,
        reply_kind: ModuleTransportKind,
        reply: Value,
        calls: Arc<Mutex<Vec<ModuleCall>>>,
    }

    impl ModuleTransport for RecordingModuleTransport {
        fn kind(&self) -> ModuleTransportKind {
            self.kind
        }

        fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
            Box::pin(async move {
                self.calls
                    .lock()
                    .map_err(|_| anyhow::anyhow!("recorded calls lock poisoned"))?
                    .push(call);
                Ok(ModuleCallReply::new(self.reply_kind, self.reply.clone()))
            })
        }
    }

    fn recording_transport(
        kind: ModuleTransportKind,
        reply_kind: ModuleTransportKind,
        reply: Value,
    ) -> (SharedModuleTransport, Arc<Mutex<Vec<ModuleCall>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        (
            Arc::new(RecordingModuleTransport {
                kind,
                reply_kind,
                reply,
                calls: Arc::clone(&calls),
            }),
            calls,
        )
    }

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
        let (transport, _) = recording_transport(
            ModuleTransportKind::LogoscoreCli,
            ModuleTransportKind::LogoscoreCli,
            Value::Null,
        );

        assert!(matches!(
            IndexerAdapter::connect(&rpc, &transport, ModuleTransportKind::LogoscoreCli),
            Ok(IndexerAdapter::Rpc { .. })
        ));
        assert!(matches!(
            IndexerAdapter::connect(&module, &transport, ModuleTransportKind::LogoscoreCli),
            Ok(IndexerAdapter::Module {
                transport_kind: ModuleTransportKind::LogoscoreCli,
                ..
            })
        ));
        assert!(matches!(
            IndexerAdapter::connect(&other_module, &transport, ModuleTransportKind::LogoscoreCli)
                .map_err(|error| error.kind),
            Err(ExecutionZoneReadErrorKind::Capability)
        ));
    }

    #[tokio::test]
    async fn module_adapter_preserves_host_transport_identity() -> Result<()> {
        let target = ChannelSourceTarget::Module {
            module_id: MODULE_ID.to_owned(),
        };
        let (transport, calls) = recording_transport(
            ModuleTransportKind::Module,
            ModuleTransportKind::Module,
            json!(42),
        );
        let Ok(adapter) = IndexerAdapter::connect(&target, &transport, ModuleTransportKind::Module)
        else {
            bail!("Indexer module adapter did not connect");
        };

        let Ok(head) = adapter.reported_head_id().await else {
            bail!("Indexer module finalized-head call failed");
        };
        if head != Some(42) {
            bail!("Indexer module adapter lost finalized head response");
        }
        let calls = calls
            .lock()
            .map_err(|_| anyhow::anyhow!("calls lock poisoned"))?;
        let call = calls
            .first()
            .context("Indexer module call was not recorded")?;
        anyhow::ensure!(call.transport() == ModuleTransportKind::Module);
        anyhow::ensure!(call.module() == MODULE_ID);
        anyhow::ensure!(call.method() == "getLastFinalizedBlockId");
        Ok(())
    }

    #[tokio::test]
    async fn module_adapter_rejects_transport_mismatch_without_dispatch() -> Result<()> {
        let target = ChannelSourceTarget::Module {
            module_id: MODULE_ID.to_owned(),
        };
        let (transport, calls) = recording_transport(
            ModuleTransportKind::LogoscoreCli,
            ModuleTransportKind::LogoscoreCli,
            json!(42),
        );
        let Ok(adapter) = IndexerAdapter::connect(&target, &transport, ModuleTransportKind::Module)
        else {
            bail!("Indexer module adapter did not connect");
        };

        let Err(error) = adapter.reported_head_id().await else {
            bail!("transport mismatch did not fail closed");
        };
        anyhow::ensure!(error.kind == ExecutionZoneReadErrorKind::Unavailable);
        if !calls
            .lock()
            .map_err(|_| anyhow::anyhow!("calls lock poisoned"))?
            .is_empty()
        {
            bail!("transport mismatch reached active adapter");
        }
        Ok(())
    }
}
