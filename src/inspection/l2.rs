use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{CatalogVerificationState, NetworkScope, ZoneKind, ZoneSourceRole, ZoneSummary};
use crate::source_routing::channel_sources::{ChannelSourceConfig, ChannelSourceMonitorSnapshot};

mod cache;
mod model;
mod normalization;
mod resolution;
mod router;
mod source;

pub(crate) use cache::*;
pub use model::*;
pub(crate) use normalization::*;
pub(crate) use router::*;
pub(crate) use source::*;

pub mod lez {
    pub use crate::lez::*;
}

pub const L2_READ_SCHEMA_VERSION: u32 = 1;
pub const L2_READ_ERROR_REPORT_KIND: &str = "lez.read_error";

#[derive(Debug, Clone)]
pub(crate) struct ZoneL2RuntimeFacts {
    pub network_scope: Option<NetworkScope>,
    pub verification: CatalogVerificationState,
    pub summaries: BTreeMap<String, ZoneSummary>,
    pub configs: Vec<ChannelSourceConfig>,
    pub observations: ChannelSourceMonitorSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveZoneContext {
    pub network_scope: NetworkScope,
    pub channel_id: String,
    pub zone_kind: ZoneKind,
    pub selected_sequencer_source_id: Option<String>,
    pub indexer_source_id: Option<String>,
    pub source_config_revision: u64,
    pub context_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneL2Request<T> {
    pub context: ActiveZoneContext,
    pub request_revision: u64,
    pub query: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneL2BlocksQuery {
    pub cursor: Option<String>,
    pub limit: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneL2BlockDetailQuery {
    pub target: ZoneL2BlockTarget,
    pub exact_source_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ZoneL2BlockTarget {
    Id { block_id: u64 },
    Hash { block_hash: String },
    Identity { block_id: u64, block_hash: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneL2TransactionQuery {
    pub transaction_id: String,
    pub exact_source_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneL2TransactionTraceQuery {
    pub transaction_id: String,
    pub exact_source_id: Option<String>,
    pub idl_program_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneL2AccountQuery {
    pub account_id: String,
    pub snapshot: ZoneL2AccountSnapshot,
    pub exact_source_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ZoneL2AccountSnapshot {
    Finalized,
    Provisional,
    Historical { block_id: u64, block_hash: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneL2AccountActivityQuery {
    pub account_id: String,
    pub cursor: Option<String>,
    pub limit: Option<u16>,
    pub order: ZoneL2AccountActivityOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneL2AccountActivityOrder {
    OldestFirst,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneL2ProgramsQuery {
    pub exact_source_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneL2CommitmentProofQuery {
    pub commitment_hex: String,
    pub exact_source_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneL2AccountNoncesQuery {
    pub account_ids: Vec<String>,
    pub exact_source_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneL2TransfersQuery {
    pub cursor: Option<String>,
    pub block_limit: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct L2ReadReport<T> {
    pub report_kind: String,
    pub schema_version: u32,
    pub context: ActiveZoneContext,
    pub request_revision: u64,
    pub route: L2ReadRoute,
    pub route_completeness: L2RouteCompleteness,
    pub warnings: Vec<L2ReadWarning>,
    pub data: L2ReadOutcome<T>,
}

impl<T> L2ReadReport<T> {
    #[must_use]
    pub fn new<Q>(
        report_kind: impl Into<String>,
        request: ZoneL2Request<Q>,
        route: L2ReadRoute,
        route_completeness: L2RouteCompleteness,
        data: L2ReadOutcome<T>,
    ) -> Self {
        Self {
            report_kind: report_kind.into(),
            schema_version: L2_READ_SCHEMA_VERSION,
            context: request.context,
            request_revision: request.request_revision,
            route,
            route_completeness,
            warnings: Vec::new(),
            data,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum L2ReadOutcome<T> {
    Found {
        value: T,
    },
    NotFound,
    Ambiguous {
        candidates: Vec<L2ExactSourceCandidate>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2ExactSourceCandidate {
    pub source_id: String,
    pub source_role: ZoneSourceRole,
    pub canonical_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2ReadRoute {
    pub policy: L2RoutePolicy,
    pub attempts: Vec<L2RouteAttempt>,
}

impl L2ReadRoute {
    #[must_use]
    pub const fn new(policy: L2RoutePolicy) -> Self {
        Self {
            policy,
            attempts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L2RoutePolicy {
    ExactSource,
    IndexerPrimary,
    SelectedSequencer,
    ConfirmedNotFoundFallback,
    Composite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2RouteAttempt {
    pub source_id: String,
    pub source_role: ZoneSourceRole,
    pub outcome: L2RouteAttemptOutcome,
    pub contribution: L2RouteContribution,
    pub finality: Option<L2RouteFinality>,
    pub source_config_revision: u64,
    pub retrieval: L2Retrieval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L2Retrieval {
    Live,
    MemoryCache,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L2RouteAttemptOutcome {
    Returned,
    NotFound,
    Failed,
    SkippedIneligible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L2RouteContribution {
    Payload,
    FinalizedPrefix,
    ProvisionalTail,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L2RouteFinality {
    Finalized,
    Provisional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L2RouteCompleteness {
    AllConfigured,
    SingleConfigured,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2ReadWarning {
    pub code: L2ReadErrorCode,
    pub recovery: L2RecoveryAction,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneL2EntityRef {
    pub network_scope: NetworkScope,
    pub channel_id: String,
    pub zone_kind: ZoneKind,
    pub entity_kind: ZoneL2EntityKind,
    pub canonical_key: String,
    pub source: ZoneL2SourceQualifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectionResolveTargetRequest {
    pub query: String,
    pub active_zone_context: Option<ActiveZoneContext>,
    pub request_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectionTargetResolutionReport {
    pub report_kind: String,
    pub schema_version: u32,
    pub query: String,
    pub request_revision: u64,
    pub context_revision: Option<u64>,
    pub status: InspectionTargetResolutionStatus,
    pub candidates: Vec<InspectionTargetCandidate>,
    pub recovery: Option<L2RecoveryAction>,
    pub warnings: Vec<InspectionTargetResolutionWarning>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionTargetResolutionStatus {
    Resolved,
    Ambiguous,
    NotFound,
    Recovery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectionTargetCandidate {
    pub entity_ref: InspectionEntityRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finality: Option<L2RouteFinality>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "layer", rename_all = "snake_case")]
pub enum InspectionEntityRef {
    Zone {
        network_scope: NetworkScope,
        channel_id: String,
        zone_kind: ZoneKind,
    },
    L1 {
        network_scope: NetworkScope,
        entity_kind: InspectionL1EntityKind,
        canonical_key: String,
        block_id: Option<u64>,
        block_hash: Option<String>,
    },
    L2 {
        #[serde(flatten)]
        entity: ZoneL2EntityRef,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionL1EntityKind {
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectionTargetResolutionWarning {
    pub code: L2ReadErrorCode,
    pub recovery: L2RecoveryAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneL2EntityKind {
    Block,
    Transaction,
    Account,
    Program,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ZoneL2SourceQualifier {
    Policy,
    Exact {
        source_id: String,
        source_role: ZoneSourceRole,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2ReadErrorDetails {
    pub report_kind: String,
    pub schema_version: u32,
    pub context: ActiveZoneContext,
    pub request_revision: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_context_revision: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempted_route: Option<L2ReadRoute>,
    pub code: L2ReadErrorCode,
    pub recovery: L2RecoveryAction,
}

impl L2ReadErrorDetails {
    #[must_use]
    pub fn new<T>(request: &ZoneL2Request<T>, code: L2ReadErrorCode) -> Self {
        Self {
            report_kind: L2_READ_ERROR_REPORT_KIND.to_owned(),
            schema_version: L2_READ_SCHEMA_VERSION,
            context: request.context.clone(),
            request_revision: request.request_revision,
            current_context_revision: None,
            attempted_route: None,
            code,
            recovery: code.recovery(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L2ReadErrorCode {
    InvalidRequest,
    StaleContext,
    ZoneUnverified,
    L2NotApplicable,
    SourceUnconfigured,
    SourceIneligible,
    SourceUnavailable,
    SourceProtocolError,
    SourceCapabilityUnavailable,
    CursorInvalidated,
    Internal,
}

impl L2ReadErrorCode {
    #[must_use]
    pub const fn recovery(self) -> L2RecoveryAction {
        match self {
            Self::InvalidRequest | Self::L2NotApplicable | Self::SourceCapabilityUnavailable => {
                L2RecoveryAction::None
            }
            Self::StaleContext | Self::ZoneUnverified => L2RecoveryAction::RefreshContext,
            Self::SourceUnconfigured => L2RecoveryAction::ConfigureSource,
            Self::SourceIneligible => L2RecoveryAction::SelectSource,
            Self::SourceUnavailable
            | Self::SourceProtocolError
            | Self::CursorInvalidated
            | Self::Internal => L2RecoveryAction::Retry,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L2RecoveryAction {
    Retry,
    RefreshContext,
    ConfigureSource,
    SelectSource,
    None,
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};
    use serde_json::{Value, json};

    use super::*;

    #[test]
    fn read_outcomes_have_stable_tagged_shapes() -> Result<()> {
        let found = serde_json::to_value(L2ReadOutcome::Found { value: 7_u64 })?;
        let not_found = serde_json::to_value(L2ReadOutcome::<u64>::NotFound)?;
        let ambiguous = serde_json::to_value(L2ReadOutcome::<u64>::Ambiguous {
            candidates: vec![L2ExactSourceCandidate {
                source_id: source_id('a'),
                source_role: ZoneSourceRole::Sequencer,
                canonical_key: "block:7:aaaa".to_owned(),
            }],
        })?;

        if found != json!({ "outcome": "found", "value": 7 })
            || not_found != json!({ "outcome": "not_found" })
            || ambiguous.pointer("/outcome").and_then(Value::as_str) != Some("ambiguous")
            || ambiguous
                .pointer("/candidates/0/source_id")
                .and_then(Value::as_str)
                != Some(source_id('a').as_str())
        {
            bail!("unexpected L2 outcome shapes: {found}, {not_found}, {ambiguous}");
        }
        Ok(())
    }

    #[test]
    fn every_error_code_has_stable_recovery() -> Result<()> {
        let mappings = [
            (L2ReadErrorCode::InvalidRequest, L2RecoveryAction::None),
            (
                L2ReadErrorCode::StaleContext,
                L2RecoveryAction::RefreshContext,
            ),
            (
                L2ReadErrorCode::ZoneUnverified,
                L2RecoveryAction::RefreshContext,
            ),
            (L2ReadErrorCode::L2NotApplicable, L2RecoveryAction::None),
            (
                L2ReadErrorCode::SourceUnconfigured,
                L2RecoveryAction::ConfigureSource,
            ),
            (
                L2ReadErrorCode::SourceIneligible,
                L2RecoveryAction::SelectSource,
            ),
            (L2ReadErrorCode::SourceUnavailable, L2RecoveryAction::Retry),
            (
                L2ReadErrorCode::SourceProtocolError,
                L2RecoveryAction::Retry,
            ),
            (
                L2ReadErrorCode::SourceCapabilityUnavailable,
                L2RecoveryAction::None,
            ),
            (L2ReadErrorCode::CursorInvalidated, L2RecoveryAction::Retry),
            (L2ReadErrorCode::Internal, L2RecoveryAction::Retry),
        ];
        for (code, expected) in mappings {
            if code.recovery() != expected {
                bail!("unexpected recovery for {code:?}");
            }
        }
        Ok(())
    }

    #[test]
    fn error_details_expose_only_stable_route_identity() -> Result<()> {
        let request = request(ZoneL2ProgramsQuery {
            exact_source_id: Some(source_id('b')),
        });
        let mut details = L2ReadErrorDetails::new(&request, L2ReadErrorCode::SourceUnavailable);
        details.attempted_route = Some(L2ReadRoute {
            policy: L2RoutePolicy::SelectedSequencer,
            attempts: vec![L2RouteAttempt {
                source_id: source_id('b'),
                source_role: ZoneSourceRole::Sequencer,
                outcome: L2RouteAttemptOutcome::Failed,
                contribution: L2RouteContribution::None,
                finality: Some(L2RouteFinality::Provisional),
                source_config_revision: 1,
                retrieval: L2Retrieval::Live,
            }],
        });
        let value = serde_json::to_value(details)?;
        let serialized = value.to_string();

        if value.pointer("/code").and_then(Value::as_str) != Some("source_unavailable")
            || value.pointer("/recovery").and_then(Value::as_str) != Some("retry")
            || serialized.contains("endpoint")
            || serialized.contains("credential")
            || serialized.contains("module_id")
        {
            bail!("unsafe or malformed L2 error details: {value}");
        }
        Ok(())
    }

    #[test]
    fn read_report_and_entity_reference_keep_typed_provenance_without_targets() -> Result<()> {
        let request = request(ZoneL2ProgramsQuery {
            exact_source_id: Some(source_id('a')),
        });
        let mut report = L2ReadReport::new(
            "lez.programs",
            request,
            L2ReadRoute::new(L2RoutePolicy::SelectedSequencer),
            L2RouteCompleteness::SingleConfigured,
            L2ReadOutcome::Found {
                value: vec!["program-a".to_owned()],
            },
        );
        report.warnings.push(L2ReadWarning {
            code: L2ReadErrorCode::SourceUnavailable,
            recovery: L2RecoveryAction::Retry,
            message: "secondary source unavailable".to_owned(),
        });
        let entity = ZoneL2EntityRef {
            network_scope: NetworkScope::GenesisId {
                genesis_id: "11".repeat(32),
            },
            channel_id: "22".repeat(32),
            zone_kind: ZoneKind::SequencerZone,
            entity_kind: ZoneL2EntityKind::Program,
            canonical_key: "program-a".to_owned(),
            source: ZoneL2SourceQualifier::Exact {
                source_id: source_id('a'),
                source_role: ZoneSourceRole::Sequencer,
            },
        };
        let report_value = serde_json::to_value(report)?;
        let entity_value = serde_json::to_value(entity)?;
        let serialized = format!("{report_value}{entity_value}");
        let serialized_entity = entity_value.to_string();

        if report_value
            .get("route_completeness")
            .and_then(Value::as_str)
            != Some("single_configured")
            || report_value
                .pointer("/warnings/0/code")
                .and_then(Value::as_str)
                != Some("source_unavailable")
            || entity_value.pointer("/source/kind").and_then(Value::as_str) != Some("exact")
            || serialized.contains("endpoint")
            || serialized.contains("module_id")
            || serialized_entity.contains("context_revision")
        {
            bail!("unsafe or malformed L2 report/entity reference: {serialized}");
        }
        Ok(())
    }

    fn request<T>(query: T) -> ZoneL2Request<T> {
        ZoneL2Request {
            context: ActiveZoneContext {
                network_scope: NetworkScope::GenesisId {
                    genesis_id: "11".repeat(32),
                },
                channel_id: "22".repeat(32),
                zone_kind: ZoneKind::SequencerZone,
                selected_sequencer_source_id: Some(source_id('a')),
                indexer_source_id: None,
                source_config_revision: 1,
                context_revision: 4,
            },
            request_revision: 9,
            query,
        }
    }

    fn source_id(character: char) -> String {
        format!("src_{}", character.to_string().repeat(32))
    }
}
