use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::{
    social::{
        SharedAccountIdlQuery, SocialCommentQuery,
        accepted_shared_account_idls as decode_accepted_shared_idls,
        build_comment_topic as build_social_comment_topic, build_lez_account_idl_topic,
        decode_comment_page as decode_social_comment_page, decode_social_messages,
        project_comment_event as decode_social_comment_row, validate_topic,
    },
    source_routing::Args,
};

use super::super::value::to_value;
use super::RuntimeMethodEntry;

pub(super) const METHOD_CATALOG: &[RuntimeMethodEntry] = &[
    RuntimeMethodEntry::sync("socialCommentTopic", social_comment_topic),
    RuntimeMethodEntry::sync("socialLezAccountIdlTopic", social_lez_account_idl_topic),
    RuntimeMethodEntry::sync("socialMessagesFromStore", social_messages_from_store),
    RuntimeMethodEntry::sync("socialCommentPageFromStore", social_comment_page_from_store),
    RuntimeMethodEntry::sync("socialCommentRowFromEvent", social_comment_row_from_event),
    RuntimeMethodEntry::sync("socialTopicValid", social_topic_valid),
    RuntimeMethodEntry::sync(
        "acceptedSharedIdlEntriesFromStore",
        accepted_shared_idl_entries_from_store,
    ),
];

pub(super) fn social_comment_topic(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(
        build_social_comment_topic(
            args.string(0, "social layer")?,
            args.string(1, "social entity")?,
            args.string(2, "social topic id")?,
        )
        .unwrap_or_default(),
    )
}

pub(super) fn social_lez_account_idl_topic(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(build_lez_account_idl_topic(args.string(0, "account id")?).unwrap_or_default())
}

pub(super) fn social_messages_from_store(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let topic = args.string(0, "social topic")?;
    let value = args
        .value(1)
        .context("Delivery Store response is required")?;
    let expected_account = args.optional_string(2);
    to_value(decode_social_messages(
        SocialCommentQuery {
            topic,
            expected_account_id: expected_account,
        },
        value,
    ))
}

pub(super) fn social_comment_page_from_store(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let topic = args.string(0, "social topic")?;
    let value = args
        .value(1)
        .context("Delivery Store response is required")?;
    let expected_account = args.optional_string(2);
    to_value(decode_social_comment_page(
        SocialCommentQuery {
            topic,
            expected_account_id: expected_account,
        },
        value,
    ))
}

pub(super) fn social_comment_row_from_event(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let value = args.value(0).context("social event is required")?;
    to_value(decode_social_comment_row(value))
}

pub(super) fn social_topic_valid(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(validate_topic(args.string(0, "social topic")?))
}

pub(super) fn accepted_shared_idl_entries_from_store(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let topic = args.string(0, "social topic")?;
    let value = args
        .value(1)
        .context("Delivery Store response is required")?;
    to_value(decode_accepted_shared_idls(
        SharedAccountIdlQuery {
            topic,
            account_id: args.string(2, "account id")?,
            account_data_hex: args.string(3, "account data hex")?,
            owner_program_id: args.optional_string(4),
        },
        value,
    ))
}
