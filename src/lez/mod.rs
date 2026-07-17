mod accounts;
mod block;
mod indexer;
mod programs;
mod sequencer;
mod transactions;
mod transfers;

#[derive(Debug)]
pub(crate) struct EvidenceProtocolError {
    message: &'static str,
}

#[derive(Debug)]
pub(crate) struct EvidenceCapabilityError {
    message: &'static str,
}

impl std::fmt::Display for EvidenceCapabilityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for EvidenceCapabilityError {}

impl std::fmt::Display for EvidenceProtocolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for EvidenceProtocolError {}

pub(crate) fn evidence_protocol_error(message: &'static str) -> anyhow::Error {
    anyhow::Error::new(EvidenceProtocolError { message })
}

pub(crate) fn is_evidence_protocol_error(error: &anyhow::Error) -> bool {
    error.downcast_ref::<EvidenceProtocolError>().is_some()
}

pub(crate) fn evidence_capability_error(message: &'static str) -> anyhow::Error {
    anyhow::Error::new(EvidenceCapabilityError { message })
}

pub(crate) fn is_evidence_capability_error(error: &anyhow::Error) -> bool {
    error.downcast_ref::<EvidenceCapabilityError>().is_some()
}

pub use accounts::{
    AccountReport, SequencerAccountIdlReport, account_transactions_by_account, sequencer_account,
};
pub(crate) use accounts::{indexer_account_report, summarize_account_transaction};
pub(crate) use block::decode_sequencer_block;
pub use block::{BlockSummary, summarize_block};
pub use indexer::{
    AccountTransactionSummary, IndexerBlockReport, IndexerStatusReport,
    TransactionTransferOutputSummary, indexer_account_at_block, indexer_block_by_hash,
    indexer_block_by_id, indexer_blocks, indexer_finalized_block_id, indexer_health,
    indexer_transaction,
};
pub(crate) use indexer::{
    summarize_indexer_status_response, validated_indexer_module_block_for_hash,
    validated_indexer_module_block_for_id, validated_indexer_module_block_report,
    validated_indexer_module_transaction_summary,
};
pub use programs::{
    ProgramFileInfo, ProgramIdEntry, program_file_info, program_id_base58, program_id_hex,
};
pub use sequencer::{
    last_sequencer_block_id, sequencer_account_nonces, sequencer_block, sequencer_blocks,
    sequencer_channel_id, sequencer_commitment_proof, sequencer_health, sequencer_program_ids,
    sequencer_transaction,
};
pub use transactions::{
    TransactionIdlInspectionReport, TransactionInspectionReport, TransactionInspectionRow,
    TransactionInspectionSection, TransactionSummary, TransactionTraceRefs, TransactionTraceReport,
    TransactionTraceStep, inspect_transaction_summary, inspect_transaction_summary_with_idl,
    summarize_transaction, trace_transaction_summary, trace_transaction_summary_with_idl,
};
pub(crate) use transfers::transfer_recipient_summaries_from_blocks;
pub use transfers::{RecipientTransferSummary, TransferActivityPage, TransferRecipientSummary};
