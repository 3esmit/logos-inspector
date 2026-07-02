pub mod blockchain;
pub mod bridge;
pub mod channels;
pub mod local_indexer;
pub mod logoscore;
pub mod modules;
mod probe;
pub mod spel;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use anyhow::{Context as _, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use common::{
    HashType,
    block::{BedrockStatus, Block, BlockBody, BlockHeader},
    transaction::LeeTransaction,
};
use k256::ecdsa::signature::hazmat::PrehashVerifier as _;
use lee::{
    AccountId, ProgramDeploymentTransaction, PublicKey, program::Program,
    program_deployment_transaction::Message, public_transaction::Message as PublicMessage,
};
use lee_core::program::ProgramId;
use sequencer_service_rpc::{RpcClient as _, SequencerClientBuilder};
use serde::Serialize;
use serde_json::{Map, Value, json};
use sha2::{Digest as _, Sha256};

pub use probe::ProbeReport;

pub const TESTNET_SEQUENCER_ENDPOINT: &str = "https://testnet.lez.logos.co/";
pub const LOCAL_SEQUENCER_ENDPOINT: &str = "http://127.0.0.1:3040/";
pub const DEFAULT_SEQUENCER_ENDPOINT: &str = TESTNET_SEQUENCER_ENDPOINT;
pub const DEFAULT_INDEXER_ENDPOINT: &str = "http://127.0.0.1:8779/";
pub const DEFAULT_NODE_ENDPOINT: &str = "http://127.0.0.1:8080/";
pub const DEFAULT_NETWORK_PROFILE: &str = "default";
pub const LOCAL_NODE_NETWORK_PROFILE: &str = "local-node";
pub const CUSTOM_NETWORK_PROFILE: &str = "custom";
pub const ACCOUNT_TRANSACTION_LIMIT: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct NetworkProfile {
    pub id: &'static str,
    pub label: &'static str,
    pub sequencer_endpoint: &'static str,
    pub indexer_endpoint: &'static str,
    pub node_endpoint: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NetworkEndpoints {
    pub profile: String,
    pub sequencer_endpoint: String,
    pub indexer_endpoint: String,
    pub node_endpoint: String,
}

const NETWORK_PROFILES: &[NetworkProfile] = &[
    NetworkProfile {
        id: DEFAULT_NETWORK_PROFILE,
        label: "Testnet",
        sequencer_endpoint: DEFAULT_SEQUENCER_ENDPOINT,
        indexer_endpoint: DEFAULT_INDEXER_ENDPOINT,
        node_endpoint: DEFAULT_NODE_ENDPOINT,
    },
    NetworkProfile {
        id: "testnet-indexer-local",
        label: "Testnet + local indexer",
        sequencer_endpoint: DEFAULT_SEQUENCER_ENDPOINT,
        indexer_endpoint: DEFAULT_INDEXER_ENDPOINT,
        node_endpoint: DEFAULT_NODE_ENDPOINT,
    },
    NetworkProfile {
        id: LOCAL_NODE_NETWORK_PROFILE,
        label: "Local Logos node",
        sequencer_endpoint: DEFAULT_SEQUENCER_ENDPOINT,
        indexer_endpoint: DEFAULT_INDEXER_ENDPOINT,
        node_endpoint: DEFAULT_NODE_ENDPOINT,
    },
    NetworkProfile {
        id: "local",
        label: "Local sequencer",
        sequencer_endpoint: LOCAL_SEQUENCER_ENDPOINT,
        indexer_endpoint: DEFAULT_INDEXER_ENDPOINT,
        node_endpoint: DEFAULT_NODE_ENDPOINT,
    },
];

#[must_use]
pub const fn network_profiles() -> &'static [NetworkProfile] {
    NETWORK_PROFILES
}

pub fn resolve_network_endpoints(
    profile_id: Option<&str>,
    sequencer_url: Option<&str>,
    indexer_url: Option<&str>,
    node_url: Option<&str>,
) -> Result<NetworkEndpoints> {
    let selected_profile = profile_id
        .map(str::trim)
        .filter(|profile_id| !profile_id.is_empty())
        .unwrap_or(DEFAULT_NETWORK_PROFILE);
    let base = if selected_profile == CUSTOM_NETWORK_PROFILE {
        default_network_profile()
    } else {
        network_profile(selected_profile)?
    };

    let sequencer_endpoint = sequencer_url.unwrap_or(base.sequencer_endpoint).to_owned();
    let indexer_endpoint = indexer_url.unwrap_or(base.indexer_endpoint).to_owned();
    let node_endpoint = node_url.unwrap_or(base.node_endpoint).to_owned();
    let has_overrides = sequencer_url.is_some() || indexer_url.is_some() || node_url.is_some();
    let profile = if has_overrides {
        if selected_profile != CUSTOM_NETWORK_PROFILE
            && sequencer_endpoint == base.sequencer_endpoint
            && indexer_endpoint == base.indexer_endpoint
            && node_endpoint == base.node_endpoint
        {
            selected_profile.to_owned()
        } else {
            infer_network_profile(&sequencer_endpoint, &indexer_endpoint, &node_endpoint)
                .unwrap_or(CUSTOM_NETWORK_PROFILE)
                .to_owned()
        }
    } else {
        selected_profile.to_owned()
    };

    Ok(NetworkEndpoints {
        profile,
        sequencer_endpoint,
        indexer_endpoint,
        node_endpoint,
    })
}

#[must_use]
pub fn infer_network_profile(
    sequencer_endpoint: &str,
    indexer_endpoint: &str,
    node_endpoint: &str,
) -> Option<&'static str> {
    NETWORK_PROFILES
        .iter()
        .find(|profile| {
            profile.sequencer_endpoint == sequencer_endpoint
                && profile.indexer_endpoint == indexer_endpoint
                && profile.node_endpoint == node_endpoint
        })
        .map(|profile| profile.id)
}

fn network_profile(profile_id: &str) -> Result<NetworkProfile> {
    NETWORK_PROFILES
        .iter()
        .copied()
        .find(|profile| profile.id == profile_id)
        .with_context(|| {
            let mut available = NETWORK_PROFILES
                .iter()
                .map(|profile| profile.id)
                .collect::<Vec<_>>();
            available.push(CUSTOM_NETWORK_PROFILE);
            let available = available.join(", ");
            format!("unknown network profile `{profile_id}`; available profiles: {available}")
        })
}

fn default_network_profile() -> NetworkProfile {
    NetworkProfile {
        id: DEFAULT_NETWORK_PROFILE,
        label: "Testnet",
        sequencer_endpoint: DEFAULT_SEQUENCER_ENDPOINT,
        indexer_endpoint: DEFAULT_INDEXER_ENDPOINT,
        node_endpoint: DEFAULT_NODE_ENDPOINT,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct InspectorScope {
    pub name: &'static str,
    pub area: &'static str,
    pub status: &'static str,
}

#[must_use]
pub fn inspector_scopes() -> Vec<InspectorScope> {
    vec![
        InspectorScope {
            name: "Logos Blockchain",
            area: "base chain, blocks, transactions, services",
            status: "active",
        },
        InspectorScope {
            name: "Logos Execution Zone",
            area: "LEZ sequencer, indexer, accounts, programs",
            status: "active",
        },
        InspectorScope {
            name: "Logos Messaging",
            area: "message transport and routing inspection",
            status: "active",
        },
        InspectorScope {
            name: "Logos Storage",
            area: "storage node and content inspection",
            status: "active",
        },
    ]
}

#[derive(Debug, Clone, Serialize)]
pub struct ProbeField {
    pub ok: bool,
    pub value: Option<Value>,
    pub error: Option<String>,
}

impl ProbeField {
    fn ok(value: impl Serialize) -> Self {
        Self {
            ok: true,
            value: Some(serde_json::to_value(value).unwrap_or(Value::Null)),
            error: None,
        }
    }

    fn err(error: impl std::fmt::Display) -> Self {
        Self {
            ok: false,
            value: None,
            error: Some(error.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceProbe {
    pub endpoint: String,
    pub health: ProbeField,
    pub head: ProbeField,
    pub programs: Option<ProbeField>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeProbe {
    pub endpoint: String,
    pub consensus: ProbeField,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverviewReport {
    pub product: &'static str,
    pub scopes: Vec<InspectorScope>,
    pub node: NodeProbe,
    pub sequencer: ServiceProbe,
    pub indexer: ServiceProbe,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgramIdEntry {
    pub label: String,
    pub base58: String,
    pub hex: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransactionSummary {
    pub hash: String,
    pub kind: String,
    pub program_id_hex: Option<String>,
    pub account_ids: Vec<String>,
    pub nonces: Vec<String>,
    pub instruction_data: Vec<u32>,
    pub bytecode_len: Option<usize>,
    pub raw_signature_valid: Option<bool>,
    pub message_prehash: Option<String>,
    pub prehash_signature_valid: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransactionInspectionReport {
    pub hash: String,
    pub kind: String,
    pub sections: Vec<TransactionInspectionSection>,
    pub raw_summary: TransactionSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransactionIdlInspectionReport {
    pub inspection: TransactionInspectionReport,
    pub decoded_instruction: Option<InstructionDecodeReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransactionTraceReport {
    pub hash: String,
    pub kind: String,
    pub source: String,
    pub capabilities: Vec<String>,
    pub limitations: Vec<String>,
    pub steps: Vec<TransactionTraceStep>,
    pub inspection: TransactionInspectionReport,
    pub decoded_instruction: Option<InstructionDecodeReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransactionTraceStep {
    pub index: usize,
    pub phase: String,
    pub label: String,
    pub status: Option<String>,
    pub severity: Option<String>,
    pub details: Vec<String>,
    pub refs: Option<TransactionTraceRefs>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct TransactionTraceRefs {
    pub program_id_hex: Option<String>,
    pub program_id_base58: Option<String>,
    pub account_id: Option<String>,
    pub instruction_word_index: Option<usize>,
    pub decode_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransactionInspectionSection {
    pub title: String,
    pub rows: Vec<TransactionInspectionRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransactionInspectionRow {
    pub label: String,
    pub index: Option<usize>,
    pub value: String,
    pub decimal: Option<String>,
    pub hex: Option<String>,
    pub base58: Option<String>,
}

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

#[derive(Debug, Clone, Serialize)]
pub struct AccountReport {
    pub account_id: String,
    pub account: Value,
    pub data_hex: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_transactions: Option<Vec<AccountTransactionSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_transactions_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountTransactionSummary {
    pub index: usize,
    pub hash: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program_id_hex: Option<String>,
    pub account_ids: Vec<String>,
    pub nonces: Vec<String>,
    pub instruction_data: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytecode_len: Option<usize>,
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexerBlockReport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bedrock_status: Option<String>,
    pub tx_count: usize,
    pub transactions: Vec<AccountTransactionSummary>,
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct WalletSummary {
    pub wallet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub received: Option<String>,
    pub txs: usize,
    pub outputs: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_slot: Option<u64>,
    pub source: String,
    pub transfers: Vec<WalletTransferSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WalletTransferSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<u64>,
    pub tx_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DecodedField {
    pub path: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountIdlDecodeReport {
    pub account_id: Option<String>,
    pub account_type: String,
    pub consumed_bytes: usize,
    pub total_bytes: usize,
    pub remaining_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_data_hex: Option<String>,
    pub decoded: Value,
    pub rows: Vec<DecodedField>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SequencerAccountIdlReport {
    pub account: AccountReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decode: Option<AccountIdlDecodeReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decode_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventIdlDecodeReport {
    pub event: String,
    pub consumed_bytes: usize,
    pub total_bytes: usize,
    pub decoded: Value,
    pub rows: Vec<DecodedField>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstructionDecodeReport {
    pub program_id: String,
    pub idl_name: Option<String>,
    pub instruction: String,
    pub variant_index: u32,
    pub accounts: Vec<DecodedField>,
    pub args: Vec<DecodedField>,
    pub remaining_words: Vec<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgramFileInfo {
    pub path: String,
    pub bytecode_len: usize,
    pub program_id_hex: String,
    pub program_id_base58: String,
    pub deployment_tx_hash: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RawRpcReport {
    pub endpoint: String,
    pub method: String,
    pub response: Value,
}

pub async fn overview(
    sequencer_endpoint: &str,
    indexer_endpoint: &str,
    node_endpoint: &str,
) -> OverviewReport {
    let node = NodeProbe {
        endpoint: node_endpoint.to_owned(),
        consensus: match logos_node_cryptarchia_info(node_endpoint).await {
            Ok(value) => ProbeField::ok(value),
            Err(err) => ProbeField::err(err),
        },
    };

    let sequencer = ServiceProbe {
        endpoint: sequencer_endpoint.to_owned(),
        health: match sequencer_health(sequencer_endpoint).await {
            Ok(()) => ProbeField::ok("ok"),
            Err(err) => ProbeField::err(err),
        },
        head: match last_sequencer_block_id(sequencer_endpoint).await {
            Ok(head) => ProbeField::ok(head),
            Err(err) => ProbeField::err(err),
        },
        programs: Some(match sequencer_program_ids(sequencer_endpoint).await {
            Ok(programs) => ProbeField::ok(programs.len()),
            Err(err) => ProbeField::err(err),
        }),
    };

    let indexer_head = match raw_json_rpc(
        indexer_endpoint,
        "getLastFinalizedBlockId",
        Value::Array(vec![]),
    )
    .await
    {
        Ok(value) => ProbeField::ok(value),
        Err(err) => ProbeField::err(err),
    };
    let indexer_health = if indexer_head.ok {
        ProbeField::ok("reachable")
    } else {
        ProbeField {
            ok: false,
            value: None,
            error: indexer_head.error.clone(),
        }
    };

    let indexer = ServiceProbe {
        endpoint: indexer_endpoint.to_owned(),
        health: indexer_health,
        head: indexer_head,
        programs: None,
    };

    OverviewReport {
        product: "Logos Inspector",
        scopes: inspector_scopes(),
        node,
        sequencer,
        indexer,
    }
}

pub async fn sequencer_health(endpoint: &str) -> Result<()> {
    sequencer_client(endpoint)?
        .check_health()
        .await
        .context("sequencer health check failed")
}

pub async fn last_sequencer_block_id(endpoint: &str) -> Result<u64> {
    sequencer_client(endpoint)?
        .get_last_block_id()
        .await
        .context("failed to fetch last sequencer block id")
}

pub async fn sequencer_program_ids(endpoint: &str) -> Result<Vec<ProgramIdEntry>> {
    let programs = sequencer_client(endpoint)?
        .get_program_ids()
        .await
        .context("failed to fetch sequencer program ids")?;
    Ok(program_entries(programs))
}

pub async fn sequencer_block(endpoint: &str, block_id: u64) -> Result<Option<BlockSummary>> {
    let response = raw_json_rpc(endpoint, "getBlock", Value::Array(vec![json!(block_id)]))
        .await
        .with_context(|| format!("failed to fetch sequencer block {block_id}"))?;
    let Some(result) = json_rpc_result(&response, "getBlock")? else {
        return Ok(None);
    };
    let encoded = result
        .as_str()
        .context("sequencer getBlock result was not a base64 block")?;
    let block = decode_sequencer_block(encoded)
        .with_context(|| format!("failed to decode sequencer block {block_id}"))?;
    Ok(Some(block))
}

pub async fn indexer_block_by_hash(
    endpoint: &str,
    header_hash: &str,
) -> Result<Option<IndexerBlockReport>> {
    let parsed_hash = parse_hash(header_hash, "block header hash")?;
    let response = raw_json_rpc(endpoint, "getBlockByHash", json!([parsed_hash.to_string()]))
        .await
        .with_context(|| format!("failed to fetch indexer block {}", parsed_hash))?;
    let Some(result) = json_rpc_result(&response, "getBlockByHash")? else {
        return Ok(None);
    };
    Ok(Some(summarize_indexer_block(result)))
}

pub async fn indexer_blocks(
    endpoint: &str,
    before: Option<u64>,
    limit: u64,
) -> Result<Vec<IndexerBlockReport>> {
    let before = before.map_or(Value::Null, |block_id| json!(block_id));
    let response = raw_json_rpc(endpoint, "getBlocks", json!([before, limit]))
        .await
        .context("failed to fetch indexer blocks")?;
    let Some(result) = json_rpc_result(&response, "getBlocks")? else {
        return Ok(Vec::new());
    };
    let blocks = result
        .as_array()
        .context("getBlocks result was not an array")?;
    Ok(blocks.iter().map(summarize_indexer_block).collect())
}

pub async fn indexer_wallets(
    endpoint: &str,
    before: Option<u64>,
    limit: u64,
) -> Result<Vec<WalletSummary>> {
    let blocks = indexer_blocks(endpoint, before, limit).await?;
    Ok(wallet_summaries_from_blocks(&blocks))
}

pub async fn sequencer_transaction(
    endpoint: &str,
    tx_hash: &str,
) -> Result<Option<TransactionSummary>> {
    let tx = fetch_sequencer_transaction(endpoint, tx_hash).await?;
    Ok(tx.as_ref().map(summarize_transaction))
}

pub async fn sequencer_transaction_inspection(
    endpoint: &str,
    tx_hash: &str,
) -> Result<Option<TransactionInspectionReport>> {
    let tx = fetch_sequencer_transaction(endpoint, tx_hash).await?;
    Ok(tx.as_ref().map(inspect_transaction))
}

pub async fn sequencer_transaction_inspection_with_idl(
    endpoint: &str,
    tx_hash: &str,
    idl_json: &str,
) -> Result<Option<TransactionIdlInspectionReport>> {
    let tx = fetch_sequencer_transaction(endpoint, tx_hash).await?;
    tx.as_ref()
        .map(|tx| inspect_transaction_summary_with_idl(&summarize_transaction(tx), idl_json))
        .transpose()
}

pub async fn sequencer_transaction_trace(
    endpoint: &str,
    tx_hash: &str,
) -> Result<Option<TransactionTraceReport>> {
    let tx = fetch_sequencer_transaction(endpoint, tx_hash).await?;
    Ok(tx
        .as_ref()
        .map(|tx| trace_transaction_summary(&summarize_transaction(tx))))
}

pub async fn sequencer_transaction_trace_with_idl(
    endpoint: &str,
    tx_hash: &str,
    idl_json: &str,
) -> Result<Option<TransactionTraceReport>> {
    let tx = fetch_sequencer_transaction(endpoint, tx_hash).await?;
    tx.as_ref()
        .map(|tx| trace_transaction_summary_with_idl(&summarize_transaction(tx), idl_json))
        .transpose()
}

pub async fn sequencer_account(endpoint: &str, account_id: &str) -> Result<AccountReport> {
    let parsed_account_id = parse_account_id(account_id)?;
    let account = sequencer_client(endpoint)?
        .get_account(parsed_account_id)
        .await
        .with_context(|| format!("failed to fetch sequencer account {account_id}"))?;
    let account_json = serde_json::to_value(&account).context("failed to serialize account")?;
    let data_hex = hex::encode(account.data.into_inner());
    Ok(AccountReport {
        account_id: account_id.to_owned(),
        account: account_json,
        data_hex,
        related_transactions: None,
        related_transactions_error: None,
    })
}

pub async fn account_lookup(
    sequencer_endpoint: &str,
    indexer_endpoint: &str,
    account_id: &str,
) -> Result<AccountReport> {
    let mut account = sequencer_account(sequencer_endpoint, account_id).await?;
    match account_transactions_by_account(
        indexer_endpoint,
        account_id,
        0,
        ACCOUNT_TRANSACTION_LIMIT,
    )
    .await
    {
        Ok(transactions) => {
            account.related_transactions = Some(transactions);
        }
        Err(error) => {
            account.related_transactions = Some(Vec::new());
            account.related_transactions_error = Some(format!("{error:#}"));
        }
    }
    Ok(account)
}

pub async fn sequencer_account_with_idl(
    endpoint: &str,
    account_id: &str,
    idl_json: &str,
    account_type: Option<&str>,
) -> Result<SequencerAccountIdlReport> {
    let account = sequencer_account(endpoint, account_id).await?;
    Ok(account_report_with_optional_idl_decode(
        account,
        idl_json,
        account_type,
    ))
}

pub async fn account_lookup_with_idl(
    sequencer_endpoint: &str,
    indexer_endpoint: &str,
    account_id: &str,
    idl_json: &str,
    account_type: Option<&str>,
) -> Result<SequencerAccountIdlReport> {
    let account = account_lookup(sequencer_endpoint, indexer_endpoint, account_id).await?;
    Ok(account_report_with_optional_idl_decode(
        account,
        idl_json,
        account_type,
    ))
}

fn account_report_with_optional_idl_decode(
    account: AccountReport,
    idl_json: &str,
    account_type: Option<&str>,
) -> SequencerAccountIdlReport {
    let decode = decode_account_data_hex_with_idl(
        idl_json,
        account_type,
        &account.data_hex,
        Some(&account.account_id),
    );
    match decode {
        Ok(decode) => SequencerAccountIdlReport {
            account,
            decode: Some(decode),
            decode_error: None,
        },
        Err(error) => SequencerAccountIdlReport {
            account,
            decode: None,
            decode_error: Some(format!("{error:#}")),
        },
    }
}

pub async fn account_transactions_by_account(
    indexer_endpoint: &str,
    account_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<AccountTransactionSummary>> {
    parse_account_id(account_id)?;
    let response = raw_json_rpc(
        indexer_endpoint,
        "getTransactionsByAccount",
        json!([account_id, offset, limit]),
    )
    .await
    .with_context(|| format!("failed to fetch transactions for account {account_id}"))?;
    let Some(result) = json_rpc_result(&response, "getTransactionsByAccount")? else {
        return Ok(Vec::new());
    };
    let transactions = result
        .as_array()
        .context("getTransactionsByAccount result was not an array")?;
    Ok(transactions
        .iter()
        .enumerate()
        .map(|(index, transaction)| summarize_indexer_transaction(transaction, offset + index))
        .collect())
}

pub fn decode_account_data_hex_with_idl(
    idl_json: &str,
    account_type: Option<&str>,
    data_hex: &str,
    account_id: Option<&str>,
) -> Result<AccountIdlDecodeReport> {
    let idl: Value = serde_json::from_str(idl_json).context("failed to parse IDL JSON")?;
    let bytes = parse_hex_bytes(data_hex).context("failed to parse account data hex")?;
    let accounts = idl
        .get("accounts")
        .and_then(Value::as_array)
        .context("IDL has no accounts array")?;
    let selected = account_type
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut attempted = Vec::new();
    let mut best_partial: Option<(String, DecodedValue)> = None;

    for account in accounts {
        let name = account
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if selected.is_some_and(|selected| selected != name) {
            continue;
        }

        let Some(shape) = account.get("type") else {
            attempted.push(format!("{name}: missing type"));
            continue;
        };

        match decode_borsh_shape(shape, &bytes, 0, &idl, 0) {
            Ok(decoded) if decoded.consumed == bytes.len() => {
                return Ok(account_idl_decode_report(account_id, name, decoded, &bytes));
            }
            Ok(decoded) if selected.is_some() => {
                return Ok(account_idl_decode_report(account_id, name, decoded, &bytes));
            }
            Ok(decoded) => {
                if best_partial
                    .as_ref()
                    .is_none_or(|(_, best)| decoded.consumed > best.consumed)
                {
                    best_partial = Some((name.to_owned(), decoded));
                } else {
                    attempted.push(format!(
                        "{name}: decoded {} of {} bytes",
                        decoded.consumed,
                        bytes.len()
                    ));
                }
            }
            Err(err) if selected.is_some() => {
                return Err(err).with_context(|| format!("failed to decode as `{name}`"));
            }
            Err(err) => attempted.push(format!("{name}: {err:#}")),
        }
    }

    if let Some((name, decoded)) = best_partial {
        return Ok(account_idl_decode_report(
            account_id, &name, decoded, &bytes,
        ));
    }

    if let Some(selected) = selected {
        bail!("IDL account `{selected}` not found");
    }

    bail!(
        "no IDL account shape decoded the data: {}",
        attempted.join("; ")
    )
}

fn account_idl_decode_report(
    account_id: Option<&str>,
    account_type: &str,
    decoded: DecodedValue,
    bytes: &[u8],
) -> AccountIdlDecodeReport {
    let remaining = bytes.get(decoded.consumed..).unwrap_or_default();
    let remaining_data_hex = (!remaining.is_empty()).then(|| hex::encode(remaining));
    let mut rows = Vec::new();
    flatten_decoded_value(&decoded.value, "", &mut rows);
    if let Some(remaining_data_hex) = &remaining_data_hex {
        rows.push(DecodedField {
            path: "remaining_data_hex".to_owned(),
            value: remaining_data_hex.clone(),
        });
    }
    AccountIdlDecodeReport {
        account_id: account_id.map(ToOwned::to_owned),
        account_type: account_type.to_owned(),
        consumed_bytes: decoded.consumed,
        total_bytes: bytes.len(),
        remaining_bytes: remaining.len(),
        remaining_data_hex,
        decoded: decoded.value,
        rows,
    }
}

pub fn decode_event_data_hex_with_idl(
    idl_json: &str,
    event_name: Option<&str>,
    data_hex: &str,
) -> Result<EventIdlDecodeReport> {
    let bytes = parse_hex_bytes(data_hex).context("failed to parse event data hex")?;
    decode_event_data_with_idl(idl_json, event_name, &bytes)
}

pub fn decode_event_data_with_idl(
    idl_json: &str,
    event_name: Option<&str>,
    data: &[u8],
) -> Result<EventIdlDecodeReport> {
    let idl: Value = serde_json::from_str(idl_json).context("failed to parse IDL JSON")?;
    let event = select_idl_event(&idl, event_name)?;
    let event = decode_idl_event(&idl, event, data)?;
    Ok(event)
}

pub fn decode_instruction_words_with_idl(
    idl_json: &str,
    program_id: &str,
    instruction_words: &[u32],
    account_ids: &[String],
) -> Result<InstructionDecodeReport> {
    let idl: Value = serde_json::from_str(idl_json).context("failed to parse IDL JSON")?;
    let variant_index = *instruction_words
        .first()
        .context("instruction data is empty")?;
    let instructions = idl
        .get("instructions")
        .and_then(Value::as_array)
        .context("IDL has no instructions array")?;
    let instruction = instructions
        .get(variant_index as usize)
        .with_context(|| format!("IDL instruction variant {variant_index} not found"))?;
    let instruction_name = instruction
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();

    let mut accounts = Vec::new();
    for (index, account) in instruction
        .get("accounts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let path = account
            .get("name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("account_{index}"));
        let value = account_ids
            .get(index)
            .cloned()
            .unwrap_or_else(|| "-".to_owned());
        accounts.push(DecodedField { path, value });
    }
    for (index, account_id) in account_ids.iter().enumerate().skip(accounts.len()) {
        accounts.push(DecodedField {
            path: format!("extra_{index}"),
            value: account_id.clone(),
        });
    }

    let mut offset = 1;
    let mut args = Vec::new();
    for arg in instruction
        .get("args")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = arg.get("name").and_then(Value::as_str).unwrap_or("arg");
        let Some(ty) = arg.get("type") else {
            args.push(DecodedField {
                path: name.to_owned(),
                value: "missing type".to_owned(),
            });
            continue;
        };

        match decode_instruction_type(ty, instruction_words, offset, 0) {
            Ok(decoded) => {
                args.push(DecodedField {
                    path: format!("{name}: {}", decoded.type_label),
                    value: decoded.value,
                });
                offset += decoded.consumed;
            }
            Err(err) => {
                args.push(DecodedField {
                    path: format!("{name}: {}", idl_type_label(ty)),
                    value: format!(
                        "unsupported ({err:#}); raw words {}..{}",
                        offset,
                        instruction_words.len().saturating_sub(1)
                    ),
                });
                offset = instruction_words.len();
                break;
            }
        }
    }

    Ok(InstructionDecodeReport {
        program_id: program_id.to_owned(),
        idl_name: idl
            .get("name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        instruction: instruction_name,
        variant_index,
        accounts,
        args,
        remaining_words: instruction_words.get(offset..).unwrap_or_default().to_vec(),
    })
}

pub async fn raw_json_rpc(endpoint: &str, method: &str, params: Value) -> Result<Value> {
    if method.trim().is_empty() {
        bail!("rpc method is required");
    }
    let params = match params {
        Value::Array(_) | Value::Object(_) => params,
        Value::Null => Value::Array(vec![]),
        other => bail!("rpc params must be array or object, got {other}"),
    };
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1_u64,
        "method": method,
        "params": params,
    });
    let response = reqwest::Client::new()
        .post(endpoint)
        .json(&body)
        .send()
        .await
        .with_context(|| format!("failed to call {endpoint}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .context("failed to read rpc response body")?;
    if !status.is_success() {
        bail!("rpc HTTP {status}: {}", response_excerpt(&text));
    }
    let json: Value = serde_json::from_str(&text)
        .with_context(|| format!("invalid JSON-RPC response: {}", response_excerpt(&text)))?;
    Ok(json)
}

pub async fn logos_node_cryptarchia_info(endpoint: &str) -> Result<Value> {
    raw_http_json(endpoint, "/cryptarchia/info").await
}

pub async fn raw_http_json(endpoint: &str, path: &str) -> Result<Value> {
    let endpoint = endpoint.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    let url = format!("{endpoint}/{path}");
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
    let json: Value = serde_json::from_str(&text)
        .with_context(|| format!("invalid JSON response: {}", response_excerpt(&text)))?;
    Ok(json)
}

pub(crate) fn response_excerpt(text: &str) -> String {
    text.chars().take(400).collect()
}

fn json_rpc_result<'a>(response: &'a Value, method: &str) -> Result<Option<&'a Value>> {
    if let Some(error) = response.get("error") {
        bail!("{method} returned JSON-RPC error: {error}");
    }
    Ok(response.get("result").filter(|value| !value.is_null()))
}

fn decode_sequencer_block(encoded: &str) -> Result<BlockSummary> {
    let bytes = BASE64_STANDARD
        .decode(encoded)
        .context("sequencer block result was not valid base64")?;

    let block = borsh::from_slice::<Block>(&bytes)
        .context("sequencer block result did not match LEZ block layout")?;
    Ok(summarize_block(&block))
}

pub async fn raw_rpc_report(endpoint: &str, method: &str, params: Value) -> Result<RawRpcReport> {
    Ok(RawRpcReport {
        endpoint: endpoint.to_owned(),
        method: method.to_owned(),
        response: raw_json_rpc(endpoint, method, params).await?,
    })
}

pub fn program_file_info(path: impl AsRef<Path>) -> Result<ProgramFileInfo> {
    let path = path.as_ref();
    let bytecode = fs::read(path)
        .with_context(|| format!("failed to read program bytecode at {}", path.display()))?;
    let tx = ProgramDeploymentTransaction::new(Message::new(bytecode.clone()));
    let program = Program::new(bytecode.clone().into())
        .map_err(|err| anyhow::anyhow!("failed to parse program bytecode: {err:?}"))?;
    let program_id = program.id();
    Ok(ProgramFileInfo {
        path: path.display().to_string(),
        bytecode_len: bytecode.len(),
        program_id_hex: program_id_hex(program_id),
        program_id_base58: program_id_base58(program_id),
        deployment_tx_hash: hex::encode(tx.hash()),
    })
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

fn summarize_indexer_block(value: &Value) -> IndexerBlockReport {
    let empty = Value::Null;
    let header = value.get("header").unwrap_or(&empty);
    let body = value.get("body").unwrap_or(&empty);
    let transactions = body
        .get("transactions")
        .or_else(|| value.get("transactions"))
        .and_then(Value::as_array);
    let transaction_summaries = transactions
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(index, transaction)| summarize_indexer_transaction(transaction, index))
        .collect::<Vec<_>>();

    IndexerBlockReport {
        block_id: value_u64_any(header, &["block_id", "id", "slot", "height"])
            .or_else(|| value_u64_any(value, &["block_id", "id", "slot", "height"])),
        header_hash: value_string_any(header, &["hash", "header_hash", "header_id"])
            .or_else(|| value_string_any(value, &["hash", "header_hash", "header_id"])),
        parent_hash: value_string_any(header, &["prev_block_hash", "parent_hash", "parent_id"])
            .or_else(|| {
                value_string_any(
                    value,
                    &[
                        "prev_block_hash",
                        "parent_hash",
                        "parent_id",
                        "bedrock_parent_id",
                    ],
                )
            }),
        timestamp: value_u64_any(header, &["timestamp", "time"])
            .or_else(|| value_u64_any(value, &["timestamp", "time"])),
        bedrock_status: value_string_any(value, &["bedrock_status", "status"]),
        tx_count: transactions.map_or(transaction_summaries.len(), Vec::len),
        transactions: transaction_summaries,
        raw: value.clone(),
    }
}

#[must_use]
pub fn summarize_transaction(tx: &LeeTransaction) -> TransactionSummary {
    match tx {
        LeeTransaction::ProgramDeployment(tx) => {
            let bytecode = tx.clone().into_message().into_bytecode();
            let program_id_hex = Program::new(bytecode.clone().into())
                .ok()
                .map(|program| program_id_hex(program.id()));
            TransactionSummary {
                hash: hex::encode(tx.hash()),
                kind: "ProgramDeployment".to_owned(),
                program_id_hex,
                account_ids: vec![],
                nonces: vec![],
                instruction_data: vec![],
                bytecode_len: Some(bytecode.len()),
                raw_signature_valid: None,
                message_prehash: None,
                prehash_signature_valid: None,
            }
        }
        LeeTransaction::Public(tx) => {
            let prehash = public_message_prehash(tx.message()).ok();
            TransactionSummary {
                hash: hex::encode(tx.hash()),
                kind: "Public".to_owned(),
                program_id_hex: Some(program_id_hex(tx.message().program_id)),
                account_ids: tx
                    .message()
                    .account_ids
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                nonces: tx
                    .message()
                    .nonces
                    .iter()
                    .map(|nonce| nonce.0.to_string())
                    .collect(),
                instruction_data: tx.message().instruction_data.clone(),
                bytecode_len: None,
                raw_signature_valid: Some(tx.witness_set().is_valid_for(tx.message())),
                message_prehash: prehash.map(hex::encode),
                prehash_signature_valid: public_message_prehash(tx.message())
                    .ok()
                    .map(|hash| prehash_witness_set_is_valid(tx.witness_set(), &hash)),
            }
        }
        LeeTransaction::PrivacyPreserving(tx) => TransactionSummary {
            hash: hex::encode(tx.hash()),
            kind: "PrivacyPreserving".to_owned(),
            program_id_hex: None,
            account_ids: vec![],
            nonces: vec![],
            instruction_data: vec![],
            bytecode_len: None,
            raw_signature_valid: None,
            message_prehash: None,
            prehash_signature_valid: None,
        },
    }
}

fn summarize_indexer_transaction(value: &Value, index: usize) -> AccountTransactionSummary {
    let (kind, payload) = enum_payload(value);
    let empty = Value::Null;
    let message = payload.get("message").unwrap_or(&empty);
    let bytecode_len = message.get("bytecode").and_then(|bytecode| match bytecode {
        Value::Array(items) => Some(items.len()),
        Value::String(value) => Some(value.len()),
        _ => None,
    });
    AccountTransactionSummary {
        index,
        hash: payload
            .get("hash")
            .map(value_to_string)
            .unwrap_or_else(|| "-".to_owned()),
        kind: kind.to_owned(),
        program_id_hex: message.get("program_id").map(value_to_string),
        account_ids: value_list_strings(message.get("account_ids")),
        nonces: value_list_strings(message.get("nonces")),
        instruction_data: value_list_u32(message.get("instruction_data")),
        bytecode_len,
        raw: value.clone(),
    }
}

#[derive(Debug, Clone)]
struct TransferOutput {
    wallet: String,
    amount: u128,
}

#[derive(Debug, Default)]
struct WalletAggregate {
    received: Option<u128>,
    tx_hashes: BTreeSet<String>,
    outputs: usize,
    last_slot: Option<u64>,
    transfers: Vec<WalletTransferSummary>,
}

fn wallet_summaries_from_blocks(blocks: &[IndexerBlockReport]) -> Vec<WalletSummary> {
    let mut transfers = BTreeMap::new();
    for block in blocks {
        for tx in &block.transactions {
            let outputs = transfer_outputs_from_value(&tx.raw);
            for output in outputs {
                let aggregate = transfers
                    .entry(output.wallet)
                    .or_insert_with(WalletAggregate::default);
                aggregate.received = Some(aggregate.received.unwrap_or_default() + output.amount);
                aggregate.tx_hashes.insert(tx.hash.clone());
                aggregate.outputs += 1;
                aggregate.last_slot = max_slot(aggregate.last_slot, block.block_id);
                aggregate.transfers.push(WalletTransferSummary {
                    slot: block.block_id,
                    tx_hash: tx.hash.clone(),
                    block_hash: block.header_hash.clone(),
                    value: Some(output.amount.to_string()),
                });
            }
        }
    }
    if !transfers.is_empty() {
        return wallet_summaries_from_aggregates(transfers, "transfer_outputs");
    }

    let mut account_refs = BTreeMap::new();
    for block in blocks {
        for tx in &block.transactions {
            for account_id in &tx.account_ids {
                let aggregate = account_refs
                    .entry(account_id.clone())
                    .or_insert_with(WalletAggregate::default);
                aggregate.tx_hashes.insert(tx.hash.clone());
                aggregate.outputs += 1;
                aggregate.last_slot = max_slot(aggregate.last_slot, block.block_id);
            }
        }
    }
    wallet_summaries_from_aggregates(account_refs, "account_refs")
}

fn wallet_summaries_from_aggregates(
    aggregates: BTreeMap<String, WalletAggregate>,
    source: &str,
) -> Vec<WalletSummary> {
    let mut rows = aggregates
        .into_iter()
        .map(|(wallet, mut aggregate)| {
            aggregate.transfers.sort_by(|left, right| {
                right
                    .slot
                    .cmp(&left.slot)
                    .then_with(|| right.tx_hash.cmp(&left.tx_hash))
                    .then_with(|| right.value.cmp(&left.value))
            });
            WalletSummary {
                wallet,
                received: aggregate.received.map(|value| value.to_string()),
                txs: aggregate.tx_hashes.len(),
                outputs: aggregate.outputs,
                last_slot: aggregate.last_slot,
                source: source.to_owned(),
                transfers: aggregate.transfers,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        wallet_sort_key(right)
            .cmp(&wallet_sort_key(left))
            .then_with(|| left.wallet.cmp(&right.wallet))
    });
    rows
}

fn wallet_sort_key(row: &WalletSummary) -> (u128, usize, u64) {
    (
        row.received
            .as_deref()
            .and_then(|value| value.parse().ok())
            .unwrap_or_default(),
        row.outputs,
        row.last_slot.unwrap_or_default(),
    )
}

fn max_slot(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn transfer_outputs_from_value(value: &Value) -> Vec<TransferOutput> {
    let mut outputs = Vec::new();
    collect_transfer_outputs(value, &mut outputs);
    outputs
}

fn collect_transfer_outputs(value: &Value, outputs: &mut Vec<TransferOutput>) {
    match value {
        Value::Object(object) => {
            if let Some(output) = transfer_output_from_object(object) {
                outputs.push(output);
            }
            for child in object.values() {
                collect_transfer_outputs(child, outputs);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_transfer_outputs(item, outputs);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn transfer_output_from_object(object: &Map<String, Value>) -> Option<TransferOutput> {
    let wallet = string_for_keys(
        object,
        &[
            "wallet",
            "wallet_id",
            "address",
            "recipient",
            "recipient_wallet",
            "recipient_public_key",
            "public_key",
            "pubkey",
            "to",
        ],
    )?;
    if !plausible_wallet_id(&wallet) {
        return None;
    }
    let amount = u128_for_keys(object, &["amount", "value", "received", "quantity"])?;
    Some(TransferOutput { wallet, amount })
}

fn string_for_keys(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        object.get(*key).and_then(|value| {
            let text = value_to_string(value);
            (!text.is_empty() && text != "null").then_some(text)
        })
    })
}

fn u128_for_keys(object: &Map<String, Value>, keys: &[&str]) -> Option<u128> {
    keys.iter().find_map(|key| {
        object.get(*key).and_then(|value| {
            value
                .as_u64()
                .map(u128::from)
                .or_else(|| value.as_str().and_then(|value| value.trim().parse().ok()))
        })
    })
}

fn plausible_wallet_id(value: &str) -> bool {
    let value = value.trim();
    value.len() >= 16 && value.chars().all(|char| char.is_ascii_alphanumeric())
}

fn enum_payload(value: &Value) -> (&str, &Value) {
    if let Some(object) = value.as_object()
        && object.len() == 1
        && let Some((kind, payload)) = object.iter().next()
    {
        return (kind, payload);
    }
    ("Unknown", value)
}

fn value_list_strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items.iter().map(value_to_string).collect(),
        Some(Value::String(value)) => split_list_string(value),
        Some(value) => vec![value_to_string(value)],
        None => Vec::new(),
    }
}

fn value_list_u32(value: Option<&Value>) -> Vec<u32> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| {
                item.as_u64()
                    .and_then(|value| u32::try_from(value).ok())
                    .or_else(|| item.as_str().and_then(parse_u32_text))
            })
            .collect(),
        Some(Value::String(value)) => value.split(',').filter_map(parse_u32_text).collect(),
        Some(value) => value
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .into_iter()
            .collect(),
        None => Vec::new(),
    }
}

fn value_u64_any(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
        })
    })
}

fn value_string_any(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|value| {
            let text = value_to_string(value);
            (!text.is_empty() && text != "null").then_some(text)
        })
    })
}

fn split_list_string(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_u32_text(value: &str) -> Option<u32> {
    value.trim().parse().ok()
}

#[must_use]
pub fn inspect_transaction_summary(summary: &TransactionSummary) -> TransactionInspectionReport {
    let mut sections = Vec::with_capacity(6);
    sections.push(TransactionInspectionSection {
        title: "Summary".to_owned(),
        rows: vec![
            inspection_text_row("kind", summary.kind.clone()),
            inspection_text_row("hash", summary.hash.clone()),
        ],
    });

    let program_id_base58 = summary
        .program_id_hex
        .as_deref()
        .and_then(program_id_base58_from_hex);
    let mut program_rows = Vec::new();
    if summary.program_id_hex.is_some() || program_id_base58.is_some() {
        let value = program_id_base58
            .clone()
            .or_else(|| summary.program_id_hex.clone())
            .unwrap_or_default();
        program_rows.push(TransactionInspectionRow {
            label: "program_id".to_owned(),
            index: None,
            value,
            decimal: None,
            hex: summary.program_id_hex.clone(),
            base58: program_id_base58,
        });
    }
    if let Some(bytecode_len) = summary.bytecode_len {
        program_rows.push(TransactionInspectionRow {
            label: "deployment_bytecode_len".to_owned(),
            index: None,
            value: format!("{bytecode_len} bytes"),
            decimal: Some(bytecode_len.to_string()),
            hex: None,
            base58: None,
        });
    }
    if !program_rows.is_empty() {
        sections.push(TransactionInspectionSection {
            title: "Program".to_owned(),
            rows: program_rows,
        });
    }

    let account_rows = summary
        .account_ids
        .iter()
        .enumerate()
        .map(|(index, account_id)| inspection_indexed_text_row("account", index, account_id))
        .collect::<Vec<_>>();
    if !account_rows.is_empty() {
        sections.push(TransactionInspectionSection {
            title: "Accounts".to_owned(),
            rows: account_rows,
        });
    }

    let nonce_rows = summary
        .nonces
        .iter()
        .enumerate()
        .map(|(index, nonce)| TransactionInspectionRow {
            label: "nonce".to_owned(),
            index: Some(index),
            value: nonce.clone(),
            decimal: Some(nonce.clone()),
            hex: None,
            base58: None,
        })
        .collect::<Vec<_>>();
    if !nonce_rows.is_empty() {
        sections.push(TransactionInspectionSection {
            title: "Nonces".to_owned(),
            rows: nonce_rows,
        });
    }

    let instruction_rows = summary
        .instruction_data
        .iter()
        .enumerate()
        .map(|(index, word)| instruction_word_row(index, *word))
        .collect::<Vec<_>>();
    if !instruction_rows.is_empty() {
        sections.push(TransactionInspectionSection {
            title: "Instruction words".to_owned(),
            rows: instruction_rows,
        });
    }

    let mut validation_rows = Vec::new();
    if let Some(valid) = summary.raw_signature_valid {
        validation_rows.push(inspection_validity_row("raw_signature_valid", valid));
    }
    if let Some(prehash) = &summary.message_prehash {
        validation_rows.push(TransactionInspectionRow {
            label: "message_prehash".to_owned(),
            index: None,
            value: prehash.clone(),
            decimal: None,
            hex: Some(format!("0x{prehash}")),
            base58: None,
        });
    }
    if let Some(valid) = summary.prehash_signature_valid {
        validation_rows.push(inspection_validity_row("prehash_signature_valid", valid));
    }
    if !validation_rows.is_empty() {
        sections.push(TransactionInspectionSection {
            title: "Validation".to_owned(),
            rows: validation_rows,
        });
    }

    TransactionInspectionReport {
        hash: summary.hash.clone(),
        kind: summary.kind.clone(),
        sections,
        raw_summary: summary.clone(),
    }
}

pub fn inspect_transaction_summary_with_idl(
    summary: &TransactionSummary,
    idl_json: &str,
) -> Result<TransactionIdlInspectionReport> {
    let inspection = inspect_transaction_summary(summary);
    let decoded_instruction = if summary.kind == "Public" && !summary.instruction_data.is_empty() {
        summary
            .program_id_hex
            .as_deref()
            .map(|program_id| {
                decode_instruction_words_with_idl(
                    idl_json,
                    program_id,
                    &summary.instruction_data,
                    &summary.account_ids,
                )
            })
            .transpose()?
    } else {
        None
    };

    Ok(TransactionIdlInspectionReport {
        inspection,
        decoded_instruction,
    })
}

#[must_use]
pub fn trace_transaction_summary(summary: &TransactionSummary) -> TransactionTraceReport {
    let inspection = inspect_transaction_summary(summary);
    build_transaction_trace_report(summary, inspection, None, false, None)
}

pub fn trace_transaction_summary_with_idl(
    summary: &TransactionSummary,
    idl_json: &str,
) -> Result<TransactionTraceReport> {
    let inspection = inspect_transaction_summary(summary);
    let (decoded_instruction, decode_error) = if summary.kind == "Public"
        && !summary.instruction_data.is_empty()
        && summary.program_id_hex.is_some()
    {
        match inspect_transaction_summary_with_idl(summary, idl_json) {
            Ok(idl_report) => (idl_report.decoded_instruction, None),
            Err(err) => (None, Some(format!("{err:#}"))),
        }
    } else {
        (None, None)
    };

    Ok(build_transaction_trace_report(
        summary,
        inspection,
        decoded_instruction,
        true,
        decode_error,
    ))
}

fn sequencer_client(endpoint: &str) -> Result<sequencer_service_rpc::SequencerClient> {
    SequencerClientBuilder::default()
        .build(endpoint)
        .with_context(|| format!("failed to build sequencer client for {endpoint}"))
}

async fn fetch_sequencer_transaction(
    endpoint: &str,
    tx_hash: &str,
) -> Result<Option<LeeTransaction>> {
    let parsed_hash = parse_hash(tx_hash, "transaction hash")?;
    sequencer_client(endpoint)?
        .get_transaction(parsed_hash)
        .await
        .with_context(|| format!("failed to fetch sequencer transaction {tx_hash}"))
}

fn inspect_transaction(tx: &LeeTransaction) -> TransactionInspectionReport {
    let summary = summarize_transaction(tx);
    inspect_transaction_summary(&summary)
}

fn build_transaction_trace_report(
    summary: &TransactionSummary,
    inspection: TransactionInspectionReport,
    decoded_instruction: Option<InstructionDecodeReport>,
    used_idl: bool,
    decode_error: Option<String>,
) -> TransactionTraceReport {
    let program_id_base58 = summary
        .program_id_hex
        .as_deref()
        .and_then(program_id_base58_from_hex);
    let mut capabilities = vec![
        "ordered best-effort timeline from sequencer transaction summary fields".to_owned(),
        "includes reproducible human inspection artifact for every trace".to_owned(),
    ];
    if summary.raw_signature_valid.is_some()
        || summary.message_prehash.is_some()
        || summary.prehash_signature_valid.is_some()
    {
        capabilities.push("surfaces signature and prehash validation when available".to_owned());
    }
    if used_idl {
        capabilities
            .push("can attach user-supplied IDL instruction decode when compatible".to_owned());
    }

    let mut limitations = vec![
        "no runtime execution trace, nested calls, logs, state diffs, or gas/resource metrics exposed by current APIs".to_owned(),
    ];
    if summary.kind != "Public" {
        limitations.push(format!(
            "{} transactions currently expose only summary-level fields",
            summary.kind
        ));
    } else if let Some(error) = &decode_error {
        limitations.push(format!(
            "IDL decode failed; raw instruction trace preserved: {error}"
        ));
    } else if !used_idl {
        limitations.push(
            "instruction accounts and args stay raw unless caller supplies compatible IDL JSON"
                .to_owned(),
        );
    }

    let mut steps = Vec::new();
    push_trace_step(
        &mut steps,
        "summary",
        "transaction summary loaded",
        Some("observed"),
        None,
        vec![
            format!("hash {}", summary.hash),
            format!("kind {}", summary.kind),
            "source sequencer transaction summary".to_owned(),
        ],
        None,
    );

    if let Some(valid) = summary.raw_signature_valid {
        push_trace_step(
            &mut steps,
            "validation",
            "raw witness validation",
            Some(if valid { "valid" } else { "invalid" }),
            (!valid).then_some("warning"),
            vec!["witness_set().is_valid_for(message())".to_owned()],
            None,
        );
    }
    if let Some(prehash) = &summary.message_prehash {
        push_trace_step(
            &mut steps,
            "validation",
            "message prehash derived",
            Some("derived"),
            None,
            vec![format!("sha256(prefixed Borsh public message) 0x{prehash}")],
            None,
        );
    }
    if let Some(valid) = summary.prehash_signature_valid {
        push_trace_step(
            &mut steps,
            "validation",
            "prehash witness validation",
            Some(if valid { "valid" } else { "invalid" }),
            (!valid).then_some("warning"),
            vec!["signature verified against message prehash".to_owned()],
            None,
        );
    }

    match summary.kind.as_str() {
        "ProgramDeployment" => {
            let mut details = Vec::new();
            if let Some(program_id_hex) = &summary.program_id_hex {
                details.push(format!("derived program id {program_id_hex}"));
            }
            if let Some(bytecode_len) = summary.bytecode_len {
                details.push(format!("bytecode length {bytecode_len} bytes"));
            }
            push_trace_step(
                &mut steps,
                "program",
                "program deployment payload",
                Some("observed"),
                None,
                details,
                trace_refs(
                    summary.program_id_hex.clone(),
                    program_id_base58.clone(),
                    None,
                    None,
                    None,
                ),
            );
        }
        "Public" => {
            let mut details = Vec::new();
            if let Some(program_id_hex) = &summary.program_id_hex {
                details.push(format!("program id {program_id_hex}"));
            }
            details.push(format!("accounts {}", summary.account_ids.len()));
            details.push(format!("nonces {}", summary.nonces.len()));
            details.push(format!(
                "instruction words {}",
                summary.instruction_data.len()
            ));
            push_trace_step(
                &mut steps,
                "program",
                "public program invocation",
                Some("observed"),
                None,
                details,
                trace_refs(
                    summary.program_id_hex.clone(),
                    program_id_base58.clone(),
                    None,
                    None,
                    None,
                ),
            );
        }
        other => {
            push_trace_step(
                &mut steps,
                "program",
                "transaction payload not expanded",
                Some("limited"),
                None,
                vec![format!(
                    "{other} summary has no program/account/instruction fields"
                )],
                None,
            );
        }
    }

    for (index, account_id) in summary.account_ids.iter().enumerate() {
        push_trace_step(
            &mut steps,
            "account",
            "account reference",
            Some("observed"),
            None,
            vec![format!("account[{index}] {account_id}")],
            trace_refs(None, None, Some(account_id.clone()), None, None),
        );
    }

    for (index, nonce) in summary.nonces.iter().enumerate() {
        push_trace_step(
            &mut steps,
            "nonce",
            "nonce reference",
            Some("observed"),
            None,
            vec![format!("nonce[{index}] {nonce}")],
            None,
        );
    }

    for (index, word) in summary.instruction_data.iter().enumerate() {
        push_trace_step(
            &mut steps,
            "instruction",
            "instruction word",
            Some("observed"),
            None,
            vec![
                format!("word[{index}] decimal {word}"),
                format!("word[{index}] hex 0x{word:08x}"),
            ],
            trace_refs(None, None, None, Some(index), None),
        );
    }

    if let Some(error) = &decode_error {
        push_trace_step(
            &mut steps,
            "decode",
            "IDL instruction decode unavailable",
            Some("error"),
            Some("warning"),
            vec![
                "raw instruction timeline preserved".to_owned(),
                error.clone(),
            ],
            trace_refs(
                summary.program_id_hex.clone(),
                program_id_base58.clone(),
                None,
                None,
                None,
            ),
        );
    }

    if let Some(decoded_instruction) = &decoded_instruction {
        let mut details = vec![
            format!("instruction {}", decoded_instruction.instruction),
            format!("variant {}", decoded_instruction.variant_index),
        ];
        if let Some(idl_name) = &decoded_instruction.idl_name {
            details.push(format!("idl {}", idl_name));
        }
        if !decoded_instruction.remaining_words.is_empty() {
            details.push(format!(
                "remaining words {}",
                decoded_instruction.remaining_words.len()
            ));
        }
        push_trace_step(
            &mut steps,
            "decode",
            "IDL instruction decode",
            Some("decoded"),
            None,
            details,
            trace_refs(
                Some(decoded_instruction.program_id.clone()),
                program_id_base58.clone(),
                None,
                None,
                None,
            ),
        );

        for field in &decoded_instruction.accounts {
            push_trace_step(
                &mut steps,
                "decode",
                "decoded instruction account",
                Some("decoded"),
                None,
                vec![format!("{} {}", field.path, field.value)],
                trace_refs(
                    None,
                    None,
                    (!is_placeholder_account_value(&field.value)).then(|| field.value.clone()),
                    None,
                    Some(field.path.clone()),
                ),
            );
        }

        for field in &decoded_instruction.args {
            push_trace_step(
                &mut steps,
                "decode",
                "decoded instruction arg",
                Some("decoded"),
                None,
                vec![format!("{} {}", field.path, field.value)],
                trace_refs(None, None, None, None, Some(field.path.clone())),
            );
        }

        if !decoded_instruction.remaining_words.is_empty() {
            push_trace_step(
                &mut steps,
                "decode",
                "remaining instruction words",
                Some("observed"),
                Some("warning"),
                vec![format!("{:?}", decoded_instruction.remaining_words)],
                None,
            );
        }
    }

    TransactionTraceReport {
        hash: summary.hash.clone(),
        kind: summary.kind.clone(),
        source: if used_idl {
            "sequencer transaction summary + user supplied IDL".to_owned()
        } else {
            "sequencer transaction summary".to_owned()
        },
        capabilities,
        limitations,
        steps,
        inspection,
        decoded_instruction,
    }
}

fn push_trace_step(
    steps: &mut Vec<TransactionTraceStep>,
    phase: &str,
    label: &str,
    status: Option<&str>,
    severity: Option<&str>,
    details: Vec<String>,
    refs: Option<TransactionTraceRefs>,
) {
    steps.push(TransactionTraceStep {
        index: steps.len(),
        phase: phase.to_owned(),
        label: label.to_owned(),
        status: status.map(ToOwned::to_owned),
        severity: severity.map(ToOwned::to_owned),
        details,
        refs,
    });
}

fn trace_refs(
    program_id_hex: Option<String>,
    program_id_base58: Option<String>,
    account_id: Option<String>,
    instruction_word_index: Option<usize>,
    decode_path: Option<String>,
) -> Option<TransactionTraceRefs> {
    let refs = TransactionTraceRefs {
        program_id_hex,
        program_id_base58,
        account_id,
        instruction_word_index,
        decode_path,
    };
    (refs.program_id_hex.is_some()
        || refs.program_id_base58.is_some()
        || refs.account_id.is_some()
        || refs.instruction_word_index.is_some()
        || refs.decode_path.is_some())
    .then_some(refs)
}

fn is_placeholder_account_value(value: &str) -> bool {
    value == "-"
}

fn inspection_text_row(label: &str, value: String) -> TransactionInspectionRow {
    TransactionInspectionRow {
        label: label.to_owned(),
        index: None,
        value,
        decimal: None,
        hex: None,
        base58: None,
    }
}

fn inspection_indexed_text_row(
    label: &str,
    index: usize,
    value: impl ToString,
) -> TransactionInspectionRow {
    TransactionInspectionRow {
        label: label.to_owned(),
        index: Some(index),
        value: value.to_string(),
        decimal: None,
        hex: None,
        base58: None,
    }
}

fn inspection_validity_row(label: &str, valid: bool) -> TransactionInspectionRow {
    TransactionInspectionRow {
        label: label.to_owned(),
        index: None,
        value: if valid { "valid" } else { "invalid" }.to_owned(),
        decimal: None,
        hex: None,
        base58: None,
    }
}

fn instruction_word_row(index: usize, word: u32) -> TransactionInspectionRow {
    TransactionInspectionRow {
        label: "instruction_word".to_owned(),
        index: Some(index),
        value: word.to_string(),
        decimal: Some(word.to_string()),
        hex: Some(format!("0x{word:08x}")),
        base58: None,
    }
}

fn program_id_base58_from_hex(program_id_hex: &str) -> Option<String> {
    let bytes = hex::decode(program_id_hex).ok()?;
    let fixed: [u8; 32] = bytes.try_into().ok()?;
    Some(AccountId::new(fixed).to_string())
}

fn program_entries(programs: BTreeMap<String, ProgramId>) -> Vec<ProgramIdEntry> {
    programs
        .into_iter()
        .map(|(label, program_id)| ProgramIdEntry {
            label,
            base58: program_id_base58(program_id),
            hex: program_id_hex(program_id),
        })
        .collect()
}

fn parse_account_id(value: &str) -> Result<AccountId> {
    value
        .parse()
        .with_context(|| format!("invalid account id `{value}`"))
}

fn parse_hash(value: &str, label: &str) -> Result<HashType> {
    let value = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    value
        .parse()
        .with_context(|| format!("invalid {label} `{value}`"))
}

fn public_message_prehash(message: &PublicMessage) -> Result<[u8; 32]> {
    const PREFIX: &[u8; 32] = b"/LEE/v0.3/Message/Public/\x00\x00\x00\x00\x00\x00\x00";

    let message_bytes = borsh::to_vec(message).context("failed to serialize public message")?;
    let mut bytes = Vec::with_capacity(PREFIX.len() + message_bytes.len());
    bytes.extend_from_slice(PREFIX);
    bytes.extend_from_slice(&message_bytes);

    Ok(Sha256::digest(bytes).into())
}

fn prehash_witness_set_is_valid(
    witness_set: &lee::public_transaction::WitnessSet,
    message_hash: &[u8; 32],
) -> bool {
    witness_set
        .signatures_and_public_keys()
        .iter()
        .all(|(signature, public_key)| {
            prehash_signature_is_valid(signature, public_key, message_hash)
        })
}

fn prehash_signature_is_valid(
    signature: &lee::Signature,
    public_key: &PublicKey,
    message_hash: &[u8; 32],
) -> bool {
    let Ok(verifying_key) = k256::schnorr::VerifyingKey::from_bytes(public_key.value()) else {
        return false;
    };
    let Ok(signature) = k256::schnorr::Signature::try_from(signature.value.as_slice()) else {
        return false;
    };

    verifying_key
        .verify_prehash(message_hash, &signature)
        .is_ok()
}

struct DecodedValue {
    value: Value,
    consumed: usize,
}

struct InstructionDecoded {
    value: String,
    consumed: usize,
    type_label: String,
}

fn decode_borsh_shape(
    shape: &Value,
    bytes: &[u8],
    offset: usize,
    idl: &Value,
    depth: usize,
) -> Result<DecodedValue> {
    if depth > 32 {
        bail!("IDL nesting too deep");
    }

    match shape.get("kind").and_then(Value::as_str) {
        Some("struct") => decode_borsh_struct(shape, bytes, offset, idl, depth),
        Some("enum") => decode_borsh_enum(shape, bytes, offset, idl, depth),
        Some(kind) => bail!("unsupported IDL shape kind `{kind}`"),
        None => bail!("IDL shape missing kind"),
    }
}

fn decode_borsh_struct(
    shape: &Value,
    bytes: &[u8],
    offset: usize,
    idl: &Value,
    depth: usize,
) -> Result<DecodedValue> {
    let fields = shape
        .get("fields")
        .and_then(Value::as_array)
        .context("struct shape has no fields array")?;
    let mut cursor = offset;
    let mut object = Map::new();

    for (index, field) in fields.iter().enumerate() {
        let name = field
            .get("name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("field_{index}"));
        let ty = field
            .get("type")
            .with_context(|| format!("field `{name}` has no type"))?;
        let decoded = decode_borsh_type(ty, bytes, cursor, idl, depth + 1)
            .with_context(|| format!("failed to decode field `{name}`"))?;
        cursor += decoded.consumed;
        object.insert(name, decoded.value);
    }

    Ok(DecodedValue {
        value: Value::Object(object),
        consumed: cursor - offset,
    })
}

fn decode_borsh_enum(
    shape: &Value,
    bytes: &[u8],
    offset: usize,
    idl: &Value,
    depth: usize,
) -> Result<DecodedValue> {
    let variant_index = usize::from(byte_at(bytes, offset)?);
    let variants = shape
        .get("variants")
        .and_then(Value::as_array)
        .context("enum shape has no variants array")?;
    let variant = variants
        .get(variant_index)
        .with_context(|| format!("enum variant {variant_index} not present"))?;
    let variant_name = variant
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let mut cursor = offset + 1;
    let mut fields = Map::new();

    for (index, field) in variant
        .get("fields")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let name = field
            .get("name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("field_{index}"));
        let ty = field.get("type").unwrap_or(field);
        let decoded = decode_borsh_type(ty, bytes, cursor, idl, depth + 1)
            .with_context(|| format!("failed to decode variant field `{name}`"))?;
        cursor += decoded.consumed;
        fields.insert(name, decoded.value);
    }

    Ok(DecodedValue {
        value: json!({
            "variant": variant_name,
            "variant_index": variant_index,
            "fields": fields,
        }),
        consumed: cursor - offset,
    })
}

fn decode_borsh_type(
    ty: &Value,
    bytes: &[u8],
    offset: usize,
    idl: &Value,
    depth: usize,
) -> Result<DecodedValue> {
    if depth > 32 {
        bail!("IDL nesting too deep");
    }

    if let Some(primitive) = ty.as_str() {
        return decode_borsh_primitive(primitive, bytes, offset);
    }

    let object = ty
        .as_object()
        .with_context(|| format!("unsupported IDL type {}", idl_type_label(ty)))?;

    if let Some(inner) = object.get("option") {
        let tag = byte_at(bytes, offset)?;
        if tag == 0 {
            return Ok(DecodedValue {
                value: Value::Null,
                consumed: 1,
            });
        }
        if tag != 1 {
            bail!("invalid option tag {tag}");
        }
        let decoded = decode_borsh_type(inner, bytes, offset + 1, idl, depth + 1)?;
        return Ok(DecodedValue {
            value: decoded.value,
            consumed: decoded.consumed + 1,
        });
    }

    if let Some(inner) = object.get("vec") {
        let len = read_le_unsigned(bytes, offset, 4)?;
        let len = usize::try_from(len).context("vector length does not fit usize")?;
        if len > 100_000 {
            bail!("vector length too large: {len}");
        }
        let mut cursor = offset + 4;
        let mut values = Vec::with_capacity(len);
        for _ in 0..len {
            let decoded = decode_borsh_type(inner, bytes, cursor, idl, depth + 1)?;
            cursor += decoded.consumed;
            values.push(decoded.value);
        }
        return Ok(DecodedValue {
            value: Value::Array(values),
            consumed: cursor - offset,
        });
    }

    if let Some(name) = object.get("defined").and_then(Value::as_str) {
        let shape = find_defined_shape(idl, name)
            .with_context(|| format!("defined IDL type `{name}` not found"))?;
        return decode_borsh_shape(shape, bytes, offset, idl, depth + 1);
    }

    if object.contains_key("kind") {
        return decode_borsh_shape(ty, bytes, offset, idl, depth + 1);
    }

    bail!("unsupported IDL type {}", idl_type_label(ty))
}

fn decode_borsh_primitive(ty: &str, bytes: &[u8], offset: usize) -> Result<DecodedValue> {
    let (value, consumed) = match ty {
        "bool" => (Value::Bool(byte_at(bytes, offset)? != 0), 1),
        "u8" => (Value::String(byte_at(bytes, offset)?.to_string()), 1),
        "i8" => {
            let value = i8::from_le_bytes([byte_at(bytes, offset)?]);
            (Value::String(value.to_string()), 1)
        }
        "u16" => (
            Value::String(read_le_unsigned(bytes, offset, 2)?.to_string()),
            2,
        ),
        "i16" => (
            Value::String(read_le_signed(bytes, offset, 2)?.to_string()),
            2,
        ),
        "u32" => (
            Value::String(read_le_unsigned(bytes, offset, 4)?.to_string()),
            4,
        ),
        "i32" => (
            Value::String(read_le_signed(bytes, offset, 4)?.to_string()),
            4,
        ),
        "u64" => (
            Value::String(read_le_unsigned(bytes, offset, 8)?.to_string()),
            8,
        ),
        "i64" => (
            Value::String(read_le_signed(bytes, offset, 8)?.to_string()),
            8,
        ),
        "u128" => (
            Value::String(read_le_unsigned(bytes, offset, 16)?.to_string()),
            16,
        ),
        "i128" => (
            Value::String(read_le_signed(bytes, offset, 16)?.to_string()),
            16,
        ),
        "account_id" => (
            Value::String(account_id_base58(bytes_range(bytes, offset, 32)?)),
            32,
        ),
        "program_id" => (
            Value::String(hex::encode(bytes_range(bytes, offset, 32)?)),
            32,
        ),
        "string" => {
            let len = read_le_unsigned(bytes, offset, 4)?;
            let len = usize::try_from(len).context("string length does not fit usize")?;
            let value = std::str::from_utf8(bytes_range(bytes, offset + 4, len)?)
                .context("string field is not valid UTF-8")?;
            (Value::String(value.to_owned()), 4 + len)
        }
        other => bail!("unsupported primitive IDL type `{other}`"),
    };

    Ok(DecodedValue { value, consumed })
}

fn decode_instruction_type(
    ty: &Value,
    words: &[u32],
    offset: usize,
    depth: usize,
) -> Result<InstructionDecoded> {
    if depth > 32 {
        bail!("IDL nesting too deep");
    }

    let label = idl_type_label(ty);
    if let Some(primitive) = ty.as_str() {
        return decode_instruction_primitive(primitive, words, offset).map(|mut decoded| {
            decoded.type_label = label;
            decoded
        });
    }

    let object = ty
        .as_object()
        .with_context(|| format!("unsupported instruction type {label}"))?;

    if let Some(inner) = object.get("option") {
        let tag = *words
            .get(offset)
            .with_context(|| format!("missing option tag at word {offset}"))?;
        if tag == 0 {
            return Ok(InstructionDecoded {
                value: "None".to_owned(),
                consumed: 1,
                type_label: label,
            });
        }
        if tag != 1 {
            bail!("invalid option tag {tag}");
        }
        let decoded = decode_instruction_type(inner, words, offset + 1, depth + 1)?;
        return Ok(InstructionDecoded {
            value: format!("Some({})", decoded.value),
            consumed: decoded.consumed + 1,
            type_label: label,
        });
    }

    bail!("unsupported instruction type {label}")
}

fn decode_instruction_primitive(
    ty: &str,
    words: &[u32],
    offset: usize,
) -> Result<InstructionDecoded> {
    let (value, consumed) = match ty {
        "bool" => (word_at(words, offset)? != 0).to_string().into_pair(1),
        "u8" | "u16" | "u32" => (word_at(words, offset)? as u128).to_string().into_pair(1),
        "i8" | "i16" | "i32" => (word_at(words, offset)? as i32).to_string().into_pair(1),
        "u64" => read_words_unsigned(words, offset, 2)?
            .to_string()
            .into_pair(2),
        "i64" => read_words_signed(words, offset, 2)?
            .to_string()
            .into_pair(2),
        "u128" => read_words_unsigned(words, offset, 4)?
            .to_string()
            .into_pair(4),
        "i128" => read_words_signed(words, offset, 4)?
            .to_string()
            .into_pair(4),
        "account_id" => {
            account_id_base58(&words_to_le_bytes(words_range(words, offset, 8)?)).into_pair(8)
        }
        "program_id" => hex::encode(words_to_le_bytes(words_range(words, offset, 8)?)).into_pair(8),
        "string" => {
            let len = usize::try_from(word_at(words, offset)?)
                .context("string byte length does not fit usize")?;
            let word_len = len.div_ceil(4);
            let mut bytes = words_to_le_bytes(words_range(words, offset + 1, word_len)?);
            bytes.truncate(len);
            let value = String::from_utf8(bytes).context("string arg is not valid UTF-8")?;
            value.into_pair(1 + word_len)
        }
        other => bail!("unsupported primitive instruction type `{other}`"),
    };

    Ok(InstructionDecoded {
        value,
        consumed,
        type_label: ty.to_owned(),
    })
}

trait IntoPair {
    fn into_pair(self, consumed: usize) -> (String, usize);
}

impl IntoPair for String {
    fn into_pair(self, consumed: usize) -> (String, usize) {
        (self, consumed)
    }
}

fn find_defined_shape<'a>(idl: &'a Value, name: &str) -> Option<&'a Value> {
    idl.get("types")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|item| item.get("name").and_then(Value::as_str) == Some(name))
        .or_else(|| {
            idl.get("accounts")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .find(|item| item.get("name").and_then(Value::as_str) == Some(name))
                .and_then(|item| item.get("type"))
        })
}

fn select_idl_event<'a>(idl: &'a Value, event_name: Option<&str>) -> Result<&'a Value> {
    let events = idl
        .get("events")
        .and_then(Value::as_array)
        .context("IDL has no events array")?;
    if events.is_empty() {
        bail!("IDL events array is empty");
    }

    let selected = event_name.map(str::trim).filter(|value| !value.is_empty());
    if let Some(selected) = selected {
        return events
            .iter()
            .find(|event| event.get("name").and_then(Value::as_str) == Some(selected))
            .with_context(|| format!("IDL event `{selected}` not found"));
    }

    if events.len() == 1 {
        return events
            .first()
            .context("IDL events array is empty after length check");
    }

    let names = events
        .iter()
        .map(idl_event_name)
        .collect::<Vec<_>>()
        .join(", ");
    bail!(
        "event name required because IDL has {} events: {names}",
        events.len()
    )
}

fn decode_idl_event(idl: &Value, event: &Value, data: &[u8]) -> Result<EventIdlDecodeReport> {
    let name = idl_event_name(event).to_owned();
    let decoded = if let Some(ty) = event.get("type") {
        decode_borsh_type(ty, data, 0, idl, 0)
    } else if let Some(fields) = event.get("fields").and_then(Value::as_array) {
        let shape = json!({
            "kind": "struct",
            "fields": fields,
        });
        decode_borsh_shape(&shape, data, 0, idl, 0)
    } else {
        bail!("IDL event `{name}` must have a `type` shape or `fields` array")
    }
    .with_context(|| {
        format!(
            "failed to decode IDL event `{name}`; event data is assumed to be raw Borsh payload with no discriminator"
        )
    })?;

    if decoded.consumed != data.len() {
        bail!(
            "IDL event `{name}` decoded {} of {} bytes; event data is assumed to be raw Borsh payload with no discriminator",
            decoded.consumed,
            data.len()
        );
    }

    let mut rows = Vec::new();
    flatten_decoded_value(&decoded.value, "", &mut rows);
    Ok(EventIdlDecodeReport {
        event: name,
        consumed_bytes: decoded.consumed,
        total_bytes: data.len(),
        decoded: decoded.value,
        rows,
    })
}

fn idl_event_name(event: &Value) -> &str {
    event
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}

fn flatten_decoded_value(value: &Value, prefix: &str, rows: &mut Vec<DecodedField>) {
    match value {
        Value::Object(object) => {
            if let Some(variant) = object.get("variant") {
                rows.push(DecodedField {
                    path: prefixed(prefix, "variant"),
                    value: value_to_string(variant),
                });
                if let Some(fields) = object.get("fields") {
                    flatten_decoded_value(fields, prefix, rows);
                }
                return;
            }

            if object.is_empty() {
                rows.push(DecodedField {
                    path: prefix_or_value(prefix),
                    value: "{}".to_owned(),
                });
                return;
            }

            for (key, child) in object {
                flatten_decoded_value(child, &prefixed(prefix, key), rows);
            }
        }
        Value::Array(items) => {
            if items.is_empty() {
                rows.push(DecodedField {
                    path: prefix_or_value(prefix),
                    value: "[]".to_owned(),
                });
                return;
            }

            for (index, child) in items.iter().enumerate() {
                flatten_decoded_value(
                    child,
                    &format!("{}[{index}]", prefix_or_value(prefix)),
                    rows,
                );
            }
        }
        _ => rows.push(DecodedField {
            path: prefix_or_value(prefix),
            value: value_to_string(value),
        }),
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Null => "null".to_owned(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn prefixed(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_owned()
    } else {
        format!("{prefix}.{key}")
    }
}

fn prefix_or_value(prefix: &str) -> String {
    if prefix.is_empty() {
        "value".to_owned()
    } else {
        prefix.to_owned()
    }
}

fn idl_type_label(ty: &Value) -> String {
    if let Some(value) = ty.as_str() {
        return value.to_owned();
    }
    if let Some(inner) = ty.get("option") {
        return format!("option<{}>", idl_type_label(inner));
    }
    if let Some(inner) = ty.get("vec") {
        return format!("vec<{}>", idl_type_label(inner));
    }
    if let Some(name) = ty.get("defined").and_then(Value::as_str) {
        return name.to_owned();
    }
    ty.to_string()
}

fn parse_hex_bytes(value: &str) -> Result<Vec<u8>> {
    let mut hex = value.trim();
    if let Some(stripped) = hex.strip_prefix("0x").or_else(|| hex.strip_prefix("0X")) {
        hex = stripped;
    }
    let hex = hex
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    if hex.len() % 2 != 0 {
        bail!("hex string must have even length");
    }
    hex::decode(hex).context("invalid hex")
}

fn byte_at(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes
        .get(offset)
        .copied()
        .with_context(|| format!("unexpected end of data at byte {offset}, need 1 byte"))
}

fn bytes_range(bytes: &[u8], offset: usize, count: usize) -> Result<&[u8]> {
    if offset
        .checked_add(count)
        .is_some_and(|end| end <= bytes.len())
    {
        let end = offset + count;
        bytes
            .get(offset..end)
            .with_context(|| format!("unexpected end of data at byte {offset}, need {count} bytes"))
    } else {
        bail!("unexpected end of data at byte {offset}, need {count} bytes")
    }
}

fn read_le_unsigned(bytes: &[u8], offset: usize, count: usize) -> Result<u128> {
    let bytes = bytes_range(bytes, offset, count)?;
    if count > 16 {
        bail!("cannot decode unsigned integer wider than 128 bits");
    }
    let mut value = 0_u128;
    for (index, byte) in bytes.iter().copied().enumerate() {
        value |= u128::from(byte) << (8 * index);
    }
    Ok(value)
}

fn read_le_signed(bytes: &[u8], offset: usize, count: usize) -> Result<i128> {
    let bytes = bytes_range(bytes, offset, count)?;
    if count > 16 {
        bail!("cannot decode signed integer wider than 128 bits");
    }
    let high_byte = bytes
        .last()
        .copied()
        .context("cannot decode zero-width signed integer")?;
    let mut fixed = if high_byte & 0x80 == 0 {
        [0_u8; 16]
    } else {
        [0xff_u8; 16]
    };
    fixed
        .get_mut(..count)
        .context("cannot decode signed integer wider than 128 bits")?
        .copy_from_slice(bytes);
    Ok(i128::from_le_bytes(fixed))
}

fn word_at(words: &[u32], offset: usize) -> Result<u32> {
    words
        .get(offset)
        .copied()
        .with_context(|| format!("missing word {offset}"))
}

fn words_range(words: &[u32], offset: usize, count: usize) -> Result<&[u32]> {
    if offset
        .checked_add(count)
        .is_some_and(|end| end <= words.len())
    {
        let end = offset + count;
        words.get(offset..end).with_context(|| {
            format!("unexpected end of instruction data at word {offset}, need {count} words")
        })
    } else {
        bail!("unexpected end of instruction data at word {offset}, need {count} words")
    }
}

fn read_words_unsigned(words: &[u32], offset: usize, count: usize) -> Result<u128> {
    let words = words_range(words, offset, count)?;
    if count > 4 {
        bail!("cannot decode instruction integer wider than 128 bits");
    }
    let mut value = 0_u128;
    for (index, word) in words.iter().copied().enumerate() {
        value |= u128::from(word) << (32 * index);
    }
    Ok(value)
}

fn read_words_signed(words: &[u32], offset: usize, count: usize) -> Result<i128> {
    let words = words_range(words, offset, count)?;
    if count > 4 {
        bail!("cannot decode instruction integer wider than 128 bits");
    }
    let high_word = words
        .last()
        .copied()
        .context("cannot decode zero-width signed integer")?;
    let fill = if high_word & 0x8000_0000 == 0 {
        0_u32
    } else {
        u32::MAX
    };
    let mut fixed = [fill; 4];
    fixed
        .get_mut(..count)
        .context("cannot decode instruction integer wider than 128 bits")?
        .copy_from_slice(words);
    let bytes = words_to_le_bytes(&fixed);
    let mut fixed_bytes = [0_u8; 16];
    fixed_bytes.copy_from_slice(&bytes);
    Ok(i128::from_le_bytes(fixed_bytes))
}

fn words_to_le_bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

fn account_id_base58(bytes: &[u8]) -> String {
    let mut fixed = [0_u8; 32];
    fixed.copy_from_slice(bytes);
    AccountId::new(fixed).to_string()
}

#[must_use]
pub fn program_id_hex(program_id: ProgramId) -> String {
    program_id
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[must_use]
pub fn program_id_base58(program_id: ProgramId) -> String {
    AccountId::new(program_id_bytes(program_id)).to_string()
}

fn program_id_bytes(program_id: ProgramId) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    for (chunk, word) in bytes.chunks_exact_mut(4).zip(program_id.iter()) {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    bytes
}

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
    fn summarize_indexer_block_maps_header_hash_and_transactions() {
        let header_hash = "ab".repeat(32);
        let parent_hash = "cd".repeat(32);
        let tx_hash = "ef".repeat(32);
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
                            "program_id": "program-1",
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
        assert_eq!(summary.raw, raw);
    }

    #[test]
    fn wallet_summaries_use_transfer_outputs_when_available() {
        let wallet_a = "aa".repeat(32);
        let wallet_b = "bb".repeat(32);
        let raw = serde_json::json!({
            "Public": {
                "hash": "tx-a",
                "message": {
                    "outputs": [
                        { "recipient": wallet_a.clone(), "amount": 7 },
                        { "recipient": wallet_a.clone(), "amount": "5" },
                        { "recipient": wallet_b.clone(), "amount": 9 }
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

        let wallets = wallet_summaries_from_blocks(&[block]);

        assert_eq!(wallets.len(), 2);
        assert_eq!(
            wallets.first().map(|wallet| wallet.wallet.as_str()),
            Some(wallet_a.as_str())
        );
        assert_eq!(
            wallets
                .first()
                .and_then(|wallet| wallet.received.as_deref()),
            Some("12")
        );
        assert_eq!(wallets.first().map(|wallet| wallet.txs), Some(1));
        assert_eq!(wallets.first().map(|wallet| wallet.outputs), Some(2));
        assert_eq!(wallets.first().and_then(|wallet| wallet.last_slot), Some(9));
        assert_eq!(
            wallets.first().map(|wallet| wallet.transfers.len()),
            Some(2)
        );
        assert_eq!(
            wallets
                .first()
                .and_then(|wallet| wallet.transfers.first())
                .map(|transfer| transfer.tx_hash.as_str()),
            Some("tx-a")
        );
        assert_eq!(
            wallets
                .first()
                .and_then(|wallet| wallet.transfers.first())
                .and_then(|transfer| transfer.block_hash.as_deref()),
            Some("block-a")
        );
        assert_eq!(
            wallets.first().map(|wallet| wallet.source.as_str()),
            Some("transfer_outputs")
        );
    }

    #[test]
    fn wallet_summaries_fall_back_to_account_refs() {
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

        let wallets = wallet_summaries_from_blocks(&[block]);

        assert_eq!(wallets.len(), 2);
        assert!(wallets.iter().all(|wallet| wallet.received.is_none()));
        assert!(wallets.iter().all(|wallet| wallet.txs == 1));
        assert!(wallets.iter().all(|wallet| wallet.outputs == 1));
        assert!(wallets.iter().all(|wallet| wallet.transfers.is_empty()));
        assert!(wallets.iter().all(|wallet| wallet.last_slot == Some(8)));
        assert!(wallets.iter().all(|wallet| wallet.source == "account_refs"));
    }

    #[test]
    fn account_report_serializes_loaded_empty_related_transactions() {
        let report = AccountReport {
            account_id: "acct-a".to_owned(),
            account: serde_json::json!({}),
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
    fn resolve_network_endpoints_uses_default_profile_without_overrides() {
        let endpoints = resolve_network_endpoints(None, None, None, None);

        assert!(endpoints.is_ok(), "{endpoints:?}");
        let Ok(endpoints) = endpoints else {
            return;
        };
        assert_eq!(endpoints.profile, DEFAULT_NETWORK_PROFILE);
        assert_eq!(endpoints.sequencer_endpoint, DEFAULT_SEQUENCER_ENDPOINT);
        assert_eq!(endpoints.indexer_endpoint, DEFAULT_INDEXER_ENDPOINT);
        assert_eq!(endpoints.node_endpoint, DEFAULT_NODE_ENDPOINT);
    }

    #[test]
    fn resolve_network_endpoints_uses_testnet_indexer_local_profile() {
        let endpoints = resolve_network_endpoints(Some("testnet-indexer-local"), None, None, None);

        assert!(endpoints.is_ok(), "{endpoints:?}");
        let Ok(endpoints) = endpoints else {
            return;
        };
        assert_eq!(endpoints.profile, "testnet-indexer-local");
        assert_eq!(endpoints.sequencer_endpoint, DEFAULT_SEQUENCER_ENDPOINT);
        assert_eq!(endpoints.indexer_endpoint, DEFAULT_INDEXER_ENDPOINT);
        assert_eq!(endpoints.node_endpoint, DEFAULT_NODE_ENDPOINT);
    }

    #[test]
    fn resolve_network_endpoints_uses_local_node_profile() {
        let endpoints =
            resolve_network_endpoints(Some(LOCAL_NODE_NETWORK_PROFILE), None, None, None);

        assert!(endpoints.is_ok(), "{endpoints:?}");
        let Ok(endpoints) = endpoints else {
            return;
        };
        assert_eq!(endpoints.profile, LOCAL_NODE_NETWORK_PROFILE);
        assert_eq!(endpoints.sequencer_endpoint, DEFAULT_SEQUENCER_ENDPOINT);
        assert_eq!(endpoints.indexer_endpoint, DEFAULT_INDEXER_ENDPOINT);
        assert_eq!(endpoints.node_endpoint, DEFAULT_NODE_ENDPOINT);
    }

    #[test]
    fn resolve_network_endpoints_uses_local_profile() {
        let endpoints = resolve_network_endpoints(Some("local"), None, None, None);

        assert!(endpoints.is_ok(), "{endpoints:?}");
        let Ok(endpoints) = endpoints else {
            return;
        };
        assert_eq!(endpoints.profile, "local");
        assert_eq!(endpoints.sequencer_endpoint, LOCAL_SEQUENCER_ENDPOINT);
        assert_eq!(endpoints.indexer_endpoint, DEFAULT_INDEXER_ENDPOINT);
        assert_eq!(endpoints.node_endpoint, DEFAULT_NODE_ENDPOINT);
    }

    #[test]
    fn resolve_network_endpoints_preserves_custom_urls() {
        let sequencer = "https://sequencer.example.invalid/";
        let indexer = "http://127.0.0.1:9999/";
        let node = "http://127.0.0.1:9090/";
        let endpoints = resolve_network_endpoints(
            Some(CUSTOM_NETWORK_PROFILE),
            Some(sequencer),
            Some(indexer),
            Some(node),
        );

        assert!(endpoints.is_ok(), "{endpoints:?}");
        let Ok(endpoints) = endpoints else {
            return;
        };
        assert_eq!(endpoints.profile, CUSTOM_NETWORK_PROFILE);
        assert_eq!(endpoints.sequencer_endpoint, sequencer);
        assert_eq!(endpoints.indexer_endpoint, indexer);
        assert_eq!(endpoints.node_endpoint, node);
    }

    #[test]
    fn resolve_network_endpoints_explicit_urls_override_profile() {
        let sequencer = "https://override.example.invalid/";
        let endpoints =
            resolve_network_endpoints(Some("testnet-indexer-local"), Some(sequencer), None, None);

        assert!(endpoints.is_ok(), "{endpoints:?}");
        let Ok(endpoints) = endpoints else {
            return;
        };
        assert_eq!(endpoints.profile, CUSTOM_NETWORK_PROFILE);
        assert_eq!(endpoints.sequencer_endpoint, sequencer);
        assert_eq!(endpoints.indexer_endpoint, DEFAULT_INDEXER_ENDPOINT);
        assert_eq!(endpoints.node_endpoint, DEFAULT_NODE_ENDPOINT);
    }

    #[test]
    fn resolve_network_endpoints_rejects_unknown_profile() {
        let endpoints = resolve_network_endpoints(Some("missing"), None, None, None);

        assert!(endpoints.is_err(), "{endpoints:?}");
        let Err(err) = endpoints else {
            return;
        };
        assert!(err.to_string().contains("unknown network profile"));
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

    #[test]
    fn decode_event_data_hex_with_idl_decodes_single_event_without_name() {
        let idl = r#"{
            "name": "test_program",
            "events": [
                {
                    "name": "LogEntry",
                    "fields": [
                        { "name": "amount", "type": "u64" },
                        { "name": "memo", "type": "string" }
                    ]
                }
            ]
        }"#;

        let report = decode_event_data_hex_with_idl(idl, None, "2a00000000000000020000006f6b");

        assert!(report.is_ok(), "{report:?}");
        let Ok(report) = report else {
            return;
        };
        assert_eq!(report.event, "LogEntry");
        assert_eq!(report.consumed_bytes, 14);
        assert_eq!(report.total_bytes, 14);

        let amount = report.rows.iter().find(|row| row.path == "amount");
        assert!(amount.is_some(), "missing amount row");
        let Some(amount) = amount else {
            return;
        };
        assert_eq!(amount.value, "42");

        let memo = report.rows.iter().find(|row| row.path == "memo");
        assert!(memo.is_some(), "missing memo row");
        let Some(memo) = memo else {
            return;
        };
        assert_eq!(memo.value, "ok");
    }

    #[test]
    fn decode_account_data_hex_with_idl_preserves_remaining_account_data() {
        let idl = r#"{
            "name": "test_program",
            "accounts": [
                {
                    "name": "ShortAccount",
                    "type": {
                        "kind": "struct",
                        "fields": [
                            { "name": "tag", "type": "u8" }
                        ]
                    }
                }
            ]
        }"#;

        let report =
            decode_account_data_hex_with_idl(idl, Some("ShortAccount"), "010203", Some("acct"));

        assert!(report.is_ok(), "{report:?}");
        let Ok(report) = report else {
            return;
        };
        assert_eq!(report.account_id.as_deref(), Some("acct"));
        assert_eq!(report.account_type, "ShortAccount");
        assert_eq!(report.consumed_bytes, 1);
        assert_eq!(report.total_bytes, 3);
        assert_eq!(report.remaining_bytes, 2);
        assert_eq!(report.remaining_data_hex.as_deref(), Some("0203"));

        let tag = report.rows.iter().find(|row| row.path == "tag");
        assert!(tag.is_some(), "missing tag row");
        let Some(tag) = tag else {
            return;
        };
        assert_eq!(tag.value, "1");

        let remaining = report
            .rows
            .iter()
            .find(|row| row.path == "remaining_data_hex");
        assert!(remaining.is_some(), "missing remaining data row");
    }

    #[test]
    fn account_report_with_optional_idl_decode_preserves_account_when_decode_fails() {
        let account = AccountReport {
            account_id: "acct".to_owned(),
            account: serde_json::json!({ "balance": "0" }),
            data_hex: "ff".to_owned(),
            related_transactions: Some(Vec::new()),
            related_transactions_error: None,
        };
        let idl = r#"{
            "accounts": [
                {
                    "name": "TooLong",
                    "type": {
                        "kind": "struct",
                        "fields": [
                            { "name": "amount", "type": "u64" }
                        ]
                    }
                }
            ]
        }"#;

        let report = account_report_with_optional_idl_decode(account, idl, Some("TooLong"));

        assert_eq!(report.account.account_id, "acct");
        assert!(report.decode.is_none());
        assert!(
            report
                .decode_error
                .as_deref()
                .is_some_and(|error| error.contains("failed to decode as `TooLong`"))
        );
    }

    #[test]
    fn decode_event_data_hex_with_idl_selects_named_event_type_shape() {
        let idl = r#"{
            "name": "test_program",
            "events": [
                {
                    "name": "Ignored",
                    "fields": [
                        { "name": "value", "type": "u8" }
                    ]
                },
                {
                    "name": "ValueChanged",
                    "type": {
                        "kind": "struct",
                        "fields": [
                            { "name": "value", "type": "u16" },
                            { "name": "enabled", "type": "bool" }
                        ]
                    }
                }
            ]
        }"#;

        let report = decode_event_data_hex_with_idl(idl, Some("ValueChanged"), "010201");

        assert!(report.is_ok(), "{report:?}");
        let Ok(report) = report else {
            return;
        };
        assert_eq!(report.event, "ValueChanged");

        let value = report.rows.iter().find(|row| row.path == "value");
        assert!(value.is_some(), "missing value row");
        let Some(value) = value else {
            return;
        };
        assert_eq!(value.value, "513");

        let enabled = report.rows.iter().find(|row| row.path == "enabled");
        assert!(enabled.is_some(), "missing enabled row");
        let Some(enabled) = enabled else {
            return;
        };
        assert_eq!(enabled.value, "true");
    }

    #[test]
    fn inspect_transaction_summary_with_idl_adds_instruction_decode() {
        let summary = TransactionSummary {
            hash: "abcd1234".to_owned(),
            kind: "Public".to_owned(),
            program_id_hex: Some(
                "0100000002000000030000000400000005000000060000000700000008000000".to_owned(),
            ),
            account_ids: vec!["acct-a".to_owned()],
            nonces: vec![],
            instruction_data: vec![0, 9],
            bytecode_len: None,
            raw_signature_valid: None,
            message_prehash: None,
            prehash_signature_valid: None,
        };
        let idl = r#"{
            "name": "test_program",
            "instructions": [
                {
                    "name": "set_value",
                    "accounts": [
                        { "name": "target" }
                    ],
                    "args": [
                        { "name": "value", "type": "u32" }
                    ]
                }
            ]
        }"#;

        let report = inspect_transaction_summary_with_idl(&summary, idl);

        assert!(report.is_ok(), "{report:?}");
        let Ok(report) = report else {
            return;
        };
        assert_eq!(report.inspection.hash, "abcd1234");
        assert_eq!(report.inspection.kind, "Public");

        let decoded = report.decoded_instruction.as_ref();
        assert!(decoded.is_some(), "missing instruction decode");
        let Some(decoded) = decoded else {
            return;
        };
        assert_eq!(decoded.instruction, "set_value");
        assert_eq!(decoded.variant_index, 0);

        let target = decoded.accounts.iter().find(|row| row.path == "target");
        assert!(target.is_some(), "missing target account");
        let Some(target) = target else {
            return;
        };
        assert_eq!(target.value, "acct-a");

        let value = decoded.args.iter().find(|row| row.path == "value: u32");
        assert!(value.is_some(), "missing value arg");
        let Some(value) = value else {
            return;
        };
        assert_eq!(value.value, "9");
    }

    #[test]
    fn trace_transaction_summary_with_idl_adds_decode_steps() {
        // Arrange
        let summary = TransactionSummary {
            hash: "abcd1234".to_owned(),
            kind: "Public".to_owned(),
            program_id_hex: Some(
                "0100000002000000030000000400000005000000060000000700000008000000".to_owned(),
            ),
            account_ids: vec!["acct-a".to_owned()],
            nonces: vec![],
            instruction_data: vec![0, 9],
            bytecode_len: None,
            raw_signature_valid: None,
            message_prehash: None,
            prehash_signature_valid: None,
        };
        let idl = r#"{
            "name": "test_program",
            "instructions": [
                {
                    "name": "set_value",
                    "accounts": [
                        { "name": "target" }
                    ],
                    "args": [
                        { "name": "value", "type": "u32" }
                    ]
                }
            ]
        }"#;

        // Act
        let report = trace_transaction_summary_with_idl(&summary, idl);

        // Assert
        assert!(report.is_ok(), "{report:?}");
        let Ok(report) = report else {
            return;
        };
        assert_eq!(
            report.source,
            "sequencer transaction summary + user supplied IDL"
        );
        assert!(
            report.decoded_instruction.is_some(),
            "missing decode report"
        );

        let decode = report
            .steps
            .iter()
            .find(|step| step.label == "IDL instruction decode");
        assert!(decode.is_some(), "missing decode step");
        let Some(decode) = decode else {
            return;
        };
        assert_eq!(decode.phase, "decode");
        assert_eq!(decode.status.as_deref(), Some("decoded"));

        let decoded_account = report
            .steps
            .iter()
            .find(|step| step.label == "decoded instruction account");
        assert!(decoded_account.is_some(), "missing decoded account step");
        let Some(decoded_account) = decoded_account else {
            return;
        };
        assert_eq!(
            decoded_account
                .refs
                .as_ref()
                .and_then(|refs| refs.decode_path.as_deref()),
            Some("target")
        );
        assert_eq!(
            decoded_account
                .refs
                .as_ref()
                .and_then(|refs| refs.account_id.as_deref()),
            Some("acct-a")
        );

        let decoded_arg = report
            .steps
            .iter()
            .find(|step| step.label == "decoded instruction arg");
        assert!(decoded_arg.is_some(), "missing decoded arg step");
        let Some(decoded_arg) = decoded_arg else {
            return;
        };
        assert!(
            decoded_arg
                .details
                .iter()
                .any(|detail| detail.contains("value: u32 9")),
            "{decoded_arg:?}"
        );
    }

    #[test]
    fn trace_transaction_summary_with_invalid_idl_preserves_raw_trace() {
        // Arrange
        let summary = TransactionSummary {
            hash: "abcd1234".to_owned(),
            kind: "Public".to_owned(),
            program_id_hex: Some(
                "0100000002000000030000000400000005000000060000000700000008000000".to_owned(),
            ),
            account_ids: vec!["acct-a".to_owned()],
            nonces: vec![],
            instruction_data: vec![3, 9],
            bytecode_len: None,
            raw_signature_valid: None,
            message_prehash: None,
            prehash_signature_valid: None,
        };

        // Act
        let report = trace_transaction_summary_with_idl(&summary, "{");

        // Assert
        assert!(report.is_ok(), "{report:?}");
        let Ok(report) = report else {
            return;
        };
        assert!(report.decoded_instruction.is_none());
        assert!(
            report
                .limitations
                .iter()
                .any(|item| item.contains("IDL decode failed; raw instruction trace preserved")),
            "{report:?}"
        );

        let raw_word = report.steps.iter().find(|step| {
            step.label == "instruction word"
                && step
                    .refs
                    .as_ref()
                    .and_then(|refs| refs.instruction_word_index)
                    == Some(1)
        });
        assert!(raw_word.is_some(), "missing raw instruction word step");

        let decode_warning = report
            .steps
            .iter()
            .find(|step| step.label == "IDL instruction decode unavailable");
        assert!(decode_warning.is_some(), "missing decode warning step");
        let Some(decode_warning) = decode_warning else {
            return;
        };
        assert_eq!(decode_warning.phase, "decode");
        assert_eq!(decode_warning.status.as_deref(), Some("error"));
        assert_eq!(decode_warning.severity.as_deref(), Some("warning"));
        assert!(
            decode_warning
                .details
                .iter()
                .any(|detail| detail.contains("failed to parse IDL JSON")),
            "{decode_warning:?}"
        );
    }

    #[test]
    fn trace_transaction_summary_with_idl_omits_placeholder_account_refs() {
        // Arrange
        let summary = TransactionSummary {
            hash: "abcd1234".to_owned(),
            kind: "Public".to_owned(),
            program_id_hex: Some(
                "0100000002000000030000000400000005000000060000000700000008000000".to_owned(),
            ),
            account_ids: vec![],
            nonces: vec![],
            instruction_data: vec![0, 9],
            bytecode_len: None,
            raw_signature_valid: None,
            message_prehash: None,
            prehash_signature_valid: None,
        };
        let idl = r#"{
            "name": "test_program",
            "instructions": [
                {
                    "name": "set_value",
                    "accounts": [
                        { "name": "target" }
                    ],
                    "args": [
                        { "name": "value", "type": "u32" }
                    ]
                }
            ]
        }"#;

        // Act
        let report = trace_transaction_summary_with_idl(&summary, idl);

        // Assert
        assert!(report.is_ok(), "{report:?}");
        let Ok(report) = report else {
            return;
        };
        let decoded_account = report
            .steps
            .iter()
            .find(|step| step.label == "decoded instruction account");
        assert!(decoded_account.is_some(), "missing decoded account step");
        let Some(decoded_account) = decoded_account else {
            return;
        };
        assert_eq!(
            decoded_account
                .refs
                .as_ref()
                .and_then(|refs| refs.decode_path.as_deref()),
            Some("target")
        );
        assert_eq!(
            decoded_account
                .refs
                .as_ref()
                .and_then(|refs| refs.account_id.as_deref()),
            None
        );
    }

    #[test]
    fn inspect_transaction_summary_with_idl_skips_decode_without_public_program_invocation() {
        let summary = TransactionSummary {
            hash: "abcd1234".to_owned(),
            kind: "Public".to_owned(),
            program_id_hex: None,
            account_ids: vec!["acct-a".to_owned()],
            nonces: vec![],
            instruction_data: vec![0, 9],
            bytecode_len: None,
            raw_signature_valid: None,
            message_prehash: None,
            prehash_signature_valid: None,
        };

        let report = inspect_transaction_summary_with_idl(&summary, "{}");

        assert!(report.is_ok(), "{report:?}");
        let Ok(report) = report else {
            return;
        };
        assert!(report.decoded_instruction.is_none());

        let non_public_summary = TransactionSummary {
            kind: "ProgramDeployment".to_owned(),
            program_id_hex: Some(
                "0100000002000000030000000400000005000000060000000700000008000000".to_owned(),
            ),
            ..summary
        };
        let report = inspect_transaction_summary_with_idl(&non_public_summary, "{}");

        assert!(report.is_ok(), "{report:?}");
        let Ok(report) = report else {
            return;
        };
        assert!(report.decoded_instruction.is_none());
    }
}
