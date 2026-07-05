mod accounts;
pub mod blockchain;
mod borsh_decode;
pub mod bridge;
pub mod channels;
mod entity_id;
mod idl;
mod idl_interaction;
mod indexer;
pub mod local_indexer;
pub mod logoscore;
pub mod modules;
mod network;
mod overview;
mod probe;
mod programs;
mod rpc;
mod sequencer;
mod settings_backup;
pub mod spel;
mod state_store;
mod transactions;
mod transfers;
mod wallet;

#[cfg(test)]
use anyhow::bail;
use anyhow::{Context as _, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use common::block::{BedrockStatus, Block, BlockBody, BlockHeader};
#[cfg(test)]
use lee::AccountId;
#[cfg(test)]
use lee::PublicKey;
use serde::Serialize;
use serde_json::Value;

#[cfg(test)]
pub(crate) use accounts::summarize_account_transaction;
pub use accounts::{
    AccountReport, AccountTransactionSummary, SequencerAccountIdlReport, account_lookup,
    account_lookup_with_idl, account_transactions_by_account, sequencer_account,
    sequencer_account_with_idl,
};
pub use entity_id::normalize_program_id_hex;
pub(crate) use entity_id::{normalize_account_id_text, parse_account_id, parse_hash};
pub use idl::{
    AccountIdlDecodeReport, DecodedField, EventIdlDecodeReport, InstructionDecodeReport,
    decode_account_data_hex_with_idl, decode_event_data_hex_with_idl, decode_event_data_with_idl,
    decode_instruction_words_with_idl,
};
pub use idl_interaction::{
    LocalWalletInstructionReport, LocalWalletInstructionRequest, ResolvedInstructionAccount,
    ResolvedInstructionArg, local_wallet_instruction_preview, local_wallet_instruction_submit,
};
pub(crate) use indexer::summarize_indexer_transaction;
pub use indexer::{
    IndexerBlockReport, IndexerStatusReport, indexer_block_by_hash, indexer_blocks, indexer_health,
    indexer_status, indexer_transfer_recipients,
};
#[cfg(test)]
pub(crate) use indexer::{
    next_indexer_blocks_cursor, summarize_indexer_block, summarize_indexer_status_response,
};
pub use network::{
    CUSTOM_NETWORK_PROFILE, DEFAULT_INDEXER_ENDPOINT, DEFAULT_NETWORK_PROFILE,
    DEFAULT_NODE_ENDPOINT, DEFAULT_SEQUENCER_ENDPOINT, LOCAL_SEQUENCER_ENDPOINT, NetworkEndpoints,
    NetworkProfile, TESTNET_SEQUENCER_ENDPOINT, infer_network_profile, network_profiles,
    resolve_network_endpoints,
};
pub use overview::{
    InspectorScope, NodeProbe, OverviewReport, ServiceProbe, inspector_scopes, overview,
};
pub use probe::{ProbeField, ProbeReport};
use programs::program_id_base58_from_hex;
pub use programs::{
    ProgramFileInfo, ProgramIdEntry, program_file_info, program_id_base58, program_id_hex,
};
pub use rpc::{
    RawRpcReport, logos_node_cryptarchia_info, raw_http_json, raw_json_rpc,
    raw_json_rpc_optional_result, raw_json_rpc_result, raw_rpc_report,
};
pub(crate) use rpc::{json_rpc_result, response_excerpt};
pub use sequencer::{
    last_sequencer_block_id, sequencer_block, sequencer_blocks, sequencer_health,
    sequencer_program_ids, sequencer_transaction, sequencer_transaction_inspection,
    sequencer_transaction_inspection_with_idl, sequencer_transaction_trace,
    sequencer_transaction_trace_with_idl,
};
pub(crate) use transactions::inspect_transaction;
#[cfg(test)]
pub(crate) use transactions::instruction_word_row;
pub use transactions::{
    TransactionIdlInspectionReport, TransactionInspectionReport, TransactionInspectionRow,
    TransactionInspectionSection, TransactionSummary, TransactionTraceRefs, TransactionTraceReport,
    TransactionTraceStep, inspect_transaction_summary, inspect_transaction_summary_with_idl,
    summarize_transaction, trace_transaction_summary, trace_transaction_summary_with_idl,
};
#[cfg(test)]
pub(crate) use transfers::transfer_recipient_summaries_from_blocks;
pub use transfers::{RecipientTransferSummary, TransferActivityPage, TransferRecipientSummary};
pub use wallet::{
    LOCAL_WALLET_HOME_ENV, LocalWalletAccountRow, LocalWalletAccountsReport,
    LocalWalletCommandReport, LocalWalletDeployReport, LocalWalletProfileStatus,
    LocalWalletSyncPrivateReport, bedrock_wallet_balance, local_wallet_accounts,
    local_wallet_command, local_wallet_create_account, local_wallet_deploy_program,
    local_wallet_profile_status, local_wallet_send_transaction, local_wallet_sync_private,
};

pub const ACCOUNT_TRANSACTION_LIMIT: usize = 20;

#[derive(Debug, Clone, Serialize)]
pub struct BlockSummary {
    pub block_id: u64,
    pub header_hash: String,
    pub parent_hash: String,
    pub timestamp: u64,
    pub bedrock_status: String,
    pub tx_count: usize,
    pub transactions: Vec<TransactionSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decode_warning: Option<String>,
}

pub(crate) fn decode_sequencer_block(encoded: &str) -> Result<BlockSummary> {
    let bytes = BASE64_STANDARD
        .decode(encoded)
        .context("sequencer block result was not valid base64")?;

    let block = borsh::from_slice::<Block>(&bytes)
        .context("sequencer block result did not match LEZ block layout")?;
    Ok(summarize_block(&block))
}

#[must_use]
pub fn summarize_block(block: &Block) -> BlockSummary {
    summarize_block_parts(&block.header, &block.body, &block.bedrock_status, None)
}

#[must_use]
fn summarize_block_parts(
    header: &BlockHeader,
    body: &BlockBody,
    bedrock_status: &BedrockStatus,
    decode_warning: Option<String>,
) -> BlockSummary {
    BlockSummary {
        block_id: header.block_id,
        header_hash: header.hash.to_string(),
        parent_hash: header.prev_block_hash.to_string(),
        timestamp: header.timestamp,
        bedrock_status: format!("{bedrock_status:?}"),
        tx_count: body.transactions.len(),
        transactions: body
            .transactions
            .iter()
            .map(summarize_transaction)
            .collect(),
        decode_warning,
    }
}

pub(crate) fn enum_payload(value: &Value) -> (&str, &Value) {
    if let Some(object) = value.as_object()
        && object.len() == 1
        && let Some((kind, payload)) = object.iter().next()
    {
        return (kind, payload);
    }
    ("Unknown", value)
}

pub(crate) fn value_list_strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items.iter().map(value_to_string).collect(),
        Some(Value::String(value)) => split_list_string(value),
        Some(value) => vec![value_to_string(value)],
        None => Vec::new(),
    }
}

fn split_list_string(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Null => "null".to_owned(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

#[cfg(test)]
mod transaction_idl_tests;

#[cfg(test)]
mod tests {
    use super::*;

    const TESTNET_LEGACY_BLOCK_1234: &str = "0gQAAAAAAADgBr/57T2VP8TvanoE/U28V0Cdzfe66q1YCY203VHHaPZH+D0d+RhX4Qtz8m7atlbEG6J5XguGFqEPUWLQ8+1kb3u3+Z4BAADGt772EW9LB3inITN2BUfOdP8fHmTlcvpFP45NvGI01KYmibPzb/BkLygy6fTsHB4Oc4XoVVMp+k7Rp8xdjpgGAQAAAADiMVjm57Su7ujTA26v18dZ5R2KCU2Ce5JXELoh3v+PRgMAAAAvTEVaL0Nsb2NrUHJvZ3JhbUFjY291bnQvMDAwMDAwMS9MRVovQ2xvY2tQcm9ncmFtQWNjb3VudC8wMDAwMDEwL0xFWi9DbG9ja1Byb2dyYW1BY2NvdW50LzAwMDAwNTAAAAAAAgAAAG97t/meAQAAAAAAAAI=";

    #[test]
    fn decode_sequencer_block_fixture_without_warning() {
        let summary = decode_sequencer_block(TESTNET_LEGACY_BLOCK_1234);

        assert!(summary.is_ok(), "{summary:?}");
        let Ok(summary) = summary else {
            return;
        };
        assert_eq!(summary.block_id, 1234);
        assert_eq!(summary.tx_count, 1);
        assert_eq!(summary.transactions.len(), 1);
        assert_eq!(summary.header_hash.len(), 64);
        assert_eq!(summary.parent_hash.len(), 64);
        assert_eq!(
            summary.transactions.first().map(|tx| tx.kind.as_str()),
            Some("Public")
        );
        assert_eq!(summary.bedrock_status, "Finalized");
        assert!(summary.decode_warning.is_none());
    }

    #[test]
    fn summarize_indexer_transaction_maps_public_payload() {
        let raw = serde_json::json!({
            "Public": {
                "hash": "abcd",
                "message": {
                    "program_id": "program-1",
                    "account_ids": ["acct-a", "acct-b"],
                    "nonces": [1, "2"],
                    "instruction_data": [3, "4"]
                }
            }
        });

        let summary = summarize_indexer_transaction(&raw, 7);

        assert_eq!(summary.index, 7);
        assert_eq!(summary.hash, "abcd");
        assert_eq!(summary.kind, "Public");
        assert_eq!(summary.program_id_hex.as_deref(), Some("program-1"));
        assert_eq!(summary.account_ids, vec!["acct-a", "acct-b"]);
        assert_eq!(summary.nonces, vec!["1", "2"]);
        assert_eq!(summary.instruction_data, vec![3, 4]);
        assert_eq!(summary.raw, raw);
    }

    #[test]
    fn summarize_indexer_transaction_maps_compact_public_payload() {
        let program_id = [1_u32; 8];
        let program_id_base58 = program_id_base58(program_id);
        let program_id_hex = program_id_hex(program_id);
        let raw = serde_json::json!({
            "type": "Public",
            "hash": "tx-public",
            "program_id": program_id_base58,
            "accounts": [
                { "account_id": "acct-a", "nonce": 1 },
                { "account_id": "acct-b", "nonce": "2" }
            ],
            "instruction_data": [3, "4"],
            "signature_count": 2
        });

        let summary = summarize_indexer_transaction(&raw, 2);

        assert_eq!(summary.index, 2);
        assert_eq!(summary.hash, "tx-public");
        assert_eq!(summary.kind, "Public");
        assert_eq!(
            summary.program_id_hex.as_deref(),
            Some(program_id_hex.as_str())
        );
        assert_eq!(summary.account_ids, vec!["acct-a", "acct-b"]);
        assert_eq!(summary.nonces, vec!["1", "2"]);
        assert_eq!(summary.instruction_data, vec![3, 4]);
        assert_eq!(summary.raw, raw);
    }

    #[test]
    fn summarize_indexer_transaction_maps_compact_privacy_payload() {
        let raw = serde_json::json!({
            "type": "PrivacyPreserving",
            "hash": "tx-private",
            "accounts": [
                { "account_id": "acct-a", "nonce": "9" }
            ],
            "new_commitments_count": 3,
            "nullifiers_count": 1,
            "encrypted_states_count": 2,
            "validity_window_start": "10",
            "validity_window_end": "20",
            "signature_count": 1,
            "proof_size": 4096
        });

        let summary = summarize_indexer_transaction(&raw, 0);

        assert_eq!(summary.hash, "tx-private");
        assert_eq!(summary.kind, "PrivacyPreserving");
        assert_eq!(summary.account_ids, vec!["acct-a"]);
        assert_eq!(summary.nonces, vec!["9"]);
        assert!(summary.instruction_data.is_empty());
        assert_eq!(
            summary.raw.get("proof_size").and_then(Value::as_u64),
            Some(4096)
        );
    }

    #[test]
    fn summarize_indexer_transaction_maps_compact_program_deployment_payload() {
        let raw = serde_json::json!({
            "type": "ProgramDeployment",
            "hash": "tx-deploy",
            "bytecode_size": "1234"
        });

        let summary = summarize_indexer_transaction(&raw, 0);

        assert_eq!(summary.hash, "tx-deploy");
        assert_eq!(summary.kind, "ProgramDeployment");
        assert_eq!(summary.bytecode_len, Some(1234));
    }

    #[test]
    fn summarize_indexer_status_response_maps_status_object() {
        let raw = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "state": "syncing",
                "indexedBlockId": "42",
                "lastError": "behind tip"
            }
        });

        let summary = summarize_indexer_status_response(&raw);

        assert_eq!(summary.state, "syncing");
        assert_eq!(summary.indexed_block_id.as_deref(), Some("42"));
        assert_eq!(summary.last_error.as_deref(), Some("behind tip"));
        assert_eq!(summary.raw, raw);
    }

    #[test]
    fn summarize_indexer_status_response_maps_string_result() {
        let raw = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": "caught up"
        });

        let summary = summarize_indexer_status_response(&raw);

        assert_eq!(summary.state, "caught up");
        assert_eq!(summary.indexed_block_id, None);
        assert_eq!(summary.last_error, None);
    }

    #[test]
    fn summarize_indexer_status_response_marks_method_not_found_unavailable() {
        let raw = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32601,
                "message": "Method not found"
            }
        });

        let summary = summarize_indexer_status_response(&raw);

        assert_eq!(summary.state, "unavailable");
        assert_eq!(summary.indexed_block_id, None);
        assert!(
            summary
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("Method not found"))
        );
        assert_eq!(summary.raw, raw);
    }

    #[test]
    fn summarize_account_transaction_marks_signer_outgoing() -> Result<()> {
        let key = lee::PrivateKey::try_new([1_u8; 32]).context("valid private key")?;
        let public_key = PublicKey::new_from_private_key(&key);
        let account_id = AccountId::from(&public_key).to_string();
        let raw = serde_json::json!({
            "Public": {
                "hash": "abcd",
                "message": {
                    "account_ids": [account_id.clone()],
                    "nonces": [1]
                },
                "witness_set": {
                    "signatures_and_public_keys": [[
                        "00".repeat(64),
                        public_key.to_string()
                    ]]
                }
            }
        });

        let summary = summarize_account_transaction(&raw, 0, &account_id);

        if summary.direction.as_deref() != Some("outgoing") {
            bail!("expected outgoing direction, got {:?}", summary.direction);
        }
        Ok(())
    }

    #[test]
    fn summarize_account_transaction_marks_touched_non_signer_incoming() -> Result<()> {
        let key = lee::PrivateKey::try_new([1_u8; 32]).context("valid private key")?;
        let public_key = PublicKey::new_from_private_key(&key);
        let account_id = AccountId::new([7_u8; 32]).to_string();
        let raw = serde_json::json!({
            "Public": {
                "hash": "abcd",
                "message": {
                    "account_ids": [account_id.clone()]
                },
                "witness_set": {
                    "signatures_and_public_keys": [[
                        "00".repeat(64),
                        public_key.to_string()
                    ]]
                }
            }
        });

        let summary = summarize_account_transaction(&raw, 0, &account_id);

        if summary.direction.as_deref() != Some("incoming") {
            bail!("expected incoming direction, got {:?}", summary.direction);
        }
        Ok(())
    }

    #[test]
    fn summarize_account_transaction_marks_compact_account_incoming() -> Result<()> {
        let account_id = AccountId::new([7_u8; 32]).to_string();
        let raw = serde_json::json!({
            "type": "Public",
            "hash": "abcd",
            "accounts": [
                { "account_id": account_id.clone(), "nonce": "4" }
            ]
        });

        let summary = summarize_account_transaction(&raw, 0, &account_id);

        if summary.direction.as_deref() != Some("incoming") {
            bail!("expected incoming direction, got {:?}", summary.direction);
        }
        Ok(())
    }

    #[test]
    fn summarize_indexer_block_maps_header_hash_and_transactions() {
        let header_hash = "ab".repeat(32);
        let parent_hash = "cd".repeat(32);
        let tx_hash = "ef".repeat(32);
        let program_id = [1_u32; 8];
        let program_id_base58 = program_id_base58(program_id);
        let program_id_hex = program_id_hex(program_id);
        let raw = serde_json::json!({
            "header": {
                "block_id": 44,
                "hash": header_hash.clone(),
                "prev_block_hash": parent_hash.clone(),
                "timestamp": 1000
            },
            "body": {
                "transactions": [{
                    "Public": {
                        "hash": tx_hash.clone(),
                        "message": {
                            "program_id": program_id_base58,
                            "account_ids": ["acct-a"],
                            "instruction_data": [1, 2]
                        }
                    }
                }]
            },
            "bedrock_status": "Finalized"
        });

        let summary = summarize_indexer_block(&raw);

        assert_eq!(summary.block_id, Some(44));
        assert_eq!(summary.header_hash.as_deref(), Some(header_hash.as_str()));
        assert_eq!(summary.parent_hash.as_deref(), Some(parent_hash.as_str()));
        assert_eq!(summary.timestamp, Some(1000));
        assert_eq!(summary.bedrock_status.as_deref(), Some("Finalized"));
        assert_eq!(summary.tx_count, 1);
        assert_eq!(
            summary.transactions.first().map(|tx| tx.hash.as_str()),
            Some(tx_hash.as_str())
        );
        assert_eq!(
            summary
                .transactions
                .first()
                .and_then(|tx| tx.program_id_hex.as_deref()),
            Some(program_id_hex.as_str())
        );
        assert_eq!(summary.raw, raw);
    }

    #[test]
    fn summarize_indexer_block_maps_compact_top_level_transactions() {
        let header_hash = "ab".repeat(32);
        let parent_hash = "cd".repeat(32);
        let raw = serde_json::json!({
            "block_id": "45",
            "hash": header_hash.clone(),
            "prev_block_hash": parent_hash.clone(),
            "timestamp": "1001",
            "bedrock_status": "Finalized",
            "transactions": [{
                "type": "Public",
                "hash": "tx-public",
                "accounts": [{ "account_id": "acct-a", "nonce": 1 }],
                "instruction_data": [1, 2]
            }]
        });

        let summary = summarize_indexer_block(&raw);

        assert_eq!(summary.block_id, Some(45));
        assert_eq!(summary.header_hash.as_deref(), Some(header_hash.as_str()));
        assert_eq!(summary.parent_hash.as_deref(), Some(parent_hash.as_str()));
        assert_eq!(summary.timestamp, Some(1001));
        assert_eq!(summary.tx_count, 1);
        assert_eq!(
            summary.transactions.first().map(|tx| tx.kind.as_str()),
            Some("Public")
        );
        assert_eq!(
            summary.transactions.first().map(|tx| tx.hash.as_str()),
            Some("tx-public")
        );
    }

    #[test]
    fn summarize_indexer_block_does_not_treat_bedrock_parent_as_lez_parent() {
        let raw = serde_json::json!({
            "header": {
                "block_id": 44,
                "hash": "ab".repeat(32)
            },
            "bedrock_parent_id": "cd".repeat(32)
        });

        let summary = summarize_indexer_block(&raw);

        assert_eq!(summary.parent_hash, None);
    }

    #[test]
    fn summarize_indexer_transaction_preserves_privacy_public_account_ids() {
        let raw = serde_json::json!({
            "PrivacyPreserving": {
                "hash": "tx-a",
                "message": {
                    "public_account_ids": ["account-111111111111"]
                }
            }
        });

        let summary = summarize_indexer_transaction(&raw, 0);

        assert_eq!(summary.account_ids, vec!["account-111111111111"]);
    }

    #[test]
    fn summarize_indexer_transaction_counts_decoded_bytecode_bytes() {
        let raw = serde_json::json!({
            "ProgramDeployment": {
                "hash": "tx-a",
                "message": {
                    "bytecode": "AQIDBA=="
                }
            }
        });

        let summary = summarize_indexer_transaction(&raw, 0);

        assert_eq!(summary.bytecode_len, Some(4));
    }

    #[test]
    fn transfer_recipient_summaries_prefer_generic_transfer_outputs() {
        let raw = serde_json::json!({
            "Public": {
                "hash": "tx-a",
                "message": {
                    "account_ids": ["account-111111111111"],
                    "outputs": [
                        { "recipient": "aa".repeat(32), "amount": 7 },
                        { "recipient": "aa".repeat(32), "amount": "5" }
                    ]
                }
            }
        });
        let block = IndexerBlockReport {
            block_id: Some(9),
            header_hash: Some("block-a".to_owned()),
            parent_hash: None,
            timestamp: None,
            bedrock_status: None,
            tx_count: 1,
            transactions: vec![summarize_indexer_transaction(&raw, 0)],
            raw: serde_json::json!({}),
        };

        let recipients = transfer_recipient_summaries_from_blocks(&[block]);

        assert_eq!(recipients.len(), 1);
        assert_eq!(
            recipients
                .first()
                .map(|recipient| recipient.recipient.as_str()),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(
            recipients
                .first()
                .and_then(|recipient| recipient.received.as_deref()),
            Some("12")
        );
        assert_eq!(recipients.first().map(|recipient| recipient.txs), Some(1));
        assert_eq!(
            recipients.first().map(|recipient| recipient.outputs),
            Some(2)
        );
        assert_eq!(
            recipients.first().map(|recipient| recipient.references),
            Some(0)
        );
        assert_eq!(
            recipients.first().and_then(|recipient| recipient.last_slot),
            Some(9)
        );
        assert_eq!(
            recipients
                .first()
                .map(|recipient| recipient.transfers.len()),
            Some(2)
        );
        assert_eq!(
            recipients
                .first()
                .and_then(|recipient| recipient.transfers.first())
                .map(|transfer| transfer.tx_hash.as_str()),
            Some("tx-a")
        );
        assert_eq!(
            recipients
                .first()
                .and_then(|recipient| recipient.transfers.first())
                .and_then(|transfer| transfer.block_hash.as_deref()),
            Some("block-a")
        );
        assert_eq!(
            recipients
                .first()
                .map(|recipient| recipient.source.as_str()),
            Some("transfer_outputs")
        );
    }

    #[test]
    fn next_indexer_blocks_cursor_uses_oldest_fetched_block() {
        let blocks = vec![
            IndexerBlockReport {
                block_id: Some(100),
                header_hash: None,
                parent_hash: None,
                timestamp: None,
                bedrock_status: None,
                tx_count: 0,
                transactions: Vec::new(),
                raw: serde_json::json!({}),
            },
            IndexerBlockReport {
                block_id: Some(51),
                header_hash: None,
                parent_hash: None,
                timestamp: None,
                bedrock_status: None,
                tx_count: 0,
                transactions: Vec::new(),
                raw: serde_json::json!({}),
            },
        ];

        assert_eq!(next_indexer_blocks_cursor(&blocks), Some(51));
    }

    #[test]
    fn transfer_recipient_summaries_report_account_refs_not_outputs() {
        let raw = serde_json::json!({
            "Public": {
                "hash": "tx-a",
                "message": {
                    "account_ids": ["account-111111111111", "account-222222222222"]
                }
            }
        });
        let block = IndexerBlockReport {
            block_id: Some(8),
            header_hash: None,
            parent_hash: None,
            timestamp: None,
            bedrock_status: None,
            tx_count: 1,
            transactions: vec![summarize_indexer_transaction(&raw, 0)],
            raw: serde_json::json!({}),
        };

        let recipients = transfer_recipient_summaries_from_blocks(&[block]);

        assert_eq!(recipients.len(), 2);
        assert!(
            recipients
                .iter()
                .all(|recipient| recipient.received.is_none())
        );
        assert!(recipients.iter().all(|recipient| recipient.txs == 1));
        assert!(recipients.iter().all(|recipient| recipient.outputs == 0));
        assert!(recipients.iter().all(|recipient| recipient.references == 1));
        assert!(
            recipients
                .iter()
                .all(|recipient| recipient.transfers.len() == 1)
        );
        assert!(recipients.iter().all(|recipient| {
            recipient
                .transfers
                .first()
                .is_some_and(|transfer| transfer.tx_hash == "tx-a")
        }));
        assert!(
            recipients
                .iter()
                .all(|recipient| recipient.last_slot == Some(8))
        );
        assert!(
            recipients
                .iter()
                .all(|recipient| recipient.source == "account_refs")
        );
    }

    #[test]
    fn account_report_serializes_loaded_empty_related_transactions() {
        let report = AccountReport {
            account_id: "acct-a".to_owned(),
            account_id_base58: "acct-a".to_owned(),
            account_id_hex: "00".to_owned(),
            account: serde_json::json!({}),
            balance: "0".to_owned(),
            nonce: "0".to_owned(),
            owner_base58: "owner".to_owned(),
            owner_hex: "00".to_owned(),
            data_hex: String::new(),
            related_transactions: Some(Vec::new()),
            related_transactions_error: None,
        };

        let value = serde_json::to_value(report);

        assert!(value.is_ok(), "{value:?}");
        let Ok(value) = value else {
            return;
        };
        assert_eq!(
            value.get("related_transactions"),
            Some(&serde_json::json!([]))
        );
    }

    #[test]
    fn instruction_word_row_includes_index_decimal_and_hex() {
        let row = instruction_word_row(2, 255);

        assert_eq!(row.label, "instruction_word");
        assert_eq!(row.index, Some(2));
        assert_eq!(row.value, "255");
        assert_eq!(row.decimal.as_deref(), Some("255"));
        assert_eq!(row.hex.as_deref(), Some("0x000000ff"));
        assert_eq!(row.base58, None);
    }

    #[test]
    fn inspect_transaction_summary_builds_human_sections() {
        let summary = TransactionSummary {
            hash: "abcd1234".to_owned(),
            kind: "Public".to_owned(),
            program_id_hex: Some(
                "0100000002000000030000000400000005000000060000000700000008000000".to_owned(),
            ),
            account_ids: vec!["acct-a".to_owned(), "acct-b".to_owned()],
            nonces: vec!["9".to_owned(), "10".to_owned()],
            instruction_data: vec![7, 255],
            bytecode_len: Some(42),
            raw_signature_valid: Some(true),
            message_prehash: Some("feedbeef".to_owned()),
            prehash_signature_valid: Some(false),
        };

        let report = inspect_transaction_summary(&summary);
        assert_eq!(report.hash, summary.hash);
        assert_eq!(report.kind, summary.kind);
        assert_eq!(report.sections.len(), 6);

        let program_section = report
            .sections
            .iter()
            .find(|section| section.title == "Program");
        assert!(program_section.is_some(), "missing Program section");
        let Some(program_section) = program_section else {
            return;
        };
        let program_row = program_section
            .rows
            .iter()
            .find(|row| row.label == "program_id");
        assert!(program_row.is_some(), "missing program_id row");
        let Some(program_row) = program_row else {
            return;
        };
        assert_eq!(
            program_row.hex.as_deref(),
            summary.program_id_hex.as_deref()
        );
        assert!(program_row.base58.is_some());

        let instruction_section = report
            .sections
            .iter()
            .find(|section| section.title == "Instruction words");
        assert!(
            instruction_section.is_some(),
            "missing Instruction words section"
        );
        let Some(instruction_section) = instruction_section else {
            return;
        };
        let instruction_row = instruction_section.rows.get(1);
        assert!(instruction_row.is_some(), "missing instruction row 1");
        let Some(instruction_row) = instruction_row else {
            return;
        };
        assert_eq!(instruction_row.index, Some(1));
        assert_eq!(instruction_row.decimal.as_deref(), Some("255"));
        assert_eq!(instruction_row.hex.as_deref(), Some("0x000000ff"));

        let validation_section = report
            .sections
            .iter()
            .find(|section| section.title == "Validation");
        assert!(validation_section.is_some(), "missing Validation section");
        let Some(validation_section) = validation_section else {
            return;
        };
        let raw_signature_row = validation_section.rows.first();
        assert!(
            raw_signature_row.is_some(),
            "missing raw signature validation row"
        );
        let Some(raw_signature_row) = raw_signature_row else {
            return;
        };
        let prehash_signature_row = validation_section.rows.get(2);
        assert!(
            prehash_signature_row.is_some(),
            "missing prehash signature validation row"
        );
        let Some(prehash_signature_row) = prehash_signature_row else {
            return;
        };
        assert_eq!(raw_signature_row.value, "valid");
        assert_eq!(prehash_signature_row.value, "invalid");
    }

    #[test]
    fn trace_transaction_summary_builds_public_validation_timeline() {
        // Arrange
        let summary = TransactionSummary {
            hash: "abcd1234".to_owned(),
            kind: "Public".to_owned(),
            program_id_hex: Some(
                "0100000002000000030000000400000005000000060000000700000008000000".to_owned(),
            ),
            account_ids: vec!["acct-a".to_owned(), "acct-b".to_owned()],
            nonces: vec!["9".to_owned()],
            instruction_data: vec![7, 255],
            bytecode_len: None,
            raw_signature_valid: Some(true),
            message_prehash: Some("feedbeef".to_owned()),
            prehash_signature_valid: Some(false),
        };

        // Act
        let report = trace_transaction_summary(&summary);

        // Assert
        assert_eq!(report.hash, summary.hash);
        assert_eq!(report.kind, summary.kind);
        assert_eq!(report.source, "sequencer transaction summary");
        assert!(
            report
                .limitations
                .iter()
                .any(|item| item.contains("no runtime execution trace")),
            "{report:?}"
        );

        let raw_validation = report
            .steps
            .iter()
            .find(|step| step.label == "raw witness validation");
        assert!(raw_validation.is_some(), "missing raw validation step");
        let Some(raw_validation) = raw_validation else {
            return;
        };
        assert_eq!(raw_validation.phase, "validation");
        assert_eq!(raw_validation.status.as_deref(), Some("valid"));

        let public_program = report
            .steps
            .iter()
            .find(|step| step.label == "public program invocation");
        assert!(public_program.is_some(), "missing public program step");
        let Some(public_program) = public_program else {
            return;
        };
        assert_eq!(
            public_program
                .refs
                .as_ref()
                .and_then(|refs| refs.program_id_hex.as_deref()),
            summary.program_id_hex.as_deref()
        );

        let account_step = report
            .steps
            .iter()
            .find(|step| step.label == "account reference");
        assert!(account_step.is_some(), "missing account reference step");
        let Some(account_step) = account_step else {
            return;
        };
        assert_eq!(
            account_step
                .refs
                .as_ref()
                .and_then(|refs| refs.account_id.as_deref()),
            Some("acct-a")
        );

        let invalid_prehash = report
            .steps
            .iter()
            .find(|step| step.label == "prehash witness validation");
        assert!(invalid_prehash.is_some(), "missing prehash validation step");
        let Some(invalid_prehash) = invalid_prehash else {
            return;
        };
        assert_eq!(invalid_prehash.status.as_deref(), Some("invalid"));
        assert_eq!(invalid_prehash.severity.as_deref(), Some("warning"));
    }

    #[test]
    fn trace_transaction_summary_builds_program_deployment_step() {
        // Arrange
        let summary = TransactionSummary {
            hash: "deploy1234".to_owned(),
            kind: "ProgramDeployment".to_owned(),
            program_id_hex: Some(
                "0100000002000000030000000400000005000000060000000700000008000000".to_owned(),
            ),
            account_ids: vec![],
            nonces: vec![],
            instruction_data: vec![],
            bytecode_len: Some(42),
            raw_signature_valid: None,
            message_prehash: None,
            prehash_signature_valid: None,
        };

        // Act
        let report = trace_transaction_summary(&summary);

        // Assert
        let deployment = report
            .steps
            .iter()
            .find(|step| step.label == "program deployment payload");
        assert!(deployment.is_some(), "missing deployment step");
        let Some(deployment) = deployment else {
            return;
        };
        assert_eq!(deployment.phase, "program");
        assert!(
            deployment
                .details
                .iter()
                .any(|detail| detail.contains("bytecode length 42 bytes")),
            "{deployment:?}"
        );
        assert_eq!(
            deployment
                .refs
                .as_ref()
                .and_then(|refs| refs.program_id_hex.as_deref()),
            summary.program_id_hex.as_deref()
        );
    }
}
