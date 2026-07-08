use anyhow::{Context as _, Result, bail};
use serde::Serialize;
use serde_json::{Value, json};
use std::time::Duration;

use crate::{ProbeReport, logos_node_cryptarchia_info, raw_http_json, response_excerpt};

const BLOCK_STREAM_SNAPSHOT_TIMEOUT: Duration = Duration::from_millis(250);
const BLOCK_STREAM_SNAPSHOT_MAX_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct BlockchainNodeReport {
    pub endpoint: String,
    pub cryptarchia_info: ProbeReport,
    pub headers: ProbeReport,
    pub network_info: ProbeReport,
    pub mantle_metrics: ProbeReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlockchainLiveBlocksReport {
    pub endpoint: String,
    pub source: String,
    pub blocks: Vec<Value>,
    pub unknown_events: Vec<Value>,
}

pub async fn blockchain_node_report(endpoint: &str) -> BlockchainNodeReport {
    let (cryptarchia_info, headers, network_info, mantle_metrics) = tokio::join!(
        logos_node_cryptarchia_info(endpoint),
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

pub async fn blockchain_live_blocks_snapshot(
    endpoint: &str,
    slot_from: u64,
    slot_to: u64,
    limit: u64,
) -> Result<BlockchainLiveBlocksReport> {
    if slot_from > slot_to {
        bail!("slot_from must be less than or equal to slot_to");
    }
    let limit = limit.clamp(1, 500);
    let mut report = match blockchain_blocks_range_text(endpoint, slot_from, slot_to, limit).await {
        Ok(text) => {
            let mut report = parse_block_stream_response(&text)?;
            report.endpoint = endpoint.to_owned();
            report.source = "blocks_range".to_owned();
            report
        }
        Err(range_error) => {
            let blocks = legacy_recent_blocks(endpoint, slot_from, slot_to, limit)
                .await
                .with_context(|| format!("live blocks_range failed: {range_error:#}"))?;
            BlockchainLiveBlocksReport {
                endpoint: endpoint.to_owned(),
                source: "range_fallback".to_owned(),
                blocks: value_array(blocks),
                unknown_events: Vec::new(),
            }
        }
    };

    if let Ok(text) = blockchain_blocks_stream_text(endpoint, limit).await {
        match parse_block_stream_response(&text) {
            Ok(stream_report) => {
                report.blocks.extend(stream_report.blocks);
                report.unknown_events.extend(stream_report.unknown_events);
                report.source.push_str("+stream");
            }
            Err(error) => {
                report.unknown_events.push(json!({
                    "source": "cryptarchia/events/blocks/stream",
                    "error": error.to_string(),
                    "raw": response_excerpt(&text),
                }));
            }
        }
    }
    report.blocks = dedupe_stream_blocks(report.blocks, limit);
    Ok(report)
}

async fn blockchain_blocks_range(
    endpoint: &str,
    slot_from: u64,
    slot_to: u64,
    limit: u64,
) -> Result<Value> {
    parse_blocks_range_response(
        &blockchain_blocks_range_text(endpoint, slot_from, slot_to, limit).await?,
    )
}

async fn blockchain_blocks_range_text(
    endpoint: &str,
    slot_from: u64,
    slot_to: u64,
    limit: u64,
) -> Result<String> {
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
    Ok(text)
}

async fn blockchain_blocks_stream_text(endpoint: &str, limit: u64) -> Result<String> {
    let endpoint = endpoint.trim_end_matches('/');
    let limit = limit.min(100);
    let url = format!("{endpoint}/cryptarchia/events/blocks/stream?prefetch-limit={limit}");
    let mut response = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .with_context(|| format!("failed to call {url}"))?;
    let status = response.status();
    if !status.is_success() {
        bail!("block stream HTTP {status}");
    }

    let mut text = String::new();
    loop {
        if text.len() >= BLOCK_STREAM_SNAPSHOT_MAX_BYTES {
            break;
        }
        let chunk =
            match tokio::time::timeout(BLOCK_STREAM_SNAPSHOT_TIMEOUT, response.chunk()).await {
                Ok(chunk) => chunk.context("failed to read block stream chunk")?,
                Err(_) => break,
            };
        let Some(chunk) = chunk else {
            break;
        };
        text.push_str(&String::from_utf8_lossy(&chunk));
    }

    if text.trim().is_empty() {
        bail!("block stream produced no snapshot events");
    }
    Ok(text)
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
    let info = logos_node_cryptarchia_info(endpoint).await?;
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
        .or_else(|| info.get(field).and_then(Value::as_u64))
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

fn value_array(value: Value) -> Vec<Value> {
    match value {
        Value::Array(values) => values,
        _ => Vec::new(),
    }
}

fn dedupe_stream_blocks(blocks: Vec<Value>, limit: u64) -> Vec<Value> {
    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();
    for block in blocks {
        let keys = block_dedupe_keys(&block);
        if !keys.is_empty() && keys.iter().any(|key| seen.contains(key)) {
            continue;
        }
        for key in keys {
            seen.insert(key);
        }
        deduped.push(block);
    }
    deduped.sort_by_key(|block| std::cmp::Reverse(block_slot(block)));
    deduped.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
    deduped
}

fn block_dedupe_keys(block: &Value) -> Vec<String> {
    let header = block.get("header").and_then(Value::as_object);
    let mut keys = Vec::new();
    let hash = header
        .and_then(|header| header.get("id").or_else(|| header.get("hash")))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !hash.is_empty() {
        keys.push(format!("hash:{hash}"));
    }
    let slot = header
        .and_then(|header| header.get("slot"))
        .and_then(Value::as_u64)
        .unwrap_or_default();
    if slot > 0 {
        keys.push(format!("slot:{slot}"));
    }
    keys
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

fn parse_block_stream_response(text: &str) -> Result<BlockchainLiveBlocksReport> {
    let mut blocks = Vec::new();
    let mut unknown_events = Vec::new();
    for event in stream_event_values(text)? {
        if let Some(block) = block_from_stream_event(&event) {
            blocks.push(block);
        } else {
            unknown_events.push(event);
        }
    }
    Ok(BlockchainLiveBlocksReport {
        endpoint: String::new(),
        source: "stream".to_owned(),
        blocks,
        unknown_events,
    })
}

fn stream_event_values(text: &str) -> Result<Vec<Value>> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if trimmed.starts_with('[') {
        return serde_json::from_str(trimmed)
            .with_context(|| format!("invalid stream JSON: {}", response_excerpt(trimmed)));
    }

    Ok(text
        .lines()
        .filter_map(stream_line_payload)
        .map(|line| serde_json::from_str(line).unwrap_or_else(|_| Value::String(line.to_owned())))
        .collect())
}

fn stream_line_payload(line: &str) -> Option<&str> {
    let line = line.trim();
    if line.is_empty() || line.starts_with("event:") || line.starts_with("id:") {
        return None;
    }
    Some(line.strip_prefix("data:").map(str::trim).unwrap_or(line))
}

fn block_from_stream_event(event: &Value) -> Option<Value> {
    if let Some(block) = event.get("block").and_then(block_payload_value) {
        return Some(block_with_event_chain_state(block, event));
    }
    if let Some(block) = event
        .get("newBlock")
        .or_else(|| event.get("new_block"))
        .and_then(|value| block_from_stream_event(value).or_else(|| block_payload_value(value)))
    {
        return Some(block);
    }
    if event.get("header").is_some() {
        return Some(event.clone());
    }
    None
}

fn block_payload_value(value: &Value) -> Option<Value> {
    match value {
        Value::Object(_) => Some(value.clone()),
        Value::String(text) => serde_json::from_str::<Value>(text)
            .ok()
            .filter(|value| value.get("header").is_some()),
        _ => None,
    }
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
        .and_then(|block| block_payload_value(&block))
    else {
        bail!("blocks_range event did not include a block object");
    };

    Ok(block_with_chain_state(block, tip, tip_slot, lib, lib_slot))
}

fn block_with_event_chain_state(block: Value, event: &Value) -> Value {
    block_with_chain_state(
        block,
        event.get("tip").cloned().unwrap_or(Value::Null),
        event.get("tip_slot").cloned().unwrap_or(Value::Null),
        event.get("lib").cloned().unwrap_or(Value::Null),
        event.get("lib_slot").cloned().unwrap_or(Value::Null),
    )
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
    fn block_stream_response_accepts_stringified_block_payload() -> Result<()> {
        let body = r#"{"block":"{\"header\":{\"id\":\"live-1\",\"slot\":41},\"transactions\":[]}","tip":"live-1","tip_slot":41,"lib":"lib","lib_slot":20}"#;

        let report = parse_block_stream_response(body)?;

        ensure_block_count(&report, 1)?;
        ensure_no_unknown_events(&report)?;
        let block = report.blocks.first().context("first block should exist")?;
        ensure_nested_string(block, &["header", "id"], "live-1")?;
        ensure_nested_string(block, &["_chain", "status"], "pending")?;
        Ok(())
    }

    #[test]
    fn block_stream_response_accepts_object_block_payload() -> Result<()> {
        let body = r#"[{"block":{"header":{"id":"live-2","slot":42},"transactions":[]},"tip":"live-2","tip_slot":42,"lib":"lib","lib_slot":20}]"#;

        let report = parse_block_stream_response(body)?;

        ensure_block_count(&report, 1)?;
        ensure_no_unknown_events(&report)?;
        let block = report.blocks.first().context("first block should exist")?;
        ensure_nested_string(block, &["header", "id"], "live-2")?;
        ensure_nested_string(block, &["_chain", "status"], "pending")?;
        Ok(())
    }

    #[test]
    fn block_stream_response_accepts_direct_block_payload() -> Result<()> {
        let body = r#"event: newBlock
data: {"header":{"id":"live-3","slot":43},"transactions":[]}"#;

        let report = parse_block_stream_response(body)?;

        ensure_block_count(&report, 1)?;
        ensure_no_unknown_events(&report)?;
        let block = report.blocks.first().context("first block should exist")?;
        ensure_nested_string(block, &["header", "id"], "live-3")?;
        Ok(())
    }

    #[test]
    fn block_stream_response_accepts_new_block_wrapper() -> Result<()> {
        let body =
            r#"{"newBlock":{"block":{"header":{"id":"live-3b","slot":43},"transactions":[]}}}"#;

        let report = parse_block_stream_response(body)?;

        ensure_block_count(&report, 1)?;
        ensure_no_unknown_events(&report)?;
        let block = report.blocks.first().context("first block should exist")?;
        ensure_nested_string(block, &["header", "id"], "live-3b")?;
        Ok(())
    }

    #[test]
    fn block_stream_response_preserves_unknown_events() -> Result<()> {
        let body = r#"{"kind":"heartbeat"}
{"block":{"header":{"id":"live-4","slot":44},"transactions":[]}}"#;

        let report = parse_block_stream_response(body)?;

        ensure_block_count(&report, 1)?;
        ensure_unknown_event_count(&report, 1)?;
        let unknown = report
            .unknown_events
            .first()
            .context("first unknown event should exist")?;
        if unknown.get("kind") != Some(&json!("heartbeat")) {
            bail!("expected heartbeat unknown event, got {unknown:?}");
        }
        let block = report.blocks.first().context("first block should exist")?;
        ensure_nested_string(block, &["header", "id"], "live-4")?;
        Ok(())
    }

    #[test]
    fn dedupe_stream_blocks_suppresses_duplicate_headers() -> Result<()> {
        let blocks = vec![
            json!({ "header": { "id": "dup", "slot": 30 } }),
            json!({ "header": { "id": "dup", "slot": 30 } }),
            json!({ "header": { "id": "same-slot", "slot": 30 } }),
            json!({ "header": { "id": "new", "slot": 31 } }),
        ];

        let deduped = dedupe_stream_blocks(blocks, 10);

        if deduped.len() != 2 {
            bail!("expected two deduped blocks, got {}", deduped.len());
        }
        let first = deduped
            .first()
            .context("first deduped block should exist")?;
        let second = deduped
            .get(1)
            .context("second deduped block should exist")?;
        ensure_nested_string(first, &["header", "id"], "new")?;
        ensure_nested_string(second, &["header", "id"], "dup")?;
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

    fn ensure_block_count(report: &BlockchainLiveBlocksReport, expected: usize) -> Result<()> {
        if report.blocks.len() != expected {
            bail!(
                "expected {expected} live blocks, got {}",
                report.blocks.len()
            );
        }
        Ok(())
    }

    fn ensure_no_unknown_events(report: &BlockchainLiveBlocksReport) -> Result<()> {
        ensure_unknown_event_count(report, 0)
    }

    fn ensure_unknown_event_count(
        report: &BlockchainLiveBlocksReport,
        expected: usize,
    ) -> Result<()> {
        if report.unknown_events.len() != expected {
            bail!(
                "expected {expected} unknown events, got {}",
                report.unknown_events.len()
            );
        }
        Ok(())
    }
}
