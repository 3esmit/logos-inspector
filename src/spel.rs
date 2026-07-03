use std::collections::BTreeMap;

use anyhow::{Context as _, Result};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct SpelIdlReport {
    pub name: Option<String>,
    pub version: Option<String>,
    pub spec: Option<Value>,
    pub metadata: Option<Value>,
    pub counts: BTreeMap<String, usize>,
    pub instructions: Vec<SpelInstructionSummary>,
    pub accounts: Vec<SpelAccountTypeSummary>,
    pub types: Vec<SpelTypeSummary>,
    pub errors: Vec<Value>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpelInstructionSummary {
    pub name: String,
    pub discriminator: Option<Value>,
    pub execution: Option<String>,
    pub variant: Option<Value>,
    pub accounts: Vec<SpelInstructionAccountSummary>,
    pub args: Vec<SpelArgSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpelInstructionAccountSummary {
    pub name: String,
    pub writable: Option<bool>,
    pub signer: Option<bool>,
    pub init: Option<bool>,
    pub owner: Option<String>,
    pub rest: Option<bool>,
    pub visibility: Option<String>,
    pub pda: Option<SpelPdaSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpelArgSummary {
    pub name: String,
    pub type_label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpelPdaSummary {
    pub private: Option<bool>,
    pub seeds: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpelAccountTypeSummary {
    pub name: String,
    pub type_label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpelTypeSummary {
    pub name: String,
    pub type_label: String,
}

pub fn spel_idl_report(idl_json: &str) -> Result<SpelIdlReport> {
    let idl: Value = serde_json::from_str(idl_json).context("failed to parse SPEL IDL JSON")?;
    let object = idl
        .as_object()
        .context("SPEL IDL root must be a JSON object")?;
    let instructions = array_field(&idl, "instructions")
        .iter()
        .map(instruction_summary)
        .collect::<Vec<_>>();
    let accounts = array_field(&idl, "accounts")
        .iter()
        .map(account_type_summary)
        .collect::<Vec<_>>();
    let types = array_field(&idl, "types")
        .iter()
        .map(type_summary)
        .collect::<Vec<_>>();
    let errors = array_field(&idl, "errors").to_vec();

    let mut counts = BTreeMap::new();
    for key in ["instructions", "accounts", "types", "errors"] {
        counts.insert(key.to_owned(), array_field(&idl, key).len());
    }

    Ok(SpelIdlReport {
        name: string_field(&idl, "name"),
        version: stringish_field(&idl, "version"),
        spec: object.get("spec").cloned(),
        metadata: object.get("metadata").cloned(),
        counts,
        instructions,
        accounts,
        types,
        errors,
        warnings: Vec::new(),
    })
}

fn instruction_summary(value: &Value) -> SpelInstructionSummary {
    SpelInstructionSummary {
        name: string_field(value, "name").unwrap_or_else(|| "unknown".to_owned()),
        discriminator: value.get("discriminator").cloned(),
        execution: stringish_field(value, "execution"),
        variant: value.get("variant").cloned(),
        accounts: array_field(value, "accounts")
            .iter()
            .map(instruction_account_summary)
            .collect(),
        args: array_field(value, "args").iter().map(arg_summary).collect(),
    }
}

fn instruction_account_summary(value: &Value) -> SpelInstructionAccountSummary {
    SpelInstructionAccountSummary {
        name: string_field(value, "name").unwrap_or_else(|| "unknown".to_owned()),
        writable: bool_field(value, "writable"),
        signer: bool_field(value, "signer"),
        init: bool_field(value, "init"),
        owner: stringish_field(value, "owner"),
        rest: bool_field(value, "rest"),
        visibility: stringish_field(value, "visibility"),
        pda: value.get("pda").map(pda_summary),
    }
}

fn pda_summary(value: &Value) -> SpelPdaSummary {
    SpelPdaSummary {
        private: bool_field(value, "private"),
        seeds: array_field(value, "seeds")
            .iter()
            .map(seed_summary)
            .collect(),
    }
}

fn seed_summary(value: &Value) -> String {
    if let Some(kind) = stringish_field(value, "kind") {
        if let Some(value) = value.get("value").or_else(|| value.get("path")) {
            return format!("{kind}:{}", value_label(value));
        }
        return kind;
    }
    value_label(value)
}

fn arg_summary(value: &Value) -> SpelArgSummary {
    SpelArgSummary {
        name: string_field(value, "name").unwrap_or_else(|| "arg".to_owned()),
        type_label: value
            .get("type")
            .map(type_label)
            .unwrap_or_else(|| "unknown".to_owned()),
    }
}

fn account_type_summary(value: &Value) -> SpelAccountTypeSummary {
    SpelAccountTypeSummary {
        name: string_field(value, "name").unwrap_or_else(|| "unknown".to_owned()),
        type_label: value
            .get("type")
            .map(type_label)
            .unwrap_or_else(|| "unknown".to_owned()),
    }
}

fn type_summary(value: &Value) -> SpelTypeSummary {
    SpelTypeSummary {
        name: string_field(value, "name").unwrap_or_else(|| "unknown".to_owned()),
        type_label: value
            .get("type")
            .map(type_label)
            .unwrap_or_else(|| "unknown".to_owned()),
    }
}

fn array_field<'a>(value: &'a Value, field: &str) -> &'a [Value] {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn stringish_field(value: &Value, field: &str) -> Option<String> {
    value.get(field).map(value_label)
}

fn bool_field(value: &Value, field: &str) -> Option<bool> {
    value.get(field).and_then(Value::as_bool)
}

fn type_label(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Object(object) => {
            if let Some(inner) = object.get("vec") {
                return format!("Vec<{}>", type_label(inner));
            }
            if let Some(inner) = object.get("option") {
                return format!("Option<{}>", type_label(inner));
            }
            if let Some(inner) = object.get("defined") {
                return value_label(inner);
            }
            if let Some(inner) = object.get("array") {
                return format!("array {}", value_label(inner));
            }
            value.to_string()
        }
        Value::Array(_) | Value::Number(_) | Value::Bool(_) | Value::Null => value.to_string(),
    }
}

fn value_label(value: &Value) -> String {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_instruction_accounts_and_args() {
        let idl = r#"{
            "name":"demo",
            "version":"0.1.0",
            "instructions":[{
                "name":"mint",
                "discriminator":[1,2,3],
                "accounts":[{"name":"owner","signer":true}],
                "args":[{"name":"amount","type":"u64"}]
            }]
        }"#;

        let report = spel_idl_report(idl);
        assert!(report.is_ok(), "{report:#?}");
        let Some(report) = report.ok() else {
            return;
        };

        assert_eq!(report.name.as_deref(), Some("demo"));
        let instruction = report.instructions.first();
        assert_eq!(
            instruction.map(|instruction| instruction.name.as_str()),
            Some("mint")
        );
        let signer = instruction
            .and_then(|instruction| instruction.accounts.first())
            .and_then(|account| account.signer);
        assert_eq!(signer, Some(true));
        let arg_type = instruction
            .and_then(|instruction| instruction.args.first())
            .map(|arg| arg.type_label.as_str());
        assert_eq!(arg_type, Some("u64"));
    }
}
