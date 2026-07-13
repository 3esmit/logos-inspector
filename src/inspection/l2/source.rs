use std::{future::Future, pin::Pin};

use serde_json::Value;

use super::{
    L2AccountActivityRow, L2AccountValue, NormalizedL2Block, normalize_account,
    normalize_activity_row, normalize_indexer_block, normalize_sequencer_block,
};
use crate::{
    inspection::ZoneSourceRole,
    lez::{IndexerBlockReport, ProgramIdEntry, TransactionSummary},
    source_routing::{channel_sources::ChannelSourceTarget, execution_zone_layer},
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
            match (&source.role, &source.target) {
                (ZoneSourceRole::Sequencer, ChannelSourceTarget::Rpc { endpoint }) => {
                    let block_id = execution_zone_layer::sequencer_last_block_id(endpoint)
                        .await
                        .map_err(map_direct_error)?;
                    execution_zone_layer::sequencer_block(endpoint, block_id)
                        .await
                        .map_err(map_direct_error)?
                        .map(normalize_sequencer_block)
                        .transpose()
                }
                (ZoneSourceRole::Indexer, ChannelSourceTarget::Rpc { endpoint }) => {
                    let Some(block_id) = execution_zone_layer::indexer_finalized_block_id(endpoint)
                        .await
                        .map_err(map_direct_error)?
                    else {
                        return Ok(None);
                    };
                    execution_zone_layer::indexer_block_by_id(endpoint, block_id)
                        .await
                        .map_err(map_direct_error)?
                        .map(normalize_indexer_block)
                        .transpose()
                }
                (ZoneSourceRole::Indexer, ChannelSourceTarget::Module { .. }) => {
                    module_indexer_head().await
                }
                (ZoneSourceRole::Sequencer, ChannelSourceTarget::Module { .. }) => {
                    Err(L2SourceError::capability())
                }
            }
        })
    }

    fn blocks<'a>(
        &'a self,
        source: L2SourceDescriptor,
        before: Option<u64>,
        limit: u64,
    ) -> L2SourceFuture<'a, Vec<NormalizedL2Block>> {
        Box::pin(async move {
            match (&source.role, &source.target) {
                (ZoneSourceRole::Sequencer, ChannelSourceTarget::Rpc { endpoint }) => {
                    execution_zone_layer::sequencer_blocks(endpoint, before, limit)
                        .await
                        .map_err(map_direct_error)?
                        .into_iter()
                        .map(normalize_sequencer_block)
                        .collect()
                }
                (ZoneSourceRole::Indexer, ChannelSourceTarget::Rpc { endpoint }) => {
                    execution_zone_layer::indexer_blocks(endpoint, before, limit)
                        .await
                        .map_err(map_direct_error)?
                        .into_iter()
                        .map(normalize_indexer_block)
                        .collect()
                }
                (ZoneSourceRole::Indexer, ChannelSourceTarget::Module { .. }) => {
                    module_indexer_blocks(before, limit).await
                }
                (ZoneSourceRole::Sequencer, ChannelSourceTarget::Module { .. }) => {
                    Err(L2SourceError::capability())
                }
            }
        })
    }

    fn block_by_id<'a>(
        &'a self,
        source: L2SourceDescriptor,
        block_id: u64,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>> {
        Box::pin(async move {
            match (&source.role, &source.target) {
                (ZoneSourceRole::Sequencer, ChannelSourceTarget::Rpc { endpoint }) => {
                    execution_zone_layer::sequencer_block(endpoint, block_id)
                        .await
                        .map_err(map_direct_error)?
                        .map(normalize_sequencer_block)
                        .transpose()
                }
                (ZoneSourceRole::Indexer, ChannelSourceTarget::Rpc { endpoint }) => {
                    execution_zone_layer::indexer_block_by_id(endpoint, block_id)
                        .await
                        .map_err(map_direct_error)?
                        .map(normalize_indexer_block)
                        .transpose()
                }
                (ZoneSourceRole::Indexer, ChannelSourceTarget::Module { .. }) => {
                    module_indexer_block_by_id(block_id).await
                }
                (ZoneSourceRole::Sequencer, ChannelSourceTarget::Module { .. }) => {
                    Err(L2SourceError::capability())
                }
            }
        })
    }

    fn block_by_hash<'a>(
        &'a self,
        source: L2SourceDescriptor,
        block_hash: String,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>> {
        Box::pin(async move {
            match (&source.role, &source.target) {
                (ZoneSourceRole::Indexer, ChannelSourceTarget::Rpc { endpoint }) => {
                    execution_zone_layer::indexer_block_by_hash(endpoint, &block_hash)
                        .await
                        .map_err(map_direct_error)?
                        .map(normalize_indexer_block)
                        .transpose()
                }
                (ZoneSourceRole::Indexer, ChannelSourceTarget::Module { .. }) => {
                    module_indexer_block_by_hash(block_hash).await
                }
                (ZoneSourceRole::Sequencer, _) => Err(L2SourceError::capability()),
            }
        })
    }

    fn transaction<'a>(
        &'a self,
        source: L2SourceDescriptor,
        transaction_id: String,
    ) -> L2SourceFuture<'a, Option<TransactionSummary>> {
        Box::pin(async move {
            match (&source.role, &source.target) {
                (ZoneSourceRole::Sequencer, ChannelSourceTarget::Rpc { endpoint }) => {
                    execution_zone_layer::sequencer_transaction(endpoint, &transaction_id)
                        .await
                        .map_err(map_direct_error)
                }
                (ZoneSourceRole::Indexer, ChannelSourceTarget::Rpc { endpoint }) => {
                    execution_zone_layer::indexer_transaction(endpoint, &transaction_id)
                        .await
                        .map_err(map_direct_error)
                }
                (ZoneSourceRole::Indexer, ChannelSourceTarget::Module { .. }) => {
                    module_indexer_transaction(transaction_id).await
                }
                (ZoneSourceRole::Sequencer, ChannelSourceTarget::Module { .. }) => {
                    Err(L2SourceError::capability())
                }
            }
        })
    }

    fn current_account<'a>(
        &'a self,
        source: L2SourceDescriptor,
        account_id: String,
    ) -> L2SourceFuture<'a, L2AccountValue> {
        Box::pin(async move {
            match (&source.role, &source.target) {
                (ZoneSourceRole::Sequencer, ChannelSourceTarget::Rpc { endpoint }) => {
                    execution_zone_layer::sequencer_account(endpoint, &account_id)
                        .await
                        .map(normalize_account)
                        .map_err(map_direct_error)
                }
                (ZoneSourceRole::Sequencer, ChannelSourceTarget::Module { .. })
                | (ZoneSourceRole::Indexer, _) => Err(L2SourceError::capability()),
            }
        })
    }

    fn account_at_block<'a>(
        &'a self,
        source: L2SourceDescriptor,
        account_id: String,
        block_id: u64,
    ) -> L2SourceFuture<'a, L2AccountValue> {
        Box::pin(async move {
            match (&source.role, &source.target) {
                (ZoneSourceRole::Indexer, ChannelSourceTarget::Rpc { endpoint }) => {
                    execution_zone_layer::indexer_account_at_block(endpoint, &account_id, block_id)
                        .await
                        .map(normalize_account)
                        .map_err(map_direct_error)
                }
                (ZoneSourceRole::Indexer, ChannelSourceTarget::Module { .. }) => {
                    module_indexer_account_at_block(account_id, block_id).await
                }
                (ZoneSourceRole::Sequencer, _) => Err(L2SourceError::capability()),
            }
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
            let rows = match (&source.role, &source.target) {
                (ZoneSourceRole::Indexer, ChannelSourceTarget::Rpc { endpoint }) => {
                    execution_zone_layer::indexer_account_activity(
                        endpoint,
                        &account_id,
                        offset,
                        limit,
                    )
                    .await
                    .map_err(map_direct_error)?
                }
                (ZoneSourceRole::Indexer, ChannelSourceTarget::Module { .. }) => {
                    module_indexer_account_activity(account_id, offset, limit).await?
                }
                (ZoneSourceRole::Sequencer, _) => return Err(L2SourceError::capability()),
            };
            Ok(rows.into_iter().map(normalize_activity_row).collect())
        })
    }

    fn programs<'a>(
        &'a self,
        source: L2SourceDescriptor,
    ) -> L2SourceFuture<'a, Vec<ProgramIdEntry>> {
        Box::pin(async move {
            match (&source.role, &source.target) {
                (ZoneSourceRole::Sequencer, ChannelSourceTarget::Rpc { endpoint }) => {
                    execution_zone_layer::sequencer_program_ids(endpoint)
                        .await
                        .map_err(map_direct_error)
                }
                _ => Err(L2SourceError::capability()),
            }
        })
    }

    fn commitment_proof<'a>(
        &'a self,
        source: L2SourceDescriptor,
        commitment_hex: String,
    ) -> L2SourceFuture<'a, Option<(u64, Vec<String>)>> {
        Box::pin(async move {
            match (&source.role, &source.target) {
                (ZoneSourceRole::Sequencer, ChannelSourceTarget::Rpc { endpoint }) => {
                    execution_zone_layer::sequencer_commitment_proof(endpoint, &commitment_hex)
                        .await
                        .map_err(map_direct_error)
                }
                _ => Err(L2SourceError::capability()),
            }
        })
    }

    fn account_nonces<'a>(
        &'a self,
        source: L2SourceDescriptor,
        account_ids: Vec<String>,
    ) -> L2SourceFuture<'a, Vec<String>> {
        Box::pin(async move {
            match (&source.role, &source.target) {
                (ZoneSourceRole::Sequencer, ChannelSourceTarget::Rpc { endpoint }) => {
                    execution_zone_layer::sequencer_account_nonces(endpoint, &account_ids)
                        .await
                        .map_err(map_direct_error)
                }
                _ => Err(L2SourceError::capability()),
            }
        })
    }

    fn transfer_blocks<'a>(
        &'a self,
        source: L2SourceDescriptor,
        before: Option<u64>,
        limit: u64,
    ) -> L2SourceFuture<'a, Vec<IndexerBlockReport>> {
        Box::pin(async move {
            match (&source.role, &source.target) {
                (ZoneSourceRole::Indexer, ChannelSourceTarget::Rpc { endpoint }) => {
                    execution_zone_layer::indexer_blocks(endpoint, before, limit)
                        .await
                        .map_err(map_direct_error)
                }
                (ZoneSourceRole::Indexer, ChannelSourceTarget::Module { .. }) => {
                    execution_zone_layer::module_indexer_blocks(before, limit)
                        .await
                        .map_err(map_direct_error)
                }
                (ZoneSourceRole::Sequencer, _) => Err(L2SourceError::capability()),
            }
        })
    }
}

async fn module_indexer_head() -> Result<Option<NormalizedL2Block>, L2SourceError> {
    let block_id = execution_zone_layer::module_indexer_finalized_head()
        .await
        .map_err(map_direct_error)
        .and_then(optional_u64)?;
    let Some(block_id) = block_id else {
        return Ok(None);
    };
    module_indexer_block_by_id(block_id).await
}

async fn module_indexer_blocks(
    before: Option<u64>,
    limit: u64,
) -> Result<Vec<NormalizedL2Block>, L2SourceError> {
    execution_zone_layer::module_indexer_blocks(before, limit)
        .await
        .map_err(map_direct_error)?
        .into_iter()
        .map(normalize_indexer_block)
        .collect()
}

async fn module_indexer_block_by_id(
    block_id: u64,
) -> Result<Option<NormalizedL2Block>, L2SourceError> {
    execution_zone_layer::module_indexer_block_by_id(block_id)
        .await
        .map_err(map_direct_error)?
        .map(normalize_indexer_block)
        .transpose()
}

async fn module_indexer_block_by_hash(
    block_hash: String,
) -> Result<Option<NormalizedL2Block>, L2SourceError> {
    execution_zone_layer::module_indexer_block_by_hash(block_hash)
        .await
        .map_err(map_direct_error)?
        .map(normalize_indexer_block)
        .transpose()
}

async fn module_indexer_transaction(
    transaction_id: String,
) -> Result<Option<TransactionSummary>, L2SourceError> {
    execution_zone_layer::module_indexer_transaction(transaction_id)
        .await
        .map_err(map_direct_error)
}

async fn module_indexer_account_at_block(
    account_id: String,
    block_id: u64,
) -> Result<L2AccountValue, L2SourceError> {
    execution_zone_layer::module_indexer_account_at_block(account_id, block_id)
        .await
        .map(normalize_account)
        .map_err(map_direct_error)
}

async fn module_indexer_account_activity(
    account_id: String,
    offset: usize,
    limit: usize,
) -> Result<Vec<crate::lez::AccountTransactionSummary>, L2SourceError> {
    execution_zone_layer::module_indexer_account_activity(account_id, offset, limit)
        .await
        .map_err(map_direct_error)
}

fn optional_u64(value: Value) -> Result<Option<u64>, L2SourceError> {
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        .map(Some)
        .ok_or_else(L2SourceError::protocol_error)
}

fn map_direct_error(error: anyhow::Error) -> L2SourceError {
    if crate::lez::is_evidence_capability_error(&error) {
        L2SourceError::capability()
    } else if crate::lez::is_evidence_protocol_error(&error) {
        L2SourceError::protocol_error()
    } else {
        L2SourceError::unavailable()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_adapter_preserves_protocol_and_capability_classes() {
        let protocol = map_direct_error(crate::lez::evidence_protocol_error("invalid evidence"));
        let capability =
            map_direct_error(crate::lez::evidence_capability_error("missing capability"));
        let unavailable = map_direct_error(anyhow::anyhow!("transport failed"));

        assert_eq!(protocol.kind, L2SourceErrorKind::Protocol);
        assert_eq!(capability.kind, L2SourceErrorKind::Capability);
        assert_eq!(unavailable.kind, L2SourceErrorKind::Unavailable);
    }
}
