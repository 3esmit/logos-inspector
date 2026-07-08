use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::{social::social_messages_from_store as decode_social_messages, source_routing::Args};

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
