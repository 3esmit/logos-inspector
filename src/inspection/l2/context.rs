use crate::{
    inspection::{CatalogVerificationState, ZoneKind, ZoneSourceRole},
    source_routing::channel_sources::{
        ChannelSourceBindingState, ChannelSourceConfig, ChannelSourceHealthState,
        ChannelSourceRole, ChannelSourceTarget,
    },
};

use super::{
    ActiveZoneContext, L2_READ_SCHEMA_VERSION, L2CacheScope, L2ReadErrorCode, L2SourceDescriptor,
    ZoneL2RuntimeFacts,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveZoneContextError {
    pub code: L2ReadErrorCode,
    pub message: String,
}

impl ActiveZoneContextError {
    fn new(code: L2ReadErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn stale(message: impl Into<String>) -> Self {
        Self::new(L2ReadErrorCode::StaleContext, message)
    }

    fn source_ineligible(message: impl Into<String>) -> Self {
        Self::new(L2ReadErrorCode::SourceIneligible, message)
    }
}

#[derive(Debug, Clone)]
struct ResolvedSource {
    descriptor: L2SourceDescriptor,
    eligibility: Result<(), ActiveZoneContextError>,
}

#[derive(Debug, Clone)]
pub(crate) enum ResolvedSourcePlan {
    Absent,
    Eligible(L2SourceDescriptor),
    Ineligible(L2SourceDescriptor, ActiveZoneContextError),
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedActiveZoneContext {
    config: ChannelSourceConfig,
    sequencers: Vec<ResolvedSource>,
    indexer: Option<ResolvedSource>,
}

impl ResolvedActiveZoneContext {
    pub(crate) fn resolve(
        facts: &ZoneL2RuntimeFacts,
        context: &ActiveZoneContext,
        request_revision: u64,
    ) -> Result<Self, ActiveZoneContextError> {
        validate_context_identity(facts, context, request_revision)?;
        let config = facts
            .configs
            .iter()
            .find(|config| {
                config.network_scope == context.network_scope
                    && config.channel_id == context.channel_id
            })
            .cloned()
            .unwrap_or_else(|| empty_config(context));
        if context.source_config_revision != config.config_revision
            || context.selected_sequencer_source_id != config.selected_sequencer_source_id
            || context.indexer_source_id
                != config
                    .indexer_source
                    .as_ref()
                    .map(|source| source.source_id.clone())
        {
            return Err(ActiveZoneContextError::stale(
                "Active Zone source configuration is stale",
            ));
        }
        Ok(Self::from_config(facts, config))
    }

    fn from_config(facts: &ZoneL2RuntimeFacts, config: ChannelSourceConfig) -> Self {
        let sequencers = config
            .sequencer_sources
            .iter()
            .map(|source| ResolvedSource {
                descriptor: descriptor(
                    &config,
                    &source.source_id,
                    ZoneSourceRole::Sequencer,
                    source.target.clone(),
                ),
                eligibility: source_eligibility(
                    facts,
                    &config,
                    &source.source_id,
                    ZoneSourceRole::Sequencer,
                ),
            })
            .collect();
        let indexer = config.indexer_source.as_ref().map(|source| ResolvedSource {
            descriptor: descriptor(
                &config,
                &source.source_id,
                ZoneSourceRole::Indexer,
                source.target.clone(),
            ),
            eligibility: source_eligibility(
                facts,
                &config,
                &source.source_id,
                ZoneSourceRole::Indexer,
            ),
        });
        Self {
            config,
            sequencers,
            indexer,
        }
    }

    pub(crate) fn source(
        &self,
        source_id: &str,
        required_role: Option<ZoneSourceRole>,
    ) -> Result<L2SourceDescriptor, ActiveZoneContextError> {
        let source = self.find_source(source_id).ok_or_else(|| {
            ActiveZoneContextError::source_ineligible(
                "Source does not belong to the active Channel",
            )
        })?;
        if required_role.is_some_and(|role| source.descriptor.role != role) {
            return Err(ActiveZoneContextError::source_ineligible(
                "Source role cannot serve this L2 read",
            ));
        }
        source.eligibility.clone()?;
        Ok(source.descriptor.clone())
    }

    pub(crate) fn selected_source(
        &self,
        role: ZoneSourceRole,
    ) -> Result<Option<L2SourceDescriptor>, ActiveZoneContextError> {
        let source_id = self.selected_source_id(role);
        source_id
            .map(|source_id| self.source(source_id, Some(role)))
            .transpose()
    }

    pub(crate) fn source_plan(&self, role: ZoneSourceRole) -> ResolvedSourcePlan {
        let Some(source_id) = self.selected_source_id(role) else {
            return ResolvedSourcePlan::Absent;
        };
        let Some(source) = self.find_source(source_id) else {
            return ResolvedSourcePlan::Absent;
        };
        match &source.eligibility {
            Ok(()) => ResolvedSourcePlan::Eligible(source.descriptor.clone()),
            Err(error) => ResolvedSourcePlan::Ineligible(source.descriptor.clone(), error.clone()),
        }
    }

    pub(crate) fn valid_cache_scopes(facts: &ZoneL2RuntimeFacts) -> Vec<L2CacheScope> {
        if facts.verification != CatalogVerificationState::Verified {
            return Vec::new();
        }
        facts
            .configs
            .iter()
            .flat_map(|config| {
                let resolved = Self::from_config(facts, config.clone());
                resolved
                    .sequencers
                    .into_iter()
                    .chain(resolved.indexer)
                    .filter_map(|source| {
                        source.eligibility.ok()?;
                        Some(L2CacheScope {
                            schema_version: L2_READ_SCHEMA_VERSION,
                            network_scope: config.network_scope.clone(),
                            channel_id: config.channel_id.clone(),
                            source_id: source.descriptor.source_id,
                            source_config_revision: config.config_revision,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn selected_source_id(&self, role: ZoneSourceRole) -> Option<&str> {
        match role {
            ZoneSourceRole::Sequencer => self.config.selected_sequencer_source_id.as_deref(),
            ZoneSourceRole::Indexer => self
                .config
                .indexer_source
                .as_ref()
                .map(|source| source.source_id.as_str()),
        }
    }

    fn find_source(&self, source_id: &str) -> Option<&ResolvedSource> {
        self.sequencers
            .iter()
            .find(|source| source.descriptor.source_id == source_id)
            .or_else(|| {
                self.indexer
                    .as_ref()
                    .filter(|source| source.descriptor.source_id == source_id)
            })
    }
}

fn validate_context_identity(
    facts: &ZoneL2RuntimeFacts,
    context: &ActiveZoneContext,
    request_revision: u64,
) -> Result<(), ActiveZoneContextError> {
    if context.context_revision == 0 || request_revision == 0 {
        return Err(ActiveZoneContextError::new(
            L2ReadErrorCode::InvalidRequest,
            "Zone L2 revisions must be positive",
        ));
    }
    if !is_hex_identity(&context.channel_id) {
        return Err(ActiveZoneContextError::new(
            L2ReadErrorCode::InvalidRequest,
            "Active Zone Channel id is invalid",
        ));
    }
    if facts.verification != CatalogVerificationState::Verified {
        return Err(ActiveZoneContextError::new(
            L2ReadErrorCode::ZoneUnverified,
            "Zone Catalog is not verified",
        ));
    }
    if facts.network_scope.as_ref() != Some(&context.network_scope) {
        return Err(ActiveZoneContextError::stale(
            "Active Zone network scope is stale",
        ));
    }
    let Some(summary) = facts.summaries.get(&context.channel_id) else {
        return Err(ActiveZoneContextError::stale(
            "Active Zone no longer exists",
        ));
    };
    if summary.kind() != ZoneKind::SequencerZone {
        return Err(ActiveZoneContextError::new(
            L2ReadErrorCode::L2NotApplicable,
            "Active Zone has no Sequencer-backed L2",
        ));
    }
    if context.zone_kind != summary.kind() {
        return Err(ActiveZoneContextError::stale("Active Zone kind is stale"));
    }
    Ok(())
}

fn source_eligibility(
    facts: &ZoneL2RuntimeFacts,
    config: &ChannelSourceConfig,
    source_id: &str,
    role: ZoneSourceRole,
) -> Result<(), ActiveZoneContextError> {
    let observed_role = match role {
        ZoneSourceRole::Sequencer => ChannelSourceRole::Sequencer,
        ZoneSourceRole::Indexer => ChannelSourceRole::Indexer,
    };
    let observation = (facts.observations.catalog_verified
        && facts.observations.network_scope.as_ref() == Some(&config.network_scope))
    .then(|| {
        facts
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
            })
    })
    .flatten();
    if role == ZoneSourceRole::Sequencer {
        let _source = config
            .sequencer_sources
            .iter()
            .find(|source| source.source_id == source_id)
            .ok_or_else(|| {
                ActiveZoneContextError::new(
                    L2ReadErrorCode::SourceUnconfigured,
                    "Active Zone has no source configured for this read",
                )
            })?;
        let read_eligible = observation.is_some_and(|observation| {
            observation
                .binding_state
                .is_some_and(ChannelSourceBindingState::is_read_eligible)
        });
        if !read_eligible {
            return Err(ActiveZoneContextError::source_ineligible(
                "Sequencer source has no verified Channel binding",
            ));
        }
    }
    if observation
        .is_some_and(|observation| observation.health == ChannelSourceHealthState::ChannelMismatch)
    {
        return Err(ActiveZoneContextError::source_ineligible(
            "Source reports another Channel",
        ));
    }
    Ok(())
}

fn descriptor(
    config: &ChannelSourceConfig,
    source_id: &str,
    role: ZoneSourceRole,
    target: ChannelSourceTarget,
) -> L2SourceDescriptor {
    L2SourceDescriptor {
        source_id: source_id.to_owned(),
        role,
        target,
        source_config_revision: config.config_revision,
    }
}

fn empty_config(context: &ActiveZoneContext) -> ChannelSourceConfig {
    ChannelSourceConfig {
        network_scope: context.network_scope.clone(),
        channel_id: context.channel_id.clone(),
        config_revision: 0,
        sequencer_sources: Vec::new(),
        selected_sequencer_source_id: None,
        indexer_source: None,
    }
}

fn is_hex_identity(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|character| character.is_ascii_hexdigit())
}
