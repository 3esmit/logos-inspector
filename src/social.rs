mod comments;
mod delivery_store;
mod payload;
mod shared_idl;
mod topic;

pub use comments::{
    SocialCommentPage, SocialCommentRow, social_comment_page_from_store,
    social_comment_row_from_event, social_comment_rows_from_messages,
};
pub use delivery_store::{
    SocialMessage, last_social_message_cursor, social_messages_from_store, social_store_cursor,
};
pub use payload::{SocialPayload, parse_social_payload};
pub use shared_idl::{AcceptedSharedIdlEntry, accepted_shared_idl_entries_from_store};
pub use topic::{
    SocialEntity, SocialLayer, comment_topic, lez_account_idl_topic, social_topic_is_valid,
};

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
    use serde_json::json;

    use super::*;

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
        assert_eq!(social_store_cursor(&store).as_deref(), Some("next"));
    }

    #[test]
    fn store_comment_page_projects_rows_and_cursor() {
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
            "value": {
                "messages": [{
                    "contentTopic": topic,
                    "payload": BASE64_STANDARD.encode(valid_payload.to_string()),
                    "cursor": "cursor-1"
                }],
                "pagination": {
                    "next_cursor": "cursor-2"
                }
            }
        });

        let page = social_comment_page_from_store(topic, &store, None);

        assert_eq!(page.cursor, "cursor-2");
        assert_eq!(page.rows.len(), 1);
        let first = page.rows.first();
        assert!(first.is_some(), "missing projected comment row");
        let Some(first) = first else {
            return;
        };
        assert_eq!(first.key, "cursor-1|2026-07-05T00:00:00.000Z|Ada|hello");
        assert_eq!(first.display_name, "Ada");
        assert_eq!(first.body, "hello");
    }

    #[test]
    fn incoming_comment_event_projects_comment_row() {
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

        let row = social_comment_row_from_event(&event);

        assert!(row.is_some(), "missing projected event row");
        let Some(row) = row else {
            return;
        };
        assert_eq!(row.key, "event|hash-1|2026-07-07T00:00:00Z");
        assert_eq!(row.topic, topic);
        assert_eq!(row.display_name, "Peer");
        assert_eq!(row.body, "hello");
    }

    #[test]
    fn social_topic_validation_accepts_four_segment_topics_only() {
        assert!(social_topic_is_valid("/lez/account/account-1/comments"));
        assert!(!social_topic_is_valid("/lez/account/account-1"));
        assert!(!social_topic_is_valid("lez/account/account-1/comments"));
    }

    #[test]
    fn shared_idl_acceptance_keeps_only_full_account_decodes() {
        let topic = "/lez/account/account-1/idl";
        let program_id = "1111111111111111111111111111111111111111111111111111111111111111";
        let other_program_id = "2222222222222222222222222222222222222222222222222222222222222222";
        let idl = r#"{
            "name": "test_program",
            "accounts": [
                {
                    "name": "ShortAccount",
                    "type": {
                        "kind": "struct",
                        "fields": [
                            { "name": "tag", "type": "u8" }
                        ]
                    }
                }
            ]
        }"#;
        let payload = json!({
            "kind": "lez_account_idl",
            "version": 1,
            "identity": { "display_name": "Ada" },
            "account_id": "account-1",
            "program_id": program_id,
            "idl_name": "SharedProgram",
            "idl_json": idl,
            "created_at": "2026-07-05T00:00:00.000Z"
        });
        let store = json!({
            "messages": [{
                "contentTopic": topic,
                "payload": BASE64_STANDARD.encode(payload.to_string()),
                "cursor": "cursor-1"
            }]
        });

        let accepted = accepted_shared_idl_entries_from_store(
            topic,
            &store,
            "account-1",
            "01",
            Some(program_id),
        );

        assert_eq!(accepted.len(), 1);
        let first = accepted.first();
        assert!(first.is_some(), "missing accepted shared IDL");
        let Some(first) = first else {
            return;
        };
        assert_eq!(first.name, "SharedProgram");
        assert_eq!(first.source, "shared");
        assert_eq!(first.program_id_hex, program_id);
        assert_eq!(first.account_type, "ShortAccount");
        assert_eq!(first.shared_topic, topic);
        assert_eq!(first.shared_account_id, "account-1");

        assert!(
            accepted_shared_idl_entries_from_store(
                topic,
                &store,
                "account-1",
                "0102",
                Some(program_id),
            )
            .is_empty()
        );
        assert!(
            accepted_shared_idl_entries_from_store(
                topic,
                &store,
                "account-1",
                "01",
                Some(other_program_id),
            )
            .is_empty()
        );
    }
}
