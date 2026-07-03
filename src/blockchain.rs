use anyhow::{Context as _, Result, bail};
use serde::Serialize;
use serde_json::Value;

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
}
