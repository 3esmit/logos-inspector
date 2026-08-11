use std::io::Cursor;

use anyhow::Result;
#[cfg(test)]
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use borsh::BorshDeserialize as _;
use common::block::{BedrockStatus, Block, BlockBody, BlockHeader};
use serde::Serialize;
use sha2::{Digest as _, Sha256};

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

#[cfg(test)]
pub(crate) fn decode_sequencer_block(encoded: &str) -> Result<BlockSummary> {
    let bytes = BASE64_STANDARD
        .decode(encoded)
        .map_err(|_| super::evidence_protocol_error("Sequencer block is not valid base64"))?;

    decode_sequencer_block_bytes(&bytes).map(|block| summarize_block(&block))
}

pub(crate) fn decode_sequencer_block_bytes(bytes: &[u8]) -> Result<Block> {
    let block = borsh::from_slice::<Block>(bytes)
        .map_err(|_| super::evidence_protocol_error("Sequencer block has invalid layout"))?;
    verify_block_content_hash(&block)?;
    Ok(block)
}

pub(crate) fn decode_sequencer_block_summary_bytes(bytes: &[u8]) -> Result<BlockSummary> {
    let mut remaining = bytes;
    let block = match Block::deserialize(&mut remaining) {
        Ok(block) if remaining.is_empty() => block,
        Ok(_) => {
            return Err(super::evidence_protocol_error(
                "Sequencer block has trailing bytes",
            ));
        }
        Err(_) => {
            let mut reader = Cursor::new(bytes);
            let header =
                common::block::BlockHeader::deserialize_reader(&mut reader).map_err(|_| {
                    super::evidence_protocol_error("Sequencer block has invalid header")
                })?;
            let transaction_count = u32::deserialize_reader(&mut reader).map_err(|_| {
                super::evidence_protocol_error("Sequencer block has invalid transaction count")
            })?;
            let transaction_count = usize::try_from(transaction_count).map_err(|_| {
                super::evidence_protocol_error("Sequencer block transaction count is too large")
            })?;
            return Ok(BlockSummary {
                block_id: header.block_id,
                header_hash: header.hash.to_string(),
                parent_hash: header.prev_block_hash.to_string(),
                timestamp: header.timestamp,
                bedrock_status: "Unknown".to_owned(),
                tx_count: transaction_count,
                transactions: Vec::new(),
                decode_warning: Some(
                    "Some Sequencer transactions could not be decoded; block header retained."
                        .to_owned(),
                ),
            });
        }
    };
    verify_block_content_hash(&block)?;
    Ok(summarize_block(&block))
}

pub(crate) fn verify_block_content_hash(block: &Block) -> Result<()> {
    const PREFIX: &[u8; 32] = b"/LEE/v0.3/Message/Block/\x00\x00\x00\x00\x00\x00\x00\x00";

    let hashable = common::block::HashableBlockData::from(block.clone());
    let encoded = borsh::to_vec(&hashable)
        .map_err(|_| super::evidence_protocol_error("LEZ block content cannot be encoded"))?;
    let mut digest = Sha256::new();
    digest.update(PREFIX);
    digest.update(encoded);
    let computed: [u8; 32] = digest.finalize().into();
    if computed != block.header.hash.0 {
        return Err(super::evidence_protocol_error(
            "LEZ block content hash does not match its header",
        ));
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use anyhow::ensure;

    use super::*;

    const TESTNET_LEGACY_BLOCK_1234: &str = "0gQAAAAAAADgBr/57T2VP8TvanoE/U28V0Cdzfe66q1YCY203VHHaPZH+D0d+RhX4Qtz8m7atlbEG6J5XguGFqEPUWLQ8+1kb3u3+Z4BAADGt772EW9LB3inITN2BUfOdP8fHmTlcvpFP45NvGI01KYmibPzb/BkLygy6fTsHB4Oc4XoVVMp+k7Rp8xdjpgGAQAAAADiMVjm57Su7ujTA26v18dZ5R2KCU2Ce5JXELoh3v+PRgMAAAAvTEVaL0Nsb2NrUHJvZ3JhbUFjY291bnQvMDAwMDAwMS9MRVovQ2xvY2tQcm9ncmFtQWNjb3VudC8wMDAwMDEwL0xFWi9DbG9ja1Byb2dyYW1BY2NvdW50LzAwMDAwNTAAAAAAAgAAAG97t/meAQAAAAAAAAI=";

    #[test]
    fn decode_sequencer_block_fixture_without_warning() {
        let summary = decode_sequencer_block(TESTNET_LEGACY_BLOCK_1234);

        assert!(summary.is_ok(), "{summary:?}");
        let Ok(summary) = summary else {
            return;
        };
        assert_eq!(summary.block_id, 1234);
        assert_eq!(summary.tx_count, 1);
        assert_eq!(summary.transactions.len(), 1);
        assert_eq!(summary.header_hash.len(), 64);
        assert_eq!(summary.parent_hash.len(), 64);
        assert_eq!(
            summary.transactions.first().map(|tx| tx.kind.as_str()),
            Some("Public")
        );
        assert_eq!(summary.bedrock_status, "Finalized");
        assert!(summary.decode_warning.is_none());
    }

    #[test]
    fn malformed_body_retains_verified_header_fields_with_warning() -> Result<()> {
        let key = lee::PrivateKey::try_new([7_u8; 32])?;
        let header = BlockHeader {
            block_id: 2480,
            prev_block_hash: common::HashType([1_u8; 32]),
            hash: common::HashType([2_u8; 32]),
            timestamp: 42,
            signature: lee::Signature::new(&key, &[0_u8; 32]),
        };
        let mut bytes = borsh::to_vec(&header)?;
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.push(1);

        let summary = decode_sequencer_block_summary_bytes(&bytes)?;
        ensure!(summary.block_id == 2480, "header block id was lost");
        ensure!(summary.tx_count == 1, "raw transaction count was lost");
        ensure!(
            summary.transactions.is_empty(),
            "malformed tx was fabricated"
        );
        ensure!(
            summary.bedrock_status == "Unknown",
            "fallback status was not explicit"
        );
        ensure!(
            summary.decode_warning.is_some(),
            "fallback warning was lost"
        );
        Ok(())
    }

    #[test]
    fn trailing_bytes_are_rejected_before_header_fallback() -> Result<()> {
        let mut bytes = BASE64_STANDARD.decode(TESTNET_LEGACY_BLOCK_1234)?;
        bytes.push(0);

        let result = decode_sequencer_block_summary_bytes(&bytes);
        let error = result
            .err()
            .map(|error| error.to_string())
            .ok_or_else(|| anyhow::anyhow!("trailing block bytes were accepted"))?;
        ensure!(
            error == "Sequencer block has trailing bytes",
            "unexpected trailing-byte error: {error}"
        );
        Ok(())
    }

    #[test]
    fn block_content_verification_rejects_tampering() -> Result<()> {
        let bytes = BASE64_STANDARD.decode(TESTNET_LEGACY_BLOCK_1234)?;
        let mut block = borsh::from_slice::<Block>(&bytes)?;
        block.header.timestamp = block.header.timestamp.saturating_add(1);

        let result = verify_block_content_hash(&block);
        ensure!(
            result.is_err(),
            "tampered block passed content verification"
        );
        let error = result.err().map(|error| error.to_string());
        ensure!(
            error.as_deref() == Some("LEZ block content hash does not match its header"),
            "tampered block returned unstable error"
        );
        let tampered_bytes = borsh::to_vec(&block)?;
        let summary_result = decode_sequencer_block_summary_bytes(&tampered_bytes);
        ensure!(
            summary_result.is_err(),
            "tampered fully decoded block was downgraded to a header-only summary"
        );
        Ok(())
    }
}
