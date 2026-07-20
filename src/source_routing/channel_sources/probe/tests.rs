use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use anyhow::{Result, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use common::block::{BedrockStatus, Block};

use super::*;

const TESTNET_LEGACY_BLOCK_1234: &str = "0gQAAAAAAADgBr/57T2VP8TvanoE/U28V0Cdzfe66q1YCY203VHHaPZH+D0d+RhX4Qtz8m7atlbEG6J5XguGFqEPUWLQ8+1kb3u3+Z4BAADGt772EW9LB3inITN2BUfOdP8fHmTlcvpFP45NvGI01KYmibPzb/BkLygy6fTsHB4Oc4XoVVMp+k7Rp8xdjpgGAQAAAADiMVjm57Su7ujTA26v18dZ5R2KCU2Ce5JXELoh3v+PRgMAAAAvTEVaL0Nsb2NrUHJvZ3JhbUFjY291bnQvMDAwMDAwMS9MRVovQ2xvY2tQcm9ncmFtQWNjb3VudC8wMDAwMDEwL0xFWi9DbG9ja1Byb2dyYW1BY2NvdW50LzAwMDAwNTAAAAAAAgAAAG97t/meAQAAAAAAAAI=";

struct HeadRaceTransport {
    head_calls: AtomicUsize,
    retry_block_present: bool,
}

impl HeadRaceTransport {
    fn new(retry_block_present: bool) -> Self {
        Self {
            head_calls: AtomicUsize::new(0),
            retry_block_present,
        }
    }
}

impl ChannelSourceProbeTransport for HeadRaceTransport {
    fn sequencer_health(self: Arc<Self>, _target: ChannelSourceTarget) -> TransportFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn sequencer_channel_id(
        self: Arc<Self>,
        _target: ChannelSourceTarget,
    ) -> TransportFuture<String> {
        Box::pin(async { Ok("8".repeat(64)) })
    }

    fn sequencer_head_id(self: Arc<Self>, _target: ChannelSourceTarget) -> TransportFuture<u64> {
        let call = self.head_calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { Ok(if call == 0 { 41 } else { 42 }) })
    }

    fn sequencer_block(
        self: Arc<Self>,
        _target: ChannelSourceTarget,
        block_id: u64,
    ) -> TransportFuture<Option<ChannelSourceBlock>> {
        let present = block_id == 42 && self.retry_block_present;
        Box::pin(async move {
            Ok(present.then(|| ChannelSourceBlock {
                block_id,
                header_hash: "a".repeat(64),
                parent_hash: Some("b".repeat(64)),
            }))
        })
    }

    fn sequencer_block_bytes(
        self: Arc<Self>,
        _target: ChannelSourceTarget,
        _block_id: u64,
    ) -> TransportFuture<Option<Vec<u8>>> {
        Box::pin(async { Ok(None) })
    }

    fn indexer_health(
        self: Arc<Self>,
        _target: ChannelSourceTarget,
    ) -> TransportFuture<Option<String>> {
        Box::pin(async { Ok(None) })
    }

    fn indexer_finalized_head_id(
        self: Arc<Self>,
        _target: ChannelSourceTarget,
    ) -> TransportFuture<Option<u64>> {
        Box::pin(async { Ok(Some(40)) })
    }

    fn indexer_block(
        self: Arc<Self>,
        _target: ChannelSourceTarget,
        block_id: u64,
    ) -> TransportFuture<Option<ChannelSourceBlock>> {
        Box::pin(async move {
            Ok(Some(ChannelSourceBlock {
                block_id,
                header_hash: "a".repeat(64),
                parent_hash: Some("b".repeat(64)),
            }))
        })
    }
}

struct AttestationTransport {
    identity: Result<String, ChannelSourceProbeFailure>,
    block: Result<Option<Vec<u8>>, ChannelSourceProbeFailure>,
    block_calls: AtomicUsize,
    block_ids: Mutex<Vec<u64>>,
}

impl AttestationTransport {
    fn new(
        identity: Result<String, ChannelSourceProbeFailure>,
        block: Result<Option<Vec<u8>>, ChannelSourceProbeFailure>,
    ) -> Self {
        Self {
            identity,
            block,
            block_calls: AtomicUsize::new(0),
            block_ids: Mutex::new(Vec::new()),
        }
    }

    fn requested_block_ids(&self) -> Result<Vec<u64>> {
        self.block_ids
            .lock()
            .map(|ids| ids.clone())
            .map_err(|_| anyhow::anyhow!("attestation block-id lock poisoned"))
    }
}

impl ChannelSourceProbeTransport for AttestationTransport {
    fn sequencer_health(self: Arc<Self>, _target: ChannelSourceTarget) -> TransportFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn sequencer_channel_id(
        self: Arc<Self>,
        _target: ChannelSourceTarget,
    ) -> TransportFuture<String> {
        let identity = self.identity.clone();
        Box::pin(async move { identity })
    }

    fn sequencer_head_id(self: Arc<Self>, _target: ChannelSourceTarget) -> TransportFuture<u64> {
        Box::pin(async { Ok(0) })
    }

    fn sequencer_block(
        self: Arc<Self>,
        _target: ChannelSourceTarget,
        _block_id: u64,
    ) -> TransportFuture<Option<ChannelSourceBlock>> {
        Box::pin(async { Ok(None) })
    }

    fn sequencer_block_bytes(
        self: Arc<Self>,
        _target: ChannelSourceTarget,
        block_id: u64,
    ) -> TransportFuture<Option<Vec<u8>>> {
        self.block_calls.fetch_add(1, Ordering::SeqCst);
        let block = self.block.clone();
        let recorded = self
            .block_ids
            .lock()
            .map_err(|_| probe_failure(ChannelSourceFailureKind::Protocol))
            .map(|mut block_ids| block_ids.push(block_id));
        Box::pin(async move {
            recorded?;
            block
        })
    }

    fn indexer_health(
        self: Arc<Self>,
        _target: ChannelSourceTarget,
    ) -> TransportFuture<Option<String>> {
        Box::pin(async { Ok(None) })
    }

    fn indexer_finalized_head_id(
        self: Arc<Self>,
        _target: ChannelSourceTarget,
    ) -> TransportFuture<Option<u64>> {
        Box::pin(async { Ok(None) })
    }

    fn indexer_block(
        self: Arc<Self>,
        _target: ChannelSourceTarget,
        _block_id: u64,
    ) -> TransportFuture<Option<ChannelSourceBlock>> {
        Box::pin(async { Ok(None) })
    }
}

#[tokio::test]
async fn direct_identity_attestation_skips_legacy_block_fallback() -> Result<()> {
    let expected_channel_id = "8".repeat(64);
    let anchor_loads = Arc::new(AtomicUsize::new(0));
    let deferred_anchor = legacy_anchor(fixture_block_bytes()?, Arc::new(|| true))?;
    let loads = anchor_loads.clone();
    let transport = Arc::new(AttestationTransport::new(
        Ok(expected_channel_id.clone()),
        Ok(Some(fixture_block_bytes()?)),
    ));

    let attestation = attest_sequencer_target(
        transport.clone(),
        rpc_target(),
        SequencerLegacyAnchorState::deferred(move || {
            let anchor = deferred_anchor.clone();
            let loads = loads.clone();
            async move {
                loads.fetch_add(1, Ordering::SeqCst);
                Ok(SequencerLegacyAnchorState::Available(anchor))
            }
        }),
    )
    .await?;

    ensure!(
        attestation.channel_id == expected_channel_id,
        "direct identity changed"
    );
    ensure!(
        attestation.basis == SequencerAttestationBasis::RpcReported {},
        "direct identity did not record RPC-reported basis"
    );
    ensure!(
        anchor_loads.load(Ordering::SeqCst) == 0
            && transport.block_calls.load(Ordering::SeqCst) == 0,
        "direct identity needlessly acquired a legacy anchor"
    );
    Ok(())
}

#[tokio::test]
async fn unsupported_identity_accepts_matching_finalized_l1_anchor() -> Result<()> {
    let bytes = fixture_block_bytes()?;
    let expected_basis = finalized_basis(&bytes)?;
    let anchor_loads = Arc::new(AtomicUsize::new(0));
    let loads = anchor_loads.clone();
    let deferred_anchor = SequencerLegacyAnchor::new(
        "8".repeat(64),
        expected_basis.clone(),
        bytes.clone(),
        Arc::new(|| true),
    );
    let transport = Arc::new(AttestationTransport::new(
        Err(probe_failure(ChannelSourceFailureKind::Unsupported)),
        Ok(Some(bytes.clone())),
    ));

    let attestation = attest_sequencer_target(
        transport.clone(),
        rpc_target(),
        SequencerLegacyAnchorState::deferred(move || {
            let anchor = deferred_anchor.clone();
            let loads = loads.clone();
            async move {
                loads.fetch_add(1, Ordering::SeqCst);
                Ok(SequencerLegacyAnchorState::Available(anchor))
            }
        }),
    )
    .await?;

    ensure!(
        attestation.channel_id == "8".repeat(64),
        "legacy attestation changed Channel identity"
    );
    ensure!(
        attestation.basis == expected_basis,
        "legacy attestation discarded finalized L1 basis"
    );
    ensure!(
        anchor_loads.load(Ordering::SeqCst) == 1
            && transport.block_calls.load(Ordering::SeqCst) == 1,
        "legacy attestation did not acquire one anchor and candidate block"
    );
    Ok(())
}

#[tokio::test]
async fn copied_legacy_block_never_becomes_rpc_reported_identity() -> Result<()> {
    let bytes = fixture_block_bytes()?;
    let expected_basis = finalized_basis(&bytes)?;

    for channel_id in ["8".repeat(64), "9".repeat(64)] {
        let transport = Arc::new(AttestationTransport::new(
            Err(probe_failure(ChannelSourceFailureKind::Unsupported)),
            Ok(Some(bytes.clone())),
        ));
        let attestation = attest_sequencer_target(
            transport,
            rpc_target(),
            SequencerLegacyAnchorState::Available(SequencerLegacyAnchor::new(
                channel_id.clone(),
                expected_basis.clone(),
                bytes.clone(),
                Arc::new(|| true),
            )),
        )
        .await?;

        ensure!(
            attestation.channel_id == channel_id
                && matches!(
                    attestation.basis,
                    SequencerAttestationBasis::UserTrustedFinalizedL1Evidence(_)
                ),
            "copied legacy history was mislabeled as RPC-reported identity"
        );
    }
    Ok(())
}

#[tokio::test]
async fn legacy_attestation_allows_bedrock_status_only_difference() -> Result<()> {
    let anchor_bytes = fixture_block_bytes()?;
    let mut candidate = borsh::from_slice::<Block>(&anchor_bytes)?;
    candidate.bedrock_status = BedrockStatus::Pending;
    let candidate_bytes = borsh::to_vec(&candidate)?;
    let transport = Arc::new(AttestationTransport::new(
        Err(probe_failure(ChannelSourceFailureKind::Unsupported)),
        Ok(Some(candidate_bytes)),
    ));

    let attestation = attest_sequencer_target(
        transport,
        rpc_target(),
        SequencerLegacyAnchorState::Available(legacy_anchor(anchor_bytes, Arc::new(|| true))?),
    )
    .await?;

    ensure!(
        attestation.channel_id == "8".repeat(64),
        "status-only difference changed legacy attestation"
    );
    Ok(())
}

#[tokio::test]
async fn legacy_fallback_runs_only_for_unsupported_identity() -> Result<()> {
    for kind in [
        ChannelSourceFailureKind::Timeout,
        ChannelSourceFailureKind::Unavailable,
        ChannelSourceFailureKind::Protocol,
        ChannelSourceFailureKind::Incomplete,
    ] {
        let transport = Arc::new(AttestationTransport::new(
            Err(probe_failure(kind)),
            Ok(Some(fixture_block_bytes()?)),
        ));
        let result = attest_sequencer_target(
            transport.clone(),
            rpc_target(),
            SequencerLegacyAnchorState::Available(legacy_anchor(
                fixture_block_bytes()?,
                Arc::new(|| true),
            )?),
        )
        .await;
        let Err(error) = result else {
            anyhow::bail!("{kind:?} identity failure unexpectedly used legacy fallback");
        };
        ensure!(
            error.kind == kind,
            "identity failure classification changed"
        );
        ensure!(
            transport.block_calls.load(Ordering::SeqCst) == 0,
            "non-capability identity failure fetched a legacy block"
        );
    }

    let transport = Arc::new(AttestationTransport::new(
        Err(probe_failure(ChannelSourceFailureKind::Unsupported)),
        Ok(Some(fixture_block_bytes()?)),
    ));
    let result = attest_sequencer_target(
        transport.clone(),
        rpc_target(),
        SequencerLegacyAnchorState::Missing,
    )
    .await;
    let Err(error) = result else {
        anyhow::bail!("unsupported identity without anchor unexpectedly succeeded");
    };
    ensure!(
        error.kind == ChannelSourceFailureKind::Unsupported,
        "missing anchor changed unsupported classification"
    );
    ensure!(
        transport.block_calls.load(Ordering::SeqCst) == 0,
        "missing anchor triggered a candidate block fetch"
    );

    let unavailable_transport = Arc::new(AttestationTransport::new(
        Err(probe_failure(ChannelSourceFailureKind::Unsupported)),
        Ok(Some(fixture_block_bytes()?)),
    ));
    let result = attest_sequencer_target(
        unavailable_transport.clone(),
        rpc_target(),
        SequencerLegacyAnchorState::Unavailable,
    )
    .await;
    let Err(error) = result else {
        anyhow::bail!("unavailable legacy anchor unexpectedly succeeded");
    };
    ensure!(
        error.kind == ChannelSourceFailureKind::Unavailable,
        "unavailable legacy anchor did not stay retryable"
    );
    ensure!(
        unavailable_transport.block_calls.load(Ordering::SeqCst) == 0,
        "unavailable legacy anchor triggered a candidate block fetch"
    );
    Ok(())
}

#[tokio::test]
async fn missing_legacy_candidate_is_incomplete() -> Result<()> {
    let transport = Arc::new(AttestationTransport::new(
        Err(probe_failure(ChannelSourceFailureKind::Unsupported)),
        Ok(None),
    ));

    let result = attest_sequencer_target(
        transport,
        rpc_target(),
        SequencerLegacyAnchorState::Available(legacy_anchor(
            fixture_block_bytes()?,
            Arc::new(|| true),
        )?),
    )
    .await;

    let Err(error) = result else {
        anyhow::bail!("missing legacy candidate unexpectedly succeeded");
    };
    ensure!(
        error.kind == ChannelSourceFailureKind::Incomplete,
        "missing candidate was not incomplete"
    );
    Ok(())
}

#[tokio::test]
async fn legacy_candidate_mismatch_and_tampering_are_protocol_failures() -> Result<()> {
    let anchor_bytes = fixture_block_bytes()?;
    let mut signature_mismatch = borsh::from_slice::<Block>(&anchor_bytes)?;
    let Some(first_signature_byte) = signature_mismatch.header.signature.value.first_mut() else {
        anyhow::bail!("signature fixture was unexpectedly empty");
    };
    *first_signature_byte ^= 1;

    let mut tampered = borsh::from_slice::<Block>(&anchor_bytes)?;
    tampered.header.timestamp = tampered.header.timestamp.saturating_add(1);

    for candidate in [
        borsh::to_vec(&signature_mismatch)?,
        borsh::to_vec(&tampered)?,
    ] {
        let transport = Arc::new(AttestationTransport::new(
            Err(probe_failure(ChannelSourceFailureKind::Unsupported)),
            Ok(Some(candidate)),
        ));
        let result = attest_sequencer_target(
            transport,
            rpc_target(),
            SequencerLegacyAnchorState::Available(legacy_anchor(
                anchor_bytes.clone(),
                Arc::new(|| true),
            )?),
        )
        .await;
        let Err(error) = result else {
            anyhow::bail!("mismatched or tampered legacy candidate unexpectedly succeeded");
        };
        ensure!(
            error.kind == ChannelSourceFailureKind::Protocol,
            "mismatched or tampered candidate was not a protocol failure"
        );
    }
    Ok(())
}

#[tokio::test]
async fn legacy_anchor_must_match_its_persisted_finalized_l1_basis() -> Result<()> {
    let bytes = fixture_block_bytes()?;
    let mut basis = finalized_basis(&bytes)?;
    let SequencerAttestationBasis::UserTrustedFinalizedL1Evidence(proof) = &mut basis else {
        anyhow::bail!("legacy fixture did not create finalized L1 basis");
    };
    proof.l2_header_hash = "0".repeat(64);
    let transport = Arc::new(AttestationTransport::new(
        Err(probe_failure(ChannelSourceFailureKind::Unsupported)),
        Ok(Some(bytes.clone())),
    ));

    let result = attest_sequencer_target(
        transport.clone(),
        rpc_target(),
        SequencerLegacyAnchorState::Available(SequencerLegacyAnchor::new(
            "8".repeat(64),
            basis,
            bytes,
            Arc::new(|| true),
        )),
    )
    .await;

    let Err(error) = result else {
        anyhow::bail!("legacy anchor with mismatched proof basis unexpectedly succeeded");
    };
    ensure!(
        error.kind == ChannelSourceFailureKind::Protocol,
        "legacy anchor basis mismatch was not protocol failure"
    );
    ensure!(
        transport.block_calls.load(Ordering::SeqCst) == 0,
        "invalid legacy anchor basis triggered candidate fetch"
    );
    Ok(())
}

#[tokio::test]
async fn stale_fence_rejects_candidate_only_after_block_match() -> Result<()> {
    let bytes = fixture_block_bytes()?;
    let fence_calls = Arc::new(AtomicUsize::new(0));
    let observed_fence_calls = fence_calls.clone();
    let fence = Arc::new(move || {
        observed_fence_calls.fetch_add(1, Ordering::SeqCst);
        false
    });
    let transport = Arc::new(AttestationTransport::new(
        Err(probe_failure(ChannelSourceFailureKind::Unsupported)),
        Ok(Some(bytes.clone())),
    ));

    let result = attest_sequencer_target(
        transport.clone(),
        rpc_target(),
        SequencerLegacyAnchorState::Available(legacy_anchor(bytes, fence)?),
    )
    .await;

    let Err(error) = result else {
        anyhow::bail!("stale finalized L1 fence unexpectedly succeeded");
    };
    ensure!(
        error.kind == ChannelSourceFailureKind::Protocol,
        "stale finalized L1 fence was not a protocol failure"
    );
    ensure!(
        transport.block_calls.load(Ordering::SeqCst) == 1,
        "stale fence was evaluated without matching a candidate block"
    );
    ensure!(
        fence_calls.load(Ordering::SeqCst) == 1,
        "stale fence was not evaluated exactly once"
    );
    Ok(())
}

#[tokio::test]
async fn persisted_legacy_proof_supplies_runtime_identity_on_unsupported_rpc() -> Result<()> {
    let bytes = fixture_block_bytes()?;
    let proof = persisted_legacy_proof(&bytes)?;
    let expected_block_id = proof.l2_block_id;
    let transport = Arc::new(AttestationTransport::new(
        Err(probe_failure(ChannelSourceFailureKind::Unsupported)),
        Ok(Some(bytes)),
    ));

    let (stage, identity) =
        sequencer_identity(transport.clone(), rpc_target(), Some(proof.clone())).await;
    let identity = identity?;

    ensure!(
        stage == ChannelSourceProbeStage::EvidenceConsistency
            && identity.channel_id == proof.expected_channel_id
            && identity.basis
                == SequencerSourceIdentityBasis::FinalizedL1EvidenceMatched {
                    anchor_block_id: expected_block_id,
                },
        "persisted legacy proof changed runtime Channel identity"
    );
    ensure!(
        transport.requested_block_ids()? == vec![expected_block_id],
        "runtime proof did not fetch its pinned L2 block"
    );
    Ok(())
}

#[test]
fn evidence_identity_requires_head_at_or_beyond_anchor() -> Result<()> {
    let identity = || SequencerSourceIdentity {
        channel_id: "8".repeat(64),
        basis: SequencerSourceIdentityBasis::FinalizedL1EvidenceMatched {
            anchor_block_id: 10,
        },
    };
    let behind = validate_sequencer_identity_head(
        Ok(identity()),
        &Ok(ChannelSourceBlock {
            block_id: 9,
            header_hash: "a".repeat(64),
            parent_hash: Some("b".repeat(64)),
        }),
    );
    let Err(error) = behind else {
        anyhow::bail!("head behind finalized L1 evidence became eligible");
    };
    ensure!(
        error.kind == ChannelSourceFailureKind::Protocol,
        "behind-head evidence failure was not protocol-classified"
    );

    let unavailable = validate_sequencer_identity_head(
        Ok(identity()),
        &Err(probe_failure(ChannelSourceFailureKind::Unavailable)),
    );
    let Err(error) = unavailable else {
        anyhow::bail!("evidence identity without a head became eligible");
    };
    ensure!(
        error.kind == ChannelSourceFailureKind::Unavailable,
        "missing evidence head changed failure classification"
    );

    let matching = validate_sequencer_identity_head(
        Ok(identity()),
        &Ok(ChannelSourceBlock {
            block_id: 10,
            header_hash: "a".repeat(64),
            parent_hash: Some("b".repeat(64)),
        }),
    )?;
    ensure!(
        matching.channel_id == "8".repeat(64),
        "matching evidence head changed Channel mapping"
    );
    Ok(())
}

#[test]
fn full_probe_timeout_keeps_evidence_consistency_provenance() -> Result<()> {
    let ChannelSourceProbeOutput::Sequencer(output) =
        timed_out_output(ChannelSourceRole::Sequencer, true)
    else {
        anyhow::bail!("Sequencer timeout changed probe role");
    };
    ensure!(
        output.identity_stage == ChannelSourceProbeStage::EvidenceConsistency,
        "evidence-backed timeout was mislabeled as RPC identity failure"
    );
    Ok(())
}

#[tokio::test]
async fn runtime_identity_does_not_use_proof_without_unsupported_rpc() -> Result<()> {
    let bytes = fixture_block_bytes()?;
    let proof = persisted_legacy_proof(&bytes)?;
    let transport = Arc::new(AttestationTransport::new(
        Err(probe_failure(ChannelSourceFailureKind::Protocol)),
        Ok(Some(bytes)),
    ));

    let (stage, result) = sequencer_identity(transport.clone(), rpc_target(), Some(proof)).await;

    let Err(error) = result else {
        anyhow::bail!("protocol identity failure unexpectedly used persisted proof");
    };
    ensure!(
        stage == ChannelSourceProbeStage::ChannelIdentity
            && error.kind == ChannelSourceFailureKind::Protocol,
        "runtime identity failure classification changed"
    );
    ensure!(
        transport.block_calls.load(Ordering::SeqCst) == 0,
        "runtime identity fetched proof block for non-capability failure"
    );

    let no_proof_transport = Arc::new(AttestationTransport::new(
        Err(probe_failure(ChannelSourceFailureKind::Unsupported)),
        Ok(Some(fixture_block_bytes()?)),
    ));
    let result = sequencer_identity(no_proof_transport.clone(), rpc_target(), None).await;
    let (stage, result) = result;
    let Err(error) = result else {
        anyhow::bail!("unsupported runtime identity without proof unexpectedly succeeded");
    };
    ensure!(
        stage == ChannelSourceProbeStage::ChannelIdentity
            && error.kind == ChannelSourceFailureKind::Unsupported,
        "unsupported runtime identity changed without proof"
    );
    ensure!(
        no_proof_transport.block_calls.load(Ordering::SeqCst) == 0,
        "runtime identity fetched block without persisted proof"
    );

    let direct_transport = Arc::new(AttestationTransport::new(
        Ok("8".repeat(64)),
        Ok(Some(fixture_block_bytes()?)),
    ));
    let proof = persisted_legacy_proof(&fixture_block_bytes()?)?;
    let (stage, identity) =
        sequencer_identity(direct_transport.clone(), rpc_target(), Some(proof)).await;
    let identity = identity?;
    ensure!(
        stage == ChannelSourceProbeStage::ChannelIdentity
            && identity.basis == SequencerSourceIdentityBasis::RpcReported
            && direct_transport.block_calls.load(Ordering::SeqCst) == 0,
        "newly available RPC identity was mislabeled or fetched legacy evidence"
    );
    Ok(())
}

#[tokio::test]
async fn persisted_legacy_proof_missing_candidate_is_incomplete() -> Result<()> {
    let proof = persisted_legacy_proof(&fixture_block_bytes()?)?;
    let transport = Arc::new(AttestationTransport::new(
        Err(probe_failure(ChannelSourceFailureKind::Unsupported)),
        Ok(None),
    ));

    let (stage, result) = sequencer_identity(transport, rpc_target(), Some(proof)).await;

    let Err(error) = result else {
        anyhow::bail!("missing persisted proof block unexpectedly succeeded");
    };
    ensure!(
        stage == ChannelSourceProbeStage::EvidenceConsistency
            && error.kind == ChannelSourceFailureKind::Incomplete,
        "missing persisted proof block was not incomplete"
    );
    Ok(())
}

#[tokio::test]
async fn persisted_legacy_proof_rejects_exact_field_mismatch_and_tampering() -> Result<()> {
    let bytes = fixture_block_bytes()?;
    let proof = persisted_legacy_proof(&bytes)?;
    let mut mismatched_proofs = Vec::new();
    let mut block_id_mismatch = proof.clone();
    block_id_mismatch.l2_block_id = block_id_mismatch.l2_block_id.saturating_add(1);
    mismatched_proofs.push(block_id_mismatch);
    let mut hash_mismatch = proof.clone();
    hash_mismatch.l2_header_hash = "0".repeat(64);
    mismatched_proofs.push(hash_mismatch);
    let mut signature_mismatch = proof.clone();
    signature_mismatch.l2_signature = "0".repeat(128);
    mismatched_proofs.push(signature_mismatch);

    for mismatched in mismatched_proofs {
        let transport = Arc::new(AttestationTransport::new(
            Err(probe_failure(ChannelSourceFailureKind::Unsupported)),
            Ok(Some(bytes.clone())),
        ));
        let (stage, result) = sequencer_identity(transport, rpc_target(), Some(mismatched)).await;
        let Err(error) = result else {
            anyhow::bail!("mismatched persisted proof unexpectedly succeeded");
        };
        ensure!(
            stage == ChannelSourceProbeStage::EvidenceConsistency
                && error.kind == ChannelSourceFailureKind::Protocol,
            "persisted proof mismatch was not protocol failure"
        );
    }

    let mut tampered = borsh::from_slice::<Block>(&bytes)?;
    tampered.header.timestamp = tampered.header.timestamp.saturating_add(1);
    let transport = Arc::new(AttestationTransport::new(
        Err(probe_failure(ChannelSourceFailureKind::Unsupported)),
        Ok(Some(borsh::to_vec(&tampered)?)),
    ));
    let (stage, result) = sequencer_identity(transport, rpc_target(), Some(proof)).await;
    let Err(error) = result else {
        anyhow::bail!("tampered persisted proof block unexpectedly succeeded");
    };
    ensure!(
        stage == ChannelSourceProbeStage::EvidenceConsistency
            && error.kind == ChannelSourceFailureKind::Protocol,
        "tampered persisted proof block was not protocol failure"
    );
    Ok(())
}

#[tokio::test]
async fn sequencer_head_retries_head_and_block_pair_once() -> Result<()> {
    let transport = Arc::new(HeadRaceTransport::new(true));
    let block = sequencer_head(transport.clone(), rpc_target()).await?;

    ensure!(block.block_id == 42, "retry did not accept the moved head");
    ensure!(
        transport.head_calls.load(Ordering::SeqCst) == 2,
        "head pair was not retried exactly once"
    );
    Ok(())
}

#[tokio::test]
async fn sequencer_head_marks_second_missing_block_incomplete() -> Result<()> {
    let transport = Arc::new(HeadRaceTransport::new(false));
    let result = sequencer_head(transport.clone(), rpc_target()).await;

    let Err(error) = result else {
        anyhow::bail!("missing retry block unexpectedly completed the probe");
    };
    ensure!(
        error.kind == ChannelSourceFailureKind::Incomplete,
        "missing retry block did not produce incomplete state"
    );
    ensure!(
        transport.head_calls.load(Ordering::SeqCst) == 2,
        "head pair retried an unexpected number of times"
    );
    Ok(())
}

#[tokio::test]
async fn source_probe_records_independent_health_identity_and_head_facts() -> Result<()> {
    let transport = Arc::new(HeadRaceTransport::new(true));
    let output = probe_source(
        transport,
        ChannelSourceProbeRequest {
            network_scope: crate::inspection::NetworkScope::GenesisId {
                genesis_id: "ab".repeat(32),
            },
            channel_id: "01".repeat(32),
            source_config_revision: 1,
            source_id: source_id('1'),
            role: ChannelSourceRole::Sequencer,
            target: rpc_target(),
            legacy_proof: None,
        },
    )
    .await;

    let ChannelSourceProbeOutput::Sequencer(output) = output else {
        anyhow::bail!("Sequencer request returned Indexer output");
    };

    ensure!(
        matches!(output.health, ChannelSourceProbeFact::Observed(())),
        "health fact was not retained"
    );
    ensure!(
        matches!(
            output.channel_id,
            ChannelSourceProbeFact::Observed(ref identity)
                if identity.channel_id == "8".repeat(64)
                    && identity.basis == SequencerSourceIdentityBasis::RpcReported
        ),
        "Channel identity fact was not retained"
    );
    ensure!(
        matches!(
            output.head,
            ChannelSourceProbeFact::Observed(ref block) if block.block_id == 42
        ),
        "head fact was not retained"
    );
    Ok(())
}

#[tokio::test]
async fn indexer_probe_exposes_only_indexer_facts() -> Result<()> {
    let output = probe_source(
        Arc::new(HeadRaceTransport::new(true)),
        ChannelSourceProbeRequest {
            network_scope: crate::inspection::NetworkScope::GenesisId {
                genesis_id: "ab".repeat(32),
            },
            channel_id: "01".repeat(32),
            source_config_revision: 1,
            source_id: source_id('2'),
            role: ChannelSourceRole::Indexer,
            target: rpc_target(),
            legacy_proof: None,
        },
    )
    .await;

    let ChannelSourceProbeOutput::Indexer(output) = output else {
        anyhow::bail!("Indexer request returned Sequencer output");
    };
    ensure!(
        matches!(output.health, ChannelSourceProbeFact::Observed(None)),
        "Indexer health fact was not retained"
    );
    ensure!(
        matches!(
            output.head,
            ChannelSourceProbeFact::Observed(Some(ref block)) if block.block_id == 40
        ),
        "Indexer finalized head fact was not retained"
    );
    Ok(())
}

#[tokio::test]
async fn sequencer_module_probe_is_explicitly_unsupported() -> Result<()> {
    let transport: Arc<dyn ChannelSourceProbeTransport> =
        Arc::new(DefaultChannelSourceProbeTransport::default());
    let result = transport
        .sequencer_health(ChannelSourceTarget::Module {
            module_id: super::super::layer::module_id_for_role(
                crate::source_routing::channel_sources::ChannelSourceRole::Sequencer,
            )
            .to_owned(),
        })
        .await;

    let Err(error) = result else {
        anyhow::bail!("Sequencer module probe unexpectedly succeeded");
    };
    ensure!(
        error.kind == ChannelSourceFailureKind::Unsupported,
        "unsupported module was not classified"
    );
    Ok(())
}

fn rpc_target() -> ChannelSourceTarget {
    ChannelSourceTarget::Rpc {
        endpoint: "http://localhost:3040/".to_owned(),
    }
}

fn fixture_block_bytes() -> Result<Vec<u8>> {
    Ok(BASE64_STANDARD.decode(TESTNET_LEGACY_BLOCK_1234)?)
}

fn legacy_anchor(
    block_bytes: Vec<u8>,
    fence: Arc<dyn Fn() -> bool + Send + Sync>,
) -> Result<SequencerLegacyAnchor> {
    let basis = finalized_basis(&block_bytes)?;
    Ok(SequencerLegacyAnchor::new(
        "8".repeat(64),
        basis,
        block_bytes,
        fence,
    ))
}

fn finalized_basis(block_bytes: &[u8]) -> Result<SequencerAttestationBasis> {
    let block = borsh::from_slice::<Block>(block_bytes)?;
    Ok(SequencerAttestationBasis::UserTrustedFinalizedL1Evidence(
        Box::new(FinalizedL1EvidenceBasis {
            network_scope: crate::inspection::NetworkScope::GenesisId {
                genesis_id: "1".repeat(64),
            },
            catalog_source_fingerprint: format!("sha256:{}", "2".repeat(64)),
            l1_slot: 42,
            l1_block_id: "3".repeat(64),
            transaction_hash: "4".repeat(64),
            operation_index: 5,
            l2_block_id: block.header.block_id,
            l2_header_hash: block.header.hash.to_string(),
            l2_signature: block.header.signature.to_string(),
        }),
    ))
}

fn persisted_legacy_proof(bytes: &[u8]) -> Result<SequencerPersistedLegacyProof> {
    let block = borsh::from_slice::<Block>(bytes)?;
    Ok(SequencerPersistedLegacyProof {
        expected_channel_id: "8".repeat(64),
        l2_block_id: block.header.block_id,
        l2_header_hash: block.header.hash.to_string(),
        l2_signature: block.header.signature.to_string(),
    })
}

fn probe_failure(kind: ChannelSourceFailureKind) -> ChannelSourceProbeFailure {
    ChannelSourceProbeFailure {
        kind,
        diagnostic: format!("fake {kind:?} failure"),
    }
}

fn source_id(character: char) -> String {
    format!("src_{}", character.to_string().repeat(32))
}
