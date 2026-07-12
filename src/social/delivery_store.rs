use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::Serialize;
use serde_json::Value;

use super::{
    payload::{SocialPayload, parse_social_payload_for_topic},
    social_topic_is_valid,
};

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
    let payload = message_payload(message)?;
    let bytes = BASE64_STANDARD.decode(payload).ok()?;
    let text = String::from_utf8(bytes).ok()?;
    let parsed = parse_social_payload_for_topic(&text, expected_account_id, topic).ok()?;
    Some(SocialMessage {
        topic: message_topic.unwrap_or(topic).to_owned(),
        cursor: first_string(message, &["cursor", "messageHash", "message_hash", "hash"])
            .unwrap_or_default()
            .to_owned(),
        timestamp: first_string(
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
            if message_payload(value).is_some() {
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
    first_string(
        message,
        &["contentTopic", "content_topic", "content-topic", "topic"],
    )
}

fn message_payload(message: &Value) -> Option<&str> {
    first_string(message, &["payload", "data"])
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
