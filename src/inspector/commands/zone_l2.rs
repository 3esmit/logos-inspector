use std::sync::Arc;

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::runtime::Runtime;

use super::{decode_object_request, zone_catalog::ZoneCatalogCommandInterface};
use crate::{
    inspection::{
        CatalogVerificationState, ZoneKind, ZoneSourceRole,
        l2::{
            L2ReadErrorCode, L2ReadErrorDetails, L2ReadRoute, L2RoutePolicy,
            ZoneL2AccountActivityQuery, ZoneL2AccountNoncesQuery, ZoneL2AccountQuery,
            ZoneL2AccountSnapshot, ZoneL2BlockDetailQuery, ZoneL2BlocksQuery,
            ZoneL2CommitmentProofQuery, ZoneL2ProgramsQuery, ZoneL2Request, ZoneL2TransactionQuery,
            ZoneL2TransactionTraceQuery, ZoneL2TransfersQuery,
        },
    },
    source_routing::channel_sources::{
        ChannelSourceBindingState, ChannelSourceHealthState, ChannelSourceRole,
        PersistedSequencerAttestation,
    },
    support::bridge_envelope::structured_bridge_error,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ZoneL2Command {
    Blocks,
    BlockDetail,
    Transaction,
    TransactionTrace,
    Account,
    AccountActivity,
    Programs,
    CommitmentProof,
    AccountNonces,
    Transfers,
}

const COMMANDS: [(&str, ZoneL2Command); 10] = [
    ("zoneL2Blocks", ZoneL2Command::Blocks),
    ("zoneL2BlockDetail", ZoneL2Command::BlockDetail),
    ("zoneL2Transaction", ZoneL2Command::Transaction),
    ("zoneL2TransactionTrace", ZoneL2Command::TransactionTrace),
    ("zoneL2Account", ZoneL2Command::Account),
    ("zoneL2AccountActivity", ZoneL2Command::AccountActivity),
    ("zoneL2Programs", ZoneL2Command::Programs),
    ("zoneL2CommitmentProof", ZoneL2Command::CommitmentProof),
    ("zoneL2AccountNonces", ZoneL2Command::AccountNonces),
    ("zoneL2Transfers", ZoneL2Command::Transfers),
];

pub(crate) fn zone_l2_command(method: &str) -> Option<ZoneL2Command> {
    COMMANDS
        .iter()
        .find_map(|(name, command)| (*name == method).then_some(*command))
}

#[cfg(test)]
pub(crate) fn zone_l2_command_names() -> impl Iterator<Item = &'static str> {
    COMMANDS.iter().map(|(name, _)| *name)
}

pub(crate) struct ZoneL2CommandInterface {
    catalog: Arc<ZoneCatalogCommandInterface>,
}

impl ZoneL2CommandInterface {
    #[must_use]
    pub(crate) fn new(catalog: Arc<ZoneCatalogCommandInterface>) -> Self {
        Self { catalog }
    }

    pub(crate) fn bridge_call(
        &self,
        runtime: &Runtime,
        command: ZoneL2Command,
        args: &Value,
    ) -> Result<Value> {
        match command {
            ZoneL2Command::Blocks => {
                self.validate_request::<ZoneL2BlocksQuery>(runtime, args, "zoneL2Blocks")
            }
            ZoneL2Command::BlockDetail => {
                self.validate_request::<ZoneL2BlockDetailQuery>(runtime, args, "zoneL2BlockDetail")
            }
            ZoneL2Command::Transaction => {
                self.validate_request::<ZoneL2TransactionQuery>(runtime, args, "zoneL2Transaction")
            }
            ZoneL2Command::TransactionTrace => self
                .validate_request::<ZoneL2TransactionTraceQuery>(
                    runtime,
                    args,
                    "zoneL2TransactionTrace",
                ),
            ZoneL2Command::Account => {
                self.validate_request::<ZoneL2AccountQuery>(runtime, args, "zoneL2Account")
            }
            ZoneL2Command::AccountActivity => self.validate_request::<ZoneL2AccountActivityQuery>(
                runtime,
                args,
                "zoneL2AccountActivity",
            ),
            ZoneL2Command::Programs => {
                self.validate_request::<ZoneL2ProgramsQuery>(runtime, args, "zoneL2Programs")
            }
            ZoneL2Command::CommitmentProof => self.validate_request::<ZoneL2CommitmentProofQuery>(
                runtime,
                args,
                "zoneL2CommitmentProof",
            ),
            ZoneL2Command::AccountNonces => self.validate_request::<ZoneL2AccountNoncesQuery>(
                runtime,
                args,
                "zoneL2AccountNonces",
            ),
            ZoneL2Command::Transfers => {
                self.validate_request::<ZoneL2TransfersQuery>(runtime, args, "zoneL2Transfers")
            }
        }
    }

    fn validate_request<T>(&self, runtime: &Runtime, args: &Value, command: &str) -> Result<Value>
    where
        T: DeserializeOwned + ZoneL2QueryContract,
    {
        let request: ZoneL2Request<T> = decode_object_request(args, command)?;
        let validation = self.validate(runtime, &request);
        let route = match validation {
            Ok(route) => route,
            Err(failure) => return structured_failure(&request, failure),
        };
        structured_failure(
            &request,
            ValidationFailure {
                code: L2ReadErrorCode::SourceCapabilityUnavailable,
                message: "Zone L2 source adapter is not available for this command yet".to_owned(),
                route: Some(route),
            },
        )
    }

    fn validate<T>(
        &self,
        runtime: &Runtime,
        request: &ZoneL2Request<T>,
    ) -> Result<L2ReadRoute, ValidationFailure>
    where
        T: ZoneL2QueryContract,
    {
        let facts = self
            .catalog
            .context_snapshot(runtime)
            .map_err(|_| ValidationFailure::new(L2ReadErrorCode::Internal, "Zone state failed"))?;
        validate_against_facts(request, &facts)
    }
}

fn validate_against_facts<T>(
    request: &ZoneL2Request<T>,
    facts: &super::zone_catalog::ZoneContextSnapshot,
) -> Result<L2ReadRoute, ValidationFailure>
where
    T: ZoneL2QueryContract,
{
    validate_request_identity(request)?;
    if facts.verification != CatalogVerificationState::Verified {
        return Err(ValidationFailure::new(
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
        return Err(ValidationFailure::new(
            L2ReadErrorCode::L2NotApplicable,
            "Active Zone has no Sequencer-backed L2",
        ));
    }
    if request.context.zone_kind != summary.kind() {
        return Err(stale_context("Active Zone kind is stale"));
    }
    let config = facts.configs.iter().find(|config| {
        config.network_scope == request.context.network_scope
            && config.channel_id == request.context.channel_id
    });
    let current_config_revision = config.map_or(0, |config| config.config_revision);
    if request.context.source_config_revision != current_config_revision {
        return Err(stale_context("Active Zone source configuration is stale"));
    }
    let expected_sequencer =
        config.and_then(|config| config.selected_sequencer_source_id.as_deref());
    let expected_indexer = config.and_then(|config| {
        config
            .indexer_source
            .as_ref()
            .map(|source| source.source_id.as_str())
    });
    if request.context.selected_sequencer_source_id.as_deref() != expected_sequencer
        || request.context.indexer_source_id.as_deref() != expected_indexer
    {
        return Err(stale_context("Active Zone source selection is stale"));
    }

    let policy = request.query.route_policy();
    if let Some(source_id) = request.query.exact_source_id() {
        let role = configured_source_role(config, source_id).ok_or_else(|| {
            ValidationFailure::new(
                L2ReadErrorCode::SourceIneligible,
                "Exact source does not belong to the active Channel",
            )
        })?;
        validate_source_eligibility(config, &facts.observations, source_id, role)?;
        return Ok(L2ReadRoute::new(L2RoutePolicy::ExactSource));
    }

    match request.query.source_requirement() {
        SourceRequirement::Any => {
            let mut configured = false;
            if let Some(source_id) = expected_indexer {
                configured = true;
                validate_source_eligibility(
                    config,
                    &facts.observations,
                    source_id,
                    ZoneSourceRole::Indexer,
                )?;
            } else if let Some(source_id) = expected_sequencer {
                configured = true;
                validate_source_eligibility(
                    config,
                    &facts.observations,
                    source_id,
                    ZoneSourceRole::Sequencer,
                )?;
            }
            if !configured {
                return Err(source_unconfigured());
            }
        }
        SourceRequirement::SelectedSequencer => {
            let source_id = expected_sequencer.ok_or_else(source_unconfigured)?;
            validate_source_eligibility(
                config,
                &facts.observations,
                source_id,
                ZoneSourceRole::Sequencer,
            )?;
        }
        SourceRequirement::Indexer => {
            let source_id = expected_indexer.ok_or_else(source_unconfigured)?;
            validate_source_eligibility(
                config,
                &facts.observations,
                source_id,
                ZoneSourceRole::Indexer,
            )?;
        }
    }
    Ok(L2ReadRoute::new(policy))
}

trait ZoneL2QueryContract {
    fn exact_source_id(&self) -> Option<&str> {
        None
    }

    fn source_requirement(&self) -> SourceRequirement;

    fn route_policy(&self) -> L2RoutePolicy;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceRequirement {
    Any,
    SelectedSequencer,
    Indexer,
}

impl ZoneL2QueryContract for ZoneL2BlocksQuery {
    fn source_requirement(&self) -> SourceRequirement {
        SourceRequirement::Any
    }

    fn route_policy(&self) -> L2RoutePolicy {
        L2RoutePolicy::Composite
    }
}

impl ZoneL2QueryContract for ZoneL2BlockDetailQuery {
    fn exact_source_id(&self) -> Option<&str> {
        self.exact_source_id.as_deref()
    }

    fn source_requirement(&self) -> SourceRequirement {
        SourceRequirement::Any
    }

    fn route_policy(&self) -> L2RoutePolicy {
        L2RoutePolicy::IndexerPrimary
    }
}

impl ZoneL2QueryContract for ZoneL2TransactionQuery {
    fn exact_source_id(&self) -> Option<&str> {
        self.exact_source_id.as_deref()
    }

    fn source_requirement(&self) -> SourceRequirement {
        SourceRequirement::Any
    }

    fn route_policy(&self) -> L2RoutePolicy {
        L2RoutePolicy::IndexerPrimary
    }
}

impl ZoneL2QueryContract for ZoneL2TransactionTraceQuery {
    fn exact_source_id(&self) -> Option<&str> {
        self.exact_source_id.as_deref()
    }

    fn source_requirement(&self) -> SourceRequirement {
        SourceRequirement::Any
    }

    fn route_policy(&self) -> L2RoutePolicy {
        L2RoutePolicy::IndexerPrimary
    }
}

impl ZoneL2QueryContract for ZoneL2AccountQuery {
    fn exact_source_id(&self) -> Option<&str> {
        self.exact_source_id.as_deref()
    }

    fn source_requirement(&self) -> SourceRequirement {
        match &self.snapshot {
            ZoneL2AccountSnapshot::Finalized | ZoneL2AccountSnapshot::Historical { .. } => {
                SourceRequirement::Indexer
            }
            ZoneL2AccountSnapshot::Provisional => SourceRequirement::SelectedSequencer,
        }
    }

    fn route_policy(&self) -> L2RoutePolicy {
        match &self.snapshot {
            ZoneL2AccountSnapshot::Finalized | ZoneL2AccountSnapshot::Historical { .. } => {
                L2RoutePolicy::IndexerPrimary
            }
            ZoneL2AccountSnapshot::Provisional => L2RoutePolicy::SelectedSequencer,
        }
    }
}

impl ZoneL2QueryContract for ZoneL2AccountActivityQuery {
    fn source_requirement(&self) -> SourceRequirement {
        SourceRequirement::Indexer
    }

    fn route_policy(&self) -> L2RoutePolicy {
        L2RoutePolicy::IndexerPrimary
    }
}

impl ZoneL2QueryContract for ZoneL2ProgramsQuery {
    fn exact_source_id(&self) -> Option<&str> {
        self.exact_source_id.as_deref()
    }

    fn source_requirement(&self) -> SourceRequirement {
        SourceRequirement::SelectedSequencer
    }

    fn route_policy(&self) -> L2RoutePolicy {
        L2RoutePolicy::SelectedSequencer
    }
}

impl ZoneL2QueryContract for ZoneL2CommitmentProofQuery {
    fn exact_source_id(&self) -> Option<&str> {
        self.exact_source_id.as_deref()
    }

    fn source_requirement(&self) -> SourceRequirement {
        SourceRequirement::SelectedSequencer
    }

    fn route_policy(&self) -> L2RoutePolicy {
        L2RoutePolicy::SelectedSequencer
    }
}

impl ZoneL2QueryContract for ZoneL2AccountNoncesQuery {
    fn exact_source_id(&self) -> Option<&str> {
        self.exact_source_id.as_deref()
    }

    fn source_requirement(&self) -> SourceRequirement {
        SourceRequirement::SelectedSequencer
    }

    fn route_policy(&self) -> L2RoutePolicy {
        L2RoutePolicy::SelectedSequencer
    }
}

impl ZoneL2QueryContract for ZoneL2TransfersQuery {
    fn source_requirement(&self) -> SourceRequirement {
        SourceRequirement::Indexer
    }

    fn route_policy(&self) -> L2RoutePolicy {
        L2RoutePolicy::IndexerPrimary
    }
}

#[derive(Debug)]
struct ValidationFailure {
    code: L2ReadErrorCode,
    message: String,
    route: Option<L2ReadRoute>,
}

impl ValidationFailure {
    fn new(code: L2ReadErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            route: None,
        }
    }
}

fn validate_request_identity<T>(request: &ZoneL2Request<T>) -> Result<(), ValidationFailure> {
    if request.context.context_revision == 0 || request.request_revision == 0 {
        return Err(ValidationFailure::new(
            L2ReadErrorCode::InvalidRequest,
            "Zone L2 revisions must be positive",
        ));
    }
    if !is_hex_identity(&request.context.channel_id) {
        return Err(ValidationFailure::new(
            L2ReadErrorCode::InvalidRequest,
            "Active Zone Channel id is invalid",
        ));
    }
    Ok(())
}

fn configured_source_role(
    config: Option<&crate::source_routing::channel_sources::ChannelSourceConfig>,
    source_id: &str,
) -> Option<ZoneSourceRole> {
    let config = config?;
    if config
        .sequencer_sources
        .iter()
        .any(|source| source.source_id == source_id)
    {
        return Some(ZoneSourceRole::Sequencer);
    }
    config
        .indexer_source
        .as_ref()
        .filter(|source| source.source_id == source_id)
        .map(|_| ZoneSourceRole::Indexer)
}

fn validate_source_eligibility(
    config: Option<&crate::source_routing::channel_sources::ChannelSourceConfig>,
    observations: &crate::source_routing::channel_sources::ChannelSourceMonitorSnapshot,
    source_id: &str,
    role: ZoneSourceRole,
) -> Result<(), ValidationFailure> {
    let config = config.ok_or_else(source_unconfigured)?;
    let observed_role = match role {
        ZoneSourceRole::Sequencer => ChannelSourceRole::Sequencer,
        ZoneSourceRole::Indexer => ChannelSourceRole::Indexer,
    };
    let observation = observations
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
            return Err(ValidationFailure::new(
                L2ReadErrorCode::SourceIneligible,
                "Sequencer source has no matching Channel attestation",
            ));
        }
    }
    match observation.map(|observation| observation.health) {
        Some(ChannelSourceHealthState::ChannelMismatch) => Err(ValidationFailure::new(
            L2ReadErrorCode::SourceIneligible,
            "Source reports another Channel",
        )),
        Some(ChannelSourceHealthState::Pending)
        | Some(ChannelSourceHealthState::Reachable)
        | Some(ChannelSourceHealthState::Degraded)
        | Some(ChannelSourceHealthState::Unreachable)
        | Some(ChannelSourceHealthState::Incomplete)
        | Some(ChannelSourceHealthState::Unsupported)
        | None => Ok(()),
    }
}

fn structured_failure<T>(request: &ZoneL2Request<T>, failure: ValidationFailure) -> Result<Value> {
    let mut details = L2ReadErrorDetails::new(request, failure.code);
    details.attempted_route = failure.route;
    Err(structured_bridge_error(failure.message, details)?)
}

fn stale_context(message: impl Into<String>) -> ValidationFailure {
    ValidationFailure::new(L2ReadErrorCode::StaleContext, message)
}

fn source_unconfigured() -> ValidationFailure {
    ValidationFailure::new(
        L2ReadErrorCode::SourceUnconfigured,
        "Active Zone has no source configured for this read",
    )
}

fn is_hex_identity(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|character| character.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use anyhow::{Context as _, Result, bail};
    use serde_json::{Value, json};

    use super::*;
    use crate::{
        inspection::{
            CatalogCoverageStatus, CoveragePrefixStatus, NetworkScope, RawActivitySummary,
            ZoneFacts,
            catalog::{
                CatalogFrontier, CatalogIdentityAssurance, CatalogMetadata, CatalogSnapshot,
            },
            project_catalog_zones_with_sources,
        },
        source_routing::channel_sources::{
            ChannelSourceConfig, ChannelSourceHealthState, ChannelSourceObservation,
            ChannelSourceObservationSet, ChannelSourceTarget, ConfiguredSequencerSource,
            PersistedSequencerAttestation,
        },
        support::bridge_envelope::bridge_response_json,
    };

    #[test]
    fn l2_request_decoder_rejects_connection_targets() -> Result<()> {
        let args = json!([{
            "context": {
                "network_scope": { "kind": "genesis_id", "genesis_id": "11".repeat(32) },
                "channel_id": "22".repeat(32),
                "zone_kind": "sequencer_zone",
                "selected_sequencer_source_id": null,
                "indexer_source_id": null,
                "source_config_revision": 0,
                "context_revision": 1
            },
            "request_revision": 1,
            "query": {
                "exact_source_id": null,
                "endpoint": "https://forbidden.example"
            }
        }]);
        if decode_object_request::<ZoneL2Request<ZoneL2ProgramsQuery>>(&args, "zoneL2Programs")
            .is_ok()
        {
            bail!("Zone L2 request accepted an endpoint");
        }
        Ok(())
    }

    #[test]
    fn validation_gates_unverified_stale_and_data_channel_contexts() -> Result<()> {
        let (mut facts, mut request) = facts_and_request(None);

        facts.verification = CatalogVerificationState::CachedUnverified;
        require_code(
            validate_against_facts(&request, &facts),
            L2ReadErrorCode::ZoneUnverified,
        )?;

        facts.verification = CatalogVerificationState::Verified;
        request.context.source_config_revision = 99;
        require_code(
            validate_against_facts(&request, &facts),
            L2ReadErrorCode::StaleContext,
        )?;

        request.context.source_config_revision = 1;
        let summary = facts
            .summaries
            .get_mut(&request.context.channel_id)
            .context("test Zone summary is missing")?;
        summary.facts = ZoneFacts::DataChannel {
            raw_activity: RawActivitySummary {
                inscription_count: 1,
                latest_slot: Some(7),
                latest_payload_size: None,
                finality_state: crate::inspection::L1FinalityState::Final,
            },
        };
        require_code(
            validate_against_facts(&request, &facts),
            L2ReadErrorCode::L2NotApplicable,
        )?;
        Ok(())
    }

    #[test]
    fn exact_source_cannot_cross_channels_and_error_is_sanitized() -> Result<()> {
        let foreign_source = source_id('d');
        let (facts, request) = facts_and_request(Some(foreign_source));
        let Err(failure) = validate_against_facts(&request, &facts) else {
            bail!("foreign Channel source was accepted");
        };
        if failure.code != L2ReadErrorCode::SourceIneligible {
            bail!("unexpected cross-Channel failure: {failure:?}");
        }
        let response: Value =
            serde_json::from_str(&bridge_response_json(structured_failure(&request, failure)))?;
        let serialized = response.to_string();
        if response
            .pointer("/error_details/code")
            .and_then(Value::as_str)
            != Some("source_ineligible")
            || response
                .pointer("/error_details/recovery")
                .and_then(Value::as_str)
                != Some("select_source")
            || response
                .pointer("/error_details/context/channel_id")
                .and_then(Value::as_str)
                != Some(identity('a').as_str())
            || serialized.contains("sequencer.example")
            || serialized.contains("endpoint")
            || serialized.contains("module_id")
        {
            bail!("unsafe or malformed cross-Channel error: {response}");
        }
        Ok(())
    }

    #[test]
    fn stale_context_failure_echoes_request_fences() -> Result<()> {
        let (facts, mut request) = facts_and_request(None);
        request.context.source_config_revision = 8;
        request.context.context_revision = 12;
        request.request_revision = 21;
        let Err(failure) = validate_against_facts(&request, &facts) else {
            bail!("stale context unexpectedly validated");
        };
        let response: Value =
            serde_json::from_str(&bridge_response_json(structured_failure(&request, failure)))?;
        if response
            .pointer("/error_details/code")
            .and_then(Value::as_str)
            != Some("stale_context")
            || response
                .pointer("/error_details/recovery")
                .and_then(Value::as_str)
                != Some("refresh_context")
            || response
                .pointer("/error_details/context/context_revision")
                .and_then(Value::as_u64)
                != Some(12)
            || response
                .pointer("/error_details/request_revision")
                .and_then(Value::as_u64)
                != Some(21)
        {
            bail!("stale context error lost request fences: {response}");
        }
        Ok(())
    }

    #[test]
    fn valid_context_selects_policy_without_legacy_fallback() -> Result<()> {
        let (facts, request) = facts_and_request(None);
        let route = validate_against_facts(&request, &facts)
            .map_err(|failure| anyhow::anyhow!(failure.message))?;
        if route.policy != L2RoutePolicy::SelectedSequencer || !route.attempts.is_empty() {
            bail!("unexpected validated route: {route:?}");
        }
        Ok(())
    }

    #[test]
    fn runtime_attestation_makes_pending_source_session_eligible() -> Result<()> {
        let (mut facts, request) = facts_and_request(None);
        let config = facts
            .configs
            .iter_mut()
            .find(|config| config.channel_id == request.context.channel_id)
            .context("test config is missing")?;
        let source = config
            .sequencer_sources
            .first_mut()
            .context("test Sequencer source is missing")?;
        source.channel_attestation = PersistedSequencerAttestation::Pending;
        let source_id = source.source_id.clone();
        facts.observations = crate::source_routing::channel_sources::ChannelSourceMonitorSnapshot {
            network_scope: Some(request.context.network_scope.clone()),
            catalog_verified: true,
            observation_revision: 1,
            channels: vec![ChannelSourceObservationSet {
                channel_id: request.context.channel_id.clone(),
                config_revision: request.context.source_config_revision,
                selected_sequencer_source_id: Some(source_id.clone()),
                observations: vec![ChannelSourceObservation {
                    source_id,
                    role: ChannelSourceRole::Sequencer,
                    selected: true,
                    binding_state: Some(ChannelSourceBindingState::RuntimeAttested),
                    health: ChannelSourceHealthState::Reachable,
                    last_good: None,
                    current_failure: None,
                    comparison_blocks: Vec::new(),
                }],
            }],
        };

        let route = validate_against_facts(&request, &facts)
            .map_err(|failure| anyhow::anyhow!(failure.message))?;
        if route.policy != L2RoutePolicy::SelectedSequencer {
            bail!("runtime-attested source selected wrong policy: {route:?}");
        }
        let observation = facts
            .observations
            .channels
            .first_mut()
            .and_then(|set| set.observations.first_mut())
            .context("test runtime observation is missing")?;
        observation.binding_state = Some(ChannelSourceBindingState::Pending);
        require_code(
            validate_against_facts(&request, &facts),
            L2ReadErrorCode::SourceIneligible,
        )?;
        Ok(())
    }

    fn facts_and_request(
        exact_source_id: Option<String>,
    ) -> (
        super::super::zone_catalog::ZoneContextSnapshot,
        ZoneL2Request<ZoneL2ProgramsQuery>,
    ) {
        let scope = NetworkScope::GenesisId {
            genesis_id: identity('1'),
        };
        let channel_a = identity('a');
        let channel_b = identity('b');
        let config_a = config(&scope, &channel_a, 'c');
        let config_b = config(&scope, &channel_b, 'd');
        let observations =
            crate::source_routing::channel_sources::ChannelSourceMonitorSnapshot::default();
        let catalog = snapshot(scope.clone());
        let summaries = project_catalog_zones_with_sources(
            &catalog,
            &[config_a.clone(), config_b.clone()],
            &observations,
            CatalogVerificationState::Verified,
        )
        .into_iter()
        .map(|summary| (summary.channel_id.clone(), summary))
        .collect::<BTreeMap<_, _>>();
        let request = ZoneL2Request {
            context: crate::inspection::l2::ActiveZoneContext {
                network_scope: scope.clone(),
                channel_id: channel_a,
                zone_kind: ZoneKind::SequencerZone,
                selected_sequencer_source_id: config_a.selected_sequencer_source_id.clone(),
                indexer_source_id: None,
                source_config_revision: 1,
                context_revision: 1,
            },
            request_revision: 1,
            query: ZoneL2ProgramsQuery { exact_source_id },
        };
        (
            super::super::zone_catalog::ZoneContextSnapshot {
                network_scope: Some(scope),
                verification: CatalogVerificationState::Verified,
                summaries,
                configs: vec![config_a, config_b],
                observations,
            },
            request,
        )
    }

    fn require_code(
        result: Result<L2ReadRoute, ValidationFailure>,
        expected: L2ReadErrorCode,
    ) -> Result<()> {
        let Err(failure) = result else {
            bail!("validation unexpectedly succeeded");
        };
        if failure.code != expected {
            bail!("expected {expected:?}, got {failure:?}");
        }
        Ok(())
    }

    fn snapshot(scope: NetworkScope) -> CatalogSnapshot {
        CatalogSnapshot {
            metadata: CatalogMetadata {
                catalog_file_id: "catalog_l2_test".to_owned(),
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

    fn config(
        scope: &NetworkScope,
        channel_id: &str,
        source_character: char,
    ) -> ChannelSourceConfig {
        let target = ChannelSourceTarget::Rpc {
            endpoint: "https://sequencer.example/".to_owned(),
        };
        let source = ConfiguredSequencerSource {
            source_id: source_id(source_character),
            label: None,
            channel_attestation: PersistedSequencerAttestation::PersistedAttested {
                channel_id: channel_id.to_owned(),
                target_fingerprint: target.fingerprint(),
                attested_at_unix: 1,
            },
            target,
        };
        ChannelSourceConfig {
            network_scope: scope.clone(),
            channel_id: channel_id.to_owned(),
            config_revision: 1,
            selected_sequencer_source_id: Some(source.source_id.clone()),
            sequencer_sources: vec![source],
            indexer_source: None,
        }
    }

    fn identity(character: char) -> String {
        character.to_string().repeat(64)
    }

    fn source_id(character: char) -> String {
        format!("src_{}", character.to_string().repeat(32))
    }
}
