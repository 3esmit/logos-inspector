use anyhow::Result;
use serde_json::Value;
use tokio::runtime::Runtime;

mod decode;
mod local_nodes;
mod module_reports;
mod network;
mod state;
mod storage;
mod wallet;

pub(crate) const RUNTIME_METHODS: &[&str] = &[
    "decodeTransactionSummary",
    "decodeAccount",
    "resolveAccountDecodeSession",
    "resolveTransactionDecodeSession",
    "decodeEvent",
    "spelIdl",
    "programFile",
    "normalizeProgramId",
    "overview",
    "channelScan",
    "channelState",
    "rawRpc",
    "localWalletProfileStatus",
    "localWalletInstructionPreview",
    "bedrockWalletBalance",
    "detectWalletProfile",
    "localNodesStatus",
    "localDevnetList",
    "loadIdlState",
    "saveIdlState",
    "loadWalletState",
    "saveWalletState",
    "loadSettingsState",
    "saveSettingsState",
    "sourcePolicy",
    "modules",
    "logoscoreStatus",
    "blockchainModuleReport",
    "storageReport",
    "storageSourceReport",
    "deliveryReport",
    "deliverySourceReport",
    "storageExists",
    "storageBackupSettings",
    "storageRestoreSettings",
    "socialMessagesFromStore",
];

pub(crate) fn is_runtime_method(method: &str) -> bool {
    RUNTIME_METHODS.contains(&method)
}

pub(super) fn try_handle(runtime: &Runtime, method: &str, args: Value) -> Result<Option<Value>> {
    if let Some(value) = decode::try_handle(method, args.clone())? {
        return Ok(Some(value));
    }
    if let Some(value) = network::try_handle(runtime, method, args.clone())? {
        return Ok(Some(value));
    }
    if let Some(value) = wallet::try_handle(runtime, method, args.clone())? {
        return Ok(Some(value));
    }
    if let Some(value) = local_nodes::try_handle(method, args.clone())? {
        return Ok(Some(value));
    }
    if let Some(value) = state::try_handle(method, args.clone())? {
        return Ok(Some(value));
    }
    if let Some(value) = module_reports::try_handle(runtime, method, args.clone())? {
        return Ok(Some(value));
    }
    storage::try_handle(runtime, method, args)
}
