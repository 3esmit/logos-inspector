use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::blockchain::blockchain_blocks;

#[derive(Debug, Clone, Serialize)]
pub struct ChannelScanReport {
    pub endpoint: String,
    pub slot_from: u64,
    pub slot_to: u64,
    pub block_count: usize,
    pub summaries: Vec<ChannelSummary>,
    pub matches: Vec<ChannelOperationMatch>,
    pub raw_blocks: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChannelSummary {
    pub channel: String,
    pub label: Option<String>,
    pub first_slot: Option<u64>,
    pub first_tx_hash: Option<String>,
    pub first_block_hash: Option<String>,
    pub last_slot: Option<u64>,
    pub last_tx_hash: Option<String>,
    pub last_block_hash: Option<String>,
    pub tip: Option<String>,
    pub balance: Option<String>,
    pub withdraw_threshold: Option<String>,
    pub keys: Option<usize>,
    pub key_values: Vec<String>,
    pub operations: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChannelOperationMatch {
    pub path: String,
    pub operation_type: Option<String>,
    pub channel_id: Option<String>,
    pub slot: Option<u64>,
    pub block_hash: Option<String>,
    pub tx_hash: Option<String>,
    pub label: Option<String>,
    pub balance: Option<String>,
    pub tip: Option<String>,
    pub withdraw_threshold: Option<String>,
    pub key_count: Option<usize>,
    pub key_values: Vec<String>,
    pub parent: Option<String>,
    pub signer: Option<String>,
    pub value: Value,
}

#[derive(Debug, Clone, Default)]
struct ChannelScanContext {
    slot: Option<u64>,
    block_hash: Option<String>,
    tx_hash: Option<String>,
}

pub async fn channel_scan(
    endpoint: &str,
    slot_from: u64,
    slot_to: u64,
) -> anyhow::Result<ChannelScanReport> {
    let blocks = blockchain_blocks(endpoint, slot_from, slot_to).await?;
    let block_count = count_blocks(&blocks);
    let mut matches = Vec::new();
    scan_value(&blocks, "$", &ChannelScanContext::default(), &mut matches);
    let summaries = summarize_channel_operations(&matches);
    Ok(ChannelScanReport {
        endpoint: endpoint.to_owned(),
        slot_from,
        slot_to,
        block_count,
        summaries,
        matches,
        raw_blocks: blocks,
    })
}

pub fn extract_channel_operations(value: &Value) -> Vec<ChannelOperationMatch> {
    let mut matches = Vec::new();
    scan_value(value, "$", &ChannelScanContext::default(), &mut matches);
    matches
}

#[must_use]
pub fn summarize_channel_operations(matches: &[ChannelOperationMatch]) -> Vec<ChannelSummary> {
    let mut channels = BTreeMap::<String, ChannelSummary>::new();
    for matched in matches {
        let Some(channel_id) = matched
            .channel_id
            .as_ref()
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let entry = channels
            .entry(channel_id.clone())
            .or_insert_with(|| ChannelSummary {
                channel: channel_id.clone(),
                label: None,
                first_slot: None,
                first_tx_hash: None,
                first_block_hash: None,
                last_slot: None,
                last_tx_hash: None,
                last_block_hash: None,
                tip: None,
                balance: None,
                withdraw_threshold: None,
                keys: None,
                key_values: Vec::new(),
                operations: 0,
            });
        entry.operations += 1;
        if should_replace_first(entry.first_slot, matched.slot) {
            entry.first_slot = matched.slot;
            entry.first_tx_hash = matched.tx_hash.clone();
            entry.first_block_hash = matched.block_hash.clone();
        }
        if should_replace_last(entry.last_slot, matched.slot) {
            entry.last_slot = matched.slot;
            entry.last_tx_hash = matched.tx_hash.clone();
            entry.last_block_hash = matched.block_hash.clone();
            if matched.tip.is_some() || matched.parent.is_some() {
                entry.tip = matched.tip.clone().or_else(|| matched.parent.clone());
            }
            if matched.balance.is_some() {
                entry.balance = matched.balance.clone();
            }
            if matched.withdraw_threshold.is_some() {
                entry.withdraw_threshold = matched.withdraw_threshold.clone();
            }
            if !matched.key_values.is_empty() || matched.signer.is_some() {
                entry.key_values = if matched.key_values.is_empty() {
                    matched.signer.iter().cloned().collect()
                } else {
                    matched.key_values.clone()
                };
            }
        }
        entry.last_slot = max_slot(entry.last_slot, matched.slot);
        if entry.label.is_none() {
            entry.label = matched.label.clone();
        }
        if entry.balance.is_none() {
            entry.balance = matched.balance.clone();
        }
        entry.keys = max_usize(
            entry.keys,
            matched.key_count.or(matched.signer.as_ref().map(|_| 1)),
        );
    }
    let mut summaries = channels.into_values().collect::<Vec<_>>();
    summaries.sort_by(|left, right| {
        right
            .last_slot
            .unwrap_or_default()
            .cmp(&left.last_slot.unwrap_or_default())
            .then_with(|| left.channel.cmp(&right.channel))
    });
    summaries
}

fn count_blocks(value: &Value) -> usize {
    if let Some(array) = value.as_array() {
        return array.len();
    }
    value
        .get("blocks")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

fn scan_value(
    value: &Value,
    path: &str,
    context: &ChannelScanContext,
    matches: &mut Vec<ChannelOperationMatch>,
) {
    match value {
        Value::Object(object) => {
            let slot = slot_from_object(value).or(context.slot);
            let block_hash = block_hash_from_object(value).or_else(|| context.block_hash.clone());
            let tx_hash = tx_hash_from_object(value).or_else(|| context.tx_hash.clone());
            let next_context = ChannelScanContext {
                slot,
                block_hash: block_hash.clone(),
                tx_hash: tx_hash.clone(),
            };
            let operation_key = channel_operation_key(object);
            if is_channel_object(value) {
                let payload = operation_key.and_then(|key| object.get(key));
                let signer =
                    first_stringish(value, &["signer", "sequencer", "key"]).or_else(|| {
                        payload.and_then(|payload| {
                            first_stringish(payload, &["signer", "sequencer", "key"])
                        })
                    });
                matches.push(ChannelOperationMatch {
                    path: path.to_owned(),
                    operation_type: operation_type(value),
                    channel_id: first_stringish(value, &["channel_id", "channelId", "channel"])
                        .or_else(|| {
                            payload.and_then(|payload| {
                                first_stringish(payload, &["channel_id", "channelId", "channel"])
                            })
                        }),
                    slot,
                    block_hash,
                    tx_hash,
                    label: first_stringish(value, &["label", "name", "channel_label"]).or_else(
                        || {
                            payload.and_then(|payload| {
                                first_stringish(payload, &["label", "name", "channel_label"])
                            })
                        },
                    ),
                    balance: first_stringish(value, &["balance", "channel_balance"]).or_else(
                        || {
                            payload.and_then(|payload| {
                                first_stringish(payload, &["balance", "channel_balance"])
                            })
                        },
                    ),
                    tip: first_stringish(value, &["tip", "channel_tip"]).or_else(|| {
                        payload
                            .and_then(|payload| first_stringish(payload, &["tip", "channel_tip"]))
                    }),
                    withdraw_threshold: first_stringish(
                        value,
                        &[
                            "withdraw_threshold",
                            "withdrawThreshold",
                            "withdrawal_threshold",
                            "threshold",
                        ],
                    )
                    .or_else(|| {
                        payload.and_then(|payload| {
                            first_stringish(
                                payload,
                                &[
                                    "withdraw_threshold",
                                    "withdrawThreshold",
                                    "withdrawal_threshold",
                                    "threshold",
                                ],
                            )
                        })
                    }),
                    key_count: key_count(value).or_else(|| payload.and_then(key_count)),
                    key_values: key_values(value)
                        .or_else(|| payload.and_then(key_values))
                        .unwrap_or_default(),
                    parent: first_stringish(value, &["parent", "parent_id", "parentId"]).or_else(
                        || {
                            payload.and_then(|payload| {
                                first_stringish(payload, &["parent", "parent_id", "parentId"])
                            })
                        },
                    ),
                    signer,
                    value: value.clone(),
                });
            }
            for (key, child) in object {
                if operation_key.is_some_and(|operation_key| operation_key == key) {
                    continue;
                }
                scan_value(child, &format!("{path}.{key}"), &next_context, matches);
            }
        }
        Value::Array(array) => {
            for (index, child) in array.iter().enumerate() {
                scan_value(child, &format!("{path}[{index}]"), context, matches);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn slot_from_object(value: &Value) -> Option<u64> {
    first_u64(value, &["slot", "block_id", "height"]).or_else(|| {
        value
            .get("header")
            .and_then(|header| first_u64(header, &["slot", "block_id", "height"]))
    })
}

fn block_hash_from_object(value: &Value) -> Option<String> {
    value
        .get("header")
        .and_then(|header| first_stringish(header, &["id", "hash", "block_hash", "header_hash"]))
}

fn tx_hash_from_object(value: &Value) -> Option<String> {
    value
        .get("mantle_tx")
        .and_then(|mantle| first_stringish(mantle, &["hash", "tx_hash", "transaction_hash"]))
        .or_else(|| {
            value.get("Public").and_then(|public| {
                first_stringish(public, &["hash", "tx_hash", "transaction_hash"])
            })
        })
        .or_else(|| {
            value.get("message").and_then(|message| {
                first_stringish(message, &["hash", "tx_hash", "transaction_hash"])
            })
        })
}

fn should_replace_first(current: Option<u64>, candidate: Option<u64>) -> bool {
    match (current, candidate) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(current), Some(candidate)) => candidate <= current,
    }
}

fn should_replace_last(current: Option<u64>, candidate: Option<u64>) -> bool {
    match (current, candidate) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(current), Some(candidate)) => candidate >= current,
    }
}

fn is_channel_object(value: &Value) -> bool {
    operation_type(value).is_some()
        || first_stringish(value, &["channel_id", "channelId"]).is_some()
        || value
            .as_object()
            .is_some_and(|object| object.keys().any(|key| key.contains("channel")))
}

fn operation_type(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    if let Some(key) = channel_operation_key(object) {
        return Some(key.to_owned());
    }
    object
        .get("type")
        .or_else(|| object.get("kind"))
        .or_else(|| object.get("op"))
        .and_then(Value::as_str)
        .filter(|value| value.to_ascii_lowercase().contains("channel"))
        .map(ToOwned::to_owned)
}

fn channel_operation_key(object: &serde_json::Map<String, Value>) -> Option<&str> {
    [
        "ChannelInscribe",
        "ChannelSetKeys",
        "channel_inscribe",
        "channel_set_keys",
        "inscription",
    ]
    .into_iter()
    .find(|key| object.contains_key(*key))
}

fn first_stringish(value: &Value, keys: &[&str]) -> Option<String> {
    let object = value.as_object()?;
    for key in keys {
        if let Some(value) = object.get(*key) {
            return Some(value_to_label(value));
        }
    }
    None
}

fn first_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    let object = value.as_object()?;
    keys.iter().find_map(|key| {
        object.get(*key).and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
        })
    })
}

fn key_count(value: &Value) -> Option<usize> {
    let object = value.as_object()?;
    for key in ["keys", "public_keys", "signers", "signing_keys"] {
        if let Some(value) = object.get(key) {
            return value
                .as_array()
                .map(Vec::len)
                .or_else(|| value.as_str().map(|value| value.split(',').count()));
        }
    }
    first_stringish(value, &["key", "signer", "public_key"]).map(|_| 1)
}

fn key_values(value: &Value) -> Option<Vec<String>> {
    let object = value.as_object()?;
    for key in ["keys", "public_keys", "signers", "signing_keys"] {
        if let Some(value) = object.get(key) {
            let values = match value {
                Value::Array(items) => items.iter().map(value_to_label).collect::<Vec<_>>(),
                Value::String(value) => value
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>(),
                _ => vec![value_to_label(value)],
            };
            if !values.is_empty() {
                return Some(values);
            }
        }
    }
    first_stringish(value, &["key", "signer", "public_key"]).map(|value| vec![value])
}

fn max_slot(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn max_usize(left: Option<usize>, right: Option<usize>) -> Option<usize> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn value_to_label(value: &Value) -> String {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_nested_channel_operations() {
        let value = json!({
            "blocks": [{
                "header": { "slot": 44, "id": "block-a" },
                "transactions": [{
                    "mantle_tx": {
                        "hash": "tx-a",
                        "ops": [{
                            "ChannelInscribe": {
                                "channel_id": "abc"
                            },
                            "channel_id": "abc",
                            "parent": "parent",
                            "signer": "signer"
                        }]
                    }
                }]
            }]
        });

        let matches = extract_channel_operations(&value);

        assert_eq!(matches.len(), 1);
        let first_channel_id = matches
            .first()
            .and_then(|matched| matched.channel_id.as_deref());
        assert_eq!(first_channel_id, Some("abc"));
        assert_eq!(matches.first().and_then(|matched| matched.slot), Some(44));
        assert_eq!(
            matches
                .first()
                .and_then(|matched| matched.tx_hash.as_deref()),
            Some("tx-a")
        );
        assert_eq!(
            matches
                .first()
                .and_then(|matched| matched.block_hash.as_deref()),
            Some("block-a")
        );
    }

    #[test]
    fn summarizes_channels_by_last_activity() {
        let value = json!({
            "blocks": [
                {
                    "header": { "slot": 10, "id": "block-old" },
                    "transactions": [{
                        "mantle_tx": {
                            "hash": "tx-old",
                            "ops": [{
                                "payload": {
                                    "channel_id": "older",
                                    "signer": "key-a",
                                    "balance": "7"
                                }
                            }]
                        }
                    }]
                },
                {
                    "header": { "slot": 20, "id": "block-new" },
                    "transactions": [{
                        "mantle_tx": {
                            "hash": "tx-new",
                            "ops": [{
                                "payload": {
                                    "channel_id": "newer",
                                    "keys": ["key-a", "key-b"],
                                    "label": "label-a",
                                    "parent": "tip-a",
                                    "withdraw_threshold": "1"
                                }
                            }]
                        }
                    }]
                }
            ]
        });

        let matches = extract_channel_operations(&value);
        let summaries = summarize_channel_operations(&matches);

        assert_eq!(summaries.len(), 2);
        assert_eq!(
            summaries.first().map(|summary| summary.channel.as_str()),
            Some("newer")
        );
        assert_eq!(
            summaries.first().and_then(|summary| summary.last_slot),
            Some(20)
        );
        assert_eq!(
            summaries
                .first()
                .and_then(|summary| summary.first_tx_hash.as_deref()),
            Some("tx-new")
        );
        assert_eq!(
            summaries
                .first()
                .and_then(|summary| summary.last_tx_hash.as_deref()),
            Some("tx-new")
        );
        assert_eq!(
            summaries
                .first()
                .and_then(|summary| summary.last_block_hash.as_deref()),
            Some("block-new")
        );
        assert_eq!(
            summaries.first().and_then(|summary| summary.tip.as_deref()),
            Some("tip-a")
        );
        assert_eq!(
            summaries
                .first()
                .and_then(|summary| summary.withdraw_threshold.as_deref()),
            Some("1")
        );
        assert_eq!(summaries.first().and_then(|summary| summary.keys), Some(2));
        assert_eq!(
            summaries
                .first()
                .map(|summary| summary.key_values.as_slice()),
            Some(["key-a".to_owned(), "key-b".to_owned()].as_slice())
        );
        assert_eq!(
            summaries
                .get(1)
                .and_then(|summary| summary.balance.as_deref()),
            Some("7")
        );
    }
}
