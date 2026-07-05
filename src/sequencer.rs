use anyhow::{Context as _, Result};
use common::transaction::LeeTransaction;
use sequencer_service_rpc::{RpcClient as _, SequencerClientBuilder};
use serde_json::{Value, json};

use crate::{
    BlockSummary, ProgramIdEntry, TransactionIdlInspectionReport, TransactionInspectionReport,
    TransactionSummary, TransactionTraceReport, decode_sequencer_block, inspect_transaction,
    inspect_transaction_summary_with_idl, json_rpc_result, parse_hash, raw_json_rpc,
    summarize_block, summarize_transaction, trace_transaction_summary,
    trace_transaction_summary_with_idl,
};

use crate::programs::program_entries;

pub async fn sequencer_health(endpoint: &str) -> Result<()> {
    sequencer_client(endpoint)?
        .check_health()
        .await
        .context("sequencer health check failed")
}

pub async fn last_sequencer_block_id(endpoint: &str) -> Result<u64> {
    sequencer_client(endpoint)?
        .get_last_block_id()
        .await
        .context("failed to fetch last sequencer block id")
}

pub async fn sequencer_program_ids(endpoint: &str) -> Result<Vec<ProgramIdEntry>> {
    let programs = sequencer_client(endpoint)?
        .get_program_ids()
        .await
        .context("failed to fetch sequencer program ids")?;
    Ok(program_entries(programs))
}

pub async fn sequencer_block(endpoint: &str, block_id: u64) -> Result<Option<BlockSummary>> {
    let response = raw_json_rpc(endpoint, "getBlock", Value::Array(vec![json!(block_id)]))
        .await
        .with_context(|| format!("failed to fetch sequencer block {block_id}"))?;
    let Some(result) = json_rpc_result(&response, "getBlock")? else {
        return Ok(None);
    };
    let encoded = result
        .as_str()
        .context("sequencer getBlock result was not a base64 block")?;
    let block = decode_sequencer_block(encoded)
        .with_context(|| format!("failed to decode sequencer block {block_id}"))?;
    Ok(Some(block))
}

pub async fn sequencer_blocks(
    endpoint: &str,
    before: Option<u64>,
    limit: u64,
) -> Result<Vec<BlockSummary>> {
    let limit = limit.min(50);
    if limit == 0 {
        return Ok(Vec::new());
    }

    let end_block_id = match before {
        Some(0) => return Ok(Vec::new()),
        Some(block_id) => block_id.saturating_sub(1),
        None => last_sequencer_block_id(endpoint).await?,
    };
    let start_block_id = end_block_id.saturating_sub(limit.saturating_sub(1));
    let blocks = sequencer_client(endpoint)?
        .get_block_range(start_block_id, end_block_id)
        .await
        .with_context(|| {
            format!("failed to fetch sequencer block range {start_block_id}..={end_block_id}")
        })?;
    Ok(blocks.iter().rev().map(summarize_block).collect())
}

pub async fn sequencer_transaction(
    endpoint: &str,
    tx_hash: &str,
) -> Result<Option<TransactionSummary>> {
    let tx = fetch_sequencer_transaction(endpoint, tx_hash).await?;
    Ok(tx.as_ref().map(summarize_transaction))
}

pub async fn sequencer_transaction_inspection(
    endpoint: &str,
    tx_hash: &str,
) -> Result<Option<TransactionInspectionReport>> {
    let tx = fetch_sequencer_transaction(endpoint, tx_hash).await?;
    Ok(tx.as_ref().map(inspect_transaction))
}

pub async fn sequencer_transaction_inspection_with_idl(
    endpoint: &str,
    tx_hash: &str,
    idl_json: &str,
) -> Result<Option<TransactionIdlInspectionReport>> {
    let tx = fetch_sequencer_transaction(endpoint, tx_hash).await?;
    tx.as_ref()
        .map(|tx| inspect_transaction_summary_with_idl(&summarize_transaction(tx), idl_json))
        .transpose()
}

pub async fn sequencer_transaction_trace(
    endpoint: &str,
    tx_hash: &str,
) -> Result<Option<TransactionTraceReport>> {
    let tx = fetch_sequencer_transaction(endpoint, tx_hash).await?;
    Ok(tx
        .as_ref()
        .map(|tx| trace_transaction_summary(&summarize_transaction(tx))))
}

pub async fn sequencer_transaction_trace_with_idl(
    endpoint: &str,
    tx_hash: &str,
    idl_json: &str,
) -> Result<Option<TransactionTraceReport>> {
    let tx = fetch_sequencer_transaction(endpoint, tx_hash).await?;
    tx.as_ref()
        .map(|tx| trace_transaction_summary_with_idl(&summarize_transaction(tx), idl_json))
        .transpose()
}

pub(crate) fn sequencer_client(endpoint: &str) -> Result<sequencer_service_rpc::SequencerClient> {
    SequencerClientBuilder::default()
        .build(endpoint)
        .with_context(|| format!("failed to build sequencer client for {endpoint}"))
}

async fn fetch_sequencer_transaction(
    endpoint: &str,
    tx_hash: &str,
) -> Result<Option<LeeTransaction>> {
    let hash = parse_hash(tx_hash, "transaction hash")?;
    sequencer_client(endpoint)?
        .get_transaction(hash)
        .await
        .with_context(|| format!("failed to fetch sequencer transaction {tx_hash}"))
}
