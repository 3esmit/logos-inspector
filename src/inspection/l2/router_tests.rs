use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
};

use anyhow::{Context as _, Result, bail};
use serde_json::json;

use super::*;
use crate::{
    inspection::{
        CatalogCoverageStatus, CatalogVerificationState, CoveragePrefixStatus, L1FinalityState,
        NetworkScope, RawActivitySummary, ZoneFacts, ZoneKind,
        catalog::{CatalogFrontier, CatalogIdentityAssurance, CatalogMetadata, CatalogSnapshot},
        l2::{
            InspectionEntityRef, InspectionResolveTargetRequest, InspectionTargetCandidate,
            InspectionTargetResolutionStatus, L2AccountActivityRow, L2AccountExistence,
            L2AccountValue, L2BlockSummary, L2SourceFuture, ZoneL2EntityKind,
            ZoneL2SourceQualifier,
        },
        project_catalog_zones_with_sources,
    },
    lez::{IndexerBlockReport, ProgramIdEntry, TransactionSummary},
    source_routing::channel_sources::{
        ChannelSourceConfig, ChannelSourceMonitorSnapshot, ChannelSourceTarget,
        ConfiguredIndexerSource, ConfiguredSequencerSource, PersistedSequencerAttestation,
    },
};

type Script<T> = Result<T, L2SourceError>;
type CommitmentProof = Option<(u64, Vec<String>)>;

#[derive(Default)]
struct ScriptedAdapter {
    state: Mutex<ScriptedState>,
}

struct ScriptedSequencerAdapter {
    scripts: Arc<ScriptedAdapter>,
}

struct ScriptedIndexerAdapter {
    scripts: Arc<ScriptedAdapter>,
}

#[derive(Default)]
struct ScriptedState {
    heads: BTreeMap<String, VecDeque<Script<Option<NormalizedL2Block>>>>,
    block_pages: BTreeMap<String, VecDeque<Script<Vec<NormalizedL2Block>>>>,
    blocks_by_id: BTreeMap<(String, u64), VecDeque<Script<Option<NormalizedL2Block>>>>,
    blocks_by_hash: BTreeMap<String, VecDeque<Script<Option<NormalizedL2Block>>>>,
    transactions: BTreeMap<String, VecDeque<Script<Option<TransactionSummary>>>>,
    current_accounts: BTreeMap<String, VecDeque<Script<L2AccountValue>>>,
    accounts_at_block: BTreeMap<(String, u64), VecDeque<Script<L2AccountValue>>>,
    activities: BTreeMap<String, VecDeque<Script<Vec<L2AccountActivityRow>>>>,
    programs: BTreeMap<String, VecDeque<Script<Vec<ProgramIdEntry>>>>,
    proofs: BTreeMap<String, VecDeque<Script<CommitmentProof>>>,
    nonces: BTreeMap<String, VecDeque<Script<Vec<String>>>>,
    transfer_blocks: BTreeMap<String, VecDeque<Script<Vec<IndexerBlockReport>>>>,
    calls: Vec<String>,
}

impl ScriptedAdapter {
    fn edit(&self, edit: impl FnOnce(&mut ScriptedState)) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("scripted adapter lock failed"))?;
        edit(&mut state);
        Ok(())
    }

    fn calls(&self) -> Result<Vec<String>> {
        self.state
            .lock()
            .map(|state| state.calls.clone())
            .map_err(|_| anyhow::anyhow!("scripted adapter lock failed"))
    }

    fn take<T>(
        &self,
        call: String,
        select: impl FnOnce(&mut ScriptedState) -> Option<Script<T>>,
    ) -> Script<T> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| L2SourceError::unavailable())?;
        state.calls.push(call);
        select(&mut state).unwrap_or_else(|| Err(L2SourceError::capability()))
    }

    fn head(&self, source_id: String) -> Script<Option<NormalizedL2Block>> {
        self.take(format!("head:{source_id}"), |state| {
            state.heads.get_mut(&source_id)?.pop_front()
        })
    }

    fn blocks(
        &self,
        source_id: String,
        before: Option<u64>,
        limit: u64,
    ) -> Script<Vec<NormalizedL2Block>> {
        self.take(format!("blocks:{source_id}:{before:?}:{limit}"), |state| {
            state.block_pages.get_mut(&source_id)?.pop_front()
        })
    }

    fn block_by_id(&self, source_id: String, block_id: u64) -> Script<Option<NormalizedL2Block>> {
        let key = (source_id.clone(), block_id);
        self.take(format!("block_id:{source_id}:{block_id}"), |state| {
            state.blocks_by_id.get_mut(&key)?.pop_front()
        })
    }

    fn transaction(
        &self,
        source_id: String,
        transaction_id: String,
    ) -> Script<Option<TransactionSummary>> {
        self.take(format!("tx:{source_id}:{transaction_id}"), |state| {
            state.transactions.get_mut(&source_id)?.pop_front()
        })
    }
}

impl SequencerL2SourceAdapter for ScriptedSequencerAdapter {
    fn head<'a>(
        &'a self,
        source: SequencerL2Source,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>> {
        let result = self.scripts.head(source.source_id().to_owned());
        Box::pin(async move { result })
    }

    fn blocks<'a>(
        &'a self,
        source: SequencerL2Source,
        before: Option<u64>,
        limit: u64,
    ) -> L2SourceFuture<'a, Vec<NormalizedL2Block>> {
        let result = self
            .scripts
            .blocks(source.source_id().to_owned(), before, limit);
        Box::pin(async move { result })
    }

    fn block_by_id<'a>(
        &'a self,
        source: SequencerL2Source,
        block_id: u64,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>> {
        let result = self
            .scripts
            .block_by_id(source.source_id().to_owned(), block_id);
        Box::pin(async move { result })
    }

    fn transaction<'a>(
        &'a self,
        source: SequencerL2Source,
        transaction_id: String,
    ) -> L2SourceFuture<'a, Option<TransactionSummary>> {
        let result = self
            .scripts
            .transaction(source.source_id().to_owned(), transaction_id);
        Box::pin(async move { result })
    }

    fn current_account<'a>(
        &'a self,
        source: SequencerL2Source,
        account_id: String,
    ) -> L2SourceFuture<'a, L2AccountValue> {
        let source_id = source.source_id().to_owned();
        let result = self.scripts.take(
            format!("current_account:{source_id}:{account_id}"),
            |state| state.current_accounts.get_mut(&source_id)?.pop_front(),
        );
        Box::pin(async move { result })
    }

    fn programs<'a>(
        &'a self,
        source: SequencerL2Source,
    ) -> L2SourceFuture<'a, Vec<ProgramIdEntry>> {
        let source_id = source.source_id().to_owned();
        let result = self.scripts.take(format!("programs:{source_id}"), |state| {
            state.programs.get_mut(&source_id)?.pop_front()
        });
        Box::pin(async move { result })
    }

    fn commitment_proof<'a>(
        &'a self,
        source: SequencerL2Source,
        commitment_hex: String,
    ) -> L2SourceFuture<'a, Option<(u64, Vec<String>)>> {
        let source_id = source.source_id().to_owned();
        let result = self
            .scripts
            .take(format!("proof:{source_id}:{commitment_hex}"), |state| {
                state.proofs.get_mut(&source_id)?.pop_front()
            });
        Box::pin(async move { result })
    }

    fn account_nonces<'a>(
        &'a self,
        source: SequencerL2Source,
        account_ids: Vec<String>,
    ) -> L2SourceFuture<'a, Vec<String>> {
        let source_id = source.source_id().to_owned();
        let result = self.scripts.take(
            format!("nonces:{source_id}:{}", account_ids.len()),
            |state| state.nonces.get_mut(&source_id)?.pop_front(),
        );
        Box::pin(async move { result })
    }
}

impl IndexerL2SourceAdapter for ScriptedIndexerAdapter {
    fn head<'a>(
        &'a self,
        source: IndexerL2Source,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>> {
        let result = self.scripts.head(source.source_id().to_owned());
        Box::pin(async move { result })
    }

    fn blocks<'a>(
        &'a self,
        source: IndexerL2Source,
        before: Option<u64>,
        limit: u64,
    ) -> L2SourceFuture<'a, Vec<NormalizedL2Block>> {
        let result = self
            .scripts
            .blocks(source.source_id().to_owned(), before, limit);
        Box::pin(async move { result })
    }

    fn block_by_id<'a>(
        &'a self,
        source: IndexerL2Source,
        block_id: u64,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>> {
        let result = self
            .scripts
            .block_by_id(source.source_id().to_owned(), block_id);
        Box::pin(async move { result })
    }

    fn block_by_hash<'a>(
        &'a self,
        source: IndexerL2Source,
        block_hash: String,
    ) -> L2SourceFuture<'a, Option<NormalizedL2Block>> {
        let source_id = source.source_id().to_owned();
        let result = self
            .scripts
            .take(format!("block_hash:{source_id}:{block_hash}"), |state| {
                state.blocks_by_hash.get_mut(&source_id)?.pop_front()
            });
        Box::pin(async move { result })
    }

    fn transaction<'a>(
        &'a self,
        source: IndexerL2Source,
        transaction_id: String,
    ) -> L2SourceFuture<'a, Option<TransactionSummary>> {
        let result = self
            .scripts
            .transaction(source.source_id().to_owned(), transaction_id);
        Box::pin(async move { result })
    }

    fn account_at_block<'a>(
        &'a self,
        source: IndexerL2Source,
        account_id: String,
        block_id: u64,
    ) -> L2SourceFuture<'a, L2AccountValue> {
        let source_id = source.source_id().to_owned();
        let key = (source_id.clone(), block_id);
        let result = self.scripts.take(
            format!("account_at:{source_id}:{account_id}:{block_id}"),
            |state| state.accounts_at_block.get_mut(&key)?.pop_front(),
        );
        Box::pin(async move { result })
    }

    fn account_activity<'a>(
        &'a self,
        source: IndexerL2Source,
        account_id: String,
        offset: usize,
        limit: usize,
    ) -> L2SourceFuture<'a, Vec<L2AccountActivityRow>> {
        let source_id = source.source_id().to_owned();
        let result = self.scripts.take(
            format!("activity:{source_id}:{account_id}:{offset}:{limit}"),
            |state| state.activities.get_mut(&source_id)?.pop_front(),
        );
        Box::pin(async move { result })
    }

    fn transfer_blocks<'a>(
        &'a self,
        source: IndexerL2Source,
        before: Option<u64>,
        limit: u64,
    ) -> L2SourceFuture<'a, Vec<IndexerBlockReport>> {
        let source_id = source.source_id().to_owned();
        let result = self.scripts.take(
            format!("transfer_blocks:{source_id}:{before:?}:{limit}"),
            |state| state.transfer_blocks.get_mut(&source_id)?.pop_front(),
        );
        Box::pin(async move { result })
    }
}

fn scripted_router(scripts: Arc<ScriptedAdapter>) -> ZoneL2Router {
    ZoneL2Router::new(
        Arc::new(ScriptedSequencerAdapter {
            scripts: scripts.clone(),
        }),
        Arc::new(ScriptedIndexerAdapter { scripts }),
    )
}

#[tokio::test]
async fn composite_blocks_preserve_conflicts_and_pin_heads() -> Result<()> {
    let (facts, config) = facts(true, true);
    let adapter = Arc::new(ScriptedAdapter::default());
    adapter.edit(|state| {
        state
            .heads
            .entry(indexer_id())
            .or_default()
            .push_back(Ok(Some(block(10, 'a'))));
        state
            .heads
            .entry(sequencer_id())
            .or_default()
            .push_back(Ok(Some(block(12, 'c'))));
        state
            .block_pages
            .entry(indexer_id())
            .or_default()
            .push_back(Ok(vec![block(10, 'a'), block(9, '9'), block(8, '8')]));
        state
            .block_pages
            .entry(sequencer_id())
            .or_default()
            .push_back(Ok(vec![block(12, 'c'), block(11, 'b'), block(10, 'd')]));
    })?;
    let report = scripted_router(adapter)
        .blocks(
            &facts,
            request(
                &config,
                ZoneL2BlocksQuery {
                    cursor: None,
                    limit: Some(3),
                },
            ),
        )
        .await?;
    let serialized = serde_json::to_string(&report)?;
    if serialized.contains("sequencer.example")
        || serialized.contains("indexer.example")
        || serialized.contains("endpoint")
        || serialized.contains("module_id")
    {
        bail!("L2 report exposed a source target");
    }
    let L2ReadOutcome::Found { value } = report.data else {
        bail!("composite block page was not found");
    };
    if report.route_completeness != L2RouteCompleteness::AllConfigured
        || value.distinct_block_ids != 3
        || value.rows.len() != 4
        || value.source_heads.len() != 2
        || value.next_cursor.is_none()
    {
        bail!("unexpected composite page: {value:?}");
    }
    if value
        .rows
        .iter()
        .filter(|row| row.summary.block_id == 10)
        .count()
        != 2
    {
        bail!("conflicting block id did not retain both hashes");
    }
    Ok(())
}

#[tokio::test]
async fn composite_blocks_degrade_only_when_one_planned_source_returns() -> Result<()> {
    let (facts, config) = facts(true, true);
    let adapter = Arc::new(ScriptedAdapter::default());
    adapter.edit(|state| {
        state
            .heads
            .entry(indexer_id())
            .or_default()
            .push_back(Err(L2SourceError::unavailable()));
        state
            .heads
            .entry(sequencer_id())
            .or_default()
            .push_back(Ok(Some(block(2, '2'))));
        state
            .block_pages
            .entry(sequencer_id())
            .or_default()
            .push_back(Ok(vec![block(2, '2'), block(1, '1')]));
    })?;
    let report = scripted_router(adapter)
        .blocks(
            &facts,
            request(
                &config,
                ZoneL2BlocksQuery {
                    cursor: None,
                    limit: Some(2),
                },
            ),
        )
        .await?;
    if report.route_completeness != L2RouteCompleteness::Degraded
        || report.warnings.len() != 1
        || report.route.attempts.first().map(|attempt| attempt.outcome)
            != Some(L2RouteAttemptOutcome::Failed)
    {
        bail!("composite degradation evidence is wrong: {report:?}");
    }
    Ok(())
}

#[tokio::test]
async fn changed_block_anchor_invalidates_cursor() -> Result<()> {
    let (facts, config) = facts(false, true);
    let adapter = Arc::new(ScriptedAdapter::default());
    adapter.edit(|state| {
        state
            .heads
            .entry(sequencer_id())
            .or_default()
            .push_back(Ok(Some(block(3, '3'))));
        state
            .block_pages
            .entry(sequencer_id())
            .or_default()
            .push_back(Ok(vec![block(3, '3'), block(2, '2'), block(1, '1')]));
        state
            .blocks_by_id
            .entry((sequencer_id(), 3))
            .or_default()
            .push_back(Ok(Some(block(3, 'f'))));
    })?;
    let router = scripted_router(adapter);
    let first = router
        .blocks(
            &facts,
            request(
                &config,
                ZoneL2BlocksQuery {
                    cursor: None,
                    limit: Some(1),
                },
            ),
        )
        .await?;
    let L2ReadOutcome::Found { value } = first.data else {
        bail!("first block page was not found");
    };
    let cursor = value.next_cursor.context("first page cursor is missing")?;
    let result = router
        .blocks(
            &facts,
            request(
                &config,
                ZoneL2BlocksQuery {
                    cursor: Some(cursor),
                    limit: Some(1),
                },
            ),
        )
        .await;
    let Err(failure) = result else {
        bail!("changed anchor should invalidate cursor");
    };
    if failure.code != L2ReadErrorCode::CursorInvalidated {
        bail!("unexpected cursor failure: {failure:?}");
    }
    Ok(())
}

#[tokio::test]
async fn transactions_fallback_only_after_confirmed_not_found() -> Result<()> {
    let (facts, config) = facts(true, true);
    let transaction = transaction('a');
    let adapter = Arc::new(ScriptedAdapter::default());
    adapter.edit(|state| {
        state
            .transactions
            .entry(indexer_id())
            .or_default()
            .push_back(Ok(None));
        state
            .transactions
            .entry(sequencer_id())
            .or_default()
            .push_back(Ok(Some(transaction.clone())));
    })?;
    let report = scripted_router(adapter.clone())
        .transaction(
            &facts,
            request(
                &config,
                ZoneL2TransactionQuery {
                    transaction_id: transaction.hash,
                    exact_source_id: None,
                },
            ),
        )
        .await?;
    if report.route.policy != L2RoutePolicy::ConfirmedNotFoundFallback
        || report.route.attempts.len() != 2
        || report.route.attempts.first().map(|attempt| attempt.outcome)
            != Some(L2RouteAttemptOutcome::NotFound)
    {
        bail!("transaction fallback route is wrong: {:?}", report.route);
    }
    let calls = adapter.calls()?;
    if !matches!(calls.as_slice(), [first, second] if first.starts_with("tx:src_b") && second.starts_with("tx:src_a"))
    {
        bail!("transaction source order is wrong: {calls:?}");
    }
    Ok(())
}

#[tokio::test]
async fn transaction_error_never_falls_back() -> Result<()> {
    let (facts, config) = facts(true, true);
    let transaction = transaction('a');
    let adapter = Arc::new(ScriptedAdapter::default());
    adapter.edit(|state| {
        state
            .transactions
            .entry(indexer_id())
            .or_default()
            .push_back(Err(L2SourceError::unavailable()));
        state
            .transactions
            .entry(sequencer_id())
            .or_default()
            .push_back(Ok(Some(transaction.clone())));
    })?;
    let result = scripted_router(adapter.clone())
        .transaction(
            &facts,
            request(
                &config,
                ZoneL2TransactionQuery {
                    transaction_id: transaction.hash,
                    exact_source_id: None,
                },
            ),
        )
        .await;
    let Err(failure) = result else {
        bail!("Indexer error should terminate transaction route");
    };
    if failure.code != L2ReadErrorCode::SourceUnavailable
        || adapter
            .calls()?
            .iter()
            .any(|call| call.contains(&sequencer_id()))
    {
        bail!("transaction error caused fallback: {failure:?}");
    }
    Ok(())
}

#[tokio::test]
async fn exact_transaction_uses_verified_memory_cache() -> Result<()> {
    let (facts, config) = facts(false, true);
    let transaction = transaction('a');
    let adapter = Arc::new(ScriptedAdapter::default());
    adapter.edit(|state| {
        state
            .transactions
            .entry(sequencer_id())
            .or_default()
            .push_back(Ok(Some(transaction.clone())));
    })?;
    let router = scripted_router(adapter.clone());
    for expected in [L2Retrieval::Live, L2Retrieval::MemoryCache] {
        let report = router
            .transaction(
                &facts,
                request(
                    &config,
                    ZoneL2TransactionQuery {
                        transaction_id: transaction.hash.clone(),
                        exact_source_id: Some(sequencer_id()),
                    },
                ),
            )
            .await?;
        if report
            .route
            .attempts
            .first()
            .map(|attempt| attempt.retrieval)
            != Some(expected)
        {
            bail!("unexpected transaction retrieval evidence: {report:?}");
        }
    }
    let trace = router
        .transaction_trace(
            &facts,
            request(
                &config,
                ZoneL2TransactionTraceQuery {
                    transaction_id: transaction.hash.clone(),
                    exact_source_id: Some(sequencer_id()),
                    idl_program_id: None,
                },
            ),
        )
        .await?;
    let L2ReadOutcome::Found { value: trace } = trace.data else {
        bail!("cached transaction trace was not derived");
    };
    if trace.trace.hash != transaction.hash {
        bail!("transaction trace lost fetched payload identity");
    }
    if adapter
        .calls()?
        .iter()
        .filter(|call| call.starts_with("tx:"))
        .count()
        != 1
    {
        bail!("transaction cache did not suppress second source call");
    }
    Ok(())
}

#[tokio::test]
async fn conflicting_numeric_block_is_ambiguous() -> Result<()> {
    let (facts, config) = facts(true, true);
    let adapter = Arc::new(ScriptedAdapter::default());
    adapter.edit(|state| {
        state
            .heads
            .entry(indexer_id())
            .or_default()
            .push_back(Ok(Some(block(10, 'a'))));
        state
            .blocks_by_id
            .entry((indexer_id(), 5))
            .or_default()
            .push_back(Ok(Some(block(5, '5'))));
        state
            .blocks_by_id
            .entry((sequencer_id(), 5))
            .or_default()
            .push_back(Ok(Some(block(5, '6'))));
    })?;
    let report = scripted_router(adapter)
        .block_detail(
            &facts,
            request(
                &config,
                ZoneL2BlockDetailQuery {
                    target: ZoneL2BlockTarget::Id { block_id: 5 },
                    exact_source_id: None,
                },
            ),
        )
        .await?;
    let L2ReadOutcome::Ambiguous { candidates } = report.data else {
        bail!("conflicting block did not return ambiguity");
    };
    if candidates.len() != 2 {
        bail!("numeric block ambiguity lost a source");
    }
    Ok(())
}

#[tokio::test]
async fn provisional_account_retries_moving_head_once() -> Result<()> {
    let (facts, config) = facts(false, true);
    let account_id = identity('4');
    let canonical_account = crate::parse_account_id(&account_id)?.to_string();
    let adapter = Arc::new(ScriptedAdapter::default());
    adapter.edit(|state| {
        state.heads.entry(sequencer_id()).or_default().extend([
            Ok(Some(block(10, 'a'))),
            Ok(Some(block(11, 'b'))),
            Ok(Some(block(11, 'b'))),
            Ok(Some(block(11, 'b'))),
        ]);
        state
            .current_accounts
            .entry(sequencer_id())
            .or_default()
            .extend([
                Ok(account(&canonical_account, "1")),
                Ok(account(&canonical_account, "2")),
            ]);
    })?;
    let report = scripted_router(adapter)
        .account(
            &facts,
            request(
                &config,
                ZoneL2AccountQuery {
                    account_id,
                    snapshot: ZoneL2AccountSnapshot::Provisional,
                    exact_source_id: None,
                },
            ),
        )
        .await?;
    let L2ReadOutcome::Found { value } = report.data else {
        bail!("provisional account was not found");
    };
    if value.account.nonce != "2"
        || value.anchor_state != L2AccountAnchorState::Exact
        || value.anchor.as_ref().map(|anchor| anchor.block_id) != Some(11)
    {
        bail!("provisional account bracket is wrong: {value:?}");
    }
    Ok(())
}

#[tokio::test]
async fn historical_account_cache_keeps_exact_anchor() -> Result<()> {
    let (facts, config) = facts(true, false);
    let account_id = identity('4');
    let canonical_account = crate::parse_account_id(&account_id)?.to_string();
    let anchor = block(7, '7');
    let adapter = Arc::new(ScriptedAdapter::default());
    adapter.edit(|state| {
        state
            .blocks_by_id
            .entry((indexer_id(), 7))
            .or_default()
            .push_back(Ok(Some(anchor.clone())));
        state
            .accounts_at_block
            .entry((indexer_id(), 7))
            .or_default()
            .push_back(Ok(account(&canonical_account, "9")));
    })?;
    let router = scripted_router(adapter);
    for expected in [L2Retrieval::Live, L2Retrieval::MemoryCache] {
        let report = router
            .account(
                &facts,
                request(
                    &config,
                    ZoneL2AccountQuery {
                        account_id: account_id.clone(),
                        snapshot: ZoneL2AccountSnapshot::Historical {
                            block_id: 7,
                            block_hash: identity('7'),
                        },
                        exact_source_id: None,
                    },
                ),
            )
            .await?;
        if report
            .route
            .attempts
            .first()
            .map(|attempt| attempt.retrieval)
            != Some(expected)
        {
            bail!("unexpected historical account retrieval: {report:?}");
        }
    }
    Ok(())
}

#[tokio::test]
async fn activity_and_transfer_pages_use_independent_opaque_cursors() -> Result<()> {
    let (facts, config) = facts(true, false);
    let account_id = identity('4');
    let adapter = Arc::new(ScriptedAdapter::default());
    adapter.edit(|state| {
        state
            .activities
            .entry(indexer_id())
            .or_default()
            .push_back(Ok(vec![activity(0, '1'), activity(1, '2')]));
        state
            .heads
            .entry(indexer_id())
            .or_default()
            .push_back(Ok(Some(block(4, '4'))));
        state
            .transfer_blocks
            .entry(indexer_id())
            .or_default()
            .push_back(Ok(vec![indexer_block(4, '4'), indexer_block(3, '3')]));
    })?;
    let router = scripted_router(adapter);
    let activity = router
        .account_activity(
            &facts,
            request(
                &config,
                ZoneL2AccountActivityQuery {
                    account_id,
                    cursor: None,
                    limit: Some(1),
                    order: ZoneL2AccountActivityOrder::OldestFirst,
                },
            ),
        )
        .await?;
    let transfers = router
        .transfers(
            &facts,
            request(
                &config,
                ZoneL2TransfersQuery {
                    cursor: None,
                    block_limit: Some(1),
                },
            ),
        )
        .await?;
    let L2ReadOutcome::Found { value: activity } = activity.data else {
        bail!("activity page was not found");
    };
    let L2ReadOutcome::Found { value: transfers } = transfers.data else {
        bail!("transfer page was not found");
    };
    if !activity.has_more
        || activity.next_cursor.is_none()
        || !transfers.has_more
        || transfers.next_cursor.is_none()
        || transfers.scanned_blocks != 1
        || transfers.newest_block != Some(4)
    {
        bail!("paged read metadata is wrong: {activity:?}, {transfers:?}");
    }
    Ok(())
}

#[tokio::test]
async fn selected_sequencer_serves_programs_proofs_and_nonces() -> Result<()> {
    let (facts, config) = facts(false, true);
    let account_id = identity('4');
    let adapter = Arc::new(ScriptedAdapter::default());
    adapter.edit(|state| {
        state
            .programs
            .entry(sequencer_id())
            .or_default()
            .push_back(Ok(Vec::new()));
        state
            .proofs
            .entry(sequencer_id())
            .or_default()
            .push_back(Ok(Some((3, vec![identity('5')]))));
        state
            .nonces
            .entry(sequencer_id())
            .or_default()
            .push_back(Ok(vec!["8".to_owned()]));
    })?;
    let router = scripted_router(adapter);
    let programs = router
        .programs(
            &facts,
            request(
                &config,
                ZoneL2ProgramsQuery {
                    exact_source_id: None,
                },
            ),
        )
        .await?;
    let proof = router
        .commitment_proof(
            &facts,
            request(
                &config,
                ZoneL2CommitmentProofQuery {
                    commitment_hex: identity('6'),
                    exact_source_id: None,
                },
            ),
        )
        .await?;
    let nonces = router
        .account_nonces(
            &facts,
            request(
                &config,
                ZoneL2AccountNoncesQuery {
                    account_ids: vec![account_id],
                    exact_source_id: None,
                },
            ),
        )
        .await?;
    if programs.route.policy != L2RoutePolicy::SelectedSequencer
        || !matches!(proof.data, L2ReadOutcome::Found { .. })
        || !matches!(nonces.data, L2ReadOutcome::Found { .. })
    {
        bail!("selected Sequencer capabilities were routed incorrectly");
    }
    Ok(())
}

#[tokio::test]
async fn context_gates_unverified_stale_and_data_channel_reads() -> Result<()> {
    let (mut facts, config) = facts(false, true);
    let router = scripted_router(Arc::new(ScriptedAdapter::default()));
    let query = || ZoneL2ProgramsQuery {
        exact_source_id: None,
    };

    facts.verification = CatalogVerificationState::CachedUnverified;
    let result = router.programs(&facts, request(&config, query())).await;
    let Err(failure) = result else {
        bail!("unverified Zone read unexpectedly succeeded");
    };
    if failure.code != L2ReadErrorCode::ZoneUnverified {
        bail!("unexpected unverified Zone failure: {failure:?}");
    }

    facts.verification = CatalogVerificationState::Verified;
    let mut stale = request(&config, query());
    stale.context.source_config_revision = 99;
    let result = router.programs(&facts, stale).await;
    let Err(failure) = result else {
        bail!("stale Zone read unexpectedly succeeded");
    };
    if failure.code != L2ReadErrorCode::StaleContext {
        bail!("unexpected stale Zone failure: {failure:?}");
    }

    let summary = facts
        .summaries
        .get_mut(&config.channel_id)
        .context("test Zone summary is missing")?;
    summary.facts = ZoneFacts::DataChannel {
        raw_activity: RawActivitySummary {
            inscription_count: 1,
            latest_slot: Some(7),
            latest_payload_size: None,
            finality_state: L1FinalityState::Final,
        },
    };
    let result = router.programs(&facts, request(&config, query())).await;
    let Err(failure) = result else {
        bail!("Data Channel L2 read unexpectedly succeeded");
    };
    if failure.code != L2ReadErrorCode::L2NotApplicable {
        bail!("unexpected Data Channel failure: {failure:?}");
    }
    Ok(())
}

#[tokio::test]
async fn exact_source_cannot_cross_channels() -> Result<()> {
    let (mut facts, config) = facts(false, true);
    let mut foreign = source_config(&config.network_scope, &identity('d'), false, true);
    foreign.selected_sequencer_source_id = Some("src_f".to_owned());
    if let Some(source) = foreign.sequencer_sources.first_mut() {
        source.source_id = "src_f".to_owned();
    }
    let foreign_source = foreign
        .selected_sequencer_source_id
        .clone()
        .context("foreign source is missing")?;
    facts.configs.push(foreign);
    let result = scripted_router(Arc::new(ScriptedAdapter::default()))
        .programs(
            &facts,
            request(
                &config,
                ZoneL2ProgramsQuery {
                    exact_source_id: Some(foreign_source),
                },
            ),
        )
        .await;
    let Err(failure) = result else {
        bail!("foreign Channel source was accepted");
    };
    if failure.code != L2ReadErrorCode::SourceIneligible {
        bail!("unexpected foreign source failure: {failure:?}");
    }
    Ok(())
}

#[tokio::test]
async fn transaction_not_found_is_not_cached() -> Result<()> {
    let (facts, config) = facts(false, true);
    let transaction = transaction('a');
    let adapter = Arc::new(ScriptedAdapter::default());
    adapter.edit(|state| {
        state
            .transactions
            .entry(sequencer_id())
            .or_default()
            .extend([Ok(None), Ok(Some(transaction.clone()))]);
    })?;
    let router = scripted_router(adapter.clone());
    let first = router
        .transaction(
            &facts,
            request(
                &config,
                ZoneL2TransactionQuery {
                    transaction_id: transaction.hash.clone(),
                    exact_source_id: Some(sequencer_id()),
                },
            ),
        )
        .await?;
    let second = router
        .transaction(
            &facts,
            request(
                &config,
                ZoneL2TransactionQuery {
                    transaction_id: transaction.hash,
                    exact_source_id: Some(sequencer_id()),
                },
            ),
        )
        .await?;
    if !matches!(first.data, L2ReadOutcome::NotFound)
        || !matches!(second.data, L2ReadOutcome::Found { .. })
        || adapter
            .calls()?
            .iter()
            .filter(|call| call.starts_with("tx:"))
            .count()
            != 2
    {
        bail!("transaction absence was cached");
    }
    Ok(())
}

#[tokio::test]
async fn source_config_revision_invalidates_cached_transaction() -> Result<()> {
    let (mut facts, config) = facts(false, true);
    let transaction = transaction('a');
    let adapter = Arc::new(ScriptedAdapter::default());
    adapter.edit(|state| {
        state
            .transactions
            .entry(sequencer_id())
            .or_default()
            .push_back(Ok(Some(transaction.clone())));
    })?;
    let router = scripted_router(adapter);
    let first = router
        .transaction(
            &facts,
            request(
                &config,
                ZoneL2TransactionQuery {
                    transaction_id: transaction.hash.clone(),
                    exact_source_id: Some(sequencer_id()),
                },
            ),
        )
        .await?;
    if !matches!(first.data, L2ReadOutcome::Found { .. }) {
        bail!("transaction cache was not primed");
    }

    let mut revised = config;
    revised.config_revision = 2;
    facts.configs = vec![revised.clone()];
    let result = router
        .transaction(
            &facts,
            request(
                &revised,
                ZoneL2TransactionQuery {
                    transaction_id: transaction.hash,
                    exact_source_id: Some(sequencer_id()),
                },
            ),
        )
        .await;
    let Err(failure) = result else {
        bail!("old config revision cache entry was reused");
    };
    if failure.code != L2ReadErrorCode::SourceCapabilityUnavailable {
        bail!("unexpected revised-source failure: {failure:?}");
    }
    Ok(())
}

#[tokio::test]
async fn hash_only_fallback_reports_sequencer_capability_limit() -> Result<()> {
    let (facts, config) = facts(true, true);
    let adapter = Arc::new(ScriptedAdapter::default());
    adapter.edit(|state| {
        state
            .blocks_by_hash
            .entry(indexer_id())
            .or_default()
            .push_back(Ok(None));
    })?;
    let result = scripted_router(adapter)
        .block_detail(
            &facts,
            request(
                &config,
                ZoneL2BlockDetailQuery {
                    target: ZoneL2BlockTarget::Hash {
                        block_hash: identity('a'),
                    },
                    exact_source_id: None,
                },
            ),
        )
        .await;
    let Err(failure) = result else {
        bail!("hash-only Sequencer fallback unexpectedly succeeded");
    };
    if failure.code != L2ReadErrorCode::SourceCapabilityUnavailable {
        bail!("unexpected hash-only fallback failure: {failure:?}");
    }
    let attempts = failure
        .route
        .as_ref()
        .map(|route| route.attempts.len())
        .unwrap_or_default();
    if attempts != 2 {
        bail!("unexpected hash-only route evidence: {failure:?}");
    }
    Ok(())
}

#[tokio::test]
async fn advertised_head_with_empty_page_is_protocol_failure() -> Result<()> {
    let (facts, config) = facts(false, true);
    let adapter = Arc::new(ScriptedAdapter::default());
    adapter.edit(|state| {
        state
            .heads
            .entry(sequencer_id())
            .or_default()
            .push_back(Ok(Some(block(3, '3'))));
        state
            .block_pages
            .entry(sequencer_id())
            .or_default()
            .push_back(Ok(Vec::new()));
    })?;
    let result = scripted_router(adapter)
        .blocks(
            &facts,
            request(
                &config,
                ZoneL2BlocksQuery {
                    cursor: None,
                    limit: Some(2),
                },
            ),
        )
        .await;
    let Err(failure) = result else {
        bail!("empty range below advertised head was accepted");
    };
    if failure.code != L2ReadErrorCode::SourceProtocolError {
        bail!("unexpected missing-range failure: {failure:?}");
    }
    Ok(())
}

#[tokio::test]
async fn target_resolution_keeps_cross_layer_and_exact_source_candidates() -> Result<()> {
    let adapter = Arc::new(ScriptedAdapter::default());
    let router = scripted_router(adapter.clone());
    let (facts, config) = facts(true, true);
    adapter.edit(|state| {
        state
            .heads
            .entry(indexer_id())
            .or_default()
            .push_back(Ok(Some(block(42, 'f'))));
        state
            .blocks_by_id
            .entry((indexer_id(), 42))
            .or_default()
            .push_back(Ok(Some(block(42, 'a'))));
        state
            .blocks_by_id
            .entry((sequencer_id(), 42))
            .or_default()
            .push_back(Ok(Some(block(42, 'b'))));
    })?;

    let report = router
        .resolve_target(
            &facts,
            InspectionResolveTargetRequest {
                query: "42".to_owned(),
                active_zone_context: Some(request(&config, ()).context),
                request_revision: 9,
            },
        )
        .await;

    if report.status != InspectionTargetResolutionStatus::Ambiguous
        || report.request_revision != 9
        || report.candidates.len() != 3
    {
        bail!("unexpected cross-layer resolution report: {report:?}");
    }
    let l2_candidates = report
        .candidates
        .iter()
        .filter_map(|candidate| match &candidate.entity_ref {
            InspectionEntityRef::L2 { entity } => Some(entity),
            InspectionEntityRef::Zone { .. } | InspectionEntityRef::L1 { .. } => None,
        })
        .collect::<Vec<_>>();
    if l2_candidates.len() != 2
        || l2_candidates.iter().any(|entity| {
            !matches!(entity.source, ZoneL2SourceQualifier::Exact { .. })
                || !entity.canonical_key.starts_with("block:42:")
        })
    {
        bail!("conflicting blocks lost exact source identity: {l2_candidates:?}");
    }
    Ok(())
}

#[tokio::test]
async fn target_resolution_without_zone_omits_implicit_l2_and_recovers_explicit_l2() -> Result<()> {
    let adapter = Arc::new(ScriptedAdapter::default());
    let router = scripted_router(adapter.clone());
    let (facts, _) = facts(true, true);

    let numeric = router
        .resolve_target(
            &facts,
            InspectionResolveTargetRequest {
                query: "42".to_owned(),
                active_zone_context: None,
                request_revision: 1,
            },
        )
        .await;
    let explicit = router
        .resolve_target(
            &facts,
            InspectionResolveTargetRequest {
                query: "l2:42".to_owned(),
                active_zone_context: None,
                request_revision: 2,
            },
        )
        .await;

    if numeric.status != InspectionTargetResolutionStatus::Resolved
        || numeric.candidates.len() != 1
        || explicit.status != InspectionTargetResolutionStatus::Recovery
        || explicit.recovery != Some(L2RecoveryAction::RefreshContext)
        || !adapter.calls()?.is_empty()
    {
        bail!("unexpected no-Zone resolution: {numeric:?}, {explicit:?}");
    }
    Ok(())
}

#[tokio::test]
async fn explicit_account_resolution_never_probes_default_account_state() -> Result<()> {
    let adapter = Arc::new(ScriptedAdapter::default());
    let router = scripted_router(adapter.clone());
    let (facts, config) = facts(true, true);
    let account_id = crate::parse_account_id(&identity('4'))?.to_string();

    let report = router
        .resolve_target(
            &facts,
            InspectionResolveTargetRequest {
                query: format!("account:Public/{account_id}"),
                active_zone_context: Some(request(&config, ()).context),
                request_revision: 3,
            },
        )
        .await;

    if report.status != InspectionTargetResolutionStatus::Resolved
        || report.candidates.len() != 1
        || !adapter.calls()?.is_empty()
    {
        bail!("explicit account resolution performed a state probe: {report:?}");
    }
    let Some(InspectionTargetCandidate {
        entity_ref: InspectionEntityRef::L2 { entity },
        ..
    }) = report.candidates.first()
    else {
        bail!("missing account candidate");
    };
    if entity.entity_kind != ZoneL2EntityKind::Account
        || entity.canonical_key != account_id
        || entity.source != ZoneL2SourceQualifier::Policy
    {
        bail!("account candidate was not policy-qualified: {entity:?}");
    }
    Ok(())
}

fn facts(with_indexer: bool, with_sequencer: bool) -> (ZoneL2RuntimeFacts, ChannelSourceConfig) {
    let scope = NetworkScope::GenesisId {
        genesis_id: identity('1'),
    };
    let channel_id = identity('c');
    let config = source_config(&scope, &channel_id, with_indexer, with_sequencer);
    let classification_config = if with_sequencer {
        config.clone()
    } else {
        source_config(&scope, &channel_id, with_indexer, true)
    };
    let observations = ChannelSourceMonitorSnapshot::default();
    let catalog = snapshot(scope.clone());
    let summaries = project_catalog_zones_with_sources(
        &catalog,
        std::slice::from_ref(&classification_config),
        &observations,
        CatalogVerificationState::Verified,
    )
    .into_iter()
    .map(|summary| (summary.channel_id.clone(), summary))
    .collect();
    (
        ZoneL2RuntimeFacts {
            network_scope: Some(scope),
            verification: CatalogVerificationState::Verified,
            summaries,
            configs: vec![config.clone()],
            observations,
        },
        config,
    )
}

fn source_config(
    scope: &NetworkScope,
    channel_id: &str,
    with_indexer: bool,
    with_sequencer: bool,
) -> ChannelSourceConfig {
    let sequencer_target = ChannelSourceTarget::Rpc {
        endpoint: "https://sequencer.example/".to_owned(),
    };
    let sequencer = with_sequencer.then(|| ConfiguredSequencerSource {
        source_id: sequencer_id(),
        label: None,
        channel_attestation: PersistedSequencerAttestation::PersistedAttested {
            channel_id: channel_id.to_owned(),
            target_fingerprint: sequencer_target.fingerprint(),
            attested_at_unix: 1,
        },
        target: sequencer_target,
    });
    ChannelSourceConfig {
        network_scope: scope.clone(),
        channel_id: channel_id.to_owned(),
        config_revision: 1,
        selected_sequencer_source_id: sequencer.as_ref().map(|source| source.source_id.clone()),
        sequencer_sources: sequencer.into_iter().collect(),
        indexer_source: with_indexer.then(|| ConfiguredIndexerSource {
            source_id: indexer_id(),
            label: None,
            target: ChannelSourceTarget::Rpc {
                endpoint: "https://indexer.example/".to_owned(),
            },
        }),
    }
}

fn request<T>(config: &ChannelSourceConfig, query: T) -> ZoneL2Request<T> {
    ZoneL2Request {
        context: ActiveZoneContext {
            network_scope: config.network_scope.clone(),
            channel_id: config.channel_id.clone(),
            zone_kind: ZoneKind::SequencerZone,
            selected_sequencer_source_id: config.selected_sequencer_source_id.clone(),
            indexer_source_id: config
                .indexer_source
                .as_ref()
                .map(|source| source.source_id.clone()),
            source_config_revision: config.config_revision,
            context_revision: 1,
        },
        request_revision: 1,
        query,
    }
}

fn snapshot(scope: NetworkScope) -> CatalogSnapshot {
    CatalogSnapshot {
        metadata: CatalogMetadata {
            catalog_file_id: "catalog_l2_router_test".to_owned(),
            network_scope: scope,
            identity_aliases: Vec::new(),
            identity_assurance: CatalogIdentityAssurance::SourceAttested,
            identity_transition: None,
            catalog_revision: 1,
            created_at_unix: 1,
            updated_at_unix: 1,
        },
        frontier: Some(CatalogFrontier {
            scanned_through_slot: None,
            checkpoint: None,
            observed_lib: None,
            coverage_floor: None,
            prefix_status: CoveragePrefixStatus::Unknown,
            coverage_status: CatalogCoverageStatus::Rebuilding,
        }),
        traversal: None,
        zones: Vec::new(),
        evidence: Vec::new(),
        segments: Vec::new(),
        gaps: Vec::new(),
    }
}

fn block(block_id: u64, hash: char) -> NormalizedL2Block {
    NormalizedL2Block {
        summary: L2BlockSummary {
            block_id,
            block_hash: identity(hash),
            parent_hash: identity('0'),
            timestamp: block_id,
            bedrock_status: "Pending".to_owned(),
            transaction_count: 0,
        },
        transactions: Vec::new(),
    }
}

fn indexer_block(block_id: u64, hash: char) -> IndexerBlockReport {
    IndexerBlockReport {
        block_id: Some(block_id),
        header_hash: Some(identity(hash)),
        parent_hash: Some(identity('0')),
        timestamp: Some(block_id),
        bedrock_status: Some("Finalized".to_owned()),
        tx_count: 0,
        transactions: Vec::new(),
        raw: json!({}),
    }
}

fn transaction(hash: char) -> TransactionSummary {
    TransactionSummary {
        hash: identity(hash),
        kind: "Public".to_owned(),
        program_id_hex: None,
        account_ids: Vec::new(),
        nonces: Vec::new(),
        instruction_data: Vec::new(),
        bytecode_len: None,
        raw_signature_valid: None,
        message_prehash: None,
        prehash_signature_valid: None,
    }
}

fn account(account_id: &str, nonce: &str) -> L2AccountValue {
    L2AccountValue {
        account_id: account_id.to_owned(),
        account_id_base58: account_id.to_owned(),
        account_id_hex: identity('4'),
        balance: "1".to_owned(),
        nonce: nonce.to_owned(),
        owner_program_base58: "owner".to_owned(),
        owner_program_hex: identity('5'),
        data_hex: String::new(),
        existence: L2AccountExistence::Unknown,
    }
}

fn activity(index: usize, hash: char) -> L2AccountActivityRow {
    L2AccountActivityRow {
        index,
        transaction_id: identity(hash),
        kind: "Public".to_owned(),
        direction: None,
        program_id_hex: None,
        account_ids: Vec::new(),
        signer_account_ids: Vec::new(),
        nonces: Vec::new(),
        instruction_data: Vec::new(),
        transfer_outputs: Vec::new(),
        bytecode_len: None,
    }
}

fn identity(character: char) -> String {
    character.to_string().repeat(64)
}

fn sequencer_id() -> String {
    "src_a".to_owned()
}

fn indexer_id() -> String {
    "src_b".to_owned()
}
