use anyhow::{Context as _, Result};
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{
    social::{
        SocialCommentQuery, SocialPayload,
        accepted_shared_idl_entries_from_messages as decode_accepted_shared_idls_from_messages,
        build_comment_topic as build_social_comment_topic, build_zone_account_idl_topic,
        build_zone_comment_topic, decode_comment_page as decode_social_comment_page,
        decode_social_messages, project_comment_event as decode_social_comment_row, validate_topic,
    },
    source_routing::storage_layer,
    support::args::Args,
};

use super::super::value::to_value;
use super::RuntimeMethodEntry;

pub(super) const METHOD_CATALOG: &[RuntimeMethodEntry] = &[
    RuntimeMethodEntry::sync("socialCommentTopic", social_comment_topic),
    RuntimeMethodEntry::sync("socialZoneCommentTopic", social_zone_comment_topic),
    RuntimeMethodEntry::sync("socialZoneAccountIdlTopic", social_zone_account_idl_topic),
    RuntimeMethodEntry::sync("socialMessagesFromStore", social_messages_from_store),
    RuntimeMethodEntry::sync("socialCommentPageFromStore", social_comment_page_from_store),
    RuntimeMethodEntry::sync("socialCommentRowFromEvent", social_comment_row_from_event),
    RuntimeMethodEntry::sync("socialTopicValid", social_topic_valid),
    RuntimeMethodEntry::with_runtime(
        "acceptedSharedIdlEntriesFromStoreWithStorage",
        accepted_shared_idl_entries_from_store_with_storage,
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

pub(super) fn social_zone_comment_topic(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let entity = serde_json::from_value(
        args.value(0)
            .context("Zone L2 entity reference is required")?
            .clone(),
    )
    .context("Zone L2 entity reference is invalid")?;
    to_value(build_zone_comment_topic(&entity).unwrap_or_default())
}

pub(super) fn social_zone_account_idl_topic(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let entity = serde_json::from_value(
        args.value(0)
            .context("Zone L2 account reference is required")?
            .clone(),
    )
    .context("Zone L2 account reference is invalid")?;
    to_value(build_zone_account_idl_topic(&entity).unwrap_or_default())
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

pub(super) fn accepted_shared_idl_entries_from_store_with_storage(
    runtime: &Runtime,
    args: Value,
) -> Result<Value> {
    let args = Args::new(args)?;
    let topic = args.string(0, "social topic")?;
    let value = args
        .value(1)
        .context("Delivery Store response is required")?;
    let account_id = args.string(2, "account id")?;
    let account_data_hex = args.string(3, "account data hex")?;
    let owner_program_id = args.optional_string(4);
    let storage = storage_layer::StorageClient::from_initialization(
        args.value(5)
            .context("Storage adapter initialization is required")?,
    )?;
    let local_only = args.optional_bool(6);
    let mut messages = decode_social_messages(
        SocialCommentQuery {
            topic,
            expected_account_id: Some(account_id),
        },
        value,
    );
    for message in &mut messages {
        hydrate_shared_idl_payload(runtime, &storage, local_only, &mut message.payload)?;
    }
    to_value(decode_accepted_shared_idls_from_messages(
        topic,
        messages,
        account_id,
        account_data_hex,
        owner_program_id,
    ))
}

fn hydrate_shared_idl_payload(
    runtime: &Runtime,
    storage: &storage_layer::StorageClient,
    local_only: bool,
    payload: &mut SocialPayload,
) -> Result<()> {
    let SocialPayload::LezAccountIdl {
        idl_json, idl_cid, ..
    } = payload
    else {
        return Ok(());
    };
    if !idl_json.is_empty() || idl_cid.is_empty() {
        return Ok(());
    }
    let bytes = runtime
        .block_on(storage.download_bytes(
            idl_cid,
            local_only,
            "shared IDL CID fetch through storage_module needs storageDownloadDone correlation; use Direct REST source for synchronous shared IDL fetch",
        ))
        .with_context(|| format!("failed to fetch shared IDL CID {idl_cid}"))?;
    let text = String::from_utf8(bytes).context("shared IDL CID payload is not UTF-8")?;
    let value: Value = serde_json::from_str(&text).context("shared IDL CID payload is not JSON")?;
    *idl_json = value
        .get("idl_json")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or(text);
    let _idl_value: Value =
        serde_json::from_str(idl_json.as_str()).context("shared IDL JSON is not valid JSON")?;
    Ok(())
}
