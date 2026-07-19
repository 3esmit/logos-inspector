use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use tokio_util::sync::CancellationToken;

use crate::{
    inspection::NetworkScope,
    modules::logos_core::{
        LogoscoreCliTransport, SharedModuleTransport, normalize_module_call_value,
    },
    source_routing::channel_sources::{
        ChannelSourceConfig, ChannelSourceTarget, indexer, load_channel_source_configs,
    },
    support::{
        command_runner::{CommandControl, CommandTerminated},
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
        LocalNodeSummary, LocalNodesState, NodeAction, NodeKind, NodeLifecycleState,
    },
    process::{process_group_has_live_members, spawn_detached, stop_process},
    runtime::{self, LogoscoreRuntimeProfile},
    workflow::normalized_profile,
};

const STATE_FILE: &str = "channel_indexers.json";
const STATE_VERSION: u32 = 1;
const OPERATION_HISTORY_LIMIT: usize = 100;
const STATUS_TIMEOUT: Duration = Duration::from_secs(2);

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

#[derive(Debug, Clone)]
struct SourceBinding {
    config_revision: u64,
    source_id: String,
    target_fingerprint: String,
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
    if !matches!(request.action, NodeAction::Start | NodeAction::Stop) {
        bail!("Channel Indexer only supports Start and Stop actions");
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
    let base_runtime = context.base_runtime.context(
        "configure an Inspector-managed LogosCore runtime before starting a Channel Indexer",
    )?;
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
    write_indexer_config(record)?;

    let result = start_runtime_and_indexer(record, context.control);
    if let Err(error) = result {
        let cleanup_error = stop_runtime(record, None).err();
        record.state = "stopped".to_owned();
        record.indexed_block_id = None;
        record.last_error = Some(match cleanup_error {
            Some(cleanup_error) => format!("{error}; cleanup failed: {cleanup_error}"),
            None => error.to_string(),
        });
        return Err(error);
    }

    record.state = "starting".to_owned();
    record.indexed_block_id = None;
    record.last_error = None;
    Ok(ActionOutcome::starting(format!(
        "Started isolated Channel Indexer for `{}` bound to Sequencer source `{}`",
        context.channel_id, record.selected_sequencer_source_id
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
    let stop_spec = command_spec_for(
        NodeKind::Indexer,
        NodeAction::Stop,
        &record.config_path(),
        &record.data_path(),
        None,
    )
    .context("Channel Indexer stop is not implemented")?;
    let module_detail = match execute_command_spec(&stop_spec, Some(&cli), control) {
        Ok(value) => operation_detail_from_value(&value),
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

fn start_runtime_and_indexer(
    record: &mut ChannelIndexerRecord,
    control: Option<&CommandControl>,
) -> Result<()> {
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
    execute_command_spec(&spec, Some(&cli), control)?;
    Ok(())
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
    let binding = current_source_binding(&request.network_scope, channel_id)?;
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
    let Some(base_runtime) = base_runtime.filter(|runtime| runtime.is_managed()) else {
        return PackagePrerequisite::missing(
            "configure an Inspector-managed LogosCore runtime under System / Local Nodes",
        );
    };
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
    PackagePrerequisite {
        installed: true,
        detail: "exact lez_indexer_module package is available to isolated Channel runtimes"
            .to_owned(),
    }
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
    if package_installed && source_configured && legacy_inactive {
        return vec![NodeAction::Start];
    }
    Vec::new()
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

fn write_indexer_config(record: &ChannelIndexerRecord) -> Result<()> {
    let value = crate::source_routing::execution_zone_layer::managed_indexer_channel_config(
        &record.channel_id,
        &record.bedrock_endpoint,
    );
    let path = record.config_path();
    if let Some(parent) = Path::new(&path).parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create Channel Indexer config directory {}",
                parent.display()
            )
        })?;
    }
    let text = serde_json::to_string_pretty(&value)
        .context("failed to serialize Channel Indexer config")?;
    fs::write(&path, text).with_context(|| format!("failed to write Channel Indexer config {path}"))
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
mod tests {
    use anyhow::{Context as _, Result};

    use super::*;
    use crate::source_routing::channel_sources::{
        ChannelSourceTarget, ConfiguredIndexerSource, ConfiguredSequencerSource,
        PersistedSequencerAttestation,
    };

    fn network_scope() -> NetworkScope {
        NetworkScope::GenesisId {
            genesis_id: "ab".repeat(32),
        }
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
            channel_actions(true, true, true, true, "stopped")
                == vec![NodeAction::Start, NodeAction::Stop]
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
}
