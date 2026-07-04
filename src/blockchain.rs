use anyhow::{Context as _, Result, bail};
use serde::Serialize;
use serde_json::{Value, json};

use crate::{ProbeReport, raw_http_json, response_excerpt};

#[derive(Debug, Clone, Serialize)]
pub struct BlockchainNodeReport {
    pub endpoint: String,
    pub cryptarchia_info: ProbeReport,
    pub headers: ProbeReport,
    pub network_info: ProbeReport,
    pub mantle_metrics: ProbeReport,
}

pub async fn blockchain_node_report(endpoint: &str) -> BlockchainNodeReport {
    let (cryptarchia_info, headers, network_info, mantle_metrics) = tokio::join!(
        raw_http_json(endpoint, "/cryptarchia/info"),
        raw_http_json(endpoint, "/cryptarchia/headers"),
        raw_http_json(endpoint, "/network/info"),
        raw_http_json(endpoint, "/mantle/metrics"),
    );

    BlockchainNodeReport {
        endpoint: endpoint.to_owned(),
        cryptarchia_info: ProbeReport::from_result(
            "cryptarchia info",
            "/cryptarchia/info",
            cryptarchia_info,
        ),
        headers: ProbeReport::from_result("headers", "/cryptarchia/headers", headers),
        network_info: ProbeReport::from_result("network info", "/network/info", network_info),
        mantle_metrics: ProbeReport::from_result(
            "mantle metrics",
            "/mantle/metrics",
            mantle_metrics,
        ),
    }
}

pub async fn blockchain_blocks(endpoint: &str, slot_from: u64, slot_to: u64) -> Result<Value> {
    if slot_from > slot_to {
        bail!("slot_from must be less than or equal to slot_to");
    }
    raw_http_json(
        endpoint,
        &format!("/cryptarchia/blocks?slot_from={slot_from}&slot_to={slot_to}"),
    )
    .await
}

pub async fn blockchain_recent_blocks(
    endpoint: &str,
    slot_from: u64,
    slot_to: u64,
    limit: u64,
) -> Result<Value> {
    if slot_from > slot_to {
        bail!("slot_from must be less than or equal to slot_to");
    }
    let limit = limit.clamp(1, 500);
    match blockchain_blocks_range(endpoint, slot_from, slot_to, limit).await {
        Ok(blocks) => Ok(blocks),
        Err(range_error) => legacy_recent_blocks(endpoint, slot_from, slot_to, limit)
            .await
            .with_context(|| {
                format!("blocks_range failed: {range_error:#}; legacy blocks fallback failed")
            }),
    }
}

async fn blockchain_blocks_range(
    endpoint: &str,
    slot_from: u64,
    slot_to: u64,
    limit: u64,
) -> Result<Value> {
    let batch_size = limit.min(100);
    let endpoint = endpoint.trim_end_matches('/');
    let url = format!(
        "{endpoint}/cryptarchia/blocks_range?slot_from={slot_from}&slot_to={slot_to}&order=descending&blocks_limit={limit}&server_batch_size={batch_size}&block_filter=mutable_and_immutable"
    );
    let response = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .with_context(|| format!("failed to call {url}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .context("failed to read http response body")?;
    if !status.is_success() {
        bail!(
            "http call `{url}` failed with status {status}: {}",
            response_excerpt(&text)
        );
    }
    parse_blocks_range_response(&text)
}

async fn legacy_recent_blocks(
    endpoint: &str,
    slot_from: u64,
    slot_to: u64,
    limit: u64,
) -> Result<Value> {
    let mut blocks = blockchain_blocks(endpoint, slot_from, slot_to).await?;
    if value_array_len(&blocks) == Some(0)
        && let Some((fallback_from, fallback_to)) =
            legacy_finalized_fallback_range(endpoint, slot_from, slot_to).await?
    {
        blocks = blockchain_blocks(endpoint, fallback_from, fallback_to).await?;
    }
    Ok(sort_and_limit_blocks(blocks, limit))
}

async fn legacy_finalized_fallback_range(
    endpoint: &str,
    slot_from: u64,
    slot_to: u64,
) -> Result<Option<(u64, u64)>> {
    let info = raw_http_json(endpoint, "/cryptarchia/info").await?;
    Ok(cryptarchia_slot(&info, "lib_slot")
        .and_then(|lib_slot| fallback_range_ending_at_lib(slot_from, slot_to, lib_slot)))
}

fn fallback_range_ending_at_lib(slot_from: u64, slot_to: u64, lib_slot: u64) -> Option<(u64, u64)> {
    if slot_from <= lib_slot {
        return None;
    }
    let window = slot_to.saturating_sub(slot_from);
    Some((lib_slot.saturating_sub(window), lib_slot))
}

fn cryptarchia_slot(info: &Value, field: &str) -> Option<u64> {
    info.get("cryptarchia_info")
        .and_then(|info| info.get(field))
        .and_then(Value::as_u64)
}

fn value_array_len(value: &Value) -> Option<usize> {
    value.as_array().map(Vec::len)
}

fn sort_and_limit_blocks(value: Value, limit: u64) -> Value {
    let Value::Array(mut blocks) = value else {
        return value;
    };
    blocks.sort_by_key(|block| std::cmp::Reverse(block_slot(block)));
    blocks.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
    Value::Array(blocks)
}

fn block_slot(block: &Value) -> u64 {
    block
        .get("header")
        .and_then(|header| header.get("slot"))
        .and_then(Value::as_u64)
        .unwrap_or_default()
}

pub async fn blockchain_block(endpoint: &str, block_id: &str) -> Result<Value> {
    let block_id = block_id.trim();
    if block_id.is_empty() {
        bail!("block id is required");
    }
    reject_path_markers(block_id, "block id")?;
    raw_http_json(endpoint, &format!("/cryptarchia/blocks/{block_id}")).await
}

pub async fn blockchain_transaction(endpoint: &str, transaction_id: &str) -> Result<Value> {
    let transaction_id = transaction_id.trim();
    if transaction_id.is_empty() {
        bail!("transaction id is required");
    }
    reject_path_markers(transaction_id, "transaction id")?;
    raw_http_json(
        endpoint,
        &format!("/cryptarchia/transaction/{transaction_id}"),
    )
    .await
}

fn reject_path_markers(value: &str, label: &str) -> Result<()> {
    if value.contains('/') || value.contains('?') || value.contains('#') {
        bail!("{label} cannot contain path separators or query markers");
    }
    Ok(())
}

fn parse_blocks_range_response(text: &str) -> Result<Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Value::Array(Vec::new()));
    }
    if trimmed.starts_with('[') {
        let events: Vec<Value> = serde_json::from_str(trimmed)
            .with_context(|| format!("invalid JSON response: {}", response_excerpt(trimmed)))?;
        return events
            .into_iter()
            .map(block_from_processed_event)
            .collect::<Result<Vec<_>>>()
            .map(Value::Array);
    }

    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line = line.trim();
            (!line.is_empty()).then_some((index, line))
        })
        .map(|(index, line)| {
            let event: Value = serde_json::from_str(line).with_context(|| {
                format!(
                    "invalid blocks_range event on line {}: {}",
                    index + 1,
                    response_excerpt(line)
                )
            })?;
            block_from_processed_event(event)
        })
        .collect::<Result<Vec<_>>>()
        .map(Value::Array)
}

fn block_from_processed_event(mut event: Value) -> Result<Value> {
    if event.get("block").is_none() && event.get("header").is_some() {
        return Ok(event);
    }

    let tip = event.get("tip").cloned().unwrap_or(Value::Null);
    let tip_slot = event.get("tip_slot").cloned().unwrap_or(Value::Null);
    let lib = event.get("lib").cloned().unwrap_or(Value::Null);
    let lib_slot = event.get("lib_slot").cloned().unwrap_or(Value::Null);
    let Some(block) = event
        .as_object_mut()
        .and_then(|event| event.remove("block"))
    else {
        bail!("blocks_range event did not include a block object");
    };

    Ok(block_with_chain_state(block, tip, tip_slot, lib, lib_slot))
}

fn block_with_chain_state(
    mut block: Value,
    tip: Value,
    tip_slot: Value,
    lib: Value,
    lib_slot: Value,
) -> Value {
    let chain_state = chain_state_for_block(&block, &tip, &tip_slot, &lib, &lib_slot);
    if let Some(block) = block.as_object_mut() {
        block.insert("_chain".to_owned(), chain_state);
    }
    block
}

fn chain_state_for_block(
    block: &Value,
    tip: &Value,
    tip_slot: &Value,
    lib: &Value,
    lib_slot: &Value,
) -> Value {
    let header = block.get("header").and_then(Value::as_object);
    let slot = header
        .and_then(|header| header.get("slot"))
        .and_then(Value::as_u64);
    let hash = header
        .and_then(|header| header.get("id").or_else(|| header.get("hash")))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let lib_slot_number = lib_slot.as_u64();
    let tip_slot_number = tip_slot.as_u64();
    let lib_text = lib.as_str().unwrap_or_default();
    let tip_text = tip.as_str().unwrap_or_default();
    let status = match (slot, lib_slot_number, tip_slot_number) {
        (Some(slot), Some(lib_slot), _) if slot <= lib_slot => "finalized",
        (Some(slot), _, Some(tip_slot)) if slot <= tip_slot => "pending",
        _ => "",
    };

    json!({
        "tip": tip,
        "tip_slot": tip_slot,
        "lib": lib,
        "lib_slot": lib_slot,
        "status": status,
        "is_tip": !hash.is_empty() && hash == tip_text,
        "is_lib": !hash.is_empty() && hash == lib_text,
    })
}

pub async fn mantle_status(endpoint: &str, item_ids: Value) -> Result<Value> {
    post_json(endpoint, "/mantle/status", &item_ids).await
}

pub async fn storage_block(endpoint: &str, header_id: Value) -> Result<Value> {
    post_json(endpoint, "/storage/block", &header_id).await
}

async fn post_json(endpoint: &str, path: &str, body: &Value) -> Result<Value> {
    let endpoint = endpoint.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    let url = format!("{endpoint}/{path}");
    let response = reqwest::Client::new()
        .post(&url)
        .json(body)
        .send()
        .await
        .with_context(|| format!("failed to call {url}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .context("failed to read http response body")?;
    if !status.is_success() {
        bail!(
            "http call `{url}` failed with status {status}: {}",
            response_excerpt(&text)
        );
    }
    let value: Value = serde_json::from_str(&text)
        .with_context(|| format!("invalid JSON response: {}", response_excerpt(&text)))?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_id_rejects_path_or_query_markers() -> Result<()> {
        for value in ["abc/def", "abc?def", "abc#def"] {
            let error = reject_path_markers(value, "block id");
            let Err(error) = error else {
                bail!("expected block id `{value}` to be rejected");
            };
            if !error
                .to_string()
                .contains("block id cannot contain path separators")
            {
                bail!("unexpected error: {error:#}");
            }
        }
        Ok(())
    }

    #[test]
    fn blocks_range_response_extracts_blocks_and_chain_state() -> Result<()> {
        let body = r#"{"block":{"header":{"id":"tip","slot":30},"transactions":[]},"tip":"tip","tip_slot":30,"lib":"lib","lib_slot":20}
{"block":{"header":{"id":"lib","slot":20},"transactions":[]},"tip":"tip","tip_slot":30,"lib":"lib","lib_slot":20}"#;

        let parsed = parse_blocks_range_response(body)?;
        let blocks = parsed
            .as_array()
            .context("blocks_range response should parse as an array")?;
        if blocks.len() != 2 {
            bail!("expected two blocks, got {}", blocks.len());
        }
        let first = blocks.first().context("first block should exist")?;
        let second = blocks.get(1).context("second block should exist")?;
        ensure_nested_string(first, &["header", "id"], "tip")?;
        ensure_nested_string(first, &["_chain", "status"], "pending")?;
        ensure_nested_string(second, &["header", "id"], "lib")?;
        ensure_nested_string(second, &["_chain", "status"], "finalized")?;
        Ok(())
    }

    #[test]
    fn fallback_range_ending_at_lib_moves_newer_empty_window_to_lib() {
        assert_eq!(fallback_range_ending_at_lib(150, 200, 120), Some((70, 120)));
        assert_eq!(fallback_range_ending_at_lib(100, 200, 150), None);
    }

    #[test]
    fn sort_and_limit_blocks_keeps_recent_legacy_rows() -> Result<()> {
        let blocks = json!([
            { "header": { "id": "slot-2", "slot": 2 } },
            { "header": { "id": "slot-10", "slot": 10 } },
            { "header": { "id": "slot-5", "slot": 5 } }
        ]);

        let sorted = sort_and_limit_blocks(blocks, 2);
        let blocks = sorted
            .as_array()
            .context("legacy blocks should remain an array")?;
        if blocks.len() != 2 {
            bail!("expected two limited blocks, got {}", blocks.len());
        }
        let first = blocks.first().context("first block should exist")?;
        let second = blocks.get(1).context("second block should exist")?;
        ensure_nested_string(first, &["header", "id"], "slot-10")?;
        ensure_nested_string(second, &["header", "id"], "slot-5")?;
        Ok(())
    }

    fn ensure_nested_string(value: &Value, path: &[&str], expected: &str) -> Result<()> {
        let actual = nested_string(value, path);
        if actual != Some(expected) {
            bail!("expected {path:?} to be {expected}, got {actual:?}");
        }
        Ok(())
    }

    fn nested_string<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
        let mut current = value;
        for segment in path {
            current = current.get(segment)?;
        }
        current.as_str()
    }
}
