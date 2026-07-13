use std::{future::Future, pin::Pin};

use super::{
    L2AccountActivityRow, L2AccountValue, NormalizedL2Block, normalize_account,
    normalize_activity_row, normalize_indexer_block, normalize_sequencer_block,
};
use crate::{
    inspection::ZoneSourceRole,
    lez::{IndexerBlockReport, ProgramIdEntry, TransactionSummary},
    source_routing::channel_sources::{
        ChannelSourceTarget,
        indexer::IndexerAdapter,
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

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DirectIndexerL2SourceAdapter;

impl IndexerL2SourceAdapter for DirectIndexerL2SourceAdapter {
    fn head<'a>(
        &'a self,
        source: IndexerL2Source,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>> {
        Box::pin(async move {
            indexer_adapter(&source)?
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
            indexer_adapter(&source)?
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
            indexer_adapter(&source)?
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
            indexer_adapter(&source)?
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
            indexer_adapter(&source)?
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
            indexer_adapter(&source)?
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
            indexer_adapter(&source)?
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
            indexer_adapter(&source)?
                .blocks(before, limit)
                .await
                .map_err(map_execution_zone_error)
        })
    }
}

fn sequencer_adapter(source: &SequencerL2Source) -> Result<SequencerAdapter<'_>, L2SourceError> {
    SequencerAdapter::connect(source.target()).map_err(map_execution_zone_error)
}

fn indexer_adapter(source: &IndexerL2Source) -> Result<IndexerAdapter<'_>, L2SourceError> {
    IndexerAdapter::connect(source.target()).map_err(map_execution_zone_error)
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
    use super::*;

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
}
