use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{Arc, Mutex, MutexGuard},
};

use crate::{
    inspection::{CatalogVerificationState, ZoneKind, ZoneSourceRole},
    lez::{IndexerBlockReport, TransactionSummary},
    source_routing::channel_sources::{
        ChannelSourceBindingState, ChannelSourceConfig, ChannelSourceHealthState,
        ChannelSourceRole, PersistedSequencerAttestation,
    },
};

use super::{
    ActiveZoneContext, DirectZoneL2SourceAdapter, L2_READ_SCHEMA_VERSION, L2AccountActivityPage,
    L2AccountAnchorState, L2AccountNonce, L2AccountNoncesData, L2AccountSnapshotData,
    L2BlockAnchor, L2BlockDetail, L2BlockRow, L2BlocksPage, L2CacheScope, L2CommitmentProofData,
    L2EvidenceCache, L2ExactSourceCandidate, L2ProgramsData, L2ReadErrorCode, L2ReadErrorDetails,
    L2ReadOutcome, L2ReadReport, L2ReadRoute, L2ReadWarning, L2RecoveryAction, L2Retrieval,
    L2RouteAttempt, L2RouteAttemptOutcome, L2RouteCompleteness, L2RouteContribution,
    L2RouteFinality, L2RoutePolicy, L2SourceDescriptor, L2SourceError, L2SourceErrorKind,
    L2SourceHead, L2SourceObservation, L2TransactionDetail, L2TransactionTrace, L2TransfersPage,
    NormalizedL2Block, ZoneL2AccountActivityOrder, ZoneL2AccountActivityQuery,
    ZoneL2AccountNoncesQuery, ZoneL2AccountQuery, ZoneL2AccountSnapshot, ZoneL2BlockDetailQuery,
    ZoneL2BlockTarget, ZoneL2BlocksQuery, ZoneL2CommitmentProofQuery, ZoneL2ProgramsQuery,
    ZoneL2Request, ZoneL2RuntimeFacts, ZoneL2SourceAdapter, ZoneL2TransactionQuery,
    ZoneL2TransactionTraceQuery, ZoneL2TransfersQuery,
};

const DEFAULT_PAGE_LIMIT: usize = 25;
const MAX_PAGE_LIMIT: usize = 50;
const MAX_CURSOR_ENTRIES: usize = 128;
const MAX_NONCE_ACCOUNTS: usize = 100;

pub(crate) struct ZoneL2Router {
    adapter: Arc<dyn ZoneL2SourceAdapter>,
    state: Mutex<L2RouterState>,
}

impl Default for ZoneL2Router {
    fn default() -> Self {
        Self::new(Arc::new(DirectZoneL2SourceAdapter))
    }
}

impl ZoneL2Router {
    #[must_use]
    pub(crate) fn new(adapter: Arc<dyn ZoneL2SourceAdapter>) -> Self {
        Self {
            adapter,
            state: Mutex::new(L2RouterState::default()),
        }
    }

    pub(crate) async fn blocks(
        &self,
        facts: &ZoneL2RuntimeFacts,
        request: ZoneL2Request<ZoneL2BlocksQuery>,
    ) -> Result<L2ReadReport<L2BlocksPage>, L2ReadFailure> {
        let config = self.validate_and_reconcile(facts, &request)?;
        let limit = page_limit(request.query.limit)?;
        match request.query.cursor.as_deref() {
            Some(cursor) => {
                let state = self.block_cursor(cursor)?;
                validate_cursor_context(&request.context, &state.context)?;
                self.continue_blocks(facts, request, config, state, limit)
                    .await
            }
            None => self.initial_blocks(facts, request, config, limit).await,
        }
    }

    async fn initial_blocks(
        &self,
        facts: &ZoneL2RuntimeFacts,
        request: ZoneL2Request<ZoneL2BlocksQuery>,
        config: ChannelSourceConfig,
        limit: usize,
    ) -> Result<L2ReadReport<L2BlocksPage>, L2ReadFailure> {
        let indexer = optional_role_plan(facts, &config, ZoneSourceRole::Indexer);
        let sequencer = optional_role_plan(facts, &config, ZoneSourceRole::Sequencer);
        if matches!(indexer, ContributorPlan::Absent)
            && matches!(sequencer, ContributorPlan::Absent)
        {
            return Err(L2ReadFailure::new(
                L2ReadErrorCode::SourceUnconfigured,
                "Active Zone has no source configured for this read",
            ));
        }

        let fetch_limit = limit.saturating_add(1);
        let (indexer_result, sequencer_result) = tokio::join!(
            fetch_initial_contributor(self.adapter.as_ref(), indexer, fetch_limit),
            fetch_initial_contributor(self.adapter.as_ref(), sequencer, fetch_limit),
        );
        let results = [indexer_result, sequencer_result]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let route = route_from_contributors(&results);
        let successful = results
            .iter()
            .filter(|result| result.failure.is_none())
            .cloned()
            .collect::<Vec<_>>();
        if successful.is_empty() {
            return Err(composite_failure(&results, route));
        }

        let configured_count = results.len();
        let failed_count = results.len().saturating_sub(successful.len());
        let completeness = if failed_count > 0 {
            L2RouteCompleteness::Degraded
        } else if configured_count == 1 {
            L2RouteCompleteness::SingleConfigured
        } else {
            L2RouteCompleteness::AllConfigured
        };
        let (mut page, pinned) = compose_block_page(&successful, limit);
        let mut report = L2ReadReport::new(
            "lez.blocks",
            request.clone(),
            route,
            completeness,
            L2ReadOutcome::Found {
                value: page.clone(),
            },
        );
        report.warnings = results
            .iter()
            .filter_map(ContributorResult::warning)
            .collect();
        if page.has_more {
            let cursor = self.insert_block_cursor(BlockCursorState {
                context: request.context,
                contributors: pinned,
                source_heads: page.source_heads.clone(),
                next_before: page
                    .rows
                    .iter()
                    .map(|row| row.summary.block_id)
                    .min()
                    .unwrap_or_default(),
            })?;
            page.next_cursor = Some(cursor);
            report.data = L2ReadOutcome::Found { value: page };
        }
        Ok(report)
    }

    async fn continue_blocks(
        &self,
        facts: &ZoneL2RuntimeFacts,
        request: ZoneL2Request<ZoneL2BlocksQuery>,
        config: ChannelSourceConfig,
        state: BlockCursorState,
        limit: usize,
    ) -> Result<L2ReadReport<L2BlocksPage>, L2ReadFailure> {
        let mut indexer = None;
        let mut sequencer = None;
        for pinned in &state.contributors {
            let descriptor = eligible_descriptor(
                facts,
                &config,
                &pinned.descriptor.source_id,
                Some(pinned.descriptor.role),
            )
            .map_err(|_| cursor_invalidated())?;
            if descriptor != pinned.descriptor {
                return Err(cursor_invalidated());
            }
            match pinned.descriptor.role {
                ZoneSourceRole::Indexer => indexer = Some(pinned.clone()),
                ZoneSourceRole::Sequencer => sequencer = Some(pinned.clone()),
            }
        }
        let fetch_limit = limit.saturating_add(1);
        let (indexer_result, sequencer_result) = tokio::join!(
            fetch_continuation_contributor(
                self.adapter.as_ref(),
                indexer,
                &state.source_heads,
                state.next_before,
                fetch_limit,
            ),
            fetch_continuation_contributor(
                self.adapter.as_ref(),
                sequencer,
                &state.source_heads,
                state.next_before,
                fetch_limit,
            ),
        );
        let results = [indexer_result, sequencer_result]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let route = route_from_contributors(&results);
        if let Some(failure) = results.iter().find_map(|result| result.failure.clone()) {
            return Err(failure.with_route(route));
        }
        let (mut page, pinned) = compose_block_page(&results, limit);
        page.source_heads = state.source_heads;
        let mut report = L2ReadReport::new(
            "lez.blocks",
            request.clone(),
            route,
            if results.len() == 1 {
                L2RouteCompleteness::SingleConfigured
            } else {
                L2RouteCompleteness::AllConfigured
            },
            L2ReadOutcome::Found {
                value: page.clone(),
            },
        );
        if page.has_more {
            let Some(next_before) = page.rows.iter().map(|row| row.summary.block_id).min() else {
                return Err(L2ReadFailure::new(
                    L2ReadErrorCode::Internal,
                    "L2 block cursor could not advance",
                ));
            };
            page.next_cursor = Some(self.insert_block_cursor(BlockCursorState {
                context: request.context,
                contributors: pinned,
                source_heads: page.source_heads.clone(),
                next_before,
            })?);
            report.data = L2ReadOutcome::Found { value: page };
        }
        Ok(report)
    }

    pub(crate) async fn block_detail(
        &self,
        facts: &ZoneL2RuntimeFacts,
        request: ZoneL2Request<ZoneL2BlockDetailQuery>,
    ) -> Result<L2ReadReport<L2BlockDetail>, L2ReadFailure> {
        let config = self.validate_and_reconcile(facts, &request)?;
        let target = normalized_block_target(&request.query.target)?;
        if let Some(source_id) = request.query.exact_source_id.as_deref() {
            let source = eligible_descriptor(facts, &config, source_id, None)?;
            let read = self
                .read_block_source(
                    &request.context,
                    source,
                    &target,
                    L2RoutePolicy::ExactSource,
                )
                .await?;
            return Ok(block_report(
                request,
                L2RoutePolicy::ExactSource,
                vec![read.attempt],
                read.value
                    .map(|block| block_detail_value(block, &read.descriptor, read.retrieval)),
                L2RouteCompleteness::SingleConfigured,
            ));
        }

        match target {
            ZoneL2BlockTarget::Id { block_id } => {
                self.numeric_block_detail(facts, request, config, block_id)
                    .await
            }
            target @ (ZoneL2BlockTarget::Hash { .. } | ZoneL2BlockTarget::Identity { .. }) => {
                self.policy_block_detail(facts, request, config, target)
                    .await
            }
        }
    }

    async fn policy_block_detail(
        &self,
        facts: &ZoneL2RuntimeFacts,
        request: ZoneL2Request<ZoneL2BlockDetailQuery>,
        config: ChannelSourceConfig,
        target: ZoneL2BlockTarget,
    ) -> Result<L2ReadReport<L2BlockDetail>, L2ReadFailure> {
        let indexer = optional_descriptor(facts, &config, ZoneSourceRole::Indexer)?;
        let sequencer = optional_descriptor(facts, &config, ZoneSourceRole::Sequencer)?;
        let Some(primary) = indexer.or_else(|| sequencer.clone()) else {
            return Err(source_unconfigured());
        };
        let primary_policy = if primary.role == ZoneSourceRole::Indexer {
            L2RoutePolicy::IndexerPrimary
        } else {
            L2RoutePolicy::SelectedSequencer
        };
        let primary_read = self
            .read_block_source(&request.context, primary, &target, primary_policy)
            .await?;
        if primary_read.value.is_some() || primary_read.descriptor.role == ZoneSourceRole::Sequencer
        {
            let value = primary_read.value.map(|block| {
                block_detail_value(block, &primary_read.descriptor, primary_read.retrieval)
            });
            return Ok(block_report(
                request,
                primary_policy,
                vec![primary_read.attempt],
                value,
                L2RouteCompleteness::SingleConfigured,
            ));
        }
        let Some(fallback) = sequencer else {
            return Ok(block_report(
                request,
                L2RoutePolicy::IndexerPrimary,
                vec![primary_read.attempt],
                None,
                L2RouteCompleteness::SingleConfigured,
            ));
        };
        let fallback_read = self
            .read_block_source(
                &request.context,
                fallback,
                &target,
                L2RoutePolicy::ConfirmedNotFoundFallback,
            )
            .await
            .map_err(|failure| append_attempt(failure, primary_read.attempt.clone()))?;
        let value = fallback_read.value.map(|block| {
            block_detail_value(block, &fallback_read.descriptor, fallback_read.retrieval)
        });
        Ok(block_report(
            request,
            L2RoutePolicy::ConfirmedNotFoundFallback,
            vec![primary_read.attempt, fallback_read.attempt],
            value,
            L2RouteCompleteness::AllConfigured,
        ))
    }

    async fn numeric_block_detail(
        &self,
        facts: &ZoneL2RuntimeFacts,
        request: ZoneL2Request<ZoneL2BlockDetailQuery>,
        config: ChannelSourceConfig,
        block_id: u64,
    ) -> Result<L2ReadReport<L2BlockDetail>, L2ReadFailure> {
        let indexer = optional_descriptor(facts, &config, ZoneSourceRole::Indexer)?;
        let sequencer = optional_descriptor(facts, &config, ZoneSourceRole::Sequencer)?;
        let target = ZoneL2BlockTarget::Id { block_id };
        let Some(indexer_source) = indexer else {
            let sequencer_source = sequencer.ok_or_else(source_unconfigured)?;
            let read = self
                .read_block_source(
                    &request.context,
                    sequencer_source,
                    &target,
                    L2RoutePolicy::SelectedSequencer,
                )
                .await?;
            return Ok(block_report(
                request,
                L2RoutePolicy::SelectedSequencer,
                vec![read.attempt],
                read.value
                    .map(|block| block_detail_value(block, &read.descriptor, read.retrieval)),
                L2RouteCompleteness::SingleConfigured,
            ));
        };
        let indexer_head = self
            .adapter
            .head(indexer_source.clone())
            .await
            .map_err(|error| {
                source_failure_with_policy(&indexer_source, error, L2RoutePolicy::IndexerPrimary)
            })?;
        let at_or_below_finalized = indexer_head
            .as_ref()
            .is_some_and(|head| block_id <= head.summary.block_id);
        if !at_or_below_finalized && let Some(sequencer_source) = sequencer {
            let read = self
                .read_block_source(
                    &request.context,
                    sequencer_source,
                    &target,
                    L2RoutePolicy::SelectedSequencer,
                )
                .await?;
            return Ok(block_report(
                request,
                L2RoutePolicy::SelectedSequencer,
                vec![read.attempt],
                read.value
                    .map(|block| block_detail_value(block, &read.descriptor, read.retrieval)),
                L2RouteCompleteness::SingleConfigured,
            ));
        }

        let indexer_future = self.read_block_source(
            &request.context,
            indexer_source,
            &target,
            L2RoutePolicy::IndexerPrimary,
        );
        let sequencer_future = async {
            match sequencer {
                Some(source) if at_or_below_finalized => self
                    .read_block_source(
                        &request.context,
                        source,
                        &target,
                        L2RoutePolicy::IndexerPrimary,
                    )
                    .await
                    .map(Some),
                _ => Ok(None),
            }
        };
        let (indexer_read, sequencer_read) = tokio::join!(indexer_future, sequencer_future);
        let indexer_read = indexer_read?;
        let mut attempts = vec![indexer_read.attempt.clone()];
        let mut warning = None;
        let sequencer_read = match sequencer_read {
            Ok(Some(mut read)) => {
                read.attempt.contribution = L2RouteContribution::None;
                attempts.push(read.attempt.clone());
                Some(read)
            }
            Ok(None) => None,
            Err(failure) => {
                if let Some(route) = failure.route.as_ref()
                    && let Some(attempt) = route.attempts.last()
                {
                    attempts.push(attempt.clone());
                }
                warning = Some(L2ReadWarning {
                    code: failure.code,
                    recovery: failure.code.recovery(),
                    message: failure.message,
                });
                None
            }
        };
        let Some(indexer_block) = indexer_read.value else {
            if at_or_below_finalized {
                return Err(L2ReadFailure::new(
                    L2ReadErrorCode::SourceProtocolError,
                    "Indexer omitted a block inside its finalized range",
                )
                .with_route(L2ReadRoute {
                    policy: L2RoutePolicy::IndexerPrimary,
                    attempts,
                }));
            }
            return Ok(block_report(
                request,
                L2RoutePolicy::IndexerPrimary,
                attempts,
                None,
                route_completeness(sequencer_read.is_some()),
            ));
        };
        if let Some(sequencer_read) = &sequencer_read
            && let Some(sequencer_block) = &sequencer_read.value
            && sequencer_block.summary.block_hash != indexer_block.summary.block_hash
        {
            let candidates = vec![
                exact_block_candidate(&indexer_read.descriptor, &indexer_block),
                exact_block_candidate(&sequencer_read.descriptor, sequencer_block),
            ];
            let mut report = block_outcome_report(
                request,
                L2RoutePolicy::IndexerPrimary,
                attempts,
                L2ReadOutcome::Ambiguous { candidates },
                L2RouteCompleteness::AllConfigured,
            );
            if let Some(warning) = warning {
                report.warnings.push(warning);
            }
            return Ok(report);
        }
        let value = block_detail_value(
            indexer_block,
            &indexer_read.descriptor,
            indexer_read.retrieval,
        );
        let mut report = block_report(
            request,
            L2RoutePolicy::IndexerPrimary,
            attempts,
            Some(value),
            route_completeness(sequencer_read.is_some()),
        );
        if let Some(warning) = warning {
            report.warnings.push(warning);
        }
        Ok(report)
    }

    async fn read_block_source(
        &self,
        context: &ActiveZoneContext,
        descriptor: L2SourceDescriptor,
        target: &ZoneL2BlockTarget,
        policy: L2RoutePolicy,
    ) -> Result<BlockSourceRead, L2ReadFailure> {
        let scope = cache_scope_from_context(context, &descriptor);
        let cached = {
            let mut state = self.lock_state()?;
            match target {
                ZoneL2BlockTarget::Id { block_id } => state.cache.block_by_id(&scope, *block_id),
                ZoneL2BlockTarget::Hash { block_hash } => {
                    state.cache.block_by_hash(&scope, block_hash)
                }
                ZoneL2BlockTarget::Identity {
                    block_id,
                    block_hash,
                } => state
                    .cache
                    .block(&scope, &format!("block:{block_id}:{block_hash}")),
            }
        };
        if let Some(block) = cached {
            return Ok(BlockSourceRead {
                attempt: route_attempt(
                    &descriptor,
                    L2RouteAttemptOutcome::Returned,
                    L2RouteContribution::Payload,
                    L2Retrieval::MemoryCache,
                ),
                descriptor,
                value: Some(block),
                retrieval: L2Retrieval::MemoryCache,
            });
        }
        let result = match target {
            ZoneL2BlockTarget::Id { block_id } | ZoneL2BlockTarget::Identity { block_id, .. } => {
                self.adapter
                    .block_by_id(descriptor.clone(), *block_id)
                    .await
            }
            ZoneL2BlockTarget::Hash { block_hash } => {
                self.adapter
                    .block_by_hash(descriptor.clone(), block_hash.clone())
                    .await
            }
        }
        .map_err(|error| source_failure_with_policy(&descriptor, error, policy))?;
        let value = match (target, result) {
            (_, None) => None,
            (ZoneL2BlockTarget::Id { block_id }, Some(block)) => {
                if block.summary.block_id != *block_id {
                    return Err(source_failure_with_policy(
                        &descriptor,
                        L2SourceError::protocol_error(),
                        policy,
                    ));
                }
                Some(block)
            }
            (ZoneL2BlockTarget::Hash { block_hash }, Some(block)) => {
                if block.summary.block_hash != *block_hash {
                    return Err(source_failure_with_policy(
                        &descriptor,
                        L2SourceError::protocol_error(),
                        policy,
                    ));
                }
                Some(block)
            }
            (
                ZoneL2BlockTarget::Identity {
                    block_id,
                    block_hash,
                },
                Some(block),
            ) => {
                if block.summary.block_id != *block_id {
                    return Err(source_failure_with_policy(
                        &descriptor,
                        L2SourceError::protocol_error(),
                        policy,
                    ));
                }
                (block.summary.block_hash == *block_hash).then_some(block)
            }
        };
        if let Some(block) = &value {
            self.lock_state()?.cache.insert_block(scope, block.clone());
        }
        Ok(BlockSourceRead {
            attempt: route_attempt(
                &descriptor,
                if value.is_some() {
                    L2RouteAttemptOutcome::Returned
                } else {
                    L2RouteAttemptOutcome::NotFound
                },
                if value.is_some() {
                    L2RouteContribution::Payload
                } else {
                    L2RouteContribution::None
                },
                L2Retrieval::Live,
            ),
            descriptor,
            value,
            retrieval: L2Retrieval::Live,
        })
    }

    pub(crate) async fn transaction(
        &self,
        facts: &ZoneL2RuntimeFacts,
        request: ZoneL2Request<ZoneL2TransactionQuery>,
    ) -> Result<L2ReadReport<L2TransactionDetail>, L2ReadFailure> {
        let config = self.validate_and_reconcile(facts, &request)?;
        let transaction_id = normalized_hash(&request.query.transaction_id, "transaction id")?;
        let (read, route, completeness) = self
            .read_transaction_policy(
                facts,
                &config,
                &request.context,
                &transaction_id,
                request.query.exact_source_id.as_deref(),
            )
            .await?;
        let outcome = match read.value {
            Some(transaction) => L2ReadOutcome::Found {
                value: L2TransactionDetail {
                    inspection: crate::lez::inspect_transaction_summary(&transaction),
                    transaction,
                    source: source_observation(&read.descriptor, read.retrieval),
                },
            },
            None => L2ReadOutcome::NotFound,
        };
        Ok(L2ReadReport::new(
            "lez.transaction",
            request,
            route,
            completeness,
            outcome,
        ))
    }

    pub(crate) async fn transaction_trace(
        &self,
        facts: &ZoneL2RuntimeFacts,
        request: ZoneL2Request<ZoneL2TransactionTraceQuery>,
    ) -> Result<L2ReadReport<L2TransactionTrace>, L2ReadFailure> {
        let config = self.validate_and_reconcile(facts, &request)?;
        let transaction_id = normalized_hash(&request.query.transaction_id, "transaction id")?;
        let (read, route, completeness) = self
            .read_transaction_policy(
                facts,
                &config,
                &request.context,
                &transaction_id,
                request.query.exact_source_id.as_deref(),
            )
            .await?;
        let outcome = match read.value {
            Some(transaction) => {
                let trace =
                    transaction_trace_value(&transaction, request.query.idl_program_id.as_deref())?;
                L2ReadOutcome::Found {
                    value: L2TransactionTrace {
                        trace,
                        transaction,
                        source: source_observation(&read.descriptor, read.retrieval),
                    },
                }
            }
            None => L2ReadOutcome::NotFound,
        };
        Ok(L2ReadReport::new(
            "lez.transaction_trace",
            request,
            route,
            completeness,
            outcome,
        ))
    }

    async fn read_transaction_policy(
        &self,
        facts: &ZoneL2RuntimeFacts,
        config: &ChannelSourceConfig,
        context: &ActiveZoneContext,
        transaction_id: &str,
        exact_source_id: Option<&str>,
    ) -> Result<(TransactionSourceRead, L2ReadRoute, L2RouteCompleteness), L2ReadFailure> {
        if let Some(source_id) = exact_source_id {
            let source = eligible_descriptor(facts, config, source_id, None)?;
            let read = self
                .read_transaction_source(
                    context,
                    source,
                    transaction_id,
                    L2RoutePolicy::ExactSource,
                )
                .await?;
            let route = L2ReadRoute {
                policy: L2RoutePolicy::ExactSource,
                attempts: vec![read.attempt.clone()],
            };
            return Ok((read, route, L2RouteCompleteness::SingleConfigured));
        }
        let indexer = optional_descriptor(facts, config, ZoneSourceRole::Indexer)?;
        let sequencer = optional_descriptor(facts, config, ZoneSourceRole::Sequencer)?;
        let Some(primary) = indexer.or_else(|| sequencer.clone()) else {
            return Err(source_unconfigured());
        };
        let primary_policy = if primary.role == ZoneSourceRole::Indexer {
            L2RoutePolicy::IndexerPrimary
        } else {
            L2RoutePolicy::SelectedSequencer
        };
        let primary_read = self
            .read_transaction_source(context, primary, transaction_id, primary_policy)
            .await?;
        if primary_read.value.is_some()
            || primary_read.descriptor.role == ZoneSourceRole::Sequencer
            || sequencer.is_none()
        {
            let route = L2ReadRoute {
                policy: primary_policy,
                attempts: vec![primary_read.attempt.clone()],
            };
            return Ok((primary_read, route, L2RouteCompleteness::SingleConfigured));
        }
        let fallback = sequencer.ok_or_else(source_unconfigured)?;
        let fallback_read = self
            .read_transaction_source(
                context,
                fallback,
                transaction_id,
                L2RoutePolicy::ConfirmedNotFoundFallback,
            )
            .await
            .map_err(|failure| append_attempt(failure, primary_read.attempt.clone()))?;
        let route = L2ReadRoute {
            policy: L2RoutePolicy::ConfirmedNotFoundFallback,
            attempts: vec![primary_read.attempt, fallback_read.attempt.clone()],
        };
        Ok((fallback_read, route, L2RouteCompleteness::AllConfigured))
    }

    async fn read_transaction_source(
        &self,
        context: &ActiveZoneContext,
        descriptor: L2SourceDescriptor,
        transaction_id: &str,
        policy: L2RoutePolicy,
    ) -> Result<TransactionSourceRead, L2ReadFailure> {
        let scope = cache_scope_from_context(context, &descriptor);
        if let Some(transaction) = self.lock_state()?.cache.transaction(&scope, transaction_id) {
            return Ok(TransactionSourceRead {
                attempt: route_attempt(
                    &descriptor,
                    L2RouteAttemptOutcome::Returned,
                    L2RouteContribution::Payload,
                    L2Retrieval::MemoryCache,
                ),
                descriptor,
                value: Some(transaction),
                retrieval: L2Retrieval::MemoryCache,
            });
        }
        let value = self
            .adapter
            .transaction(descriptor.clone(), transaction_id.to_owned())
            .await
            .map_err(|error| source_failure_with_policy(&descriptor, error, policy))?;
        if let Some(transaction) = &value {
            let returned =
                normalized_hash(&transaction.hash, "returned transaction id").map_err(|_| {
                    source_failure_with_policy(&descriptor, L2SourceError::protocol_error(), policy)
                })?;
            if returned != transaction_id {
                return Err(source_failure_with_policy(
                    &descriptor,
                    L2SourceError::protocol_error(),
                    policy,
                ));
            }
            self.lock_state()?
                .cache
                .insert_transaction(scope, transaction.clone());
        }
        Ok(TransactionSourceRead {
            attempt: route_attempt(
                &descriptor,
                if value.is_some() {
                    L2RouteAttemptOutcome::Returned
                } else {
                    L2RouteAttemptOutcome::NotFound
                },
                if value.is_some() {
                    L2RouteContribution::Payload
                } else {
                    L2RouteContribution::None
                },
                L2Retrieval::Live,
            ),
            descriptor,
            value,
            retrieval: L2Retrieval::Live,
        })
    }

    pub(crate) async fn account(
        &self,
        facts: &ZoneL2RuntimeFacts,
        request: ZoneL2Request<ZoneL2AccountQuery>,
    ) -> Result<L2ReadReport<L2AccountSnapshotData>, L2ReadFailure> {
        let config = self.validate_and_reconcile(facts, &request)?;
        let account_id = normalized_account_id(&request.query.account_id)?;
        let required_role = match request.query.snapshot {
            ZoneL2AccountSnapshot::Provisional => ZoneSourceRole::Sequencer,
            ZoneL2AccountSnapshot::Finalized | ZoneL2AccountSnapshot::Historical { .. } => {
                ZoneSourceRole::Indexer
            }
        };
        let policy = if request.query.exact_source_id.is_some() {
            L2RoutePolicy::ExactSource
        } else if required_role == ZoneSourceRole::Indexer {
            L2RoutePolicy::IndexerPrimary
        } else {
            L2RoutePolicy::SelectedSequencer
        };
        let source = if let Some(source_id) = request.query.exact_source_id.as_deref() {
            eligible_descriptor(facts, &config, source_id, Some(required_role))?
        } else {
            optional_descriptor(facts, &config, required_role)?.ok_or_else(source_unconfigured)?
        };
        let (value, attempt, warning) = match &request.query.snapshot {
            ZoneL2AccountSnapshot::Finalized => {
                let read = self
                    .read_indexer_account(
                        &request.context,
                        source,
                        &account_id,
                        None,
                        false,
                        policy,
                    )
                    .await?;
                (read.value, read.attempt, None)
            }
            ZoneL2AccountSnapshot::Historical {
                block_id,
                block_hash,
            } => {
                let anchor = L2BlockAnchor {
                    block_id: *block_id,
                    block_hash: normalized_hash(block_hash, "historical block hash")?,
                };
                let read = self
                    .read_indexer_account(
                        &request.context,
                        source,
                        &account_id,
                        Some(anchor),
                        true,
                        policy,
                    )
                    .await?;
                (read.value, read.attempt, None)
            }
            ZoneL2AccountSnapshot::Provisional => {
                let read = self
                    .read_provisional_account(source, &account_id, policy)
                    .await?;
                (Some(read.value), read.attempt, read.warning)
            }
        };
        let mut report = L2ReadReport::new(
            "lez.account",
            request,
            L2ReadRoute {
                policy,
                attempts: vec![attempt],
            },
            L2RouteCompleteness::SingleConfigured,
            value.map_or(L2ReadOutcome::NotFound, |value| L2ReadOutcome::Found {
                value,
            }),
        );
        if let Some(warning) = warning {
            report.warnings.push(warning);
        }
        Ok(report)
    }

    async fn read_indexer_account(
        &self,
        context: &ActiveZoneContext,
        source: L2SourceDescriptor,
        account_id: &str,
        requested_anchor: Option<L2BlockAnchor>,
        cacheable: bool,
        policy: L2RoutePolicy,
    ) -> Result<IndexerAccountRead, L2ReadFailure> {
        let scope = cache_scope_from_context(context, &source);
        if let Some(anchor) = requested_anchor.as_ref()
            && let Some(value) = self
                .lock_state()?
                .cache
                .historical_account(&scope, account_id, anchor)
        {
            return Ok(IndexerAccountRead {
                value: Some(value),
                attempt: route_attempt(
                    &source,
                    L2RouteAttemptOutcome::Returned,
                    L2RouteContribution::Payload,
                    L2Retrieval::MemoryCache,
                ),
            });
        }
        let block = match requested_anchor {
            Some(anchor) => {
                let block = self
                    .adapter
                    .block_by_id(source.clone(), anchor.block_id)
                    .await
                    .map_err(|error| source_failure_with_policy(&source, error, policy))?;
                match block {
                    Some(block) if block.summary.block_hash == anchor.block_hash => Some(block),
                    Some(_) | None => None,
                }
            }
            None => self
                .adapter
                .head(source.clone())
                .await
                .map_err(|error| source_failure_with_policy(&source, error, policy))?,
        };
        let Some(block) = block else {
            return Ok(IndexerAccountRead {
                value: None,
                attempt: route_attempt(
                    &source,
                    L2RouteAttemptOutcome::NotFound,
                    L2RouteContribution::None,
                    L2Retrieval::Live,
                ),
            });
        };
        let anchor = L2BlockAnchor {
            block_id: block.summary.block_id,
            block_hash: block.summary.block_hash,
        };
        let account = self
            .adapter
            .account_at_block(source.clone(), account_id.to_owned(), anchor.block_id)
            .await
            .map_err(|error| source_failure_with_policy(&source, error, policy))?;
        if cacheable {
            self.lock_state()?.cache.insert_historical_account(
                scope,
                account.clone(),
                anchor.clone(),
            );
        }
        Ok(IndexerAccountRead {
            value: Some(L2AccountSnapshotData {
                account,
                anchor: Some(anchor),
                after_anchor: None,
                anchor_state: L2AccountAnchorState::Exact,
                source: source_observation(&source, L2Retrieval::Live),
            }),
            attempt: route_attempt(
                &source,
                L2RouteAttemptOutcome::Returned,
                L2RouteContribution::Payload,
                L2Retrieval::Live,
            ),
        })
    }

    async fn read_provisional_account(
        &self,
        source: L2SourceDescriptor,
        account_id: &str,
        policy: L2RoutePolicy,
    ) -> Result<ProvisionalAccountRead, L2ReadFailure> {
        let first = self
            .provisional_account_attempt(&source, account_id, policy)
            .await?;
        let observation = if first.before == first.after {
            first
        } else {
            self.provisional_account_attempt(&source, account_id, policy)
                .await?
        };
        let exact = observation.before == observation.after;
        Ok(ProvisionalAccountRead {
            value: L2AccountSnapshotData {
                account: observation.account,
                anchor: observation.before,
                after_anchor: (!exact).then_some(observation.after).flatten(),
                anchor_state: if exact {
                    L2AccountAnchorState::Exact
                } else {
                    L2AccountAnchorState::Moving
                },
                source: source_observation(&source, L2Retrieval::Live),
            },
            attempt: route_attempt(
                &source,
                L2RouteAttemptOutcome::Returned,
                L2RouteContribution::Payload,
                L2Retrieval::Live,
            ),
            warning: (!exact).then(|| L2ReadWarning {
                code: L2ReadErrorCode::SourceUnavailable,
                recovery: L2RecoveryAction::Retry,
                message: "Sequencer head moved while account state was read".to_owned(),
            }),
        })
    }

    async fn provisional_account_attempt(
        &self,
        source: &L2SourceDescriptor,
        account_id: &str,
        policy: L2RoutePolicy,
    ) -> Result<ProvisionalAccountObservation, L2ReadFailure> {
        let before = self
            .adapter
            .head(source.clone())
            .await
            .map_err(|error| source_failure_with_policy(source, error, policy))?
            .map(block_anchor)
            .ok_or_else(|| {
                source_failure_with_policy(source, L2SourceError::protocol_error(), policy)
            })?;
        let account = self
            .adapter
            .current_account(source.clone(), account_id.to_owned())
            .await
            .map_err(|error| source_failure_with_policy(source, error, policy))?;
        let after = self
            .adapter
            .head(source.clone())
            .await
            .map_err(|error| source_failure_with_policy(source, error, policy))?
            .map(block_anchor)
            .ok_or_else(|| {
                source_failure_with_policy(source, L2SourceError::protocol_error(), policy)
            })?;
        Ok(ProvisionalAccountObservation {
            account,
            before: Some(before),
            after: Some(after),
        })
    }

    pub(crate) async fn account_activity(
        &self,
        facts: &ZoneL2RuntimeFacts,
        request: ZoneL2Request<ZoneL2AccountActivityQuery>,
    ) -> Result<L2ReadReport<L2AccountActivityPage>, L2ReadFailure> {
        let config = self.validate_and_reconcile(facts, &request)?;
        let account_id = normalized_account_id(&request.query.account_id)?;
        let limit = page_limit(request.query.limit)?;
        let source = optional_descriptor(facts, &config, ZoneSourceRole::Indexer)?
            .ok_or_else(source_unconfigured)?;
        let offset = if let Some(cursor) = request.query.cursor.as_deref() {
            let state = self.activity_cursor(cursor)?;
            validate_cursor_context(&request.context, &state.context)?;
            if state.source != source
                || state.account_id != account_id
                || state.order != request.query.order
            {
                return Err(cursor_invalidated());
            }
            state.next_offset
        } else {
            0
        };
        let mut rows = self
            .adapter
            .account_activity(
                source.clone(),
                account_id.clone(),
                offset,
                limit.saturating_add(1),
            )
            .await
            .map_err(|error| {
                source_failure_with_policy(&source, error, L2RoutePolicy::IndexerPrimary)
            })?;
        let has_more = rows.len() > limit;
        rows.truncate(limit);
        let mut page = L2AccountActivityPage {
            account_id: account_id.clone(),
            order: request.query.order,
            rows,
            next_cursor: None,
            has_more,
        };
        if has_more {
            page.next_cursor = Some(self.insert_activity_cursor(ActivityCursorState {
                context: request.context.clone(),
                source: source.clone(),
                account_id,
                next_offset: offset.saturating_add(page.rows.len()),
                order: request.query.order,
            })?);
        }
        Ok(L2ReadReport::new(
            "lez.account_activity",
            request,
            L2ReadRoute {
                policy: L2RoutePolicy::IndexerPrimary,
                attempts: vec![route_attempt(
                    &source,
                    L2RouteAttemptOutcome::Returned,
                    L2RouteContribution::Payload,
                    L2Retrieval::Live,
                )],
            },
            L2RouteCompleteness::SingleConfigured,
            L2ReadOutcome::Found { value: page },
        ))
    }

    pub(crate) async fn programs(
        &self,
        facts: &ZoneL2RuntimeFacts,
        request: ZoneL2Request<ZoneL2ProgramsQuery>,
    ) -> Result<L2ReadReport<L2ProgramsData>, L2ReadFailure> {
        let config = self.validate_and_reconcile(facts, &request)?;
        let (source, policy) = selected_or_exact_source(
            facts,
            &config,
            request.query.exact_source_id.as_deref(),
            ZoneSourceRole::Sequencer,
        )?;
        let programs = self
            .adapter
            .programs(source.clone())
            .await
            .map_err(|error| source_failure_with_policy(&source, error, policy))?;
        Ok(single_source_report(
            "lez.programs",
            request,
            source.clone(),
            policy,
            L2ReadOutcome::Found {
                value: L2ProgramsData {
                    programs,
                    source: source_observation(&source, L2Retrieval::Live),
                },
            },
        ))
    }

    pub(crate) async fn commitment_proof(
        &self,
        facts: &ZoneL2RuntimeFacts,
        request: ZoneL2Request<ZoneL2CommitmentProofQuery>,
    ) -> Result<L2ReadReport<L2CommitmentProofData>, L2ReadFailure> {
        let config = self.validate_and_reconcile(facts, &request)?;
        let commitment_hex = normalized_hash(&request.query.commitment_hex, "commitment")?;
        let (source, policy) = selected_or_exact_source(
            facts,
            &config,
            request.query.exact_source_id.as_deref(),
            ZoneSourceRole::Sequencer,
        )?;
        let proof = self
            .adapter
            .commitment_proof(source.clone(), commitment_hex.clone())
            .await
            .map_err(|error| source_failure_with_policy(&source, error, policy))?;
        let outcome = proof.map_or(L2ReadOutcome::NotFound, |(leaf_index, sibling_hashes)| {
            L2ReadOutcome::Found {
                value: L2CommitmentProofData {
                    commitment_hex,
                    leaf_index,
                    sibling_hashes,
                    source: source_observation(&source, L2Retrieval::Live),
                },
            }
        });
        Ok(single_source_report(
            "lez.commitment_proof",
            request,
            source,
            policy,
            outcome,
        ))
    }

    pub(crate) async fn account_nonces(
        &self,
        facts: &ZoneL2RuntimeFacts,
        request: ZoneL2Request<ZoneL2AccountNoncesQuery>,
    ) -> Result<L2ReadReport<L2AccountNoncesData>, L2ReadFailure> {
        let config = self.validate_and_reconcile(facts, &request)?;
        if request.query.account_ids.len() > MAX_NONCE_ACCOUNTS {
            return Err(L2ReadFailure::new(
                L2ReadErrorCode::InvalidRequest,
                "Too many accounts requested",
            ));
        }
        let account_ids = request
            .query
            .account_ids
            .iter()
            .map(|account_id| normalized_account_id(account_id))
            .collect::<Result<Vec<_>, _>>()?;
        let (source, policy) = selected_or_exact_source(
            facts,
            &config,
            request.query.exact_source_id.as_deref(),
            ZoneSourceRole::Sequencer,
        )?;
        let nonces = self
            .adapter
            .account_nonces(source.clone(), account_ids.clone())
            .await
            .map_err(|error| source_failure_with_policy(&source, error, policy))?;
        if nonces.len() != account_ids.len() {
            return Err(source_failure_with_policy(
                &source,
                L2SourceError::protocol_error(),
                policy,
            ));
        }
        let rows = account_ids
            .into_iter()
            .zip(nonces)
            .map(|(account_id, nonce)| L2AccountNonce { account_id, nonce })
            .collect();
        Ok(single_source_report(
            "lez.account_nonces",
            request,
            source.clone(),
            policy,
            L2ReadOutcome::Found {
                value: L2AccountNoncesData {
                    rows,
                    source: source_observation(&source, L2Retrieval::Live),
                },
            },
        ))
    }

    pub(crate) async fn transfers(
        &self,
        facts: &ZoneL2RuntimeFacts,
        request: ZoneL2Request<ZoneL2TransfersQuery>,
    ) -> Result<L2ReadReport<L2TransfersPage>, L2ReadFailure> {
        let config = self.validate_and_reconcile(facts, &request)?;
        let limit = page_limit(request.query.block_limit)?;
        let source = optional_descriptor(facts, &config, ZoneSourceRole::Indexer)?
            .ok_or_else(source_unconfigured)?;
        let (anchor, before) = if let Some(cursor) = request.query.cursor.as_deref() {
            let state = self.transfer_cursor(cursor)?;
            validate_cursor_context(&request.context, &state.context)?;
            if state.source != source {
                return Err(cursor_invalidated());
            }
            let resolved = self
                .adapter
                .block_by_id(source.clone(), state.anchor.block_id)
                .await
                .map_err(|error| {
                    source_failure_with_policy(&source, error, L2RoutePolicy::IndexerPrimary)
                })?;
            if resolved
                .as_ref()
                .map(|block| block.summary.block_hash.as_str())
                != Some(state.anchor.block_hash.as_str())
            {
                return Err(cursor_invalidated());
            }
            (state.anchor, Some(state.next_before))
        } else {
            let head = self.adapter.head(source.clone()).await.map_err(|error| {
                source_failure_with_policy(&source, error, L2RoutePolicy::IndexerPrimary)
            })?;
            let Some(head) = head else {
                return Ok(single_source_report(
                    "lez.transfers",
                    request,
                    source,
                    L2RoutePolicy::IndexerPrimary,
                    L2ReadOutcome::Found {
                        value: L2TransfersPage {
                            recipients: Vec::new(),
                            next_cursor: None,
                            has_more: false,
                            newest_block: None,
                            oldest_block: None,
                            scanned_blocks: 0,
                            finalized: true,
                        },
                    },
                ));
            };
            let anchor = block_anchor(head);
            let before = anchor.block_id.checked_add(1);
            (anchor, before)
        };
        let mut blocks = self
            .adapter
            .transfer_blocks(
                source.clone(),
                before,
                u64::try_from(limit.saturating_add(1)).unwrap_or(u64::MAX),
            )
            .await
            .map_err(|error| {
                source_failure_with_policy(&source, error, L2RoutePolicy::IndexerPrimary)
            })?;
        validate_indexer_window(&blocks, before.and_then(|value| value.checked_sub(1))).map_err(
            |error| source_failure_with_policy(&source, error, L2RoutePolicy::IndexerPrimary),
        )?;
        let has_more = blocks.len() > limit;
        blocks.truncate(limit);
        let newest_block = blocks.iter().filter_map(|block| block.block_id).max();
        let oldest_block = blocks.iter().filter_map(|block| block.block_id).min();
        let recipients = crate::lez::transfer_recipient_summaries_from_blocks(&blocks);
        let mut page = L2TransfersPage {
            recipients,
            next_cursor: None,
            has_more,
            newest_block,
            oldest_block,
            scanned_blocks: blocks.len(),
            finalized: true,
        };
        if has_more {
            let next_before = oldest_block.ok_or_else(|| {
                L2ReadFailure::new(
                    L2ReadErrorCode::SourceProtocolError,
                    "Indexer transfer window has no block identity",
                )
            })?;
            page.next_cursor = Some(self.insert_transfer_cursor(TransferCursorState {
                context: request.context.clone(),
                source: source.clone(),
                anchor,
                next_before,
            })?);
        }
        Ok(single_source_report(
            "lez.transfers",
            request,
            source,
            L2RoutePolicy::IndexerPrimary,
            L2ReadOutcome::Found { value: page },
        ))
    }

    fn validate_and_reconcile<T>(
        &self,
        facts: &ZoneL2RuntimeFacts,
        request: &ZoneL2Request<T>,
    ) -> Result<ChannelSourceConfig, L2ReadFailure> {
        let config = validate_context(facts, request)?;
        let valid_scopes = cache_scopes(facts);
        let mut state = self.lock_state()?;
        if facts.verification == CatalogVerificationState::Verified {
            state.cache.retain_scopes(&valid_scopes);
        } else {
            state.cache.clear();
        }
        Ok(config)
    }

    fn block_cursor(&self, token: &str) -> Result<BlockCursorState, L2ReadFailure> {
        self.lock_state()?
            .block_cursors
            .iter()
            .find(|(candidate, _)| candidate == token)
            .map(|(_, state)| state.clone())
            .ok_or_else(cursor_invalidated)
    }

    fn insert_block_cursor(&self, state: BlockCursorState) -> Result<String, L2ReadFailure> {
        let token = cursor_token("blocks")?;
        let mut router = self.lock_state()?;
        push_cursor(&mut router.block_cursors, token.clone(), state);
        Ok(token)
    }

    fn activity_cursor(&self, token: &str) -> Result<ActivityCursorState, L2ReadFailure> {
        self.lock_state()?
            .activity_cursors
            .iter()
            .find(|(candidate, _)| candidate == token)
            .map(|(_, state)| state.clone())
            .ok_or_else(cursor_invalidated)
    }

    fn insert_activity_cursor(&self, state: ActivityCursorState) -> Result<String, L2ReadFailure> {
        let token = cursor_token("activity")?;
        let mut router = self.lock_state()?;
        push_cursor(&mut router.activity_cursors, token.clone(), state);
        Ok(token)
    }

    fn transfer_cursor(&self, token: &str) -> Result<TransferCursorState, L2ReadFailure> {
        self.lock_state()?
            .transfer_cursors
            .iter()
            .find(|(candidate, _)| candidate == token)
            .map(|(_, state)| state.clone())
            .ok_or_else(cursor_invalidated)
    }

    fn insert_transfer_cursor(&self, state: TransferCursorState) -> Result<String, L2ReadFailure> {
        let token = cursor_token("transfers")?;
        let mut router = self.lock_state()?;
        push_cursor(&mut router.transfer_cursors, token.clone(), state);
        Ok(token)
    }

    fn lock_state(&self) -> Result<MutexGuard<'_, L2RouterState>, L2ReadFailure> {
        self.state
            .lock()
            .map_err(|_| L2ReadFailure::new(L2ReadErrorCode::Internal, "L2 read state lock failed"))
    }
}

#[derive(Default)]
struct L2RouterState {
    cache: L2EvidenceCache,
    block_cursors: VecDeque<(String, BlockCursorState)>,
    activity_cursors: VecDeque<(String, ActivityCursorState)>,
    transfer_cursors: VecDeque<(String, TransferCursorState)>,
}

#[derive(Debug, Clone)]
struct BlockCursorState {
    context: ActiveZoneContext,
    contributors: Vec<PinnedContributor>,
    source_heads: Vec<L2SourceHead>,
    next_before: u64,
}

#[derive(Debug, Clone)]
struct ActivityCursorState {
    context: ActiveZoneContext,
    source: L2SourceDescriptor,
    account_id: String,
    next_offset: usize,
    order: ZoneL2AccountActivityOrder,
}

#[derive(Debug, Clone)]
struct TransferCursorState {
    context: ActiveZoneContext,
    source: L2SourceDescriptor,
    anchor: L2BlockAnchor,
    next_before: u64,
}

#[derive(Debug, Clone)]
struct PinnedContributor {
    descriptor: L2SourceDescriptor,
    exhausted: bool,
}

#[derive(Debug, Clone)]
enum ContributorPlan {
    Absent,
    Eligible(L2SourceDescriptor),
    Ineligible(L2SourceDescriptor, L2ReadFailure),
}

#[derive(Debug, Clone)]
struct ContributorResult {
    descriptor: L2SourceDescriptor,
    head: Option<NormalizedL2Block>,
    blocks: Vec<NormalizedL2Block>,
    attempt: L2RouteAttempt,
    failure: Option<L2ReadFailure>,
    exhausted: bool,
}

#[derive(Debug)]
struct BlockSourceRead {
    descriptor: L2SourceDescriptor,
    value: Option<NormalizedL2Block>,
    attempt: L2RouteAttempt,
    retrieval: L2Retrieval,
}

#[derive(Debug)]
struct TransactionSourceRead {
    descriptor: L2SourceDescriptor,
    value: Option<TransactionSummary>,
    attempt: L2RouteAttempt,
    retrieval: L2Retrieval,
}

#[derive(Debug)]
struct IndexerAccountRead {
    value: Option<L2AccountSnapshotData>,
    attempt: L2RouteAttempt,
}

#[derive(Debug)]
struct ProvisionalAccountRead {
    value: L2AccountSnapshotData,
    attempt: L2RouteAttempt,
    warning: Option<L2ReadWarning>,
}

#[derive(Debug)]
struct ProvisionalAccountObservation {
    account: super::L2AccountValue,
    before: Option<L2BlockAnchor>,
    after: Option<L2BlockAnchor>,
}

impl ContributorResult {
    fn warning(&self) -> Option<L2ReadWarning> {
        let failure = self.failure.as_ref()?;
        Some(L2ReadWarning {
            code: failure.code,
            recovery: failure.code.recovery(),
            message: failure.message.clone(),
        })
    }
}

fn optional_descriptor(
    facts: &ZoneL2RuntimeFacts,
    config: &ChannelSourceConfig,
    role: ZoneSourceRole,
) -> Result<Option<L2SourceDescriptor>, L2ReadFailure> {
    let source_id = match role {
        ZoneSourceRole::Indexer => config
            .indexer_source
            .as_ref()
            .map(|source| source.source_id.as_str()),
        ZoneSourceRole::Sequencer => config.selected_sequencer_source_id.as_deref(),
    };
    source_id
        .map(|source_id| eligible_descriptor(facts, config, source_id, Some(role)))
        .transpose()
}

fn selected_or_exact_source(
    facts: &ZoneL2RuntimeFacts,
    config: &ChannelSourceConfig,
    exact_source_id: Option<&str>,
    role: ZoneSourceRole,
) -> Result<(L2SourceDescriptor, L2RoutePolicy), L2ReadFailure> {
    match exact_source_id {
        Some(source_id) => Ok((
            eligible_descriptor(facts, config, source_id, Some(role))?,
            L2RoutePolicy::ExactSource,
        )),
        None => Ok((
            optional_descriptor(facts, config, role)?.ok_or_else(source_unconfigured)?,
            if role == ZoneSourceRole::Indexer {
                L2RoutePolicy::IndexerPrimary
            } else {
                L2RoutePolicy::SelectedSequencer
            },
        )),
    }
}

fn normalized_account_id(value: &str) -> Result<String, L2ReadFailure> {
    crate::parse_account_id(value)
        .map(|account_id| account_id.to_string())
        .map_err(|_| L2ReadFailure::new(L2ReadErrorCode::InvalidRequest, "Account id is invalid"))
}

fn block_anchor(block: NormalizedL2Block) -> L2BlockAnchor {
    L2BlockAnchor {
        block_id: block.summary.block_id,
        block_hash: block.summary.block_hash,
    }
}

fn single_source_report<Q, T>(
    report_kind: &str,
    request: ZoneL2Request<Q>,
    source: L2SourceDescriptor,
    policy: L2RoutePolicy,
    outcome: L2ReadOutcome<T>,
) -> L2ReadReport<T> {
    let attempt_outcome = match &outcome {
        L2ReadOutcome::Found { .. } => L2RouteAttemptOutcome::Returned,
        L2ReadOutcome::NotFound => L2RouteAttemptOutcome::NotFound,
        L2ReadOutcome::Ambiguous { .. } => L2RouteAttemptOutcome::Returned,
    };
    L2ReadReport::new(
        report_kind,
        request,
        L2ReadRoute {
            policy,
            attempts: vec![route_attempt(
                &source,
                attempt_outcome,
                if attempt_outcome == L2RouteAttemptOutcome::Returned {
                    L2RouteContribution::Payload
                } else {
                    L2RouteContribution::None
                },
                L2Retrieval::Live,
            )],
        },
        L2RouteCompleteness::SingleConfigured,
        outcome,
    )
}

fn validate_indexer_window(
    blocks: &[IndexerBlockReport],
    expected_first: Option<u64>,
) -> Result<(), L2SourceError> {
    let normalized = blocks
        .iter()
        .cloned()
        .map(super::normalize_indexer_block)
        .collect::<Result<Vec<_>, _>>()?;
    validate_block_sequence(&normalized, expected_first, expected_first.is_some())
}

fn normalized_block_target(target: &ZoneL2BlockTarget) -> Result<ZoneL2BlockTarget, L2ReadFailure> {
    match target {
        ZoneL2BlockTarget::Id { block_id } => Ok(ZoneL2BlockTarget::Id {
            block_id: *block_id,
        }),
        ZoneL2BlockTarget::Hash { block_hash } => Ok(ZoneL2BlockTarget::Hash {
            block_hash: normalized_hash(block_hash, "block hash")?,
        }),
        ZoneL2BlockTarget::Identity {
            block_id,
            block_hash,
        } => Ok(ZoneL2BlockTarget::Identity {
            block_id: *block_id,
            block_hash: normalized_hash(block_hash, "block hash")?,
        }),
    }
}

fn normalized_hash(value: &str, label: &str) -> Result<String, L2ReadFailure> {
    crate::parse_hash(value, label)
        .map(|hash| hash.to_string())
        .map_err(|_| {
            L2ReadFailure::new(
                L2ReadErrorCode::InvalidRequest,
                format!("{label} is invalid"),
            )
        })
}

fn block_detail_value(
    block: NormalizedL2Block,
    descriptor: &L2SourceDescriptor,
    retrieval: L2Retrieval,
) -> L2BlockDetail {
    L2BlockDetail {
        summary: block.summary,
        transactions: block.transactions,
        source: source_observation(descriptor, retrieval),
    }
}

fn block_report(
    request: ZoneL2Request<ZoneL2BlockDetailQuery>,
    policy: L2RoutePolicy,
    attempts: Vec<L2RouteAttempt>,
    value: Option<L2BlockDetail>,
    completeness: L2RouteCompleteness,
) -> L2ReadReport<L2BlockDetail> {
    let outcome = value.map_or(L2ReadOutcome::NotFound, |value| L2ReadOutcome::Found {
        value,
    });
    block_outcome_report(request, policy, attempts, outcome, completeness)
}

fn block_outcome_report(
    request: ZoneL2Request<ZoneL2BlockDetailQuery>,
    policy: L2RoutePolicy,
    attempts: Vec<L2RouteAttempt>,
    outcome: L2ReadOutcome<L2BlockDetail>,
    completeness: L2RouteCompleteness,
) -> L2ReadReport<L2BlockDetail> {
    L2ReadReport::new(
        "lez.block_detail",
        request,
        L2ReadRoute { policy, attempts },
        completeness,
        outcome,
    )
}

fn exact_block_candidate(
    descriptor: &L2SourceDescriptor,
    block: &NormalizedL2Block,
) -> L2ExactSourceCandidate {
    L2ExactSourceCandidate {
        source_id: descriptor.source_id.clone(),
        source_role: descriptor.role,
        canonical_key: block.summary.canonical_key(),
    }
}

const fn route_completeness(second_source_returned: bool) -> L2RouteCompleteness {
    if second_source_returned {
        L2RouteCompleteness::AllConfigured
    } else {
        L2RouteCompleteness::SingleConfigured
    }
}

fn cache_scope_from_context(
    context: &ActiveZoneContext,
    descriptor: &L2SourceDescriptor,
) -> L2CacheScope {
    L2CacheScope {
        schema_version: L2_READ_SCHEMA_VERSION,
        network_scope: context.network_scope.clone(),
        channel_id: context.channel_id.clone(),
        source_id: descriptor.source_id.clone(),
        source_config_revision: descriptor.source_config_revision,
    }
}

fn append_attempt(mut failure: L2ReadFailure, prior: L2RouteAttempt) -> L2ReadFailure {
    let attempts = failure
        .route
        .take()
        .map_or_else(Vec::new, |route| route.attempts);
    let mut combined = Vec::with_capacity(attempts.len().saturating_add(1));
    combined.push(prior);
    combined.extend(attempts);
    failure.route = Some(L2ReadRoute {
        policy: L2RoutePolicy::ConfirmedNotFoundFallback,
        attempts: combined,
    });
    failure
}

fn transaction_trace_value(
    transaction: &TransactionSummary,
    idl_program_id: Option<&str>,
) -> Result<crate::lez::TransactionTraceReport, L2ReadFailure> {
    let Some(program_id) = idl_program_id else {
        return Ok(crate::lez::trace_transaction_summary(transaction));
    };
    let program_id = crate::normalize_program_id_hex(program_id).map_err(|_| {
        L2ReadFailure::new(L2ReadErrorCode::InvalidRequest, "IDL program id is invalid")
    })?;
    let entries = crate::support::state_store::registered_idl_entries().map_err(|_| {
        L2ReadFailure::new(
            L2ReadErrorCode::Internal,
            "Registered IDL state could not be read",
        )
    })?;
    let entry = entries
        .iter()
        .find(|entry| entry.program_id_hex == program_id)
        .ok_or_else(|| {
            L2ReadFailure::new(
                L2ReadErrorCode::InvalidRequest,
                "Requested IDL program is not registered",
            )
        })?;
    crate::lez::trace_transaction_summary_with_idl(transaction, &entry.json).map_err(|_| {
        L2ReadFailure::new(
            L2ReadErrorCode::InvalidRequest,
            "Registered IDL cannot decode this transaction",
        )
    })
}

async fn fetch_initial_contributor(
    adapter: &dyn ZoneL2SourceAdapter,
    plan: ContributorPlan,
    limit: usize,
) -> Option<ContributorResult> {
    match plan {
        ContributorPlan::Absent => None,
        ContributorPlan::Ineligible(descriptor, failure) => Some(ContributorResult {
            attempt: route_attempt(
                &descriptor,
                L2RouteAttemptOutcome::SkippedIneligible,
                L2RouteContribution::None,
                L2Retrieval::Live,
            ),
            descriptor,
            head: None,
            blocks: Vec::new(),
            failure: Some(failure),
            exhausted: true,
        }),
        ContributorPlan::Eligible(descriptor) => {
            let head = match adapter.head(descriptor.clone()).await {
                Ok(head) => head,
                Err(error) => return Some(failed_contributor(descriptor, error)),
            };
            let Some(head_block) = head.clone() else {
                return Some(ContributorResult {
                    attempt: route_attempt(
                        &descriptor,
                        L2RouteAttemptOutcome::NotFound,
                        L2RouteContribution::None,
                        L2Retrieval::Live,
                    ),
                    descriptor,
                    head: None,
                    blocks: Vec::new(),
                    failure: None,
                    exhausted: true,
                });
            };
            let before = head_block.summary.block_id.checked_add(1);
            let blocks = match adapter
                .blocks(
                    descriptor.clone(),
                    before,
                    u64::try_from(limit).unwrap_or(u64::MAX),
                )
                .await
            {
                Ok(blocks) => blocks,
                Err(error) => return Some(failed_contributor(descriptor, error)),
            };
            if let Err(error) =
                validate_block_sequence(&blocks, Some(head_block.summary.block_id), true)
            {
                return Some(failed_contributor(descriptor, error));
            }
            Some(ContributorResult {
                attempt: route_attempt(
                    &descriptor,
                    if blocks.is_empty() {
                        L2RouteAttemptOutcome::NotFound
                    } else {
                        L2RouteAttemptOutcome::Returned
                    },
                    contribution_for(descriptor.role),
                    L2Retrieval::Live,
                ),
                descriptor,
                head,
                exhausted: blocks.len() < limit,
                blocks,
                failure: None,
            })
        }
    }
}

async fn fetch_continuation_contributor(
    adapter: &dyn ZoneL2SourceAdapter,
    pinned: Option<PinnedContributor>,
    heads: &[L2SourceHead],
    before: u64,
    limit: usize,
) -> Option<ContributorResult> {
    let pinned = pinned?;
    let descriptor = pinned.descriptor;
    let head = heads
        .iter()
        .find(|head| head.source_id == descriptor.source_id)
        .cloned();
    if let Some(anchor) = &head {
        let verification = adapter
            .block_by_id(descriptor.clone(), anchor.block_id)
            .await;
        match verification {
            Ok(Some(block)) if block.summary.block_hash == anchor.block_hash => {}
            Ok(_) => {
                return Some(failed_cursor_contributor(descriptor, cursor_invalidated()));
            }
            Err(error) => return Some(failed_contributor(descriptor, error)),
        }
    }
    if pinned.exhausted || before == 0 {
        return Some(ContributorResult {
            attempt: route_attempt(
                &descriptor,
                L2RouteAttemptOutcome::NotFound,
                L2RouteContribution::None,
                L2Retrieval::Live,
            ),
            descriptor,
            head: None,
            blocks: Vec::new(),
            failure: None,
            exhausted: true,
        });
    }
    let blocks = match adapter
        .blocks(
            descriptor.clone(),
            Some(before),
            u64::try_from(limit).unwrap_or(u64::MAX),
        )
        .await
    {
        Ok(blocks) => blocks,
        Err(error) => return Some(failed_contributor(descriptor, error)),
    };
    if let Err(error) = validate_block_sequence(&blocks, before.checked_sub(1), true) {
        return Some(failed_contributor(descriptor, error));
    }
    Some(ContributorResult {
        attempt: route_attempt(
            &descriptor,
            if blocks.is_empty() {
                L2RouteAttemptOutcome::NotFound
            } else {
                L2RouteAttemptOutcome::Returned
            },
            contribution_for(descriptor.role),
            L2Retrieval::Live,
        ),
        descriptor,
        head: None,
        exhausted: blocks.len() < limit,
        blocks,
        failure: None,
    })
}

fn compose_block_page(
    results: &[ContributorResult],
    limit: usize,
) -> (L2BlocksPage, Vec<PinnedContributor>) {
    let mut groups = BTreeMap::<u64, BTreeMap<String, L2BlockRow>>::new();
    for result in results {
        if result.failure.is_some() {
            continue;
        }
        for block in &result.blocks {
            let by_hash = groups.entry(block.summary.block_id).or_default();
            let row = by_hash
                .entry(block.summary.block_hash.clone())
                .or_insert_with(|| L2BlockRow {
                    summary: block.summary.clone(),
                    observations: Vec::new(),
                });
            row.observations
                .push(source_observation(&result.descriptor, L2Retrieval::Live));
        }
    }
    let selected_ids = groups
        .keys()
        .rev()
        .take(limit)
        .copied()
        .collect::<BTreeSet<_>>();
    let mut rows = groups
        .into_iter()
        .rev()
        .filter(|(block_id, _)| selected_ids.contains(block_id))
        .flat_map(|(_, variants)| variants.into_values())
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .summary
            .block_id
            .cmp(&left.summary.block_id)
            .then_with(|| left.summary.block_hash.cmp(&right.summary.block_hash))
    });
    for row in &mut rows {
        row.observations.sort_by(|left, right| {
            source_role_order(left.source_role)
                .cmp(&source_role_order(right.source_role))
                .then_with(|| left.source_id.cmp(&right.source_id))
        });
    }
    let lowest = rows.iter().map(|row| row.summary.block_id).min();
    let has_more = lowest.is_some_and(|boundary| {
        results.iter().any(|result| {
            result.failure.is_none()
                && (!result.exhausted
                    || result
                        .blocks
                        .iter()
                        .any(|block| block.summary.block_id < boundary))
        })
    });
    let pinned = results
        .iter()
        .filter(|result| result.failure.is_none())
        .map(|result| PinnedContributor {
            descriptor: result.descriptor.clone(),
            exhausted: lowest.is_none_or(|boundary| {
                result.exhausted
                    && !result
                        .blocks
                        .iter()
                        .any(|block| block.summary.block_id < boundary)
            }),
        })
        .collect();
    let source_heads = results
        .iter()
        .filter_map(|result| {
            let head = result.head.as_ref()?;
            Some(L2SourceHead {
                source_id: result.descriptor.source_id.clone(),
                source_role: result.descriptor.role,
                block_id: head.summary.block_id,
                block_hash: head.summary.block_hash.clone(),
            })
        })
        .collect();
    (
        L2BlocksPage {
            distinct_block_ids: selected_ids.len(),
            rows,
            next_cursor: None,
            has_more,
            source_heads,
        },
        pinned,
    )
}

fn validate_block_sequence(
    blocks: &[NormalizedL2Block],
    expected_first: Option<u64>,
    require_first: bool,
) -> Result<(), L2SourceError> {
    if require_first
        && let Some(expected) = expected_first
        && blocks.first().map(|block| block.summary.block_id) != Some(expected)
    {
        return Err(L2SourceError::protocol_error());
    }
    if blocks.windows(2).any(|pair| match pair {
        [newer, older] => newer.summary.block_id.checked_sub(1) != Some(older.summary.block_id),
        _ => false,
    }) {
        return Err(L2SourceError::protocol_error());
    }
    Ok(())
}

fn optional_role_plan(
    facts: &ZoneL2RuntimeFacts,
    config: &ChannelSourceConfig,
    role: ZoneSourceRole,
) -> ContributorPlan {
    let source_id = match role {
        ZoneSourceRole::Indexer => config
            .indexer_source
            .as_ref()
            .map(|source| source.source_id.as_str()),
        ZoneSourceRole::Sequencer => config.selected_sequencer_source_id.as_deref(),
    };
    let Some(source_id) = source_id else {
        return ContributorPlan::Absent;
    };
    let descriptor = configured_descriptor(config, source_id);
    match descriptor {
        Some(descriptor) => match source_eligibility(facts, config, source_id, role) {
            Ok(()) => ContributorPlan::Eligible(descriptor),
            Err(failure) => ContributorPlan::Ineligible(descriptor, failure),
        },
        None => ContributorPlan::Absent,
    }
}

fn validate_context<T>(
    facts: &ZoneL2RuntimeFacts,
    request: &ZoneL2Request<T>,
) -> Result<ChannelSourceConfig, L2ReadFailure> {
    if request.context.context_revision == 0 || request.request_revision == 0 {
        return Err(L2ReadFailure::new(
            L2ReadErrorCode::InvalidRequest,
            "Zone L2 revisions must be positive",
        ));
    }
    if !is_hex_identity(&request.context.channel_id) {
        return Err(L2ReadFailure::new(
            L2ReadErrorCode::InvalidRequest,
            "Active Zone Channel id is invalid",
        ));
    }
    if facts.verification != CatalogVerificationState::Verified {
        return Err(L2ReadFailure::new(
            L2ReadErrorCode::ZoneUnverified,
            "Zone Catalog is not verified",
        ));
    }
    if facts.network_scope.as_ref() != Some(&request.context.network_scope) {
        return Err(stale_context("Active Zone network scope is stale"));
    }
    let Some(summary) = facts.summaries.get(&request.context.channel_id) else {
        return Err(stale_context("Active Zone no longer exists"));
    };
    if summary.kind() != ZoneKind::SequencerZone {
        return Err(L2ReadFailure::new(
            L2ReadErrorCode::L2NotApplicable,
            "Active Zone has no Sequencer-backed L2",
        ));
    }
    if request.context.zone_kind != summary.kind() {
        return Err(stale_context("Active Zone kind is stale"));
    }
    let config = facts
        .configs
        .iter()
        .find(|config| {
            config.network_scope == request.context.network_scope
                && config.channel_id == request.context.channel_id
        })
        .cloned()
        .unwrap_or_else(|| ChannelSourceConfig {
            network_scope: request.context.network_scope.clone(),
            channel_id: request.context.channel_id.clone(),
            config_revision: 0,
            sequencer_sources: Vec::new(),
            selected_sequencer_source_id: None,
            indexer_source: None,
        });
    if request.context.source_config_revision != config.config_revision
        || request.context.selected_sequencer_source_id != config.selected_sequencer_source_id
        || request.context.indexer_source_id
            != config
                .indexer_source
                .as_ref()
                .map(|source| source.source_id.clone())
    {
        return Err(stale_context("Active Zone source configuration is stale"));
    }
    Ok(config)
}

fn configured_descriptor(
    config: &ChannelSourceConfig,
    source_id: &str,
) -> Option<L2SourceDescriptor> {
    if let Some(source) = config
        .sequencer_sources
        .iter()
        .find(|source| source.source_id == source_id)
    {
        return Some(L2SourceDescriptor {
            source_id: source.source_id.clone(),
            role: ZoneSourceRole::Sequencer,
            target: source.target.clone(),
            source_config_revision: config.config_revision,
        });
    }
    config
        .indexer_source
        .as_ref()
        .filter(|source| source.source_id == source_id)
        .map(|source| L2SourceDescriptor {
            source_id: source.source_id.clone(),
            role: ZoneSourceRole::Indexer,
            target: source.target.clone(),
            source_config_revision: config.config_revision,
        })
}

fn eligible_descriptor(
    facts: &ZoneL2RuntimeFacts,
    config: &ChannelSourceConfig,
    source_id: &str,
    required_role: Option<ZoneSourceRole>,
) -> Result<L2SourceDescriptor, L2ReadFailure> {
    let descriptor = configured_descriptor(config, source_id).ok_or_else(|| {
        L2ReadFailure::new(
            L2ReadErrorCode::SourceIneligible,
            "Source does not belong to the active Channel",
        )
    })?;
    if required_role.is_some_and(|role| descriptor.role != role) {
        return Err(L2ReadFailure::new(
            L2ReadErrorCode::SourceIneligible,
            "Source role cannot serve this L2 read",
        ));
    }
    source_eligibility(facts, config, source_id, descriptor.role)?;
    Ok(descriptor)
}

fn source_eligibility(
    facts: &ZoneL2RuntimeFacts,
    config: &ChannelSourceConfig,
    source_id: &str,
    role: ZoneSourceRole,
) -> Result<(), L2ReadFailure> {
    let observed_role = match role {
        ZoneSourceRole::Sequencer => ChannelSourceRole::Sequencer,
        ZoneSourceRole::Indexer => ChannelSourceRole::Indexer,
    };
    let observation = facts
        .observations
        .channels
        .iter()
        .find(|set| {
            set.channel_id == config.channel_id && set.config_revision == config.config_revision
        })
        .and_then(|set| {
            set.observations.iter().find(|observation| {
                observation.source_id == source_id && observation.role == observed_role
            })
        });
    if role == ZoneSourceRole::Sequencer {
        let source = config
            .sequencer_sources
            .iter()
            .find(|source| source.source_id == source_id)
            .ok_or_else(source_unconfigured)?;
        let runtime_attested = observation.is_some_and(|observation| {
            observation.binding_state == Some(ChannelSourceBindingState::RuntimeAttested)
        });
        if source.channel_attestation == PersistedSequencerAttestation::Pending && !runtime_attested
        {
            return Err(L2ReadFailure::new(
                L2ReadErrorCode::SourceIneligible,
                "Sequencer source has no matching Channel attestation",
            ));
        }
    }
    if observation
        .is_some_and(|observation| observation.health == ChannelSourceHealthState::ChannelMismatch)
    {
        return Err(L2ReadFailure::new(
            L2ReadErrorCode::SourceIneligible,
            "Source reports another Channel",
        ));
    }
    Ok(())
}

fn cache_scopes(facts: &ZoneL2RuntimeFacts) -> Vec<L2CacheScope> {
    if facts.verification != CatalogVerificationState::Verified {
        return Vec::new();
    }
    facts
        .configs
        .iter()
        .flat_map(|config| {
            let sequencers = config.sequencer_sources.iter().filter_map(|source| {
                source_eligibility(facts, config, &source.source_id, ZoneSourceRole::Sequencer)
                    .ok()?;
                Some(cache_scope(config, &source.source_id))
            });
            let indexer = config.indexer_source.iter().filter_map(|source| {
                source_eligibility(facts, config, &source.source_id, ZoneSourceRole::Indexer)
                    .ok()?;
                Some(cache_scope(config, &source.source_id))
            });
            sequencers.chain(indexer)
        })
        .collect()
}

fn cache_scope(config: &ChannelSourceConfig, source_id: &str) -> L2CacheScope {
    L2CacheScope {
        schema_version: L2_READ_SCHEMA_VERSION,
        network_scope: config.network_scope.clone(),
        channel_id: config.channel_id.clone(),
        source_id: source_id.to_owned(),
        source_config_revision: config.config_revision,
    }
}

fn route_from_contributors(results: &[ContributorResult]) -> L2ReadRoute {
    L2ReadRoute {
        policy: L2RoutePolicy::Composite,
        attempts: results
            .iter()
            .map(|result| result.attempt.clone())
            .collect(),
    }
}

fn route_attempt(
    descriptor: &L2SourceDescriptor,
    outcome: L2RouteAttemptOutcome,
    contribution: L2RouteContribution,
    retrieval: L2Retrieval,
) -> L2RouteAttempt {
    L2RouteAttempt {
        source_id: descriptor.source_id.clone(),
        source_role: descriptor.role,
        outcome,
        contribution,
        finality: Some(finality_for(descriptor.role)),
        source_config_revision: descriptor.source_config_revision,
        retrieval,
    }
}

fn source_observation(
    descriptor: &L2SourceDescriptor,
    retrieval: L2Retrieval,
) -> L2SourceObservation {
    L2SourceObservation {
        source_id: descriptor.source_id.clone(),
        source_role: descriptor.role,
        source_config_revision: descriptor.source_config_revision,
        finality: finality_for(descriptor.role),
        retrieval,
    }
}

const fn finality_for(role: ZoneSourceRole) -> L2RouteFinality {
    match role {
        ZoneSourceRole::Indexer => L2RouteFinality::Finalized,
        ZoneSourceRole::Sequencer => L2RouteFinality::Provisional,
    }
}

const fn contribution_for(role: ZoneSourceRole) -> L2RouteContribution {
    match role {
        ZoneSourceRole::Indexer => L2RouteContribution::FinalizedPrefix,
        ZoneSourceRole::Sequencer => L2RouteContribution::ProvisionalTail,
    }
}

const fn source_role_order(role: ZoneSourceRole) -> u8 {
    match role {
        ZoneSourceRole::Indexer => 0,
        ZoneSourceRole::Sequencer => 1,
    }
}

fn failed_contributor(descriptor: L2SourceDescriptor, error: L2SourceError) -> ContributorResult {
    let failure = source_failure(&descriptor, error);
    failed_cursor_contributor(descriptor, failure)
}

fn failed_cursor_contributor(
    descriptor: L2SourceDescriptor,
    failure: L2ReadFailure,
) -> ContributorResult {
    ContributorResult {
        attempt: route_attempt(
            &descriptor,
            L2RouteAttemptOutcome::Failed,
            L2RouteContribution::None,
            L2Retrieval::Live,
        ),
        descriptor,
        head: None,
        blocks: Vec::new(),
        failure: Some(failure),
        exhausted: false,
    }
}

fn composite_failure(results: &[ContributorResult], route: L2ReadRoute) -> L2ReadFailure {
    results
        .iter()
        .find_map(|result| result.failure.clone())
        .unwrap_or_else(source_unconfigured)
        .with_route(route)
}

fn source_failure(descriptor: &L2SourceDescriptor, error: L2SourceError) -> L2ReadFailure {
    let (code, message) = match error.kind {
        L2SourceErrorKind::Unavailable => (
            L2ReadErrorCode::SourceUnavailable,
            "L2 source is unavailable",
        ),
        L2SourceErrorKind::Protocol => (
            L2ReadErrorCode::SourceProtocolError,
            "L2 source returned invalid evidence",
        ),
        L2SourceErrorKind::Capability => (
            L2ReadErrorCode::SourceCapabilityUnavailable,
            "L2 source does not expose this read capability",
        ),
    };
    L2ReadFailure::new(code, message).with_route(L2ReadRoute {
        policy: if descriptor.role == ZoneSourceRole::Indexer {
            L2RoutePolicy::IndexerPrimary
        } else {
            L2RoutePolicy::SelectedSequencer
        },
        attempts: vec![route_attempt(
            descriptor,
            L2RouteAttemptOutcome::Failed,
            L2RouteContribution::None,
            L2Retrieval::Live,
        )],
    })
}

fn source_failure_with_policy(
    descriptor: &L2SourceDescriptor,
    error: L2SourceError,
    policy: L2RoutePolicy,
) -> L2ReadFailure {
    let mut failure = source_failure(descriptor, error);
    if let Some(route) = &mut failure.route {
        route.policy = policy;
    }
    failure
}

fn page_limit(limit: Option<u16>) -> Result<usize, L2ReadFailure> {
    let limit = usize::from(limit.unwrap_or(DEFAULT_PAGE_LIMIT as u16));
    if limit == 0 {
        return Err(L2ReadFailure::new(
            L2ReadErrorCode::InvalidRequest,
            "Page limit must be positive",
        ));
    }
    Ok(limit.min(MAX_PAGE_LIMIT))
}

fn validate_cursor_context(
    request: &ActiveZoneContext,
    cursor: &ActiveZoneContext,
) -> Result<(), L2ReadFailure> {
    if request != cursor {
        return Err(cursor_invalidated());
    }
    Ok(())
}

fn cursor_token(kind: &str) -> Result<String, L2ReadFailure> {
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random).map_err(|_| {
        L2ReadFailure::new(L2ReadErrorCode::Internal, "L2 cursor generation failed")
    })?;
    Ok(format!("l2c1_{kind}_{}", hex::encode(random)))
}

fn push_cursor<T>(cursors: &mut VecDeque<(String, T)>, token: String, state: T) {
    cursors.push_back((token, state));
    while cursors.len() > MAX_CURSOR_ENTRIES {
        cursors.pop_front();
    }
}

fn cursor_invalidated() -> L2ReadFailure {
    L2ReadFailure::new(
        L2ReadErrorCode::CursorInvalidated,
        "L2 cursor no longer matches current source evidence",
    )
}

fn stale_context(message: impl Into<String>) -> L2ReadFailure {
    L2ReadFailure::new(L2ReadErrorCode::StaleContext, message)
}

fn source_unconfigured() -> L2ReadFailure {
    L2ReadFailure::new(
        L2ReadErrorCode::SourceUnconfigured,
        "Active Zone has no source configured for this read",
    )
}

fn is_hex_identity(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|character| character.is_ascii_hexdigit())
}

#[derive(Debug, Clone)]
pub(crate) struct L2ReadFailure {
    pub code: L2ReadErrorCode,
    pub message: String,
    pub route: Option<L2ReadRoute>,
    pub current_context_revision: Option<u64>,
}

impl std::fmt::Display for L2ReadFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for L2ReadFailure {}

impl L2ReadFailure {
    pub(crate) fn new(code: L2ReadErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            route: None,
            current_context_revision: None,
        }
    }

    fn with_route(mut self, route: L2ReadRoute) -> Self {
        self.route = Some(route);
        self
    }

    #[must_use]
    pub(crate) fn details<T>(&self, request: &ZoneL2Request<T>) -> L2ReadErrorDetails {
        let mut details = L2ReadErrorDetails::new(request, self.code);
        details.current_context_revision = self.current_context_revision;
        details.attempted_route = self.route.clone();
        details
    }
}

#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
