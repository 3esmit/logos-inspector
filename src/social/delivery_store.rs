use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::Serialize;
use serde_json::Value;

use super::{
    payload::{SocialPayload, parse_social_payload_for_topic},
    social_topic_is_valid,
};

const MAX_SOCIAL_STORE_PAYLOAD_BYTES: usize = 256 * 1024;
const MAX_SOCIAL_STORE_BASE64_BYTES: usize = MAX_SOCIAL_STORE_PAYLOAD_BYTES.div_ceil(3) * 4;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SocialMessage {
    pub topic: String,
    pub cursor: String,
    pub timestamp: String,
    pub payload: SocialPayload,
}

pub fn social_messages_from_store(
    topic: &str,
    store_value: &Value,
    expected_account_id: Option<&str>,
) -> Vec<SocialMessage> {
    if !social_topic_is_valid(topic) {
        return Vec::new();
    }
    let mut objects = Vec::new();
    collect_store_message_objects(store_value, &mut objects);
    objects
        .into_iter()
        .filter_map(|message| social_message_from_store_object(topic, message, expected_account_id))
        .collect()
}

#[must_use]
pub fn social_store_cursor(value: &Value) -> Option<String> {
    first_store_cursor(value, 0).map(ToOwned::to_owned)
}

#[must_use]
pub fn last_social_message_cursor(messages: &[SocialMessage]) -> Option<String> {
    messages
        .iter()
        .rev()
        .find_map(|message| non_empty(message.cursor.as_str()).map(ToOwned::to_owned))
}

fn social_message_from_store_object(
    topic: &str,
    message: &Value,
    expected_account_id: Option<&str>,
) -> Option<SocialMessage> {
    let message_topic = message_content_topic(message);
    if let Some(content_topic) = message_topic
        && content_topic != topic
    {
        return None;
    }
    let bytes = message_payload_bytes(message)?;
    let text = String::from_utf8(bytes).ok()?;
    let parsed = parse_social_payload_for_topic(&text, expected_account_id, topic).ok()?;
    Some(SocialMessage {
        topic: message_topic.unwrap_or(topic).to_owned(),
        cursor: first_message_string(message, &["cursor", "messageHash", "message_hash", "hash"])
            .unwrap_or_default()
            .to_owned(),
        timestamp: first_message_string(
            message,
            &["timestamp", "timestampNs", "createdAt", "created_at"],
        )
        .unwrap_or_default()
        .to_owned(),
        payload: parsed,
    })
}

fn collect_store_message_objects<'a>(value: &'a Value, out: &mut Vec<&'a Value>) {
    match value {
        Value::Object(object) => {
            if message_payload_value(value).is_some() {
                out.push(value);
                return;
            }
            for child in object.values() {
                collect_store_message_objects(child, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_store_message_objects(item, out);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn message_content_topic(message: &Value) -> Option<&str> {
    first_message_string(
        message,
        &["contentTopic", "content_topic", "content-topic", "topic"],
    )
}

fn message_payload_bytes(message: &Value) -> Option<Vec<u8>> {
    match message_payload_value(message)? {
        Value::String(encoded) if encoded.len() <= MAX_SOCIAL_STORE_BASE64_BYTES => BASE64_STANDARD
            .decode(encoded)
            .ok()
            .filter(|bytes| bytes.len() <= MAX_SOCIAL_STORE_PAYLOAD_BYTES),
        Value::Array(bytes) if bytes.len() <= MAX_SOCIAL_STORE_PAYLOAD_BYTES => bytes
            .iter()
            .map(|value| value.as_u64().and_then(|byte| u8::try_from(byte).ok()))
            .collect(),
        _ => None,
    }
}

fn message_payload_value(message: &Value) -> Option<&Value> {
    first_value(message, &["payload", "data"]).or_else(|| {
        message
            .get("message")
            .and_then(|nested| first_value(nested, &["payload", "data"]))
    })
}

fn first_message_string<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    first_string(value, keys).or_else(|| {
        value
            .get("message")
            .and_then(|nested| first_string(nested, keys))
    })
}

fn first_value<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    let object = value.as_object()?;
    keys.iter().find_map(|key| object.get(*key))
}

fn first_string<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    let object = value.as_object()?;
    keys.iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
        .and_then(non_empty)
}

fn first_store_cursor(value: &Value, depth: usize) -> Option<&str> {
    if depth > 5 {
        return None;
    }
    match value {
        Value::Array(items) => items
            .iter()
            .find_map(|item| first_store_cursor(item, depth + 1)),
        Value::Object(object) => first_string(
            value,
            &[
                "paginationCursor",
                "pagination_cursor",
                "nextCursor",
                "next_cursor",
            ],
        )
        .or_else(|| {
            ["value", "result", "page", "pagination"]
                .iter()
                .filter_map(|key| object.get(*key))
                .find_map(|child| first_store_cursor(child, depth + 1))
        }),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => None,
    }
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn delivery_store_byte_arrays_are_bounded_and_require_bytes() {
        assert_eq!(
            message_payload_bytes(&json!({ "payload": [0, 127, 255] })),
            Some(vec![0, 127, 255])
        );
        for malformed in [
            json!({ "payload": [-1] }),
            json!({ "payload": [256] }),
            json!({ "payload": [1.5] }),
            json!({ "payload": ["1"] }),
        ] {
            assert!(
                message_payload_bytes(&malformed).is_none(),
                "malformed byte array was accepted: {malformed}"
            );
        }

        let oversized_array = json!({ "payload": vec![0_u8; MAX_SOCIAL_STORE_PAYLOAD_BYTES + 1] });
        assert!(message_payload_bytes(&oversized_array).is_none());

        let oversized_base64 = json!({
            "payload": "A".repeat(MAX_SOCIAL_STORE_BASE64_BYTES + 1)
        });
        assert!(message_payload_bytes(&oversized_base64).is_none());
    }
}
