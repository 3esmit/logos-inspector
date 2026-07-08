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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeMethod {
    DecodeTransactionSummary,
    DecodeAccount,
    ResolveAccountDecodeSession,
    ResolveTransactionDecodeSession,
    DecodeEvent,
    SpelIdl,
    ProgramFile,
    NormalizeProgramId,
    Overview,
    ChannelScan,
    ChannelState,
    RawRpc,
    LocalWalletProfileStatus,
    LocalWalletInstructionPreview,
    BedrockWalletBalance,
    DetectWalletProfile,
    LocalNodesStatus,
    LocalDevnetList,
    LoadIdlState,
    SaveIdlState,
    LoadWalletState,
    SaveWalletState,
    LoadSettingsState,
    SaveSettingsState,
    SourcePolicy,
    Modules,
    LogoscoreStatus,
    BlockchainModuleReport,
    StorageReport,
    StorageSourceReport,
    DeliveryReport,
    DeliverySourceReport,
    StorageExists,
    StorageBackupSettings,
    StorageRestoreSettings,
    SocialMessagesFromStore,
}

pub(crate) const RUNTIME_METHODS: &[RuntimeMethod] = &[
    RuntimeMethod::DecodeTransactionSummary,
    RuntimeMethod::DecodeAccount,
    RuntimeMethod::ResolveAccountDecodeSession,
    RuntimeMethod::ResolveTransactionDecodeSession,
    RuntimeMethod::DecodeEvent,
    RuntimeMethod::SpelIdl,
    RuntimeMethod::ProgramFile,
    RuntimeMethod::NormalizeProgramId,
    RuntimeMethod::Overview,
    RuntimeMethod::ChannelScan,
    RuntimeMethod::ChannelState,
    RuntimeMethod::RawRpc,
    RuntimeMethod::LocalWalletProfileStatus,
    RuntimeMethod::LocalWalletInstructionPreview,
    RuntimeMethod::BedrockWalletBalance,
    RuntimeMethod::DetectWalletProfile,
    RuntimeMethod::LocalNodesStatus,
    RuntimeMethod::LocalDevnetList,
    RuntimeMethod::LoadIdlState,
    RuntimeMethod::SaveIdlState,
    RuntimeMethod::LoadWalletState,
    RuntimeMethod::SaveWalletState,
    RuntimeMethod::LoadSettingsState,
    RuntimeMethod::SaveSettingsState,
    RuntimeMethod::SourcePolicy,
    RuntimeMethod::Modules,
    RuntimeMethod::LogoscoreStatus,
    RuntimeMethod::BlockchainModuleReport,
    RuntimeMethod::StorageReport,
    RuntimeMethod::StorageSourceReport,
    RuntimeMethod::DeliveryReport,
    RuntimeMethod::DeliverySourceReport,
    RuntimeMethod::StorageExists,
    RuntimeMethod::StorageBackupSettings,
    RuntimeMethod::StorageRestoreSettings,
    RuntimeMethod::SocialMessagesFromStore,
];

impl RuntimeMethod {
    pub(crate) fn from_str(method: &str) -> Option<Self> {
        RUNTIME_METHODS
            .iter()
            .copied()
            .find(|candidate| candidate.as_str() == method)
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::DecodeTransactionSummary => "decodeTransactionSummary",
            Self::DecodeAccount => "decodeAccount",
            Self::ResolveAccountDecodeSession => "resolveAccountDecodeSession",
            Self::ResolveTransactionDecodeSession => "resolveTransactionDecodeSession",
            Self::DecodeEvent => "decodeEvent",
            Self::SpelIdl => "spelIdl",
            Self::ProgramFile => "programFile",
            Self::NormalizeProgramId => "normalizeProgramId",
            Self::Overview => "overview",
            Self::ChannelScan => "channelScan",
            Self::ChannelState => "channelState",
            Self::RawRpc => "rawRpc",
            Self::LocalWalletProfileStatus => "localWalletProfileStatus",
            Self::LocalWalletInstructionPreview => "localWalletInstructionPreview",
            Self::BedrockWalletBalance => "bedrockWalletBalance",
            Self::DetectWalletProfile => "detectWalletProfile",
            Self::LocalNodesStatus => "localNodesStatus",
            Self::LocalDevnetList => "localDevnetList",
            Self::LoadIdlState => "loadIdlState",
            Self::SaveIdlState => "saveIdlState",
            Self::LoadWalletState => "loadWalletState",
            Self::SaveWalletState => "saveWalletState",
            Self::LoadSettingsState => "loadSettingsState",
            Self::SaveSettingsState => "saveSettingsState",
            Self::SourcePolicy => "sourcePolicy",
            Self::Modules => "modules",
            Self::LogoscoreStatus => "logoscoreStatus",
            Self::BlockchainModuleReport => "blockchainModuleReport",
            Self::StorageReport => "storageReport",
            Self::StorageSourceReport => "storageSourceReport",
            Self::DeliveryReport => "deliveryReport",
            Self::DeliverySourceReport => "deliverySourceReport",
            Self::StorageExists => "storageExists",
            Self::StorageBackupSettings => "storageBackupSettings",
            Self::StorageRestoreSettings => "storageRestoreSettings",
            Self::SocialMessagesFromStore => "socialMessagesFromStore",
        }
    }
}

pub(crate) fn is_runtime_method(method: &str) -> bool {
    RuntimeMethod::from_str(method).is_some()
}

pub(super) fn try_handle(runtime: &Runtime, method: &str, args: Value) -> Result<Option<Value>> {
    let Some(method) = RuntimeMethod::from_str(method) else {
        return Ok(None);
    };
    let value = match method {
        RuntimeMethod::DecodeTransactionSummary => decode::decode_transaction_summary(args)?,
        RuntimeMethod::DecodeAccount => decode::decode_account(args)?,
        RuntimeMethod::ResolveAccountDecodeSession => decode::resolve_account_decode_session(args)?,
        RuntimeMethod::ResolveTransactionDecodeSession => {
            decode::resolve_transaction_decode_session(args)?
        }
        RuntimeMethod::DecodeEvent => decode::decode_event(args)?,
        RuntimeMethod::SpelIdl => decode::spel_idl(args)?,
        RuntimeMethod::ProgramFile => decode::program_file(args)?,
        RuntimeMethod::NormalizeProgramId => decode::normalize_program_id(args)?,
        RuntimeMethod::Overview => network::overview(runtime, args)?,
        RuntimeMethod::ChannelScan => network::channel_scan(runtime, args)?,
        RuntimeMethod::ChannelState => network::channel_state(runtime, args)?,
        RuntimeMethod::RawRpc => network::raw_rpc(runtime, args)?,
        RuntimeMethod::LocalWalletProfileStatus => wallet::local_wallet_profile_status(args)?,
        RuntimeMethod::LocalWalletInstructionPreview => {
            wallet::local_wallet_instruction_preview(args)?
        }
        RuntimeMethod::BedrockWalletBalance => wallet::bedrock_wallet_balance(runtime, args)?,
        RuntimeMethod::DetectWalletProfile => wallet::detect_wallet_profile()?,
        RuntimeMethod::LocalNodesStatus => local_nodes::local_nodes_status(args)?,
        RuntimeMethod::LocalDevnetList => local_nodes::local_devnet_list(args)?,
        RuntimeMethod::LoadIdlState => state::load_idl_state()?,
        RuntimeMethod::SaveIdlState => state::save_idl_state(args)?,
        RuntimeMethod::LoadWalletState => state::load_wallet_state()?,
        RuntimeMethod::SaveWalletState => state::save_wallet_state(args)?,
        RuntimeMethod::LoadSettingsState => state::load_settings_state()?,
        RuntimeMethod::SaveSettingsState => state::save_settings_state(args)?,
        RuntimeMethod::SourcePolicy => module_reports::source_policy()?,
        RuntimeMethod::Modules => module_reports::modules()?,
        RuntimeMethod::LogoscoreStatus => module_reports::logoscore_status()?,
        RuntimeMethod::BlockchainModuleReport => module_reports::blockchain_module_report(args)?,
        RuntimeMethod::StorageReport => module_reports::storage_report(args)?,
        RuntimeMethod::StorageSourceReport => module_reports::storage_source_report(runtime, args)?,
        RuntimeMethod::DeliveryReport => module_reports::delivery_report(args)?,
        RuntimeMethod::DeliverySourceReport => {
            module_reports::delivery_source_report(runtime, args)?
        }
        RuntimeMethod::StorageExists => storage::storage_exists(runtime, args)?,
        RuntimeMethod::StorageBackupSettings => storage::storage_backup_settings(runtime, args)?,
        RuntimeMethod::StorageRestoreSettings => storage::storage_restore_settings(runtime, args)?,
        RuntimeMethod::SocialMessagesFromStore => storage::social_messages_from_store(args)?,
    };
    Ok(Some(value))
}
