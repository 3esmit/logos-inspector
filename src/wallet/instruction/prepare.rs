use std::{collections::BTreeMap, path::Path};

use anyhow::{Context as _, Result, bail};
use serde_json::Value;

use crate::normalize_program_id_hex;

use super::{
    LocalWalletInstructionRequest, ResolvedInstructionArg,
    accounts::resolve_accounts,
    model::{AccountPrivacy, InstructionMode, PreparedInstruction},
    plan::{parse_idl, select_instruction},
    values::{
        InstructionData, ParsedValue, named_value, parse_typed_value, program_id_from_hex,
        type_label,
    },
};

pub(super) fn prepare_instruction(
    request: &LocalWalletInstructionRequest,
) -> Result<PreparedInstruction> {
    let idl = parse_idl(&request.idl_json)?;
    let program_id_hex = normalize_program_id_hex(&request.program_id_hex)?;
    let program_id = program_id_from_hex(&program_id_hex)?;
    let selection = select_instruction(&idl, &request.instruction)?;
    let instruction = &selection.instruction;
    let instruction_name = selection.name;
    let args = instruction
        .get("args")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut parsed_args = BTreeMap::<String, ParsedValue>::new();
    let mut report_args = Vec::with_capacity(args.len());
    let mut fields = Vec::with_capacity(args.len());
    for arg in &args {
        let name = arg
            .get("name")
            .and_then(Value::as_str)
            .context("IDL arg missing name")?;
        let ty = arg
            .get("type")
            .with_context(|| format!("IDL arg `{name}` missing type"))?;
        let raw = named_value(&request.args, name)
            .with_context(|| format!("argument `{name}` is required"))?;
        let parsed = parse_typed_value(raw, ty)
            .with_context(|| format!("failed to parse argument `{name}` as {}", type_label(ty)))?;
        report_args.push(ResolvedInstructionArg {
            name: name.to_owned(),
            type_label: type_label(ty),
            value: parsed.report_value.clone(),
        });
        fields.push(parsed.dynamic.clone());
        parsed_args.insert(name.to_owned(), parsed);
    }

    let instruction_words = risc0_zkvm::serde::to_vec(&InstructionData {
        variant_index: selection.variant_index,
        fields: &fields,
    })
    .map_err(|error| anyhow::anyhow!("failed to serialize instruction data: {error}"))?;

    let accounts = resolve_accounts(instruction, request, &program_id, &parsed_args)?;
    let mode = if accounts
        .iter()
        .any(|account| account.privacy == AccountPrivacy::Private)
    {
        InstructionMode::Private
    } else {
        InstructionMode::Public
    };
    let program_binary = request.program_binary.trim().to_owned();
    if mode == InstructionMode::Private {
        if program_binary.is_empty() {
            bail!("private IDL instruction requires a program binary");
        }
        if !Path::new(&program_binary).is_file() {
            bail!("program binary is not reachable");
        }
    }

    Ok(PreparedInstruction {
        instruction: instruction_name,
        #[cfg(feature = "local-wallet-runtime")]
        program_id,
        program_id_hex,
        program_binary,
        program_binary_required: mode == InstructionMode::Private,
        mode,
        accounts,
        args: report_args,
        instruction_words,
    })
}
