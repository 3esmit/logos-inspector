use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::{
    TransactionSummary, decode_account_data_hex_with_idl, decode_event_data_hex_with_idl,
    idl_decode::spel_idl_report,
    inspect_transaction_summary_with_idl,
    inspection::l2::lez::{
        ProgramDecodeCandidate, resolve_account_decode_session, resolve_transaction_decode_session,
    },
    normalize_program_id_hex, program_file_info,
    source_routing::Args,
};

use super::super::bridge::to_value;

pub(super) fn try_handle(method: &str, args: Value) -> Result<Option<Value>> {
    let value = match method {
        "decodeTransactionSummary" => {
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
            )?)?
        }
        "decodeAccount" => {
            let args = Args::new(args)?;
            to_value(decode_account_data_hex_with_idl(
                args.string(1, "IDL JSON")?,
                args.optional_string(2),
                args.string(0, "account data hex")?,
                None,
            )?)?
        }
        "resolveAccountDecodeSession" => {
            let args = Args::new(args)?;
            let candidates: Vec<ProgramDecodeCandidate> = serde_json::from_value(
                args.value(2)
                    .cloned()
                    .unwrap_or_else(|| Value::Array(Vec::new())),
            )
            .context("failed to parse decode candidates")?;
            to_value(resolve_account_decode_session(
                args.optional_string(1),
                args.string(0, "account data hex")?,
                &candidates,
            ))?
        }
        "resolveTransactionDecodeSession" => {
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
            to_value(resolve_transaction_decode_session(&summary, &candidates))?
        }
        "decodeEvent" => {
            let args = Args::new(args)?;
            to_value(decode_event_data_hex_with_idl(
                args.string(1, "IDL JSON")?,
                args.optional_string(2),
                args.string(0, "event data hex")?,
            )?)?
        }
        "spelIdl" => {
            let args = Args::new(args)?;
            to_value(spel_idl_report(args.string(0, "IDL JSON")?)?)?
        }
        "programFile" => {
            let args = Args::new(args)?;
            to_value(program_file_info(args.string(0, "program path")?)?)?
        }
        "normalizeProgramId" => {
            let args = Args::new(args)?;
            to_value(normalize_program_id_hex(args.string(0, "program id")?)?)?
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}
