use anyhow::{Result, ensure};

use super::*;
use crate::{
    inspection::NetworkScope,
    source_routing::channel_sources::{
        ChannelSourceBindingState, ChannelSourceConfig, ChannelSourceCurrentFailure,
        ChannelSourceFailureKind, ChannelSourceLastGood, ChannelSourceProbeStage,
        ChannelSourceTarget, ConfiguredIndexerSource, ConfiguredSequencerSource,
        PersistedSequencerAttestation,
    },
};

#[test]
fn projects_not_applicable_unconfigured_and_unobserved_states() -> Result<()> {
    let channel_id = id('8');
    let config = config(&channel_id, &[1], false);
    let snapshot = snapshot(&config, Vec::new());

    ensure!(
        project_zone_sources(ZoneKind::DataChannel, &channel_id, Some(&config), &snapshot)
            .agreement
            .state
            == SequencerAgreementState::NotApplicable,
        "Data Channel agreement was applicable"
    );
    ensure!(
        project_zone_sources(
            ZoneKind::SequencerZone,
            &channel_id,
            None,
            &ChannelSourceMonitorSnapshot::default(),
        )
        .agreement
        .state
            == SequencerAgreementState::Unconfigured,
        "missing configuration was not unconfigured"
    );
    ensure!(
        project_zone_sources(
            ZoneKind::SequencerZone,
            &channel_id,
            Some(&config),
            &snapshot,
        )
        .agreement
        .state
            == SequencerAgreementState::Unobserved,
        "empty observation set was not unobserved"
    );
    Ok(())
}

#[test]
fn never_claims_convergence_from_one_eligible_source() -> Result<()> {
    let channel_id = id('8');
    let config = config(&channel_id, &[1], false);
    let snapshot = snapshot(&config, vec![sequencer(1, &channel_id, 10, Some("aa"))]);
    let projection = project_zone_sources(
        ZoneKind::SequencerZone,
        &channel_id,
        Some(&config),
        &snapshot,
    );

    ensure!(
        projection.agreement.state == SequencerAgreementState::SingleSource,
        "single source was reported as agreement"
    );
    ensure!(
        projection.selected_head.as_ref().map(|head| head.block_id) == Some(10),
        "selected source did not supply summary head"
    );
    Ok(())
}

#[test]
fn projects_degraded_health_without_discarding_last_good_source_facts() -> Result<()> {
    let channel_id = id('8');
    let config = config(&channel_id, &[1], false);
    let mut degraded = sequencer(1, &channel_id, 10, Some("aa"));
    degraded.health = ChannelSourceHealthState::Degraded;
    degraded.current_failure = Some(ChannelSourceCurrentFailure {
        kind: ChannelSourceFailureKind::Unavailable,
        stage: ChannelSourceProbeStage::Health,
        diagnostic: "health probe failed".to_owned(),
        failed_at_unix: 11,
        consecutive_failures: 1,
    });
    let snapshot = snapshot(&config, vec![degraded]);
    let projection = project_zone_sources(
        ZoneKind::SequencerZone,
        &channel_id,
        Some(&config),
        &snapshot,
    );

    ensure!(
        projection.source_status == L2SourceStatus::Degraded,
        "selected source aggregate status was not degraded"
    );
    ensure!(
        projection.agreement.state == SequencerAgreementState::SingleSource,
        "degraded last-good source stopped participating in comparison"
    );
    ensure!(
        projection.selected_head.as_ref().map(|head| head.block_id) == Some(10),
        "degraded source discarded its last-good head"
    );
    let observation = projection
        .observations
        .first()
        .ok_or_else(|| anyhow::anyhow!("degraded observation was not projected"))?;
    ensure!(
        observation.last_error.as_deref() == Some("health probe failed"),
        "degraded observation discarded its diagnostic"
    );
    ensure!(
        observation.health == ZoneSourceHealth::Degraded,
        "degraded observation was projected as another health state"
    );
    let serialized = serde_json::to_value(observation)?;
    ensure!(
        serialized.get("health").and_then(serde_json::Value::as_str) == Some("degraded"),
        "degraded observation was serialized as another health state"
    );
    Ok(())
}

#[test]
fn equal_heads_require_equal_nonempty_hashes_for_convergence() -> Result<()> {
    let channel_id = id('8');
    let config = config(&channel_id, &[1, 2], false);
    let converged = snapshot(
        &config,
        vec![
            sequencer(1, &channel_id, 10, Some("aa")),
            sequencer(2, &channel_id, 10, Some("aa")),
        ],
    );
    let divergent = snapshot(
        &config,
        vec![
            sequencer(1, &channel_id, 10, Some("aa")),
            sequencer(2, &channel_id, 10, Some("bb")),
        ],
    );
    let hashless = snapshot(
        &config,
        vec![
            sequencer(1, &channel_id, 10, Some("aa")),
            sequencer(2, &channel_id, 10, None),
        ],
    );

    ensure!(
        agreement(&channel_id, &config, &converged) == SequencerAgreementState::Converged,
        "matching heads were not converged"
    );
    ensure!(
        agreement(&channel_id, &config, &divergent) == SequencerAgreementState::Divergent,
        "different hashes were not divergent"
    );
    ensure!(
        agreement(&channel_id, &config, &hashless) == SequencerAgreementState::SingleSource,
        "hashless observation entered comparison"
    );
    Ok(())
}

#[test]
fn overlap_samples_distinguish_lag_from_unverified_skew_and_divergence() -> Result<()> {
    let channel_id = id('8');
    let config = config(&channel_id, &[1, 2], false);
    let lower = sequencer(1, &channel_id, 10, Some("lower"));
    let mut matching_higher = sequencer(2, &channel_id, 12, Some("higher"));
    matching_higher
        .comparison_blocks
        .push(block(10, Some("lower")));
    let missing_higher = sequencer(2, &channel_id, 12, Some("higher"));
    let mut divergent_higher = missing_higher.clone();
    divergent_higher
        .comparison_blocks
        .push(block(10, Some("fork")));

    let lagging = snapshot(&config, vec![lower.clone(), matching_higher]);
    let unverified = snapshot(&config, vec![lower.clone(), missing_higher]);
    let divergent = snapshot(&config, vec![lower, divergent_higher]);
    let lagging_projection = project_zone_sources(
        ZoneKind::SequencerZone,
        &channel_id,
        Some(&config),
        &lagging,
    );

    ensure!(
        lagging_projection.agreement.state == SequencerAgreementState::Lagging,
        "matching overlap was not lagging"
    );
    ensure!(
        lagging_projection.agreement.lagging_source_ids == vec![source_id(1)],
        "lower source was not identified"
    );
    ensure!(
        agreement(&channel_id, &config, &unverified) == SequencerAgreementState::SkewUnverified,
        "missing overlap was not unverified"
    );
    ensure!(
        agreement(&channel_id, &config, &divergent) == SequencerAgreementState::Divergent,
        "overlap fork was not divergent"
    );
    Ok(())
}

#[test]
fn finalized_conflict_has_deterministic_highest_severity() -> Result<()> {
    let channel_id = id('8');
    let config = config(&channel_id, &[1, 2], true);
    let mut first = sequencer(1, &channel_id, 12, Some("head-a"));
    first.comparison_blocks.push(block(10, Some("final")));
    let mut second = sequencer(2, &channel_id, 12, Some("head-b"));
    second.comparison_blocks.push(block(10, Some("fork")));
    let snapshot = snapshot(&config, vec![first, second, indexer(99, 10, "final")]);
    let projection = project_zone_sources(
        ZoneKind::SequencerZone,
        &channel_id,
        Some(&config),
        &snapshot,
    );

    ensure!(
        projection.agreement.state == SequencerAgreementState::FinalizedConflict,
        "finalized conflict did not outrank provisional divergence"
    );
    ensure!(
        projection.agreement.finalized_conflict_source_ids == vec![source_id(2)],
        "conflicting source was not retained"
    );
    ensure!(
        projection.finalized_head.as_ref().map(|head| head.block_id) == Some(10),
        "Indexer finalized head was not projected"
    );
    Ok(())
}

#[test]
fn excludes_mismatch_unreachable_and_stale_revision_observations() -> Result<()> {
    let channel_id = id('8');
    let config = config(&channel_id, &[1, 2], false);
    let mut mismatch = sequencer(1, &channel_id, 10, Some("aa"));
    mismatch.binding_state = Some(ChannelSourceBindingState::ChannelMismatch);
    mismatch.health = ChannelSourceHealthState::ChannelMismatch;
    let mut unreachable = sequencer(2, &channel_id, 10, Some("aa"));
    unreachable.health = ChannelSourceHealthState::Unreachable;
    let excluded = snapshot(&config, vec![mismatch, unreachable]);
    let mut stale = snapshot(&config, vec![sequencer(1, &channel_id, 10, Some("aa"))]);
    if let Some(set) = stale.channels.first_mut() {
        set.config_revision = set.config_revision.saturating_add(1);
    }

    ensure!(
        agreement(&channel_id, &config, &excluded) == SequencerAgreementState::Unobserved,
        "ineligible observations entered agreement"
    );
    ensure!(
        agreement(&channel_id, &config, &stale) == SequencerAgreementState::Unobserved,
        "stale configuration revision was accepted"
    );
    Ok(())
}

fn agreement(
    channel_id: &str,
    config: &ChannelSourceConfig,
    snapshot: &ChannelSourceMonitorSnapshot,
) -> SequencerAgreementState {
    project_zone_sources(ZoneKind::SequencerZone, channel_id, Some(config), snapshot)
        .agreement
        .state
}

fn config(channel_id: &str, sequencers: &[u8], with_indexer: bool) -> ChannelSourceConfig {
    let sequencer_sources = sequencers
        .iter()
        .map(|number| ConfiguredSequencerSource {
            source_id: source_id(*number),
            label: None,
            target: ChannelSourceTarget::Rpc {
                endpoint: format!("http://localhost:{}/", 3000_u16 + u16::from(*number)),
            },
            channel_attestation: PersistedSequencerAttestation::PersistedAttested {
                channel_id: channel_id.to_owned(),
                target_fingerprint: ChannelSourceTarget::Rpc {
                    endpoint: format!("http://localhost:{}/", 3000_u16 + u16::from(*number)),
                }
                .fingerprint(),
                attested_at_unix: 1,
            },
        })
        .collect::<Vec<_>>();
    ChannelSourceConfig {
        network_scope: scope(),
        channel_id: channel_id.to_owned(),
        config_revision: 7,
        selected_sequencer_source_id: sequencers.first().map(|number| source_id(*number)),
        sequencer_sources,
        indexer_source: with_indexer.then(|| ConfiguredIndexerSource {
            source_id: source_id(99),
            label: None,
            target: ChannelSourceTarget::Rpc {
                endpoint: "http://localhost:7080/".to_owned(),
            },
        }),
    }
}

fn snapshot(
    config: &ChannelSourceConfig,
    observations: Vec<ChannelSourceObservation>,
) -> ChannelSourceMonitorSnapshot {
    ChannelSourceMonitorSnapshot {
        network_scope: Some(config.network_scope.clone()),
        catalog_verified: true,
        observation_revision: 19,
        channels: vec![ChannelSourceObservationSet {
            channel_id: config.channel_id.clone(),
            config_revision: config.config_revision,
            selected_sequencer_source_id: config.selected_sequencer_source_id.clone(),
            observations,
        }],
    }
}

fn sequencer(
    number: u8,
    channel_id: &str,
    block_id: u64,
    hash: Option<&str>,
) -> ChannelSourceObservation {
    ChannelSourceObservation {
        source_id: source_id(number),
        role: ChannelSourceRole::Sequencer,
        selected: number == 1,
        binding_state: Some(ChannelSourceBindingState::PersistedAttested),
        health: ChannelSourceHealthState::Reachable,
        last_good: Some(ChannelSourceLastGood {
            observed_at_unix: 10,
            latency_millis: 3,
            health_ok: true,
            reported_channel_id: Some(channel_id.to_owned()),
            head: Some(block(block_id, hash)),
        }),
        current_failure: None,
        comparison_blocks: Vec::new(),
    }
}

fn indexer(number: u8, block_id: u64, hash: &str) -> ChannelSourceObservation {
    ChannelSourceObservation {
        source_id: source_id(number),
        role: ChannelSourceRole::Indexer,
        selected: false,
        binding_state: None,
        health: ChannelSourceHealthState::Reachable,
        last_good: Some(ChannelSourceLastGood {
            observed_at_unix: 10,
            latency_millis: 4,
            health_ok: true,
            reported_channel_id: None,
            head: Some(block(block_id, Some(hash))),
        }),
        current_failure: None,
        comparison_blocks: Vec::new(),
    }
}

fn block(block_id: u64, hash: Option<&str>) -> ChannelSourceBlockObservation {
    ChannelSourceBlockObservation {
        block_id,
        header_hash: hash.map(ToOwned::to_owned),
        parent_hash: Some("parent".to_owned()),
        observed_at_unix: 10,
        failure_kind: None,
    }
}

fn scope() -> NetworkScope {
    NetworkScope::GenesisId {
        genesis_id: id('a'),
    }
}

fn id(character: char) -> String {
    character.to_string().repeat(64)
}

fn source_id(number: u8) -> String {
    format!("src_{number:032x}")
}
