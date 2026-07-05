use anyhow::{Context as _, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocialLayer {
    Cryptarchia,
    Lez,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocialEntity {
    Transaction,
    Block,
    Account,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SocialMessage {
    pub topic: String,
    pub cursor: String,
    pub timestamp: String,
    pub payload: SocialPayload,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind")]
pub enum SocialPayload {
    #[serde(rename = "comment")]
    Comment {
        version: u64,
        identity: Value,
        body: String,
        created_at: String,
        conversation_id: String,
    },
    #[serde(rename = "lez_account_idl")]
    LezAccountIdl {
        version: u64,
        identity: Value,
        account_id: String,
        program_id: String,
        idl_name: String,
        idl_json: String,
        created_at: String,
    },
}

#[must_use]
pub fn comment_topic(layer: SocialLayer, entity: SocialEntity, id: &str) -> Option<String> {
    let id = normalized_topic_id(id)?;
    Some(format!(
        "/{}/{}/{}/comments",
        layer.as_topic_segment(),
        entity.as_topic_segment(),
        id
    ))
}

#[must_use]
pub fn lez_account_idl_topic(id: &str) -> Option<String> {
    let id = normalized_topic_id(id)?;
    Some(format!("/lez/account/{id}/idl"))
}

pub fn parse_social_payload(
    raw_json: &str,
    expected_account_id: Option<&str>,
) -> Result<SocialPayload> {
    let value = serde_json::from_str::<Value>(raw_json).context("social payload is not JSON")?;
    parse_social_payload_value(&value, expected_account_id)
}

pub fn social_messages_from_store(
    topic: &str,
    store_value: &Value,
    expected_account_id: Option<&str>,
) -> Vec<SocialMessage> {
    let mut objects = Vec::new();
    collect_store_message_objects(store_value, &mut objects);
    objects
        .into_iter()
        .filter_map(|message| social_message_from_store_object(topic, message, expected_account_id))
        .collect()
}

impl SocialLayer {
    fn as_topic_segment(self) -> &'static str {
        match self {
            Self::Cryptarchia => "cryptarchia",
            Self::Lez => "lez",
        }
    }
}

impl SocialEntity {
    fn as_topic_segment(self) -> &'static str {
        match self {
            Self::Transaction => "transaction",
            Self::Block => "block",
            Self::Account => "account",
        }
    }
}

fn parse_social_payload_value(
    value: &Value,
    expected_account_id: Option<&str>,
) -> Result<SocialPayload> {
    let object = value
        .as_object()
        .context("social payload must be a JSON object")?;
    let kind = required_string(value, "kind")?;
    let version = object
        .get("version")
        .and_then(Value::as_u64)
        .context("social payload version is required")?;
    if version != 1 {
        bail!("social payload version is not supported");
    }
    let identity = object
        .get("identity")
        .filter(|value| value.as_object().is_some())
        .cloned()
        .context("social payload identity is required")?;

    match kind {
        "comment" => Ok(SocialPayload::Comment {
            version,
            identity,
            body: required_string(value, "body")?.to_owned(),
            created_at: required_string(value, "created_at")?.to_owned(),
            conversation_id: required_string(value, "conversation_id")?.to_owned(),
        }),
        "lez_account_idl" => {
            let account_id = required_string(value, "account_id")?.to_owned();
            if let Some(expected) = expected_account_id
                && !ids_match(&account_id, expected)
            {
                bail!("shared IDL account does not match requested account");
            }
            let idl_json = required_string(value, "idl_json")?.to_owned();
            let _idl_value: Value =
                serde_json::from_str(&idl_json).context("shared IDL JSON is not valid JSON")?;
            Ok(SocialPayload::LezAccountIdl {
                version,
                identity,
                account_id,
                program_id: required_string(value, "program_id")?.to_owned(),
                idl_name: required_string(value, "idl_name")?.to_owned(),
                idl_json,
                created_at: required_string(value, "created_at")?.to_owned(),
            })
        }
        _ => bail!("social payload kind is not supported"),
    }
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("social payload {key} is required"))
}

fn normalized_topic_id(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_matches('/');
    if trimmed.is_empty() || trimmed.contains('/') {
        return None;
    }
    Some(trimmed.to_owned())
}

fn ids_match(left: &str, right: &str) -> bool {
    normalized_id_for_match(left) == normalized_id_for_match(right)
}

fn normalized_id_for_match(value: &str) -> String {
    let trimmed = value.trim().trim_start_matches("0x");
    if trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return trimmed.to_ascii_lowercase();
    }
    value.trim().to_owned()
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
    let parsed = parse_social_payload(&text, expected_account_id).ok()?;
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
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn comment_topic_builds_supported_detail_topics() {
        let cases = [
            (
                SocialLayer::Cryptarchia,
                SocialEntity::Transaction,
                "tx-1",
                "/cryptarchia/transaction/tx-1/comments",
            ),
            (
                SocialLayer::Cryptarchia,
                SocialEntity::Block,
                "block-1",
                "/cryptarchia/block/block-1/comments",
            ),
            (
                SocialLayer::Cryptarchia,
                SocialEntity::Account,
                "acct-1",
                "/cryptarchia/account/acct-1/comments",
            ),
            (
                SocialLayer::Lez,
                SocialEntity::Transaction,
                "tx-2",
                "/lez/transaction/tx-2/comments",
            ),
            (
                SocialLayer::Lez,
                SocialEntity::Block,
                "block-2",
                "/lez/block/block-2/comments",
            ),
            (
                SocialLayer::Lez,
                SocialEntity::Account,
                "acct-2",
                "/lez/account/acct-2/comments",
            ),
        ];

        for (layer, entity, id, expected) in cases {
            assert_eq!(comment_topic(layer, entity, id).as_deref(), Some(expected));
        }
        assert_eq!(
            lez_account_idl_topic("acct-2").as_deref(),
            Some("/lez/account/acct-2/idl")
        );
    }

    #[test]
    fn topic_builders_reject_missing_or_slash_ids() {
        assert_eq!(
            comment_topic(SocialLayer::Lez, SocialEntity::Account, ""),
            None
        );
        assert_eq!(
            comment_topic(SocialLayer::Lez, SocialEntity::Account, "account/one"),
            None
        );
        assert_eq!(lez_account_idl_topic("/"), None);
    }

    #[test]
    fn comment_payload_requires_supported_kind_and_body() {
        let payload = json!({
            "kind": "comment",
            "version": 1,
            "identity": { "display_name": "Ada" },
            "body": "hello",
            "created_at": "2026-07-05T00:00:00.000Z",
            "conversation_id": "/lez/account/acct/comments"
        });

        let parsed = parse_social_payload(&payload.to_string(), None);

        assert!(matches!(parsed, Ok(SocialPayload::Comment { .. })));
        assert!(parse_social_payload("{}", None).is_err());
        assert!(
            parse_social_payload(
                &json!({
                    "kind": "comment",
                    "version": 1,
                    "identity": {},
                    "body": "",
                    "created_at": "2026-07-05T00:00:00.000Z",
                    "conversation_id": "topic"
                })
                .to_string(),
                None,
            )
            .is_err()
        );
        assert!(
            parse_social_payload(
                &json!({
                    "kind": "unknown",
                    "version": 1,
                    "identity": {}
                })
                .to_string(),
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn idl_payload_rejects_missing_idl_and_account_mismatch() {
        let payload = json!({
            "kind": "lez_account_idl",
            "version": 1,
            "identity": { "display_name": "Ada" },
            "account_id": "account-1",
            "program_id": "program-1",
            "idl_name": "Sample",
            "idl_json": "{\"name\":\"Sample\",\"accounts\":[]}",
            "created_at": "2026-07-05T00:00:00.000Z"
        });

        let parsed = parse_social_payload(&payload.to_string(), Some("account-1"));

        assert!(matches!(parsed, Ok(SocialPayload::LezAccountIdl { .. })));
        assert!(parse_social_payload(&payload.to_string(), Some("account-2")).is_err());
        assert!(
            parse_social_payload(
                &json!({
                    "kind": "lez_account_idl",
                    "version": 1,
                    "identity": {},
                    "account_id": "account-1",
                    "program_id": "program-1",
                    "idl_name": "Sample",
                    "idl_json": "",
                    "created_at": "2026-07-05T00:00:00.000Z"
                })
                .to_string(),
                Some("account-1"),
            )
            .is_err()
        );
    }

    #[test]
    fn store_messages_decode_base64_payloads_and_ignore_malformed_rows() {
        let topic = "/lez/account/account-1/comments";
        let valid_payload = json!({
            "kind": "comment",
            "version": 1,
            "identity": { "display_name": "Ada" },
            "body": "hello",
            "created_at": "2026-07-05T00:00:00.000Z",
            "conversation_id": topic
        });
        let store = json!({
            "messages": [
                {
                    "contentTopic": topic,
                    "payload": BASE64_STANDARD.encode(valid_payload.to_string()),
                    "cursor": "cursor-1"
                },
                {
                    "contentTopic": topic,
                    "payload": "not-base64"
                },
                {
                    "contentTopic": "/lez/account/other/comments",
                    "payload": BASE64_STANDARD.encode(valid_payload.to_string())
                }
            ],
            "paginationCursor": "next"
        });

        let messages = social_messages_from_store(topic, &store, None);

        assert_eq!(messages.len(), 1);
        let first = messages.first();
        assert!(first.is_some(), "missing decoded message");
        let Some(first) = first else {
            return;
        };
        assert_eq!(first.cursor, "cursor-1");
        assert!(matches!(first.payload, SocialPayload::Comment { .. }));
    }
}
