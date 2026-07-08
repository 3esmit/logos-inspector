use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context as _, Result, bail};
use serde::Serialize;
use serde_json::Value;

use super::{LocalWalletInstructionRequest, values::type_label};

#[derive(Debug, Clone, Serialize)]
pub struct LocalWalletInstructionPlanReport {
    pub instruction: String,
    pub instructions: Vec<String>,
    pub accounts: Vec<InstructionPlanField>,
    pub args: Vec<InstructionPlanField>,
    pub private_mode: bool,
    pub program_binary_required: bool,
    pub inputs_complete: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstructionPlanField {
    pub name: String,
    pub label: String,
    pub placeholder: String,
    pub required: bool,
    pub rest: bool,
    pub kind: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub type_label: String,
}

pub(super) struct InstructionSelection {
    pub(super) name: String,
    pub(super) variant_index: usize,
    pub(super) instruction: Value,
}

pub(super) fn instruction_plan(
    request: &LocalWalletInstructionRequest,
) -> Result<LocalWalletInstructionPlanReport> {
    let idl = parse_idl(&request.idl_json)?;
    let instructions = instruction_rows(&idl)?;
    let names = instruction_names(instructions);
    let selection = select_instruction(instructions, &request.instruction)?;
    let accounts = account_fields(&selection.instruction);
    let args = arg_fields(&selection.instruction);
    let private_mode = request_private_mode(&request.accounts);
    let inputs_complete = fields_complete(&accounts, &request.accounts)
        && fields_complete(&args, &request.args)
        && (!private_mode || !request.program_binary.trim().is_empty());

    Ok(LocalWalletInstructionPlanReport {
        instruction: selection.name,
        instructions: names,
        accounts,
        args,
        private_mode,
        program_binary_required: private_mode,
        inputs_complete,
    })
}

pub(super) fn parse_idl(raw: &str) -> Result<Value> {
    serde_json::from_str(raw).context("failed to parse IDL JSON")
}

pub(super) fn instruction_rows(idl: &Value) -> Result<&[Value]> {
    idl.get("instructions")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .context("IDL has no instructions array")
}

pub(super) fn select_instruction(
    instructions: &[Value],
    selected: &str,
) -> Result<InstructionSelection> {
    let selected = selected.trim();
    if selected.is_empty() {
        bail!("instruction is required");
    }
    let (variant_index, instruction) = instructions
        .iter()
        .enumerate()
        .find(|(_, instruction)| {
            let name = instruction_name(instruction);
            name == selected || super::values::kebab_name(name) == selected
        })
        .with_context(|| format!("IDL instruction `{selected}` not found"))?;
    Ok(InstructionSelection {
        name: instruction_name(instruction).to_owned(),
        variant_index,
        instruction: instruction.clone(),
    })
}

pub(super) fn instruction_name(instruction: &Value) -> &str {
    instruction
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}

fn instruction_names(instructions: &[Value]) -> Vec<String> {
    instructions
        .iter()
        .map(instruction_name)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn account_fields(instruction: &Value) -> Vec<InstructionPlanField> {
    let accounts = instruction
        .get("accounts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for account in accounts
        .iter()
        .filter(|account| account.get("pda").is_none())
    {
        let name = account.get("name").and_then(Value::as_str).unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let rest = account.get("rest").and_then(Value::as_bool) == Some(true);
        let signer = account.get("signer").and_then(Value::as_bool) == Some(true);
        rows.push(InstructionPlanField {
            name: name.to_owned(),
            label: if signer {
                format!("{} signer", display_label(name))
            } else {
                display_label(name)
            },
            placeholder: if rest {
                "Public/<id>, Private/<id>".to_owned()
            } else {
                "Public/<id> or Private/<id>".to_owned()
            },
            required: !rest,
            rest,
            kind: "account".to_owned(),
            type_label: String::new(),
        });
        seen.insert(name.to_owned());
    }
    for account in &accounts {
        let seeds = account
            .get("pda")
            .and_then(|pda| pda.get("seeds"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten();
        for seed in seeds {
            if seed.get("kind").and_then(Value::as_str) != Some("account") {
                continue;
            }
            let path = seed.get("path").and_then(Value::as_str).unwrap_or("");
            if path.is_empty() || seen.contains(path) {
                continue;
            }
            rows.push(InstructionPlanField {
                name: path.to_owned(),
                label: format!("{} seed", display_label(path)),
                placeholder: "Public/<id>".to_owned(),
                required: true,
                rest: false,
                kind: "account".to_owned(),
                type_label: String::new(),
            });
            seen.insert(path.to_owned());
        }
    }
    rows
}

fn arg_fields(instruction: &Value) -> Vec<InstructionPlanField> {
    instruction
        .get("args")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|arg| {
            let name = arg.get("name").and_then(Value::as_str)?;
            let ty = arg.get("type")?;
            let type_label = type_label(ty);
            Some(InstructionPlanField {
                name: name.to_owned(),
                label: format!("{} ({type_label})", display_label(name)),
                placeholder: placeholder_for_type(&type_label),
                required: true,
                rest: false,
                kind: "arg".to_owned(),
                type_label,
            })
        })
        .collect()
}

fn fields_complete(fields: &[InstructionPlanField], values: &BTreeMap<String, String>) -> bool {
    fields.iter().filter(|field| field.required).all(|field| {
        values
            .get(&field.name)
            .is_some_and(|value| !value.trim().is_empty())
    })
}

fn request_private_mode(accounts: &BTreeMap<String, String>) -> bool {
    accounts
        .values()
        .any(|value| value.trim().to_ascii_lowercase().starts_with("private/"))
}

fn placeholder_for_type(label: &str) -> String {
    if label == "bool" {
        "true or false".to_owned()
    } else if label.starts_with("[u8;") {
        "0x...".to_owned()
    } else if label.starts_with("Vec<") {
        "comma values".to_owned()
    } else {
        "value".to_owned()
    }
}

fn display_label(name: &str) -> String {
    let mut label = String::new();
    let mut previous_lower = false;
    for character in name.chars() {
        if character == '_' || character == '-' {
            label.push(' ');
            previous_lower = false;
            continue;
        }
        if character.is_ascii_uppercase() && previous_lower {
            label.push(' ');
        }
        if label.is_empty() {
            label.push(character.to_ascii_uppercase());
        } else {
            label.push(character);
        }
        previous_lower = character.is_ascii_lowercase() || character.is_ascii_digit();
    }
    label
}
