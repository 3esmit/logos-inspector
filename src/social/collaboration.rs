use serde_json::Value;

use super::{
    comments::{
        SocialCommentPage, SocialCommentRow, social_comment_page_from_store,
        social_comment_row_from_event,
    },
    delivery_store::{SocialMessage, social_messages_from_store},
    topic::{comment_topic_from_parts, lez_account_idl_topic, social_topic_is_valid},
};

#[derive(Debug, Clone, Copy)]
pub struct SocialCommentQuery<'a> {
    pub topic: &'a str,
    pub expected_account_id: Option<&'a str>,
}

#[must_use]
pub fn build_comment_topic(layer: &str, entity: &str, id: &str) -> Option<String> {
    comment_topic_from_parts(layer, entity, id)
}

#[must_use]
pub fn build_lez_account_idl_topic(account_id: &str) -> Option<String> {
    lez_account_idl_topic(account_id)
}

#[must_use]
pub fn validate_topic(topic: &str) -> bool {
    social_topic_is_valid(topic)
}

#[must_use]
pub fn decode_social_messages(
    query: SocialCommentQuery<'_>,
    store_value: &Value,
) -> Vec<SocialMessage> {
    social_messages_from_store(query.topic, store_value, query.expected_account_id)
}

#[must_use]
pub fn decode_comment_page(
    query: SocialCommentQuery<'_>,
    store_value: &Value,
) -> SocialCommentPage {
    social_comment_page_from_store(query.topic, store_value, query.expected_account_id)
}

#[must_use]
pub fn project_comment_event(event: &Value) -> Option<SocialCommentRow> {
    social_comment_row_from_event(event)
}

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
    use serde_json::json;

    use super::*;

    #[test]
    fn builds_and_validates_collaboration_topics() {
        let topic = build_comment_topic("l2", "account", "account-1");

        assert_eq!(topic.as_deref(), Some("/lez/account/account-1/comments"));
        assert_eq!(
            build_lez_account_idl_topic("account-1").as_deref(),
            Some("/lez/account/account-1/idl")
        );
        assert!(topic.as_deref().is_some_and(validate_topic));
        assert!(!validate_topic("lez/account/account-1/comments"));
    }

    #[test]
    fn decodes_comment_page_from_delivery_store_response() {
        let topic = "/lez/account/account-1/comments";
        let payload = json!({
            "kind": "comment",
            "version": 1,
            "identity": { "display_name": "Ada" },
            "body": "hello",
            "created_at": "2026-07-05T00:00:00.000Z",
            "conversation_id": topic
        });
        let store = json!({
            "value": {
                "messages": [{
                    "contentTopic": topic,
                    "payload": BASE64_STANDARD.encode(payload.to_string()),
                    "cursor": "cursor-1"
                }],
                "pagination": {
                    "next_cursor": "cursor-2"
                }
            }
        });

        let page = decode_comment_page(
            SocialCommentQuery {
                topic,
                expected_account_id: None,
            },
            &store,
        );

        assert_eq!(page.cursor, "cursor-2");
        assert_eq!(page.rows.len(), 1);
        assert_eq!(
            page.rows.first().map(|row| row.key.as_str()),
            Some("cursor-1|2026-07-05T00:00:00.000Z|Ada|hello")
        );
    }

    #[test]
    fn projects_incoming_comment_event() {
        let topic = "/lez/account/account-1/comments";
        let event = json!({
            "topic": topic,
            "messageHash": "hash-1",
            "payload": {
                "kind": "comment",
                "version": 1,
                "identity": { "display_name": "Peer" },
                "body": "hello",
                "created_at": "2026-07-07T00:00:00Z",
                "conversation_id": topic
            }
        });

        let row = project_comment_event(&event);

        assert!(row.is_some(), "missing projected event row");
        let Some(row) = row else {
            return;
        };
        assert_eq!(row.key, "event|hash-1|2026-07-07T00:00:00Z");
        assert_eq!(row.topic, topic);
    }
}
