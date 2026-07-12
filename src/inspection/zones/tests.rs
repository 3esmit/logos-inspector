use anyhow::{Result, bail};
use serde_json::Value;

use super::fixtures::{
    complete_replay_catalog, data_channel, l1_only_sequencer_zone, linked_sequencer_zone,
    partial_raw_catalog_with_connected_lifecycle, partial_raw_catalog_without_connected_lifecycle,
    point_snapshot_catalog_across_gap, sequencer_source_config, unknown_l1_channel,
};
use super::*;

#[test]
fn agreed_fixtures_serialize_as_closed_zone_variants() -> Result<()> {
    let fixtures = [
        (linked_sequencer_zone(), "sequencer_zone"),
        (l1_only_sequencer_zone(), "sequencer_zone"),
        (data_channel(), "data_channel"),
        (unknown_l1_channel(), "unknown"),
    ];

    for (fixture, expected_kind) in fixtures {
        let value = serde_json::to_value(&fixture)?;
        if value.get("kind").and_then(Value::as_str) != Some(expected_kind) {
            bail!("unexpected Zone kind for {}: {value}", fixture.channel_id);
        }
        if !fixture.activity_state.is_valid_for(fixture.kind()) {
            bail!("invalid fixture activity state: {fixture:?}");
        }

        let decoded: ZoneSummary = serde_json::from_value(value)?;
        require_equal(&decoded, &fixture, "Zone fixture round trip")?;
    }
    Ok(())
}

#[test]
fn sequencer_fixtures_keep_shared_l2_and_committee_facts() -> Result<()> {
    let linked = linked_sequencer_zone();
    require_equal(&linked.kind(), &ZoneKind::SequencerZone, "linked Zone kind")?;
    require_equal(
        &linked.settlement_link.status,
        &SettlementLinkStatus::Linked,
        "linked settlement state",
    )?;
    let Some(l2_zone) = linked.l2_zone() else {
        bail!("linked Sequencer Zone omitted L2 facts");
    };
    require_equal(&l2_zone.latest_block_id, &Some(1_099), "latest L2 block")?;
    require_equal(&l2_zone.safe_block_id, &Some(1_097), "safe L2 block")?;
    require_equal(
        &l2_zone.finalized_block_id,
        &Some(1_088),
        "finalized L2 block",
    )?;
    require_equal(
        &l2_zone.finality_state,
        &L2FinalityState::Safe,
        "L2 finality",
    )?;
    require_equal(
        &linked
            .sequencer_committee()
            .map(|value| value.members.len()),
        &Some(2),
        "committee size",
    )?;

    let l1_only = l1_only_sequencer_zone();
    require_equal(
        &l1_only.kind(),
        &ZoneKind::SequencerZone,
        "L1-only Zone kind",
    )?;
    require_equal(
        &l1_only.settlement_link.status,
        &SettlementLinkStatus::L1Only,
        "L1-only settlement state",
    )?;
    let Some(l2_zone) = l1_only.l2_zone() else {
        bail!("L1-only Sequencer Zone omitted L2 boundary");
    };
    require_equal(
        &l2_zone.source_status,
        &L2SourceStatus::Unconfigured,
        "L1-only source status",
    )?;
    require_equal(&l2_zone.latest_block_id, &None, "L1-only L2 tip")?;
    require_equal(
        &l2_zone.finality_state,
        &L2FinalityState::Unknown,
        "L1-only L2 finality",
    )?;
    Ok(())
}

#[test]
fn data_channel_uses_l1_finality_and_cannot_carry_l2_facts() -> Result<()> {
    let fixture = data_channel();
    let value = serde_json::to_value(&fixture)?;

    require_equal(&fixture.kind(), &ZoneKind::DataChannel, "Data Channel kind")?;
    require_equal(
        &fixture.settlement_link.status,
        &SettlementLinkStatus::RawData,
        "Data Channel settlement state",
    )?;
    require(fixture.l2_zone().is_none(), "Data Channel L2 facts")?;
    require(
        fixture.sequencer_committee().is_none(),
        "Data Channel committee facts",
    )?;
    let Some(raw_activity) = fixture.raw_activity() else {
        bail!("Data Channel omitted raw activity");
    };
    require_equal(&raw_activity.inscription_count, &3, "raw activity count")?;
    require_equal(
        &raw_activity.finality_state,
        &L1FinalityState::Final,
        "raw L1 finality",
    )?;
    require(value.get("l2_zone").is_none(), "serialized Data Channel L2")?;
    require(
        value.get("sequencer_committee").is_none(),
        "serialized Data Channel committee",
    )?;
    Ok(())
}

#[test]
fn unknown_fixture_preserves_partial_coverage_without_guessing_kind() -> Result<()> {
    let fixture = unknown_l1_channel();

    require_equal(&fixture.kind(), &ZoneKind::Unknown, "unknown Zone kind")?;
    require(fixture.l2_zone().is_none(), "unknown Zone L2 facts")?;
    require(
        fixture.raw_activity().is_none(),
        "unknown Zone raw activity",
    )?;
    require_equal(
        &fixture.provenance.coverage.status,
        &CatalogCoverageStatus::Partial,
        "unknown Zone coverage",
    )?;
    require_equal(
        &fixture.provenance.coverage.prefix_status,
        &CoveragePrefixStatus::Unavailable,
        "unknown Zone prefix coverage",
    )?;
    Ok(())
}

#[test]
fn fixtures_match_conservative_classification_evidence() {
    let cases = [
        (
            linked_sequencer_zone(),
            ZoneClassificationEvidence {
                recognized_l2_evidence: true,
                configured_sequencer_link: true,
                raw_inscription_evidence: false,
                l2_absence_is_covered: false,
                conflicting_evidence: false,
            },
        ),
        (
            l1_only_sequencer_zone(),
            ZoneClassificationEvidence {
                recognized_l2_evidence: true,
                configured_sequencer_link: false,
                raw_inscription_evidence: false,
                l2_absence_is_covered: false,
                conflicting_evidence: false,
            },
        ),
        (
            data_channel(),
            ZoneClassificationEvidence {
                recognized_l2_evidence: false,
                configured_sequencer_link: false,
                raw_inscription_evidence: true,
                l2_absence_is_covered: true,
                conflicting_evidence: false,
            },
        ),
        (
            unknown_l1_channel(),
            ZoneClassificationEvidence {
                recognized_l2_evidence: false,
                configured_sequencer_link: false,
                raw_inscription_evidence: false,
                l2_absence_is_covered: false,
                conflicting_evidence: false,
            },
        ),
    ];

    for (fixture, evidence) in cases {
        assert_eq!(classify_zone(evidence), fixture.kind());
    }
}

#[test]
fn zone_detail_adds_compact_facts_without_repeating_summary_sections() -> Result<()> {
    let summary = linked_sequencer_zone();
    let detail = ZoneDetail {
        summary,
        l1_channel_snapshot: L1ChannelSnapshot {
            channel_tip: Some("tip-1097".to_owned()),
            keys: vec!["committee-key-a".to_owned(), "committee-key-b".to_owned()],
            observed_at_slot: Some(187_085),
        },
        channel_source_config: ChannelSourceConfigSummary {
            config_revision: 4,
            selected_sequencer_source_id: Some("seq-primary".to_owned()),
            sequencer_sources: vec![ConfiguredZoneSource {
                source_id: "seq-primary".to_owned(),
                label: Some("Primary".to_owned()),
                target: ZoneSourceTarget::Rpc {
                    endpoint: "http://127.0.0.1:3040/".to_owned(),
                },
            }],
            indexer_source: Some(ConfiguredZoneSource {
                source_id: "indexer-main".to_owned(),
                label: None,
                target: ZoneSourceTarget::Module {
                    module_id: "indexer".to_owned(),
                },
            }),
        },
        source_observations: vec![ZoneSourceObservation {
            source_id: "seq-primary".to_owned(),
            role: ZoneSourceRole::Sequencer,
            binding_state: Some(ZoneSourceBindingState::PersistedAttested),
            health: ZoneSourceHealth::Reachable,
            reported_channel_id: Some("8".repeat(64)),
            head_block_id: Some(1_099),
            head_block_hash: Some("b".repeat(64)),
            head_parent_hash: Some("a".repeat(64)),
            observed_at_unix: Some(1_782_985_805),
            latency_millis: Some(4),
            last_error: None,
        }],
        classification_evidence: ZoneClassificationEvidence {
            recognized_l2_evidence: true,
            configured_sequencer_link: true,
            raw_inscription_evidence: false,
            l2_absence_is_covered: false,
            conflicting_evidence: false,
        },
        activity_counts: ZoneActivityCounts {
            l1_operations: 42,
            recognized_l2_blocks: 1_100,
            raw_inscriptions: 0,
        },
        detail_revision: 9,
    };
    let value = serde_json::to_value(&detail)?;

    for key in [
        "summary",
        "l1_channel_snapshot",
        "channel_source_config",
        "source_observations",
        "classification_evidence",
        "activity_counts",
        "detail_revision",
    ] {
        if value.get(key).is_none() {
            bail!("Zone detail missing `{key}`: {value}");
        }
    }
    for repeated_key in [
        "settlement_link",
        "sequencer_committee",
        "agreement",
        "coverage_provenance",
    ] {
        if value.get(repeated_key).is_some() {
            bail!("Zone detail repeats summary field `{repeated_key}`: {value}");
        }
    }

    let decoded: ZoneDetail = serde_json::from_value(value)?;
    require_equal(&decoded, &detail, "Zone detail round trip")?;
    Ok(())
}

#[test]
fn complete_catalog_projection_keeps_authoritative_replay_facts() -> Result<()> {
    let snapshot = complete_replay_catalog();

    let rows = project_catalog_zones(&snapshot, &[], CatalogVerificationState::Verified);

    let Some(row) = rows.first() else {
        bail!("catalog projection omitted Zone");
    };
    require_equal(&rows.len(), &1, "catalog row count")?;
    require_equal(&row.kind(), &ZoneKind::SequencerZone, "catalog Zone kind")?;
    require_equal(
        &row.settlement_link.status,
        &SettlementLinkStatus::L1Only,
        "catalog settlement status",
    )?;
    require_equal(
        &row.l1_channel.balance.as_deref(),
        &Some("1000"),
        "authoritative replay balance",
    )?;
    require_equal(
        &row.sequencer_committee()
            .map(|committee| committee.members.len()),
        &Some(1),
        "authoritative replay committee",
    )?;
    require_equal(
        &row.provenance.coverage.status,
        &CatalogCoverageStatus::Complete,
        "catalog coverage projection",
    )?;
    Ok(())
}

#[test]
fn incomplete_lifecycle_suppresses_replay_state_and_data_channel_guess() -> Result<()> {
    let snapshot = partial_raw_catalog_without_connected_lifecycle();

    let rows = project_catalog_zones(&snapshot, &[], CatalogVerificationState::CachedUnverified);

    let Some(row) = rows.first() else {
        bail!("catalog projection omitted partial Zone");
    };
    require_equal(&row.kind(), &ZoneKind::Unknown, "partial Zone kind")?;
    require_equal(&row.l1_channel.balance, &None, "uncovered replay balance")?;
    require_equal(
        &row.l1_channel.tip_hash,
        &None,
        "uncovered replay Channel tip",
    )?;
    require_equal(
        &row.provenance.verification_state,
        &CatalogVerificationState::CachedUnverified,
        "cached verification state",
    )?;
    require_equal(
        &row.provenance.coverage.status,
        &CatalogCoverageStatus::Partial,
        "partial coverage state",
    )?;
    Ok(())
}

#[test]
fn connected_partial_lifecycle_projects_data_channel_facts() -> Result<()> {
    let snapshot = partial_raw_catalog_with_connected_lifecycle();

    let rows = project_catalog_zones(&snapshot, &[], CatalogVerificationState::Verified);

    let Some(row) = rows.first() else {
        bail!("catalog projection omitted Data Channel");
    };
    require_equal(&row.kind(), &ZoneKind::DataChannel, "Data Channel kind")?;
    require_equal(
        &row.settlement_link.status,
        &SettlementLinkStatus::RawData,
        "Data Channel settlement state",
    )?;
    let Some(raw_activity) = row.raw_activity() else {
        bail!("Data Channel projection omitted raw activity");
    };
    require_equal(&raw_activity.inscription_count, &1, "raw inscription count")?;
    require_equal(&raw_activity.latest_slot, &Some(9), "latest raw slot")?;
    Ok(())
}

#[test]
fn point_snapshot_keeps_explicit_channel_state_across_gap() -> Result<()> {
    let snapshot = point_snapshot_catalog_across_gap();

    let rows = project_catalog_zones(&snapshot, &[], CatalogVerificationState::Verified);

    let Some(row) = rows.first() else {
        bail!("catalog projection omitted point-snapshot Zone");
    };
    require_equal(&row.kind(), &ZoneKind::SequencerZone, "point Zone kind")?;
    require_equal(
        &row.l1_channel.balance.as_deref(),
        &Some("1000"),
        "point-snapshot balance",
    )?;
    require_equal(
        &row.sequencer_committee()
            .map(|committee| committee.members.len()),
        &Some(1),
        "point-snapshot committee",
    )?;
    require_equal(
        &row.provenance.coverage.status,
        &CatalogCoverageStatus::Partial,
        "point-snapshot global coverage",
    )?;
    Ok(())
}

#[test]
fn configured_channels_overlay_catalog_and_materialize_before_discovery() -> Result<()> {
    let snapshot = complete_replay_catalog();
    let scope = &snapshot.metadata.network_scope;
    let configured_only_id = "0".repeat(64);
    let catalog_id = snapshot
        .zones
        .first()
        .map(|record| record.channel_id.clone())
        .ok_or_else(|| anyhow::anyhow!("catalog fixture omitted Zone"))?;
    let configured_only = sequencer_source_config(scope, &configured_only_id);
    let catalog_config = sequencer_source_config(scope, &catalog_id);
    let mut other_network = sequencer_source_config(scope, &"f".repeat(64));
    other_network.network_scope = NetworkScope::GenesisId {
        genesis_id: "1".repeat(64),
    };

    let rows = project_catalog_zones(
        &snapshot,
        &[configured_only, catalog_config, other_network],
        CatalogVerificationState::Verified,
    );

    require_equal(&rows.len(), &2, "configured catalog row count")?;
    let Some(first) = rows.first() else {
        bail!("configured projection omitted first row");
    };
    let Some(last) = rows.last() else {
        bail!("configured projection omitted last row");
    };
    require_equal(
        &first.channel_id,
        &configured_only_id,
        "canonical configured row order",
    )?;
    require_equal(
        &first.kind(),
        &ZoneKind::SequencerZone,
        "configured-only Zone kind",
    )?;
    require_equal(
        &first.settlement_link.status,
        &SettlementLinkStatus::Linked,
        "configured-only settlement state",
    )?;
    require_equal(
        &first.l1_channel.balance,
        &None,
        "configured-only L1 balance",
    )?;
    require_equal(&last.channel_id, &catalog_id, "catalog overlay row")?;
    require_equal(
        &last.settlement_link.source,
        &SettlementLinkSource::Configured,
        "catalog source overlay",
    )?;
    require_equal(
        &last.l2_zone().map(|l2| l2.configured_source_count),
        &Some(1),
        "configured source count",
    )?;
    let serialized = serde_json::to_string(&rows)?;
    if serialized.contains("127.0.0.1") || serialized.contains("http://") {
        bail!("Zone summary leaked Channel source target: {serialized}");
    }
    Ok(())
}

fn require(condition: bool, context: &str) -> Result<()> {
    if !condition {
        bail!("unexpected {context}");
    }
    Ok(())
}

fn require_equal<T>(actual: &T, expected: &T, context: &str) -> Result<()>
where
    T: std::fmt::Debug + PartialEq,
{
    if actual != expected {
        bail!("unexpected {context}: actual={actual:?}, expected={expected:?}");
    }
    Ok(())
}
