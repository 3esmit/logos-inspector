use serde::Serialize;
use serde_json::Value;

use crate::blockchain::blockchain_blocks;

#[derive(Debug, Clone, Serialize)]
pub struct ChannelScanReport {
    pub endpoint: String,
    pub slot_from: u64,
    pub slot_to: u64,
    pub block_count: usize,
    pub matches: Vec<ChannelOperationMatch>,
    pub raw_blocks: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChannelOperationMatch {
    pub path: String,
    pub operation_type: Option<String>,
    pub channel_id: Option<String>,
    pub parent: Option<String>,
    pub signer: Option<String>,
    pub value: Value,
}

pub async fn channel_scan(
    endpoint: &str,
    slot_from: u64,
    slot_to: u64,
) -> anyhow::Result<ChannelScanReport> {
    let blocks = blockchain_blocks(endpoint, slot_from, slot_to).await?;
    let block_count = count_blocks(&blocks);
    let mut matches = Vec::new();
    scan_value(&blocks, "$", &mut matches);
    Ok(ChannelScanReport {
        endpoint: endpoint.to_owned(),
        slot_from,
        slot_to,
        block_count,
        matches,
        raw_blocks: blocks,
    })
}

pub fn extract_channel_operations(value: &Value) -> Vec<ChannelOperationMatch> {
    let mut matches = Vec::new();
    scan_value(value, "$", &mut matches);
    matches
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

fn scan_value(value: &Value, path: &str, matches: &mut Vec<ChannelOperationMatch>) {
    match value {
        Value::Object(object) => {
            let operation_key = channel_operation_key(object);
            if is_channel_object(value) {
                let payload = operation_key.and_then(|key| object.get(key));
                matches.push(ChannelOperationMatch {
                    path: path.to_owned(),
                    operation_type: operation_type(value),
                    channel_id: first_stringish(value, &["channel_id", "channelId", "channel"])
                        .or_else(|| {
                            payload.and_then(|payload| {
                                first_stringish(payload, &["channel_id", "channelId", "channel"])
                            })
                        }),
                    parent: first_stringish(value, &["parent", "parent_id", "parentId"]).or_else(
                        || {
                            payload.and_then(|payload| {
                                first_stringish(payload, &["parent", "parent_id", "parentId"])
                            })
                        },
                    ),
                    signer: first_stringish(value, &["signer", "sequencer", "key"]).or_else(|| {
                        payload.and_then(|payload| {
                            first_stringish(payload, &["signer", "sequencer", "key"])
                        })
                    }),
                    value: value.clone(),
                });
            }
            for (key, child) in object {
                if operation_key.is_some_and(|operation_key| operation_key == key) {
                    continue;
                }
                scan_value(child, &format!("{path}.{key}"), matches);
            }
        }
        Value::Array(array) => {
            for (index, child) in array.iter().enumerate() {
                scan_value(child, &format!("{path}[{index}]"), matches);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
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
                "ops": [{
                    "ChannelInscribe": {
                        "channel_id": "abc"
                    },
                    "channel_id": "abc",
                    "parent": "parent",
                    "signer": "signer"
                }]
            }]
        });

        let matches = extract_channel_operations(&value);

        assert_eq!(matches.len(), 1);
        let first_channel_id = matches
            .first()
            .and_then(|matched| matched.channel_id.as_deref());
        assert_eq!(first_channel_id, Some("abc"));
    }
}
