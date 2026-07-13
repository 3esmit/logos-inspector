use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Result, ensure};

use super::*;

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
    fn health(
        self: Arc<Self>,
        _role: ChannelSourceRole,
        _target: ChannelSourceTarget,
    ) -> TransportFuture<()> {
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

    fn indexer_finalized_head_id(
        self: Arc<Self>,
        _target: ChannelSourceTarget,
    ) -> TransportFuture<Option<u64>> {
        Box::pin(async { Ok(Some(40)) })
    }

    fn block(
        self: Arc<Self>,
        _role: ChannelSourceRole,
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
            source_id: source_id('1'),
            role: ChannelSourceRole::Sequencer,
            target: rpc_target(),
        },
    )
    .await;

    ensure!(
        matches!(output.health, ChannelSourceProbeFact::Observed(())),
        "health fact was not retained"
    );
    ensure!(
        matches!(
            output.channel_id,
            ChannelSourceProbeFact::Observed(ref channel_id) if channel_id == &"8".repeat(64)
        ),
        "Channel identity fact was not retained"
    );
    ensure!(
        matches!(
            output.head,
            ChannelSourceProbeFact::Observed(Some(ref block)) if block.block_id == 42
        ),
        "head fact was not retained"
    );
    Ok(())
}

#[tokio::test]
async fn sequencer_module_probe_is_explicitly_unsupported() -> Result<()> {
    let transport: Arc<dyn ChannelSourceProbeTransport> =
        Arc::new(DefaultChannelSourceProbeTransport);
    let result = transport
        .health(
            ChannelSourceRole::Sequencer,
            ChannelSourceTarget::Module {
                module_id: super::super::layer::module_id_for_role(
                    crate::source_routing::channel_sources::ChannelSourceRole::Sequencer,
                )
                .to_owned(),
            },
        )
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

fn source_id(character: char) -> String {
    format!("src_{}", character.to_string().repeat(32))
}
