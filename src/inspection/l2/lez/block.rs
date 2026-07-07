use anyhow::{Context as _, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use common::block::{BedrockStatus, Block, BlockBody, BlockHeader};
use serde::Serialize;

use super::{TransactionSummary, summarize_transaction};

#[derive(Debug, Clone, Serialize)]
pub struct BlockSummary {
    pub block_id: u64,
    pub header_hash: String,
    pub parent_hash: String,
    pub timestamp: u64,
    pub bedrock_status: String,
    pub tx_count: usize,
    pub transactions: Vec<TransactionSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decode_warning: Option<String>,
}

pub(crate) fn decode_sequencer_block(encoded: &str) -> Result<BlockSummary> {
    let bytes = BASE64_STANDARD
        .decode(encoded)
        .context("sequencer block result was not valid base64")?;

    let block = borsh::from_slice::<Block>(&bytes)
        .context("sequencer block result did not match LEZ block layout")?;
    Ok(summarize_block(&block))
}

#[must_use]
pub fn summarize_block(block: &Block) -> BlockSummary {
    summarize_block_parts(&block.header, &block.body, &block.bedrock_status, None)
}

#[must_use]
fn summarize_block_parts(
    header: &BlockHeader,
    body: &BlockBody,
    bedrock_status: &BedrockStatus,
    decode_warning: Option<String>,
) -> BlockSummary {
    BlockSummary {
        block_id: header.block_id,
        header_hash: header.hash.to_string(),
        parent_hash: header.prev_block_hash.to_string(),
        timestamp: header.timestamp,
        bedrock_status: format!("{bedrock_status:?}"),
        tx_count: body.transactions.len(),
        transactions: body
            .transactions
            .iter()
            .map(summarize_transaction)
            .collect(),
        decode_warning,
    }
}
