use std::{future::Future, pin::Pin};

use super::{
    L2AccountActivityRow, L2AccountValue, NormalizedL2Block, normalize_account,
    normalize_activity_row, normalize_indexer_block, normalize_sequencer_block,
};
use crate::{
    inspection::ZoneSourceRole,
    lez::{IndexerBlockReport, ProgramIdEntry, TransactionSummary},
    source_routing::{
        channel_sources::{ChannelSourceRole, ChannelSourceTarget},
        execution_zone_layer,
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

pub(crate) trait ZoneL2SourceAdapter: Send + Sync {
    fn head<'a>(
        &'a self,
        _source: L2SourceDescriptor,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>> {
        Box::pin(async { Err(L2SourceError::capability()) })
    }

    fn blocks<'a>(
        &'a self,
        _source: L2SourceDescriptor,
        _before: Option<u64>,
        _limit: u64,
    ) -> L2SourceFuture<'a, Vec<NormalizedL2Block>> {
        Box::pin(async { Err(L2SourceError::capability()) })
    }

    fn block_by_id<'a>(
        &'a self,
        _source: L2SourceDescriptor,
        _block_id: u64,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>> {
        Box::pin(async { Err(L2SourceError::capability()) })
    }

    fn block_by_hash<'a>(
        &'a self,
        _source: L2SourceDescriptor,
        _block_hash: String,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>> {
        Box::pin(async { Err(L2SourceError::capability()) })
    }

    fn transaction<'a>(
        &'a self,
        _source: L2SourceDescriptor,
        _transaction_id: String,
    ) -> L2SourceFuture<'a, Option<TransactionSummary>> {
        Box::pin(async { Err(L2SourceError::capability()) })
    }

    fn current_account<'a>(
        &'a self,
        _source: L2SourceDescriptor,
        _account_id: String,
    ) -> L2SourceFuture<'a, L2AccountValue> {
        Box::pin(async { Err(L2SourceError::capability()) })
    }

    fn account_at_block<'a>(
        &'a self,
        _source: L2SourceDescriptor,
        _account_id: String,
        _block_id: u64,
    ) -> L2SourceFuture<'a, L2AccountValue> {
        Box::pin(async { Err(L2SourceError::capability()) })
    }

    fn account_activity<'a>(
        &'a self,
        _source: L2SourceDescriptor,
        _account_id: String,
        _offset: usize,
        _limit: usize,
    ) -> L2SourceFuture<'a, Vec<L2AccountActivityRow>> {
        Box::pin(async { Err(L2SourceError::capability()) })
    }

    fn programs<'a>(
        &'a self,
        _source: L2SourceDescriptor,
    ) -> L2SourceFuture<'a, Vec<ProgramIdEntry>> {
        Box::pin(async { Err(L2SourceError::capability()) })
    }

    fn commitment_proof<'a>(
        &'a self,
        _source: L2SourceDescriptor,
        _commitment_hex: String,
    ) -> L2SourceFuture<'a, Option<(u64, Vec<String>)>> {
        Box::pin(async { Err(L2SourceError::capability()) })
    }

    fn account_nonces<'a>(
        &'a self,
        _source: L2SourceDescriptor,
        _account_ids: Vec<String>,
    ) -> L2SourceFuture<'a, Vec<String>> {
        Box::pin(async { Err(L2SourceError::capability()) })
    }

    fn transfer_blocks<'a>(
        &'a self,
        _source: L2SourceDescriptor,
        _before: Option<u64>,
        _limit: u64,
    ) -> L2SourceFuture<'a, Vec<IndexerBlockReport>> {
        Box::pin(async { Err(L2SourceError::capability()) })
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DirectZoneL2SourceAdapter;

impl ZoneL2SourceAdapter for DirectZoneL2SourceAdapter {
    fn head<'a>(
        &'a self,
        source: L2SourceDescriptor,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>> {
        Box::pin(async move {
            execution_zone_adapter(&source)
                .head()
                .await
                .map_err(map_execution_zone_error)?
                .map(normalize_execution_zone_block)
                .transpose()
        })
    }

    fn blocks<'a>(
        &'a self,
        source: L2SourceDescriptor,
        before: Option<u64>,
        limit: u64,
    ) -> L2SourceFuture<'a, Vec<NormalizedL2Block>> {
        Box::pin(async move {
            execution_zone_adapter(&source)
                .blocks(before, limit)
                .await
                .map_err(map_execution_zone_error)?
                .into_iter()
                .map(normalize_execution_zone_block)
                .collect()
        })
    }

    fn block_by_id<'a>(
        &'a self,
        source: L2SourceDescriptor,
        block_id: u64,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>> {
        Box::pin(async move {
            execution_zone_adapter(&source)
                .block_by_id(block_id)
                .await
                .map_err(map_execution_zone_error)?
                .map(normalize_execution_zone_block)
                .transpose()
        })
    }

    fn block_by_hash<'a>(
        &'a self,
        source: L2SourceDescriptor,
        block_hash: String,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>> {
        Box::pin(async move {
            execution_zone_adapter(&source)
                .block_by_hash(&block_hash)
                .await
                .map_err(map_execution_zone_error)?
                .map(normalize_execution_zone_block)
                .transpose()
        })
    }

    fn transaction<'a>(
        &'a self,
        source: L2SourceDescriptor,
        transaction_id: String,
    ) -> L2SourceFuture<'a, Option<TransactionSummary>> {
        Box::pin(async move {
            execution_zone_adapter(&source)
                .transaction(&transaction_id)
                .await
                .map_err(map_execution_zone_error)
        })
    }

    fn current_account<'a>(
        &'a self,
        source: L2SourceDescriptor,
        account_id: String,
    ) -> L2SourceFuture<'a, L2AccountValue> {
        Box::pin(async move {
            execution_zone_adapter(&source)
                .current_account(&account_id)
                .await
                .map(normalize_account)
                .map_err(map_execution_zone_error)
        })
    }

    fn account_at_block<'a>(
        &'a self,
        source: L2SourceDescriptor,
        account_id: String,
        block_id: u64,
    ) -> L2SourceFuture<'a, L2AccountValue> {
        Box::pin(async move {
            execution_zone_adapter(&source)
                .account_at_block(&account_id, block_id)
                .await
                .map(normalize_account)
                .map_err(map_execution_zone_error)
        })
    }

    fn account_activity<'a>(
        &'a self,
        source: L2SourceDescriptor,
        account_id: String,
        offset: usize,
        limit: usize,
    ) -> L2SourceFuture<'a, Vec<L2AccountActivityRow>> {
        Box::pin(async move {
            execution_zone_adapter(&source)
                .account_activity(&account_id, offset, limit)
                .await
                .map(|rows| rows.into_iter().map(normalize_activity_row).collect())
                .map_err(map_execution_zone_error)
        })
    }

    fn programs<'a>(
        &'a self,
        source: L2SourceDescriptor,
    ) -> L2SourceFuture<'a, Vec<ProgramIdEntry>> {
        Box::pin(async move {
            execution_zone_adapter(&source)
                .programs()
                .await
                .map_err(map_execution_zone_error)
        })
    }

    fn commitment_proof<'a>(
        &'a self,
        source: L2SourceDescriptor,
        commitment_hex: String,
    ) -> L2SourceFuture<'a, Option<(u64, Vec<String>)>> {
        Box::pin(async move {
            execution_zone_adapter(&source)
                .commitment_proof(&commitment_hex)
                .await
                .map_err(map_execution_zone_error)
        })
    }

    fn account_nonces<'a>(
        &'a self,
        source: L2SourceDescriptor,
        account_ids: Vec<String>,
    ) -> L2SourceFuture<'a, Vec<String>> {
        Box::pin(async move {
            execution_zone_adapter(&source)
                .account_nonces(&account_ids)
                .await
                .map_err(map_execution_zone_error)
        })
    }

    fn transfer_blocks<'a>(
        &'a self,
        source: L2SourceDescriptor,
        before: Option<u64>,
        limit: u64,
    ) -> L2SourceFuture<'a, Vec<IndexerBlockReport>> {
        Box::pin(async move {
            execution_zone_adapter(&source)
                .transfer_blocks(before, limit)
                .await
                .map_err(map_execution_zone_error)
        })
    }
}

fn execution_zone_adapter(
    source: &L2SourceDescriptor,
) -> execution_zone_layer::ExecutionZoneAdapter<'_> {
    let role = match source.role {
        ZoneSourceRole::Sequencer => ChannelSourceRole::Sequencer,
        ZoneSourceRole::Indexer => ChannelSourceRole::Indexer,
    };
    execution_zone_layer::ExecutionZoneAdapter::select(role, &source.target)
}

fn normalize_execution_zone_block(
    block: execution_zone_layer::ExecutionZoneBlock,
) -> Result<NormalizedL2Block, L2SourceError> {
    match block {
        execution_zone_layer::ExecutionZoneBlock::Sequencer(block) => {
            normalize_sequencer_block(block)
        }
        execution_zone_layer::ExecutionZoneBlock::Indexer(block) => normalize_indexer_block(block),
    }
}

fn map_execution_zone_error(error: execution_zone_layer::ExecutionZoneReadError) -> L2SourceError {
    match error.kind {
        execution_zone_layer::ExecutionZoneReadErrorKind::Unavailable => {
            L2SourceError::unavailable()
        }
        execution_zone_layer::ExecutionZoneReadErrorKind::Protocol => {
            L2SourceError::protocol_error()
        }
        execution_zone_layer::ExecutionZoneReadErrorKind::Capability => L2SourceError::capability(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_error_mapping_preserves_all_error_classes() {
        for (source, expected) in [
            (
                execution_zone_layer::ExecutionZoneReadErrorKind::Unavailable,
                L2SourceErrorKind::Unavailable,
            ),
            (
                execution_zone_layer::ExecutionZoneReadErrorKind::Protocol,
                L2SourceErrorKind::Protocol,
            ),
            (
                execution_zone_layer::ExecutionZoneReadErrorKind::Capability,
                L2SourceErrorKind::Capability,
            ),
        ] {
            let mapped = map_execution_zone_error(execution_zone_layer::ExecutionZoneReadError {
                kind: source,
            });
            assert_eq!(mapped.kind, expected);
        }
    }
}
