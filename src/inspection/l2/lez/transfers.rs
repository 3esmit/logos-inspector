use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::{
    indexer::IndexerBlockReport,
    transaction_facts::{AccountTransactionSummary, summarize_transfer_outputs},
};

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
            for output in transaction_transfer_outputs(tx) {
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

fn transaction_transfer_outputs(tx: &AccountTransactionSummary) -> Vec<DecodedTransferOutput> {
    let outputs = if tx.transfer_outputs.is_empty() {
        summarize_transfer_outputs(&tx.raw)
    } else {
        tx.transfer_outputs.clone()
    };
    outputs
        .into_iter()
        .map(|output| DecodedTransferOutput {
            recipient: output.recipient,
            amount: output
                .amount
                .as_deref()
                .and_then(|amount| amount.parse::<u128>().ok()),
        })
        .collect()
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
