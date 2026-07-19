use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{
    L2SourceStatus, L2ZoneSummary, SequencerAgreementState, ZoneKind, ZoneSourceBindingState,
    ZoneSourceHealth, ZoneSourceObservation, ZoneSourceRole,
};
use crate::source_routing::channel_sources::{
    ChannelSourceBindingState, ChannelSourceBlockObservation, ChannelSourceConfig,
    ChannelSourceHealthState, ChannelSourceMonitorSnapshot, ChannelSourceObservation,
    ChannelSourceObservationSet, ChannelSourceRole,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneSourceAgreement {
    pub state: SequencerAgreementState,
    pub comparable_source_count: u64,
    pub eligible_source_ids: Vec<String>,
    pub compared_block_ids: Vec<u64>,
    pub lagging_source_ids: Vec<String>,
    pub divergent_block_ids: Vec<u64>,
    pub finalized_conflict_source_ids: Vec<String>,
    pub unverified_block_ids: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneSourceProjection {
    pub channel_id: String,
    pub config_revision: Option<u64>,
    pub observation_revision: u64,
    pub observations: Vec<ZoneSourceObservation>,
    pub agreement: ZoneSourceAgreement,
    pub source_status: L2SourceStatus,
    pub observed_source_count: u64,
    pub selected_head: Option<ChannelSourceBlockObservation>,
    pub finalized_head: Option<ChannelSourceBlockObservation>,
}

impl ZoneSourceProjection {
    pub fn apply_to_l2_zone(&self, l2_zone: &mut L2ZoneSummary) {
        l2_zone.source_status = self.source_status;
        l2_zone.observed_source_count = self.observed_source_count;
        l2_zone.latest_block_id = self.selected_head.as_ref().map(|head| head.block_id);
        l2_zone.latest_block_hash = self
            .selected_head
            .as_ref()
            .and_then(|head| head.header_hash.clone());
        l2_zone.finalized_block_id = self.finalized_head.as_ref().map(|head| head.block_id);
        l2_zone.agreement_state = self.agreement.state;
    }
}

#[must_use]
pub fn project_zone_sources(
    kind: ZoneKind,
    channel_id: &str,
    config: Option<&ChannelSourceConfig>,
    snapshot: &ChannelSourceMonitorSnapshot,
) -> ZoneSourceProjection {
    let observation_set = matching_observation_set(channel_id, config, snapshot);
    let observations = observation_set.map_or_else(Vec::new, |set| {
        set.observations
            .iter()
            .map(project_zone_observation)
            .collect()
    });
    let agreement = project_agreement(kind, config, observation_set);
    let observed_source_count = observation_set.map_or(0, |set| {
        usize_to_u64(
            set.observations
                .iter()
                .filter(|observation| {
                    observation.role == ChannelSourceRole::Sequencer
                        && observation.last_good.is_some()
                })
                .count(),
        )
    });
    let selected = config
        .and_then(|config| config.selected_sequencer_source_id.as_deref())
        .and_then(|source_id| {
            observation_set.and_then(|set| {
                set.observations
                    .iter()
                    .find(|observation| observation.source_id == source_id)
            })
        });
    let selected_head = selected
        .and_then(|observation| observation.last_good.as_ref())
        .and_then(|last_good| last_good.head.clone());
    let finalized_head = observation_set
        .and_then(|set| {
            set.observations
                .iter()
                .find(|observation| observation.role == ChannelSourceRole::Indexer)
        })
        .and_then(|observation| observation.last_good.as_ref())
        .and_then(|last_good| last_good.head.clone());

    ZoneSourceProjection {
        channel_id: channel_id.to_owned(),
        config_revision: config.map(|config| config.config_revision),
        observation_revision: snapshot.observation_revision,
        observations,
        agreement,
        source_status: project_source_status(kind, config, selected),
        observed_source_count,
        selected_head,
        finalized_head,
    }
}

fn matching_observation_set<'a>(
    channel_id: &str,
    config: Option<&ChannelSourceConfig>,
    snapshot: &'a ChannelSourceMonitorSnapshot,
) -> Option<&'a ChannelSourceObservationSet> {
    if !snapshot.catalog_verified {
        return None;
    }
    let config = config?;
    if snapshot.network_scope.as_ref() != Some(&config.network_scope)
        || config.channel_id != channel_id
    {
        return None;
    }
    snapshot
        .channels
        .iter()
        .find(|set| set.channel_id == channel_id && set.config_revision == config.config_revision)
}

fn project_zone_observation(observation: &ChannelSourceObservation) -> ZoneSourceObservation {
    let last_good = observation.last_good.as_ref();
    ZoneSourceObservation {
        source_id: observation.source_id.clone(),
        role: match observation.role {
            ChannelSourceRole::Sequencer => ZoneSourceRole::Sequencer,
            ChannelSourceRole::Indexer => ZoneSourceRole::Indexer,
        },
        binding_state: observation.binding_state.map(|state| match state {
            ChannelSourceBindingState::PersistedAttested => {
                ZoneSourceBindingState::PersistedAttested
            }
            ChannelSourceBindingState::Pending => ZoneSourceBindingState::Pending,
            ChannelSourceBindingState::RuntimeAttested => ZoneSourceBindingState::RuntimeAttested,
            ChannelSourceBindingState::ChannelMismatch => ZoneSourceBindingState::ChannelMismatch,
        }),
        health: match observation.health {
            ChannelSourceHealthState::Pending => ZoneSourceHealth::Pending,
            ChannelSourceHealthState::Reachable => ZoneSourceHealth::Reachable,
            ChannelSourceHealthState::Degraded => ZoneSourceHealth::Degraded,
            ChannelSourceHealthState::Unreachable
            | ChannelSourceHealthState::Incomplete
            | ChannelSourceHealthState::Unsupported => ZoneSourceHealth::Unreachable,
            ChannelSourceHealthState::ChannelMismatch => ZoneSourceHealth::ChannelMismatch,
        },
        reported_channel_id: last_good
            .and_then(|observation| observation.reported_channel_id.clone()),
        head_block_id: last_good
            .and_then(|observation| observation.head.as_ref())
            .map(|head| head.block_id),
        head_block_hash: last_good
            .and_then(|observation| observation.head.as_ref())
            .and_then(|head| head.header_hash.clone()),
        head_parent_hash: last_good
            .and_then(|observation| observation.head.as_ref())
            .and_then(|head| head.parent_hash.clone()),
        observed_at_unix: last_good.map(|observation| observation.observed_at_unix),
        latency_millis: last_good.map(|observation| observation.latency_millis),
        last_error: observation
            .current_failure
            .as_ref()
            .map(|failure| failure.diagnostic.clone()),
    }
}

fn project_source_status(
    kind: ZoneKind,
    config: Option<&ChannelSourceConfig>,
    selected: Option<&ChannelSourceObservation>,
) -> L2SourceStatus {
    if kind != ZoneKind::SequencerZone {
        return L2SourceStatus::Unknown;
    }
    let Some(config) = config else {
        return L2SourceStatus::Unconfigured;
    };
    if config.sequencer_sources.is_empty() {
        return L2SourceStatus::Unconfigured;
    }
    match selected.map(|observation| observation.health) {
        Some(ChannelSourceHealthState::Reachable) => L2SourceStatus::Reachable,
        Some(ChannelSourceHealthState::Degraded) => L2SourceStatus::Degraded,
        Some(ChannelSourceHealthState::Unreachable)
        | Some(ChannelSourceHealthState::Incomplete)
        | Some(ChannelSourceHealthState::Unsupported)
        | Some(ChannelSourceHealthState::ChannelMismatch) => L2SourceStatus::Unreachable,
        Some(ChannelSourceHealthState::Pending) | None => L2SourceStatus::Unknown,
    }
}

fn project_agreement(
    kind: ZoneKind,
    config: Option<&ChannelSourceConfig>,
    observation_set: Option<&ChannelSourceObservationSet>,
) -> ZoneSourceAgreement {
    if kind == ZoneKind::DataChannel {
        return empty_agreement(SequencerAgreementState::NotApplicable);
    }
    let Some(config) = config else {
        return empty_agreement(SequencerAgreementState::Unconfigured);
    };
    if config.sequencer_sources.is_empty() {
        return empty_agreement(SequencerAgreementState::Unconfigured);
    }
    let Some(observation_set) = observation_set else {
        return empty_agreement(SequencerAgreementState::Unobserved);
    };

    let sequencers = observation_set
        .observations
        .iter()
        .filter(|observation| observation.role == ChannelSourceRole::Sequencer)
        .filter(|observation| observation.is_comparable())
        .filter_map(|observation| {
            observation
                .last_good
                .as_ref()
                .and_then(|last_good| last_good.head.as_ref())
                .map(|head| ComparableSource { observation, head })
        })
        .collect::<Vec<_>>();
    if sequencers.is_empty() {
        return empty_agreement(SequencerAgreementState::Unobserved);
    }

    let mut compared_block_ids = BTreeSet::new();
    let mut lagging_source_ids = BTreeSet::new();
    let mut divergent_block_ids = BTreeSet::new();
    let mut finalized_conflict_source_ids = BTreeSet::new();
    let mut unverified_block_ids = BTreeSet::new();
    compare_sequencer_pairs(
        &sequencers,
        &mut compared_block_ids,
        &mut lagging_source_ids,
        &mut divergent_block_ids,
        &mut unverified_block_ids,
    );
    compare_finalized_head(
        &sequencers,
        observation_set,
        &mut compared_block_ids,
        &mut finalized_conflict_source_ids,
        &mut unverified_block_ids,
    );

    let distinct_heads = sequencers
        .iter()
        .map(|source| source.head.block_id)
        .collect::<BTreeSet<_>>();
    let state = if !finalized_conflict_source_ids.is_empty() {
        SequencerAgreementState::FinalizedConflict
    } else if !divergent_block_ids.is_empty() {
        SequencerAgreementState::Divergent
    } else if sequencers.len() == 1 {
        SequencerAgreementState::SingleSource
    } else if !unverified_block_ids.is_empty() {
        SequencerAgreementState::SkewUnverified
    } else if distinct_heads.len() > 1 {
        SequencerAgreementState::Lagging
    } else {
        SequencerAgreementState::Converged
    };

    ZoneSourceAgreement {
        state,
        comparable_source_count: usize_to_u64(sequencers.len()),
        eligible_source_ids: sequencers
            .iter()
            .map(|source| source.observation.source_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        compared_block_ids: compared_block_ids.into_iter().collect(),
        lagging_source_ids: lagging_source_ids.into_iter().collect(),
        divergent_block_ids: divergent_block_ids.into_iter().collect(),
        finalized_conflict_source_ids: finalized_conflict_source_ids.into_iter().collect(),
        unverified_block_ids: unverified_block_ids.into_iter().collect(),
    }
}

struct ComparableSource<'a> {
    observation: &'a ChannelSourceObservation,
    head: &'a ChannelSourceBlockObservation,
}

fn compare_sequencer_pairs(
    sequencers: &[ComparableSource<'_>],
    compared_block_ids: &mut BTreeSet<u64>,
    lagging_source_ids: &mut BTreeSet<String>,
    divergent_block_ids: &mut BTreeSet<u64>,
    unverified_block_ids: &mut BTreeSet<u64>,
) {
    for (position, left) in sequencers.iter().enumerate() {
        for right in sequencers.iter().skip(position.saturating_add(1)) {
            if left.head.block_id == right.head.block_id {
                compared_block_ids.insert(left.head.block_id);
                if block_hash(left.head) != block_hash(right.head) {
                    divergent_block_ids.insert(left.head.block_id);
                }
                continue;
            }
            let (lower, higher) = if left.head.block_id < right.head.block_id {
                (left, right)
            } else {
                (right, left)
            };
            let block_id = lower.head.block_id;
            let Some(higher_hash) = observation_hash_at(higher.observation, block_id) else {
                unverified_block_ids.insert(block_id);
                continue;
            };
            compared_block_ids.insert(block_id);
            if Some(higher_hash) == block_hash(lower.head) {
                lagging_source_ids.insert(lower.observation.source_id.clone());
            } else {
                divergent_block_ids.insert(block_id);
            }
        }
    }
}

fn compare_finalized_head(
    sequencers: &[ComparableSource<'_>],
    observation_set: &ChannelSourceObservationSet,
    compared_block_ids: &mut BTreeSet<u64>,
    finalized_conflict_source_ids: &mut BTreeSet<String>,
    unverified_block_ids: &mut BTreeSet<u64>,
) {
    let Some(indexer) = observation_set
        .observations
        .iter()
        .find(|observation| observation.role == ChannelSourceRole::Indexer)
        .filter(|observation| observation.is_comparable())
    else {
        return;
    };
    let Some(finalized_head) = indexer
        .last_good
        .as_ref()
        .and_then(|last_good| last_good.head.as_ref())
    else {
        return;
    };
    let Some(finalized_hash) = block_hash(finalized_head) else {
        return;
    };

    for sequencer in sequencers {
        let block_id = finalized_head.block_id;
        if sequencer.head.block_id < block_id {
            unverified_block_ids.insert(block_id);
            continue;
        }
        let Some(sequencer_hash) = observation_hash_at(sequencer.observation, block_id) else {
            unverified_block_ids.insert(block_id);
            continue;
        };
        compared_block_ids.insert(block_id);
        if sequencer_hash != finalized_hash {
            finalized_conflict_source_ids.insert(sequencer.observation.source_id.clone());
        }
    }
}

fn observation_hash_at(observation: &ChannelSourceObservation, block_id: u64) -> Option<&str> {
    let last_good = observation.last_good.as_ref()?;
    let head = last_good.head.as_ref()?;
    if head.block_id == block_id {
        return block_hash(head);
    }
    observation
        .comparison_blocks
        .iter()
        .find(|sample| sample.block_id == block_id)
        .and_then(block_hash)
}

fn block_hash(block: &ChannelSourceBlockObservation) -> Option<&str> {
    block
        .header_hash
        .as_deref()
        .filter(|header_hash| !header_hash.is_empty())
}

fn empty_agreement(state: SequencerAgreementState) -> ZoneSourceAgreement {
    ZoneSourceAgreement {
        state,
        comparable_source_count: 0,
        eligible_source_ids: Vec::new(),
        compared_block_ids: Vec::new(),
        lagging_source_ids: Vec::new(),
        divergent_block_ids: Vec::new(),
        finalized_conflict_source_ids: Vec::new(),
        unverified_block_ids: Vec::new(),
    }
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests;
