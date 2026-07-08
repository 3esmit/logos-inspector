use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::{
    social::{
        accepted_shared_idl_entries_from_store as decode_accepted_shared_idls,
        social_comment_page_from_store as decode_social_comment_page,
        social_comment_row_from_event as decode_social_comment_row,
        social_messages_from_store as decode_social_messages, social_topic_is_valid,
    },
    source_routing::Args,
};

use super::super::value::to_value;

pub(super) fn social_messages_from_store(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let topic = args.string(0, "social topic")?;
    let value = args
        .value(1)
        .context("Delivery Store response is required")?;
    let expected_account = args.optional_string(2);
    to_value(decode_social_messages(topic, value, expected_account))
}

pub(super) fn social_comment_page_from_store(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let topic = args.string(0, "social topic")?;
    let value = args
        .value(1)
        .context("Delivery Store response is required")?;
    let expected_account = args.optional_string(2);
    to_value(decode_social_comment_page(topic, value, expected_account))
}

pub(super) fn social_comment_row_from_event(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let value = args.value(0).context("social event is required")?;
    to_value(decode_social_comment_row(value))
}

pub(super) fn social_topic_valid(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(social_topic_is_valid(args.string(0, "social topic")?))
}

pub(super) fn accepted_shared_idl_entries_from_store(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let topic = args.string(0, "social topic")?;
    let value = args
        .value(1)
        .context("Delivery Store response is required")?;
    to_value(decode_accepted_shared_idls(
        topic,
        value,
        args.string(2, "account id")?,
        args.string(3, "account data hex")?,
        args.optional_string(4),
    ))
}
