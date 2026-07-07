use std::collections::BTreeSet;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use lee::{AccountId, PublicKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::TransactionSummary;
use crate::{
    enum_payload, normalize_account_id_text, normalize_program_id_hex, value_list_strings,
    value_to_string,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AccountTransactionSummary {
    pub index: usize,
    pub hash: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program_id_hex: Option<String>,
    pub account_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signer_account_ids: Vec<String>,
    pub nonces: Vec<String>,
    pub instruction_data: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transfer_outputs: Vec<TransactionTransferOutputSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytecode_len: Option<usize>,
    pub raw: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransactionTransferOutputSummary {
    pub recipient: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
}

impl From<&AccountTransactionSummary> for TransactionSummary {
    fn from(summary: &AccountTransactionSummary) -> Self {
        Self {
            hash: summary.hash.clone(),
            kind: summary.kind.clone(),
            program_id_hex: summary.program_id_hex.clone(),
            account_ids: summary.account_ids.clone(),
            nonces: summary.nonces.clone(),
            instruction_data: summary.instruction_data.clone(),
            bytecode_len: summary.bytecode_len,
            raw_signature_valid: None,
            message_prehash: None,
            prehash_signature_valid: None,
        }
    }
}

pub(crate) fn summarize_indexer_transaction(
    value: &Value,
    index: usize,
) -> AccountTransactionSummary {
    if let Some(kind) = compact_transaction_kind(value) {
        return AccountTransactionSummary {
            index,
            hash: value
                .get("hash")
                .map(value_to_string)
                .unwrap_or_else(|| "-".to_owned()),
            kind,
            direction: None,
            program_id_hex: value
                .get("program_id")
                .map(value_to_string)
                .map(|program_id| normalize_program_id_hex(&program_id).unwrap_or(program_id)),
            account_ids: compact_transaction_account_field_strings(value, "account_id"),
            signer_account_ids: transaction_signer_account_ids(value),
            nonces: compact_transaction_account_field_strings(value, "nonce"),
            instruction_data: value_list_u32(value.get("instruction_data")),
            transfer_outputs: summarize_transfer_outputs(value),
            bytecode_len: value_usize(value.get("bytecode_size")),
            raw: value.clone(),
        };
    }

    let (kind, payload) = enum_payload(value);
    let empty = Value::Null;
    let message = payload.get("message").unwrap_or(&empty);
    let bytecode_len = message.get("bytecode").and_then(|bytecode| match bytecode {
        Value::Array(items) => Some(items.len()),
        Value::String(value) => BASE64_STANDARD.decode(value).ok().map(|bytes| bytes.len()),
        _ => None,
    });
    AccountTransactionSummary {
        index,
        hash: payload
            .get("hash")
            .map(value_to_string)
            .unwrap_or_else(|| "-".to_owned()),
        kind: kind.to_owned(),
        direction: None,
        program_id_hex: message
            .get("program_id")
            .map(value_to_string)
            .map(|program_id| normalize_program_id_hex(&program_id).unwrap_or(program_id)),
        account_ids: indexer_transaction_account_ids(message),
        signer_account_ids: transaction_signer_account_ids(payload),
        nonces: value_list_strings(message.get("nonces")),
        instruction_data: value_list_u32(message.get("instruction_data")),
        transfer_outputs: summarize_transfer_outputs(value),
        bytecode_len,
        raw: value.clone(),
    }
}

pub(crate) fn with_account_direction(
    mut summary: AccountTransactionSummary,
    account_id: &str,
) -> AccountTransactionSummary {
    let Some(normalized_account_id) = normalize_account_id_text(account_id) else {
        return summary;
    };
    if summary.signer_account_ids.iter().any(|signer| {
        normalize_account_id_text(signer).as_deref() == Some(normalized_account_id.as_str())
    }) {
        summary.direction = Some("outgoing".to_owned());
    } else if summary.account_ids.iter().any(|account| {
        normalize_account_id_text(account).as_deref() == Some(normalized_account_id.as_str())
    }) {
        summary.direction = Some("incoming".to_owned());
    }
    summary
}

fn indexer_transaction_account_ids(message: &Value) -> Vec<String> {
    let mut account_ids = value_list_strings(message.get("account_ids"));
    for account_id in value_list_strings(message.get("public_account_ids")) {
        if !account_ids.iter().any(|value| value == &account_id) {
            account_ids.push(account_id);
        }
    }
    account_ids
}

fn compact_transaction_kind(value: &Value) -> Option<String> {
    value
        .get("type")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|kind| !kind.is_empty())
        .map(ToOwned::to_owned)
}

fn compact_transaction_account_field_strings(value: &Value, field: &str) -> Vec<String> {
    value
        .get("accounts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|account| account.get(field))
        .map(value_to_string)
        .collect()
}

fn value_list_u32(value: Option<&Value>) -> Vec<u32> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| {
                item.as_u64()
                    .and_then(|value| u32::try_from(value).ok())
                    .or_else(|| item.as_str().and_then(parse_u32_text))
            })
            .collect(),
        Some(Value::String(value)) => value.split(',').filter_map(parse_u32_text).collect(),
        Some(value) => value
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .into_iter()
            .collect(),
        None => Vec::new(),
    }
}

fn value_usize(value: Option<&Value>) -> Option<usize> {
    value
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|value| value.trim().parse().ok()))
        })
        .and_then(|value| usize::try_from(value).ok())
}

fn parse_u32_text(value: &str) -> Option<u32> {
    value.trim().parse().ok()
}

fn transaction_signer_account_ids(payload: &Value) -> Vec<String> {
    let Some(signatures) = payload
        .get("witness_set")
        .and_then(|witness_set| witness_set.get("signatures_and_public_keys"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    signatures
        .iter()
        .filter_map(transaction_signature_public_key)
        .filter_map(|public_key| public_key.parse::<PublicKey>().ok())
        .map(|public_key| AccountId::from(&public_key).to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn transaction_signature_public_key(signature: &Value) -> Option<String> {
    match signature {
        Value::Array(items) => items.get(1).map(value_to_string),
        Value::Object(object) => object
            .get("public_key")
            .or_else(|| object.get("publicKey"))
            .map(value_to_string),
        _ => None,
    }
}

fn summarize_transfer_outputs(value: &Value) -> Vec<TransactionTransferOutputSummary> {
    let mut outputs = Vec::new();
    collect_transfer_outputs(value, &mut outputs);
    outputs
}

fn collect_transfer_outputs(value: &Value, outputs: &mut Vec<TransactionTransferOutputSummary>) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if transfer_outputs_key(key) {
                    if let Some(items) = value.as_array() {
                        outputs.extend(items.iter().filter_map(transaction_transfer_output));
                    }
                } else {
                    collect_transfer_outputs(value, outputs);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_transfer_outputs(item, outputs);
            }
        }
        _ => {}
    }
}

fn transaction_transfer_output(value: &Value) -> Option<TransactionTransferOutputSummary> {
    let Value::Object(object) = value else {
        return None;
    };
    let recipient = first_output_field(
        object,
        &[
            "recipient",
            "recipient_id",
            "recipientId",
            "account_id",
            "accountId",
            "to",
            "address",
            "public_key",
            "publicKey",
        ],
    )?;
    Some(TransactionTransferOutputSummary {
        recipient,
        amount: first_output_field(object, &["amount", "value", "quantity", "balance"]),
    })
}

fn first_output_field(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key))
        .map(value_to_string)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty() && value != "null")
}

fn transfer_outputs_key(key: &str) -> bool {
    matches!(key, "outputs" | "transfer_outputs" | "transferOutputs")
}
