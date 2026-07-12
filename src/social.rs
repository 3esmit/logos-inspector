pub mod collaboration;
mod comments;
mod delivery_store;
mod payload;
mod shared_idl;
mod topic;

pub use collaboration::{
    SocialCommentQuery, build_comment_topic, build_zone_account_idl_topic,
    build_zone_comment_topic, decode_comment_page, decode_social_messages, project_comment_event,
    validate_topic,
};
pub use comments::{
    SocialCommentPage, SocialCommentRow, social_comment_page_from_store,
    social_comment_row_from_event, social_comment_rows_from_messages,
};
pub use delivery_store::{
    SocialMessage, last_social_message_cursor, social_messages_from_store, social_store_cursor,
};
pub use payload::{SocialPayload, parse_social_payload};
pub use shared_idl::{AcceptedSharedIdlEntry, accepted_shared_idl_entries_from_messages};
pub use topic::{
    SocialEntity, SocialLayer, ZoneSocialScope, comment_topic, comment_topic_from_parts,
    social_topic_is_valid, zone_account_idl_topic, zone_comment_topic, zone_topic_matches_scope,
};

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
    use serde_json::json;

    use super::*;
    use crate::inspection::{
        NetworkScope, ZoneKind,
        l2::{ZoneL2EntityKind, ZoneL2EntityRef, ZoneL2SourceQualifier},
    };

    fn zone_entity(entity_kind: ZoneL2EntityKind, canonical_key: &str) -> ZoneL2EntityRef {
        ZoneL2EntityRef {
            network_scope: NetworkScope::GenesisId {
                genesis_id: "11".repeat(32),
            },
            channel_id: "22".repeat(32),
            zone_kind: ZoneKind::SequencerZone,
            entity_kind,
            canonical_key: test_entity_key(entity_kind, canonical_key),
            source: ZoneL2SourceQualifier::Policy,
        }
    }

    fn zone_scope(entity_kind: ZoneL2EntityKind, canonical_key: &str) -> ZoneSocialScope {
        ZoneSocialScope {
            network_scope: NetworkScope::GenesisId {
                genesis_id: "11".repeat(32),
            },
            zone_id: "22".repeat(32),
            entity_kind,
            canonical_entity_key: test_entity_key(entity_kind, canonical_key),
        }
    }

    fn test_entity_key(entity_kind: ZoneL2EntityKind, value: &str) -> String {
        if entity_kind != ZoneL2EntityKind::Account {
            return value.to_owned();
        }
        let byte = if value.ends_with('2') { 2 } else { 1 };
        crate::parse_account_id(&format!("{byte:02x}").repeat(32))
            .map(|account_id| account_id.to_string())
            .unwrap_or_default()
    }

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
        ];

        for (layer, entity, id, expected) in cases {
            assert_eq!(comment_topic(layer, entity, id).as_deref(), Some(expected));
        }
        assert_eq!(
            comment_topic(SocialLayer::Lez, SocialEntity::Account, "acct-2"),
            None
        );
        let account = zone_entity(ZoneL2EntityKind::Account, "account-2");
        assert!(zone_comment_topic(&account).is_some_and(|topic| validate_topic(&topic)));
        assert!(zone_account_idl_topic(&account).is_some_and(|topic| validate_topic(&topic)));
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
        let mut invalid = zone_entity(ZoneL2EntityKind::Account, "account-1");
        invalid.network_scope = NetworkScope::FinalizedAnchor {
            genesis_time: "2026-01-01T00:00:00Z".to_owned(),
            block_slot: 1,
            block_id: "33".repeat(32),
            parent_id: "44".repeat(32),
        };
        assert_eq!(zone_account_idl_topic(&invalid), None);
    }

    #[test]
    fn zone_topic_digest_is_stable_and_scope_separated() {
        let account = zone_entity(ZoneL2EntityKind::Account, "account-1");
        let topic = zone_comment_topic(&account).unwrap_or_default();
        assert_eq!(
            topic,
            "/lez/account/f2616d1bce2ca85cd7cd1e8ab584ffed3069b63134f55483d59ba89cff4e0d87/comments"
        );
        let mut account_hex_alias = account.clone();
        account_hex_alias.canonical_key = "01".repeat(32);
        assert_eq!(zone_comment_topic(&account_hex_alias), Some(topic.clone()));

        let mut other_zone = account.clone();
        other_zone.channel_id = "33".repeat(32);
        let mut conflicting_block = zone_entity(
            ZoneL2EntityKind::Block,
            &format!("block:7:{}", "44".repeat(32)),
        );
        let first_block_topic = zone_comment_topic(&conflicting_block);
        conflicting_block.canonical_key = format!("block:7:{}", "55".repeat(32));

        assert_ne!(zone_comment_topic(&other_zone), Some(topic));
        assert_ne!(zone_comment_topic(&conflicting_block), first_block_topic);
    }

    #[test]
    fn lez_store_rejects_payload_scope_topic_mismatch() {
        let entity = zone_entity(ZoneL2EntityKind::Account, "account-1");
        let topic = zone_comment_topic(&entity).unwrap_or_default();
        let mut scope = zone_scope(ZoneL2EntityKind::Account, "account-1");
        scope.zone_id = "33".repeat(32);
        let payload = json!({
            "kind": "comment",
            "version": 2,
            "identity": { "display_name": "Ada" },
            "body": "wrong Zone",
            "created_at": "2026-07-05T00:00:00.000Z",
            "conversation_id": topic,
            "scope": scope
        });
        let store = json!({
            "messages": [{
                "contentTopic": topic,
                "payload": BASE64_STANDARD.encode(payload.to_string())
            }]
        });

        assert!(social_messages_from_store(&topic, &store, None).is_empty());
    }

    #[test]
    fn comment_topic_from_parts_accepts_ui_aliases() {
        assert_eq!(
            comment_topic_from_parts("bedrock", "tx", "tx-1").as_deref(),
            Some("/cryptarchia/transaction/tx-1/comments")
        );
        assert_eq!(comment_topic_from_parts("l2", "account", "account-1"), None);
        assert_eq!(comment_topic_from_parts("missing", "tx", "tx-1"), None);
    }

    #[test]
    fn comment_payload_requires_supported_kind_and_body() {
        let payload = json!({
            "kind": "comment",
            "version": 1,
            "identity": { "display_name": "Ada" },
            "body": "hello",
            "created_at": "2026-07-05T00:00:00.000Z",
            "conversation_id": "/cryptarchia/account/acct/comments"
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
    fn idl_payload_requires_storage_cid_and_account_match() {
        let scope = zone_scope(ZoneL2EntityKind::Account, "account-1");
        let account_id = scope.canonical_entity_key.clone();
        let payload = json!({
            "kind": "lez_account_idl",
            "version": 2,
            "identity": { "display_name": "Ada" },
            "account_id": account_id,
            "program_id": "program-1",
            "idl_name": "Sample",
            "idl_cid": "cid-idl",
            "storage": {
                "cid": "cid-idl",
                "provider": "logos_storage",
                "endpoint": "http://storage.example"
            },
            "created_at": "2026-07-05T00:00:00.000Z",
            "scope": scope
        });

        let parsed = parse_social_payload(&payload.to_string(), Some(&account_id));

        assert!(matches!(parsed, Ok(SocialPayload::LezAccountIdl { .. })));
        assert!(
            parse_social_payload(
                &json!({
                    "kind": "lez_account_idl",
                    "version": 1,
                    "identity": { "display_name": "Ada" },
                    "account_id": "account-1",
                    "program_id": "program-1",
                    "idl_name": "Sample",
                    "idl_json": "{\"name\":\"Sample\",\"accounts\":[]}",
                    "created_at": "2026-07-05T00:00:00.000Z"
                })
                .to_string(),
                Some("account-1"),
            )
            .is_err()
        );
        assert!(
            parse_social_payload(
                &payload.to_string(),
                Some(&test_entity_key(ZoneL2EntityKind::Account, "account-2"))
            )
            .is_err()
        );
        assert!(
            parse_social_payload(
                &json!({
                    "kind": "lez_account_idl",
                    "version": 1,
                    "identity": {},
                    "account_id": "account-1",
                    "program_id": "program-1",
                    "idl_name": "Sample",
                    "idl_cid": "cid-idl",
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
        let entity = zone_entity(ZoneL2EntityKind::Account, "account-1");
        let scope = zone_scope(ZoneL2EntityKind::Account, "account-1");
        let topic = zone_comment_topic(&entity).unwrap_or_default();
        let other_topic = zone_comment_topic(&zone_entity(ZoneL2EntityKind::Account, "account-2"))
            .unwrap_or_default();
        let valid_payload = json!({
            "kind": "comment",
            "version": 2,
            "identity": { "display_name": "Ada" },
            "body": "hello",
            "created_at": "2026-07-05T00:00:00.000Z",
            "conversation_id": topic,
            "scope": scope
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
                    "contentTopic": other_topic,
                    "payload": BASE64_STANDARD.encode(valid_payload.to_string())
                }
            ],
            "paginationCursor": "next"
        });

        let messages = social_messages_from_store(&topic, &store, None);

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
        let entity = zone_entity(ZoneL2EntityKind::Account, "account-1");
        let scope = zone_scope(ZoneL2EntityKind::Account, "account-1");
        let topic = zone_comment_topic(&entity).unwrap_or_default();
        let valid_payload = json!({
            "kind": "comment",
            "version": 2,
            "identity": { "display_name": "Ada" },
            "body": "hello",
            "created_at": "2026-07-05T00:00:00.000Z",
            "conversation_id": topic,
            "scope": scope
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

        let page = social_comment_page_from_store(&topic, &store, None);

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
        let entity = zone_entity(ZoneL2EntityKind::Account, "account-1");
        let scope = zone_scope(ZoneL2EntityKind::Account, "account-1");
        let topic = zone_comment_topic(&entity).unwrap_or_default();
        let event = json!({
            "topic": topic,
            "messageHash": "hash-1",
            "payload": {
                "kind": "comment",
                "version": 2,
                "identity": { "display_name": "Peer" },
                "body": "hello",
                "created_at": "2026-07-07T00:00:00Z",
                "conversation_id": topic,
                "scope": scope
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
    fn social_topic_validation_accepts_scoped_lez_and_unchanged_l1_topics() {
        let topic = zone_comment_topic(&zone_entity(ZoneL2EntityKind::Account, "account-1"))
            .unwrap_or_default();
        assert!(social_topic_is_valid(&topic));
        assert!(social_topic_is_valid(
            "/cryptarchia/account/account-1/comments"
        ));
        assert!(!social_topic_is_valid("/lez/account/account-1/comments"));
        assert!(!social_topic_is_valid("/lez/account/account-1"));
        assert!(!social_topic_is_valid("lez/account/account-1/comments"));
    }

    #[test]
    fn shared_idl_acceptance_keeps_only_full_account_decodes() {
        let entity = zone_entity(ZoneL2EntityKind::Account, "account-1");
        let scope = zone_scope(ZoneL2EntityKind::Account, "account-1");
        let topic = zone_account_idl_topic(&entity).unwrap_or_default();
        let account_id = scope.canonical_entity_key.clone();
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
        let messages = vec![SocialMessage {
            topic: topic.to_owned(),
            cursor: "cursor-1".to_owned(),
            timestamp: "2026-07-05T00:00:00.000Z".to_owned(),
            payload: SocialPayload::LezAccountIdl {
                version: 2,
                identity: json!({ "display_name": "Ada" }),
                account_id: account_id.clone(),
                program_id: program_id.to_owned(),
                idl_name: "SharedProgram".to_owned(),
                idl_json: idl.to_owned(),
                idl_cid: "cid-idl".to_owned(),
                storage: Some(json!({ "cid": "cid-idl", "provider": "logos_storage" })),
                created_at: "2026-07-05T00:00:00.000Z".to_owned(),
                scope: Some(scope.clone()),
            },
        }];

        let accepted = accepted_shared_idl_entries_from_messages(
            &topic,
            messages.clone(),
            &account_id,
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
        assert_eq!(first.shared_account_id, account_id);

        assert!(
            accepted_shared_idl_entries_from_messages(
                &topic,
                messages.clone(),
                &scope.canonical_entity_key,
                "0102",
                Some(program_id),
            )
            .is_empty()
        );
        assert!(
            accepted_shared_idl_entries_from_messages(
                &topic,
                messages,
                &scope.canonical_entity_key,
                "01",
                Some(other_program_id),
            )
            .is_empty()
        );
    }
}
