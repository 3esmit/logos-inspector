mod delivery_store;
mod payload;
mod topic;

pub use delivery_store::{SocialMessage, social_messages_from_store};
pub use payload::{SocialPayload, parse_social_payload};
pub use topic::{SocialEntity, SocialLayer, comment_topic, lez_account_idl_topic};

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
    }
}
