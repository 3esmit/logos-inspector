use std::{future::Future, pin::Pin, sync::Arc};

use super::{
    L2AccountActivityRow, L2AccountValue, NormalizedL2Block, normalize_account,
    normalize_activity_row, normalize_indexer_block, normalize_sequencer_block,
};
use crate::{
    inspection::{NetworkScope, ZoneSourceRole},
    lez::{IndexerBlockReport, ProgramIdEntry, TransactionSummary},
    modules::logos_core::{LogoscoreCliTransport, ModuleTransportKind, SharedModuleTransport},
    source_routing::channel_sources::{
        ChannelSourceTarget,
        indexer::{IndexerAdapter, MODULE_ID},
        layer::{ExecutionZoneReadError, ExecutionZoneReadErrorKind},
        sequencer::SequencerAdapter,
    },
};

pub(crate) type L2SourceFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, L2SourceError>> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum L2SourceErrorKind {
    Unavailable,
    Protocol,
    Capability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct L2SourceError {
    pub kind: L2SourceErrorKind,
}

impl L2SourceError {
    #[must_use]
    pub const fn unavailable() -> Self {
        Self {
            kind: L2SourceErrorKind::Unavailable,
        }
    }

    #[must_use]
    pub const fn protocol_error() -> Self {
        Self {
            kind: L2SourceErrorKind::Protocol,
        }
    }

    #[must_use]
    pub const fn capability() -> Self {
        Self {
            kind: L2SourceErrorKind::Capability,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct L2SourceDescriptor {
    pub network_scope: NetworkScope,
    pub channel_id: String,
    pub source_id: String,
    pub role: ZoneSourceRole,
    pub target: ChannelSourceTarget,
    pub source_config_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SequencerL2Source {
    descriptor: L2SourceDescriptor,
}

impl SequencerL2Source {
    pub(crate) fn parse(descriptor: L2SourceDescriptor) -> Result<Self, L2SourceError> {
        if descriptor.role != ZoneSourceRole::Sequencer {
            return Err(L2SourceError::capability());
        }
        Ok(Self { descriptor })
    }

    #[must_use]
    #[cfg(test)]
    pub(crate) fn source_id(&self) -> &str {
        &self.descriptor.source_id
    }

    #[must_use]
    fn target(&self) -> &ChannelSourceTarget {
        &self.descriptor.target
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexerL2Source {
    descriptor: L2SourceDescriptor,
}

impl IndexerL2Source {
    pub(crate) fn parse(descriptor: L2SourceDescriptor) -> Result<Self, L2SourceError> {
        if descriptor.role != ZoneSourceRole::Indexer {
            return Err(L2SourceError::capability());
        }
        Ok(Self { descriptor })
    }

    #[must_use]
    #[cfg(test)]
    pub(crate) fn source_id(&self) -> &str {
        &self.descriptor.source_id
    }

    #[must_use]
    fn target(&self) -> &ChannelSourceTarget {
        &self.descriptor.target
    }
}

pub(crate) trait SequencerL2SourceAdapter: Send + Sync {
    fn head<'a>(
        &'a self,
        source: SequencerL2Source,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>>;

    fn blocks<'a>(
        &'a self,
        source: SequencerL2Source,
        before: Option<u64>,
        limit: u64,
    ) -> L2SourceFuture<'a, Vec<NormalizedL2Block>>;

    fn block_by_id<'a>(
        &'a self,
        source: SequencerL2Source,
        block_id: u64,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>>;

    fn transaction<'a>(
        &'a self,
        source: SequencerL2Source,
        transaction_id: String,
    ) -> L2SourceFuture<'a, Option<TransactionSummary>>;

    fn current_account<'a>(
        &'a self,
        source: SequencerL2Source,
        account_id: String,
    ) -> L2SourceFuture<'a, L2AccountValue>;

    fn programs<'a>(&'a self, source: SequencerL2Source)
    -> L2SourceFuture<'a, Vec<ProgramIdEntry>>;

    fn commitment_proof<'a>(
        &'a self,
        source: SequencerL2Source,
        commitment_hex: String,
    ) -> L2SourceFuture<'a, Option<(u64, Vec<String>)>>;

    fn account_nonces<'a>(
        &'a self,
        source: SequencerL2Source,
        account_ids: Vec<String>,
    ) -> L2SourceFuture<'a, Vec<String>>;
}

pub(crate) trait IndexerL2SourceAdapter: Send + Sync {
    fn head<'a>(&'a self, source: IndexerL2Source)
    -> L2SourceFuture<'a, Option<NormalizedL2Block>>;

    fn blocks<'a>(
        &'a self,
        source: IndexerL2Source,
        before: Option<u64>,
        limit: u64,
    ) -> L2SourceFuture<'a, Vec<NormalizedL2Block>>;

    fn block_by_id<'a>(
        &'a self,
        source: IndexerL2Source,
        block_id: u64,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>>;

    fn block_by_hash<'a>(
        &'a self,
        source: IndexerL2Source,
        block_hash: String,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>>;

    fn transaction<'a>(
        &'a self,
        source: IndexerL2Source,
        transaction_id: String,
    ) -> L2SourceFuture<'a, Option<TransactionSummary>>;

    fn account_at_block<'a>(
        &'a self,
        source: IndexerL2Source,
        account_id: String,
        block_id: u64,
    ) -> L2SourceFuture<'a, L2AccountValue>;

    fn account_activity<'a>(
        &'a self,
        source: IndexerL2Source,
        account_id: String,
        offset: usize,
        limit: usize,
    ) -> L2SourceFuture<'a, Vec<L2AccountActivityRow>>;

    fn transfer_blocks<'a>(
        &'a self,
        source: IndexerL2Source,
        before: Option<u64>,
        limit: u64,
    ) -> L2SourceFuture<'a, Vec<IndexerBlockReport>>;
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DirectSequencerL2SourceAdapter;

impl SequencerL2SourceAdapter for DirectSequencerL2SourceAdapter {
    fn head<'a>(
        &'a self,
        source: SequencerL2Source,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>> {
        Box::pin(async move {
            sequencer_adapter(&source)?
                .head()
                .await
                .map_err(map_execution_zone_error)?
                .map(normalize_sequencer_block)
                .transpose()
        })
    }

    fn blocks<'a>(
        &'a self,
        source: SequencerL2Source,
        before: Option<u64>,
        limit: u64,
    ) -> L2SourceFuture<'a, Vec<NormalizedL2Block>> {
        Box::pin(async move {
            sequencer_adapter(&source)?
                .blocks(before, limit)
                .await
                .map_err(map_execution_zone_error)?
                .into_iter()
                .map(normalize_sequencer_block)
                .collect()
        })
    }

    fn block_by_id<'a>(
        &'a self,
        source: SequencerL2Source,
        block_id: u64,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>> {
        Box::pin(async move {
            sequencer_adapter(&source)?
                .block_by_id(block_id)
                .await
                .map_err(map_execution_zone_error)?
                .map(normalize_sequencer_block)
                .transpose()
        })
    }

    fn transaction<'a>(
        &'a self,
        source: SequencerL2Source,
        transaction_id: String,
    ) -> L2SourceFuture<'a, Option<TransactionSummary>> {
        Box::pin(async move {
            sequencer_adapter(&source)?
                .transaction(&transaction_id)
                .await
                .map_err(map_execution_zone_error)
        })
    }

    fn current_account<'a>(
        &'a self,
        source: SequencerL2Source,
        account_id: String,
    ) -> L2SourceFuture<'a, L2AccountValue> {
        Box::pin(async move {
            sequencer_adapter(&source)?
                .current_account(&account_id)
                .await
                .map(normalize_account)
                .map_err(map_execution_zone_error)
        })
    }

    fn programs<'a>(
        &'a self,
        source: SequencerL2Source,
    ) -> L2SourceFuture<'a, Vec<ProgramIdEntry>> {
        Box::pin(async move {
            sequencer_adapter(&source)?
                .programs()
                .await
                .map_err(map_execution_zone_error)
        })
    }

    fn commitment_proof<'a>(
        &'a self,
        source: SequencerL2Source,
        commitment_hex: String,
    ) -> L2SourceFuture<'a, Option<(u64, Vec<String>)>> {
        Box::pin(async move {
            sequencer_adapter(&source)?
                .commitment_proof(&commitment_hex)
                .await
                .map_err(map_execution_zone_error)
        })
    }

    fn account_nonces<'a>(
        &'a self,
        source: SequencerL2Source,
        account_ids: Vec<String>,
    ) -> L2SourceFuture<'a, Vec<String>> {
        Box::pin(async move {
            sequencer_adapter(&source)?
                .account_nonces(&account_ids)
                .await
                .map_err(map_execution_zone_error)
        })
    }
}

#[derive(Clone)]
pub(crate) struct DirectIndexerL2SourceAdapter {
    module_transport: SharedModuleTransport,
    module_transport_kind: ModuleTransportKind,
    use_channel_indexer_runtime: bool,
}

impl Default for DirectIndexerL2SourceAdapter {
    fn default() -> Self {
        Self::with_channel_indexer_runtime(
            Arc::new(LogoscoreCliTransport::default()),
            ModuleTransportKind::LogoscoreCli,
        )
    }
}

impl DirectIndexerL2SourceAdapter {
    #[must_use]
    pub(crate) fn new(
        module_transport: SharedModuleTransport,
        module_transport_kind: ModuleTransportKind,
    ) -> Self {
        Self {
            module_transport,
            module_transport_kind,
            use_channel_indexer_runtime: false,
        }
    }

    #[must_use]
    pub(crate) fn with_channel_indexer_runtime(
        module_transport: SharedModuleTransport,
        module_transport_kind: ModuleTransportKind,
    ) -> Self {
        Self {
            module_transport,
            module_transport_kind,
            use_channel_indexer_runtime: true,
        }
    }

    fn module_transport_for(
        &self,
        source: &IndexerL2Source,
    ) -> Result<SharedModuleTransport, L2SourceError> {
        if !matches!(source.target(), ChannelSourceTarget::Module { module_id } if module_id == MODULE_ID)
        {
            return Ok(Arc::clone(&self.module_transport));
        }
        if self.module_transport_kind == ModuleTransportKind::Module {
            return crate::local_nodes::basecamp_channel_indexer_module_transport(
                Arc::clone(&self.module_transport),
                &source.descriptor.network_scope,
                &source.descriptor.channel_id,
            )
            .map_err(|_| L2SourceError::unavailable());
        }
        if !self.use_channel_indexer_runtime {
            return Ok(Arc::clone(&self.module_transport));
        }
        crate::local_nodes::channel_indexer_module_transport(
            &source.descriptor.network_scope,
            &source.descriptor.channel_id,
            source.descriptor.source_config_revision,
            &source.descriptor.source_id,
        )
        .map_err(|_| L2SourceError::unavailable())
    }
}

impl IndexerL2SourceAdapter for DirectIndexerL2SourceAdapter {
    fn head<'a>(
        &'a self,
        source: IndexerL2Source,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>> {
        Box::pin(async move {
            let module_transport = self.module_transport_for(&source)?;
            indexer_adapter(&source, &module_transport, self.module_transport_kind)?
                .head()
                .await
                .map_err(map_execution_zone_error)?
                .map(normalize_indexer_block)
                .transpose()
        })
    }

    fn blocks<'a>(
        &'a self,
        source: IndexerL2Source,
        before: Option<u64>,
        limit: u64,
    ) -> L2SourceFuture<'a, Vec<NormalizedL2Block>> {
        Box::pin(async move {
            let module_transport = self.module_transport_for(&source)?;
            indexer_adapter(&source, &module_transport, self.module_transport_kind)?
                .blocks(before, limit)
                .await
                .map_err(map_execution_zone_error)?
                .into_iter()
                .map(normalize_indexer_block)
                .collect()
        })
    }

    fn block_by_id<'a>(
        &'a self,
        source: IndexerL2Source,
        block_id: u64,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>> {
        Box::pin(async move {
            let module_transport = self.module_transport_for(&source)?;
            indexer_adapter(&source, &module_transport, self.module_transport_kind)?
                .block_by_id(block_id)
                .await
                .map_err(map_execution_zone_error)?
                .map(normalize_indexer_block)
                .transpose()
        })
    }

    fn block_by_hash<'a>(
        &'a self,
        source: IndexerL2Source,
        block_hash: String,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>> {
        Box::pin(async move {
            let module_transport = self.module_transport_for(&source)?;
            indexer_adapter(&source, &module_transport, self.module_transport_kind)?
                .block_by_hash(&block_hash)
                .await
                .map_err(map_execution_zone_error)?
                .map(normalize_indexer_block)
                .transpose()
        })
    }

    fn transaction<'a>(
        &'a self,
        source: IndexerL2Source,
        transaction_id: String,
    ) -> L2SourceFuture<'a, Option<TransactionSummary>> {
        Box::pin(async move {
            let module_transport = self.module_transport_for(&source)?;
            indexer_adapter(&source, &module_transport, self.module_transport_kind)?
                .transaction(&transaction_id)
                .await
                .map_err(map_execution_zone_error)
        })
    }

    fn account_at_block<'a>(
        &'a self,
        source: IndexerL2Source,
        account_id: String,
        block_id: u64,
    ) -> L2SourceFuture<'a, L2AccountValue> {
        Box::pin(async move {
            let module_transport = self.module_transport_for(&source)?;
            indexer_adapter(&source, &module_transport, self.module_transport_kind)?
                .account_at_block(&account_id, block_id)
                .await
                .map(normalize_account)
                .map_err(map_execution_zone_error)
        })
    }

    fn account_activity<'a>(
        &'a self,
        source: IndexerL2Source,
        account_id: String,
        offset: usize,
        limit: usize,
    ) -> L2SourceFuture<'a, Vec<L2AccountActivityRow>> {
        Box::pin(async move {
            let module_transport = self.module_transport_for(&source)?;
            indexer_adapter(&source, &module_transport, self.module_transport_kind)?
                .account_activity(&account_id, offset, limit)
                .await
                .map(|rows| rows.into_iter().map(normalize_activity_row).collect())
                .map_err(map_execution_zone_error)
        })
    }

    fn transfer_blocks<'a>(
        &'a self,
        source: IndexerL2Source,
        before: Option<u64>,
        limit: u64,
    ) -> L2SourceFuture<'a, Vec<IndexerBlockReport>> {
        Box::pin(async move {
            let module_transport = self.module_transport_for(&source)?;
            indexer_adapter(&source, &module_transport, self.module_transport_kind)?
                .blocks(before, limit)
                .await
                .map_err(map_execution_zone_error)
        })
    }
}

fn sequencer_adapter(source: &SequencerL2Source) -> Result<SequencerAdapter<'_>, L2SourceError> {
    SequencerAdapter::connect(source.target()).map_err(map_execution_zone_error)
}

fn indexer_adapter<'a>(
    source: &'a IndexerL2Source,
    module_transport: &SharedModuleTransport,
    module_transport_kind: ModuleTransportKind,
) -> Result<IndexerAdapter<'a>, L2SourceError> {
    IndexerAdapter::connect(source.target(), module_transport, module_transport_kind)
        .map_err(map_execution_zone_error)
}

fn map_execution_zone_error(error: ExecutionZoneReadError) -> L2SourceError {
    match error.kind {
        ExecutionZoneReadErrorKind::Unavailable => L2SourceError::unavailable(),
        ExecutionZoneReadErrorKind::Protocol => L2SourceError::protocol_error(),
        ExecutionZoneReadErrorKind::Capability => L2SourceError::capability(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use anyhow::{Context as _, Result, ensure};
    use serde_json::json;
    use sha2::{Digest as _, Sha256};

    use super::*;
    use crate::modules::logos_core::{
        ModuleCall, ModuleCallFuture, ModuleCallReply, ModuleTransport,
    };

    struct RecordingModuleTransport {
        calls: Arc<Mutex<Vec<ModuleCall>>>,
    }

    impl ModuleTransport for RecordingModuleTransport {
        fn kind(&self) -> ModuleTransportKind {
            ModuleTransportKind::Module
        }

        fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
            Box::pin(async move {
                self.calls
                    .lock()
                    .map_err(|_| anyhow::anyhow!("recorded module calls lock poisoned"))?
                    .push(call);
                Ok(ModuleCallReply::new(ModuleTransportKind::Module, json!(17)))
            })
        }
    }

    fn basecamp_indexer_source(channel_id: &str) -> Result<IndexerL2Source> {
        IndexerL2Source::parse(L2SourceDescriptor {
            network_scope: NetworkScope::GenesisId {
                genesis_id: "a".repeat(64),
            },
            channel_id: channel_id.to_owned(),
            source_id: format!("indexer-{channel_id}"),
            role: ZoneSourceRole::Indexer,
            target: ChannelSourceTarget::Module {
                module_id: MODULE_ID.to_owned(),
            },
            source_config_revision: 1,
        })
        .map_err(|error| anyhow::anyhow!("invalid test Indexer source: {:?}", error.kind))
    }

    #[test]
    fn adapter_error_mapping_preserves_all_error_classes() {
        for (source, expected) in [
            (
                ExecutionZoneReadErrorKind::Unavailable,
                L2SourceErrorKind::Unavailable,
            ),
            (
                ExecutionZoneReadErrorKind::Protocol,
                L2SourceErrorKind::Protocol,
            ),
            (
                ExecutionZoneReadErrorKind::Capability,
                L2SourceErrorKind::Capability,
            ),
        ] {
            let mapped = map_execution_zone_error(ExecutionZoneReadError { kind: source });
            assert_eq!(mapped.kind, expected);
        }
    }

    #[tokio::test]
    async fn basecamp_indexer_reads_use_distinct_channel_instances() -> Result<()> {
        let first_channel = "1".repeat(64);
        let second_channel = "2".repeat(64);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let transport: SharedModuleTransport = Arc::new(RecordingModuleTransport {
            calls: Arc::clone(&calls),
        });
        let adapter = DirectIndexerL2SourceAdapter::new(transport, ModuleTransportKind::Module);

        for source in [
            basecamp_indexer_source(&first_channel)?,
            basecamp_indexer_source(&second_channel)?,
        ] {
            let transport = adapter.module_transport_for(&source).map_err(|error| {
                anyhow::anyhow!(
                    "Basecamp Indexer transport was unavailable: {:?}",
                    error.kind
                )
            })?;
            let head = indexer_adapter(&source, &transport, ModuleTransportKind::Module)
                .map_err(|error| {
                    anyhow::anyhow!("Basecamp Indexer adapter was unavailable: {:?}", error.kind)
                })?
                .reported_head_id()
                .await
                .map_err(|error| {
                    anyhow::anyhow!("Indexer finalized-head read failed: {error:?}")
                })?;
            ensure!(head == Some(17), "Indexer finalized-head result changed");
        }

        let calls = calls
            .lock()
            .map_err(|_| anyhow::anyhow!("recorded module calls lock poisoned"))?;
        let [first_call, second_call] = calls.as_slice() else {
            return Err(anyhow::anyhow!("expected exactly two Indexer reads"));
        };
        for (call, channel_id) in calls.iter().zip([&first_channel, &second_channel]) {
            ensure!(
                call.module() == MODULE_ID,
                "Indexer read used another module"
            );
            ensure!(
                call.method() == "getLastFinalizedBlockId",
                "unexpected Indexer read method `{}`",
                call.method()
            );
            let instance_id = call
                .instance_id()
                .context("Basecamp Indexer read fell back to a default instance")?;
            let channel_key = hex::encode(Sha256::digest(channel_id.as_bytes()));
            ensure!(
                instance_id.ends_with(&channel_key[..32]),
                "Indexer read used an instance for another Channel"
            );
        }
        ensure!(
            first_call.instance_id() != second_call.instance_id(),
            "two Zone Channels shared one Basecamp Indexer instance"
        );
        Ok(())
    }
}
