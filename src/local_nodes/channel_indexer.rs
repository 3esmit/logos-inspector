use std::{
    collections::BTreeSet,
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use tokio_util::sync::CancellationToken;

use crate::{
    inspection::NetworkScope,
    modules::logos_core::{
        BoxedModuleEventSubscription, LogoscoreCliRuntime, LogoscoreCliTransport,
        LogoscoreEventWatch, ModuleCall, ModuleTransportEvent, ModuleTransportKind,
        ScopedModuleTransport, SharedModuleTransport, dispatch_module_call,
        module_transport_event_from_watch_frame, normalize_module_call_value,
    },
    source_routing::channel_sources::{
        ChannelSourceConfig, ChannelSourceTarget, indexer, load_channel_source_configs,
    },
    support::{
        command_runner::{CommandControl, CommandTerminated},
        confirmation::ConfirmationPolicy,
        state_store::config_dir,
        time::now_millis,
    },
};

use super::{
    action_engine::LocalNodeReportProjector,
    action_workspace::{normalized_bedrock_endpoint, validate_channel_id},
    adapters::adapter_for,
    commands::{
        command_spec_for, ensure_module_loaded, execute_command_spec, operation_detail_from_value,
    },
    model::{
        LocalNodeConfigRecord, LocalNodeOperationReport, LocalNodeReport, LocalNodeStatus,
        LocalNodeSummary, LocalNodeTools, LocalNodesState, NodeAction, NodeKind,
        NodeLifecycleState, ToolStatus,
    },
    package,
    process::{process_group_has_live_members, spawn_detached, stop_process},
    runtime::{self, LogoscoreRuntimeProfile},
    workflow::normalized_profile,
};

const STATE_FILE: &str = "channel_indexers.json";
const STATE_VERSION: u32 = 1;
const OPERATION_HISTORY_LIMIT: usize = 100;
const STATUS_TIMEOUT: Duration = Duration::from_secs(2);
const INDEXER_MODULE: &str = "lez_indexer_module";
const INDEXER_NODE_STATUS_METHOD: &str = "nodeStatus";
const INDEXER_NODE_STATUS_SIGNATURE: &str = "nodeStatus()";
const INDEXER_NODE_ACTION_METHOD: &str = "nodeAction";
const INDEXER_NODE_ACTION_SIGNATURE: &str = "nodeAction(QString)";
const INDEXER_NODE_CHANGED_EVENT: &str = "nodeChanged";
const INDEXER_NODE_CHANGED_SIGNATURE: &str = "nodeChanged(QString)";
const INDEXER_LIFECYCLE_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(30);
const INDEXER_LIFECYCLE_EVENT_READ_INTERVAL: Duration = Duration::from_millis(250);
static INDEXER_LIFECYCLE_OPERATION_SERIAL: AtomicU64 = AtomicU64::new(0);
const BASECAMP_CORE_SERVICE_MODULE: &str = "core_service";
const BASECAMP_HOST_CAPABILITIES_METHOD: &str = "getHostCapabilities";
const BASECAMP_LOAD_INSTANCE_METHOD: &str = "loadModuleInstance";
const BASECAMP_INSTANCE_LOADED_METHOD: &str = "isModuleInstanceLoaded";
const BASECAMP_INDEXER_INSTANCE_PREFIX: &str = "indexer";
// LogosCore ModuleAddress allows instance IDs of at most 128 bytes in
// [A-Za-z0-9_-]. The naive form indexer-{sha256(scope)}-{channel64} is 137
// bytes and is rejected as "invalid runtime address". Keep the full Channel
// ID for operator readability; truncate the scope digest to 32 hex chars
// (128 bits), which still disambiguates network scopes.
const BASECAMP_INDEXER_SCOPE_KEY_PREFIX_LEN: usize = 32;
const BASECAMP_MAX_INSTANCE_ID_BYTES: usize = 128;
const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
const MAX_CONFIG_BYTES_USIZE: usize = 1024 * 1024;
const CONFIG_ROLE: &str = "Zone-owned Indexer";
const CONFIG_VALIDATION_SCOPE: &str =
    "JSON syntax, Zone identity, Bedrock source, and supported Indexer fields";
const CONFIG_ACTIVE_RUNTIME_REASON: &str =
    "Stop this Channel Indexer before editing its configuration.";
const CONFIG_CREDENTIALS_REASON: &str = "Bedrock credentials are not editable in Inspector. Remove them before opening this configuration.";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChannelIndexerActionRequest {
    pub(crate) action: NodeAction,
    pub(crate) network_scope: NetworkScope,
    pub(crate) channel_id: String,
    #[serde(default)]
    pub(crate) bedrock_endpoint: Option<String>,
    #[serde(default)]
    pub(crate) source_config_revision: Option<u64>,
    #[serde(default)]
    pub(crate) selected_sequencer_source_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChannelIndexerConfigRequest {
    pub(crate) network_scope: NetworkScope,
    pub(crate) channel_id: String,
    pub(crate) bedrock_endpoint: String,
    pub(crate) source_config_revision: u64,
    pub(crate) selected_sequencer_source_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChannelIndexerConfigSnapshot {
    pub(crate) profile: String,
    pub(crate) network_scope: NetworkScope,
    pub(crate) channel_id: String,
    pub(crate) source_config_revision: u64,
    pub(crate) selected_sequencer_source_id: String,
    pub(crate) node_label: String,
    pub(crate) config_path: String,
    pub(crate) config_role: String,
    pub(crate) format: String,
    pub(crate) raw_text: String,
    pub(crate) revision: String,
    pub(crate) editable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) blocked_reason: Option<String>,
    pub(crate) validation_scope: String,
    pub(crate) common_fields: Vec<ChannelIndexerConfigField>,
    pub(crate) protected_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChannelIndexerConfigField {
    pub(crate) path: String,
    pub(crate) label: String,
    pub(crate) section: String,
    pub(crate) kind: String,
    pub(crate) value: Value,
    pub(crate) required: bool,
    pub(crate) editable: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChannelIndexerConfigValidation {
    pub(crate) valid: bool,
    pub(crate) error: String,
    pub(crate) common_fields: Vec<ChannelIndexerConfigField>,
}

#[derive(Debug, Clone)]
struct SourceBinding {
    config_revision: u64,
    source_id: String,
    target_fingerprint: String,
}

#[derive(Debug, Clone)]
struct ChannelIndexerConfigContext {
    channel_id: String,
    bedrock_endpoint: String,
    binding: SourceBinding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChannelIndexerState {
    version: u32,
    #[serde(default)]
    records: Vec<ChannelIndexerRecord>,
}

impl Default for ChannelIndexerState {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            records: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChannelIndexerRecord {
    network_scope: NetworkScope,
    channel_id: String,
    source_config_revision: u64,
    selected_sequencer_source_id: String,
    selected_sequencer_target_fingerprint: String,
    bedrock_endpoint: String,
    runtime: LogoscoreRuntimeProfile,
    state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    indexed_block_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_error: Option<String>,
    #[serde(default)]
    operations: Vec<LocalNodeOperationReport>,
}

#[derive(Debug, Clone)]
struct ChannelIndexerStore {
    config_root: PathBuf,
}

impl ChannelIndexerStore {
    fn for_config_dir(config_root: &Path) -> Self {
        Self {
            config_root: config_root.to_path_buf(),
        }
    }

    fn load(&self) -> Result<ChannelIndexerState> {
        let path = self.state_path();
        if !path.is_file() {
            return Ok(ChannelIndexerState::default());
        }
        let text = fs::read_to_string(&path).with_context(|| {
            format!(
                "failed to read Channel Indexer state from {}",
                path.display()
            )
        })?;
        let state: ChannelIndexerState = serde_json::from_str(&text).with_context(|| {
            format!(
                "failed to parse Channel Indexer state from {}",
                path.display()
            )
        })?;
        validate_state(&state, &self.config_root)?;
        Ok(state)
    }

    fn save(&self, state: &ChannelIndexerState) -> Result<()> {
        validate_state(state, &self.config_root)?;
        let path = self.state_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create Channel Indexer state directory {}",
                    parent.display()
                )
            })?;
        }
        let text = serde_json::to_string_pretty(state)
            .context("failed to serialize Channel Indexer state")?;
        fs::write(&path, text).with_context(|| {
            format!(
                "failed to write Channel Indexer state to {}",
                path.display()
            )
        })
    }

    fn state_path(&self) -> PathBuf {
        self.config_root.join(STATE_FILE)
    }
}

pub(super) fn config_snapshot(
    config_root: &Path,
    profile: &str,
    request: &ChannelIndexerConfigRequest,
) -> Result<ChannelIndexerConfigSnapshot> {
    let context = config_context(request)?;
    config_snapshot_with_context(config_root, profile, request, &context)
}

fn config_snapshot_with_context(
    config_root: &Path,
    profile: &str,
    request: &ChannelIndexerConfigRequest,
    context: &ChannelIndexerConfigContext,
) -> Result<ChannelIndexerConfigSnapshot> {
    let path =
        channel_indexer_config_path(config_root, &request.network_scope, &context.channel_id)?;
    let state = ChannelIndexerStore::for_config_dir(config_root).load()?;
    let bytes = match read_optional_indexer_config(config_root, &path)? {
        Some(bytes) => bytes,
        None => default_indexer_config_bytes(context)?,
    };
    snapshot_from_config_bytes(profile, request, context, path, &state, &bytes)
}

pub(super) fn config_validate(
    request: &ChannelIndexerConfigRequest,
    text: &str,
) -> Result<ChannelIndexerConfigValidation> {
    let result: Result<Vec<ChannelIndexerConfigField>> = (|| {
        let context = config_context(request)?;
        let value = parse_indexer_config_text(text)?;
        validate_indexer_config_value(&value, &context)?;
        Ok(project_config_fields(&value))
    })();
    match result {
        Ok(common_fields) => Ok(ChannelIndexerConfigValidation {
            valid: true,
            error: String::new(),
            common_fields,
        }),
        Err(error) => Ok(ChannelIndexerConfigValidation {
            valid: false,
            error: error.to_string(),
            common_fields: Vec::new(),
        }),
    }
}

pub(super) fn save_config(
    config_root: &Path,
    profile: &str,
    request: &ChannelIndexerConfigRequest,
    text: &str,
    expected_revision: &str,
) -> Result<ChannelIndexerConfigSnapshot> {
    let context = config_context(request)?;
    save_config_with_context(
        config_root,
        profile,
        request,
        &context,
        text,
        expected_revision,
    )
}

fn save_config_with_context(
    config_root: &Path,
    profile: &str,
    request: &ChannelIndexerConfigRequest,
    context: &ChannelIndexerConfigContext,
    text: &str,
    expected_revision: &str,
) -> Result<ChannelIndexerConfigSnapshot> {
    let store = ChannelIndexerStore::for_config_dir(config_root);
    let state = store.load()?;
    if config_is_active(&state, &request.network_scope, &context.channel_id) {
        bail!(CONFIG_ACTIVE_RUNTIME_REASON);
    }
    let path =
        channel_indexer_config_path(config_root, &request.network_scope, &context.channel_id)?;
    let current = match read_optional_indexer_config(config_root, &path)? {
        Some(bytes) => bytes,
        None => default_indexer_config_bytes(context)?,
    };
    if revision_for(&current) != expected_revision {
        bail!("configuration changed on disk; reload it before saving");
    }
    let value = parse_indexer_config_text(text)?;
    validate_indexer_config_value(&value, context)?;
    let bytes = serde_json::to_vec_pretty(&value)
        .context("failed to serialize Channel Indexer configuration")?;
    write_indexer_config_bytes(config_root, &path, &bytes)?;
    config_snapshot_with_context(config_root, profile, request, context)
}

fn snapshot_from_config_bytes(
    profile: &str,
    request: &ChannelIndexerConfigRequest,
    context: &ChannelIndexerConfigContext,
    path: PathBuf,
    state: &ChannelIndexerState,
    bytes: &[u8],
) -> Result<ChannelIndexerConfigSnapshot> {
    let raw_text = std::str::from_utf8(bytes)
        .context("Channel Indexer configuration is not valid UTF-8")?
        .to_owned();
    let parsed = serde_json::from_slice::<Value>(bytes).ok();
    let contains_credentials = parsed
        .as_ref()
        .is_some_and(indexer_config_contains_bedrock_credentials);
    let blocked_reason = if contains_credentials {
        Some(CONFIG_CREDENTIALS_REASON.to_owned())
    } else if config_is_active(state, &request.network_scope, &context.channel_id) {
        Some(CONFIG_ACTIVE_RUNTIME_REASON.to_owned())
    } else {
        None
    };
    let common_fields = parsed
        .as_ref()
        .filter(|value| validate_indexer_config_value(value, context).is_ok())
        .map_or_else(Vec::new, project_config_fields);
    Ok(ChannelIndexerConfigSnapshot {
        profile: normalized_profile(profile).to_owned(),
        network_scope: request.network_scope.clone(),
        channel_id: context.channel_id.clone(),
        source_config_revision: context.binding.config_revision,
        selected_sequencer_source_id: context.binding.source_id.clone(),
        node_label: "Channel Indexer".to_owned(),
        config_path: path.display().to_string(),
        config_role: CONFIG_ROLE.to_owned(),
        format: "json".to_owned(),
        raw_text: if contains_credentials {
            String::new()
        } else {
            raw_text
        },
        revision: revision_for(bytes),
        editable: blocked_reason.is_none(),
        blocked_reason,
        validation_scope: CONFIG_VALIDATION_SCOPE.to_owned(),
        common_fields,
        protected_fields: vec![
            "Zone channel ID (derived from the selected Zone)".to_owned(),
            "Bedrock API URL (derived from the active Bedrock source)".to_owned(),
        ],
    })
}

pub(super) fn status(
    config_root: &Path,
    profile: &str,
    state: &LocalNodesState,
    base_runtime: Option<&LogoscoreRuntimeProfile>,
    projector: LocalNodeReportProjector,
    network_scope: &NetworkScope,
    channel_id: &str,
) -> Result<LocalNodeReport> {
    let channel_id = normalized_channel_id(channel_id)?;
    let store = ChannelIndexerStore::for_config_dir(config_root);
    let mut channel_state = store.load()?;
    let (report, changed) = build_report(
        profile,
        state,
        base_runtime,
        projector,
        &mut channel_state,
        network_scope,
        &channel_id,
    )?;
    if changed {
        store.save(&channel_state)?;
    }
    Ok(report)
}

pub(super) fn module_transport(
    network_scope: &NetworkScope,
    channel_id: &str,
    source_config_revision: u64,
    source_id: &str,
) -> Result<SharedModuleTransport> {
    let channel_id = normalized_channel_id(channel_id)?;
    let config_root = crate::support::state_store::config_dir()?;
    let state = ChannelIndexerStore::for_config_dir(&config_root).load()?;
    let configs = load_channel_source_configs()?;
    let runtime = runtime_for_module_source(
        &state,
        &configs,
        network_scope,
        &channel_id,
        source_config_revision,
        source_id,
    )?;
    Ok(Arc::new(LogoscoreCliTransport::fixed_runtime(runtime)))
}

/// Returns a transport that can address exactly one Basecamp Indexer instance.
///
/// The stable instance name is derived only from the network scope and Channel
/// ID. Source revision is intentionally excluded: configuration changes must
/// not redirect reads or lifecycle calls to another Zone instance.
pub(super) fn basecamp_module_transport(
    module_transport: SharedModuleTransport,
    network_scope: &NetworkScope,
    channel_id: &str,
) -> Result<SharedModuleTransport> {
    anyhow::ensure!(
        module_transport.kind() == ModuleTransportKind::Module,
        "Basecamp Channel Indexer requires the host module transport"
    );
    let channel_id = normalized_channel_id(channel_id)?;
    let instance_id = basecamp_instance_id(network_scope, &channel_id)?;
    Ok(Arc::new(ScopedModuleTransport::new(
        module_transport,
        INDEXER_MODULE,
        instance_id,
    )?))
}

pub(super) async fn basecamp_status(
    profile: &str,
    module_transport: &SharedModuleTransport,
    network_scope: &NetworkScope,
    channel_id: &str,
) -> Result<LocalNodeReport> {
    let config_root = config_dir()?;
    let configs = load_channel_source_configs()?;
    basecamp_status_with_configs(
        profile,
        &config_root,
        &configs,
        module_transport,
        network_scope,
        channel_id,
    )
    .await
}

/// Reads Channel Indexer configuration with Basecamp lifecycle editability.
///
/// The configuration is local Inspector state, but an active Basecamp instance
/// consumes it. Do not allow a user to rewrite it while that exact instance is
/// transitioning or running.
pub(super) async fn basecamp_config_snapshot(
    profile: &str,
    request: &ChannelIndexerConfigRequest,
    module_transport: &SharedModuleTransport,
) -> Result<ChannelIndexerConfigSnapshot> {
    let config_root = config_dir()?;
    let configs = load_channel_source_configs()?;
    basecamp_config_snapshot_with_configs(
        &config_root,
        profile,
        request,
        &configs,
        module_transport,
    )
    .await
}

async fn basecamp_config_snapshot_with_configs(
    config_root: &Path,
    profile: &str,
    request: &ChannelIndexerConfigRequest,
    configs: &[ChannelSourceConfig],
    module_transport: &SharedModuleTransport,
) -> Result<ChannelIndexerConfigSnapshot> {
    let context = config_context_from_configs(request, configs)?;
    let mut snapshot = config_snapshot_with_context(config_root, profile, request, &context)?;
    let status = basecamp_status_with_configs(
        profile,
        config_root,
        configs,
        module_transport,
        &request.network_scope,
        &snapshot.channel_id,
    )
    .await?;
    if snapshot.blocked_reason.is_none() && basecamp_config_is_active(&status)? {
        snapshot.editable = false;
        snapshot.blocked_reason = Some(CONFIG_ACTIVE_RUNTIME_REASON.to_owned());
    }
    Ok(snapshot)
}

/// Saves a Channel Indexer configuration after checking its exact Basecamp
/// instance is inactive. This never consults an unscoped/default module.
pub(super) async fn basecamp_save_config(
    profile: &str,
    request: &ChannelIndexerConfigRequest,
    text: &str,
    expected_revision: &str,
    confirmation: Option<&str>,
    module_transport: &SharedModuleTransport,
) -> Result<ChannelIndexerConfigSnapshot> {
    ConfirmationPolicy::LocalNodeAction.require(confirmation)?;
    let config_root = config_dir()?;
    let configs = load_channel_source_configs()?;
    basecamp_save_config_with_configs(
        &config_root,
        profile,
        request,
        text,
        expected_revision,
        &configs,
        module_transport,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn basecamp_save_config_with_configs(
    config_root: &Path,
    profile: &str,
    request: &ChannelIndexerConfigRequest,
    text: &str,
    expected_revision: &str,
    configs: &[ChannelSourceConfig],
    module_transport: &SharedModuleTransport,
) -> Result<ChannelIndexerConfigSnapshot> {
    let context = config_context_from_configs(request, configs)?;
    let before = basecamp_config_snapshot_with_configs(
        config_root,
        profile,
        request,
        configs,
        module_transport,
    )
    .await?;
    if !before.editable {
        bail!(
            "{}",
            before
                .blocked_reason
                .as_deref()
                .unwrap_or("Channel Indexer configuration cannot be edited")
        );
    }
    let saved = {
        let _state_lock = super::lifecycle::acquire_state_lock()?;
        save_config_with_context(
            config_root,
            profile,
            request,
            &context,
            text,
            expected_revision,
        )?
    };
    let mut snapshot = saved;
    let status = basecamp_status_with_configs(
        profile,
        config_root,
        configs,
        module_transport,
        &request.network_scope,
        &snapshot.channel_id,
    )
    .await?;
    if basecamp_config_is_active(&status)? {
        snapshot.editable = false;
        snapshot.blocked_reason = Some(CONFIG_ACTIVE_RUNTIME_REASON.to_owned());
    }
    Ok(snapshot)
}

fn basecamp_config_is_active(status: &LocalNodeReport) -> Result<bool> {
    let node = status
        .nodes
        .first()
        .context("Basecamp Channel Indexer status did not include its node")?;
    Ok(!matches!(
        node.run_state.as_str(),
        "stopped" | "uninitialized"
    ))
}

async fn basecamp_status_with_configs(
    profile: &str,
    config_root: &Path,
    configs: &[ChannelSourceConfig],
    module_transport: &SharedModuleTransport,
    network_scope: &NetworkScope,
    channel_id: &str,
) -> Result<LocalNodeReport> {
    let channel_id = normalized_channel_id(channel_id)?;
    let config_path = channel_indexer_config_path(config_root, network_scope, &channel_id)?;
    let instance_id = basecamp_instance_id(network_scope, &channel_id)?;
    let binding = source_binding_from_configs(configs, network_scope, &channel_id);
    let indexer_source = basecamp_indexer_source_configured(configs, network_scope, &channel_id);
    let source_ready = binding.is_ok() && indexer_source.is_ok();
    let source_detail = basecamp_source_detail(&binding, &indexer_source);

    if let Err(error) = ensure_basecamp_host_capabilities(module_transport).await {
        return Ok(basecamp_report(
            profile,
            config_root,
            &channel_id,
            &config_path,
            "needs_configuration",
            "unknown",
            format!(
                "Basecamp cannot host a scoped Channel Indexer instance: {error}; {source_detail}"
            ),
            Some(error.to_string()),
            Vec::new(),
            None,
        ));
    }

    let loaded = match basecamp_instance_loaded(module_transport, &instance_id).await {
        Ok(loaded) => loaded,
        Err(error) => {
            return Ok(basecamp_report(
                profile,
                config_root,
                &channel_id,
                &config_path,
                "needs_configuration",
                "unknown",
                format!(
                    "Basecamp could not inspect Channel Indexer instance `{instance_id}`: {error}; {source_detail}"
                ),
                Some(error.to_string()),
                Vec::new(),
                None,
            ));
        }
    };
    if !loaded {
        let mut actions = source_ready
            .then_some(NodeAction::Start)
            .into_iter()
            .collect::<Vec<_>>();
        if config_path.is_file() {
            actions.push(NodeAction::Purge);
        }
        return Ok(basecamp_report(
            profile,
            config_root,
            &channel_id,
            &config_path,
            "installed",
            "stopped",
            format!(
                "Basecamp Channel Indexer instance `{instance_id}` is not loaded; {source_detail}"
            ),
            None,
            actions,
            None,
        ));
    }

    let scoped =
        basecamp_module_transport(Arc::clone(module_transport), network_scope, &channel_id)?;
    match basecamp_indexer_snapshot(&scoped, &channel_id).await {
        Ok(snapshot) => Ok(basecamp_report(
            profile,
            config_root,
            &channel_id,
            &config_path,
            "installed",
            &snapshot.state,
            format!(
                "Basecamp Channel Indexer instance `{instance_id}` is {}; {source_detail}",
                snapshot.state
            ),
            None,
            basecamp_available_actions(&snapshot, source_ready),
            None,
        )),
        Err(error) => Ok(basecamp_report(
            profile,
            config_root,
            &channel_id,
            &config_path,
            "installed",
            "unknown",
            format!(
                "Basecamp Channel Indexer instance `{instance_id}` is loaded, but its lifecycle state is unavailable: {error}; {source_detail}"
            ),
            Some(error.to_string()),
            Vec::new(),
            None,
        )),
    }
}

fn basecamp_indexer_source_configured(
    configs: &[ChannelSourceConfig],
    network_scope: &NetworkScope,
    channel_id: &str,
) -> Result<()> {
    let config = configs
        .iter()
        .find(|config| config.network_scope == *network_scope && config.channel_id == channel_id)
        .context("Channel source configuration is unavailable for this Channel")?;
    let source = config
        .indexer_source
        .as_ref()
        .context("configure the Channel-owned Indexer source before starting Indexer")?;
    anyhow::ensure!(
        matches!(&source.target, ChannelSourceTarget::Module { module_id } if module_id == indexer::MODULE_ID),
        "the configured Indexer source is not the Channel-owned Indexer module"
    );
    Ok(())
}

fn basecamp_source_detail(binding: &Result<SourceBinding>, indexer_source: &Result<()>) -> String {
    match (binding, indexer_source) {
        (Ok(binding), Ok(())) => binding_detail(binding),
        (Err(binding), Ok(())) => format!("Selected Sequencer binding unavailable: {binding}"),
        (Ok(_), Err(indexer_source)) => format!("Indexer source unavailable: {indexer_source}"),
        (Err(binding), Err(indexer_source)) => {
            format!(
                "Selected Sequencer binding unavailable: {binding}; Indexer source unavailable: {indexer_source}"
            )
        }
    }
}

fn basecamp_instance_id(network_scope: &NetworkScope, channel_id: &str) -> Result<String> {
    let scope_key = network_scope_key(network_scope)?;
    let channel_id = normalized_channel_id(channel_id)?;
    let scope_prefix_len = BASECAMP_INDEXER_SCOPE_KEY_PREFIX_LEN.min(scope_key.len());
    let scope_prefix = scope_key
        .get(..scope_prefix_len)
        .context("Channel Indexer network scope key is shorter than expected")?;
    let instance_id = format!("{BASECAMP_INDEXER_INSTANCE_PREFIX}-{scope_prefix}-{channel_id}");
    anyhow::ensure!(
        instance_id.len() <= BASECAMP_MAX_INSTANCE_ID_BYTES,
        "Channel Indexer Basecamp instance id exceeds the runtime address limit ({} > {BASECAMP_MAX_INSTANCE_ID_BYTES})",
        instance_id.len()
    );
    anyhow::ensure!(
        instance_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'),
        "Channel Indexer Basecamp instance id contains characters outside the runtime address allowlist"
    );
    Ok(instance_id)
}

async fn ensure_basecamp_host_capabilities(module_transport: &SharedModuleTransport) -> Result<()> {
    let capabilities = basecamp_core_service_value(
        module_transport,
        BASECAMP_HOST_CAPABILITIES_METHOD,
        Vec::new(),
    )
    .await?;
    anyhow::ensure!(
        capabilities.get("schema").and_then(Value::as_str) == Some("logos.basecamp_host")
            && capabilities.get("version").and_then(Value::as_u64) == Some(1),
        "Basecamp host returned an unsupported scoped-instance capability schema"
    );
    for capability in [
        "scoped_module_instances",
        "direct_scoped_clients",
        "direct_scoped_events",
    ] {
        anyhow::ensure!(
            capabilities.get(capability).and_then(Value::as_bool) == Some(true),
            "Basecamp host does not support `{capability}`"
        );
    }
    Ok(())
}

async fn basecamp_instance_loaded(
    module_transport: &SharedModuleTransport,
    instance_id: &str,
) -> Result<bool> {
    let value = basecamp_core_service_value(
        module_transport,
        BASECAMP_INSTANCE_LOADED_METHOD,
        vec![json!(INDEXER_MODULE), json!(instance_id)],
    )
    .await?;
    validate_basecamp_instance_response(&value, BASECAMP_INSTANCE_LOADED_METHOD, instance_id)?;
    value
        .get("loaded")
        .and_then(Value::as_bool)
        .context("Basecamp host did not report whether the Channel Indexer instance is loaded")
}

async fn load_basecamp_instance(
    module_transport: &SharedModuleTransport,
    instance_id: &str,
) -> Result<()> {
    let value = basecamp_core_service_value(
        module_transport,
        BASECAMP_LOAD_INSTANCE_METHOD,
        vec![json!(INDEXER_MODULE), json!(instance_id)],
    )
    .await?;
    validate_basecamp_instance_response(&value, BASECAMP_LOAD_INSTANCE_METHOD, instance_id)
}

async fn basecamp_core_service_value(
    module_transport: &SharedModuleTransport,
    method: &str,
    args: Vec<Value>,
) -> Result<Value> {
    anyhow::ensure!(
        module_transport.kind() == ModuleTransportKind::Module,
        "Basecamp scoped Channel Indexer requires the host module transport"
    );
    let call = ModuleCall::new(
        ModuleTransportKind::Module,
        BASECAMP_CORE_SERVICE_MODULE,
        method,
        args,
    )?;
    let value = dispatch_module_call(module_transport.as_ref(), call)
        .await?
        .into_value();
    normalize_module_call_value(BASECAMP_CORE_SERVICE_MODULE, method, value)
}

fn validate_basecamp_instance_response(
    value: &Value,
    method: &str,
    instance_id: &str,
) -> Result<()> {
    anyhow::ensure!(
        value.get("status").and_then(Value::as_str) == Some("ok")
            && value.get("module_name").and_then(Value::as_str) == Some(INDEXER_MODULE)
            && value.get("instance_id").and_then(Value::as_str) == Some(instance_id),
        "Basecamp core_service.{method} returned an invalid Channel Indexer instance response"
    );
    Ok(())
}

async fn basecamp_indexer_snapshot(
    scoped_transport: &SharedModuleTransport,
    channel_id: &str,
) -> Result<IndexerLifecycleSnapshot> {
    let call = ModuleCall::new(
        ModuleTransportKind::Module,
        INDEXER_MODULE,
        INDEXER_NODE_STATUS_METHOD,
        Vec::new(),
    )?;
    let value = dispatch_module_call(scoped_transport.as_ref(), call)
        .await?
        .into_value();
    let value = normalize_module_call_value(INDEXER_MODULE, INDEXER_NODE_STATUS_METHOD, value)?;
    let snapshot = IndexerLifecycleSnapshot::parse(&value)?;
    if let Some(scoped_channel_id) = snapshot.channel_id.as_deref() {
        anyhow::ensure!(
            scoped_channel_id == channel_id,
            "Basecamp Indexer nodeStatus is scoped to a different Channel"
        );
    }
    Ok(snapshot)
}

fn basecamp_available_actions(
    snapshot: &IndexerLifecycleSnapshot,
    source_ready: bool,
) -> Vec<NodeAction> {
    let mut actions = Vec::new();
    if source_ready && snapshot.supports_action("start") {
        actions.push(NodeAction::Start);
    }
    if snapshot.supports_action("stop") {
        actions.push(NodeAction::Stop);
    }
    if matches!(snapshot.state.as_str(), "uninitialized" | "stopped") {
        actions.push(NodeAction::Purge);
    }
    actions
}

#[allow(clippy::too_many_arguments)]
fn basecamp_report(
    profile: &str,
    config_root: &Path,
    channel_id: &str,
    config_path: &Path,
    install_state: &str,
    run_state: &str,
    detail: String,
    indexer_error: Option<String>,
    available_actions: Vec<NodeAction>,
    operation: Option<LocalNodeOperationReport>,
) -> LocalNodeReport {
    let mut node = empty_indexer_status();
    node.label = "Channel Indexer".to_owned();
    node.install_state = install_state.to_owned();
    node.run_state = run_state.to_owned();
    node.ownership = "inspector_managed".to_owned();
    node.config_path = Some(config_path.display().to_string());
    node.managed_channel_id = Some(channel_id.to_owned());
    node.indexer_state = Some(run_state.to_owned());
    node.indexer_error = indexer_error;
    node.available_actions = available_actions;
    node.detail = detail;
    node.last_action = operation.clone();
    let installed = usize::from(install_state == "installed");
    let running = usize::from(run_state == "running");
    let needs_configuration = usize::from(install_state == "needs_configuration");
    LocalNodeReport {
        profile: normalized_profile(profile).to_owned(),
        mode: super::presentation::mode_for_profile(profile).to_owned(),
        available_network_actions: Vec::new(),
        available_runtime_actions: Vec::new(),
        primary_problem: None,
        active_devnet: None,
        workspace_root: config_root.display().to_string(),
        summary: LocalNodeSummary {
            total: 1,
            installed,
            running,
            needs_configuration,
        },
        nodes: vec![node],
        operations: operation.into_iter().collect(),
        tools: LocalNodeTools {
            logoscore: ToolStatus {
                available: true,
                command: "Basecamp host".to_owned(),
                path: None,
            },
            lgpd: ToolStatus {
                available: false,
                command: "lgpd".to_owned(),
                path: None,
            },
            lgpm: ToolStatus {
                available: false,
                command: "lgpm".to_owned(),
                path: None,
            },
        },
        runtime: super::runtime::LogoscoreRuntimeStatus {
            ownership: "basecamp_host".to_owned(),
            run_state: "running".to_owned(),
            id: Some("basecamp".to_owned()),
            binary_path: None,
            config_dir: None,
            modules_dir: None,
            persistence_path: None,
            process_id: None,
            service_unit: None,
            detail: "Channel Indexers are owned by Basecamp".to_owned(),
        },
    }
}

pub(super) async fn basecamp_action(
    profile: &str,
    request: ChannelIndexerActionRequest,
    confirmation: Option<&str>,
    module_transport: &SharedModuleTransport,
) -> Result<LocalNodeReport> {
    ConfirmationPolicy::LocalNodeAction.require(confirmation)?;
    let config_root = config_dir()?;
    let configs = load_channel_source_configs()?;
    basecamp_action_with_configs(profile, &config_root, &configs, request, module_transport).await
}

async fn basecamp_action_with_configs(
    profile: &str,
    config_root: &Path,
    configs: &[ChannelSourceConfig],
    request: ChannelIndexerActionRequest,
    module_transport: &SharedModuleTransport,
) -> Result<LocalNodeReport> {
    let channel_id = normalized_channel_id(&request.channel_id)?;
    if !matches!(
        request.action,
        NodeAction::Start | NodeAction::Stop | NodeAction::Purge
    ) {
        bail!("Channel Indexer only supports Start, Stop, and Reset data actions");
    }

    let result = match request.action {
        NodeAction::Start => {
            basecamp_start_indexer(
                config_root,
                configs,
                &request,
                &channel_id,
                module_transport,
            )
            .await
        }
        NodeAction::Stop => basecamp_stop_indexer(&request, &channel_id, module_transport).await,
        NodeAction::Purge => {
            basecamp_purge_indexer(
                config_root,
                configs,
                &request,
                &channel_id,
                module_transport,
            )
            .await
        }
        _ => unreachable!("Channel Indexer action was validated"),
    };
    let (status, detail) = match result {
        Ok(detail) => (
            match request.action {
                NodeAction::Start => "running",
                NodeAction::Stop => "stopped",
                NodeAction::Purge => "purged",
                _ => unreachable!("Channel Indexer action was validated"),
            },
            detail,
        ),
        Err(error) => ("failed", error.to_string()),
    };
    let operation = operation_report(request.action, status, detail);
    let mut report = basecamp_status_with_configs(
        profile,
        config_root,
        configs,
        module_transport,
        &request.network_scope,
        &channel_id,
    )
    .await?;
    report.operations = vec![operation.clone()];
    if let Some(node) = report.nodes.first_mut() {
        node.last_action = Some(operation);
    }
    Ok(report)
}

async fn basecamp_start_indexer(
    config_root: &Path,
    configs: &[ChannelSourceConfig],
    request: &ChannelIndexerActionRequest,
    channel_id: &str,
    module_transport: &SharedModuleTransport,
) -> Result<String> {
    basecamp_indexer_source_configured(configs, &request.network_scope, channel_id)?;
    let binding = requested_source_binding_from_configs(request, channel_id, configs)?;
    let endpoint = normalized_bedrock_endpoint(
        request
            .bedrock_endpoint
            .as_deref()
            .context("Indexer Bedrock endpoint is required")?,
    )?;
    let context = ChannelIndexerConfigContext {
        channel_id: channel_id.to_owned(),
        bedrock_endpoint: endpoint,
        binding,
    };
    let config_path =
        ensure_basecamp_indexer_config(config_root, &request.network_scope, &context)?;
    ensure_basecamp_host_capabilities(module_transport).await?;
    let instance_id = basecamp_instance_id(&request.network_scope, channel_id)?;
    if !basecamp_instance_loaded(module_transport, &instance_id).await? {
        load_basecamp_instance(module_transport, &instance_id).await?;
    }
    let scoped = basecamp_module_transport(
        Arc::clone(module_transport),
        &request.network_scope,
        channel_id,
    )?;
    execute_basecamp_indexer_lifecycle_action(
        &scoped,
        &instance_id,
        channel_id,
        NodeAction::Start,
        Some(&config_path),
    )
    .await
}

async fn basecamp_stop_indexer(
    request: &ChannelIndexerActionRequest,
    channel_id: &str,
    module_transport: &SharedModuleTransport,
) -> Result<String> {
    ensure_basecamp_host_capabilities(module_transport).await?;
    let instance_id = basecamp_instance_id(&request.network_scope, channel_id)?;
    if !basecamp_instance_loaded(module_transport, &instance_id).await? {
        return Ok(format!(
            "Basecamp Channel Indexer instance `{instance_id}` is already stopped"
        ));
    }
    let scoped = basecamp_module_transport(
        Arc::clone(module_transport),
        &request.network_scope,
        channel_id,
    )?;
    execute_basecamp_indexer_lifecycle_action(
        &scoped,
        &instance_id,
        channel_id,
        NodeAction::Stop,
        None,
    )
    .await
}

async fn basecamp_purge_indexer(
    config_root: &Path,
    _configs: &[ChannelSourceConfig],
    request: &ChannelIndexerActionRequest,
    channel_id: &str,
    module_transport: &SharedModuleTransport,
) -> Result<String> {
    let config_path =
        persisted_basecamp_indexer_config(config_root, &request.network_scope, channel_id)?;
    ensure_basecamp_host_capabilities(module_transport).await?;
    let instance_id = basecamp_instance_id(&request.network_scope, channel_id)?;
    if !basecamp_instance_loaded(module_transport, &instance_id).await? {
        load_basecamp_instance(module_transport, &instance_id).await?;
    }
    let scoped = basecamp_module_transport(
        Arc::clone(module_transport),
        &request.network_scope,
        channel_id,
    )?;
    let snapshot = basecamp_indexer_snapshot(&scoped, channel_id).await?;
    anyhow::ensure!(
        matches!(snapshot.state.as_str(), "uninitialized" | "stopped"),
        "stop the Basecamp Channel Indexer before resetting its data"
    );
    let call = ModuleCall::new(
        ModuleTransportKind::Module,
        INDEXER_MODULE,
        "reset_storage",
        vec![Value::String(config_path)],
    )?;
    let value = dispatch_module_call(scoped.as_ref(), call)
        .await?
        .into_value();
    let value = normalize_module_call_value(INDEXER_MODULE, "reset_storage", value)?;
    anyhow::ensure!(
        value.as_i64() == Some(0),
        "lez_indexer_module.reset_storage failed with OperationStatus {}",
        value
    );
    Ok(format!(
        "Reset data for Basecamp Channel Indexer `{channel_id}`"
    ))
}

fn persisted_basecamp_indexer_config(
    config_root: &Path,
    network_scope: &NetworkScope,
    channel_id: &str,
) -> Result<String> {
    let channel_id = normalized_channel_id(channel_id)?;
    let path = channel_indexer_config_path(config_root, network_scope, &channel_id)?;
    let bytes = read_optional_indexer_config(config_root, &path)?.with_context(|| {
        format!("Basecamp Channel Indexer configuration is unavailable for Channel `{channel_id}`")
    })?;
    let text = std::str::from_utf8(&bytes)
        .context("Basecamp Channel Indexer configuration is not valid UTF-8")?;
    let value = parse_indexer_config_text(text)?;
    let configured_channel = required_config_string(
        value
            .as_object()
            .and_then(|object| object.get("channel_id")),
        "Zone channel ID",
    )?;
    anyhow::ensure!(
        normalized_channel_id(configured_channel)? == channel_id,
        "Basecamp Channel Indexer configuration belongs to a different Channel"
    );
    Ok(path.display().to_string())
}

fn ensure_basecamp_indexer_config(
    config_root: &Path,
    network_scope: &NetworkScope,
    context: &ChannelIndexerConfigContext,
) -> Result<String> {
    let path = channel_indexer_config_path(config_root, network_scope, &context.channel_id)?;
    anyhow::ensure!(
        path.is_absolute(),
        "Basecamp Channel Indexer configuration path must be absolute"
    );
    match read_optional_indexer_config(config_root, &path)? {
        Some(bytes) => {
            let text = std::str::from_utf8(&bytes)
                .context("Channel Indexer configuration is not valid UTF-8")?;
            let value = parse_indexer_config_text(text)?;
            validate_indexer_config_value(&value, context).context(
                "Channel Indexer configuration is invalid; open Zone Sources and repair it before starting",
            )?;
        }
        None => {
            write_indexer_config_bytes(config_root, &path, &default_indexer_config_bytes(context)?)?
        }
    }
    Ok(path.display().to_string())
}

async fn execute_basecamp_indexer_lifecycle_action(
    scoped_transport: &SharedModuleTransport,
    scoped_instance_id: &str,
    channel_id: &str,
    action: NodeAction,
    config_path: Option<&str>,
) -> Result<String> {
    let snapshot = basecamp_indexer_snapshot(scoped_transport, channel_id).await?;
    let (action_name, expected_transition, expected_terminal, parameters) =
        basecamp_indexer_lifecycle_action_parameters(action, config_path)?;
    validate_basecamp_indexer_snapshot_for_action(
        &snapshot,
        channel_id,
        action_name,
        expected_transition,
    )?;
    let subscription = scoped_transport
        .subscribe_module_event(INDEXER_MODULE, INDEXER_NODE_CHANGED_EVENT)
        .context("Basecamp host cannot subscribe to Channel Indexer nodeChanged confirmation")?;
    let serial = INDEXER_LIFECYCLE_OPERATION_SERIAL.fetch_add(1, Ordering::Relaxed);
    let operation_id = format!(
        "logos-inspector-indexer-{action_name}-{}-{serial}",
        now_millis()
    );
    let request = json!({
        "schema": "logos.managed_node_lifecycle.command",
        "version": 1,
        "operation_id": operation_id,
        "action": action_name,
        "expected": {
            "instance_id": snapshot.instance_id,
            "epoch": snapshot.epoch,
            "sequence": snapshot.sequence,
        },
        "parameters": parameters,
    });
    let call = ModuleCall::new(
        ModuleTransportKind::Module,
        INDEXER_MODULE,
        INDEXER_NODE_ACTION_METHOD,
        vec![Value::String(request.to_string())],
    )?;
    let value = dispatch_module_call(scoped_transport.as_ref(), call)
        .await?
        .into_value();
    let acknowledgement =
        normalize_module_call_value(INDEXER_MODULE, INDEXER_NODE_ACTION_METHOD, value)?;
    validate_indexer_lifecycle_acknowledgement(
        &acknowledgement,
        &snapshot,
        &operation_id,
        expected_transition,
    )?;
    let terminal = wait_for_basecamp_indexer_lifecycle_terminal_event(
        subscription,
        scoped_instance_id,
        &snapshot,
        channel_id,
        &operation_id,
        action_name,
        expected_transition,
        expected_terminal,
    )
    .await?;
    Ok(format!(
        "Basecamp V1 nodeChanged confirmed {action_name} for Channel `{channel_id}` at lifecycle sequence {}",
        terminal.sequence
    ))
}

fn basecamp_indexer_lifecycle_action_parameters(
    action: NodeAction,
    config_path: Option<&str>,
) -> Result<(&'static str, &'static str, &'static str, Value)> {
    match action {
        NodeAction::Start => {
            let config_path =
                config_path.context("Channel Indexer start configuration is required")?;
            anyhow::ensure!(
                Path::new(config_path).is_absolute(),
                "Basecamp Channel Indexer configuration path must be absolute"
            );
            Ok((
                "start",
                "starting",
                "running",
                json!({ "config_path": config_path }),
            ))
        }
        NodeAction::Stop => Ok(("stop", "stopping", "stopped", json!({}))),
        _ => bail!(
            "Basecamp Channel Indexer V1 lifecycle does not support {}",
            action.as_str()
        ),
    }
}

fn validate_basecamp_indexer_snapshot_for_action(
    snapshot: &IndexerLifecycleSnapshot,
    channel_id: &str,
    action: &str,
    expected_transition: &str,
) -> Result<()> {
    anyhow::ensure!(
        snapshot.supports_action(action),
        "Basecamp Indexer nodeStatus does not allow `{action}` in its current state `{}`",
        snapshot.state
    );
    if let Some(snapshot_channel_id) = snapshot.channel_id.as_deref() {
        anyhow::ensure!(
            snapshot_channel_id == channel_id,
            "Basecamp Indexer nodeStatus is scoped to a different Channel"
        );
    }
    anyhow::ensure!(
        snapshot.state != expected_transition,
        "Basecamp Indexer nodeStatus is already transitioning"
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn wait_for_basecamp_indexer_lifecycle_terminal_event(
    mut subscription: BoxedModuleEventSubscription,
    scoped_instance_id: &str,
    snapshot: &IndexerLifecycleSnapshot,
    channel_id: &str,
    operation_id: &str,
    action: &str,
    expected_transition: &str,
    expected_terminal: &str,
) -> Result<IndexerLifecycleSnapshot> {
    let scoped_instance_id = scoped_instance_id.to_owned();
    let initial_instance_id = snapshot.instance_id.clone();
    let initial_epoch = snapshot.epoch;
    let initial_sequence = snapshot.sequence;
    let channel_id = channel_id.to_owned();
    let operation_id = operation_id.to_owned();
    let action = action.to_owned();
    let expected_transition = expected_transition.to_owned();
    let expected_terminal = expected_terminal.to_owned();
    tokio::task::spawn_blocking(move || {
        let deadline = Instant::now() + INDEXER_LIFECYCLE_CONFIRMATION_TIMEOUT;
        let mut accepted = false;
        let mut last_sequence = initial_sequence;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let transport = subscription.next_within(remaining)?.with_context(|| {
                "Basecamp Channel Indexer did not emit nodeChanged before lifecycle confirmation timeout"
            })?;
            anyhow::ensure!(
                transport.module() == INDEXER_MODULE
                    && transport.event() == INDEXER_NODE_CHANGED_EVENT
                    && transport.instance_id() == Some(scoped_instance_id.as_str()),
                "Basecamp Channel Indexer lifecycle subscription received an unexpected module instance event"
            );
            let event = IndexerLifecycleEvent::from_transport_event(&transport)?;
            if event.operation_id.as_deref() != Some(operation_id.as_str())
                || event.instance_id != initial_instance_id
            {
                continue;
            }
            anyhow::ensure!(
                event.sequence > last_sequence && event.epoch >= initial_epoch,
                "Basecamp Indexer nodeChanged event has a stale lifecycle cursor"
            );
            anyhow::ensure!(
                event.action == action,
                "Basecamp Indexer nodeChanged event has an unexpected action"
            );
            anyhow::ensure!(
                event.status.instance_id == event.instance_id
                    && event.status.epoch == event.epoch
                    && event.status.sequence == event.sequence,
                "Basecamp Indexer nodeChanged event has an inconsistent status snapshot"
            );
            last_sequence = event.sequence;
            match event.phase.as_str() {
                "accepted" => {
                    anyhow::ensure!(
                        !accepted
                            && event.outcome == "accepted"
                            && event.status.state == expected_transition,
                        "Basecamp Indexer nodeChanged accepted event has an invalid lifecycle state"
                    );
                    if let Some(event_channel_id) = event.channel_id.as_deref() {
                        anyhow::ensure!(
                            event_channel_id == channel_id,
                            "Basecamp Indexer nodeChanged accepted event is scoped to a different Channel"
                        );
                    }
                    accepted = true;
                }
                "settled" => {
                    if event.outcome != "succeeded" {
                        let detail = event
                            .error
                            .as_ref()
                            .map(|error| format!("{}: {}", error.code, error.message))
                            .unwrap_or_else(|| format!("outcome `{}`", event.outcome));
                        bail!(
                            "Indexer V1 nodeChanged terminal event reported failure: {detail}"
                        );
                    }
                    anyhow::ensure!(
                        accepted && event.status.state == expected_terminal,
                        "Basecamp Indexer nodeChanged terminal event did not confirm the requested action"
                    );
                    anyhow::ensure!(
                        event.channel_id.as_deref() == Some(channel_id.as_str())
                            && event.status.channel_id.as_deref() == Some(channel_id.as_str()),
                        "Basecamp Indexer nodeChanged terminal event is scoped to a different Channel"
                    );
                    return Ok(event.status);
                }
                _ => bail!("Basecamp Indexer nodeChanged event has an unsupported phase"),
            }
        }
    })
    .await
    .context("Basecamp Channel Indexer lifecycle event worker failed")?
}

fn runtime_for_module_source(
    state: &ChannelIndexerState,
    configs: &[ChannelSourceConfig],
    network_scope: &NetworkScope,
    channel_id: &str,
    source_config_revision: u64,
    source_id: &str,
) -> Result<crate::modules::logos_core::LogoscoreCliRuntime> {
    let record = find_record(state, network_scope, channel_id)
        .context("no isolated Channel Indexer is configured for this Channel")?;
    if !record.runtime.is_running() || record.state == "stopped" {
        bail!("isolated Channel Indexer is not running for this Channel");
    }
    let config = configs
        .iter()
        .find(|config| config.network_scope == *network_scope && config.channel_id == channel_id)
        .context("Channel source configuration is unavailable for this Channel")?;
    if config.config_revision != source_config_revision
        || record.source_config_revision != source_config_revision
    {
        bail!("Channel source configuration changed since this Indexer started");
    }
    let source = config
        .indexer_source
        .as_ref()
        .filter(|source| source.source_id == source_id)
        .context("configured Indexer source does not match this Channel runtime")?;
    if !matches!(
        &source.target,
        ChannelSourceTarget::Module { module_id } if module_id == indexer::MODULE_ID
    ) {
        bail!("configured Indexer source is not the Channel-owned Indexer module");
    }
    let binding = source_binding_from_configs(configs, network_scope, channel_id)?;
    if !record_matches_binding(record, &binding) {
        bail!("selected Sequencer binding changed since this Indexer started");
    }
    record.runtime.cli_runtime()
}

pub(super) fn apply(
    config_root: &Path,
    profile: &str,
    state: &LocalNodesState,
    base_runtime: Option<&LogoscoreRuntimeProfile>,
    projector: LocalNodeReportProjector,
    request: ChannelIndexerActionRequest,
    control: Option<&CommandControl>,
) -> Result<LocalNodeReport> {
    let channel_id = normalized_channel_id(&request.channel_id)?;
    if !matches!(
        request.action,
        NodeAction::Start | NodeAction::Stop | NodeAction::Purge
    ) {
        bail!("Channel Indexer only supports Start, Stop, and Reset data actions");
    }
    if let Some(control) = control {
        control.check_active()?;
    }

    let store = ChannelIndexerStore::for_config_dir(config_root);
    let mut channel_state = store.load()?;
    let operation = match request.action {
        NodeAction::Start => start(
            &mut channel_state,
            StartContext {
                config_root,
                profile,
                state,
                base_runtime,
                request: &request,
                channel_id: &channel_id,
                control,
            },
        ),
        NodeAction::Stop => stop(
            &mut channel_state,
            &request.network_scope,
            &channel_id,
            control,
        ),
        NodeAction::Purge => purge(
            &mut channel_state,
            &request.network_scope,
            &channel_id,
            control,
        ),
        _ => unreachable!("Channel Indexer action was validated"),
    };
    let operation = match operation {
        Ok(outcome) => operation_report(request.action, outcome.status, outcome.detail),
        Err(error) if is_control_interruption(&error) => return Err(error),
        Err(error) => operation_report(request.action, "failed", error.to_string()),
    };
    let record = find_record_mut(&mut channel_state, &request.network_scope, &channel_id);
    if let Some(record) = record {
        push_operation(&mut record.operations, operation.clone());
    }
    store.save(&channel_state)?;

    let (mut report, changed) = build_report(
        profile,
        state,
        base_runtime,
        projector,
        &mut channel_state,
        &request.network_scope,
        &channel_id,
    )?;
    if changed {
        store.save(&channel_state)?;
    }
    if find_record(&channel_state, &request.network_scope, &channel_id).is_none() {
        report.operations = vec![operation];
    }
    Ok(report)
}

struct StartContext<'a> {
    config_root: &'a Path,
    profile: &'a str,
    state: &'a LocalNodesState,
    base_runtime: Option<&'a LogoscoreRuntimeProfile>,
    request: &'a ChannelIndexerActionRequest,
    channel_id: &'a str,
    control: Option<&'a CommandControl>,
}

fn start(
    channel_state: &mut ChannelIndexerState,
    context: StartContext<'_>,
) -> Result<ActionOutcome> {
    let package = package_prerequisite(context.state, context.profile, context.base_runtime);
    if !package.installed {
        return Ok(ActionOutcome::needs_configuration(package.detail));
    }
    if let Some(detail) = legacy_indexer_problem(context.state, context.profile) {
        return Ok(ActionOutcome::needs_configuration(detail));
    }
    let binding = requested_source_binding(context.request, context.channel_id)?;
    let endpoint = normalized_bedrock_endpoint(
        context
            .request
            .bedrock_endpoint
            .as_deref()
            .context("Indexer Bedrock endpoint is required")?,
    )?;
    let base_runtime = context
        .base_runtime
        .context("connect a local LogosCore runtime before starting a Channel Indexer")?;
    if base_runtime.is_attached() {
        let modules_dir = base_runtime.channel_indexer_modules_dir()?;
        let authority = base_runtime.package_install_authority()?;
        if !package::verify_installed_indexer_module(
            Path::new(&modules_dir),
            &authority,
            context.control,
        )? {
            return Ok(ActionOutcome::needs_configuration(
                "install lez_indexer_module into the modules directory used by the local LogosCore service",
            ));
        }
    }
    let scope_key = network_scope_key(&context.request.network_scope)?;

    let record = match find_record_mut(
        channel_state,
        &context.request.network_scope,
        context.channel_id,
    ) {
        Some(record) => {
            if record.runtime.is_running() && record.state != "stopped" {
                return Ok(ActionOutcome::needs_configuration(
                    "this Channel Indexer is already running; stop it before starting it again",
                ));
            }
            if !record.runtime.is_running() {
                record.runtime = LogoscoreRuntimeProfile::create_channel_indexer(
                    context.config_root,
                    &scope_key,
                    context.channel_id,
                    base_runtime,
                )?;
            }
            update_record_binding(record, binding, endpoint);
            record
        }
        None => {
            let runtime = LogoscoreRuntimeProfile::create_channel_indexer(
                context.config_root,
                &scope_key,
                context.channel_id,
                base_runtime,
            )?;
            channel_state.records.push(ChannelIndexerRecord {
                network_scope: context.request.network_scope.clone(),
                channel_id: context.channel_id.to_owned(),
                source_config_revision: binding.config_revision,
                selected_sequencer_source_id: binding.source_id,
                selected_sequencer_target_fingerprint: binding.target_fingerprint,
                bedrock_endpoint: endpoint,
                runtime,
                state: "stopped".to_owned(),
                indexed_block_id: None,
                last_error: None,
                operations: Vec::new(),
            });
            channel_state
                .records
                .last_mut()
                .context("new Channel Indexer record is missing")?
        }
    };
    ensure_valid_indexer_config(context.config_root, record)?;

    let module_detail = match start_runtime_and_indexer(record, context.control) {
        Ok(detail) => detail,
        Err(error) => {
            let cleanup_error = stop_runtime(record, None).err();
            record.state = "stopped".to_owned();
            record.indexed_block_id = None;
            record.last_error = Some(match cleanup_error {
                Some(cleanup_error) => format!("{error}; cleanup failed: {cleanup_error}"),
                None => error.to_string(),
            });
            return Err(error);
        }
    };

    record.state = "starting".to_owned();
    record.indexed_block_id = None;
    record.last_error = None;
    Ok(ActionOutcome::starting(format!(
        "Started isolated Channel Indexer for `{}` bound to Sequencer source `{}` ({module_detail})",
        context.channel_id, record.selected_sequencer_source_id,
    )))
}

fn stop(
    channel_state: &mut ChannelIndexerState,
    network_scope: &NetworkScope,
    channel_id: &str,
    control: Option<&CommandControl>,
) -> Result<ActionOutcome> {
    let Some(record) = find_record_mut(channel_state, network_scope, channel_id) else {
        return Ok(ActionOutcome::needs_configuration(
            "no isolated Channel Indexer is configured for this Channel",
        ));
    };
    if !record.runtime.is_running() {
        record.runtime.daemon_process_id = None;
        record.state = "stopped".to_owned();
        record.indexed_block_id = None;
        record.last_error = None;
        return Ok(ActionOutcome::stopped(
            "Channel Indexer runtime is already stopped".to_owned(),
        ));
    }

    let cli = record.runtime.cli_runtime()?;
    let module_detail =
        match execute_indexer_lifecycle_action(&cli, record, NodeAction::Stop, control) {
            Ok(detail) => detail,
            Err(error) if is_control_interruption(&error) => return Err(error),
            Err(error) => format!("module stop could not be confirmed: {error}"),
        };
    stop_runtime(record, control)?;
    record.state = "stopped".to_owned();
    record.indexed_block_id = None;
    record.last_error = None;
    Ok(ActionOutcome::stopped(format!(
        "Stopped isolated Channel Indexer for `{channel_id}` ({module_detail})"
    )))
}

fn purge(
    channel_state: &mut ChannelIndexerState,
    network_scope: &NetworkScope,
    channel_id: &str,
    control: Option<&CommandControl>,
) -> Result<ActionOutcome> {
    let Some(record) = find_record_mut(channel_state, network_scope, channel_id) else {
        return Ok(ActionOutcome::needs_configuration(
            "no isolated Channel Indexer data is configured for this Channel",
        ));
    };
    anyhow::ensure!(
        record.state == "stopped",
        "stop this Channel Indexer before resetting its data"
    );

    let started_runtime = !record.runtime.is_running();
    if started_runtime {
        let command = record.runtime.daemon_command()?;
        let process_id = spawn_detached(command, "isolated Channel Indexer maintenance runtime")?;
        record.runtime.daemon_process_id = Some(process_id);
        let readiness = match control {
            Some(control) => record.runtime.wait_until_ready_controlled(control),
            None => record.runtime.wait_until_ready(),
        };
        if let Err(error) = readiness {
            let cleanup_control = fresh_maintenance_cleanup_control();
            return match stop_runtime(record, Some(&cleanup_control)) {
                Ok(()) => Err(error),
                Err(cleanup_error) => Err(error).context(format!(
                    "failed to stop maintenance runtime: {cleanup_error:#}"
                )),
            };
        }
    }

    let result = (|| {
        let cli = record.runtime.cli_runtime()?;
        let spec = command_spec_for(
            NodeKind::Indexer,
            NodeAction::Purge,
            &record.config_path(),
            &record.data_path(),
            None,
        )
        .context("Channel Indexer data reset is not implemented")?;
        ensure_module_loaded(&spec, Some(&cli), control)?;
        let output = execute_command_spec(&spec, Some(&cli), control)?;
        Ok::<String, anyhow::Error>(format!(
            "Reset data for Channel Indexer `{channel_id}` ({})",
            operation_detail_from_value(&output)
        ))
    })();
    let cleanup = if started_runtime {
        let cleanup_control = fresh_maintenance_cleanup_control();
        stop_runtime(record, Some(&cleanup_control))
    } else {
        Ok(())
    };
    match (result, cleanup) {
        (Ok(detail), Ok(())) => {
            record.state = "stopped".to_owned();
            record.indexed_block_id = None;
            record.last_error = None;
            Ok(ActionOutcome::purged(detail))
        }
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error).context("failed to stop maintenance runtime"),
        (Err(error), Err(cleanup_error)) => Err(error).context(format!(
            "failed to stop maintenance runtime: {cleanup_error:#}"
        )),
    }
}

fn fresh_maintenance_cleanup_control() -> CommandControl {
    let now = Instant::now();
    let deadline = now
        .checked_add(INDEXER_LIFECYCLE_CONFIRMATION_TIMEOUT)
        .unwrap_or(now);
    CommandControl::new(CancellationToken::new(), deadline)
}

fn start_runtime_and_indexer(
    record: &mut ChannelIndexerRecord,
    control: Option<&CommandControl>,
) -> Result<String> {
    if let Some(control) = control {
        control.check_active()?;
    }
    if !record.runtime.is_running() {
        let command = record.runtime.daemon_command()?;
        let process_id = spawn_detached(command, "isolated Channel Indexer LogosCore runtime")?;
        record.runtime.daemon_process_id = Some(process_id);
        let readiness = match control {
            Some(control) => record.runtime.wait_until_ready_controlled(control),
            None => record.runtime.wait_until_ready(),
        };
        readiness?;
    }
    let cli = record.runtime.cli_runtime()?;
    let spec = command_spec_for(
        NodeKind::Indexer,
        NodeAction::Start,
        &record.config_path(),
        &record.data_path(),
        None,
    )
    .context("Channel Indexer start is not implemented")?;
    ensure_module_loaded(&spec, Some(&cli), control)?;
    execute_indexer_lifecycle_action(&cli, record, NodeAction::Start, control)
}

fn execute_indexer_lifecycle_action(
    cli: &LogoscoreCliRuntime,
    record: &ChannelIndexerRecord,
    action: NodeAction,
    control: Option<&CommandControl>,
) -> Result<String> {
    let lifecycle_control = indexer_lifecycle_control(control);
    let metadata = cli
        .module_info_controlled(INDEXER_MODULE, lifecycle_control.clone())
        .context("failed to inspect the installed Channel Indexer module")?;
    let use_v1 = supports_indexer_lifecycle_v1(&metadata.value);
    if use_v1 {
        return execute_indexer_lifecycle_v1_action(cli, record, action, lifecycle_control);
    }

    let spec = command_spec_for(
        NodeKind::Indexer,
        action,
        &record.config_path(),
        &record.data_path(),
        None,
    )
    .with_context(|| format!("Channel Indexer {} is not implemented", action.as_str()))?;
    let value = execute_command_spec(&spec, Some(cli), control)?;
    Ok(format!(
        "legacy {} confirmed: {}",
        action.as_str(),
        operation_detail_from_value(&value)
    ))
}

fn indexer_lifecycle_control(control: Option<&CommandControl>) -> CommandControl {
    let now = Instant::now();
    let deadline = now
        .checked_add(INDEXER_LIFECYCLE_CONFIRMATION_TIMEOUT)
        .unwrap_or(now);
    match control {
        Some(control) => control.with_deadline(deadline),
        None => CommandControl::new(CancellationToken::new(), deadline),
    }
}

fn supports_indexer_lifecycle_v1(metadata: &Value) -> bool {
    metadata_has_method(
        metadata,
        INDEXER_NODE_STATUS_METHOD,
        INDEXER_NODE_STATUS_SIGNATURE,
    ) && metadata_has_method(
        metadata,
        INDEXER_NODE_ACTION_METHOD,
        INDEXER_NODE_ACTION_SIGNATURE,
    ) && metadata_has_event(
        metadata,
        INDEXER_NODE_CHANGED_EVENT,
        INDEXER_NODE_CHANGED_SIGNATURE,
    )
}

fn metadata_has_method(metadata: &Value, method: &str, signature: &str) -> bool {
    metadata
        .get("methods")
        .and_then(Value::as_array)
        .is_some_and(|methods| {
            methods.iter().any(|candidate| {
                candidate.get("name").and_then(Value::as_str) == Some(method)
                    && candidate.get("signature").and_then(Value::as_str) == Some(signature)
                    && candidate.get("isInvokable").and_then(Value::as_bool) != Some(false)
            })
        })
}

fn metadata_has_event(metadata: &Value, event: &str, signature: &str) -> bool {
    metadata
        .get("events")
        .and_then(Value::as_array)
        .is_some_and(|events| {
            events.iter().any(|candidate| {
                candidate.get("name").and_then(Value::as_str) == Some(event)
                    && candidate.get("signature").and_then(Value::as_str) == Some(signature)
            })
        })
}

fn execute_indexer_lifecycle_v1_action(
    cli: &LogoscoreCliRuntime,
    record: &ChannelIndexerRecord,
    action: NodeAction,
    control: CommandControl,
) -> Result<String> {
    let snapshot = indexer_lifecycle_snapshot(cli, &control)?;
    let (action_name, expected_transition, expected_terminal, parameters) =
        indexer_lifecycle_action_parameters(record, action)?;
    validate_indexer_lifecycle_snapshot_for_action(
        &snapshot,
        record,
        action_name,
        expected_transition,
    )?;

    let mut watch = cli.start_event_watch(INDEXER_MODULE, INDEXER_NODE_CHANGED_EVENT, &control)?;
    let result = (|| {
        watch.wait_ready(&control)?;
        let serial = INDEXER_LIFECYCLE_OPERATION_SERIAL.fetch_add(1, Ordering::Relaxed);
        let operation_id = format!(
            "logos-inspector-indexer-{action_name}-{}-{serial}",
            now_millis()
        );
        let request = serde_json::json!({
            "schema": "logos.managed_node_lifecycle.command",
            "version": 1,
            "operation_id": operation_id,
            "action": action_name,
            "expected": {
                "instance_id": snapshot.instance_id,
                "epoch": snapshot.epoch,
                "sequence": snapshot.sequence,
            },
            "parameters": parameters,
        });
        let output = cli.call_controlled(
            INDEXER_MODULE,
            INDEXER_NODE_ACTION_METHOD,
            &[request.to_string()],
            control.clone(),
        )?;
        let acknowledgement =
            normalize_module_call_value(INDEXER_MODULE, INDEXER_NODE_ACTION_METHOD, output.value)?;
        validate_indexer_lifecycle_acknowledgement(
            &acknowledgement,
            &snapshot,
            &operation_id,
            expected_transition,
        )?;
        let terminal = wait_for_indexer_lifecycle_terminal_event(
            &mut watch,
            &control,
            &snapshot,
            record,
            &operation_id,
            action_name,
            expected_transition,
            expected_terminal,
        )?;
        Ok(format!(
            "V1 nodeChanged confirmed {action_name} for Channel `{}` at lifecycle sequence {}",
            record.channel_id, terminal.sequence
        ))
    })();
    let cleanup = watch.stop();
    match (result, cleanup) {
        (Ok(detail), Ok(())) => Ok(detail),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error).context("Indexer lifecycle event watcher cleanup failed"),
        (Err(error), Err(cleanup_error)) => Err(error).context(format!(
            "Indexer lifecycle event watcher cleanup failed: {cleanup_error}"
        )),
    }
}

fn indexer_lifecycle_action_parameters(
    record: &ChannelIndexerRecord,
    action: NodeAction,
) -> Result<(&'static str, &'static str, &'static str, Value)> {
    match action {
        NodeAction::Start => {
            let config_path = record.config_path();
            anyhow::ensure!(
                Path::new(&config_path).is_absolute(),
                "Channel Indexer lifecycle configuration path must be absolute"
            );
            Ok((
                "start",
                "starting",
                "running",
                serde_json::json!({ "config_path": config_path }),
            ))
        }
        NodeAction::Stop => Ok(("stop", "stopping", "stopped", serde_json::json!({}))),
        _ => bail!(
            "Channel Indexer V1 lifecycle does not support {}",
            action.as_str()
        ),
    }
}

fn indexer_lifecycle_snapshot(
    cli: &LogoscoreCliRuntime,
    control: &CommandControl,
) -> Result<IndexerLifecycleSnapshot> {
    let output = cli.call_controlled(
        INDEXER_MODULE,
        INDEXER_NODE_STATUS_METHOD,
        &[],
        control.clone(),
    )?;
    let value =
        normalize_module_call_value(INDEXER_MODULE, INDEXER_NODE_STATUS_METHOD, output.value)?;
    IndexerLifecycleSnapshot::parse(&value)
}

fn validate_indexer_lifecycle_snapshot_for_action(
    snapshot: &IndexerLifecycleSnapshot,
    record: &ChannelIndexerRecord,
    action: &str,
    expected_transition: &str,
) -> Result<()> {
    anyhow::ensure!(
        snapshot.supports_action(action),
        "Indexer V1 nodeStatus does not allow `{action}` in its current state `{}`",
        snapshot.state
    );
    if let Some(channel_id) = snapshot.channel_id.as_deref() {
        anyhow::ensure!(
            channel_id == record.channel_id,
            "Indexer V1 nodeStatus is scoped to a different Channel"
        );
    }
    anyhow::ensure!(
        snapshot.state != expected_transition,
        "Indexer V1 nodeStatus is already transitioning"
    );
    Ok(())
}

fn validate_indexer_lifecycle_acknowledgement(
    value: &Value,
    snapshot: &IndexerLifecycleSnapshot,
    operation_id: &str,
    expected_transition: &str,
) -> Result<()> {
    let value = json_string_value(value, "Indexer V1 nodeAction response")?;
    anyhow::ensure!(
        value.get("schema").and_then(Value::as_str) == Some("logos.managed_node_lifecycle.ack")
            && value.get("version").and_then(Value::as_u64) == Some(1),
        "Indexer V1 nodeAction response has an unsupported schema or version"
    );
    anyhow::ensure!(
        value.get("operation_id").and_then(Value::as_str) == Some(operation_id),
        "Indexer V1 nodeAction response does not acknowledge the submitted operation"
    );
    anyhow::ensure!(
        value.get("duplicate").and_then(Value::as_bool) == Some(false),
        "Indexer V1 nodeAction response unexpectedly reused the operation"
    );
    anyhow::ensure!(
        value.get("accepted").and_then(Value::as_bool) == Some(true),
        "Indexer V1 nodeAction rejected the lifecycle request"
    );
    anyhow::ensure!(
        value.get("instance_id").and_then(Value::as_str) == Some(&snapshot.instance_id)
            && value.get("epoch").and_then(Value::as_u64) == Some(snapshot.epoch)
            && value
                .get("sequence")
                .and_then(Value::as_u64)
                .is_some_and(|sequence| sequence > snapshot.sequence),
        "Indexer V1 nodeAction response has an invalid lifecycle cursor"
    );
    anyhow::ensure!(
        value.get("state").and_then(Value::as_str) == Some(expected_transition),
        "Indexer V1 nodeAction response has an unexpected lifecycle state"
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn wait_for_indexer_lifecycle_terminal_event(
    watch: &mut LogoscoreEventWatch,
    control: &CommandControl,
    snapshot: &IndexerLifecycleSnapshot,
    record: &ChannelIndexerRecord,
    operation_id: &str,
    action: &str,
    expected_transition: &str,
    expected_terminal: &str,
) -> Result<IndexerLifecycleSnapshot> {
    let mut accepted = false;
    let mut last_sequence = snapshot.sequence;
    loop {
        let Some(frame) =
            watch.next_value_within(control, INDEXER_LIFECYCLE_EVENT_READ_INTERVAL)?
        else {
            continue;
        };
        let event = IndexerLifecycleEvent::from_watch_frame(&frame)?;
        if event.operation_id.as_deref() != Some(operation_id)
            || event.instance_id != snapshot.instance_id
        {
            continue;
        }
        anyhow::ensure!(
            event.sequence > last_sequence && event.epoch >= snapshot.epoch,
            "Indexer V1 nodeChanged event has a stale lifecycle cursor"
        );
        anyhow::ensure!(
            event.action == action,
            "Indexer V1 nodeChanged event has an unexpected action"
        );
        anyhow::ensure!(
            event.status.instance_id == event.instance_id
                && event.status.epoch == event.epoch
                && event.status.sequence == event.sequence,
            "Indexer V1 nodeChanged event has an inconsistent status snapshot"
        );
        last_sequence = event.sequence;
        match event.phase.as_str() {
            "accepted" => {
                anyhow::ensure!(
                    !accepted
                        && event.outcome == "accepted"
                        && event.status.state == expected_transition,
                    "Indexer V1 nodeChanged accepted event has an invalid lifecycle state"
                );
                if let Some(channel_id) = event.channel_id.as_deref() {
                    anyhow::ensure!(
                        channel_id == record.channel_id,
                        "Indexer V1 nodeChanged accepted event is scoped to a different Channel"
                    );
                }
                accepted = true;
            }
            "settled" => {
                if event.outcome != "succeeded" {
                    let detail = event
                        .error
                        .as_ref()
                        .map(|error| format!("{}: {}", error.code, error.message))
                        .unwrap_or_else(|| format!("outcome `{}`", event.outcome));
                    bail!("Indexer V1 nodeChanged terminal event reported failure: {detail}");
                }
                anyhow::ensure!(
                    accepted && event.status.state == expected_terminal,
                    "Indexer V1 nodeChanged terminal event did not confirm the requested action"
                );
                anyhow::ensure!(
                    event.channel_id.as_deref() == Some(record.channel_id.as_str())
                        && event.status.channel_id.as_deref() == Some(record.channel_id.as_str()),
                    "Indexer V1 nodeChanged terminal event is scoped to a different Channel"
                );
                return Ok(event.status);
            }
            _ => bail!("Indexer V1 nodeChanged event has an unsupported phase"),
        }
    }
}

fn json_string_value(value: &Value, label: &str) -> Result<Value> {
    match value {
        Value::String(text) => {
            serde_json::from_str(text).with_context(|| format!("{label} is not valid JSON"))
        }
        value => Ok(value.clone()),
    }
}

#[derive(Debug, Clone)]
struct IndexerLifecycleSnapshot {
    instance_id: String,
    epoch: u64,
    sequence: u64,
    state: String,
    channel_id: Option<String>,
    supported_actions: Vec<String>,
}

impl IndexerLifecycleSnapshot {
    fn parse(value: &Value) -> Result<Self> {
        let value = json_string_value(value, "Indexer V1 nodeStatus response")?;
        let payload: IndexerLifecycleSnapshotPayload = serde_json::from_value(value)
            .context("Indexer V1 nodeStatus response has an invalid shape")?;
        anyhow::ensure!(
            payload.schema == "logos.managed_node_lifecycle.snapshot" && payload.version == 1,
            "Indexer V1 nodeStatus response has an unsupported schema or version"
        );
        anyhow::ensure!(
            !payload.instance_id.trim().is_empty(),
            "Indexer V1 nodeStatus response has no instance ID"
        );
        validate_indexer_lifecycle_scope(&payload.scope, "Indexer V1 nodeStatus response")?;
        anyhow::ensure!(
            !payload.state.trim().is_empty(),
            "Indexer V1 nodeStatus response has invalid lifecycle actions"
        );
        let mut seen_actions = BTreeSet::new();
        anyhow::ensure!(
            payload.supported_actions.iter().all(|action| {
                !action.trim().is_empty() && seen_actions.insert(action.as_str())
            }),
            "Indexer V1 nodeStatus response has invalid lifecycle actions"
        );
        Ok(Self {
            instance_id: payload.instance_id,
            epoch: payload.epoch,
            sequence: payload.sequence,
            state: payload.state,
            channel_id: payload.scope.channel_id,
            supported_actions: payload.supported_actions,
        })
    }

    fn supports_action(&self, action: &str) -> bool {
        self.supported_actions
            .iter()
            .any(|candidate| candidate == action)
    }
}

#[derive(Debug, Deserialize)]
struct IndexerLifecycleSnapshotPayload {
    schema: String,
    version: u64,
    instance_id: String,
    epoch: u64,
    sequence: u64,
    scope: IndexerLifecycleScope,
    state: String,
    supported_actions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct IndexerLifecycleScope {
    kind: String,
    #[serde(default)]
    channel_id: Option<String>,
}

fn validate_indexer_lifecycle_scope(scope: &IndexerLifecycleScope, label: &str) -> Result<()> {
    anyhow::ensure!(scope.kind == "indexer", "{label} has an invalid scope");
    if let Some(channel_id) = scope.channel_id.as_deref() {
        let canonical = normalized_channel_id(channel_id)
            .with_context(|| format!("{label} has an invalid Channel scope"))?;
        anyhow::ensure!(
            canonical == channel_id,
            "{label} has a non-canonical Channel scope"
        );
    }
    Ok(())
}

#[derive(Debug)]
struct IndexerLifecycleEvent {
    instance_id: String,
    epoch: u64,
    sequence: u64,
    channel_id: Option<String>,
    operation_id: Option<String>,
    action: String,
    phase: String,
    outcome: String,
    status: IndexerLifecycleSnapshot,
    error: Option<IndexerLifecycleError>,
}

#[derive(Debug, Deserialize)]
struct IndexerLifecycleError {
    code: String,
    message: String,
}

impl IndexerLifecycleEvent {
    fn from_watch_frame(frame: &Value) -> Result<Self> {
        let transport = module_transport_event_from_watch_frame(frame, INDEXER_MODULE)?;
        anyhow::ensure!(
            transport.event() == INDEXER_NODE_CHANGED_EVENT,
            "Indexer lifecycle watch emitted an unexpected event"
        );
        Self::from_transport_event(&transport)
    }

    fn from_transport_event(transport: &ModuleTransportEvent) -> Result<Self> {
        anyhow::ensure!(
            transport.module() == INDEXER_MODULE && transport.event() == INDEXER_NODE_CHANGED_EVENT,
            "Indexer lifecycle event has an unexpected module or event name"
        );
        let [payload] = transport.args() else {
            bail!("Indexer V1 nodeChanged event must contain exactly one payload");
        };
        let value = json_string_value(payload, "Indexer V1 nodeChanged payload")?;
        let payload: IndexerLifecycleEventPayload = serde_json::from_value(value)
            .context("Indexer V1 nodeChanged payload has an invalid shape")?;
        anyhow::ensure!(
            payload.schema == "logos.managed_node_lifecycle.event" && payload.version == 1,
            "Indexer V1 nodeChanged payload has an unsupported schema or version"
        );
        anyhow::ensure!(
            !payload.instance_id.trim().is_empty()
                && !payload.action.trim().is_empty()
                && !payload.phase.trim().is_empty()
                && !payload.outcome.trim().is_empty()
                && !payload.previous_state.trim().is_empty()
                && payload.emitted_at_ms >= 0,
            "Indexer V1 nodeChanged payload has invalid lifecycle fields"
        );
        validate_indexer_lifecycle_scope(&payload.scope, "Indexer V1 nodeChanged payload")?;
        let status = IndexerLifecycleSnapshot::parse(&payload.status)?;
        anyhow::ensure!(
            payload.scope.channel_id.as_deref() == status.channel_id.as_deref(),
            "Indexer V1 nodeChanged payload and status disagree on Channel scope"
        );
        Ok(Self {
            instance_id: payload.instance_id,
            epoch: payload.epoch,
            sequence: payload.sequence,
            channel_id: payload.scope.channel_id,
            operation_id: payload.operation_id,
            action: payload.action,
            phase: payload.phase,
            outcome: payload.outcome,
            status,
            error: payload.error,
        })
    }
}

#[derive(Debug, Deserialize)]
struct IndexerLifecycleEventPayload {
    schema: String,
    version: u64,
    instance_id: String,
    epoch: u64,
    sequence: u64,
    scope: IndexerLifecycleScope,
    #[serde(default)]
    operation_id: Option<String>,
    action: String,
    phase: String,
    outcome: String,
    previous_state: String,
    status: Value,
    #[serde(default)]
    error: Option<IndexerLifecycleError>,
    emitted_at_ms: i64,
}

fn stop_runtime(record: &mut ChannelIndexerRecord, control: Option<&CommandControl>) -> Result<()> {
    let Some(process_id) = record.runtime.daemon_process_id else {
        return Ok(());
    };
    let cli = record.runtime.cli_runtime()?;
    let _stop_result = match control {
        Some(control) => cli.stop_controlled(control.clone()),
        None => cli.stop(),
    };
    let stopped = match control {
        Some(control) => record.runtime.wait_until_stopped_controlled(control)?,
        None => record.runtime.wait_until_stopped(),
    };
    if !stopped && process_group_has_live_members(process_id) {
        stop_process(process_id)?;
        let stopped = match control {
            Some(control) => record.runtime.wait_until_stopped_controlled(control)?,
            None => record.runtime.wait_until_stopped(),
        };
        if !stopped && process_group_has_live_members(process_id) {
            bail!("isolated Channel Indexer runtime process {process_id} did not stop");
        }
    }
    record.runtime.daemon_process_id = None;
    Ok(())
}

fn build_report(
    profile: &str,
    state: &LocalNodesState,
    base_runtime: Option<&LogoscoreRuntimeProfile>,
    projector: LocalNodeReportProjector,
    channel_state: &mut ChannelIndexerState,
    network_scope: &NetworkScope,
    channel_id: &str,
) -> Result<(LocalNodeReport, bool)> {
    let profile = normalized_profile(profile);
    let package = package_prerequisite(state, profile, base_runtime);
    let current_binding = current_source_binding(network_scope, channel_id);
    let legacy_problem = legacy_indexer_problem(state, profile);
    let mut changed = false;
    if let Some(record) = find_record_mut(channel_state, network_scope, channel_id) {
        changed = reconcile_record(record);
    }

    let mut report = projector.report(profile, state, base_runtime);
    let record = find_record(channel_state, network_scope, channel_id);
    let mut node = report
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Indexer)
        .cloned()
        .unwrap_or_else(empty_indexer_status);
    let record_is_running = record.is_some_and(|record| record.runtime.is_running());
    let record_state = record
        .map(|record| record.state.as_str())
        .unwrap_or("stopped");
    let binding_detail = match &current_binding {
        Ok(binding) => binding_detail(binding),
        Err(error) => format!("Selected Sequencer binding unavailable: {error}"),
    };
    let binding_matches = record.is_none_or(|record| {
        current_binding
            .as_ref()
            .is_ok_and(|binding| record_matches_binding(record, binding))
    });

    node.install_state = if package.installed {
        "installed".to_owned()
    } else {
        "needs_configuration".to_owned()
    };
    node.run_state = record_state.to_owned();
    node.ownership = "inspector_managed".to_owned();
    node.endpoint = None;
    node.data_dir = record.and_then(|record| record.runtime.persistence_path.clone());
    node.config_path = record.map(ChannelIndexerRecord::config_path);
    node.managed_channel_id = Some(channel_id.to_owned());
    node.indexer_state = Some(record_state.to_owned());
    node.indexer_head = record.and_then(|record| record.indexed_block_id.clone());
    node.indexer_error = record.and_then(|record| record.last_error.clone());
    node.process_id = record
        .and_then(|record| record.runtime.daemon_process_id)
        .filter(|_| record_is_running);
    node.last_action = record.and_then(|record| record.operations.last().cloned());
    node.available_actions = channel_actions(
        package.installed,
        current_binding.is_ok(),
        legacy_problem.is_none(),
        record_is_running,
        record.is_some(),
        record_state,
    );
    node.detail = indexer_detail(
        &package,
        &binding_detail,
        legacy_problem.as_deref(),
        record_is_running,
        binding_matches,
    );

    report.summary = LocalNodeSummary {
        total: 1,
        installed: usize::from(package.installed),
        running: usize::from(record_is_running && record_state != "stopped"),
        needs_configuration: usize::from(!package.installed),
    };
    report.nodes = vec![node];
    report.operations = record
        .map(|record| record.operations.clone())
        .unwrap_or_default();
    report.runtime = runtime::status(record.map(|record| &record.runtime));
    report.available_network_actions = Vec::new();
    report.available_runtime_actions = Vec::new();
    Ok((report, changed))
}

fn reconcile_record(record: &mut ChannelIndexerRecord) -> bool {
    if !record.runtime.is_running() {
        let changed = record.runtime.daemon_process_id.take().is_some()
            || record.state != "stopped"
            || record.indexed_block_id.is_some();
        record.state = "stopped".to_owned();
        record.indexed_block_id = None;
        return changed;
    }
    if record.state == "stopped" {
        return false;
    }
    match indexer_status(&record.runtime) {
        Ok(status) => update_record_status(record, status),
        Err(error) => update_record_failure(
            record,
            format!("Indexer status could not be verified: {error}"),
        ),
    }
}

fn indexer_status(runtime: &LogoscoreRuntimeProfile) -> Result<IndexerStatus> {
    let cli = runtime.cli_runtime()?;
    let now = Instant::now();
    let deadline = now.checked_add(STATUS_TIMEOUT).unwrap_or(now);
    let control = CommandControl::new(CancellationToken::new(), deadline);
    let output = cli.call_controlled("lez_indexer_module", "getStatus", &[], control)?;
    let value = normalize_module_call_value("lez_indexer_module", "getStatus", output.value)?;
    parse_indexer_status(&value)
}

#[derive(Debug, Clone)]
enum IndexerStatus {
    Stopped,
    Running {
        state: String,
        indexed_block_id: Option<String>,
        last_error: Option<String>,
    },
}

fn parse_indexer_status(value: &Value) -> Result<IndexerStatus> {
    if value.is_null() {
        return Ok(IndexerStatus::Stopped);
    }
    if let Some(value) = value.as_str() {
        let value = value.trim();
        if value.is_empty() {
            return Ok(IndexerStatus::Stopped);
        }
        let value = serde_json::from_str::<Value>(value)
            .context("Indexer getStatus returned an invalid JSON string")?;
        return parse_indexer_status(&value);
    }
    let state = value
        .get("state")
        .and_then(Value::as_str)
        .context("Indexer getStatus returned no state")?;
    let normalized = state
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    let state = match normalized.as_str() {
        "starting" => "starting",
        "syncing" => "syncing",
        "caughtup" => "caught_up",
        "running" => "running",
        "error" => "error",
        "stalled" => "stalled",
        "stopped" => return Ok(IndexerStatus::Stopped),
        _ => bail!("Indexer getStatus returned unsupported state `{state}`"),
    };
    Ok(IndexerStatus::Running {
        state: state.to_owned(),
        indexed_block_id: value
            .get("indexedBlockId")
            .or_else(|| value.get("indexed_block_id"))
            .and_then(indexer_status_scalar),
        last_error: value
            .get("lastError")
            .or_else(|| value.get("last_error"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
    })
}

fn indexer_status_scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => None,
    }
}

fn update_record_status(record: &mut ChannelIndexerRecord, status: IndexerStatus) -> bool {
    match status {
        IndexerStatus::Stopped => {
            let changed = record.state != "stopped"
                || record.indexed_block_id.is_some()
                || record.last_error.is_some();
            record.state = "stopped".to_owned();
            record.indexed_block_id = None;
            record.last_error = None;
            changed
        }
        IndexerStatus::Running {
            state,
            indexed_block_id,
            last_error,
        } => {
            let changed = record.state != state
                || record.indexed_block_id != indexed_block_id
                || record.last_error != last_error;
            record.state = state;
            record.indexed_block_id = indexed_block_id;
            record.last_error = last_error;
            changed
        }
    }
}

fn update_record_failure(record: &mut ChannelIndexerRecord, detail: String) -> bool {
    let changed = record.state != "unknown" || record.last_error.as_deref() != Some(&detail);
    record.state = "unknown".to_owned();
    record.last_error = Some(detail);
    changed
}

fn requested_source_binding(
    request: &ChannelIndexerActionRequest,
    channel_id: &str,
) -> Result<SourceBinding> {
    let configs = load_channel_source_configs()?;
    requested_source_binding_from_configs(request, channel_id, &configs)
}

fn requested_source_binding_from_configs(
    request: &ChannelIndexerActionRequest,
    channel_id: &str,
    configs: &[ChannelSourceConfig],
) -> Result<SourceBinding> {
    let expected_revision = request
        .source_config_revision
        .filter(|value| *value > 0)
        .context("Channel source configuration revision is required")?;
    let expected_source = request
        .selected_sequencer_source_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("selected Sequencer source is required")?;
    let binding = source_binding_from_configs(configs, &request.network_scope, channel_id)?;
    if binding.config_revision != expected_revision {
        bail!("Channel source configuration changed; refresh Zone Sources before starting Indexer");
    }
    if binding.source_id != expected_source {
        bail!("selected Sequencer source changed; refresh Zone Sources before starting Indexer");
    }
    Ok(binding)
}

fn current_source_binding(network_scope: &NetworkScope, channel_id: &str) -> Result<SourceBinding> {
    let configs = load_channel_source_configs()?;
    source_binding_from_configs(&configs, network_scope, channel_id)
}

fn source_binding_from_configs(
    configs: &[ChannelSourceConfig],
    network_scope: &NetworkScope,
    channel_id: &str,
) -> Result<SourceBinding> {
    let config = configs
        .iter()
        .find(|config| config.network_scope == *network_scope && config.channel_id == channel_id)
        .context(
            "configure a selected Sequencer source for this Channel before starting Indexer",
        )?;
    let source_id = config
        .selected_sequencer_source_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("select a Sequencer source for this Channel before starting Indexer")?;
    let source = config
        .sequencer_sources
        .iter()
        .find(|source| source.source_id == source_id)
        .context("selected Sequencer source is no longer configured for this Channel")?;
    Ok(SourceBinding {
        config_revision: config.config_revision,
        source_id: source_id.to_owned(),
        target_fingerprint: source.target.fingerprint(),
    })
}

fn package_prerequisite(
    state: &LocalNodesState,
    profile: &str,
    base_runtime: Option<&LogoscoreRuntimeProfile>,
) -> PackagePrerequisite {
    let Some(base_runtime) = base_runtime else {
        return PackagePrerequisite::missing(
            "connect a local LogosCore runtime under System / Local Nodes",
        );
    };
    if base_runtime.is_attached() {
        let modules_dir = match base_runtime.channel_indexer_modules_dir() {
            Ok(modules_dir) => modules_dir,
            Err(error) => {
                return PackagePrerequisite::missing(format!(
                    "could not verify modules supplied by the local LogosCore service: {error}"
                ));
            }
        };
        return match package::installed_indexer_module_on_disk(Path::new(&modules_dir)) {
            Ok(true) => PackagePrerequisite::available(),
            Ok(false) => PackagePrerequisite::missing(
                "install lez_indexer_module into the modules directory used by the local LogosCore service",
            ),
            Err(error) => PackagePrerequisite::missing(format!(
                "could not verify lez_indexer_module in the local LogosCore service modules directory: {error}"
            )),
        };
    }
    if !base_runtime.is_managed() {
        return PackagePrerequisite::missing(
            "connect a local LogosCore runtime under System / Local Nodes",
        );
    }
    let Some(config) = indexer_config(state, profile) else {
        return PackagePrerequisite::missing(
            "install an exact lez_indexer_module version under System / Local Nodes",
        );
    };
    if !config.installed
        || !adapter_for(NodeKind::Indexer)
            .package_installation_matches_runtime(config, Some(base_runtime))
    {
        return PackagePrerequisite::missing(
            "install lez_indexer_module for the configured Inspector-managed LogosCore modules directory",
        );
    }
    PackagePrerequisite::available()
}

#[derive(Debug, Clone)]
struct PackagePrerequisite {
    installed: bool,
    detail: String,
}

impl PackagePrerequisite {
    fn missing(detail: impl Into<String>) -> Self {
        Self {
            installed: false,
            detail: detail.into(),
        }
    }

    fn available() -> Self {
        Self {
            installed: true,
            detail: "exact lez_indexer_module package is available to isolated Channel runtimes"
                .to_owned(),
        }
    }
}

fn indexer_config<'a>(
    state: &'a LocalNodesState,
    profile: &str,
) -> Option<&'a LocalNodeConfigRecord> {
    state.active_topology(profile).and_then(|topology| {
        topology
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Indexer)
    })
}

fn legacy_indexer_problem(state: &LocalNodesState, profile: &str) -> Option<String> {
    let config = indexer_config(state, profile)?;
    let active = matches!(
        config.lifecycle_state,
        NodeLifecycleState::Initializing
            | NodeLifecycleState::Starting
            | NodeLifecycleState::Running
            | NodeLifecycleState::Stopping
            | NodeLifecycleState::Unknown
            | NodeLifecycleState::Failed
    );
    active.then(|| {
        "a legacy single-runtime Indexer is active; stop it under System / Local Nodes before starting isolated Channel Indexers"
            .to_owned()
    })
}

fn channel_actions(
    package_installed: bool,
    source_configured: bool,
    legacy_inactive: bool,
    runtime_running: bool,
    record_exists: bool,
    state: &str,
) -> Vec<NodeAction> {
    if runtime_running {
        let mut actions = Vec::new();
        if state == "stopped" && package_installed && source_configured && legacy_inactive {
            actions.push(NodeAction::Start);
        }
        actions.push(NodeAction::Stop);
        return actions;
    }
    let mut actions = Vec::new();
    if package_installed && source_configured && legacy_inactive {
        actions.push(NodeAction::Start);
    }
    if record_exists {
        actions.push(NodeAction::Purge);
    }
    actions
}

fn indexer_detail(
    package: &PackagePrerequisite,
    binding_detail: &str,
    legacy_problem: Option<&str>,
    runtime_running: bool,
    binding_matches: bool,
) -> String {
    let mut parts = vec![package.detail.clone(), binding_detail.to_owned()];
    if let Some(problem) = legacy_problem {
        parts.push(problem.to_owned());
    }
    if runtime_running && !binding_matches {
        parts.push(
            "Selected Sequencer binding changed since this Indexer started; stop it before applying the new binding"
                .to_owned(),
        );
    }
    parts.join("; ")
}

fn binding_detail(binding: &SourceBinding) -> String {
    format!(
        "Bound to selected Sequencer source `{}` at Channel source revision {}; Indexer reads finalized Bedrock data",
        binding.source_id, binding.config_revision
    )
}

fn record_matches_binding(record: &ChannelIndexerRecord, binding: &SourceBinding) -> bool {
    record.source_config_revision == binding.config_revision
        && record.selected_sequencer_source_id == binding.source_id
        && record.selected_sequencer_target_fingerprint == binding.target_fingerprint
}

fn update_record_binding(
    record: &mut ChannelIndexerRecord,
    binding: SourceBinding,
    endpoint: String,
) {
    record.source_config_revision = binding.config_revision;
    record.selected_sequencer_source_id = binding.source_id;
    record.selected_sequencer_target_fingerprint = binding.target_fingerprint;
    record.bedrock_endpoint = endpoint;
    record.indexed_block_id = None;
    record.last_error = None;
}

fn ensure_valid_indexer_config(config_root: &Path, record: &ChannelIndexerRecord) -> Result<()> {
    let expected_path =
        channel_indexer_config_path(config_root, &record.network_scope, &record.channel_id)?;
    let path = PathBuf::from(record.config_path());
    if path != expected_path {
        bail!("Channel Indexer configuration path does not match its Channel scope");
    }
    let context = ChannelIndexerConfigContext {
        channel_id: record.channel_id.clone(),
        bedrock_endpoint: record.bedrock_endpoint.clone(),
        binding: SourceBinding {
            config_revision: record.source_config_revision,
            source_id: record.selected_sequencer_source_id.clone(),
            target_fingerprint: record.selected_sequencer_target_fingerprint.clone(),
        },
    };
    let Some(bytes) = read_optional_indexer_config(config_root, &path)? else {
        return write_indexer_config_bytes(
            config_root,
            &path,
            &default_indexer_config_bytes(&context)?,
        );
    };
    let text =
        std::str::from_utf8(&bytes).context("Channel Indexer configuration is not valid UTF-8")?;
    let value = parse_indexer_config_text(text)?;
    validate_indexer_config_value(&value, &context).context(
        "Channel Indexer configuration is invalid; open Zone Sources and repair it before starting",
    )
}

impl ChannelIndexerRecord {
    fn config_path(&self) -> String {
        Path::new(&self.runtime.config_dir)
            .parent()
            .map(|path| path.join("indexer-config.json").display().to_string())
            .unwrap_or_default()
    }

    fn data_path(&self) -> String {
        self.runtime.persistence_path.clone().unwrap_or_default()
    }
}

fn find_record<'a>(
    state: &'a ChannelIndexerState,
    network_scope: &NetworkScope,
    channel_id: &str,
) -> Option<&'a ChannelIndexerRecord> {
    state
        .records
        .iter()
        .find(|record| record.network_scope == *network_scope && record.channel_id == channel_id)
}

fn find_record_mut<'a>(
    state: &'a mut ChannelIndexerState,
    network_scope: &NetworkScope,
    channel_id: &str,
) -> Option<&'a mut ChannelIndexerRecord> {
    state
        .records
        .iter_mut()
        .find(|record| record.network_scope == *network_scope && record.channel_id == channel_id)
}

fn normalized_channel_id(channel_id: &str) -> Result<String> {
    let channel_id = channel_id.trim();
    validate_channel_id(channel_id)?;
    Ok(channel_id.to_ascii_lowercase())
}

fn config_context(request: &ChannelIndexerConfigRequest) -> Result<ChannelIndexerConfigContext> {
    let configs = load_channel_source_configs()?;
    config_context_from_configs(request, &configs)
}

fn config_context_from_configs(
    request: &ChannelIndexerConfigRequest,
    configs: &[ChannelSourceConfig],
) -> Result<ChannelIndexerConfigContext> {
    let channel_id = normalized_channel_id(&request.channel_id)?;
    let bedrock_endpoint = normalized_bedrock_endpoint(&request.bedrock_endpoint)?;
    if request.source_config_revision == 0 {
        bail!("Channel source configuration revision is required");
    }
    let selected_sequencer_source_id = request.selected_sequencer_source_id.trim().to_owned();
    if selected_sequencer_source_id.is_empty() {
        bail!("selected Sequencer source is required");
    }
    let config = configs
        .iter()
        .find(|config| {
            config.network_scope == request.network_scope && config.channel_id == channel_id
        })
        .context(
            "configure a selected Sequencer source for this Channel before configuring Indexer",
        )?;
    if config.config_revision != request.source_config_revision {
        bail!(
            "Channel source configuration changed; refresh Zone Sources before configuring Indexer"
        );
    }
    let indexer_source = config
        .indexer_source
        .as_ref()
        .context("configure the Channel-owned Indexer source before editing its configuration")?;
    if !matches!(
        &indexer_source.target,
        ChannelSourceTarget::Module { module_id } if module_id == indexer::MODULE_ID
    ) {
        bail!("the configured Indexer source is not the Channel-owned Indexer module");
    }
    let binding = source_binding_from_configs(configs, &request.network_scope, &channel_id)?;
    if binding.source_id != selected_sequencer_source_id {
        bail!("selected Sequencer source changed; refresh Zone Sources before configuring Indexer");
    }
    Ok(ChannelIndexerConfigContext {
        channel_id,
        bedrock_endpoint,
        binding,
    })
}

fn channel_indexer_config_path(
    config_root: &Path,
    network_scope: &NetworkScope,
    channel_id: &str,
) -> Result<PathBuf> {
    let scope_key = network_scope_key(network_scope)?;
    let channel_id = normalized_channel_id(channel_id)?;
    let root = config_root.join("channel-indexers");
    let path = root
        .join(scope_key)
        .join(channel_id)
        .join("indexer-config.json");
    if !path.starts_with(&root) {
        bail!("Channel Indexer configuration path escapes its managed root");
    }
    Ok(path)
}

fn read_optional_indexer_config(config_root: &Path, path: &Path) -> Result<Option<Vec<u8>>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to inspect Channel Indexer configuration {}",
                    path.display()
                )
            });
        }
    };
    validate_indexer_config_location(config_root, path, false)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!("Channel Indexer configuration must be a regular file");
    }
    if metadata.len() > MAX_CONFIG_BYTES {
        bail!("Channel Indexer configuration exceeds the 1 MiB editor limit");
    }
    fs::read(path)
        .with_context(|| {
            format!(
                "failed to read Channel Indexer configuration {}",
                path.display()
            )
        })
        .map(Some)
}

fn write_indexer_config_bytes(config_root: &Path, path: &Path, bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_CONFIG_BYTES_USIZE {
        bail!("Channel Indexer configuration exceeds the 1 MiB editor limit");
    }
    validate_indexer_config_location(config_root, path, true)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            bail!("Channel Indexer configuration must be a regular file")
        }
        Ok(_) | Err(_) => {}
    }
    let parent = path
        .parent()
        .context("Channel Indexer configuration has no parent directory")?;
    let mut staged = tempfile::Builder::new()
        .prefix(".indexer-config-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .context("failed to stage Channel Indexer configuration")?;
    staged
        .write_all(bytes)
        .context("failed to write staged Channel Indexer configuration")?;
    staged
        .as_file_mut()
        .flush()
        .context("failed to flush staged Channel Indexer configuration")?;
    staged
        .as_file()
        .sync_all()
        .context("failed to sync staged Channel Indexer configuration")?;
    staged
        .persist(path)
        .map_err(|error| error.error)
        .context("failed to atomically replace Channel Indexer configuration")?;
    sync_config_directory(parent)
}

fn validate_indexer_config_location(
    config_root: &Path,
    path: &Path,
    create_parent: bool,
) -> Result<()> {
    let root = config_root.join("channel-indexers");
    if !path.starts_with(&root) {
        bail!("Channel Indexer configuration is outside its managed root");
    }
    let parent = path
        .parent()
        .context("Channel Indexer configuration has no parent directory")?;
    if create_parent {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create Channel Indexer configuration directory {}",
                parent.display()
            )
        })?;
    }
    if !parent.is_dir() {
        bail!("Channel Indexer configuration directory is unavailable");
    }
    let canonical_root = fs::canonicalize(&root).with_context(|| {
        format!(
            "failed to resolve Channel Indexer configuration root {}",
            root.display()
        )
    })?;
    let canonical_parent = fs::canonicalize(parent).with_context(|| {
        format!(
            "failed to resolve Channel Indexer configuration directory {}",
            parent.display()
        )
    })?;
    if canonical_parent != canonical_root && !canonical_parent.starts_with(&canonical_root) {
        bail!("Channel Indexer configuration directory escapes its managed root");
    }
    Ok(())
}

fn default_indexer_config_bytes(context: &ChannelIndexerConfigContext) -> Result<Vec<u8>> {
    serde_json::to_vec_pretty(
        &crate::source_routing::execution_zone_layer::managed_indexer_channel_config(
            &context.channel_id,
            &context.bedrock_endpoint,
        ),
    )
    .context("failed to serialize default Channel Indexer configuration")
}

fn parse_indexer_config_text(text: &str) -> Result<Value> {
    if text.len() > MAX_CONFIG_BYTES_USIZE {
        bail!("Channel Indexer configuration exceeds the 1 MiB editor limit");
    }
    serde_json::from_str(text).context("Channel Indexer configuration is not valid JSON")
}

fn validate_indexer_config_value(
    value: &Value,
    context: &ChannelIndexerConfigContext,
) -> Result<()> {
    let object = value
        .as_object()
        .context("Channel Indexer configuration must be a JSON object")?;
    let channel_id = required_config_string(object.get("channel_id"), "Zone channel ID")?;
    if channel_id != context.channel_id {
        bail!("Zone channel ID is derived from the active Zone and cannot be changed");
    }
    let bedrock = object
        .get("bedrock_config")
        .and_then(Value::as_object)
        .context("Bedrock configuration must be a JSON object")?;
    if bedrock.contains_key("auth") {
        bail!(CONFIG_CREDENTIALS_REASON);
    }
    let endpoint = normalized_bedrock_endpoint(required_config_string(
        bedrock.get("addr"),
        "Bedrock API URL",
    )?)?;
    if endpoint != context.bedrock_endpoint {
        bail!("Bedrock API URL is derived from the active Bedrock source and cannot be changed");
    }
    let interval = required_config_string(
        object.get("consensus_info_polling_interval"),
        "Consensus polling interval",
    )?;
    let interval = humantime::parse_duration(interval)
        .context("Consensus polling interval must be a positive human-readable duration")?;
    if interval.is_zero() {
        bail!("Consensus polling interval must be greater than zero");
    }
    if let Some(allow_chain_reset) = object.get("allow_chain_reset")
        && !allow_chain_reset.is_boolean()
    {
        bail!("Allow automatic chain reset must be true or false");
    }
    Ok(())
}

fn required_config_string<'a>(value: Option<&'a Value>, label: &str) -> Result<&'a str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("{label} is required"))
}

fn indexer_config_contains_bedrock_credentials(value: &Value) -> bool {
    value
        .get("bedrock_config")
        .and_then(Value::as_object)
        .is_some_and(|config| config.contains_key("auth"))
}

fn project_config_fields(value: &Value) -> Vec<ChannelIndexerConfigField> {
    let mut fields = Vec::new();
    for (path, label, section, kind, required, editable) in [
        (
            "/channel_id",
            "Zone channel ID",
            "Protocol",
            "string",
            true,
            false,
        ),
        (
            "/bedrock_config/addr",
            "Bedrock API URL",
            "API",
            "string",
            true,
            false,
        ),
        (
            "/consensus_info_polling_interval",
            "Consensus polling interval",
            "Protocol",
            "string",
            true,
            true,
        ),
        (
            "/allow_chain_reset",
            "Allow automatic chain reset",
            "Recovery",
            "boolean",
            false,
            true,
        ),
    ] {
        if let Some(field_value) = value.pointer(path) {
            fields.push(ChannelIndexerConfigField {
                path: path.to_owned(),
                label: label.to_owned(),
                section: section.to_owned(),
                kind: kind.to_owned(),
                value: field_value.clone(),
                required,
                editable,
            });
        }
    }
    fields
}

fn config_is_active(
    state: &ChannelIndexerState,
    network_scope: &NetworkScope,
    channel_id: &str,
) -> bool {
    find_record(state, network_scope, channel_id).is_some_and(|record| record.runtime.is_running())
}

#[cfg(unix)]
fn sync_config_directory(path: &Path) -> Result<()> {
    fs::File::open(path)
        .context("failed to open Channel Indexer configuration directory")?
        .sync_all()
        .context("failed to sync Channel Indexer configuration directory")
}

#[cfg(not(unix))]
fn sync_config_directory(_path: &Path) -> Result<()> {
    Ok(())
}

fn revision_for(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn network_scope_key(network_scope: &NetworkScope) -> Result<String> {
    let bytes = serde_json::to_vec(network_scope)
        .context("failed to serialize Channel Indexer network scope")?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn validate_state(state: &ChannelIndexerState, config_root: &Path) -> Result<()> {
    if state.version != STATE_VERSION {
        bail!(
            "unsupported Channel Indexer state version {}",
            state.version
        );
    }
    let mut identities = BTreeSet::new();
    for record in &state.records {
        let channel_id = normalized_channel_id(&record.channel_id)?;
        if channel_id != record.channel_id {
            bail!("Channel Indexer state has a non-canonical Channel ID");
        }
        if normalized_bedrock_endpoint(&record.bedrock_endpoint)? != record.bedrock_endpoint {
            bail!("Channel Indexer state has a non-canonical Bedrock endpoint");
        }
        let scope_key = network_scope_key(&record.network_scope)?;
        let identity = format!("{scope_key}:{}", record.channel_id);
        if !identities.insert(identity) {
            bail!("Channel Indexer state has duplicate Channel records");
        }
        record.runtime.validate_for_config_root(config_root)?;
        let expected = LogoscoreRuntimeProfile::create_channel_indexer(
            config_root,
            &scope_key,
            &record.channel_id,
            &record.runtime,
        )?;
        if expected.id != record.runtime.id
            || expected.config_dir != record.runtime.config_dir
            || expected.persistence_path != record.runtime.persistence_path
        {
            bail!("Channel Indexer runtime paths do not match its Channel scope");
        }
    }
    Ok(())
}

fn empty_indexer_status() -> LocalNodeStatus {
    LocalNodeStatus {
        kind: NodeKind::Indexer,
        key: "indexer".to_owned(),
        label: "Indexer".to_owned(),
        install_state: "needs_configuration".to_owned(),
        run_state: "stopped".to_owned(),
        ownership: "inspector_managed".to_owned(),
        endpoint: None,
        data_dir: None,
        config_path: None,
        initialization_configuration_ready: None,
        package_path: None,
        package_version: None,
        managed_channel_id: None,
        indexer_state: None,
        indexer_head: None,
        indexer_error: None,
        process_id: None,
        last_action: None,
        available_actions: Vec::new(),
        detail: String::new(),
    }
}

#[derive(Debug)]
struct ActionOutcome {
    status: &'static str,
    detail: String,
}

impl ActionOutcome {
    fn starting(detail: String) -> Self {
        Self {
            status: "starting",
            detail,
        }
    }

    fn stopped(detail: String) -> Self {
        Self {
            status: "stopped",
            detail,
        }
    }

    fn purged(detail: String) -> Self {
        Self {
            status: "purged",
            detail,
        }
    }

    fn needs_configuration(detail: impl Into<String>) -> Self {
        Self {
            status: "needs_configuration",
            detail: detail.into(),
        }
    }
}

fn operation_report(
    action: NodeAction,
    status: impl Into<String>,
    detail: String,
) -> LocalNodeOperationReport {
    let timestamp = now_millis();
    LocalNodeOperationReport {
        id: format!("channel-indexer-op-{timestamp}"),
        time: timestamp.to_string(),
        timestamp_millis: timestamp,
        action,
        node: Some(NodeKind::Indexer),
        network_id: None,
        status: status.into(),
        detail,
        command: None,
    }
}

fn push_operation(
    operations: &mut Vec<LocalNodeOperationReport>,
    operation: LocalNodeOperationReport,
) {
    operations.push(operation);
    if operations.len() > OPERATION_HISTORY_LIMIT {
        let keep_from = operations.len().saturating_sub(OPERATION_HISTORY_LIMIT);
        operations.drain(0..keep_from);
    }
}

fn is_control_interruption(error: &anyhow::Error) -> bool {
    error.downcast_ref::<CommandTerminated>().is_some()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use anyhow::{Context as _, Result};
    use serde_json::json;
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex, mpsc},
        time::Duration,
    };

    use super::*;
    use crate::modules::logos_core::{
        ModuleCall, ModuleCallFuture, ModuleCallReply, ModuleEventSubscription, ModuleTransport,
        ModuleTransportResult,
    };
    use crate::source_routing::channel_sources::{
        ChannelSourceTarget, ConfiguredIndexerSource, ConfiguredSequencerSource,
        PersistedSequencerAttestation,
    };

    fn network_scope() -> NetworkScope {
        NetworkScope::GenesisId {
            genesis_id: "ab".repeat(32),
        }
    }

    #[test]
    fn basecamp_instance_id_fits_runtime_address_limit() -> Result<()> {
        let channel_id = "01".repeat(32);
        let instance_id = basecamp_instance_id(&network_scope(), &channel_id)?;
        anyhow::ensure!(
            instance_id.len() <= BASECAMP_MAX_INSTANCE_ID_BYTES,
            "instance id length {} exceeds Basecamp limit {}",
            instance_id.len(),
            BASECAMP_MAX_INSTANCE_ID_BYTES
        );
        anyhow::ensure!(
            instance_id.starts_with(&format!("{BASECAMP_INDEXER_INSTANCE_PREFIX}-")),
            "instance id must keep the indexer prefix"
        );
        anyhow::ensure!(
            instance_id.ends_with(&channel_id),
            "instance id must retain the full channel id for operators"
        );
        let other_scope = NetworkScope::GenesisId {
            genesis_id: "cd".repeat(32),
        };
        let other_instance = basecamp_instance_id(&other_scope, &channel_id)?;
        anyhow::ensure!(
            instance_id != other_instance,
            "distinct network scopes must produce distinct instance ids"
        );
        let other_channel = "88".repeat(32);
        let other_channel_instance = basecamp_instance_id(&network_scope(), &other_channel)?;
        anyhow::ensure!(
            instance_id != other_channel_instance,
            "distinct channels must produce distinct instance ids"
        );
        Ok(())
    }

    fn source_config(channel_id: &str) -> ChannelSourceConfig {
        ChannelSourceConfig {
            network_scope: network_scope(),
            channel_id: channel_id.to_owned(),
            config_revision: 7,
            sequencer_sources: vec![ConfiguredSequencerSource {
                source_id: "src_selected".to_owned(),
                label: Some("Selected".to_owned()),
                target: ChannelSourceTarget::Rpc {
                    endpoint: "https://sequencer.example/".to_owned(),
                },
                channel_attestation: PersistedSequencerAttestation::Pending,
            }],
            selected_sequencer_source_id: Some("src_selected".to_owned()),
            indexer_source: None,
        }
    }

    fn module_source_config(channel_id: &str) -> ChannelSourceConfig {
        let mut config = source_config(channel_id);
        config.indexer_source = Some(ConfiguredIndexerSource {
            source_id: "src_indexer".to_owned(),
            label: Some("Managed Indexer".to_owned()),
            target: ChannelSourceTarget::Module {
                module_id: indexer::MODULE_ID.to_owned(),
            },
        });
        config
    }

    fn running_record(
        config_root: &Path,
        config: &ChannelSourceConfig,
    ) -> Result<ChannelIndexerRecord> {
        let modules = tempfile::tempdir()?;
        let base = LogoscoreRuntimeProfile::create_or_restart(
            config_root,
            None,
            Some("/bin/sh"),
            Some(&modules.path().display().to_string()),
        )?;
        let mut runtime = LogoscoreRuntimeProfile::create_channel_indexer(
            config_root,
            &network_scope_key(&config.network_scope)?,
            &config.channel_id,
            &base,
        )?;
        runtime.daemon_process_id = Some(std::process::id());
        let binding = source_binding_from_configs(
            std::slice::from_ref(config),
            &config.network_scope,
            &config.channel_id,
        )?;
        Ok(ChannelIndexerRecord {
            network_scope: config.network_scope.clone(),
            channel_id: config.channel_id.clone(),
            source_config_revision: config.config_revision,
            selected_sequencer_source_id: binding.source_id,
            selected_sequencer_target_fingerprint: binding.target_fingerprint,
            bedrock_endpoint: "http://127.0.0.1:8080".to_owned(),
            runtime,
            state: "caught_up".to_owned(),
            indexed_block_id: Some("42".to_owned()),
            last_error: None,
            operations: Vec::new(),
        })
    }

    fn config_request(channel_id: &str) -> ChannelIndexerConfigRequest {
        ChannelIndexerConfigRequest {
            network_scope: network_scope(),
            channel_id: channel_id.to_owned(),
            bedrock_endpoint: "http://127.0.0.1:8080/".to_owned(),
            source_config_revision: 7,
            selected_sequencer_source_id: "src_selected".to_owned(),
        }
    }

    fn indexer_v1_metadata() -> Value {
        json!({
            "methods": [
                { "name": "nodeStatus", "signature": "nodeStatus()", "isInvokable": true },
                { "name": "nodeAction", "signature": "nodeAction(QString)", "isInvokable": true }
            ],
            "events": [
                { "name": "nodeChanged", "signature": "nodeChanged(QString)" }
            ]
        })
    }

    fn indexer_v1_snapshot(
        channel_id: Option<&str>,
        state: &str,
        supported_actions: &[&str],
        epoch: u64,
        sequence: u64,
    ) -> Value {
        json!({
            "schema": "logos.managed_node_lifecycle.snapshot",
            "version": 1,
            "instance_id": "indexer-test-instance",
            "epoch": epoch,
            "sequence": sequence,
            "scope": { "kind": "indexer", "channel_id": channel_id },
            "state": state,
            "supported_actions": supported_actions,
            "health": "unknown",
            "last_error": null,
            "updated_at_ms": 1
        })
    }

    fn indexer_v1_event_frame(
        channel_id: Option<&str>,
        state: &str,
        supported_actions: &[&str],
        epoch: u64,
        sequence: u64,
    ) -> Value {
        let status = indexer_v1_snapshot(channel_id, state, supported_actions, epoch, sequence);
        let payload = json!({
            "schema": "logos.managed_node_lifecycle.event",
            "version": 1,
            "instance_id": "indexer-test-instance",
            "epoch": epoch,
            "sequence": sequence,
            "scope": { "kind": "indexer", "channel_id": channel_id },
            "operation_id": "indexer-test-operation",
            "action": "start",
            "phase": "settled",
            "outcome": "succeeded",
            "previous_state": "uninitialized",
            "status": status,
            "error": null,
            "emitted_at_ms": 1
        });
        json!({
            "type": "event",
            "protocol": "logoscore.watch",
            "version": 1,
            "timestamp": "2026-07-28T00:37:20Z",
            "module": INDEXER_MODULE,
            "event": INDEXER_NODE_CHANGED_EVENT,
            "data": { "arg0": payload.to_string() }
        })
    }

    #[test]
    fn zone_owned_configuration_locks_identity_and_projects_common_fields() -> Result<()> {
        let channel_id = "01".repeat(32);
        let config = module_source_config(&channel_id);
        let request = config_request(&channel_id);
        let context = config_context_from_configs(&request, &[config])?;
        let bytes = default_indexer_config_bytes(&context)?;
        let value: Value = serde_json::from_slice(&bytes)?;

        validate_indexer_config_value(&value, &context)?;
        let fields = project_config_fields(&value);
        anyhow::ensure!(fields.len() == 4);
        anyhow::ensure!(fields[0].editable == false);
        anyhow::ensure!(fields[1].editable == false);
        anyhow::ensure!(fields[2].editable);
        anyhow::ensure!(fields[3].editable);

        let mut changed_channel = value.clone();
        changed_channel
            .as_object_mut()
            .context("default config must be an object")?
            .insert("channel_id".to_owned(), json!("88".repeat(32)));
        let channel_error = validate_indexer_config_value(&changed_channel, &context)
            .expect_err("different Zone channel must be rejected");
        anyhow::ensure!(
            channel_error
                .to_string()
                .contains("derived from the active Zone")
        );

        let mut changed_endpoint = value.clone();
        changed_endpoint
            .pointer_mut("/bedrock_config/addr")
            .context("default config must include Bedrock URL")?
            .clone_from(&json!("https://other.example"));
        let endpoint_error = validate_indexer_config_value(&changed_endpoint, &context)
            .expect_err("different Bedrock endpoint must be rejected");
        anyhow::ensure!(
            endpoint_error
                .to_string()
                .contains("derived from the active Bedrock")
        );

        let mut invalid_interval = value.clone();
        invalid_interval
            .as_object_mut()
            .context("default config must be an object")?
            .insert(
                "consensus_info_polling_interval".to_owned(),
                json!("not-a-duration"),
            );
        anyhow::ensure!(validate_indexer_config_value(&invalid_interval, &context).is_err());
        Ok(())
    }

    #[test]
    fn start_preserves_valid_saved_channel_indexer_configuration() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let channel_id = "01".repeat(32);
        let source = module_source_config(&channel_id);
        let record = running_record(directory.path(), &source)?;
        let path = PathBuf::from(record.config_path());

        ensure_valid_indexer_config(directory.path(), &record)?;
        let default_bytes = fs::read(&path)?;
        let default_value: Value = serde_json::from_slice(&default_bytes)?;
        anyhow::ensure!(default_value.get("allow_chain_reset") == Some(&Value::Bool(false)));

        let mut saved_value = default_value;
        saved_value
            .as_object_mut()
            .context("default config must be an object")?
            .insert("consensus_info_polling_interval".to_owned(), json!("2s"));
        saved_value
            .as_object_mut()
            .context("default config must remain an object")?
            .insert("cross_zone".to_owned(), Value::Null);
        let saved_bytes = serde_json::to_vec(&saved_value)?;
        write_indexer_config_bytes(directory.path(), &path, &saved_bytes)?;

        ensure_valid_indexer_config(directory.path(), &record)?;
        anyhow::ensure!(fs::read(&path)? == saved_bytes);

        let mut invalid_value = saved_value;
        invalid_value
            .as_object_mut()
            .context("saved config must be an object")?
            .insert("channel_id".to_owned(), json!("88".repeat(32)));
        let invalid_bytes = serde_json::to_vec(&invalid_value)?;
        write_indexer_config_bytes(directory.path(), &path, &invalid_bytes)?;
        anyhow::ensure!(ensure_valid_indexer_config(directory.path(), &record).is_err());
        anyhow::ensure!(fs::read(&path)? == invalid_bytes);
        Ok(())
    }

    #[test]
    fn source_binding_requires_the_exact_selected_source() -> Result<()> {
        let channel_id = "01".repeat(32);
        let config = source_config(&channel_id);
        let binding = source_binding_from_configs(&[config], &network_scope(), &channel_id)?;

        anyhow::ensure!(binding.config_revision == 7);
        anyhow::ensure!(binding.source_id == "src_selected");
        anyhow::ensure!(binding.target_fingerprint.starts_with("sha256:"));
        Ok(())
    }

    #[test]
    fn source_binding_rejects_an_unselected_channel() -> Result<()> {
        let channel_id = "01".repeat(32);
        match source_binding_from_configs(
            &[source_config(&channel_id)],
            &network_scope(),
            &("88".repeat(32)),
        ) {
            Ok(_) => anyhow::bail!("unconfigured Channel was accepted"),
            Err(error) => anyhow::ensure!(
                error
                    .to_string()
                    .contains("configure a selected Sequencer source")
            ),
        }
        Ok(())
    }

    #[test]
    fn parser_preserves_indexer_head_and_error() -> Result<()> {
        let status = parse_indexer_status(&serde_json::json!({
            "state": "CaughtUp",
            "indexedBlockId": 42,
            "lastError": ""
        }))?;

        let IndexerStatus::Running {
            state,
            indexed_block_id,
            last_error,
        } = status
        else {
            anyhow::bail!("running Indexer status was projected as stopped");
        };
        anyhow::ensure!(state == "caught_up");
        anyhow::ensure!(indexed_block_id.as_deref() == Some("42"));
        anyhow::ensure!(last_error.is_none());
        Ok(())
    }

    #[test]
    fn live_idle_runtime_can_be_stopped_or_reused() -> Result<()> {
        anyhow::ensure!(
            channel_actions(true, true, true, true, true, "stopped")
                == vec![NodeAction::Start, NodeAction::Stop]
        );
        Ok(())
    }

    #[test]
    fn stopped_channel_indexer_exposes_reset_data_recovery() -> Result<()> {
        anyhow::ensure!(
            channel_actions(true, true, true, false, true, "stopped")
                == vec![NodeAction::Start, NodeAction::Purge]
        );
        anyhow::ensure!(
            channel_actions(false, false, true, false, true, "stopped") == vec![NodeAction::Purge]
        );
        anyhow::ensure!(
            !channel_actions(true, true, true, true, true, "running").contains(&NodeAction::Purge)
        );
        Ok(())
    }

    #[test]
    fn managed_module_runtime_requires_exact_live_channel_binding() -> Result<()> {
        let config_root = tempfile::tempdir()?;
        let channel_id = "01".repeat(32);
        let config = module_source_config(&channel_id);
        let source_id = config
            .indexer_source
            .as_ref()
            .map(|source| source.source_id.clone())
            .context("module Indexer fixture is missing")?;
        let record = running_record(config_root.path(), &config)?;
        let expected = record.runtime.cli_runtime()?;
        let state = ChannelIndexerState {
            version: STATE_VERSION,
            records: vec![record],
        };

        let resolved = runtime_for_module_source(
            &state,
            std::slice::from_ref(&config),
            &config.network_scope,
            &channel_id,
            config.config_revision,
            &source_id,
        )?;
        anyhow::ensure!(resolved == expected);

        let missing = runtime_for_module_source(
            &state,
            std::slice::from_ref(&config),
            &config.network_scope,
            &"88".repeat(32),
            config.config_revision,
            &source_id,
        )
        .err()
        .context("foreign Channel was allowed to use this runtime")?;
        anyhow::ensure!(missing.to_string().contains("no isolated Channel Indexer"));

        let stale = runtime_for_module_source(
            &state,
            std::slice::from_ref(&config),
            &config.network_scope,
            &channel_id,
            config.config_revision.saturating_add(1),
            &source_id,
        )
        .err()
        .context("stale source revision was accepted")?;
        anyhow::ensure!(stale.to_string().contains("configuration changed"));

        let wrong_source = runtime_for_module_source(
            &state,
            std::slice::from_ref(&config),
            &config.network_scope,
            &channel_id,
            config.config_revision,
            "src_other",
        )
        .err()
        .context("another Indexer source was accepted")?;
        anyhow::ensure!(wrong_source.to_string().contains("does not match"));

        let mut changed_binding = config.clone();
        let selected = changed_binding
            .sequencer_sources
            .first_mut()
            .context("selected Sequencer fixture is missing")?;
        selected.target = ChannelSourceTarget::Rpc {
            endpoint: "https://other-sequencer.example/".to_owned(),
        };
        let changed = runtime_for_module_source(
            &state,
            &[changed_binding],
            &config.network_scope,
            &channel_id,
            config.config_revision,
            &source_id,
        )
        .err()
        .context("changed Sequencer binding was accepted")?;
        anyhow::ensure!(
            changed
                .to_string()
                .contains("selected Sequencer binding changed")
        );
        Ok(())
    }

    #[test]
    fn indexer_v1_metadata_requires_the_complete_exact_contract() -> Result<()> {
        let metadata = indexer_v1_metadata();
        anyhow::ensure!(supports_indexer_lifecycle_v1(&metadata));

        let mut missing_event = metadata.clone();
        missing_event
            .get_mut("events")
            .and_then(Value::as_array_mut)
            .context("fixture must include lifecycle events")?
            .clear();
        anyhow::ensure!(!supports_indexer_lifecycle_v1(&missing_event));

        let mut wrong_signature = metadata;
        let methods = wrong_signature
            .get_mut("methods")
            .and_then(Value::as_array_mut)
            .context("fixture must include lifecycle methods")?;
        let node_action = methods
            .iter_mut()
            .find(|method| method.get("name").and_then(Value::as_str) == Some("nodeAction"))
            .context("fixture must include nodeAction")?;
        node_action
            .as_object_mut()
            .context("nodeAction fixture must be an object")?
            .insert("signature".to_owned(), json!("nodeAction(QByteArray)"));
        anyhow::ensure!(!supports_indexer_lifecycle_v1(&wrong_signature));
        Ok(())
    }

    #[test]
    fn indexer_v1_snapshot_rejects_invalid_or_foreign_channel_scope() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let channel_id = "01".repeat(32);
        let source = module_source_config(&channel_id);
        let record = running_record(directory.path(), &source)?;
        let snapshot = IndexerLifecycleSnapshot::parse(&indexer_v1_snapshot(
            Some(&channel_id),
            "stopped",
            &["start"],
            1,
            4,
        ))?;
        validate_indexer_lifecycle_snapshot_for_action(&snapshot, &record, "start", "starting")?;

        let foreign = IndexerLifecycleSnapshot::parse(&indexer_v1_snapshot(
            Some(&"88".repeat(32)),
            "stopped",
            &["start"],
            1,
            4,
        ))?;
        let foreign_error =
            validate_indexer_lifecycle_snapshot_for_action(&foreign, &record, "start", "starting")
                .expect_err("foreign Channel scope must be rejected");
        anyhow::ensure!(foreign_error.to_string().contains("different Channel"));

        let invalid = indexer_v1_snapshot(Some("not-a-channel"), "stopped", &["start"], 1, 4);
        let invalid_error = IndexerLifecycleSnapshot::parse(&invalid)
            .expect_err("invalid Channel scope must be rejected");
        anyhow::ensure!(invalid_error.to_string().contains("invalid Channel scope"));
        Ok(())
    }

    #[test]
    fn indexer_v1_acknowledgement_rejects_reused_operation() -> Result<()> {
        let snapshot = IndexerLifecycleSnapshot::parse(&indexer_v1_snapshot(
            None,
            "uninitialized",
            &["start"],
            0,
            0,
        ))?;
        let acknowledgement = json!({
            "schema": "logos.managed_node_lifecycle.ack",
            "version": 1,
            "operation_id": "indexer-test-operation",
            "accepted": true,
            "duplicate": false,
            "instance_id": "indexer-test-instance",
            "epoch": 0,
            "sequence": 1,
            "state": "starting"
        });
        validate_indexer_lifecycle_acknowledgement(
            &acknowledgement,
            &snapshot,
            "indexer-test-operation",
            "starting",
        )?;

        let mut duplicate = acknowledgement;
        duplicate
            .as_object_mut()
            .context("acknowledgement fixture must be an object")?
            .insert("duplicate".to_owned(), Value::Bool(true));
        let error = validate_indexer_lifecycle_acknowledgement(
            &duplicate,
            &snapshot,
            "indexer-test-operation",
            "starting",
        )
        .expect_err("reused operation acknowledgement must be rejected");
        anyhow::ensure!(error.to_string().contains("reused the operation"));
        Ok(())
    }

    #[test]
    fn indexer_v1_event_parses_watch_frame_and_requires_matching_scope() -> Result<()> {
        let channel_id = "01".repeat(32);
        let frame = indexer_v1_event_frame(Some(&channel_id), "running", &["stop"], 1, 2);
        let event = IndexerLifecycleEvent::from_watch_frame(&frame)?;
        anyhow::ensure!(event.operation_id.as_deref() == Some("indexer-test-operation"));
        anyhow::ensure!(event.channel_id.as_deref() == Some(channel_id.as_str()));
        anyhow::ensure!(event.status.channel_id.as_deref() == Some(channel_id.as_str()));
        anyhow::ensure!(event.status.state == "running");

        let mut mismatched = frame;
        let data = mismatched
            .get_mut("data")
            .and_then(Value::as_object_mut)
            .context("watch fixture must contain data")?;
        let text = data
            .get("arg0")
            .and_then(Value::as_str)
            .context("watch fixture must contain an event payload")?
            .to_owned();
        let mut payload: Value = serde_json::from_str(&text)?;
        payload
            .pointer_mut("/status/scope/channel_id")
            .context("event status fixture must include Channel scope")?
            .clone_from(&Value::Null);
        data.insert("arg0".to_owned(), Value::String(payload.to_string()));
        let error = IndexerLifecycleEvent::from_watch_frame(&mismatched)
            .expect_err("mismatched event and status scopes must be rejected");
        anyhow::ensure!(error.to_string().contains("disagree on Channel scope"));
        Ok(())
    }

    #[test]
    fn indexer_v1_failed_event_preserves_module_error_detail() -> Result<()> {
        let channel_id = "01".repeat(32);
        let mut frame =
            indexer_v1_event_frame(Some(&channel_id), "uninitialized", &["start"], 0, 2);
        let payload = frame
            .pointer_mut("/data/arg0")
            .and_then(|value| value.as_str())
            .context("failed-event fixture is missing payload")?;
        let mut payload: Value = serde_json::from_str(payload)?;
        payload["outcome"] = json!("failed");
        payload["error"] = json!({
            "code": "start_failed",
            "message": "Indexer start failed.",
            "at_ms": 1
        });
        frame["data"]["arg0"] = json!(payload.to_string());
        let event = IndexerLifecycleEvent::from_watch_frame(&frame)?;
        anyhow::ensure!(event.outcome == "failed");
        anyhow::ensure!(
            event.error.as_ref().map(|error| error.code.as_str()) == Some("start_failed")
        );
        Ok(())
    }

    #[derive(Debug, Clone)]
    struct BasecampIndexerTestInstance {
        channel_id: Option<String>,
        state: String,
        epoch: u64,
        sequence: u64,
    }

    impl Default for BasecampIndexerTestInstance {
        fn default() -> Self {
            Self {
                channel_id: None,
                state: "uninitialized".to_owned(),
                epoch: 1,
                sequence: 0,
            }
        }
    }

    struct BasecampIndexerTestSubscription {
        module: String,
        instance_id: String,
        event: String,
        sender: mpsc::Sender<ModuleTransportEvent>,
    }

    #[derive(Default)]
    struct BasecampIndexerTestState {
        instances: BTreeMap<String, BasecampIndexerTestInstance>,
        calls: Vec<ModuleCall>,
        subscriptions: Vec<BasecampIndexerTestSubscription>,
    }

    struct BasecampIndexerTestTransport {
        state: Arc<Mutex<BasecampIndexerTestState>>,
    }

    struct BasecampIndexerTestEventSubscription {
        receiver: mpsc::Receiver<ModuleTransportEvent>,
    }

    impl ModuleEventSubscription for BasecampIndexerTestEventSubscription {
        fn next_within(&mut self, timeout: Duration) -> Result<Option<ModuleTransportEvent>> {
            match self.receiver.recv_timeout(timeout) {
                Ok(event) => Ok(Some(event)),
                Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    bail!("Basecamp Indexer test lifecycle subscription disconnected")
                }
            }
        }
    }

    impl BasecampIndexerTestTransport {
        fn new() -> Self {
            Self {
                state: Arc::new(Mutex::new(BasecampIndexerTestState::default())),
            }
        }

        fn snapshot(instance_id: &str, instance: &BasecampIndexerTestInstance) -> Value {
            let supported_actions = match instance.state.as_str() {
                "uninitialized" | "stopped" => json!(["start"]),
                "running" => json!(["stop"]),
                "starting" | "stopping" => json!([]),
                _ => json!([]),
            };
            json!({
                "schema": "logos.managed_node_lifecycle.snapshot",
                "version": 1,
                "instance_id": format!("lifecycle-{instance_id}"),
                "epoch": instance.epoch,
                "sequence": instance.sequence,
                "scope": { "kind": "indexer", "channel_id": instance.channel_id },
                "state": instance.state,
                "supported_actions": supported_actions,
            })
        }

        fn lifecycle_event(
            scoped_instance_id: &str,
            instance: &BasecampIndexerTestInstance,
            operation_id: &str,
            action: &str,
            phase: &str,
            outcome: &str,
            previous_state: &str,
        ) -> Result<ModuleTransportEvent> {
            ModuleTransportEvent::new_instance(
                INDEXER_MODULE,
                scoped_instance_id,
                INDEXER_NODE_CHANGED_EVENT,
                vec![Value::String(
                    json!({
                        "schema": "logos.managed_node_lifecycle.event",
                        "version": 1,
                        "instance_id": format!("lifecycle-{scoped_instance_id}"),
                        "epoch": instance.epoch,
                        "sequence": instance.sequence,
                        "scope": { "kind": "indexer", "channel_id": instance.channel_id },
                        "operation_id": operation_id,
                        "action": action,
                        "phase": phase,
                        "outcome": outcome,
                        "previous_state": previous_state,
                        "status": Self::snapshot(scoped_instance_id, instance),
                        "emitted_at_ms": 1,
                    })
                    .to_string(),
                )],
            )
        }

        fn subscribers_for(
            state: &BasecampIndexerTestState,
            scoped_instance_id: &str,
        ) -> Vec<mpsc::Sender<ModuleTransportEvent>> {
            state
                .subscriptions
                .iter()
                .filter(|subscription| {
                    subscription.module == INDEXER_MODULE
                        && subscription.instance_id == scoped_instance_id
                        && subscription.event == INDEXER_NODE_CHANGED_EVENT
                })
                .map(|subscription| subscription.sender.clone())
                .collect()
        }

        fn handle_core_call(&self, call: &ModuleCall) -> Result<Value> {
            anyhow::ensure!(call.instance_id().is_none(), "core service call was scoped");
            match call.method() {
                BASECAMP_HOST_CAPABILITIES_METHOD => Ok(json!({
                    "schema": "logos.basecamp_host",
                    "version": 1,
                    "scoped_module_instances": true,
                    "direct_scoped_clients": true,
                    "direct_scoped_events": true,
                })),
                BASECAMP_INSTANCE_LOADED_METHOD => {
                    let instance_id = call
                        .args()
                        .get(1)
                        .and_then(Value::as_str)
                        .context("test isModuleInstanceLoaded has no instance id")?;
                    let state = self
                        .state
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Basecamp Indexer test lock poisoned"))?;
                    Ok(json!({
                        "status": "ok",
                        "module_name": INDEXER_MODULE,
                        "instance_id": instance_id,
                        "loaded": state.instances.contains_key(instance_id),
                    }))
                }
                BASECAMP_LOAD_INSTANCE_METHOD => {
                    let instance_id = call
                        .args()
                        .get(1)
                        .and_then(Value::as_str)
                        .context("test loadModuleInstance has no instance id")?
                        .to_owned();
                    let mut state = self
                        .state
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Basecamp Indexer test lock poisoned"))?;
                    state.instances.entry(instance_id.clone()).or_default();
                    Ok(json!({
                        "status": "ok",
                        "module_name": INDEXER_MODULE,
                        "instance_id": instance_id,
                    }))
                }
                method => bail!("unexpected Basecamp core service method `{method}`"),
            }
        }

        fn handle_indexer_call(&self, call: &ModuleCall) -> Result<Value> {
            let scoped_instance_id = call
                .instance_id()
                .context("Indexer call was dispatched without an explicit Basecamp instance")?
                .to_owned();
            match call.method() {
                "reset_storage" => {
                    let config_path = call
                        .args()
                        .first()
                        .and_then(Value::as_str)
                        .context("Indexer reset_storage request has no configuration path")?;
                    anyhow::ensure!(
                        Path::new(config_path).is_file(),
                        "Basecamp Indexer test configuration path is not a file"
                    );
                    Ok(json!(0))
                }
                INDEXER_NODE_STATUS_METHOD => {
                    let state = self
                        .state
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Basecamp Indexer test lock poisoned"))?;
                    let instance = state
                        .instances
                        .get(&scoped_instance_id)
                        .context("Indexer status reached an unloaded Basecamp instance")?;
                    Ok(Self::snapshot(&scoped_instance_id, instance))
                }
                INDEXER_NODE_ACTION_METHOD => {
                    let request = call
                        .args()
                        .first()
                        .and_then(Value::as_str)
                        .context("Indexer nodeAction request is missing")?;
                    let request: Value = serde_json::from_str(request)
                        .context("Indexer nodeAction request is invalid JSON")?;
                    let action = request
                        .get("action")
                        .and_then(Value::as_str)
                        .context("Indexer nodeAction request has no action")?;
                    let operation_id = request
                        .get("operation_id")
                        .and_then(Value::as_str)
                        .context("Indexer nodeAction request has no operation id")?
                        .to_owned();
                    let start_channel_id = if action == "start" {
                        let config_path = request
                            .pointer("/parameters/config_path")
                            .and_then(Value::as_str)
                            .context("Indexer start request has no configuration path")?;
                        let config: Value = serde_json::from_slice(
                            &fs::read(config_path)
                                .context("failed to read Basecamp Indexer test config")?,
                        )
                        .context("Basecamp Indexer test config is invalid")?;
                        Some(
                            config
                                .get("channel_id")
                                .and_then(Value::as_str)
                                .context("Basecamp Indexer test config has no Channel ID")?
                                .to_owned(),
                        )
                    } else {
                        None
                    };
                    let (acknowledgement, events, subscribers) = {
                        let mut state = self
                            .state
                            .lock()
                            .map_err(|_| anyhow::anyhow!("Basecamp Indexer test lock poisoned"))?;
                        let instance = state
                            .instances
                            .get_mut(&scoped_instance_id)
                            .context("Indexer action reached an unloaded Basecamp instance")?;
                        let previous_state = instance.state.clone();
                        let (transition, terminal) = match action {
                            "start"
                                if instance.state == "uninitialized"
                                    || instance.state == "stopped" =>
                            {
                                instance.channel_id = start_channel_id;
                                ("starting", "running")
                            }
                            "stop" if instance.state == "running" => ("stopping", "stopped"),
                            _ => bail!(
                                "Indexer action `{action}` is unavailable in state `{}`",
                                instance.state
                            ),
                        };
                        instance.sequence = instance.sequence.saturating_add(1);
                        instance.state = transition.to_owned();
                        let acknowledgement = json!({
                            "schema": "logos.managed_node_lifecycle.ack",
                            "version": 1,
                            "operation_id": operation_id,
                            "accepted": true,
                            "duplicate": false,
                            "instance_id": format!("lifecycle-{scoped_instance_id}"),
                            "epoch": instance.epoch,
                            "sequence": instance.sequence,
                            "state": transition,
                        });
                        let accepted = Self::lifecycle_event(
                            &scoped_instance_id,
                            instance,
                            &operation_id,
                            action,
                            "accepted",
                            "accepted",
                            &previous_state,
                        )?;
                        instance.sequence = instance.sequence.saturating_add(1);
                        instance.state = terminal.to_owned();
                        let settled = Self::lifecycle_event(
                            &scoped_instance_id,
                            instance,
                            &operation_id,
                            action,
                            "settled",
                            "succeeded",
                            transition,
                        )?;
                        (
                            acknowledgement,
                            vec![accepted, settled],
                            Self::subscribers_for(&state, &scoped_instance_id),
                        )
                    };
                    for event in events {
                        for subscriber in &subscribers {
                            match subscriber.send(event.clone()) {
                                Ok(()) | Err(_) => {}
                            }
                        }
                    }
                    Ok(acknowledgement)
                }
                method => bail!("unexpected scoped Indexer method `{method}`"),
            }
        }
    }

    impl ModuleTransport for BasecampIndexerTestTransport {
        fn kind(&self) -> ModuleTransportKind {
            ModuleTransportKind::Module
        }

        fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
            Box::pin(async move {
                self.state
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Basecamp Indexer test lock poisoned"))?
                    .calls
                    .push(call.clone());
                let value = match call.module() {
                    BASECAMP_CORE_SERVICE_MODULE => self.handle_core_call(&call)?,
                    INDEXER_MODULE => self.handle_indexer_call(&call)?,
                    module => bail!("unexpected test module `{module}`"),
                };
                Ok(ModuleCallReply::new(ModuleTransportKind::Module, value))
            })
        }

        fn subscribe_module_instance_event(
            &self,
            module: &str,
            instance_id: &str,
            event: &str,
        ) -> ModuleTransportResult<BoxedModuleEventSubscription> {
            let (sender, receiver) = mpsc::channel();
            self.state
                .lock()
                .map_err(|_| anyhow::anyhow!("Basecamp Indexer test lock poisoned"))?
                .subscriptions
                .push(BasecampIndexerTestSubscription {
                    module: module.to_owned(),
                    instance_id: instance_id.to_owned(),
                    event: event.to_owned(),
                    sender,
                });
            Ok(Box::new(BasecampIndexerTestEventSubscription { receiver }))
        }
    }

    fn basecamp_start_request(channel_id: &str) -> ChannelIndexerActionRequest {
        ChannelIndexerActionRequest {
            action: NodeAction::Start,
            network_scope: network_scope(),
            channel_id: channel_id.to_owned(),
            bedrock_endpoint: Some("http://127.0.0.1:8080/".to_owned()),
            source_config_revision: Some(7),
            selected_sequencer_source_id: Some("src_selected".to_owned()),
        }
    }

    #[tokio::test]
    async fn basecamp_unknown_lifecycle_status_blocks_configuration_edit() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let channel_id = "01".repeat(32);
        let configs = vec![module_source_config(&channel_id)];
        let implementation = Arc::new(BasecampIndexerTestTransport::new());
        let transport: SharedModuleTransport = implementation.clone();
        let instance_id = basecamp_instance_id(&network_scope(), &channel_id)?;
        implementation
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Basecamp Indexer test lock poisoned"))?
            .instances
            .insert(
                instance_id,
                BasecampIndexerTestInstance {
                    channel_id: Some(channel_id.clone()),
                    state: "unknown".to_owned(),
                    ..BasecampIndexerTestInstance::default()
                },
            );

        let unknown = basecamp_config_snapshot_with_configs(
            directory.path(),
            "default",
            &config_request(&channel_id),
            &configs,
            &transport,
        )
        .await?;
        anyhow::ensure!(!unknown.editable);
        anyhow::ensure!(
            unknown.blocked_reason.as_deref() == Some(CONFIG_ACTIVE_RUNTIME_REASON),
            "unknown Basecamp lifecycle status did not block configuration editing"
        );

        let instance_id = basecamp_instance_id(&network_scope(), &channel_id)?;
        implementation
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Basecamp Indexer test lock poisoned"))?
            .instances
            .get_mut(&instance_id)
            .context("missing Basecamp Indexer test instance")?
            .state = "stopped".to_owned();
        let stopped = basecamp_config_snapshot_with_configs(
            directory.path(),
            "default",
            &config_request(&channel_id),
            &configs,
            &transport,
        )
        .await?;
        anyhow::ensure!(
            stopped.editable,
            "explicitly stopped Basecamp lifecycle status remained blocked"
        );
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_indexers_are_scoped_per_channel_and_confirm_lifecycle_events() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let first_channel = "01".repeat(32);
        let second_channel = "88".repeat(32);
        let configs = vec![
            module_source_config(&first_channel),
            module_source_config(&second_channel),
        ];
        let implementation = Arc::new(BasecampIndexerTestTransport::new());
        let transport: SharedModuleTransport = implementation.clone();

        let initial = basecamp_status_with_configs(
            "default",
            directory.path(),
            &configs,
            &transport,
            &network_scope(),
            &first_channel,
        )
        .await?;
        anyhow::ensure!(
            initial
                .nodes
                .first()
                .map(|node| node.available_actions.as_slice())
                == Some([NodeAction::Start].as_slice()),
            "unloaded Basecamp Channel Indexer did not expose Start"
        );

        let first = basecamp_action_with_configs(
            "default",
            directory.path(),
            &configs,
            basecamp_start_request(&first_channel),
            &transport,
        )
        .await?;
        anyhow::ensure!(
            first.nodes.first().map(|node| node.run_state.as_str()) == Some("running"),
            "first Basecamp Channel Indexer did not reach running"
        );
        anyhow::ensure!(
            first
                .operations
                .first()
                .map(|operation| operation.status.as_str())
                == Some("running"),
            "terminal Basecamp Channel Indexer Start was not recorded as running"
        );
        let first_config = basecamp_config_snapshot_with_configs(
            directory.path(),
            "default",
            &config_request(&first_channel),
            &configs,
            &transport,
        )
        .await?;
        anyhow::ensure!(
            !first_config.editable
                && first_config.blocked_reason.as_deref() == Some(CONFIG_ACTIVE_RUNTIME_REASON),
            "running Basecamp Channel Indexer did not lock its configuration"
        );
        let save_while_running = basecamp_save_config_with_configs(
            directory.path(),
            "default",
            &config_request(&first_channel),
            &first_config.raw_text,
            &first_config.revision,
            &configs,
            &transport,
        )
        .await
        .expect_err("running Basecamp Channel Indexer configuration save must fail");
        anyhow::ensure!(
            save_while_running
                .to_string()
                .contains(CONFIG_ACTIVE_RUNTIME_REASON),
            "running Basecamp Channel Indexer save did not explain its lifecycle lock"
        );

        let second = basecamp_action_with_configs(
            "default",
            directory.path(),
            &configs,
            basecamp_start_request(&second_channel),
            &transport,
        )
        .await?;
        anyhow::ensure!(
            second.nodes.first().map(|node| node.run_state.as_str()) == Some("running"),
            "second Basecamp Channel Indexer did not reach running"
        );

        let stop_first = ChannelIndexerActionRequest {
            action: NodeAction::Stop,
            network_scope: network_scope(),
            channel_id: first_channel.clone(),
            bedrock_endpoint: None,
            source_config_revision: None,
            selected_sequencer_source_id: None,
        };
        let stopped = basecamp_action_with_configs(
            "default",
            directory.path(),
            &configs,
            stop_first,
            &transport,
        )
        .await?;
        anyhow::ensure!(
            stopped.nodes.first().map(|node| node.run_state.as_str()) == Some("stopped"),
            "stopping one Basecamp Channel Indexer did not settle its own instance"
        );
        let second_after_first_stop = basecamp_status_with_configs(
            "default",
            directory.path(),
            &configs,
            &transport,
            &network_scope(),
            &second_channel,
        )
        .await?;
        anyhow::ensure!(
            second_after_first_stop
                .nodes
                .first()
                .map(|node| node.run_state.as_str())
                == Some("running"),
            "stopping one Basecamp Channel Indexer changed the other Channel instance"
        );
        let first_config_after_stop = basecamp_config_snapshot_with_configs(
            directory.path(),
            "default",
            &config_request(&first_channel),
            &configs,
            &transport,
        )
        .await?;
        anyhow::ensure!(
            first_config_after_stop.editable,
            "stopped Basecamp Channel Indexer remained configuration-locked"
        );

        let state = implementation
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Basecamp Indexer test lock poisoned"))?;
        let first_instance = basecamp_instance_id(&network_scope(), &first_channel)?;
        let second_instance = basecamp_instance_id(&network_scope(), &second_channel)?;
        anyhow::ensure!(
            state
                .instances
                .get(&first_instance)
                .map(|instance| instance.state.as_str())
                == Some("stopped"),
            "first scoped Basecamp Indexer state was not retained independently"
        );
        anyhow::ensure!(
            state
                .instances
                .get(&second_instance)
                .map(|instance| instance.state.as_str())
                == Some("running"),
            "second scoped Basecamp Indexer state was not retained independently"
        );
        let indexer_calls: Vec<_> = state
            .calls
            .iter()
            .filter(|call| call.module() == INDEXER_MODULE)
            .collect();
        anyhow::ensure!(
            !indexer_calls.is_empty()
                && indexer_calls.iter().all(|call| {
                    matches!(
                        call.instance_id(),
                        Some(instance_id) if instance_id == first_instance || instance_id == second_instance
                    )
                }),
            "Indexer calls were not bound to one of the two explicit Basecamp instances"
        );
        anyhow::ensure!(
            state
                .calls
                .iter()
                .filter(|call| call.module() == BASECAMP_CORE_SERVICE_MODULE)
                .all(|call| call.instance_id().is_none()),
            "Basecamp core service calls must remain unscoped"
        );
        anyhow::ensure!(
            state.subscriptions.iter().all(|subscription| {
                subscription.module == INDEXER_MODULE
                    && (subscription.instance_id == first_instance
                        || subscription.instance_id == second_instance)
                    && subscription.event == INDEXER_NODE_CHANGED_EVENT
            }),
            "Indexer lifecycle subscriptions were not exact-instance subscriptions"
        );
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_purge_uses_persisted_config_without_live_source_binding() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let channel_id = "01".repeat(32);
        let source = module_source_config(&channel_id);
        let context = ChannelIndexerConfigContext {
            channel_id: channel_id.clone(),
            bedrock_endpoint: "http://127.0.0.1:8080/".to_owned(),
            binding: SourceBinding {
                config_revision: source.config_revision,
                source_id: "src_selected".to_owned(),
                target_fingerprint: "sha256:test".to_owned(),
            },
        };
        let config_path =
            channel_indexer_config_path(directory.path(), &network_scope(), &channel_id)?;
        write_indexer_config_bytes(
            directory.path(),
            &config_path,
            &default_indexer_config_bytes(&context)?,
        )?;
        let implementation = Arc::new(BasecampIndexerTestTransport::new());
        let transport: SharedModuleTransport = implementation.clone();
        let request = ChannelIndexerActionRequest {
            action: NodeAction::Purge,
            network_scope: network_scope(),
            channel_id: channel_id.clone(),
            bedrock_endpoint: None,
            source_config_revision: None,
            selected_sequencer_source_id: None,
        };

        let report =
            basecamp_action_with_configs("default", directory.path(), &[], request, &transport)
                .await?;
        anyhow::ensure!(
            report
                .operations
                .first()
                .map(|operation| operation.status.as_str())
                == Some("purged"),
            "Basecamp purge did not complete without live source bindings"
        );
        let state = implementation
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Basecamp Indexer test lock poisoned"))?;
        anyhow::ensure!(
            state.calls.iter().any(|call| {
                call.module() == INDEXER_MODULE && call.method() == "reset_storage"
            }),
            "Basecamp purge did not dispatch reset_storage"
        );
        Ok(())
    }

    #[test]
    fn maintenance_cleanup_control_survives_parent_cancellation() -> Result<()> {
        let cancellation = CancellationToken::new();
        let parent = CommandControl::new(
            cancellation.clone(),
            Instant::now()
                .checked_add(Duration::from_secs(1))
                .context("parent cleanup control deadline overflow")?,
        );
        cancellation.cancel();
        parent
            .check_active()
            .expect_err("cancelled parent control unexpectedly remained active");

        let cleanup = fresh_maintenance_cleanup_control();
        cleanup
            .check_active()
            .context("maintenance cleanup control inherited parent cancellation")?;
        anyhow::ensure!(
            cleanup.deadline() > Instant::now(),
            "maintenance cleanup control did not receive a bounded future deadline"
        );
        Ok(())
    }
}
