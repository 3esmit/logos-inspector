use std::collections::BTreeMap;

use anyhow::{Context as _, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde_json::{Value, json};

use super::*;
use crate::blockchain::channel_operations::classify_inscription;
use crate::inspection::catalog::{
    CatalogL1ChainSnapshot, CatalogL1ChainStatus, CatalogL1RangeRequest, CatalogL1SourceFuture,
    CatalogL1TimeStatus, CatalogMetadata, ZoneCatalog,
};

const TESTNET_SEQUENCER_BLOCK: &str = "0gQAAAAAAADgBr/57T2VP8TvanoE/U28V0Cdzfe66q1YCY203VHHaPZH+D0d+RhX4Qtz8m7atlbEG6J5XguGFqEPUWLQ8+1kb3u3+Z4BAADGt772EW9LB3inITN2BUfOdP8fHmTlcvpFP45NvGI01KYmibPzb/BkLygy6fTsHB4Oc4XoVVMp+k7Rp8xdjpgGAQAAAADiMVjm57Su7ujTA26v18dZ5R2KCU2Ce5JXELoh3v+PRgMAAAAvTEVaL0Nsb2NrUHJvZ3JhbUFjY291bnQvMDAwMDAwMS9MRVovQ2xvY2tQcm9ncmFtQWNjb3VudC8wMDAwMDEwL0xFWi9DbG9ja1Byb2dyYW1BY2NvdW50LzAwMDAwNTAAAAAAAgAAAG97t/meAQAAAAAAAAI=";

#[test]
fn latest_evidence_cache_preserves_evidence_id_tie_break() -> Result<()> {
    let (_directory, catalog) = test_catalog(100)?;
    let lower = tied_evidence("evidence-a");
    let higher = tied_evidence("evidence-z");
    let evidence = BTreeMap::from([
        (lower.evidence_id.clone(), lower.clone()),
        (higher.evidence_id.clone(), higher.clone()),
    ]);
    let (primary, configuration) = latest_evidence_ids(&evidence);
    ensure!(
        primary.get(&lower.channel_id) == Some(&higher.evidence_id)
            && configuration.get(&lower.channel_id) == Some(&higher.evidence_id),
        "snapshot cache did not preserve BTreeMap evidence-id tie-break"
    );

    let mut working = WorkingCatalog::from_snapshot(&catalog.snapshot()?);
    insert_evidence(&mut working, higher.clone())?;
    insert_evidence(&mut working, lower.clone())?;
    ensure!(
        working.latest_primary_evidence.get(&lower.channel_id) == Some(&higher.evidence_id)
            && working.latest_configuration_evidence.get(&lower.channel_id)
                == Some(&higher.evidence_id),
        "incremental cache replaced a higher evidence-id tie"
    );
    Ok(())
}

fn tied_evidence(evidence_id: &str) -> ZoneEvidenceReference {
    ZoneEvidenceReference {
        evidence_id: evidence_id.to_owned(),
        channel_id: id('8'),
        coverage_segment_id: "segment-test".to_owned(),
        l1_slot: 5,
        block_id: id('5'),
        transaction_hash: Some(id('a')),
        operation_index: 0,
        message_id: None,
        evidence_kind: ZoneEvidenceKind::ChannelConfiguration,
        evidence_use: CatalogEvidenceUse::PointSnapshot,
    }
}

#[test]
fn connected_page_commits_complete_catalog_and_channel_evidence() -> Result<()> {
    let (_directory, catalog) = test_catalog(100)?;
    let target = reference(10, 'a');
    let snapshot = prepare_and_commit(&catalog, target.clone(), context(1, 101)?)?;
    let channel_id = id('8');
    let config = config_transaction('c', &channel_id, '1');
    let sequencer =
        inscription_transaction('d', &channel_id, &id('4'), TESTNET_SEQUENCER_BLOCK, '1')?;
    let page = page(
        &target,
        vec![
            block_event(0, '0', 'f', Vec::new(), &target),
            block_event(5, '5', '0', vec![config], &target),
            block_event(10, 'a', '5', vec![sequencer], &target),
        ],
    );

    let (batch, remaining) =
        expect_commit(reduce_catalog_page(&snapshot, page, context(1, 102)?)?)?;
    ensure!(remaining.is_empty(), "connected page left events behind");
    let committed = catalog.commit_batch(batch)?;

    let frontier = committed.frontier.as_ref().context("frontier missing")?;
    ensure!(
        frontier.coverage_status == CatalogCoverageStatus::Complete,
        "connected target was not complete"
    );
    ensure!(committed.segments.len() == 1, "expected one segment");
    ensure!(committed.gaps.is_empty(), "connected scan created a gap");
    let zone = committed.zones.first().context("Zone missing")?;
    ensure!(zone.channel_id == channel_id, "wrong Zone identity");
    ensure!(
        zone.classification.channel_operations == 2
            && zone.classification.recognized_l2_blocks == 1
            && zone.classification.raw_inscriptions == 0
            && !zone.classification.conflicting_evidence,
        "wrong Zone classification counters: {:?}",
        zone.classification
    );
    ensure!(zone.evidence_count == 2, "wrong evidence count");
    ensure!(
        zone.snapshot_provenance.origin == CatalogSnapshotOrigin::FullConfiguration,
        "configuration snapshot provenance was lost"
    );
    let committee = zone
        .sequencer_committee
        .as_ref()
        .context("committee missing")?;
    ensure!(committee.members == vec![id('1')], "wrong committee");
    ensure!(
        prepare_catalog_catch_up(&committed, target, context(1, 103)?)?.is_none(),
        "same target produced a redundant batch"
    );
    Ok(())
}

#[test]
fn advancing_target_resets_completion_without_losing_catalog_rows() -> Result<()> {
    let (_directory, catalog) = complete_empty_catalog()?;
    let snapshot = catalog.snapshot()?;

    let batch = prepare_catalog_catch_up(&snapshot, reference(12, 'c'), context(2, 104)?)?
        .context("advanced target should produce a batch")?;
    let committed = catalog.commit_batch(batch)?;

    ensure!(committed.zones.is_empty(), "empty catalog gained Zones");
    ensure!(
        committed
            .segments
            .first()
            .is_some_and(|segment| !segment.reaches_target_lib),
        "old segment still claims new target"
    );
    ensure!(
        committed
            .frontier
            .as_ref()
            .is_some_and(|frontier| frontier.coverage_status == CatalogCoverageStatus::Rebuilding),
        "advanced target did not re-enter rebuilding"
    );
    Ok(())
}

#[test]
fn resumed_source_retargets_only_an_unfinished_traversal() -> Result<()> {
    let (_directory, catalog) = test_catalog(100)?;
    let previous_target = reference(20, 'e');
    let snapshot = prepare_and_commit(&catalog, previous_target.clone(), context(1, 101)?)?;
    let partial_page = page(
        &previous_target,
        vec![
            block_event(0, '0', 'f', Vec::new(), &previous_target),
            block_event(10, 'a', '0', Vec::new(), &previous_target),
        ],
    );
    let (batch, remaining) = expect_commit(reduce_catalog_page(
        &snapshot,
        partial_page,
        context(1, 102)?,
    )?)?;
    ensure!(remaining.is_empty(), "partial setup left events");
    let partial = catalog.commit_batch(batch)?;
    let resumed_target = reference(15, 'f');

    let error = prepare_catalog_catch_up(&partial, resumed_target.clone(), context(1, 103)?)
        .err()
        .context("same-run target rollback should remain rejected")?;
    ensure!(
        matches!(error, CatalogEngineError::SourceInconsistent(_)),
        "same-run target rollback returned wrong error: {error}"
    );

    let batch =
        prepare_resumed_catalog_catch_up(&partial, resumed_target.clone(), context(2, 104)?)?
            .context("resumed unfinished traversal should retarget")?;
    let resumed = catalog.commit_batch(batch)?;
    ensure!(
        resumed
            .traversal
            .as_ref()
            .and_then(|traversal| traversal.target_lib.as_ref())
            == Some(&resumed_target),
        "resumed traversal kept stale target"
    );
    ensure!(
        resumed
            .traversal
            .as_ref()
            .and_then(|traversal| traversal.ingestion_cursor.as_ref())
            == Some(&reference(10, 'a')),
        "resumed traversal moved its verified cursor"
    );

    let final_page = page(
        &resumed_target,
        vec![block_event(15, 'f', 'a', Vec::new(), &resumed_target)],
    );
    let (batch, remaining) =
        expect_commit(reduce_catalog_page(&resumed, final_page, context(2, 105)?)?)?;
    ensure!(remaining.is_empty(), "resumed page left events");
    let completed = catalog.commit_batch(batch)?;
    ensure!(
        completed
            .traversal
            .as_ref()
            .and_then(|traversal| traversal.ingestion_cursor.as_ref())
            == Some(&resumed_target),
        "resumed traversal did not reach reconciled target"
    );
    Ok(())
}

#[test]
fn resumed_source_rejects_target_behind_completed_cursor() -> Result<()> {
    let (_directory, catalog) = complete_empty_catalog()?;
    let completed = catalog.snapshot()?;
    let error = prepare_resumed_catalog_catch_up(&completed, reference(8, '8'), context(2, 104)?)
        .err()
        .context("completed target rollback should fail")?;
    ensure!(
        matches!(error, CatalogEngineError::SourceInconsistent(_)),
        "completed target rollback returned wrong error: {error}"
    );
    Ok(())
}

#[test]
fn bounded_repair_connects_page_without_opening_gap() -> Result<()> {
    let (_directory, catalog, snapshot, target) = lower_segment_catalog()?;
    let pending = page(
        &target,
        vec![
            block_event(8, '8', '7', Vec::new(), &target),
            block_event(10, 'a', '8', Vec::new(), &target),
        ],
    );
    let (request, pending_events) =
        expect_repair(reduce_catalog_page(&snapshot, pending, context(1, 104)?)?)?;
    let source = MockSource::with_blocks([block(7, '7', '3', Vec::new())]);
    let runtime = tokio::runtime::Runtime::new()?;

    let outcome = runtime.block_on(repair_catalog_ancestry(&source, &request))?;
    let (batch, remaining) = expect_commit(reduce_catalog_repair(
        &snapshot,
        &request,
        pending_events,
        outcome,
        context(1, 104)?,
    )?)?;
    ensure!(remaining.is_empty(), "repair left connected events");
    let committed = catalog.commit_batch(batch)?;

    ensure!(
        committed.segments.len() == 1,
        "repair did not extend segment"
    );
    ensure!(committed.gaps.is_empty(), "repair created a gap");
    ensure!(
        committed
            .frontier
            .as_ref()
            .is_some_and(|frontier| frontier.coverage_status == CatalogCoverageStatus::Complete),
        "repaired target was not complete"
    );
    Ok(())
}

#[test]
fn reducer_rejects_repair_blocks_outside_requested_boundaries() -> Result<()> {
    let (_directory, _catalog, snapshot, target) = lower_segment_catalog()?;
    let pending = page(&target, vec![block_event(8, '8', '7', Vec::new(), &target)]);
    let (request, pending_events) =
        expect_repair(reduce_catalog_page(&snapshot, pending, context(1, 104)?)?)?;
    let fabricated = CatalogAncestryRepairOutcome::Connected {
        recovered_blocks: vec![block(9, '7', '3', Vec::new())],
    };

    let error = reduce_catalog_repair(
        &snapshot,
        &request,
        pending_events,
        fabricated,
        context(1, 104)?,
    )
    .err()
    .context("out-of-bound repair should fail")?;
    ensure!(
        matches!(error, CatalogEngineError::SourceInconsistent(_)),
        "wrong out-of-bound repair error: {error}"
    );
    Ok(())
}

#[test]
fn page_commits_only_maximal_linked_prefix_and_retains_remainder() -> Result<()> {
    let (_directory, catalog, snapshot, target) = lower_segment_catalog()?;
    let page = page(
        &target,
        vec![
            block_event(5, '5', '3', Vec::new(), &target),
            block_event(8, '8', '7', Vec::new(), &target),
            block_event(10, 'a', '8', Vec::new(), &target),
        ],
    );

    let (batch, remaining) =
        expect_commit(reduce_catalog_page(&snapshot, page, context(1, 104)?)?)?;
    ensure!(
        remaining.len() == 2,
        "disconnected page remainder was consumed"
    );
    let committed = catalog.commit_batch(batch)?;
    ensure!(
        committed
            .traversal
            .as_ref()
            .and_then(|traversal| traversal.ingestion_cursor.as_ref())
            == Some(&reference(5, '5')),
        "cursor advanced beyond linked prefix"
    );

    let (request, pending) = expect_repair(reduce_catalog_page(
        &committed,
        CatalogL1RangePage { events: remaining },
        context(1, 105)?,
    )?)?;
    ensure!(pending.len() == 2, "repair did not retain full remainder");
    ensure!(
        request.lower_checkpoint == Some(reference(5, '5'))
            && request.upper_checkpoint == checkpoint(8, '8', '7'),
        "repair boundaries do not match linked-prefix break"
    );
    Ok(())
}

#[test]
fn unresolved_break_opens_gap_then_partial_repair_shrinks_and_merges_it() -> Result<()> {
    let (_directory, catalog, snapshot, target) = lower_segment_catalog()?;
    let pending = page(
        &target,
        vec![
            block_event(
                8,
                '8',
                '7',
                vec![raw_inscription_transaction('d', &id('8'), &id('4'), '1')],
                &target,
            ),
            block_event(10, 'a', '8', Vec::new(), &target),
        ],
    );
    let (request, pending_events) =
        expect_repair(reduce_catalog_page(&snapshot, pending, context(1, 104)?)?)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let missing_source = MockSource::default();
    let missing = runtime.block_on(repair_catalog_ancestry(&missing_source, &request))?;
    let (confirmation_request, confirmation_events, confirmation_outcome) =
        expect_gap_confirmation(reduce_catalog_repair(
            &snapshot,
            &request,
            pending_events,
            missing,
            context(1, 104)?,
        )?)?;
    let stale_confirmation = CatalogRepairConfirmation::new(
        reference(10, 'b'),
        confirmation_request.lower_checkpoint.clone(),
        None,
    );
    let error = confirm_catalog_repair_gap(
        &snapshot,
        &confirmation_request,
        confirmation_events.clone(),
        confirmation_outcome.clone(),
        &stale_confirmation,
        context(1, 104)?,
    )
    .err()
    .context("stale repair confirmation should fail")?;
    ensure!(
        matches!(error, CatalogEngineError::SourceInconsistent(_)),
        "wrong stale confirmation error: {error}"
    );
    let confirmation = CatalogRepairConfirmation::new(
        target.clone(),
        confirmation_request.lower_checkpoint.clone(),
        None,
    );
    let (batch, remaining) = expect_commit(confirm_catalog_repair_gap(
        &snapshot,
        &confirmation_request,
        confirmation_events,
        confirmation_outcome,
        &confirmation,
        context(1, 104)?,
    )?)?;
    ensure!(remaining.is_empty(), "gap opening left events");
    let gapped = catalog.commit_batch(batch)?;

    ensure!(gapped.segments.len() == 2, "gap did not split segments");
    ensure!(gapped.gaps.len() == 1, "gap record missing");
    ensure!(
        gapped
            .frontier
            .as_ref()
            .and_then(|frontier| frontier.checkpoint.as_ref())
            .is_some_and(|checkpoint| checkpoint.slot == 3),
        "connected frontier crossed gap"
    );
    ensure!(
        gapped
            .traversal
            .as_ref()
            .and_then(|traversal| traversal.ingestion_cursor.as_ref())
            == Some(&target),
        "ingestion cursor did not continue through upper segment"
    );
    let gapped_zone = gapped.zones.first().context("gapped Zone missing")?;
    ensure!(
        gapped_zone.snapshot_provenance.origin == CatalogSnapshotOrigin::ReplayDerived,
        "cross-gap evidence overstated configuration provenance"
    );

    let gap_id = gapped.gaps.first().context("gap missing")?.gap_id.clone();
    let partial_request = catalog_gap_repair_request(&gapped, &gap_id, context(1, 105)?)?;
    let partial_source = MockSource::with_blocks([block(7, '7', '6', Vec::new())]);
    let partial = runtime.block_on(repair_catalog_ancestry(&partial_source, &partial_request))?;
    let partial_confirmation = gap_confirmation(&gapped, &gap_id, None)?;
    let partial_batch = reduce_catalog_gap_repair(
        &gapped,
        &gap_id,
        partial,
        &partial_confirmation,
        context(1, 105)?,
    )?;
    let shrunk = catalog.commit_batch(partial_batch)?;
    let gap = shrunk.gaps.first().context("shrunk gap missing")?;
    ensure!(gap.required_parent_id == id('6'), "gap did not shrink");
    let upper = shrunk
        .segments
        .iter()
        .find(|segment| segment.segment_id == gap.upper_segment_id)
        .context("upper segment missing")?;
    ensure!(upper.floor.slot == 7, "upper segment floor did not move");

    let connect_request = catalog_gap_repair_request(&shrunk, &gap_id, context(1, 106)?)?;
    let connect_source = MockSource::with_blocks([block(6, '6', '3', Vec::new())]);
    let connected = runtime.block_on(repair_catalog_ancestry(&connect_source, &connect_request))?;
    let connect_confirmation = gap_confirmation(&shrunk, &gap_id, Some(checkpoint(10, 'a', '8')))?;
    let merged_batch = reduce_catalog_gap_repair(
        &shrunk,
        &gap_id,
        connected,
        &connect_confirmation,
        context(1, 106)?,
    )?;
    let merged = catalog.commit_batch(merged_batch)?;

    ensure!(merged.segments.len() == 1, "segments did not merge");
    ensure!(merged.gaps.is_empty(), "closed gap remained persisted");
    ensure!(
        merged
            .frontier
            .as_ref()
            .is_some_and(|frontier| frontier.coverage_status == CatalogCoverageStatus::Complete),
        "merged catalog did not regain completeness"
    );
    let segment_id = &merged
        .segments
        .first()
        .context("merged segment missing")?
        .segment_id;
    ensure!(
        merged
            .evidence
            .iter()
            .all(|evidence| &evidence.coverage_segment_id == segment_id),
        "merged evidence retained obsolete segment ids"
    );
    let merged_zone = merged.zones.first().context("merged Zone missing")?;
    ensure!(
        merged_zone.snapshot_provenance.origin == CatalogSnapshotOrigin::FullConfiguration,
        "merged evidence did not restore full configuration provenance"
    );
    Ok(())
}

#[test]
fn unavailable_initial_parent_creates_partial_prefix_without_internal_gap() -> Result<()> {
    let (_directory, catalog) = test_catalog(100)?;
    let target = reference(10, 'a');
    let snapshot = prepare_and_commit(&catalog, target.clone(), context(1, 101)?)?;
    let pending = page(
        &target,
        vec![
            block_event(5, '5', '4', Vec::new(), &target),
            block_event(10, 'a', '5', Vec::new(), &target),
        ],
    );
    let (request, pending_events) =
        expect_repair(reduce_catalog_page(&snapshot, pending, context(1, 102)?)?)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let outcome = runtime.block_on(repair_catalog_ancestry(&MockSource::default(), &request))?;
    let (confirmation_request, confirmation_events, confirmation_outcome) =
        expect_gap_confirmation(reduce_catalog_repair(
            &snapshot,
            &request,
            pending_events,
            outcome,
            context(1, 102)?,
        )?)?;
    let confirmation = CatalogRepairConfirmation::new(target.clone(), None, None);
    let (batch, _) = expect_commit(confirm_catalog_repair_gap(
        &snapshot,
        &confirmation_request,
        confirmation_events,
        confirmation_outcome,
        &confirmation,
        context(1, 102)?,
    )?)?;
    let committed = catalog.commit_batch(batch)?;

    let frontier = committed.frontier.as_ref().context("frontier missing")?;
    ensure!(
        frontier.prefix_status == CoveragePrefixStatus::Unavailable
            && frontier.coverage_floor == Some(5)
            && frontier.coverage_status == CatalogCoverageStatus::Partial,
        "unavailable prefix was overstated: {frontier:?}"
    );
    ensure!(committed.gaps.is_empty(), "prefix became internal gap");

    let request = catalog_prefix_repair_request(&committed, context(1, 103)?)?;
    let source = MockSource::with_blocks([
        block(0, '0', 'f', Vec::new()),
        block(4, '4', '0', Vec::new()),
    ]);
    let outcome = runtime.block_on(repair_catalog_ancestry(&source, &request))?;
    let upper_frontier = committed
        .frontier
        .as_ref()
        .and_then(|frontier| frontier.checkpoint.clone())
        .context("prefix repair frontier missing")?;
    let confirmation = CatalogRepairConfirmation::new(target, None, Some(upper_frontier));
    let batch = reduce_catalog_prefix_repair(&committed, outcome, &confirmation, context(1, 103)?)?
        .context("prefix repair should produce a batch")?;
    let repaired = catalog.commit_batch(batch)?;
    let frontier = repaired
        .frontier
        .as_ref()
        .context("repaired frontier missing")?;
    ensure!(
        frontier.prefix_status == CoveragePrefixStatus::Complete
            && frontier.coverage_floor == Some(0)
            && frontier.coverage_status == CatalogCoverageStatus::Complete,
        "prefix repair did not restore complete coverage: {frontier:?}"
    );
    Ok(())
}

#[test]
fn malformed_known_channel_operation_rejects_entire_reduction() -> Result<()> {
    let (_directory, catalog) = test_catalog(100)?;
    let target = reference(0, '0');
    let snapshot = prepare_and_commit(&catalog, target.clone(), context(1, 101)?)?;
    let malformed = json!({
        "mantle_tx": {
            "hash": id('c'),
            "ops": [{
                "opcode": 16,
                "payload": { "channel": id('8') }
            }]
        },
        "ops_proofs": []
    });
    let page = page(
        &target,
        vec![block_event(0, '0', 'f', vec![malformed], &target)],
    );

    let error = reduce_catalog_page(&snapshot, page, context(1, 102)?)
        .err()
        .context("malformed ChannelConfig should fail")?;

    ensure!(
        matches!(error, CatalogEngineError::InvalidBlock(_)),
        "wrong malformed operation error: {error}"
    );
    Ok(())
}

#[test]
fn inscription_decoder_distinguishes_valid_raw_and_hash_conflict() -> Result<()> {
    let bytes = BASE64_STANDARD.decode(TESTNET_SEQUENCER_BLOCK)?;
    let mut corrupt = bytes.clone();
    let hash_byte = corrupt
        .get_mut(40)
        .context("fixture is shorter than header hash")?;
    *hash_byte ^= 1;

    ensure!(
        classify_inscription(&bytes)? == InscriptionClassification::SequencerBlock,
        "valid Sequencer block was not recognized"
    );
    ensure!(
        classify_inscription(b"plain data")? == InscriptionClassification::Raw,
        "raw inscription was not preserved"
    );
    ensure!(
        classify_inscription(&corrupt)? == InscriptionClassification::Conflicting,
        "hash-conflicting block was not marked conflicting"
    );
    Ok(())
}

#[test]
fn exact_evidence_payload_revalidates_catalog_identity() -> Result<()> {
    let channel_id = id('8');
    let transaction_hash = id('c');
    let source_block = block(
        3,
        '3',
        '2',
        vec![raw_inscription_transaction('c', &channel_id, &id('0'), '1')],
    );
    let reference = ZoneEvidenceReference {
        evidence_id: format!("evidence-{transaction_hash}-0-raw_inscription"),
        channel_id: channel_id.clone(),
        coverage_segment_id: "segment-test".to_owned(),
        l1_slot: 3,
        block_id: id('3'),
        transaction_hash: Some(transaction_hash),
        operation_index: 0,
        message_id: None,
        evidence_kind: ZoneEvidenceKind::RawInscription,
        evidence_use: CatalogEvidenceUse::Presence,
    };

    let payload = extract_catalog_evidence_payload(&source_block, &reference)?;
    ensure!(
        payload.opcode == 0x11
            && payload.format == CatalogEvidencePayloadFormat::Bytes
            && payload.bytes == b"plain data",
        "unexpected exact evidence payload: {payload:?}"
    );

    let mut tampered = reference;
    tampered.channel_id = id('9');
    let error = extract_catalog_evidence_payload(&source_block, &tampered)
        .err()
        .context("tampered evidence reference should fail")?;
    ensure!(
        matches!(error, CatalogEngineError::SourceInconsistent(_)),
        "tampered evidence returned wrong error: {error}"
    );
    Ok(())
}

fn complete_empty_catalog() -> Result<(tempfile::TempDir, ZoneCatalog)> {
    let (directory, catalog) = test_catalog(100)?;
    let target = reference(10, 'a');
    let snapshot = prepare_and_commit(&catalog, target.clone(), context(1, 101)?)?;
    let page = page(
        &target,
        vec![
            block_event(0, '0', 'f', Vec::new(), &target),
            block_event(10, 'a', '0', Vec::new(), &target),
        ],
    );
    let (batch, remaining) =
        expect_commit(reduce_catalog_page(&snapshot, page, context(1, 102)?)?)?;
    ensure!(remaining.is_empty(), "complete setup left events");
    catalog.commit_batch(batch)?;
    Ok((directory, catalog))
}

fn lower_segment_catalog() -> Result<(
    tempfile::TempDir,
    ZoneCatalog,
    CatalogSnapshot,
    CatalogBlockReference,
)> {
    let (directory, catalog) = test_catalog(100)?;
    let target = reference(10, 'a');
    let snapshot = prepare_and_commit(&catalog, target.clone(), context(1, 101)?)?;
    let lower = page(
        &target,
        vec![
            block_event(0, '0', 'f', Vec::new(), &target),
            block_event(
                3,
                '3',
                '0',
                vec![config_transaction('c', &id('8'), '1')],
                &target,
            ),
        ],
    );
    let (batch, remaining) =
        expect_commit(reduce_catalog_page(&snapshot, lower, context(1, 102)?)?)?;
    ensure!(remaining.is_empty(), "lower setup left events");
    let snapshot = catalog.commit_batch(batch)?;
    Ok((directory, catalog, snapshot, target))
}

fn test_catalog(created_at: u64) -> Result<(tempfile::TempDir, ZoneCatalog)> {
    let directory = tempfile::tempdir()?;
    let metadata = CatalogMetadata::new(
        NetworkScope::GenesisId {
            genesis_id: id('0'),
        },
        created_at,
    )?;
    let catalog = ZoneCatalog::create(directory.path().join("catalog.redb"), metadata)?;
    Ok((directory, catalog))
}

fn prepare_and_commit(
    catalog: &ZoneCatalog,
    target: CatalogBlockReference,
    context: CatalogEngineContext,
) -> Result<CatalogSnapshot> {
    let snapshot = catalog.snapshot()?;
    let batch = prepare_catalog_catch_up(&snapshot, target, context)?
        .context("catch-up preparation should produce a batch")?;
    Ok(catalog.commit_batch(batch)?)
}

fn page(_target: &CatalogBlockReference, events: Vec<CatalogL1BlockEvent>) -> CatalogL1RangePage {
    CatalogL1RangePage { events }
}

fn block_event(
    slot: u64,
    block_id: char,
    parent_id: char,
    transactions: Vec<Value>,
    target: &CatalogBlockReference,
) -> CatalogL1BlockEvent {
    CatalogL1BlockEvent {
        block: block(slot, block_id, parent_id, transactions),
        snapshot: CatalogL1ChainSnapshot {
            tip: reference(target.slot.saturating_add(5), 'f'),
            lib: target.clone(),
        },
    }
}

fn block(slot: u64, block_id: char, parent_id: char, transactions: Vec<Value>) -> CatalogL1Block {
    CatalogL1Block {
        checkpoint: checkpoint(slot, block_id, parent_id),
        payload: json!({
            "header": {
                "id": id(block_id),
                "parent_block": id(parent_id),
                "slot": slot
            },
            "transactions": transactions
        }),
    }
}

fn config_transaction(tx: char, channel_id: &str, key: char) -> Value {
    json!({
        "mantle_tx": {
            "hash": id(tx),
            "ops": [{
                "opcode": 16,
                "payload": {
                    "channel": channel_id,
                    "keys": [id(key)],
                    "posting_timeframe": 10,
                    "posting_timeout": 20,
                    "configuration_threshold": 1,
                    "withdraw_threshold": 1
                }
            }]
        },
        "ops_proofs": []
    })
}

fn inscription_transaction(
    tx: char,
    channel_id: &str,
    parent: &str,
    inscription_base64: &str,
    signer: char,
) -> Result<Value> {
    let inscription = BASE64_STANDARD.decode(inscription_base64)?;
    Ok(json!({
        "mantle_tx": {
            "hash": id(tx),
            "ops": [{
                "opcode": 17,
                "payload": {
                    "channel_id": channel_id,
                    "inscription": hex::encode(inscription),
                    "parent": parent,
                    "signer": id(signer)
                }
            }]
        },
        "ops_proofs": []
    }))
}

fn raw_inscription_transaction(tx: char, channel_id: &str, parent: &str, signer: char) -> Value {
    json!({
        "mantle_tx": {
            "hash": id(tx),
            "ops": [{
                "opcode": 17,
                "payload": {
                    "channel_id": channel_id,
                    "inscription": hex::encode(b"plain data"),
                    "parent": parent,
                    "signer": id(signer)
                }
            }]
        },
        "ops_proofs": []
    })
}

fn checkpoint(slot: u64, block_id: char, parent_id: char) -> CatalogBlockCheckpoint {
    CatalogBlockCheckpoint {
        slot,
        block_id: id(block_id),
        parent_id: id(parent_id),
    }
}

fn reference(slot: u64, block_id: char) -> CatalogBlockReference {
    CatalogBlockReference {
        slot,
        block_id: id(block_id),
    }
}

fn id(value: char) -> String {
    value.to_string().repeat(64)
}

fn context(
    source_revision: u64,
    updated_at_unix: u64,
) -> CatalogEngineResult<CatalogEngineContext> {
    CatalogEngineContext::with_repair_block_limit(source_revision, updated_at_unix, 8)
}

fn gap_confirmation(
    snapshot: &CatalogSnapshot,
    gap_id: &str,
    upper_frontier: Option<CatalogBlockCheckpoint>,
) -> Result<CatalogRepairConfirmation> {
    let gap = snapshot
        .gaps
        .iter()
        .find(|gap| gap.gap_id == gap_id)
        .context("gap confirmation boundary missing")?;
    let target = snapshot
        .traversal
        .as_ref()
        .and_then(|traversal| traversal.target_lib.clone())
        .context("gap confirmation target missing")?;
    Ok(CatalogRepairConfirmation::new(
        target,
        Some(gap.lower_checkpoint.clone()),
        upper_frontier,
    ))
}

fn expect_commit(
    reduction: CatalogPageReduction,
) -> Result<(CatalogBatch, Vec<CatalogL1BlockEvent>)> {
    match reduction {
        CatalogPageReduction::Commit {
            batch,
            remaining_events,
        } => Ok((*batch, remaining_events)),
        other => bail!("expected commit reduction, got {other:?}"),
    }
}

fn expect_repair(
    reduction: CatalogPageReduction,
) -> Result<(CatalogAncestryRepairRequest, Vec<CatalogL1BlockEvent>)> {
    match reduction {
        CatalogPageReduction::RepairRequired {
            request,
            pending_events,
        } => Ok((request, pending_events)),
        other => bail!("expected repair reduction, got {other:?}"),
    }
}

fn expect_gap_confirmation(
    reduction: CatalogPageReduction,
) -> Result<(
    CatalogAncestryRepairRequest,
    Vec<CatalogL1BlockEvent>,
    CatalogAncestryRepairOutcome,
)> {
    match reduction {
        CatalogPageReduction::GapConfirmationRequired {
            request,
            pending_events,
            outcome,
        } => Ok((request, pending_events, *outcome)),
        other => bail!("expected gap confirmation reduction, got {other:?}"),
    }
}

#[derive(Default)]
struct MockSource {
    blocks: BTreeMap<String, CatalogL1Block>,
}

impl MockSource {
    fn with_blocks(blocks: impl IntoIterator<Item = CatalogL1Block>) -> Self {
        Self {
            blocks: blocks
                .into_iter()
                .map(|block| (block.checkpoint.block_id.clone(), block))
                .collect(),
        }
    }
}

impl CatalogL1Source for MockSource {
    fn chain_status(&self) -> CatalogL1SourceFuture<'_, CatalogL1ChainStatus> {
        Box::pin(async {
            Err(CatalogL1SourceError::InvalidRequest(
                "chain status is not used by repair test".to_owned(),
            ))
        })
    }

    fn time_status(&self) -> CatalogL1SourceFuture<'_, CatalogL1TimeStatus> {
        Box::pin(async {
            Err(CatalogL1SourceError::InvalidRequest(
                "time status is not used by repair test".to_owned(),
            ))
        })
    }

    fn finalized_range(
        &self,
        _request: CatalogL1RangeRequest,
    ) -> CatalogL1SourceFuture<'_, CatalogL1RangePage> {
        Box::pin(async {
            Err(CatalogL1SourceError::InvalidRequest(
                "range is not used by repair test".to_owned(),
            ))
        })
    }

    fn block(&self, block_id: String) -> CatalogL1SourceFuture<'_, Option<CatalogL1Block>> {
        Box::pin(async move { Ok(self.blocks.get(&block_id).cloned()) })
    }
}
