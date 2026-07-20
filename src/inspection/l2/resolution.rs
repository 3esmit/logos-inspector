use std::collections::BTreeSet;

use super::{
    ActiveZoneContext, InspectionEntityRef, InspectionL1EntityKind, InspectionResolveTargetRequest,
    InspectionTargetCandidate, InspectionTargetResolutionReport, InspectionTargetResolutionStatus,
    InspectionTargetResolutionWarning, L2ReadFailure, L2ReadOutcome, L2RecoveryAction,
    L2RouteFinality, ZoneL2BlockDetailQuery, ZoneL2BlockTarget, ZoneL2EntityKind, ZoneL2EntityRef,
    ZoneL2ProgramsQuery, ZoneL2Request, ZoneL2Router, ZoneL2RuntimeFacts, ZoneL2SourceQualifier,
    ZoneL2TransactionQuery, canonical_hash,
};
use crate::inspection::{CatalogVerificationState, NetworkScope, ZoneSourceRole};

pub(crate) const INSPECTION_RESOLUTION_REPORT_KIND: &str = "inspection.target_resolution";
pub(crate) const INSPECTION_RESOLUTION_SCHEMA_VERSION: u32 = 1;

impl ZoneL2Router {
    pub(crate) async fn resolve_target(
        &self,
        facts: &ZoneL2RuntimeFacts,
        request: InspectionResolveTargetRequest,
    ) -> InspectionTargetResolutionReport {
        let parsed = ParsedTarget::parse(&request.query);
        let mut report = InspectionTargetResolutionReport {
            report_kind: INSPECTION_RESOLUTION_REPORT_KIND.to_owned(),
            schema_version: INSPECTION_RESOLUTION_SCHEMA_VERSION,
            query: request.query.trim().to_owned(),
            request_revision: request.request_revision,
            context_revision: request
                .active_zone_context
                .as_ref()
                .map(|context| context.context_revision),
            status: InspectionTargetResolutionStatus::NotFound,
            candidates: Vec::new(),
            recovery: None,
            warnings: Vec::new(),
        };
        let Some(parsed) = parsed else {
            return report;
        };

        self.resolve_zone_and_l1(facts, &parsed, &mut report);
        if parsed.needs_l2() {
            let Some(context) = request.active_zone_context else {
                if parsed.explicit_l2() {
                    report.status = InspectionTargetResolutionStatus::Recovery;
                    report.recovery = Some(L2RecoveryAction::RefreshContext);
                }
                finalize_report(&mut report);
                return report;
            };
            self.resolve_l2(
                facts,
                &context,
                request.request_revision,
                &parsed,
                &mut report,
            )
            .await;
        }

        finalize_report(&mut report);
        report
    }

    fn resolve_zone_and_l1(
        &self,
        facts: &ZoneL2RuntimeFacts,
        parsed: &ParsedTarget,
        report: &mut InspectionTargetResolutionReport,
    ) {
        let Some(network_scope) = facts.network_scope.clone() else {
            return;
        };
        match parsed {
            ParsedTarget::Zone(channel_id) => {
                if let Some(summary) = facts.summaries.get(channel_id) {
                    report.candidates.push(InspectionTargetCandidate {
                        entity_ref: InspectionEntityRef::Zone {
                            network_scope,
                            channel_id: summary.channel_id.clone(),
                            zone_kind: summary.kind(),
                        },
                        finality: None,
                    });
                }
            }
            ParsedTarget::L1Block(BlockInput::Id(block_id))
            | ParsedTarget::UnprefixedNumber(block_id)
            | ParsedTarget::TypedBlock(BlockInput::Id(block_id)) => {
                report
                    .candidates
                    .push(l1_block_candidate(network_scope, *block_id, None));
            }
            ParsedTarget::L1Block(BlockInput::Hash(block_hash))
            | ParsedTarget::TypedBlock(BlockInput::Hash(block_hash))
            | ParsedTarget::UnprefixedHash(block_hash) => {
                let mut found = BTreeSet::new();
                for summary in facts.summaries.values() {
                    if summary.l1_channel.tip_hash.as_deref() != Some(block_hash.as_str()) {
                        continue;
                    }
                    let Some(block_id) = summary.l1_channel.tip_slot else {
                        continue;
                    };
                    if found.insert((block_id, block_hash.clone())) {
                        report.candidates.push(l1_block_candidate(
                            network_scope.clone(),
                            block_id,
                            Some(block_hash.clone()),
                        ));
                    }
                }
            }
            ParsedTarget::L2Block(_)
            | ParsedTarget::Transaction(_)
            | ParsedTarget::Account(_)
            | ParsedTarget::Program(_) => {}
        }
    }

    async fn resolve_l2(
        &self,
        facts: &ZoneL2RuntimeFacts,
        context: &ActiveZoneContext,
        request_revision: u64,
        parsed: &ParsedTarget,
        report: &mut InspectionTargetResolutionReport,
    ) {
        if facts.verification != CatalogVerificationState::Verified {
            report.recovery = Some(L2RecoveryAction::RefreshContext);
            return;
        }
        match parsed {
            ParsedTarget::L2Block(input) | ParsedTarget::TypedBlock(input) => {
                self.resolve_l2_block(facts, context, request_revision, input, report)
                    .await;
            }
            ParsedTarget::UnprefixedNumber(block_id) => {
                self.resolve_l2_block(
                    facts,
                    context,
                    request_revision,
                    &BlockInput::Id(*block_id),
                    report,
                )
                .await;
            }
            ParsedTarget::UnprefixedHash(hash) => {
                self.resolve_l2_block(
                    facts,
                    context,
                    request_revision,
                    &BlockInput::Hash(hash.clone()),
                    report,
                )
                .await;
                self.resolve_l2_transaction(facts, context, request_revision, hash, report)
                    .await;
                self.resolve_l2_program(facts, context, request_revision, hash, report)
                    .await;
            }
            ParsedTarget::Transaction(transaction_id) => {
                self.resolve_l2_transaction(
                    facts,
                    context,
                    request_revision,
                    transaction_id,
                    report,
                )
                .await;
            }
            ParsedTarget::Account(account_id) => {
                report.candidates.push(l2_candidate(
                    context,
                    ZoneL2EntityKind::Account,
                    account_id.clone(),
                    ZoneL2SourceQualifier::Policy,
                    None,
                ));
            }
            ParsedTarget::Program(program_id) => {
                self.resolve_l2_program(facts, context, request_revision, program_id, report)
                    .await;
            }
            ParsedTarget::Zone(_) | ParsedTarget::L1Block(_) => {}
        }
    }

    async fn resolve_l2_block(
        &self,
        facts: &ZoneL2RuntimeFacts,
        context: &ActiveZoneContext,
        request_revision: u64,
        input: &BlockInput,
        report: &mut InspectionTargetResolutionReport,
    ) {
        let target = match input {
            BlockInput::Id(block_id) => ZoneL2BlockTarget::Id {
                block_id: *block_id,
            },
            BlockInput::Hash(block_hash) => ZoneL2BlockTarget::Hash {
                block_hash: block_hash.clone(),
            },
        };
        match self
            .block_detail(
                facts,
                ZoneL2Request {
                    context: context.clone(),
                    request_revision,
                    query: ZoneL2BlockDetailQuery {
                        target,
                        exact_source_id: None,
                    },
                },
            )
            .await
        {
            Ok(read) => match read.data {
                L2ReadOutcome::Found { value } => {
                    report.candidates.push(l2_candidate(
                        context,
                        ZoneL2EntityKind::Block,
                        value.summary.canonical_key(),
                        exact_source(&value.source.source_id, value.source.source_role),
                        Some(value.source.finality),
                    ));
                }
                L2ReadOutcome::Ambiguous { candidates } => {
                    report
                        .candidates
                        .extend(candidates.into_iter().map(|candidate| {
                            l2_candidate(
                                context,
                                ZoneL2EntityKind::Block,
                                candidate.canonical_key,
                                exact_source(&candidate.source_id, candidate.source_role),
                                Some(finality_for_role(candidate.source_role)),
                            )
                        }));
                }
                L2ReadOutcome::NotFound => {}
            },
            Err(failure) => record_warning(report, failure),
        }
    }

    async fn resolve_l2_transaction(
        &self,
        facts: &ZoneL2RuntimeFacts,
        context: &ActiveZoneContext,
        request_revision: u64,
        transaction_id: &str,
        report: &mut InspectionTargetResolutionReport,
    ) {
        match self
            .transaction(
                facts,
                ZoneL2Request {
                    context: context.clone(),
                    request_revision,
                    query: ZoneL2TransactionQuery {
                        transaction_id: transaction_id.to_owned(),
                        exact_source_id: None,
                    },
                },
            )
            .await
        {
            Ok(read) => match read.data {
                L2ReadOutcome::Found { value } => report.candidates.push(l2_candidate(
                    context,
                    ZoneL2EntityKind::Transaction,
                    value.transaction.hash,
                    exact_source(&value.source.source_id, value.source.source_role),
                    Some(value.source.finality),
                )),
                L2ReadOutcome::Ambiguous { candidates } => {
                    report
                        .candidates
                        .extend(candidates.into_iter().map(|candidate| {
                            l2_candidate(
                                context,
                                ZoneL2EntityKind::Transaction,
                                candidate.canonical_key,
                                exact_source(&candidate.source_id, candidate.source_role),
                                Some(finality_for_role(candidate.source_role)),
                            )
                        }));
                }
                L2ReadOutcome::NotFound => {}
            },
            Err(failure) => record_warning(report, failure),
        }
    }

    async fn resolve_l2_program(
        &self,
        facts: &ZoneL2RuntimeFacts,
        context: &ActiveZoneContext,
        request_revision: u64,
        program_id: &str,
        report: &mut InspectionTargetResolutionReport,
    ) {
        let Ok(canonical) = canonical_hash(program_id) else {
            return;
        };
        match self
            .programs(
                facts,
                ZoneL2Request {
                    context: context.clone(),
                    request_revision,
                    query: ZoneL2ProgramsQuery {
                        exact_source_id: None,
                    },
                },
            )
            .await
        {
            Ok(read) => {
                let L2ReadOutcome::Found { value } = read.data else {
                    return;
                };
                if value
                    .programs
                    .iter()
                    .any(|program| program.hex.eq_ignore_ascii_case(&canonical))
                {
                    report.candidates.push(l2_candidate(
                        context,
                        ZoneL2EntityKind::Program,
                        canonical,
                        exact_source(&value.source.source_id, value.source.source_role),
                        Some(value.source.finality),
                    ));
                }
            }
            Err(failure) => record_warning(report, failure),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ParsedTarget {
    Zone(String),
    L1Block(BlockInput),
    L2Block(BlockInput),
    TypedBlock(BlockInput),
    Transaction(String),
    Account(String),
    Program(String),
    UnprefixedNumber(u64),
    UnprefixedHash(String),
}

impl ParsedTarget {
    fn parse(query: &str) -> Option<Self> {
        let query = query.trim();
        if query.is_empty() {
            return None;
        }
        let (prefix, target) = split_prefix(query);
        match prefix.as_deref() {
            Some("zone" | "channel") => normalized_hash(target).map(Self::Zone),
            Some("l1" | "slot") => block_input(target).map(Self::L1Block),
            Some("l2" | "lez") => block_input(target).map(Self::L2Block),
            Some("block") => block_input(target).map(Self::TypedBlock),
            Some("tx" | "transaction") => normalized_hash(target).map(Self::Transaction),
            Some("account") => canonical_account(target).map(Self::Account),
            Some("program") => normalized_hash(target).map(Self::Program),
            Some(_) => None,
            None if query.starts_with("Public/") => canonical_account(query).map(Self::Account),
            None if query.chars().all(|character| character.is_ascii_digit()) => query
                .parse::<u64>()
                .ok()
                .map(Self::UnprefixedNumber)
                .or_else(|| normalized_hash(query).map(Self::UnprefixedHash)),
            None => normalized_hash(query).map(Self::UnprefixedHash),
        }
    }

    const fn needs_l2(&self) -> bool {
        matches!(
            self,
            Self::L2Block(_)
                | Self::TypedBlock(_)
                | Self::Transaction(_)
                | Self::Account(_)
                | Self::Program(_)
                | Self::UnprefixedNumber(_)
                | Self::UnprefixedHash(_)
        )
    }

    const fn explicit_l2(&self) -> bool {
        matches!(
            self,
            Self::L2Block(_) | Self::Transaction(_) | Self::Account(_) | Self::Program(_)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BlockInput {
    Id(u64),
    Hash(String),
}

fn split_prefix(query: &str) -> (Option<String>, &str) {
    if let Some((prefix, target)) = query.split_once(':') {
        return (Some(prefix.trim().to_ascii_lowercase()), target.trim());
    }
    let Some(separator) = query.find(char::is_whitespace) else {
        return (None, query);
    };
    let prefix = query[..separator].to_ascii_lowercase();
    let target = query[separator..].trim();
    if matches!(
        prefix.as_str(),
        "zone"
            | "channel"
            | "l1"
            | "slot"
            | "l2"
            | "lez"
            | "block"
            | "tx"
            | "transaction"
            | "account"
            | "program"
    ) {
        (Some(prefix), target)
    } else {
        (None, query)
    }
}

fn block_input(value: &str) -> Option<BlockInput> {
    value
        .parse::<u64>()
        .ok()
        .map(BlockInput::Id)
        .or_else(|| normalized_hash(value).map(BlockInput::Hash))
}

fn normalized_hash(value: &str) -> Option<String> {
    canonical_hash(value.trim()).ok()
}

fn canonical_account(value: &str) -> Option<String> {
    crate::parse_account_id(value)
        .ok()
        .map(|account_id| account_id.to_string())
}

fn l1_block_candidate(
    network_scope: NetworkScope,
    block_id: u64,
    block_hash: Option<String>,
) -> InspectionTargetCandidate {
    let canonical_key = block_hash.as_ref().map_or_else(
        || format!("block:{block_id}"),
        |hash| format!("block:{block_id}:{hash}"),
    );
    InspectionTargetCandidate {
        entity_ref: InspectionEntityRef::L1 {
            network_scope,
            entity_kind: InspectionL1EntityKind::Block,
            canonical_key,
            block_id: Some(block_id),
            block_hash,
        },
        finality: None,
    }
}

fn l2_candidate(
    context: &ActiveZoneContext,
    entity_kind: ZoneL2EntityKind,
    canonical_key: String,
    source: ZoneL2SourceQualifier,
    finality: Option<L2RouteFinality>,
) -> InspectionTargetCandidate {
    InspectionTargetCandidate {
        entity_ref: InspectionEntityRef::L2 {
            entity: ZoneL2EntityRef {
                network_scope: context.network_scope.clone(),
                channel_id: context.channel_id.clone(),
                zone_kind: context.zone_kind,
                entity_kind,
                canonical_key,
                source,
            },
        },
        finality,
    }
}

fn exact_source(source_id: &str, source_role: ZoneSourceRole) -> ZoneL2SourceQualifier {
    ZoneL2SourceQualifier::Exact {
        source_id: source_id.to_owned(),
        source_role,
    }
}

const fn finality_for_role(source_role: ZoneSourceRole) -> L2RouteFinality {
    match source_role {
        ZoneSourceRole::Indexer => L2RouteFinality::Finalized,
        ZoneSourceRole::Sequencer => L2RouteFinality::Provisional,
    }
}

fn record_warning(report: &mut InspectionTargetResolutionReport, failure: L2ReadFailure) {
    let recovery = failure.code.recovery();
    report.warnings.push(InspectionTargetResolutionWarning {
        code: failure.code,
        recovery,
    });
    if report.recovery.is_none() && recovery != L2RecoveryAction::None {
        report.recovery = Some(recovery);
    }
}

fn candidate_sort_key(candidate: &InspectionTargetCandidate) -> (u8, u8, u8, String, u8, String) {
    match &candidate.entity_ref {
        InspectionEntityRef::Zone { channel_id, .. } => {
            (0, 0, 0, channel_id.clone(), 0, String::new())
        }
        InspectionEntityRef::L1 { canonical_key, .. } => {
            (1, 0, 0, canonical_key.clone(), 0, String::new())
        }
        InspectionEntityRef::L2 { entity } => {
            let entity_rank = match entity.entity_kind {
                ZoneL2EntityKind::Block => 2,
                ZoneL2EntityKind::Transaction => 3,
                ZoneL2EntityKind::Account => 4,
                ZoneL2EntityKind::Program => 5,
            };
            let finality_rank = match candidate.finality {
                Some(L2RouteFinality::Finalized) => 0,
                Some(L2RouteFinality::Provisional) => 1,
                None => 2,
            };
            let (source_rank, source_id) = match &entity.source {
                ZoneL2SourceQualifier::Exact {
                    source_id,
                    source_role,
                } => (
                    match source_role {
                        ZoneSourceRole::Indexer => 0,
                        ZoneSourceRole::Sequencer => 1,
                    },
                    source_id.clone(),
                ),
                ZoneL2SourceQualifier::Policy => (2, String::new()),
            };
            (
                2,
                entity_rank,
                finality_rank,
                entity.canonical_key.clone(),
                source_rank,
                source_id,
            )
        }
    }
}

fn finalize_report(report: &mut InspectionTargetResolutionReport) {
    report.candidates.sort_by_key(candidate_sort_key);
    report
        .candidates
        .dedup_by(|left, right| left.entity_ref == right.entity_ref);
    report.status = match report.candidates.len() {
        0 if report.recovery.is_some() => InspectionTargetResolutionStatus::Recovery,
        0 => InspectionTargetResolutionStatus::NotFound,
        1 => InspectionTargetResolutionStatus::Resolved,
        _ => InspectionTargetResolutionStatus::Ambiguous,
    };
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};

    use super::*;

    #[test]
    fn parses_supported_prefixes_and_never_guesses_accounts() {
        let channel_id = "aa".repeat(32);
        let account_id = crate::parse_account_id(&"bb".repeat(32))
            .map(|account| account.to_string())
            .unwrap_or_default();
        assert_eq!(
            ParsedTarget::parse(&format!("channel: {channel_id}")),
            Some(ParsedTarget::Zone(channel_id))
        );
        assert_eq!(
            ParsedTarget::parse("slot 42"),
            Some(ParsedTarget::L1Block(BlockInput::Id(42)))
        );
        assert!(matches!(
            ParsedTarget::parse(&format!("tx:{}", "11".repeat(32))),
            Some(ParsedTarget::Transaction(_))
        ));
        assert!(matches!(
            ParsedTarget::parse(&format!("Public/{account_id}")),
            Some(ParsedTarget::Account(_))
        ));
        assert_eq!(ParsedTarget::parse("account-id"), None);
    }

    #[test]
    fn unprefixed_numbers_and_hashes_keep_cross_layer_intent() {
        assert_eq!(
            ParsedTarget::parse("42"),
            Some(ParsedTarget::UnprefixedNumber(42))
        );
        assert!(matches!(
            ParsedTarget::parse(&"22".repeat(32)),
            Some(ParsedTarget::UnprefixedHash(_))
        ));
    }

    #[test]
    fn finalization_ranks_finalized_exact_candidates_before_provisional() -> Result<()> {
        let network_scope = NetworkScope::GenesisId {
            genesis_id: "11".repeat(32),
        };
        let exact_block =
            |source_id: &str, source_role: ZoneSourceRole, finality: L2RouteFinality| {
                InspectionTargetCandidate {
                    entity_ref: InspectionEntityRef::L2 {
                        entity: ZoneL2EntityRef {
                            network_scope: network_scope.clone(),
                            channel_id: "22".repeat(32),
                            zone_kind: crate::inspection::ZoneKind::SequencerZone,
                            entity_kind: ZoneL2EntityKind::Block,
                            canonical_key: "block:42:".to_owned() + &"aa".repeat(32),
                            source: ZoneL2SourceQualifier::Exact {
                                source_id: source_id.to_owned(),
                                source_role,
                            },
                        },
                    },
                    finality: Some(finality),
                }
            };
        let mut report = InspectionTargetResolutionReport {
            report_kind: INSPECTION_RESOLUTION_REPORT_KIND.to_owned(),
            schema_version: INSPECTION_RESOLUTION_SCHEMA_VERSION,
            query: "42".to_owned(),
            request_revision: 1,
            context_revision: Some(1),
            status: InspectionTargetResolutionStatus::NotFound,
            candidates: vec![
                exact_block(
                    "sequencer-source",
                    ZoneSourceRole::Sequencer,
                    L2RouteFinality::Provisional,
                ),
                exact_block(
                    "indexer-source",
                    ZoneSourceRole::Indexer,
                    L2RouteFinality::Finalized,
                ),
                l1_block_candidate(network_scope, 42, None),
            ],
            recovery: None,
            warnings: Vec::new(),
        };

        finalize_report(&mut report);

        let mut candidates = report.candidates.iter();
        let Some(first) = candidates.next() else {
            bail!("missing L1 candidate");
        };
        if !matches!(&first.entity_ref, InspectionEntityRef::L1 { .. }) {
            bail!("L1 candidate was not ranked first: {first:?}");
        }
        let Some(second) = candidates.next() else {
            bail!("missing finalized candidate");
        };
        let Some(third) = candidates.next() else {
            bail!("missing provisional candidate");
        };
        if second.finality != Some(L2RouteFinality::Finalized)
            || third.finality != Some(L2RouteFinality::Provisional)
        {
            bail!("finality priority was not deterministic: {report:?}");
        }
        let InspectionEntityRef::L2 { entity } = &second.entity_ref else {
            bail!("finalized result lost its L2 entity reference");
        };
        if !matches!(
            &entity.source,
            ZoneL2SourceQualifier::Exact { source_id, source_role }
                if source_id == "indexer-source" && *source_role == ZoneSourceRole::Indexer
        ) {
            bail!("finalized result lost exact Indexer provenance: {entity:?}");
        }
        if candidates.next().is_some() {
            bail!("candidate finalization changed result count");
        }
        Ok(())
    }
}
