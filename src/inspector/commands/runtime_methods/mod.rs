use anyhow::Result;
use serde_json::Value;
use tokio::runtime::Runtime;

mod decode;
mod local_nodes;
mod module_reports;
mod network;
mod social;
mod state;
mod storage;
mod wallet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeMethodEntry {
    method: RuntimeMethod,
    name: &'static str,
}

impl RuntimeMethodEntry {
    pub(super) const fn new(method: RuntimeMethod, name: &'static str) -> Self {
        Self { method, name }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeMethod {
    DecodeTransactionSummary,
    DecodeAccount,
    ResolveAccountDecodeSession,
    ResolveTransactionDecodeSession,
    DecodeInstruction,
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
    CapabilitiesReport,
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
    SocialCommentPageFromStore,
    SocialCommentRowFromEvent,
    SocialTopicValid,
    AcceptedSharedIdlEntriesFromStore,
}

const RUNTIME_METHOD_CATALOGS: &[&[RuntimeMethodEntry]] = &[
    decode::METHOD_CATALOG,
    network::METHOD_CATALOG,
    wallet::METHOD_CATALOG,
    local_nodes::METHOD_CATALOG,
    state::METHOD_CATALOG,
    module_reports::METHOD_CATALOG,
    storage::METHOD_CATALOG,
    social::METHOD_CATALOG,
];

fn runtime_method_entries() -> impl Iterator<Item = &'static RuntimeMethodEntry> {
    RUNTIME_METHOD_CATALOGS
        .iter()
        .flat_map(|catalog| catalog.iter())
}

#[cfg(test)]
pub(crate) fn runtime_methods() -> impl Iterator<Item = RuntimeMethod> {
    runtime_method_entries().map(|entry| entry.method)
}

#[cfg(test)]
pub(crate) fn runtime_method_names() -> impl Iterator<Item = &'static str> {
    runtime_method_entries().map(|entry| entry.name)
}

impl RuntimeMethod {
    pub(crate) fn from_str(method: &str) -> Option<Self> {
        runtime_method_entries()
            .find(|entry| entry.name == method)
            .map(|entry| entry.method)
    }

    #[cfg(test)]
    pub(crate) fn as_str(self) -> &'static str {
        runtime_method_entries()
            .find(|entry| entry.method == self)
            .map_or("runtimeMethod", |entry| entry.name)
    }
}

pub(crate) fn handle(runtime: &Runtime, method: RuntimeMethod, args: Value) -> Result<Value> {
    let value = match method {
        RuntimeMethod::DecodeTransactionSummary => decode::decode_transaction_summary(args)?,
        RuntimeMethod::DecodeAccount => decode::decode_account(args)?,
        RuntimeMethod::ResolveAccountDecodeSession => decode::resolve_account_decode_session(args)?,
        RuntimeMethod::ResolveTransactionDecodeSession => {
            decode::resolve_transaction_decode_session(args)?
        }
        RuntimeMethod::DecodeInstruction => decode::decode_instruction(args)?,
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
        RuntimeMethod::CapabilitiesReport => module_reports::capabilities_report()?,
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
        RuntimeMethod::SocialMessagesFromStore => social::social_messages_from_store(args)?,
        RuntimeMethod::SocialCommentPageFromStore => social::social_comment_page_from_store(args)?,
        RuntimeMethod::SocialCommentRowFromEvent => social::social_comment_row_from_event(args)?,
        RuntimeMethod::SocialTopicValid => social::social_topic_valid(args)?,
        RuntimeMethod::AcceptedSharedIdlEntriesFromStore => {
            social::accepted_shared_idl_entries_from_store(args)?
        }
    };
    Ok(value)
}
