use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use common::block::{Block, HashableBlockData};
use serde::{Deserialize, Serialize};

use crate::modules::logos_core::{
    LogoscoreCliTransport, ModuleTransportKind, SharedModuleTransport,
};

use super::{
    ChannelSourceProbeStage, ChannelSourceRole, ChannelSourceTarget, FinalizedL1EvidenceBasis,
    SequencerAttestationBasis, indexer::IndexerAdapter, layer::ExecutionZoneReadErrorKind,
    sequencer::SequencerAdapter,
};

const SOURCE_PROBE_TIMEOUT: Duration = Duration::from_secs(8);

pub(crate) type ChannelSourceProbeFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
pub(crate) type SequencerAttestorFuture = Pin<
    Box<
        dyn Future<Output = Result<SequencerTargetAttestation, ChannelSourceProbeFailure>>
            + Send
            + 'static,
    >,
>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SequencerTargetAttestation {
    pub(crate) channel_id: String,
    pub(crate) basis: SequencerAttestationBasis,
}

#[derive(Clone)]
pub(crate) struct SequencerLegacyAnchor {
    channel_id: String,
    basis: Box<SequencerAttestationBasis>,
    block_bytes: Vec<u8>,
    fence: Arc<dyn Fn() -> bool + Send + Sync>,
}

#[derive(Clone)]
pub(crate) enum SequencerLegacyAnchorState {
    Missing,
    Unavailable,
    Available(SequencerLegacyAnchor),
    Deferred(SequencerLegacyAnchorProvider),
}

type SequencerLegacyAnchorFuture =
    Pin<Box<dyn Future<Output = anyhow::Result<SequencerLegacyAnchorState>> + Send + 'static>>;

#[derive(Clone)]
pub(crate) struct SequencerLegacyAnchorProvider {
    load: Arc<dyn Fn() -> SequencerLegacyAnchorFuture + Send + Sync>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SequencerPersistedLegacyProof {
    pub(crate) expected_channel_id: String,
    pub(crate) l2_block_id: u64,
    pub(crate) l2_header_hash: String,
    pub(crate) l2_signature: String,
}

impl SequencerLegacyAnchor {
    #[must_use]
    pub(crate) fn new(
        channel_id: String,
        basis: SequencerAttestationBasis,
        block_bytes: Vec<u8>,
        fence: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Self {
        Self {
            channel_id,
            basis: Box::new(basis),
            block_bytes,
            fence,
        }
    }
}

impl SequencerLegacyAnchorState {
    pub(crate) fn deferred<F, Fut>(load: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = anyhow::Result<Self>> + Send + 'static,
    {
        Self::Deferred(SequencerLegacyAnchorProvider {
            load: Arc::new(move || Box::pin(load())),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelSourceFailureKind {
    Timeout,
    Unavailable,
    Protocol,
    Incomplete,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChannelSourceProbeFailure {
    pub(crate) kind: ChannelSourceFailureKind,
    pub(crate) diagnostic: String,
}

pub(crate) trait SequencerTargetAttestor: Send + Sync + 'static {
    fn attest(
        self: Arc<Self>,
        target: ChannelSourceTarget,
        legacy_anchor: SequencerLegacyAnchorState,
    ) -> SequencerAttestorFuture;
}

pub(crate) struct DefaultSequencerTargetAttestor {
    transport: Arc<dyn ChannelSourceProbeTransport>,
}

impl DefaultSequencerTargetAttestor {
    #[must_use]
    pub(crate) fn with_module_transport(
        module_transport: SharedModuleTransport,
        module_transport_kind: ModuleTransportKind,
    ) -> Self {
        Self {
            transport: Arc::new(DefaultChannelSourceProbeTransport::new(
                module_transport,
                module_transport_kind,
            )),
        }
    }
}

impl SequencerTargetAttestor for DefaultSequencerTargetAttestor {
    fn attest(
        self: Arc<Self>,
        target: ChannelSourceTarget,
        legacy_anchor: SequencerLegacyAnchorState,
    ) -> SequencerAttestorFuture {
        Box::pin(async move {
            match tokio::time::timeout(
                SOURCE_PROBE_TIMEOUT,
                attest_sequencer_target(self.transport.clone(), target, legacy_anchor),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => Err(ChannelSourceProbeFailure::timeout()),
            }
        })
    }
}

async fn attest_sequencer_target(
    transport: Arc<dyn ChannelSourceProbeTransport>,
    target: ChannelSourceTarget,
    legacy_anchor: SequencerLegacyAnchorState,
) -> Result<SequencerTargetAttestation, ChannelSourceProbeFailure> {
    match transport.clone().sequencer_channel_id(target.clone()).await {
        Ok(channel_id) => Ok(SequencerTargetAttestation {
            channel_id,
            basis: SequencerAttestationBasis::RpcReported {},
        }),
        Err(failure) if failure.kind == ChannelSourceFailureKind::Unsupported => {
            match resolve_legacy_anchor(legacy_anchor).await? {
                SequencerLegacyAnchorState::Missing => Err(failure),
                SequencerLegacyAnchorState::Unavailable => {
                    Err(ChannelSourceProbeFailure::unavailable(
                        "Finalized L1 anchor is temporarily unavailable",
                    ))
                }
                SequencerLegacyAnchorState::Available(legacy_anchor) => {
                    attest_legacy_sequencer_target(transport, target, legacy_anchor).await
                }
                SequencerLegacyAnchorState::Deferred(_) => {
                    Err(ChannelSourceProbeFailure::protocol(
                        "Sequencer legacy anchor provider returned another deferred anchor",
                    ))
                }
            }
        }
        Err(failure) => Err(failure),
    }
}

async fn resolve_legacy_anchor(
    legacy_anchor: SequencerLegacyAnchorState,
) -> Result<SequencerLegacyAnchorState, ChannelSourceProbeFailure> {
    let SequencerLegacyAnchorState::Deferred(provider) = legacy_anchor else {
        return Ok(legacy_anchor);
    };
    (provider.load)().await.map_err(|_| {
        ChannelSourceProbeFailure::protocol(
            "Finalized L1 anchor became invalid or stale during Sequencer source verification",
        )
    })
}

async fn attest_legacy_sequencer_target(
    transport: Arc<dyn ChannelSourceProbeTransport>,
    target: ChannelSourceTarget,
    legacy_anchor: SequencerLegacyAnchor,
) -> Result<SequencerTargetAttestation, ChannelSourceProbeFailure> {
    let anchor_block = decode_verified_legacy_block(&legacy_anchor.block_bytes)?;
    let SequencerAttestationBasis::UserTrustedFinalizedL1Evidence(basis) =
        legacy_anchor.basis.as_ref()
    else {
        return Err(ChannelSourceProbeFailure::protocol(
            "Sequencer legacy anchor has no user-trusted finalized L1 evidence",
        ));
    };
    let FinalizedL1EvidenceBasis {
        l2_block_id,
        l2_header_hash,
        l2_signature,
        ..
    } = basis.as_ref();
    if anchor_block.header.block_id != *l2_block_id
        || anchor_block.header.hash.to_string() != *l2_header_hash
        || anchor_block.header.signature.to_string() != *l2_signature
    {
        return Err(ChannelSourceProbeFailure::protocol(
            "Sequencer legacy anchor does not match its finalized L1 evidence basis",
        ));
    }
    let candidate_bytes = transport
        .sequencer_block_bytes(target, anchor_block.header.block_id)
        .await?
        .ok_or_else(|| {
            ChannelSourceProbeFailure::incomplete(
                "Sequencer did not return the finalized L1 anchor block",
            )
        })?;
    let candidate_block = decode_verified_legacy_block(&candidate_bytes)?;

    if HashableBlockData::from(anchor_block.clone())
        != HashableBlockData::from(candidate_block.clone())
        || anchor_block.header.hash != candidate_block.header.hash
        || anchor_block.header.signature != candidate_block.header.signature
    {
        return Err(ChannelSourceProbeFailure::protocol(
            "Sequencer block does not match the finalized L1 anchor",
        ));
    }
    if !(legacy_anchor.fence)() {
        return Err(ChannelSourceProbeFailure::protocol(
            "Finalized L1 anchor became stale during Sequencer source verification",
        ));
    }

    Ok(SequencerTargetAttestation {
        channel_id: legacy_anchor.channel_id,
        basis: *legacy_anchor.basis,
    })
}

fn decode_verified_legacy_block(bytes: &[u8]) -> Result<Block, ChannelSourceProbeFailure> {
    crate::lez::decode_sequencer_block_bytes(bytes).map_err(|_| {
        ChannelSourceProbeFailure::protocol(
            "Sequencer evidence block layout or content hash is invalid",
        )
    })
}

impl std::fmt::Display for ChannelSourceProbeFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.diagnostic)
    }
}

impl std::error::Error for ChannelSourceProbeFailure {}

impl ChannelSourceProbeFailure {
    fn timeout() -> Self {
        Self {
            kind: ChannelSourceFailureKind::Timeout,
            diagnostic: "source probe timed out".to_owned(),
        }
    }

    fn unavailable(diagnostic: impl Into<String>) -> Self {
        Self {
            kind: ChannelSourceFailureKind::Unavailable,
            diagnostic: diagnostic.into(),
        }
    }

    fn protocol(diagnostic: impl Into<String>) -> Self {
        Self {
            kind: ChannelSourceFailureKind::Protocol,
            diagnostic: diagnostic.into(),
        }
    }

    fn incomplete(diagnostic: impl Into<String>) -> Self {
        Self {
            kind: ChannelSourceFailureKind::Incomplete,
            diagnostic: diagnostic.into(),
        }
    }

    fn unsupported(diagnostic: impl Into<String>) -> Self {
        Self {
            kind: ChannelSourceFailureKind::Unsupported,
            diagnostic: diagnostic.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChannelSourceProbeFact<T> {
    Observed(T),
    Failed(ChannelSourceProbeFailure),
}

impl<T> ChannelSourceProbeFact<T> {
    pub(crate) fn failure(&self) -> Option<&ChannelSourceProbeFailure> {
        match self {
            Self::Failed(failure) => Some(failure),
            Self::Observed(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChannelSourceBlock {
    pub(crate) block_id: u64,
    pub(crate) header_hash: String,
    pub(crate) parent_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChannelSourceProbeRequest {
    pub(crate) source_id: String,
    pub(crate) role: ChannelSourceRole,
    pub(crate) target: ChannelSourceTarget,
    pub(crate) legacy_proof: Option<SequencerPersistedLegacyProof>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SequencerSourceProbeOutput {
    pub(crate) health: ChannelSourceProbeFact<()>,
    pub(crate) channel_id: ChannelSourceProbeFact<SequencerSourceIdentity>,
    pub(crate) identity_stage: ChannelSourceProbeStage,
    pub(crate) head: ChannelSourceProbeFact<ChannelSourceBlock>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SequencerSourceIdentityBasis {
    RpcReported,
    FinalizedL1EvidenceMatched { anchor_block_id: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SequencerSourceIdentity {
    pub(crate) channel_id: String,
    pub(crate) basis: SequencerSourceIdentityBasis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexerSourceProbeOutput {
    pub(crate) health: ChannelSourceProbeFact<()>,
    pub(crate) head: ChannelSourceProbeFact<Option<ChannelSourceBlock>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChannelSourceProbeOutput {
    Sequencer(SequencerSourceProbeOutput),
    Indexer(IndexerSourceProbeOutput),
}

pub(crate) trait ChannelSourceProbe: Send + Sync + 'static {
    fn probe(
        self: Arc<Self>,
        request: ChannelSourceProbeRequest,
    ) -> ChannelSourceProbeFuture<ChannelSourceProbeOutput>;

    fn block(
        self: Arc<Self>,
        request: ChannelSourceProbeRequest,
        block_id: u64,
    ) -> ChannelSourceProbeFuture<Result<Option<ChannelSourceBlock>, ChannelSourceProbeFailure>>;
}

pub(crate) struct DefaultChannelSourceProbe {
    transport: Arc<dyn ChannelSourceProbeTransport>,
}

impl Default for DefaultChannelSourceProbe {
    fn default() -> Self {
        let transport: SharedModuleTransport = Arc::new(LogoscoreCliTransport::default());
        Self::with_module_transport(transport, ModuleTransportKind::LogoscoreCli)
    }
}

impl DefaultChannelSourceProbe {
    #[must_use]
    pub(crate) fn with_module_transport(
        module_transport: SharedModuleTransport,
        module_transport_kind: ModuleTransportKind,
    ) -> Self {
        Self {
            transport: Arc::new(DefaultChannelSourceProbeTransport::new(
                module_transport,
                module_transport_kind,
            )),
        }
    }
}

impl ChannelSourceProbe for DefaultChannelSourceProbe {
    fn probe(
        self: Arc<Self>,
        request: ChannelSourceProbeRequest,
    ) -> ChannelSourceProbeFuture<ChannelSourceProbeOutput> {
        Box::pin(async move {
            match tokio::time::timeout(
                SOURCE_PROBE_TIMEOUT,
                probe_source(self.transport.clone(), request.clone()),
            )
            .await
            {
                Ok(output) => output,
                Err(_) => timed_out_output(request.role, request.legacy_proof.is_some()),
            }
        })
    }

    fn block(
        self: Arc<Self>,
        request: ChannelSourceProbeRequest,
        block_id: u64,
    ) -> ChannelSourceProbeFuture<Result<Option<ChannelSourceBlock>, ChannelSourceProbeFailure>>
    {
        Box::pin(async move {
            let block = match request.role {
                ChannelSourceRole::Sequencer => self
                    .transport
                    .clone()
                    .sequencer_block(request.target, block_id),
                ChannelSourceRole::Indexer => self
                    .transport
                    .clone()
                    .indexer_block(request.target, block_id),
            };
            match tokio::time::timeout(SOURCE_PROBE_TIMEOUT, block).await {
                Ok(result) => result,
                Err(_) => Err(ChannelSourceProbeFailure::timeout()),
            }
        })
    }
}

async fn probe_source(
    transport: Arc<dyn ChannelSourceProbeTransport>,
    request: ChannelSourceProbeRequest,
) -> ChannelSourceProbeOutput {
    match request.role {
        ChannelSourceRole::Sequencer => {
            let health = transport.clone().sequencer_health(request.target.clone());
            let channel_id = sequencer_identity(
                transport.clone(),
                request.target.clone(),
                request.legacy_proof,
            );
            let head = sequencer_head(transport, request.target);
            let (health, (identity_stage, channel_id), head) =
                tokio::join!(health, channel_id, head);
            let channel_id = validate_sequencer_identity_head(channel_id, &head);
            ChannelSourceProbeOutput::Sequencer(SequencerSourceProbeOutput {
                health: result_fact(health),
                channel_id: result_fact(channel_id),
                identity_stage,
                head: result_fact(head),
            })
        }
        ChannelSourceRole::Indexer => {
            let health = transport.clone().indexer_health(request.target.clone());
            let head = indexer_head(transport, request.target);
            let (health, head) = tokio::join!(health, head);
            ChannelSourceProbeOutput::Indexer(IndexerSourceProbeOutput {
                health: result_fact(health),
                head: result_fact(head),
            })
        }
    }
}

fn validate_sequencer_identity_head(
    identity: Result<SequencerSourceIdentity, ChannelSourceProbeFailure>,
    head: &Result<ChannelSourceBlock, ChannelSourceProbeFailure>,
) -> Result<SequencerSourceIdentity, ChannelSourceProbeFailure> {
    let identity = identity?;
    if let SequencerSourceIdentityBasis::FinalizedL1EvidenceMatched { anchor_block_id } =
        identity.basis
    {
        let head = head.as_ref().map_err(Clone::clone)?;
        if head.block_id < anchor_block_id {
            return Err(ChannelSourceProbeFailure::protocol(
                "Sequencer head is behind its finalized L1 evidence block",
            ));
        }
    }
    Ok(identity)
}

async fn sequencer_identity(
    transport: Arc<dyn ChannelSourceProbeTransport>,
    target: ChannelSourceTarget,
    legacy_proof: Option<SequencerPersistedLegacyProof>,
) -> (
    ChannelSourceProbeStage,
    Result<SequencerSourceIdentity, ChannelSourceProbeFailure>,
) {
    match transport.clone().sequencer_channel_id(target.clone()).await {
        Ok(channel_id) => (
            ChannelSourceProbeStage::ChannelIdentity,
            Ok(SequencerSourceIdentity {
                channel_id,
                basis: SequencerSourceIdentityBasis::RpcReported,
            }),
        ),
        Err(failure) if failure.kind == ChannelSourceFailureKind::Unsupported => {
            let Some(legacy_proof) = legacy_proof else {
                return (ChannelSourceProbeStage::ChannelIdentity, Err(failure));
            };
            let anchor_block_id = legacy_proof.l2_block_id;
            (
                ChannelSourceProbeStage::EvidenceConsistency,
                validate_persisted_legacy_proof(transport, target, legacy_proof)
                    .await
                    .map(|channel_id| SequencerSourceIdentity {
                        channel_id,
                        basis: SequencerSourceIdentityBasis::FinalizedL1EvidenceMatched {
                            anchor_block_id,
                        },
                    }),
            )
        }
        Err(failure) => (ChannelSourceProbeStage::ChannelIdentity, Err(failure)),
    }
}

async fn validate_persisted_legacy_proof(
    transport: Arc<dyn ChannelSourceProbeTransport>,
    target: ChannelSourceTarget,
    legacy_proof: SequencerPersistedLegacyProof,
) -> Result<String, ChannelSourceProbeFailure> {
    let candidate_bytes = transport
        .sequencer_block_bytes(target, legacy_proof.l2_block_id)
        .await?
        .ok_or_else(|| {
            ChannelSourceProbeFailure::incomplete(
                "Sequencer did not return its persisted finalized L1 evidence block",
            )
        })?;
    let candidate = decode_verified_legacy_block(&candidate_bytes)?;
    if candidate.header.block_id != legacy_proof.l2_block_id
        || candidate.header.hash.to_string() != legacy_proof.l2_header_hash
        || candidate.header.signature.to_string() != legacy_proof.l2_signature
    {
        return Err(ChannelSourceProbeFailure::protocol(
            "Sequencer block does not match its persisted finalized L1 evidence",
        ));
    }
    Ok(legacy_proof.expected_channel_id)
}

fn result_fact<T>(result: Result<T, ChannelSourceProbeFailure>) -> ChannelSourceProbeFact<T> {
    match result {
        Ok(value) => ChannelSourceProbeFact::Observed(value),
        Err(failure) => ChannelSourceProbeFact::Failed(failure),
    }
}

fn timed_out_output(
    role: ChannelSourceRole,
    uses_finalized_l1_evidence: bool,
) -> ChannelSourceProbeOutput {
    match role {
        ChannelSourceRole::Sequencer => {
            ChannelSourceProbeOutput::Sequencer(SequencerSourceProbeOutput {
                health: ChannelSourceProbeFact::Failed(ChannelSourceProbeFailure::timeout()),
                channel_id: ChannelSourceProbeFact::Failed(ChannelSourceProbeFailure::timeout()),
                identity_stage: if uses_finalized_l1_evidence {
                    ChannelSourceProbeStage::EvidenceConsistency
                } else {
                    ChannelSourceProbeStage::ChannelIdentity
                },
                head: ChannelSourceProbeFact::Failed(ChannelSourceProbeFailure::timeout()),
            })
        }
        ChannelSourceRole::Indexer => ChannelSourceProbeOutput::Indexer(IndexerSourceProbeOutput {
            health: ChannelSourceProbeFact::Failed(ChannelSourceProbeFailure::timeout()),
            head: ChannelSourceProbeFact::Failed(ChannelSourceProbeFailure::timeout()),
        }),
    }
}

async fn sequencer_head(
    transport: Arc<dyn ChannelSourceProbeTransport>,
    target: ChannelSourceTarget,
) -> Result<ChannelSourceBlock, ChannelSourceProbeFailure> {
    let first_id = transport.clone().sequencer_head_id(target.clone()).await?;
    if let Some(block) = transport
        .clone()
        .sequencer_block(target.clone(), first_id)
        .await?
    {
        return Ok(block);
    }

    let retry_id = transport.clone().sequencer_head_id(target.clone()).await?;
    transport
        .sequencer_block(target, retry_id)
        .await?
        .ok_or_else(|| {
            ChannelSourceProbeFailure::incomplete(
                "Sequencer head block remained unavailable after one retry",
            )
        })
}

async fn indexer_head(
    transport: Arc<dyn ChannelSourceProbeTransport>,
    target: ChannelSourceTarget,
) -> Result<Option<ChannelSourceBlock>, ChannelSourceProbeFailure> {
    let Some(block_id) = transport
        .clone()
        .indexer_finalized_head_id(target.clone())
        .await?
    else {
        return Ok(None);
    };
    transport
        .indexer_block(target, block_id)
        .await?
        .map(Some)
        .ok_or_else(|| {
            ChannelSourceProbeFailure::incomplete("Indexer finalized head block was unavailable")
        })
}

type TransportFuture<T> =
    Pin<Box<dyn Future<Output = Result<T, ChannelSourceProbeFailure>> + Send + 'static>>;

trait ChannelSourceProbeTransport: Send + Sync + 'static {
    fn sequencer_health(self: Arc<Self>, target: ChannelSourceTarget) -> TransportFuture<()>;

    fn sequencer_channel_id(
        self: Arc<Self>,
        target: ChannelSourceTarget,
    ) -> TransportFuture<String>;

    fn sequencer_head_id(self: Arc<Self>, target: ChannelSourceTarget) -> TransportFuture<u64>;

    fn sequencer_block(
        self: Arc<Self>,
        target: ChannelSourceTarget,
        block_id: u64,
    ) -> TransportFuture<Option<ChannelSourceBlock>>;

    fn sequencer_block_bytes(
        self: Arc<Self>,
        target: ChannelSourceTarget,
        block_id: u64,
    ) -> TransportFuture<Option<Vec<u8>>>;

    fn indexer_health(self: Arc<Self>, target: ChannelSourceTarget) -> TransportFuture<()>;

    fn indexer_finalized_head_id(
        self: Arc<Self>,
        target: ChannelSourceTarget,
    ) -> TransportFuture<Option<u64>>;

    fn indexer_block(
        self: Arc<Self>,
        target: ChannelSourceTarget,
        block_id: u64,
    ) -> TransportFuture<Option<ChannelSourceBlock>>;
}

struct DefaultChannelSourceProbeTransport {
    module_transport: SharedModuleTransport,
    module_transport_kind: ModuleTransportKind,
}

impl Default for DefaultChannelSourceProbeTransport {
    fn default() -> Self {
        Self::new(
            Arc::new(LogoscoreCliTransport::default()),
            ModuleTransportKind::LogoscoreCli,
        )
    }
}

impl DefaultChannelSourceProbeTransport {
    fn new(
        module_transport: SharedModuleTransport,
        module_transport_kind: ModuleTransportKind,
    ) -> Self {
        Self {
            module_transport,
            module_transport_kind,
        }
    }
}

impl ChannelSourceProbeTransport for DefaultChannelSourceProbeTransport {
    fn sequencer_health(self: Arc<Self>, target: ChannelSourceTarget) -> TransportFuture<()> {
        Box::pin(async move {
            SequencerAdapter::connect(&target)
                .map_err(|error| {
                    probe_read_failure(
                        error.kind,
                        "Sequencer health request failed",
                        "configured source does not expose Sequencer health inspection",
                    )
                })?
                .health()
                .await
                .map_err(|error| {
                    probe_read_failure(
                        error.kind,
                        "Sequencer health request failed",
                        "configured source does not expose Sequencer health inspection",
                    )
                })
        })
    }

    fn sequencer_channel_id(
        self: Arc<Self>,
        target: ChannelSourceTarget,
    ) -> TransportFuture<String> {
        Box::pin(async move {
            SequencerAdapter::connect(&target)
                .map_err(|error| {
                    probe_read_failure(
                        error.kind,
                        "Sequencer Channel identity request failed",
                        "configured source does not expose Sequencer Channel identity",
                    )
                })?
                .channel_id()
                .await
                .map_err(|error| {
                    probe_read_failure(
                        error.kind,
                        "Sequencer Channel identity request failed",
                        "configured source does not expose Sequencer Channel identity",
                    )
                })
        })
    }

    fn sequencer_head_id(self: Arc<Self>, target: ChannelSourceTarget) -> TransportFuture<u64> {
        Box::pin(async move {
            SequencerAdapter::connect(&target)
                .map_err(|error| {
                    probe_read_failure(
                        error.kind,
                        "Sequencer head request failed",
                        "configured source does not expose Sequencer head inspection",
                    )
                })?
                .reported_head_id()
                .await
                .map_err(|error| {
                    probe_read_failure(
                        error.kind,
                        "Sequencer head request failed",
                        "configured source does not expose Sequencer head inspection",
                    )
                })
        })
    }

    fn sequencer_block(
        self: Arc<Self>,
        target: ChannelSourceTarget,
        block_id: u64,
    ) -> TransportFuture<Option<ChannelSourceBlock>> {
        Box::pin(async move {
            SequencerAdapter::connect(&target)
                .map_err(|error| {
                    probe_read_failure(
                        error.kind,
                        "Sequencer block request failed",
                        "configured source does not expose Sequencer block inspection",
                    )
                })?
                .block_by_id(block_id)
                .await
                .map(|block| block.map(sequencer_block_reference))
                .map_err(|error| {
                    probe_read_failure(
                        error.kind,
                        "Sequencer block request failed",
                        "configured source does not expose Sequencer block inspection",
                    )
                })
        })
    }

    fn sequencer_block_bytes(
        self: Arc<Self>,
        target: ChannelSourceTarget,
        block_id: u64,
    ) -> TransportFuture<Option<Vec<u8>>> {
        Box::pin(async move {
            SequencerAdapter::connect(&target)
                .map_err(|error| {
                    probe_read_failure(
                        error.kind,
                        "Sequencer block request failed",
                        "configured source does not expose Sequencer block inspection",
                    )
                })?
                .raw_block_by_id(block_id)
                .await
                .map_err(|error| {
                    probe_read_failure(
                        error.kind,
                        "Sequencer block request failed",
                        "configured source does not expose Sequencer block inspection",
                    )
                })
        })
    }

    fn indexer_health(self: Arc<Self>, target: ChannelSourceTarget) -> TransportFuture<()> {
        Box::pin(async move {
            IndexerAdapter::connect(&target, &self.module_transport, self.module_transport_kind)
                .map_err(|error| {
                    probe_read_failure(
                        error.kind,
                        "Indexer health request failed",
                        "configured source does not expose Indexer health inspection",
                    )
                })?
                .health()
                .await
                .map_err(|error| {
                    probe_read_failure(
                        error.kind,
                        "Indexer health request failed",
                        "configured source does not expose Indexer health inspection",
                    )
                })
        })
    }

    fn indexer_finalized_head_id(
        self: Arc<Self>,
        target: ChannelSourceTarget,
    ) -> TransportFuture<Option<u64>> {
        Box::pin(async move {
            IndexerAdapter::connect(&target, &self.module_transport, self.module_transport_kind)
                .map_err(|error| {
                    probe_read_failure(
                        error.kind,
                        "Indexer finalized-head request failed",
                        "configured source does not expose Indexer head inspection",
                    )
                })?
                .reported_head_id()
                .await
                .map_err(|error| {
                    probe_read_failure(
                        error.kind,
                        "Indexer finalized-head request failed",
                        "configured source does not expose Indexer head inspection",
                    )
                })
        })
    }

    fn indexer_block(
        self: Arc<Self>,
        target: ChannelSourceTarget,
        block_id: u64,
    ) -> TransportFuture<Option<ChannelSourceBlock>> {
        Box::pin(async move {
            IndexerAdapter::connect(&target, &self.module_transport, self.module_transport_kind)
                .map_err(|error| {
                    probe_read_failure(
                        error.kind,
                        "Indexer block request failed",
                        "configured source does not expose Indexer block inspection",
                    )
                })?
                .block_by_id(block_id)
                .await
                .map(|block| block.and_then(indexer_block_reference))
                .map_err(|error| {
                    probe_read_failure(
                        error.kind,
                        "Indexer block request failed",
                        "configured source does not expose Indexer block inspection",
                    )
                })
        })
    }
}

fn probe_read_failure(
    kind: ExecutionZoneReadErrorKind,
    diagnostic: &'static str,
    unsupported: &'static str,
) -> ChannelSourceProbeFailure {
    match kind {
        ExecutionZoneReadErrorKind::Unavailable => {
            ChannelSourceProbeFailure::unavailable(diagnostic)
        }
        ExecutionZoneReadErrorKind::Protocol => ChannelSourceProbeFailure::protocol(diagnostic),
        ExecutionZoneReadErrorKind::Capability => {
            ChannelSourceProbeFailure::unsupported(unsupported)
        }
    }
}

fn sequencer_block_reference(block: crate::lez::BlockSummary) -> ChannelSourceBlock {
    ChannelSourceBlock {
        block_id: block.block_id,
        header_hash: block.header_hash,
        parent_hash: Some(block.parent_hash),
    }
}

fn indexer_block_reference(block: crate::lez::IndexerBlockReport) -> Option<ChannelSourceBlock> {
    Some(ChannelSourceBlock {
        block_id: block.block_id?,
        header_hash: block.header_hash?,
        parent_hash: block.parent_hash,
    })
}

#[cfg(test)]
mod tests;
