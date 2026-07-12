use serde::{Deserialize, Serialize};

use super::{L2Retrieval, L2RouteFinality};
use crate::{
    inspection::{ZoneSourceRole, l2::ZoneL2AccountActivityOrder},
    lez::{
        ProgramIdEntry, TransactionInspectionReport, TransactionSummary, TransactionTraceReport,
        TransactionTransferOutputSummary, TransferRecipientSummary,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2BlockSummary {
    pub block_id: u64,
    pub block_hash: String,
    pub parent_hash: String,
    pub timestamp: u64,
    pub bedrock_status: String,
    pub transaction_count: usize,
}

impl L2BlockSummary {
    #[must_use]
    pub fn canonical_key(&self) -> String {
        format!("block:{}:{}", self.block_id, self.block_hash)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2SourceObservation {
    pub source_id: String,
    pub source_role: ZoneSourceRole,
    pub source_config_revision: u64,
    pub finality: L2RouteFinality,
    pub retrieval: L2Retrieval,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2BlockRow {
    pub summary: L2BlockSummary,
    pub observations: Vec<L2SourceObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2SourceHead {
    pub source_id: String,
    pub source_role: ZoneSourceRole,
    pub block_id: u64,
    pub block_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2BlocksPage {
    pub rows: Vec<L2BlockRow>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub distinct_block_ids: usize,
    pub source_heads: Vec<L2SourceHead>,
}

#[derive(Debug, Clone, Serialize)]
pub struct L2BlockDetail {
    pub summary: L2BlockSummary,
    pub transactions: Vec<TransactionSummary>,
    pub source: L2SourceObservation,
}

#[derive(Debug, Clone, Serialize)]
pub struct L2TransactionDetail {
    pub transaction: TransactionSummary,
    pub inspection: TransactionInspectionReport,
    pub source: L2SourceObservation,
}

#[derive(Debug, Clone, Serialize)]
pub struct L2TransactionTrace {
    pub transaction: TransactionSummary,
    pub trace: TransactionTraceReport,
    pub source: L2SourceObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L2AccountExistence {
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2AccountValue {
    pub account_id: String,
    pub account_id_base58: String,
    pub account_id_hex: String,
    pub balance: String,
    pub nonce: String,
    pub owner_program_base58: String,
    pub owner_program_hex: String,
    pub data_hex: String,
    pub existence: L2AccountExistence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2BlockAnchor {
    pub block_id: u64,
    pub block_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L2AccountAnchorState {
    Exact,
    Moving,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2AccountSnapshotData {
    pub account: L2AccountValue,
    pub anchor: Option<L2BlockAnchor>,
    pub after_anchor: Option<L2BlockAnchor>,
    pub anchor_state: L2AccountAnchorState,
    pub source: L2SourceObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2AccountActivityRow {
    pub index: usize,
    pub transaction_id: String,
    pub kind: String,
    pub direction: Option<String>,
    pub program_id_hex: Option<String>,
    pub account_ids: Vec<String>,
    pub signer_account_ids: Vec<String>,
    pub nonces: Vec<String>,
    pub instruction_data: Vec<u32>,
    pub transfer_outputs: Vec<TransactionTransferOutputSummary>,
    pub bytecode_len: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2AccountActivityPage {
    pub account_id: String,
    pub order: ZoneL2AccountActivityOrder,
    pub rows: Vec<L2AccountActivityRow>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct L2ProgramsData {
    pub programs: Vec<ProgramIdEntry>,
    pub source: L2SourceObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2CommitmentProofData {
    pub commitment_hex: String,
    pub leaf_index: u64,
    pub sibling_hashes: Vec<String>,
    pub source: L2SourceObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2AccountNonce {
    pub account_id: String,
    pub nonce: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2AccountNoncesData {
    pub rows: Vec<L2AccountNonce>,
    pub source: L2SourceObservation,
}

#[derive(Debug, Clone, Serialize)]
pub struct L2TransfersPage {
    pub recipients: Vec<TransferRecipientSummary>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub newest_block: Option<u64>,
    pub oldest_block: Option<u64>,
    pub scanned_blocks: usize,
    pub finalized: bool,
}
