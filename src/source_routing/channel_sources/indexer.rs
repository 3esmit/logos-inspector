use std::sync::Arc;

use serde_json::{Value, json};

use crate::{
    AccountReport, AccountTransactionSummary, IndexerBlockReport, TransactionSummary,
    modules::logos_core::{LogoscoreCliRuntime, ModuleTransportKind, SharedModuleTransport},
    source_routing::{
        adapter::{
            AdapterConnectionType, AdapterInputPolicy, ManagedModuleCallSpec, ManagedNodeAction,
            ManagedNodeContract, SourceAdapterPolicy, SourceModePolicy,
        },
        core::adapters::{self as module_adapters, INDEXER_MODULE},
    },
};

use super::{
    ChannelSourceTarget,
    layer::{ExecutionZoneReadResult, capability_error, map_read_error, optional_u64},
};

pub(crate) const MODULE_ID: &str = INDEXER_MODULE;

static MANAGED_CONTRACT: ManagedNodeContract = ManagedNodeContract::new(
    MODULE_ID,
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

pub(crate) fn ensure_managed_module(runtime: &LogoscoreCliRuntime) -> anyhow::Result<()> {
    runtime.ensure_module_loaded(MODULE_ID)
}

pub(crate) fn call_managed_module(
    runtime: &LogoscoreCliRuntime,
    method: &str,
    signature: &str,
    args: &[String],
) -> anyhow::Result<Value> {
    runtime.call_checked(MODULE_ID, method, signature, args)
}

#[must_use]
pub(crate) fn managed_call_spec(
    action: ManagedNodeAction,
    config_path: &str,
) -> Option<ManagedModuleCallSpec> {
    match action {
        ManagedNodeAction::Initialize | ManagedNodeAction::Destroy => None,
        ManagedNodeAction::Start => Some(ManagedModuleCallSpec::new(
            "start_indexer",
            "start_indexer(QString)",
            vec![config_path.to_owned()],
        )),
        ManagedNodeAction::Stop => Some(ManagedModuleCallSpec::new(
            "stop_indexer",
            "stop_indexer()",
            Vec::new(),
        )),
    }
}

#[must_use]
pub(crate) fn managed_reset_storage_call_spec(config_path: &str) -> ManagedModuleCallSpec {
    ManagedModuleCallSpec::new(
        "reset_storage",
        "reset_storage(QString)",
        vec![config_path.to_owned()],
    )
}

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

    pub(crate) async fn health(&self) -> ExecutionZoneReadResult<Option<String>> {
        match self {
            Self::Rpc { endpoint } => crate::lez::indexer_health(endpoint)
                .await
                .map(|_| None)
                .map_err(map_read_error),
            Self::Module {
                transport,
                transport_kind,
            } => module_adapters::indexer_status(transport, *transport_kind)
                .await
                .map(|status| Some(normalized_runtime_state(&status.state)))
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

fn normalized_runtime_state(value: &str) -> String {
    let normalized = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    match normalized.as_str() {
        "starting" => "starting",
        "syncing" => "syncing",
        "caughtup" => "caught_up",
        "running" => "running",
        "stopped" | "notinitialized" => "stopped",
        "error" | "failed" => "error",
        "stalled" => "stalled",
        "unavailable" | "offline" => "unavailable",
        _ => "unknown",
    }
    .to_owned()
}

#[must_use]
pub(crate) fn channel_config(channel_id: &str, bedrock_endpoint: &str) -> Value {
    json!({
        "consensus_info_polling_interval": "1s",
        "bedrock_config": {
            "addr": bedrock_endpoint.trim_end_matches('/'),
        },
        "channel_id": channel_id,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use anyhow::{Context as _, Result, bail};
    use serde_json::json;

    use super::*;
    use crate::{
        modules::logos_core::{ModuleCall, ModuleCallFuture, ModuleCallReply, ModuleTransport},
        source_routing::{
            adapter::contract_tests::assert_managed_module_contract,
            channel_sources::layer::ExecutionZoneReadErrorKind,
        },
    };

    #[test]
    fn managed_lifecycle_calls_match_indexer_module_contract() {
        assert_managed_module_contract(
            "execution_zone.indexer",
            managed_contract(),
            &[ManagedNodeAction::Start, ManagedNodeAction::Stop],
        );

        let reset = managed_reset_storage_call_spec("/tmp/channel/indexer.json");
        assert_eq!(reset.method, "reset_storage");
        assert_eq!(reset.signature, "reset_storage(QString)");
        assert_eq!(reset.args, ["/tmp/channel/indexer.json"]);
    }

    #[test]
    fn channel_config_uses_selected_channel_and_bedrock_endpoint() {
        let config = channel_config(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "http://127.0.0.1:18080/",
        );

        assert_eq!(
            config,
            json!({
                "consensus_info_polling_interval": "1s",
                "bedrock_config": { "addr": "http://127.0.0.1:18080" },
                "channel_id": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            })
        );
    }

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
    async fn module_adapter_preserves_normalized_runtime_state() -> Result<()> {
        let target = ChannelSourceTarget::Module {
            module_id: MODULE_ID.to_owned(),
        };
        let (transport, calls) = recording_transport(
            ModuleTransportKind::LogoscoreCli,
            ModuleTransportKind::LogoscoreCli,
            json!({"state": "CaughtUp", "indexedBlockId": "26001"}),
        );
        let Ok(adapter) =
            IndexerAdapter::connect(&target, &transport, ModuleTransportKind::LogoscoreCli)
        else {
            bail!("Indexer module adapter did not connect");
        };
        let Ok(state) = adapter.health().await else {
            bail!("Indexer module status request failed");
        };
        anyhow::ensure!(state.as_deref() == Some("caught_up"));
        let calls = calls
            .lock()
            .map_err(|_| anyhow::anyhow!("calls lock poisoned"))?;
        let call = calls
            .first()
            .context("Indexer status call was not recorded")?;
        anyhow::ensure!(call.method() == "getStatus");
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
