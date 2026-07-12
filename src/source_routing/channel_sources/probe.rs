use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{ChannelSourceRole, ChannelSourceTarget};
use crate::{
    lez::{
        indexer_block_by_id as direct_indexer_block_by_id, indexer_finalized_block_id,
        indexer_health as direct_indexer_health, last_sequencer_block_id, sequencer_block,
        sequencer_channel_id, sequencer_health,
    },
    source_routing::core::adapters::module,
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
    NotApplicable,
}

impl<T> ChannelSourceProbeFact<T> {
    pub(crate) fn failure(&self) -> Option<&ChannelSourceProbeFailure> {
        match self {
            Self::Failed(failure) => Some(failure),
            Self::Observed(_) | Self::NotApplicable => None,
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
pub(crate) struct ChannelSourceProbeOutput {
    pub(crate) health: ChannelSourceProbeFact<()>,
    pub(crate) channel_id: ChannelSourceProbeFact<String>,
    pub(crate) head: ChannelSourceProbeFact<Option<ChannelSourceBlock>>,
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
            match tokio::time::timeout(
                SOURCE_PROBE_TIMEOUT,
                self.transport
                    .clone()
                    .block(request.role, request.target, block_id),
            )
            .await
            {
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
    let health = transport
        .clone()
        .health(request.role, request.target.clone());
    match request.role {
        ChannelSourceRole::Sequencer => {
            let channel_id = transport
                .clone()
                .sequencer_channel_id(request.target.clone());
            let head = sequencer_head(transport, request.target);
            let (health, channel_id, head) = tokio::join!(health, channel_id, head);
            ChannelSourceProbeOutput {
                health: result_fact(health),
                channel_id: result_fact(channel_id),
                head: result_fact(head.map(Some)),
            }
        }
        ChannelSourceRole::Indexer => {
            let head = indexer_head(transport, request.target);
            let (health, head) = tokio::join!(health, head);
            ChannelSourceProbeOutput {
                health: result_fact(health),
                channel_id: ChannelSourceProbeFact::NotApplicable,
                head: result_fact(head),
            }
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
    ChannelSourceProbeOutput {
        health: ChannelSourceProbeFact::Failed(ChannelSourceProbeFailure::timeout()),
        channel_id: match role {
            ChannelSourceRole::Sequencer => {
                ChannelSourceProbeFact::Failed(ChannelSourceProbeFailure::timeout())
            }
            ChannelSourceRole::Indexer => ChannelSourceProbeFact::NotApplicable,
        },
        head: ChannelSourceProbeFact::Failed(ChannelSourceProbeFailure::timeout()),
    }
}

async fn sequencer_head(
    transport: Arc<dyn ChannelSourceProbeTransport>,
    target: ChannelSourceTarget,
) -> Result<ChannelSourceBlock, ChannelSourceProbeFailure> {
    let first_id = transport.clone().sequencer_head_id(target.clone()).await?;
    if let Some(block) = transport
        .clone()
        .block(ChannelSourceRole::Sequencer, target.clone(), first_id)
        .await?
    {
        return Ok(block);
    }

    let retry_id = transport.clone().sequencer_head_id(target.clone()).await?;
    transport
        .block(ChannelSourceRole::Sequencer, target, retry_id)
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
        .block(ChannelSourceRole::Indexer, target, block_id)
        .await?
        .map(Some)
        .ok_or_else(|| {
            ChannelSourceProbeFailure::incomplete("Indexer finalized head block was unavailable")
        })
}

type TransportFuture<T> =
    Pin<Box<dyn Future<Output = Result<T, ChannelSourceProbeFailure>> + Send + 'static>>;

trait ChannelSourceProbeTransport: Send + Sync + 'static {
    fn health(
        self: Arc<Self>,
        role: ChannelSourceRole,
        target: ChannelSourceTarget,
    ) -> TransportFuture<()>;

    fn sequencer_channel_id(
        self: Arc<Self>,
        target: ChannelSourceTarget,
    ) -> TransportFuture<String>;

    fn sequencer_head_id(self: Arc<Self>, target: ChannelSourceTarget) -> TransportFuture<u64>;

    fn indexer_finalized_head_id(
        self: Arc<Self>,
        target: ChannelSourceTarget,
    ) -> TransportFuture<Option<u64>>;

    fn block(
        self: Arc<Self>,
        role: ChannelSourceRole,
        target: ChannelSourceTarget,
        block_id: u64,
    ) -> TransportFuture<Option<ChannelSourceBlock>>;
}

struct DefaultChannelSourceProbeTransport;

impl ChannelSourceProbeTransport for DefaultChannelSourceProbeTransport {
    fn health(
        self: Arc<Self>,
        role: ChannelSourceRole,
        target: ChannelSourceTarget,
    ) -> TransportFuture<()> {
        Box::pin(async move {
            match (role, target) {
                (ChannelSourceRole::Sequencer, ChannelSourceTarget::Rpc { endpoint }) => {
                    sequencer_health(&endpoint).await.map_err(|_| {
                        ChannelSourceProbeFailure::unavailable("Sequencer health request failed")
                    })
                }
                (ChannelSourceRole::Indexer, ChannelSourceTarget::Rpc { endpoint }) => {
                    direct_indexer_health(&endpoint)
                        .await
                        .map(|_| ())
                        .map_err(|_| {
                            ChannelSourceProbeFailure::unavailable("Indexer health request failed")
                        })
                }
                (ChannelSourceRole::Sequencer, ChannelSourceTarget::Module { .. }) => {
                    Err(ChannelSourceProbeFailure::unsupported(
                        "configured module does not expose Sequencer inspection",
                    ))
                }
                (ChannelSourceRole::Indexer, ChannelSourceTarget::Module { .. }) => {
                    run_module(module::indexer_health).await.map(|_| ())
                }
            }
        })
    }

    fn sequencer_channel_id(
        self: Arc<Self>,
        target: ChannelSourceTarget,
    ) -> TransportFuture<String> {
        Box::pin(async move {
            match target {
                ChannelSourceTarget::Rpc { endpoint } => {
                    sequencer_channel_id(&endpoint).await.map_err(|_| {
                        ChannelSourceProbeFailure::unavailable(
                            "Sequencer Channel identity request failed",
                        )
                    })
                }
                ChannelSourceTarget::Module { .. } => Err(ChannelSourceProbeFailure::unsupported(
                    "configured module does not expose Sequencer Channel identity",
                )),
            }
        })
    }

    fn sequencer_head_id(self: Arc<Self>, target: ChannelSourceTarget) -> TransportFuture<u64> {
        Box::pin(async move {
            match target {
                ChannelSourceTarget::Rpc { endpoint } => {
                    last_sequencer_block_id(&endpoint).await.map_err(|_| {
                        ChannelSourceProbeFailure::unavailable("Sequencer head request failed")
                    })
                }
                ChannelSourceTarget::Module { .. } => Err(ChannelSourceProbeFailure::unsupported(
                    "configured module does not expose Sequencer head inspection",
                )),
            }
        })
    }

    fn indexer_finalized_head_id(
        self: Arc<Self>,
        target: ChannelSourceTarget,
    ) -> TransportFuture<Option<u64>> {
        Box::pin(async move {
            match target {
                ChannelSourceTarget::Rpc { endpoint } => {
                    indexer_finalized_block_id(&endpoint).await.map_err(|_| {
                        ChannelSourceProbeFailure::unavailable(
                            "Indexer finalized-head request failed",
                        )
                    })
                }
                ChannelSourceTarget::Module { .. } => run_module(module::indexer_finalized_head)
                    .await
                    .and_then(parse_optional_block_id),
            }
        })
    }

    fn block(
        self: Arc<Self>,
        role: ChannelSourceRole,
        target: ChannelSourceTarget,
        block_id: u64,
    ) -> TransportFuture<Option<ChannelSourceBlock>> {
        Box::pin(async move {
            match (role, target) {
                (ChannelSourceRole::Sequencer, ChannelSourceTarget::Rpc { endpoint }) => {
                    sequencer_block(&endpoint, block_id)
                        .await
                        .map(|block| block.map(sequencer_block_reference))
                        .map_err(|_| {
                            ChannelSourceProbeFailure::unavailable("Sequencer block request failed")
                        })
                }
                (ChannelSourceRole::Indexer, ChannelSourceTarget::Rpc { endpoint }) => {
                    direct_indexer_block_by_id(&endpoint, block_id)
                        .await
                        .map(|block| block.and_then(indexer_block_reference))
                        .map_err(|_| {
                            ChannelSourceProbeFailure::unavailable("Indexer block request failed")
                        })
                }
                (ChannelSourceRole::Sequencer, ChannelSourceTarget::Module { .. }) => {
                    Err(ChannelSourceProbeFailure::unsupported(
                        "configured module does not expose Sequencer block inspection",
                    ))
                }
                (ChannelSourceRole::Indexer, ChannelSourceTarget::Module { .. }) => {
                    let block =
                        tokio::task::spawn_blocking(move || module::indexer_block_by_id(block_id))
                            .await
                            .map_err(|_| {
                                ChannelSourceProbeFailure::unavailable("Indexer module task failed")
                            })?
                            .map_err(|_| {
                                ChannelSourceProbeFailure::unavailable(
                                    "Indexer module block request failed",
                                )
                            })?;
                    Ok(block.and_then(indexer_block_reference))
                }
            }
        })
    }
}

async fn run_module<T, F>(task: F) -> Result<T, ChannelSourceProbeFailure>
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|_| ChannelSourceProbeFailure::unavailable("module probe task failed"))?
        .map_err(|_| ChannelSourceProbeFailure::unavailable("module probe request failed"))
}

fn parse_optional_block_id(value: Value) -> Result<Option<u64>, ChannelSourceProbeFailure> {
    if value.is_null() || value.as_str().is_some_and(str::is_empty) {
        return Ok(None);
    }
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
        .map(Some)
        .ok_or_else(|| {
            ChannelSourceProbeFailure::protocol(
                "Indexer finalized block id was not an unsigned integer",
            )
        })
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
