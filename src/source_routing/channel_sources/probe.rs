use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};

use super::{
    ChannelSourceRole, ChannelSourceTarget, indexer::IndexerAdapter,
    layer::ExecutionZoneReadErrorKind, sequencer::SequencerAdapter,
};

const SOURCE_PROBE_TIMEOUT: Duration = Duration::from_secs(8);

pub(crate) type ChannelSourceProbeFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SequencerSourceProbeOutput {
    pub(crate) health: ChannelSourceProbeFact<()>,
    pub(crate) channel_id: ChannelSourceProbeFact<String>,
    pub(crate) head: ChannelSourceProbeFact<ChannelSourceBlock>,
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
        Self {
            transport: Arc::new(DefaultChannelSourceProbeTransport),
        }
    }
}

pub(crate) async fn attest_sequencer_target(target: ChannelSourceTarget) -> anyhow::Result<String> {
    match tokio::time::timeout(
        SOURCE_PROBE_TIMEOUT,
        Arc::new(DefaultChannelSourceProbeTransport).sequencer_channel_id(target),
    )
    .await
    {
        Ok(Ok(channel_id)) => Ok(channel_id),
        Ok(Err(failure)) => Err(anyhow::Error::new(failure)),
        Err(_) => Err(anyhow::Error::new(ChannelSourceProbeFailure::timeout())),
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
                Err(_) => timed_out_output(request.role),
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
            let channel_id = transport
                .clone()
                .sequencer_channel_id(request.target.clone());
            let head = sequencer_head(transport, request.target);
            let (health, channel_id, head) = tokio::join!(health, channel_id, head);
            ChannelSourceProbeOutput::Sequencer(SequencerSourceProbeOutput {
                health: result_fact(health),
                channel_id: result_fact(channel_id),
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

fn result_fact<T>(result: Result<T, ChannelSourceProbeFailure>) -> ChannelSourceProbeFact<T> {
    match result {
        Ok(value) => ChannelSourceProbeFact::Observed(value),
        Err(failure) => ChannelSourceProbeFact::Failed(failure),
    }
}

fn timed_out_output(role: ChannelSourceRole) -> ChannelSourceProbeOutput {
    match role {
        ChannelSourceRole::Sequencer => {
            ChannelSourceProbeOutput::Sequencer(SequencerSourceProbeOutput {
                health: ChannelSourceProbeFact::Failed(ChannelSourceProbeFailure::timeout()),
                channel_id: ChannelSourceProbeFact::Failed(ChannelSourceProbeFailure::timeout()),
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

struct DefaultChannelSourceProbeTransport;

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

    fn indexer_health(self: Arc<Self>, target: ChannelSourceTarget) -> TransportFuture<()> {
        Box::pin(async move {
            IndexerAdapter::connect(&target)
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
            IndexerAdapter::connect(&target)
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
            IndexerAdapter::connect(&target)
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
