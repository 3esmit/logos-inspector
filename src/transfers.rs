use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::indexer::IndexerBlockReport;

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
    references: usize,
    last_slot: Option<u64>,
    transfers: Vec<RecipientTransferSummary>,
}

pub(crate) fn transfer_recipient_summaries_from_blocks(
    blocks: &[IndexerBlockReport],
) -> Vec<TransferRecipientSummary> {
    let mut account_refs = BTreeMap::new();
    for block in blocks {
        for tx in &block.transactions {
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
                outputs: 0,
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
