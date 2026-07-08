use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::{
    TransactionSummary, decode_account_data_hex_with_idl, decode_event_data_hex_with_idl,
    decode_instruction_words_with_idl, inspect_transaction_summary_with_idl,
    normalize_program_id_hex,
    program_decode::{
        ProgramDecodeCandidate,
        select_account_decode_session as select_account_decode_session_report,
        select_transaction_decode_session as select_transaction_decode_session_report,
        spel_idl_report,
    },
    program_file_info,
    support::args::Args,
};

use super::super::value::to_value;
use super::RuntimeMethodEntry;

pub(super) const METHOD_CATALOG: &[RuntimeMethodEntry] = &[
    RuntimeMethodEntry::sync("decodeTransactionSummary", decode_transaction_summary),
    RuntimeMethodEntry::sync("decodeAccount", decode_account),
    RuntimeMethodEntry::sync("selectAccountDecodeSession", select_account_decode_session),
    RuntimeMethodEntry::sync(
        "selectTransactionDecodeSession",
        select_transaction_decode_session,
    ),
    RuntimeMethodEntry::sync(
        "resolveAccountDecodeSession",
        resolve_account_decode_session,
    ),
    RuntimeMethodEntry::sync(
        "resolveTransactionDecodeSession",
        resolve_transaction_decode_session,
    ),
    RuntimeMethodEntry::sync("decodeInstruction", decode_instruction),
    RuntimeMethodEntry::sync("decodeEvent", decode_event),
    RuntimeMethodEntry::sync("spelIdl", spel_idl),
    RuntimeMethodEntry::sync("programFile", program_file),
    RuntimeMethodEntry::sync("normalizeProgramId", normalize_program_id),
];

pub(super) fn decode_transaction_summary(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let summary: TransactionSummary = serde_json::from_value(
        args.value(0)
            .cloned()
            .context("transaction summary is required")?,
    )
    .context("failed to parse transaction summary")?;
    to_value(inspect_transaction_summary_with_idl(
        &summary,
        args.string(1, "IDL JSON")?,
    )?)
}

pub(super) fn decode_account(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(decode_account_data_hex_with_idl(
        args.string(1, "IDL JSON")?,
        args.optional_string(2),
        args.string(0, "account data hex")?,
        None,
    )?)
}

pub(super) fn resolve_account_decode_session(args: Value) -> Result<Value> {
    select_account_decode_session(args)
}

pub(super) fn select_account_decode_session(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let candidates: Vec<ProgramDecodeCandidate> = serde_json::from_value(
        args.value(2)
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new())),
    )
    .context("failed to parse decode candidates")?;
    to_value(select_account_decode_session_report(
        args.optional_string(1),
        args.optional_string(3),
        args.string(0, "account data hex")?,
        &candidates,
    ))
}

pub(super) fn resolve_transaction_decode_session(args: Value) -> Result<Value> {
    select_transaction_decode_session(args)
}

pub(super) fn select_transaction_decode_session(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let summary: TransactionSummary = serde_json::from_value(
        args.value(0)
            .cloned()
            .context("transaction summary is required")?,
    )
    .context("failed to parse transaction summary")?;
    let candidates: Vec<ProgramDecodeCandidate> = serde_json::from_value(
        args.value(1)
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new())),
    )
    .context("failed to parse decode candidates")?;
    to_value(select_transaction_decode_session_report(
        &summary,
        &candidates,
    ))
}

pub(super) fn decode_instruction(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let words: Vec<u32> = serde_json::from_value(
        args.value(1)
            .cloned()
            .context("instruction words are required")?,
    )
    .context("failed to parse instruction words")?;
    let accounts: Vec<String> = serde_json::from_value(
        args.value(3)
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new())),
    )
    .context("failed to parse instruction accounts")?;
    to_value(decode_instruction_words_with_idl(
        args.string(2, "IDL JSON")?,
        args.string(0, "program id")?,
        &words,
        &accounts,
    )?)
}

pub(super) fn decode_event(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(decode_event_data_hex_with_idl(
        args.string(1, "IDL JSON")?,
        args.optional_string(2),
        args.string(0, "event data hex")?,
    )?)
}

pub(super) fn spel_idl(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(spel_idl_report(args.string(0, "IDL JSON")?)?)
}

pub(super) fn program_file(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(program_file_info(args.string(0, "program path")?)?)
}

pub(super) fn normalize_program_id(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(normalize_program_id_hex(args.string(0, "program id")?)?)
}
