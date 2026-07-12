mod accounts;
mod block;
mod block_list_report;
mod idl_resolver;
mod indexer;
mod programs;
mod sequencer;
mod target_decode;
mod target_resolution;
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

#[cfg(test)]
mod transaction_decode_tests;

pub(crate) use accounts::account_report_with_optional_idl_decode;
pub use accounts::{
    AccountReport, SequencerAccountIdlReport, account_lookup, account_lookup_with_idl,
    account_transactions_by_account, sequencer_account, sequencer_account_with_idl,
};
pub(crate) use accounts::{indexer_account_report, summarize_account_transaction};
pub(crate) use block::decode_sequencer_block;
pub use block::{BlockSummary, summarize_block};
pub(crate) use block_list_report::block_list_report;
pub(crate) use idl_resolver::RegisteredIdlResolver;
pub use indexer::{
    AccountTransactionSummary, IndexerBlockReport, IndexerStatusReport,
    TransactionTransferOutputSummary, indexer_account_at_block, indexer_block_by_hash,
    indexer_block_by_id, indexer_blocks, indexer_finalized_block_id, indexer_health,
    indexer_status, indexer_transaction, indexer_transfer_recipients,
};
pub(crate) use indexer::{
    next_indexer_blocks_cursor, summarize_indexer_status_response, verified_indexer_block_report,
    verified_indexer_transaction_summary,
};
pub use programs::{
    ProgramFileInfo, ProgramIdEntry, program_file_info, program_id_base58, program_id_hex,
};
pub use sequencer::{
    last_sequencer_block_id, sequencer_account_nonces, sequencer_block, sequencer_blocks,
    sequencer_channel_id, sequencer_commitment_proof, sequencer_health, sequencer_program_ids,
    sequencer_transaction, sequencer_transaction_inspection,
    sequencer_transaction_inspection_with_idl, sequencer_transaction_trace,
    sequencer_transaction_trace_with_idl,
};
pub(crate) use target_resolution::LezTargetResolver;
pub(crate) use transactions::inspect_transaction;
pub use transactions::{
    TransactionIdlInspectionReport, TransactionInspectionReport, TransactionInspectionRow,
    TransactionInspectionSection, TransactionSummary, TransactionTraceRefs, TransactionTraceReport,
    TransactionTraceStep, inspect_transaction_summary, inspect_transaction_summary_with_idl,
    summarize_transaction, trace_transaction_summary, trace_transaction_summary_with_idl,
};
pub(crate) use transactions::{
    inspect_transaction_summary_with_optional_idl_decode, transaction_decode_input_from_summary,
};
pub(crate) use transfers::transfer_recipient_summaries_from_blocks;
pub use transfers::{RecipientTransferSummary, TransferActivityPage, TransferRecipientSummary};
