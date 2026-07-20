use std::{
    sync::{Arc, atomic::AtomicU8},
    time::Duration,
};

use anyhow::{Context as _, Result};
use serde_json::Value;
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

use crate::{
    modules::logos_core::{ModuleCallControl, SharedModuleTransport},
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

const SHARED_IDL_DOWNLOAD_MAX_BYTES: usize = 16 * 1024 * 1024;
const SHARED_IDL_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) const METHOD_CATALOG: &[RuntimeMethodEntry] = &[
    RuntimeMethodEntry::sync("socialCommentTopic", social_comment_topic),
    RuntimeMethodEntry::sync("socialZoneCommentTopic", social_zone_comment_topic),
    RuntimeMethodEntry::sync("socialZoneAccountIdlTopic", social_zone_account_idl_topic),
    RuntimeMethodEntry::sync("socialMessagesFromStore", social_messages_from_store),
    RuntimeMethodEntry::sync("socialCommentPageFromStore", social_comment_page_from_store),
    RuntimeMethodEntry::sync("socialCommentRowFromEvent", social_comment_row_from_event),
    RuntimeMethodEntry::sync("socialTopicValid", social_topic_valid),
    RuntimeMethodEntry::with_module_transport(
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
    module_transport: SharedModuleTransport,
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
        hydrate_shared_idl_payload(
            runtime,
            &storage,
            &module_transport,
            local_only,
            &mut message.payload,
        )?;
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
    module_transport: &SharedModuleTransport,
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
        .block_on(storage.download_bytes_bounded_controlled(
            module_transport,
            idl_cid,
            local_only,
            "shared IDL CID fetch through Basecamp storage_module is unavailable; select Logoscore CLI or Direct REST",
            SHARED_IDL_DOWNLOAD_MAX_BYTES,
            shared_idl_download_control(),
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

fn shared_idl_download_control() -> ModuleCallControl {
    ModuleCallControl::new(
        CancellationToken::new(),
        tokio::time::Instant::now() + SHARED_IDL_DOWNLOAD_TIMEOUT,
        Arc::new(AtomicU8::new(0)),
    )
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read as _, Write as _},
        net::TcpListener,
        sync::Arc,
        thread,
        time::Duration,
    };

    use anyhow::{Context as _, Result, bail};
    use serde_json::json;

    use super::*;

    #[test]
    fn shared_idl_hydration_has_its_own_explicit_download_bound() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let server = thread::spawn(move || -> Result<()> {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let bytes = stream.read(&mut buffer)?;
                if bytes == 0 {
                    bail!("shared IDL request headers were incomplete");
                }
                request.extend_from_slice(
                    buffer
                        .get(..bytes)
                        .context("shared IDL request chunk was invalid")?,
                );
            }
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                SHARED_IDL_DOWNLOAD_MAX_BYTES.saturating_add(1)
            )?;
            Ok(())
        });
        let storage = storage_layer::StorageClient::from_initialization(&json!({
            "source_mode": "rest",
            "inputs": { "rest_endpoint": endpoint }
        }))?;
        let mut payload = SocialPayload::LezAccountIdl {
            version: 1,
            identity: json!({}),
            account_id: "account-1".to_owned(),
            program_id: "program-1".to_owned(),
            idl_name: "IDL".to_owned(),
            idl_json: String::new(),
            idl_cid: "cid-idl".to_owned(),
            storage: Some(json!({})),
            created_at: "1".to_owned(),
            scope: None,
        };
        let runtime = Runtime::new()?;
        let module_transport: SharedModuleTransport = Arc::new(
            crate::modules::logos_core::UnavailableModuleTransport::basecamp_host_not_configured(),
        );

        let error =
            hydrate_shared_idl_payload(&runtime, &storage, &module_transport, false, &mut payload)
                .err()
                .context("oversized shared IDL should fail")?;
        server
            .join()
            .map_err(|_| anyhow::anyhow!("shared IDL test server panicked"))??;

        if !format!("{error:#}").contains(&format!(
            "http response body exceeded {} byte limit",
            SHARED_IDL_DOWNLOAD_MAX_BYTES
        )) {
            bail!("unexpected shared IDL size error: {error:#}");
        }
        Ok(())
    }
}
