use serde::Serialize;
use serde_json::Value;

use super::delivery_store::{
    SocialMessage, last_social_message_cursor, social_messages_from_store, social_store_cursor,
};
use super::payload::{SocialPayload, parse_social_payload_value};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SocialCommentPage {
    pub rows: Vec<SocialCommentRow>,
    pub cursor: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SocialCommentRow {
    pub key: String,
    pub cursor: String,
    pub topic: String,
    pub identity: Value,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub body: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "conversationId")]
    pub conversation_id: String,
}

#[must_use]
pub fn social_comment_page_from_store(
    topic: &str,
    store_value: &Value,
    expected_account_id: Option<&str>,
) -> SocialCommentPage {
    let messages = social_messages_from_store(topic, store_value, expected_account_id);
    SocialCommentPage {
        rows: social_comment_rows_from_messages(&messages),
        cursor: social_store_cursor(store_value)
            .or_else(|| last_social_message_cursor(&messages))
            .unwrap_or_default(),
    }
}

#[must_use]
pub fn social_comment_rows_from_messages(messages: &[SocialMessage]) -> Vec<SocialCommentRow> {
    messages
        .iter()
        .filter_map(social_comment_row_from_message)
        .collect()
}

#[must_use]
pub fn social_comment_row_from_event(event: &Value) -> Option<SocialCommentRow> {
    let object = event.as_object()?;
    let payload = object.get("payload")?;
    let event_topic = first_string(event, &["topic", "contentTopic", "content_topic"]);
    let parsed = parse_social_payload_value(payload, None, event_topic).ok()?;
    let SocialPayload::Comment {
        identity,
        body,
        created_at,
        conversation_id,
        ..
    } = parsed
    else {
        return None;
    };
    let topic = event_topic.unwrap_or(conversation_id.as_str()).to_owned();
    if topic.is_empty() {
        return None;
    }
    let message_hash =
        first_string(event, &["messageHash", "message_hash", "hash"]).unwrap_or_default();
    let display_name = social_identity_display_name(&identity);
    Some(SocialCommentRow {
        key: ["event", message_hash, created_at.as_str()].join("|"),
        cursor: String::new(),
        topic: topic.clone(),
        identity,
        display_name,
        body,
        created_at,
        conversation_id: if conversation_id.is_empty() {
            topic.to_owned()
        } else {
            conversation_id
        },
    })
}

fn social_comment_row_from_message(message: &SocialMessage) -> Option<SocialCommentRow> {
    let SocialPayload::Comment {
        identity,
        body,
        created_at,
        conversation_id,
        ..
    } = &message.payload
    else {
        return None;
    };
    Some(SocialCommentRow {
        key: social_message_row_key(message),
        cursor: message.cursor.clone(),
        topic: if message.topic.is_empty() {
            conversation_id.clone()
        } else {
            message.topic.clone()
        },
        identity: identity.clone(),
        display_name: social_identity_display_name(identity),
        body: body.clone(),
        created_at: created_at.clone(),
        conversation_id: conversation_id.clone(),
    })
}

fn first_string<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    let object = value.as_object()?;
    keys.iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn social_message_row_key(message: &SocialMessage) -> String {
    match &message.payload {
        SocialPayload::Comment {
            identity,
            body,
            created_at,
            ..
        } => {
            let display_name = social_identity_display_name(identity);
            [
                message.cursor.as_str(),
                created_at.as_str(),
                display_name.as_str(),
                body.as_str(),
            ]
            .join("|")
        }
        SocialPayload::LezAccountIdl {
            identity,
            idl_name,
            created_at,
            ..
        } => {
            let display_name = social_identity_display_name(identity);
            [
                message.cursor.as_str(),
                created_at.as_str(),
                display_name.as_str(),
                idl_name.as_str(),
            ]
            .join("|")
        }
    }
}

fn social_identity_display_name(identity: &Value) -> String {
    let Some(object) = identity.as_object() else {
        return "Pseudonym".to_owned();
    };
    ["display_name", "displayName", "name", "local_id", "localId"]
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Pseudonym")
        .to_owned()
}
