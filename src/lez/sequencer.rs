use std::{future::Future, time::Duration};

use anyhow::{Context as _, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use borsh::BorshDeserialize as _;
use common::transaction::LeeTransaction;
use lee_core::program::ProgramId;
use sequencer_service_rpc::{RpcClient as _, SequencerClientBuilder};
use serde_json::{Value, json};
use tokio::time::sleep;

use super::{
    BlockSummary, ProgramIdEntry, TransactionSummary,
    block::decode_sequencer_block_summary_bytes,
    programs::{program_entries, program_entries_from_ids},
    summarize_transaction,
};
use crate::{
    parse_hash,
    rpc::{raw_json_rpc, raw_json_rpc_optional_result},
};

const SEQUENCER_RPC_TIMEOUT: Duration = Duration::from_secs(8);
const SEQUENCER_RPC_ATTEMPTS: usize = 2;
const SEQUENCER_RPC_RETRY_DELAY: Duration = Duration::from_millis(100);

pub async fn sequencer_health(endpoint: &str) -> Result<()> {
    with_sequencer_rpc_retry(
        endpoint,
        |client| async move { client.check_health().await },
    )
    .await
    .context("sequencer health check failed")
}

pub async fn sequencer_channel_id(endpoint: &str) -> Result<String> {
    let response = raw_json_rpc(endpoint, "getChannelId", Value::Array(Vec::new()))
        .await
        .context("failed to fetch Sequencer Channel id")?;
    sequencer_channel_id_from_response(&response)
}

pub async fn last_sequencer_block_id(endpoint: &str) -> Result<u64> {
    with_sequencer_rpc_retry(endpoint, |client| async move {
        client.get_last_block_id().await
    })
    .await
    .context("failed to fetch last sequencer block id")
}

pub async fn sequencer_program_ids(endpoint: &str) -> Result<Vec<ProgramIdEntry>> {
    let response = raw_json_rpc(endpoint, "listPrograms", Value::Array(Vec::new()))
        .await
        .context("failed to fetch deployed Sequencer program ids")?;
    if let Some(program_ids) = list_program_ids_from_response(&response)? {
        return Ok(program_entries_from_ids(program_ids));
    }

    let programs =
        with_sequencer_rpc_retry(
            endpoint,
            |client| async move { client.get_program_ids().await },
        )
        .await
        .context("failed to fetch sequencer program ids")?;
    Ok(program_entries(programs))
}

pub async fn sequencer_account_nonces(
    endpoint: &str,
    account_ids: &[String],
) -> Result<Vec<String>> {
    let parsed = account_ids
        .iter()
        .map(|account_id| crate::parse_account_id(account_id))
        .collect::<Result<Vec<_>>>()?;
    let nonces = with_sequencer_rpc_retry(endpoint, move |client| {
        let parsed = parsed.clone();
        async move { client.get_accounts_nonces(parsed).await }
    })
    .await
    .context("failed to fetch Sequencer account nonces")?;
    if nonces.len() != account_ids.len() {
        return Err(super::evidence_protocol_error(
            "Sequencer returned an invalid account nonce count",
        ));
    }
    Ok(nonces
        .into_iter()
        .map(|nonce| nonce.0.to_string())
        .collect())
}

pub async fn sequencer_commitment_proof(
    endpoint: &str,
    commitment_hex: &str,
) -> Result<Option<(u64, Vec<String>)>> {
    let bytes = hex::decode(commitment_hex).context("commitment must be hexadecimal")?;
    if bytes.len() != 32 {
        anyhow::bail!("commitment must contain 32 bytes");
    }
    let commitment = lee_core::Commitment::deserialize_reader(&mut bytes.as_slice())
        .context("commitment did not match the LEZ commitment layout")?;
    let proof = with_sequencer_rpc_retry(endpoint, move |client| {
        let commitment = commitment.clone();
        async move { client.get_proof_for_commitment(commitment).await }
    })
    .await
    .context("failed to fetch Sequencer commitment proof")?;
    proof
        .map(|(leaf_index, siblings)| {
            Ok((
                u64::try_from(leaf_index).context("commitment proof index exceeds u64")?,
                siblings.into_iter().map(hex::encode).collect(),
            ))
        })
        .transpose()
}

pub async fn sequencer_block(endpoint: &str, block_id: u64) -> Result<Option<BlockSummary>> {
    let response = raw_json_rpc(endpoint, "getBlock", json!([block_id]))
        .await
        .with_context(|| format!("failed to fetch sequencer block {block_id}"))?;
    let Some(bytes) = sequencer_block_bytes_from_response(&response)? else {
        return Ok(None);
    };
    let summary = decode_sequencer_block_summary_bytes(&bytes)?;
    if summary.block_id != block_id {
        return Err(super::evidence_protocol_error(
            "Sequencer block response returned another block id",
        ));
    }
    Ok(Some(summary))
}

pub(crate) async fn sequencer_block_bytes(
    endpoint: &str,
    block_id: u64,
) -> Result<Option<Vec<u8>>> {
    let response = raw_json_rpc(endpoint, "getBlock", json!([block_id]))
        .await
        .with_context(|| format!("failed to fetch sequencer block {block_id}"))?;
    sequencer_block_bytes_from_response(&response)
}

pub async fn sequencer_blocks(
    endpoint: &str,
    before: Option<u64>,
    limit: u64,
) -> Result<Vec<BlockSummary>> {
    let limit = limit.min(51);
    if limit == 0 {
        return Ok(Vec::new());
    }

    let end_block_id = match before {
        Some(0) => return Ok(Vec::new()),
        Some(block_id) => block_id.saturating_sub(1),
        None => last_sequencer_block_id(endpoint).await?,
    };
    let start_block_id = end_block_id.saturating_sub(limit.saturating_sub(1));
    let response = raw_json_rpc(
        endpoint,
        "getBlockRange",
        json!([start_block_id, end_block_id]),
    )
    .await
    .with_context(|| {
        format!("failed to fetch sequencer block range {start_block_id}..={end_block_id}")
    })?;
    let blocks = sequencer_block_range_bytes_from_response(&response)?;
    blocks
        .iter()
        .rev()
        .map(|bytes| decode_sequencer_block_summary_bytes(bytes))
        .collect()
}

pub async fn sequencer_transaction(
    endpoint: &str,
    tx_hash: &str,
) -> Result<Option<TransactionSummary>> {
    let tx = fetch_sequencer_transaction(endpoint, tx_hash).await?;
    Ok(tx.as_ref().map(summarize_transaction))
}

pub(crate) fn sequencer_client(endpoint: &str) -> Result<sequencer_service_rpc::SequencerClient> {
    SequencerClientBuilder::default()
        .request_timeout(SEQUENCER_RPC_TIMEOUT)
        .build(endpoint)
        .with_context(|| format!("failed to build sequencer client for {endpoint}"))
}

async fn fetch_sequencer_transaction(
    endpoint: &str,
    tx_hash: &str,
) -> Result<Option<LeeTransaction>> {
    let hash = parse_hash(tx_hash, "transaction hash")?;
    let result =
        raw_json_rpc_optional_result(endpoint, "getTransaction", json!([hash.to_string()]))
            .await
            .with_context(|| format!("failed to fetch sequencer transaction {tx_hash}"))?;
    decode_sequencer_transaction_result(&result)
}

fn decode_sequencer_transaction_result(result: &Value) -> Result<Option<LeeTransaction>> {
    if result.is_null() {
        return Ok(None);
    }
    let Some(tuple) = result.as_array() else {
        return Err(super::evidence_protocol_error(
            "Sequencer transaction response is malformed",
        ));
    };
    if tuple.len() != 2 {
        return Err(super::evidence_protocol_error(
            "Sequencer transaction response is malformed",
        ));
    }
    let Some(encoded) = tuple.first().and_then(Value::as_str) else {
        return Err(super::evidence_protocol_error(
            "Sequencer transaction response is malformed",
        ));
    };
    if tuple.get(1).and_then(Value::as_u64).is_none() {
        return Err(super::evidence_protocol_error(
            "Sequencer transaction response is malformed",
        ));
    }
    let bytes = BASE64_STANDARD.decode(encoded).map_err(|_| {
        super::evidence_protocol_error("Sequencer transaction response is not valid base64")
    })?;
    LeeTransaction::try_from_slice(&bytes)
        .map(Some)
        .map_err(|_| super::evidence_protocol_error("Sequencer transaction payload is malformed"))
}

pub(crate) async fn with_sequencer_rpc_retry<T, F, Fut>(
    endpoint: &str,
    mut operation: F,
) -> Result<T>
where
    F: FnMut(sequencer_service_rpc::SequencerClient) -> Fut,
    Fut: Future<Output = std::result::Result<T, sequencer_service_rpc::ClientError>>,
{
    let mut last_error = None;
    for attempt in 0..SEQUENCER_RPC_ATTEMPTS {
        let client = sequencer_client(endpoint)?;
        match operation(client).await {
            Ok(value) => return Ok(value),
            Err(error)
                if attempt + 1 < SEQUENCER_RPC_ATTEMPTS && is_retryable_sequencer_error(&error) =>
            {
                last_error = Some(error);
                sleep(SEQUENCER_RPC_RETRY_DELAY).await;
            }
            Err(error) => return Err(error.into()),
        }
    }
    match last_error {
        Some(error) => Err(error.into()),
        None => anyhow::bail!("Sequencer RPC retry loop completed without a result"),
    }
}

fn is_retryable_sequencer_error(error: &sequencer_service_rpc::ClientError) -> bool {
    matches!(
        error,
        sequencer_service_rpc::ClientError::Transport(_)
            | sequencer_service_rpc::ClientError::RequestTimeout
            | sequencer_service_rpc::ClientError::RestartNeeded(_)
            | sequencer_service_rpc::ClientError::ServiceDisconnect
    )
}

fn sequencer_channel_id_from_response(response: &Value) -> Result<String> {
    if response.get("error").is_some() {
        if response.pointer("/error/code").and_then(Value::as_i64) == Some(-32601) {
            return Err(super::evidence_capability_error(
                "Sequencer does not expose Channel identity",
            ));
        }
        return Err(super::evidence_protocol_error(
            "Sequencer Channel identity request returned an RPC error",
        ));
    }
    let Some(channel_id) = response.get("result").and_then(Value::as_str) else {
        return Err(super::evidence_protocol_error(
            "Sequencer Channel identity response is malformed",
        ));
    };
    parse_hash(channel_id, "Sequencer Channel id")
        .map(|channel_id| channel_id.to_string())
        .map_err(|_| {
            super::evidence_protocol_error("Sequencer Channel identity response is malformed")
        })
}

fn list_program_ids_from_response(response: &Value) -> Result<Option<Vec<ProgramId>>> {
    if response.get("error").is_some() {
        if response.pointer("/error/code").and_then(Value::as_i64) == Some(-32601) {
            return Ok(None);
        }
        return Err(super::evidence_protocol_error(
            "Sequencer deployed program request returned an RPC error",
        ));
    }
    let Some(result) = response.get("result") else {
        return Err(super::evidence_protocol_error(
            "Sequencer deployed program response is malformed",
        ));
    };
    let program_ids = serde_json::from_value::<Vec<ProgramId>>(result.clone()).map_err(|_| {
        super::evidence_protocol_error("Sequencer deployed program response is malformed")
    })?;
    if program_ids.windows(2).any(|pair| {
        let [previous, current] = pair else {
            return false;
        };
        previous >= current
    }) {
        return Err(super::evidence_protocol_error(
            "Sequencer deployed program response is not strictly ordered",
        ));
    }
    Ok(Some(program_ids))
}

fn sequencer_block_bytes_from_response(response: &Value) -> Result<Option<Vec<u8>>> {
    if response.get("error").is_some() {
        return Err(super::evidence_protocol_error(
            "Sequencer block request returned an RPC error",
        ));
    }
    let Some(result) = response.get("result") else {
        return Err(super::evidence_protocol_error(
            "Sequencer block response is malformed",
        ));
    };
    if result.is_null() {
        return Ok(None);
    }
    let Some(encoded) = result.as_str() else {
        return Err(super::evidence_protocol_error(
            "Sequencer block response is malformed",
        ));
    };
    BASE64_STANDARD
        .decode(encoded)
        .map(Some)
        .map_err(|_| super::evidence_protocol_error("Sequencer block response is not valid base64"))
}

fn sequencer_block_range_bytes_from_response(response: &Value) -> Result<Vec<Vec<u8>>> {
    if response.get("error").is_some() {
        return Err(super::evidence_protocol_error(
            "Sequencer block range request returned an RPC error",
        ));
    }
    let Some(result) = response.get("result") else {
        return Err(super::evidence_protocol_error(
            "Sequencer block range response is malformed",
        ));
    };
    let Some(rows) = result.as_array() else {
        return Err(super::evidence_protocol_error(
            "Sequencer block range response is malformed",
        ));
    };
    rows.iter()
        .map(|row| {
            let Some(encoded) = row.as_str() else {
                return Err(super::evidence_protocol_error(
                    "Sequencer block range response is malformed",
                ));
            };
            BASE64_STANDARD.decode(encoded).map_err(|_| {
                super::evidence_protocol_error("Sequencer block range contains invalid base64")
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use anyhow::{Result, bail, ensure};

    use super::*;

    #[tokio::test]
    async fn typed_transport_retry_replays_only_transient_failures() -> Result<()> {
        let attempts = Arc::new(AtomicUsize::new(0));
        let result = with_sequencer_rpc_retry("http://127.0.0.1:1", {
            let attempts = Arc::clone(&attempts);
            move |_client| {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt == 0 {
                        Err(sequencer_service_rpc::ClientError::RequestTimeout)
                    } else {
                        Ok(7_u64)
                    }
                }
            }
        })
        .await?;

        ensure!(result == 7, "retry did not return the successful result");
        ensure!(
            attempts.load(Ordering::SeqCst) == 2,
            "transient transport failure was not retried"
        );
        Ok(())
    }

    #[tokio::test]
    async fn typed_protocol_error_is_not_retried() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let result = with_sequencer_rpc_retry::<(), _, _>("http://127.0.0.1:1", {
            let attempts = Arc::clone(&attempts);
            move |_client| {
                attempts.fetch_add(1, Ordering::SeqCst);
                async {
                    Err(sequencer_service_rpc::ClientError::Custom(
                        "protocol failure".to_owned(),
                    ))
                }
            }
        })
        .await;

        assert!(matches!(
            result,
            Err(error) if error.to_string().contains("protocol failure")
        ));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn channel_identity_response_is_canonicalized() -> Result<()> {
        let expected = "ab".repeat(32);
        let response = json!({ "result": format!("0x{}", expected.to_uppercase()) });

        let actual = sequencer_channel_id_from_response(&response)?;

        ensure!(actual == expected, "Channel identity was not canonicalized");
        Ok(())
    }

    #[test]
    fn only_numeric_method_not_found_is_an_unsupported_identity_capability() -> Result<()> {
        let unsupported = identity_response_error(&json!({
            "error": { "code": -32601, "message": "Method not found" }
        }))?;
        ensure!(
            super::super::is_evidence_capability_error(&unsupported),
            "numeric -32601 was not classified as a capability failure"
        );

        for response in [
            json!({ "error": { "code": -32603, "message": "Method not found" } }),
            json!({ "error": { "code": "-32601", "message": "Method not found" } }),
        ] {
            let error = identity_response_error(&response)?;
            ensure!(
                super::super::is_evidence_protocol_error(&error),
                "non-exact method error was not classified as protocol failure"
            );
        }
        Ok(())
    }

    #[test]
    fn malformed_identity_results_are_protocol_failures() -> Result<()> {
        for response in [json!({}), json!({ "result": null }), json!({ "result": 7 })] {
            let error = identity_response_error(&response)?;
            ensure!(
                super::super::is_evidence_protocol_error(&error),
                "malformed identity was not classified as protocol failure"
            );
        }
        Ok(())
    }

    #[test]
    fn deployed_program_response_preserves_canonical_entries() -> Result<()> {
        let response = json!({
            "result": [
                [1, 0, 0, 0, 0, 0, 0, 0],
                [2, 0, 0, 0, 0, 0, 0, 0]
            ]
        });

        let Some(program_ids) = list_program_ids_from_response(&response)? else {
            bail!("deployed program response unexpectedly requested legacy fallback");
        };
        let entries = super::super::programs::program_entries_from_ids(program_ids);

        ensure!(
            entries.len() == 2,
            "unexpected deployed program entry count"
        );
        let Some(first) = entries.first() else {
            bail!("deployed program entries unexpectedly omitted the first program");
        };
        ensure!(
            first.label.is_empty(),
            "deployed program received a legacy label"
        );
        ensure!(
            first.hex == "01000000".to_owned() + &"00".repeat(28),
            "deployed program id did not use canonical little-endian bytes"
        );
        ensure!(
            !first.base58.is_empty(),
            "deployed program id did not produce a Base58 identity"
        );
        Ok(())
    }

    #[test]
    fn only_numeric_method_not_found_falls_back_to_legacy_program_profile() -> Result<()> {
        let fallback = list_program_ids_from_response(&json!({
            "error": { "code": -32601, "message": "Method not found" }
        }))?;
        ensure!(
            fallback.is_none(),
            "numeric -32601 did not request legacy fallback"
        );

        for response in [
            json!({ "error": { "code": -32603, "message": "Method not found" } }),
            json!({ "error": { "code": "-32601", "message": "Method not found" } }),
            json!({ "error": null }),
        ] {
            let error = list_program_response_error(&response)?;
            ensure!(
                super::super::is_evidence_protocol_error(&error),
                "non-exact method error was not classified as a protocol failure"
            );
        }
        Ok(())
    }

    #[test]
    fn malformed_or_unordered_deployed_program_responses_are_protocol_failures() -> Result<()> {
        for response in [
            json!({}),
            json!({ "result": null }),
            json!({ "result": [[1, 2, 3]] }),
            json!({ "result": [[2, 0, 0, 0, 0, 0, 0, 0], [1, 0, 0, 0, 0, 0, 0, 0]] }),
            json!({ "result": [[1, 0, 0, 0, 0, 0, 0, 0], [1, 0, 0, 0, 0, 0, 0, 0]] }),
        ] {
            let error = list_program_response_error(&response)?;
            ensure!(
                super::super::is_evidence_protocol_error(&error),
                "invalid deployed program response was not classified as a protocol failure"
            );
        }
        Ok(())
    }

    #[test]
    fn sequencer_transaction_wire_result_decodes_base64_payload() -> Result<()> {
        let result = json!([
            "ADGfvAVNdyB8uuwLMenMgT7KC0DTENcRLsSg/mQ5XGH8AwAAAC9MRVovQ2xvY2tQcm9ncmFtQWNjb3VudC8wMDAwMDAxL0xFWi9DbG9ja1Byb2dyYW1BY2NvdW50LzAwMDAwMTAvTEVaL0Nsb2NrUHJvZ3JhbUFjY291bnQvMDAwMDA1MAAAAAACAAAAFDfB5p8BAAAAAAAA",
            2416
        ]);

        let transaction = decode_sequencer_transaction_result(&result)?
            .ok_or_else(|| anyhow::anyhow!("transaction unexpectedly missing"))?;
        ensure!(
            hex::encode(transaction.hash())
                == "307f0ee5f458c010b5288e7d261d02fca39ee35faaf70cd6e0f5354f2f3e7402",
            "decoded transaction hash did not match wire payload"
        );
        Ok(())
    }

    #[test]
    fn malformed_sequencer_transaction_wire_results_are_protocol_failures() -> Result<()> {
        for result in [
            json!({}),
            json!(["not-a-transaction"]),
            json!([7, 2416]),
            json!(["not-base64", 2416]),
            json!(["AA==", 2416]),
            json!(["AA==", "2416"]),
        ] {
            let error = match decode_sequencer_transaction_result(&result) {
                Ok(_) => bail!("malformed transaction unexpectedly succeeded"),
                Err(error) => error,
            };
            ensure!(
                super::super::is_evidence_protocol_error(&error),
                "malformed transaction was not classified as protocol failure"
            );
        }
        Ok(())
    }

    #[test]
    fn sequencer_block_range_wire_result_decodes_each_raw_block() -> Result<()> {
        let blocks = sequencer_block_range_bytes_from_response(&json!({
            "result": [BASE64_STANDARD.encode([1_u8, 2, 3]), BASE64_STANDARD.encode([4_u8, 5])]
        }))?;

        ensure!(
            blocks == vec![vec![1, 2, 3], vec![4, 5]],
            "raw block range changed"
        );
        Ok(())
    }

    #[test]
    fn malformed_sequencer_block_range_wire_results_are_protocol_failures() -> Result<()> {
        for response in [
            json!({}),
            json!({ "result": null }),
            json!({ "result": [7] }),
            json!({ "result": ["not-base64"] }),
            json!({ "error": { "code": -32000, "message": "failed" } }),
        ] {
            let error = match sequencer_block_range_bytes_from_response(&response) {
                Ok(_) => bail!("malformed block range unexpectedly succeeded"),
                Err(error) => error,
            };
            ensure!(
                super::super::is_evidence_protocol_error(&error),
                "malformed block range was not classified as protocol failure"
            );
        }
        Ok(())
    }

    fn identity_response_error(response: &Value) -> Result<anyhow::Error> {
        let Err(error) = sequencer_channel_id_from_response(response) else {
            anyhow::bail!("invalid identity response unexpectedly succeeded");
        };
        Ok(error)
    }

    fn list_program_response_error(response: &Value) -> Result<anyhow::Error> {
        let Err(error) = list_program_ids_from_response(response) else {
            bail!("invalid deployed program response unexpectedly succeeded");
        };
        Ok(error)
    }
}
