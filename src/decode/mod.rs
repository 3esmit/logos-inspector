pub(crate) mod borsh;
pub(crate) mod idl_type;
pub(crate) mod instruction_codec;
mod reports;
mod selection;
mod session;
mod spel;

pub use reports::{
    AccountIdlDecodeReport, DecodedField, EventIdlDecodeReport, InstructionDecodeReport,
    decode_account_data_hex_with_idl, decode_event_data_hex_with_idl, decode_event_data_with_idl,
    decode_instruction_words_with_idl,
};
pub(crate) use selection::{select_account_decode_session, select_transaction_decode_session};
pub use session::{
    AccountDecodeSelection, ProgramDecodeCandidate, ResolvedAccountDecodeSession,
    ResolvedTransactionDecodeSession, SelectedDecodeEvidence, TransactionDecodeSelection,
    resolve_account_decode_session, resolve_transaction_decode_session,
};
pub use spel::{
    SpelAccountTypeSummary, SpelArgSummary, SpelIdlReport, SpelInstructionAccountSummary,
    SpelInstructionSummary, SpelPdaSummary, SpelTypeSummary, spel_idl_report,
};

#[cfg(test)]
mod transaction_tests;
