use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::Value;

use super::indexer::IndexerBlockReport;
use crate::value_to_string;

#[derive(Debug, Clone, Serialize)]
pub struct TransferRecipientSummary {
    pub recipient: String,
    pub account_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub received: Option<String>,
    pub txs: usize,
    pub outputs: usize,
    pub references: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_slot: Option<u64>,
    pub source: String,
    pub transfers: Vec<RecipientTransferSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferActivityPage {
    pub recipients: Vec<TransferRecipientSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_before_block: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecipientTransferSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<u64>,
    pub tx_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Debug, Default)]
struct RecipientAggregate {
    received: Option<u128>,
    tx_hashes: BTreeSet<String>,
    outputs: usize,
    references: usize,
    last_slot: Option<u64>,
    transfers: Vec<RecipientTransferSummary>,
}

pub(crate) fn transfer_recipient_summaries_from_blocks(
    blocks: &[IndexerBlockReport],
) -> Vec<TransferRecipientSummary> {
    let mut account_refs = BTreeMap::new();
    let mut output_refs = BTreeMap::new();
    for block in blocks {
        for tx in &block.transactions {
            for output in decoded_transfer_outputs(&tx.raw) {
                let aggregate = output_refs
                    .entry(output.recipient)
                    .or_insert_with(RecipientAggregate::default);
                aggregate.tx_hashes.insert(tx.hash.clone());
                aggregate.outputs += 1;
                aggregate.received = add_optional_amounts(aggregate.received, output.amount);
                aggregate.last_slot = max_slot(aggregate.last_slot, block.block_id);
                aggregate.transfers.push(RecipientTransferSummary {
                    slot: block.block_id,
                    tx_hash: tx.hash.clone(),
                    block_hash: block.header_hash.clone(),
                    value: output.amount.map(|value| value.to_string()),
                });
            }
            for account_id in &tx.account_ids {
                let aggregate = account_refs
                    .entry(account_id.clone())
                    .or_insert_with(RecipientAggregate::default);
                aggregate.tx_hashes.insert(tx.hash.clone());
                aggregate.references += 1;
                aggregate.last_slot = max_slot(aggregate.last_slot, block.block_id);
                aggregate.transfers.push(RecipientTransferSummary {
                    slot: block.block_id,
                    tx_hash: tx.hash.clone(),
                    block_hash: block.header_hash.clone(),
                    value: None,
                });
            }
        }
    }
    if !output_refs.is_empty() {
        return transfer_recipient_summaries_from_aggregates(output_refs, "transfer_outputs");
    }
    transfer_recipient_summaries_from_aggregates(account_refs, "account_refs")
}

fn transfer_recipient_summaries_from_aggregates(
    aggregates: BTreeMap<String, RecipientAggregate>,
    source: &str,
) -> Vec<TransferRecipientSummary> {
    let mut rows = aggregates
        .into_iter()
        .map(|(recipient, mut aggregate)| {
            aggregate.transfers.sort_by(|left, right| {
                right
                    .slot
                    .cmp(&left.slot)
                    .then_with(|| right.tx_hash.cmp(&left.tx_hash))
                    .then_with(|| right.value.cmp(&left.value))
            });
            let account_ref = recipient;
            TransferRecipientSummary {
                recipient: account_ref.clone(),
                account_ref,
                received: aggregate.received.map(|value| value.to_string()),
                txs: aggregate.tx_hashes.len(),
                outputs: aggregate.outputs,
                references: aggregate.references,
                last_slot: aggregate.last_slot,
                source: source.to_owned(),
                transfers: aggregate.transfers,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        transfer_recipient_sort_key(right)
            .cmp(&transfer_recipient_sort_key(left))
            .then_with(|| left.recipient.cmp(&right.recipient))
    });
    rows
}

#[derive(Debug)]
struct DecodedTransferOutput {
    recipient: String,
    amount: Option<u128>,
}

fn decoded_transfer_outputs(value: &Value) -> Vec<DecodedTransferOutput> {
    let mut outputs = Vec::new();
    collect_decoded_transfer_outputs(value, &mut outputs);
    outputs
}

fn collect_decoded_transfer_outputs(value: &Value, outputs: &mut Vec<DecodedTransferOutput>) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if transfer_outputs_key(key) {
                    if let Some(items) = value.as_array() {
                        outputs.extend(items.iter().filter_map(decoded_transfer_output));
                    }
                } else {
                    collect_decoded_transfer_outputs(value, outputs);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_decoded_transfer_outputs(item, outputs);
            }
        }
        _ => {}
    }
}

fn decoded_transfer_output(value: &Value) -> Option<DecodedTransferOutput> {
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
    Some(DecodedTransferOutput {
        recipient,
        amount: first_output_field(object, &["amount", "value", "quantity", "balance"])
            .and_then(|value| value.parse::<u128>().ok()),
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

fn add_optional_amounts(left: Option<u128>, right: Option<u128>) -> Option<u128> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.saturating_add(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn transfer_recipient_sort_key(row: &TransferRecipientSummary) -> (u128, usize, u64) {
    (
        row.received
            .as_deref()
            .and_then(|value| value.parse().ok())
            .unwrap_or_default(),
        row.references,
        row.last_slot.unwrap_or_default(),
    )
}

fn max_slot(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}
