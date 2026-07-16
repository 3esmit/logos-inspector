use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, bail};
use serde_json::Value;

use crate::modules::logos_core::normalize_module_call_value;
use crate::support::{
    command_runner::{CommandControl, CommandTerminated},
    time::now_millis,
};

use super::adapters::{NodeActionPolicy, NodeConfigContext, adapter_for};
use super::commands::{
    command_spec_for, ensure_module_loaded, execute_command_spec, execute_ready_process_spec,
    operation_detail_from_value,
};
use super::lifecycle::{has_event_contract, reset_module_contexts};
use super::model::{
    LocalDevnetRecord, LocalNodeActionRequest, LocalNodeConfigRecord, LocalNodeDeployment,
    LocalNodeOperationReport, LocalNodesState, NodeAction, NodeKind, NodeLifecycleState,
};
use super::paths::{path_is_inside, remove_dir_inside};
use super::process::{
    find_command, process_group_has_live_members, process_group_is_alive, process_is_alive,
    spawn_detached, stop_process,
};
use super::runtime::LogoscoreRuntimeProfile;
use super::workflow::node_set_for_profile;

const MANIFEST_FILE: &str = "local-network.json";
const TESTNET_ID: &str = "logos-testnet";
const MESSAGING_CONTEXT_PROBE_ATTEMPTS: usize = 20;
const MESSAGING_CONTEXT_PROBE_INTERVAL: Duration = Duration::from_millis(250);
const MESSAGING_CONTEXT_RUNTIME_RESTARTS: usize = 1;
const RUNTIME_PROCESS_GROUP_REAP_TIMEOUT: Duration = Duration::from_secs(5);
const RUNTIME_PROCESS_GROUP_REAP_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct LocalNodeActionWorkspace;

pub(super) struct LocalNodeActionResult {
    pub(super) report: LocalNodeOperationReport,
    pub(super) interruption: Option<anyhow::Error>,
}

impl LocalNodeActionWorkspace {
    pub(super) fn system() -> Self {
        Self
    }

    pub(super) fn apply(
        self,
        state: &mut LocalNodesState,
        runtime: &mut Option<LogoscoreRuntimeProfile>,
        runtime_config_root: &Path,
        normalized_profile: &str,
        request: &LocalNodeActionRequest,
        control: Option<&CommandControl>,
    ) -> LocalNodeActionResult {
        dispatch_action(
            state,
            runtime,
            runtime_config_root,
            normalized_profile,
            request,
            control,
        )
    }
}

fn dispatch_action(
    state: &mut LocalNodesState,
    runtime: &mut Option<LogoscoreRuntimeProfile>,
    runtime_config_root: &Path,
    normalized_profile: &str,
    request: &LocalNodeActionRequest,
    control: Option<&CommandControl>,
) -> LocalNodeActionResult {
    if let Some(control) = control
        && let Err(error) = control.check_active()
    {
        return interrupted_operation(request, request.node, error.into());
    }
    match request.action {
        NodeAction::StartRuntime => {
            runtime_start(state, runtime, runtime_config_root, request, control)
        }
        NodeAction::StopRuntime => runtime_stop(state, runtime, request, control),
        NodeAction::NewNetwork => new_network(state, runtime.as_ref(), request),
        NodeAction::LoadNetwork => load_network(state, runtime.as_ref(), request),
        NodeAction::DeleteNetwork => delete_network(state, runtime.as_ref(), request),
        NodeAction::ResetNetwork => reset_network(state, runtime.as_ref(), request),
        NodeAction::Install => node_install(state, normalized_profile, request),
        NodeAction::Initialize => {
            node_initialize(state, runtime, normalized_profile, request, control)
        }
        NodeAction::Uninstall => node_uninstall(
            state,
            runtime.as_ref(),
            normalized_profile,
            request,
            control,
        ),
        NodeAction::Start => node_start(
            state,
            runtime.as_ref(),
            normalized_profile,
            request,
            control,
        ),
        NodeAction::Stop => node_stop(
            state,
            runtime.as_ref(),
            normalized_profile,
            request,
            control,
        ),
        NodeAction::Purge => node_purge(state, normalized_profile, request),
    }
}

fn new_network(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    request: &LocalNodeActionRequest,
) -> LocalNodeActionResult {
    operation_result(request, None, || {
        require_runtime_stopped(runtime)?;
        let id = request
            .network_id
            .as_deref()
            .map(sanitize_network_id)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("devnet-{}", now_millis()));
        if state.devnets.iter().any(|record| record.id == id) {
            bail!("local devnet `{id}` already exists");
        }

        let workspace_root = PathBuf::from(&state.managed_workspace_root);
        let workspace = workspace_root.join(&id);
        fs::create_dir_all(&workspace)
            .with_context(|| format!("failed to create workspace {}", workspace.display()))?;
        let now = now_millis();
        let record = LocalDevnetRecord {
            deployment: LocalNodeDeployment::LocalDevnet,
            id: id.clone(),
            label: request
                .label
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| id.clone()),
            workspace: workspace.display().to_string(),
            manifest_path: workspace.join(MANIFEST_FILE).display().to_string(),
            created_at: now,
            updated_at: now,
            nodes: node_set_for_profile("local")
                .into_iter()
                .map(|kind| default_node_config(&workspace, kind))
                .collect(),
        };
        generate_devnet_files(&record)?;
        write_devnet_manifest(&record)?;
        state.active_devnet = Some(record.id.clone());
        state.devnets.push(record);
        Ok(OperationOutcome {
            status: "created".to_owned(),
            detail: format!("created local devnet `{id}`"),
            command: None,
        })
    })
}

fn load_network(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    request: &LocalNodeActionRequest,
) -> LocalNodeActionResult {
    operation_result(request, None, || {
        require_runtime_stopped(runtime)?;
        let workspace = request
            .workspace_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .context("workspace path is required")?;
        let manifest_path = Path::new(workspace).join(MANIFEST_FILE);
        let text = fs::read_to_string(&manifest_path)
            .with_context(|| format!("failed to read {}", manifest_path.display()))?;
        let mut record: LocalDevnetRecord = serde_json::from_str(&text)
            .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
        record.workspace = Path::new(workspace).display().to_string();
        record.manifest_path = manifest_path.display().to_string();
        record.updated_at = now_millis();
        if let Some(existing) = state.devnet_mut(&record.id) {
            *existing = record.clone();
        } else {
            state.devnets.push(record.clone());
        }
        state.active_devnet = Some(record.id.clone());
        Ok(OperationOutcome {
            status: "loaded".to_owned(),
            detail: format!("loaded local devnet `{}`", record.id),
            command: None,
        })
    })
}

fn delete_network(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    request: &LocalNodeActionRequest,
) -> LocalNodeActionResult {
    operation_result(request, None, || {
        require_runtime_stopped(runtime)?;
        let network_id = target_network_id(state, request)?;
        stop_all_owned_processes(state, &network_id);
        let Some(position) = state
            .devnets
            .iter()
            .position(|record| record.id == network_id)
        else {
            bail!("local devnet `{network_id}` was not found");
        };
        let record = state.devnets.remove(position);
        remove_dir_inside(
            Path::new(&state.managed_workspace_root),
            Path::new(&record.workspace),
        )?;
        if state.active_devnet.as_deref() == Some(&network_id) {
            state.active_devnet = None;
        }
        Ok(OperationOutcome {
            status: "deleted".to_owned(),
            detail: format!("deleted local devnet `{network_id}`"),
            command: None,
        })
    })
}

fn reset_network(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    request: &LocalNodeActionRequest,
) -> LocalNodeActionResult {
    operation_result(request, None, || {
        require_runtime_stopped(runtime)?;
        let network_id = target_network_id(state, request)?;
        stop_all_owned_processes(state, &network_id);
        let workspace_root = PathBuf::from(&state.managed_workspace_root);
        let Some(record) = state.devnet_mut(&network_id) else {
            bail!("local devnet `{network_id}` was not found");
        };
        let workspace = PathBuf::from(&record.workspace);
        for node in &mut record.nodes {
            remove_dir_inside(&workspace_root, Path::new(&node.data_dir))?;
            node.process_id = None;
            fs::create_dir_all(&node.data_dir)
                .with_context(|| format!("failed to recreate {}", node.data_dir))?;
        }
        record.updated_at = now_millis();
        generate_devnet_files(record)?;
        write_devnet_manifest(record)?;
        if !path_is_inside(&workspace_root, &workspace) {
            bail!("workspace is outside managed local node root");
        }
        Ok(OperationOutcome {
            status: "reset".to_owned(),
            detail: format!("reset local devnet `{network_id}`"),
            command: None,
        })
    })
}

fn runtime_start(
    state: &mut LocalNodesState,
    runtime: &mut Option<LogoscoreRuntimeProfile>,
    runtime_config_root: &Path,
    request: &LocalNodeActionRequest,
    control: Option<&CommandControl>,
) -> LocalNodeActionResult {
    operation_result(request, None, || {
        let mut profile = LogoscoreRuntimeProfile::create_or_restart(
            runtime_config_root,
            runtime.as_ref(),
            request.runtime_binary_path.as_deref(),
            request.runtime_modules_dir.as_deref(),
        )?;
        let command = profile.daemon_command()?;
        let display = command_display(&command);
        let process_id =
            super::process::spawn_detached(command, "Inspector-managed logoscore daemon")?;
        profile.daemon_process_id = Some(process_id);
        reset_module_contexts(state);
        let readiness = match control {
            Some(control) => profile.wait_until_ready_controlled(control),
            None => profile.wait_until_ready(),
        };
        let still_running = profile.is_running();
        *runtime = Some(profile);
        match readiness {
            Ok(()) => Ok(OperationOutcome {
                status: "started".to_owned(),
                detail: "Inspector-managed logoscore daemon is ready".to_owned(),
                command: Some(display),
            }),
            Err(error) if is_control_interruption(&error) => Err(error),
            Err(error) if still_running => Ok(OperationOutcome {
                status: "starting".to_owned(),
                detail: operation_error_detail(&error),
                command: Some(display),
            }),
            Err(error) => Ok(OperationOutcome {
                status: "failed".to_owned(),
                detail: operation_error_detail(&error),
                command: Some(display),
            }),
        }
    })
}

fn runtime_stop(
    state: &mut LocalNodesState,
    runtime: &mut Option<LogoscoreRuntimeProfile>,
    request: &LocalNodeActionRequest,
    control: Option<&CommandControl>,
) -> LocalNodeActionResult {
    operation_result(request, None, || {
        let Some(profile) = runtime.as_mut() else {
            return Ok(OperationOutcome {
                status: "needs_configuration".to_owned(),
                detail: "no Inspector-managed logoscore runtime is configured".to_owned(),
                command: None,
            });
        };
        if !profile.is_managed() {
            return Ok(OperationOutcome {
                status: "needs_configuration".to_owned(),
                detail: "external logoscore runtimes are never stopped by Inspector".to_owned(),
                command: None,
            });
        }
        if !profile.is_running() {
            reap_runtime_process_group(profile)?;
            profile.daemon_process_id = None;
            reset_module_contexts(state);
            return Ok(OperationOutcome {
                status: "stopped".to_owned(),
                detail: "Inspector-managed logoscore daemon is already stopped".to_owned(),
                command: None,
            });
        }
        let cli = profile.cli_runtime()?;
        let value = match control {
            Some(control) => cli.stop_controlled(control.clone())?,
            None => cli.stop()?,
        }
        .value;
        let stopped = match control {
            Some(control) => profile.wait_until_stopped_controlled(control)?,
            None => profile.wait_until_stopped(),
        };
        if stopped {
            reap_runtime_process_group(profile)?;
            profile.daemon_process_id = None;
            reset_module_contexts(state);
            return Ok(OperationOutcome {
                status: "stopped".to_owned(),
                detail: operation_detail_from_value(&value),
                command: Some("logoscore --config-dir <managed> stop --json".to_owned()),
            });
        }
        Ok(OperationOutcome {
            status: "stopping".to_owned(),
            detail: "stop request accepted; waiting for managed daemon exit".to_owned(),
            command: Some("logoscore --config-dir <managed> stop --json".to_owned()),
        })
    })
}

fn reap_runtime_process_group(profile: &LogoscoreRuntimeProfile) -> Result<()> {
    let Some(process_id) = profile.daemon_process_id else {
        return Ok(());
    };
    if !process_group_is_alive(process_id) {
        return Ok(());
    }
    stop_process(process_id).with_context(|| {
        format!(
            "failed to stop remaining processes owned by Inspector-managed logoscore runtime {process_id}"
        )
    })
}

fn restart_managed_runtime_for_messaging_context(
    runtime: &mut LogoscoreRuntimeProfile,
    control: Option<&CommandControl>,
) -> Result<()> {
    if runtime.is_running() {
        return Ok(());
    }
    if let Some(process_id) = runtime.daemon_process_id {
        reap_runtime_process_group(runtime)?;
        wait_for_runtime_process_group_exit(process_id, control)?;
    }
    if let Some(control) = control {
        control.check_active()?;
    }
    runtime.daemon_process_id = None;
    let command = runtime.daemon_command()?;
    let process_id = spawn_detached(command, "Inspector-managed logoscore daemon")?;
    runtime.daemon_process_id = Some(process_id);
    match control {
        Some(control) => runtime.wait_until_ready_controlled(control),
        None => runtime.wait_until_ready(),
    }
    .context("restarted Inspector-managed logoscore daemon did not become ready")?;

    let cli = runtime.cli_runtime()?;
    match control {
        Some(control) => cli.ensure_module_loaded_controlled("delivery_module", control.clone()),
        None => cli.ensure_module_loaded("delivery_module"),
    }
    .context("restarted Inspector-managed logoscore daemon did not load delivery_module")
}

fn wait_for_runtime_process_group_exit(
    process_id: u32,
    control: Option<&CommandControl>,
) -> Result<()> {
    let deadline = Instant::now() + RUNTIME_PROCESS_GROUP_REAP_TIMEOUT;
    while process_group_has_live_members(process_id) {
        if let Some(control) = control {
            control.check_active()?;
        }
        if Instant::now() >= deadline {
            bail!(
                "Inspector-managed logoscore process group {process_id} still has live members after termination"
            );
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        thread::sleep(RUNTIME_PROCESS_GROUP_REAP_POLL_INTERVAL.min(remaining));
    }
    Ok(())
}

fn node_install(
    state: &mut LocalNodesState,
    profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeActionResult {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let adapter = adapter_for(kind);
        let policy = adapter.action_policy(NodeAction::Install);
        if let Some(reason) = policy.blocked_reason() {
            return Ok(needs_configuration(reason));
        }
        let NodeActionPolicy::RegisterExecutable {
            program: executable,
        } = policy
        else {
            bail!(
                "{} adapter returned an invalid install policy",
                adapter.label()
            );
        };
        let Some(binary) = find_command(executable) else {
            return Ok(needs_configuration(&format!("{executable} not found")));
        };
        let record = active_topology_mut(state, profile)?;
        let config = required_node_config(record, kind)?;
        config.package_path = Some(binary);
        config.installed = true;
        record.updated_at = now_millis();
        write_devnet_manifest(record)?;
        Ok(OperationOutcome {
            status: "installed".to_owned(),
            detail: format!("{executable} registered"),
            command: None,
        })
    })
}

fn node_initialize(
    state: &mut LocalNodesState,
    runtime: &mut Option<LogoscoreRuntimeProfile>,
    profile: &str,
    request: &LocalNodeActionRequest,
    control: Option<&CommandControl>,
) -> LocalNodeActionResult {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let adapter = adapter_for(kind);
        let policy = adapter.action_policy(NodeAction::Initialize);
        if let Some(reason) = policy.blocked_reason() {
            return Ok(needs_configuration(reason));
        }
        let NodeActionPolicy::ExecuteManaged { ensure_loaded, .. } = policy else {
            bail!(
                "{} adapter returned an invalid initialize policy",
                adapter.label()
            );
        };
        let cli = {
            let Some(runtime) = managed_runtime(runtime.as_ref()) else {
                return Ok(needs_configuration(
                    "start an Inspector-managed logoscore runtime before initializing a module node",
                ));
            };
            runtime.cli_runtime()?
        };
        let (spec, reuse_generated_config) = {
            let record = active_topology_mut(state, profile)?;
            let config = required_node_config(record, kind)?;
            let action_config_path = action_config_path(config, NodeAction::Initialize);
            let spec = command_spec_for(
                kind,
                NodeAction::Initialize,
                action_config_path,
                &config.data_dir,
                config.port,
            )
            .with_context(|| format!("{} initialization is not implemented", adapter.label()))?;
            (spec, reusable_generated_config(adapter, config))
        };
        if ensure_loaded {
            ensure_module_loaded(&spec, Some(&cli), control)?;
        }
        if reuse_generated_config {
            mark_node_initialized(state, profile, kind, &spec.program)?;
            return Ok(OperationOutcome {
                status: "initialized".to_owned(),
                detail: "reused existing generated configuration; module is loaded".to_owned(),
                command: adapter
                    .managed_contract()
                    .map(|contract| format!("logoscore module load {}", contract.module_id())),
            });
        }
        match execute_command_spec(&spec, Some(&cli), control) {
            Ok(value) => {
                mark_node_initialized(state, profile, kind, &spec.program)?;
                Ok(OperationOutcome {
                    status: "initialized".to_owned(),
                    detail: operation_detail_from_value(&value),
                    command: Some(spec.display),
                })
            }
            Err(error)
                if kind == NodeKind::Messaging && is_ambiguous_messaging_create_error(&error) =>
            {
                let runtime = runtime
                    .as_mut()
                    .filter(|profile| profile.is_managed())
                    .context(
                        "Inspector-managed logoscore runtime disappeared during Messaging recovery",
                    )?;
                match verify_messaging_context(state, runtime, control) {
                    Ok(verification) => {
                        mark_node_initialized(state, profile, kind, &spec.program)?;
                        Ok(OperationOutcome {
                            status: "initialized".to_owned(),
                            detail: messaging_context_recovery_detail(&verification),
                            command: Some(spec.display),
                        })
                    }
                    Err(probe_error) if is_control_interruption(&probe_error) => Err(probe_error),
                    Err(probe_error) => Ok(OperationOutcome {
                        status: "failed".to_owned(),
                        detail: format!(
                            "{error:#}; Messaging context verification failed: {probe_error:#}"
                        ),
                        command: Some(spec.display),
                    }),
                }
            }
            Err(error) if is_control_interruption(&error) => Err(error),
            Err(error) => Ok(OperationOutcome {
                status: "failed".to_owned(),
                detail: operation_error_detail(&error),
                command: Some(spec.display),
            }),
        }
    })
}

fn mark_node_initialized(
    state: &mut LocalNodesState,
    profile: &str,
    kind: NodeKind,
    package_path: &str,
) -> Result<()> {
    let record = active_topology_mut(state, profile)?;
    let config = required_node_config(record, kind)?;
    config.installed = true;
    config.package_path = Some(package_path.to_owned());
    config.lifecycle_state = NodeLifecycleState::Stopped;
    config.pending_lifecycle_action = None;
    record.updated_at = now_millis();
    write_devnet_manifest(record)
}

fn is_ambiguous_messaging_create_error(error: &anyhow::Error) -> bool {
    error.to_string().contains("RPC_FAILED")
}

struct MessagingContextVerification {
    peer_id: String,
    restarted_runtime: bool,
}

fn messaging_context_recovery_detail(verification: &MessagingContextVerification) -> String {
    if verification.restarted_runtime {
        format!(
            "createNode response was lost; restarted the Inspector-managed LogosCore runtime and verified Messaging context with MyPeerId `{}`",
            verification.peer_id
        )
    } else {
        format!(
            "createNode response was lost; verified Messaging context with MyPeerId `{}`",
            verification.peer_id
        )
    }
}

fn verify_messaging_context(
    state: &mut LocalNodesState,
    runtime: &mut LogoscoreRuntimeProfile,
    control: Option<&CommandControl>,
) -> Result<MessagingContextVerification> {
    let deadline = control.map_or_else(
        || {
            Instant::now()
                + MESSAGING_CONTEXT_PROBE_ATTEMPTS as u32 * MESSAGING_CONTEXT_PROBE_INTERVAL
        },
        CommandControl::deadline,
    );
    let mut last_error = None;
    let mut runtime_restarts = 0;
    for attempt in 0..MESSAGING_CONTEXT_PROBE_ATTEMPTS {
        if let Some(control) = control {
            control.check_active()?;
        }
        if !runtime.is_running() {
            if runtime_restarts >= MESSAGING_CONTEXT_RUNTIME_RESTARTS {
                bail!(
                    "Messaging context verification found the restarted Inspector-managed LogosCore runtime stopped"
                );
            }
            reset_module_contexts(state);
            restart_managed_runtime_for_messaging_context(runtime, control)?;
            runtime_restarts += 1;
        }
        match messaging_peer_id(runtime, control) {
            Ok(peer_id) => {
                return Ok(MessagingContextVerification {
                    peer_id,
                    restarted_runtime: runtime_restarts > 0,
                });
            }
            Err(error) => last_error = Some(error),
        }
        if attempt + 1 == MESSAGING_CONTEXT_PROBE_ATTEMPTS {
            break;
        }
        if let Some(control) = control {
            control.check_active()?;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        thread::sleep(MESSAGING_CONTEXT_PROBE_INTERVAL.min(remaining));
    }
    let detail = last_error
        .map(|error| error.to_string())
        .unwrap_or_else(|| "no getNodeInfo response".to_owned());
    bail!("Messaging context verification did not find MyPeerId: {detail}")
}

fn messaging_peer_id(
    runtime: &LogoscoreRuntimeProfile,
    control: Option<&CommandControl>,
) -> Result<String> {
    let cli = runtime.cli_runtime()?;
    let args = ["MyPeerId".to_owned()];
    let response = match control {
        Some(control) => cli.call_checked_controlled(
            "delivery_module",
            "getNodeInfo",
            "getNodeInfo(QString)",
            &args,
            control.clone(),
        ),
        None => cli.call_checked(
            "delivery_module",
            "getNodeInfo",
            "getNodeInfo(QString)",
            &args,
        ),
    }?;
    let value = response
        .get("value")
        .cloned()
        .context("Messaging getNodeInfo returned no LogosCore value")?;
    let peer_id = normalize_module_call_value("delivery_module", "getNodeInfo", value)?;
    peer_id
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .context("Messaging getNodeInfo returned an empty MyPeerId")
}

fn node_uninstall(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    profile: &str,
    request: &LocalNodeActionRequest,
    control: Option<&CommandControl>,
) -> LocalNodeActionResult {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let adapter = adapter_for(kind);
        let policy = adapter.action_policy(NodeAction::Uninstall);
        if let Some(reason) = policy.blocked_reason() {
            return Ok(needs_configuration(reason));
        }
        let record = active_topology_mut(state, profile)?;
        let config = required_node_config(record, kind)?;
        if policy == NodeActionPolicy::RemoveExecutableRegistration {
            stop_owned_process(config);
            config.installed = false;
            config.package_path = None;
            record.updated_at = now_millis();
            write_devnet_manifest(record)?;
            return Ok(OperationOutcome {
                status: "uninstalled".to_owned(),
                detail: format!("{} registration removed", adapter.label()),
                command: None,
            });
        }
        if !matches!(policy, NodeActionPolicy::ExecuteManaged { .. }) {
            bail!(
                "{} adapter returned an invalid uninstall policy",
                adapter.label()
            );
        }
        let Some(runtime) = managed_runtime(runtime) else {
            return Ok(needs_configuration(
                "start the Inspector-managed logoscore runtime before removing a module context",
            ));
        };
        if config.lifecycle_state.is_pending()
            || config.lifecycle_state == NodeLifecycleState::Running
        {
            return Ok(needs_configuration(
                "stop the module node and wait for lifecycle confirmation before removing its context",
            ));
        }
        let Some(spec) = command_spec_for(
            kind,
            NodeAction::Uninstall,
            &config.config_path,
            &config.data_dir,
            config.port,
        ) else {
            return Ok(needs_configuration(
                "this module has no verified context-destroy contract; stop the managed runtime to clear it",
            ));
        };
        let cli = runtime.cli_runtime()?;
        match execute_command_spec(&spec, Some(&cli), control) {
            Ok(value) => {
                clear_module_context(config);
                record.updated_at = now_millis();
                write_devnet_manifest(record)?;
                Ok(OperationOutcome {
                    status: "uninstalled".to_owned(),
                    detail: operation_detail_from_value(&value),
                    command: Some(spec.display),
                })
            }
            Err(error) if is_control_interruption(&error) => Err(error),
            Err(error) => Ok(OperationOutcome {
                status: "failed".to_owned(),
                detail: operation_error_detail(&error),
                command: Some(spec.display),
            }),
        }
    })
}

fn node_start(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    profile: &str,
    request: &LocalNodeActionRequest,
    control: Option<&CommandControl>,
) -> LocalNodeActionResult {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let adapter = adapter_for(kind);
        let policy = adapter.action_policy(NodeAction::Start);
        if let Some(reason) = policy.blocked_reason() {
            return Ok(needs_configuration(reason));
        }
        let record = active_topology_mut(state, profile)?;
        let config = required_node_config(record, kind)?;
        fs::create_dir_all(&config.data_dir)
            .with_context(|| format!("failed to create {}", config.data_dir))?;
        let spec = command_spec_for(
            kind,
            NodeAction::Start,
            &config.config_path,
            &config.data_dir,
            config.port,
        )
        .with_context(|| format!("{} start is not implemented", adapter.label()))?;
        if policy == NodeActionPolicy::ExecuteDetached {
            let execution = match adapter.startup_rpc_health_method() {
                Some(method) => execute_ready_process_spec(
                    &spec,
                    config
                        .endpoint
                        .as_deref()
                        .context("registered process RPC endpoint is required")?,
                    method,
                    control,
                ),
                None => execute_command_spec(&spec, None, control),
            };
            return match execution {
                Ok(value) => {
                    config.process_id = value
                        .get("pid")
                        .and_then(Value::as_u64)
                        .and_then(|pid| u32::try_from(pid).ok());
                    config.installed = true;
                    record.updated_at = now_millis();
                    write_devnet_manifest(record)?;
                    Ok(OperationOutcome {
                        status: "started".to_owned(),
                        detail: operation_detail_from_value(&value),
                        command: Some(spec.display),
                    })
                }
                Err(error) if is_control_interruption(&error) => Err(error),
                Err(error) => Ok(OperationOutcome {
                    status: "failed".to_owned(),
                    detail: operation_error_detail(&error),
                    command: Some(spec.display),
                }),
            };
        }
        let NodeActionPolicy::ExecuteManaged {
            ensure_loaded,
            requires_installed_context,
        } = policy
        else {
            bail!(
                "{} adapter returned an invalid start policy",
                adapter.label()
            );
        };
        let Some(runtime) = managed_runtime(runtime) else {
            return Ok(needs_configuration(
                "start an Inspector-managed logoscore runtime before starting a module node",
            ));
        };
        if config.lifecycle_state.is_pending() {
            return Ok(needs_configuration(
                "a module lifecycle action is already pending confirmation",
            ));
        }
        if requires_installed_context && !config.installed {
            return Ok(needs_configuration(
                "initialize the module node before starting it",
            ));
        }
        let cli = runtime.cli_runtime()?;
        if ensure_loaded {
            ensure_module_loaded(&spec, Some(&cli), control)?;
        }
        match execute_command_spec(&spec, Some(&cli), control) {
            Ok(value) => {
                config.installed = true;
                if has_event_contract(kind, NodeAction::Start) {
                    config.lifecycle_state = NodeLifecycleState::Starting;
                    config.pending_lifecycle_action = Some(NodeAction::Start);
                } else {
                    config.lifecycle_state = NodeLifecycleState::Unknown;
                    config.pending_lifecycle_action = None;
                }
                record.updated_at = now_millis();
                write_devnet_manifest(record)?;
                Ok(OperationOutcome {
                    status: "starting".to_owned(),
                    detail: lifecycle_dispatch_detail(kind, NodeAction::Start, &value),
                    command: Some(spec.display),
                })
            }
            Err(error) if is_control_interruption(&error) => Err(error),
            Err(error) => Ok(OperationOutcome {
                status: "failed".to_owned(),
                detail: operation_error_detail(&error),
                command: Some(spec.display),
            }),
        }
    })
}

fn node_stop(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    profile: &str,
    request: &LocalNodeActionRequest,
    control: Option<&CommandControl>,
) -> LocalNodeActionResult {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let adapter = adapter_for(kind);
        let policy = adapter.action_policy(NodeAction::Stop);
        if let Some(reason) = policy.blocked_reason() {
            return Ok(needs_configuration(reason));
        }
        let record = active_topology_mut(state, profile)?;
        let config = required_node_config(record, kind)?;
        if policy == NodeActionPolicy::ExecuteDetached {
            if config.process_id.is_none() {
                return Ok(needs_configuration(&format!(
                    "no Inspector-owned {} process is recorded",
                    adapter.label()
                )));
            }
            stop_owned_process(config);
            record.updated_at = now_millis();
            write_devnet_manifest(record)?;
            return Ok(OperationOutcome {
                status: "stopped".to_owned(),
                detail: format!("stopped recorded {} process", adapter.label()),
                command: None,
            });
        }
        let NodeActionPolicy::ExecuteManaged {
            requires_installed_context,
            ..
        } = policy
        else {
            bail!(
                "{} adapter returned an invalid stop policy",
                adapter.label()
            );
        };
        let Some(runtime) = managed_runtime(runtime) else {
            return Ok(needs_configuration(
                "start an Inspector-managed logoscore runtime before stopping a module node",
            ));
        };
        if config.lifecycle_state.is_pending() {
            return Ok(needs_configuration(
                "a module lifecycle action is already pending confirmation",
            ));
        }
        if requires_installed_context && !config.installed {
            return Ok(needs_configuration(
                "initialize the module node before stopping it",
            ));
        }
        let spec = command_spec_for(
            kind,
            NodeAction::Stop,
            &config.config_path,
            &config.data_dir,
            config.port,
        )
        .with_context(|| format!("{} stop is not implemented", adapter.label()))?;
        let cli = runtime.cli_runtime()?;
        match execute_command_spec(&spec, Some(&cli), control) {
            Ok(value) => {
                if has_event_contract(kind, NodeAction::Stop) {
                    config.lifecycle_state = NodeLifecycleState::Stopping;
                    config.pending_lifecycle_action = Some(NodeAction::Stop);
                } else {
                    config.lifecycle_state = NodeLifecycleState::Unknown;
                    config.pending_lifecycle_action = None;
                }
                record.updated_at = now_millis();
                write_devnet_manifest(record)?;
                Ok(OperationOutcome {
                    status: "stopping".to_owned(),
                    detail: lifecycle_dispatch_detail(kind, NodeAction::Stop, &value),
                    command: Some(spec.display),
                })
            }
            Err(error) if is_control_interruption(&error) => Err(error),
            Err(error) => Ok(OperationOutcome {
                status: "failed".to_owned(),
                detail: operation_error_detail(&error),
                command: Some(spec.display),
            }),
        }
    })
}

fn node_purge(
    state: &mut LocalNodesState,
    profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeActionResult {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let adapter = adapter_for(kind);
        let policy = adapter.action_policy(NodeAction::Purge);
        if let Some(reason) = policy.blocked_reason() {
            return Ok(needs_configuration(reason));
        }
        let NodeActionPolicy::PurgeData {
            requires_removed_context,
        } = policy
        else {
            bail!(
                "{} adapter returned an invalid purge policy",
                adapter.label()
            );
        };
        let workspace_root = PathBuf::from(&state.managed_workspace_root);
        let Some(record) = state.active_topology_mut(profile) else {
            bail!("active local node topology is required");
        };
        let Some(config) = node_config_mut(record, kind) else {
            bail!("{} config is not available", adapter.label());
        };
        if requires_removed_context && config.installed {
            return Ok(needs_configuration(
                "remove the module context before purging its data directory",
            ));
        }
        stop_owned_process(config);
        remove_dir_inside(&workspace_root, Path::new(&config.data_dir))?;
        fs::create_dir_all(&config.data_dir)
            .with_context(|| format!("failed to recreate {}", config.data_dir))?;
        config.process_id = None;
        config.lifecycle_state = NodeLifecycleState::NotInitialized;
        config.pending_lifecycle_action = None;
        record.updated_at = now_millis();
        write_devnet_manifest(record)?;
        Ok(OperationOutcome {
            status: "purged".to_owned(),
            detail: format!("purged {} data directory", adapter.label()),
            command: None,
        })
    })
}

fn require_runtime_stopped(runtime: Option<&LogoscoreRuntimeProfile>) -> Result<()> {
    if runtime.is_some_and(LogoscoreRuntimeProfile::is_running) {
        bail!(
            "stop the Inspector-managed logoscore runtime before changing Local Devnet workspaces"
        );
    }
    Ok(())
}

fn managed_runtime(runtime: Option<&LogoscoreRuntimeProfile>) -> Option<&LogoscoreRuntimeProfile> {
    runtime.filter(|profile| profile.is_managed() && profile.is_running())
}

fn needs_configuration(detail: &str) -> OperationOutcome {
    OperationOutcome {
        status: "needs_configuration".to_owned(),
        detail: detail.to_owned(),
        command: None,
    }
}

fn active_topology_mut<'a>(
    state: &'a mut LocalNodesState,
    profile: &str,
) -> Result<&'a mut LocalDevnetRecord> {
    state
        .active_topology_mut(profile)
        .context("active local node topology is required")
}

fn action_config_path(config: &LocalNodeConfigRecord, action: NodeAction) -> &str {
    if action == NodeAction::Initialize {
        config
            .initialization_config_path
            .as_deref()
            .unwrap_or(&config.config_path)
    } else {
        &config.config_path
    }
}

fn reusable_generated_config(
    adapter: &dyn super::adapters::LocalNodeAdapter,
    config: &LocalNodeConfigRecord,
) -> bool {
    adapter.preserve_generated_config_on_runtime_reset() && Path::new(&config.config_path).is_file()
}

fn required_node_config(
    record: &mut LocalDevnetRecord,
    kind: NodeKind,
) -> Result<&mut LocalNodeConfigRecord> {
    node_config_mut(record, kind)
        .with_context(|| format!("{} config is not available", adapter_for(kind).label()))
}

fn clear_module_context(config: &mut LocalNodeConfigRecord) {
    config.installed = false;
    config.package_path = None;
    config.module_path = None;
    config.process_id = None;
    config.lifecycle_state = NodeLifecycleState::NotInitialized;
    config.pending_lifecycle_action = None;
}

fn lifecycle_dispatch_detail(kind: NodeKind, action: NodeAction, value: &Value) -> String {
    let result = operation_detail_from_value(value);
    if has_event_contract(kind, action) {
        format!("{result}; waiting for module lifecycle event")
    } else {
        format!("{result}; no verified module lifecycle observer")
    }
}

fn command_display(command: &std::process::Command) -> String {
    let mut parts = vec![command.get_program().to_string_lossy().into_owned()];
    parts.extend(
        command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned()),
    );
    parts.join(" ")
}

struct OperationOutcome {
    status: String,
    detail: String,
    command: Option<String>,
}

fn operation_result(
    request: &LocalNodeActionRequest,
    node: Option<NodeKind>,
    operation: impl FnOnce() -> Result<OperationOutcome>,
) -> LocalNodeActionResult {
    let timestamp = now_millis();
    match operation() {
        Ok(outcome) => LocalNodeActionResult {
            report: LocalNodeOperationReport {
                id: format!("op-{timestamp}"),
                time: timestamp.to_string(),
                timestamp_millis: timestamp,
                action: request.action,
                node,
                network_id: request.network_id.clone(),
                status: outcome.status,
                detail: outcome.detail,
                command: outcome.command,
            },
            interruption: None,
        },
        Err(error) => interrupted_operation(request, node, error),
    }
}

fn interrupted_operation(
    request: &LocalNodeActionRequest,
    node: Option<NodeKind>,
    error: anyhow::Error,
) -> LocalNodeActionResult {
    let timestamp = now_millis();
    let detail = error.to_string();
    let interruption = is_control_interruption(&error).then_some(error);
    LocalNodeActionResult {
        report: LocalNodeOperationReport {
            id: format!("op-{timestamp}"),
            time: timestamp.to_string(),
            timestamp_millis: timestamp,
            action: request.action,
            node,
            network_id: request.network_id.clone(),
            status: "failed".to_owned(),
            detail,
            command: None,
        },
        interruption,
    }
}

fn is_control_interruption(error: &anyhow::Error) -> bool {
    error.downcast_ref::<CommandTerminated>().is_some()
}

fn operation_error_detail(error: &anyhow::Error) -> String {
    format!("{error:#}")
}

fn required_node(request: &LocalNodeActionRequest) -> Result<NodeKind> {
    request.node.context("node kind is required")
}

fn target_network_id(state: &LocalNodesState, request: &LocalNodeActionRequest) -> Result<String> {
    request
        .network_id
        .clone()
        .or_else(|| state.active_devnet.clone())
        .context("local devnet id is required")
}

fn default_node_config(workspace: &Path, kind: NodeKind) -> LocalNodeConfigRecord {
    let adapter = adapter_for(kind);
    let port = adapter.default_port();
    let config_name = if kind == NodeKind::Bedrock {
        "bedrock.yaml".to_owned()
    } else {
        format!("{}.json", kind.as_str())
    };
    LocalNodeConfigRecord {
        kind,
        config_path: workspace
            .join("configs")
            .join(config_name)
            .display()
            .to_string(),
        initialization_config_path: (kind == NodeKind::Bedrock).then(|| {
            workspace
                .join("configs/bedrock.init.json")
                .display()
                .to_string()
        }),
        data_dir: workspace
            .join("data")
            .join(kind.as_str())
            .display()
            .to_string(),
        endpoint: adapter.endpoint(port),
        port,
        package_path: None,
        module_path: None,
        process_id: None,
        installed: false,
        lifecycle_state: NodeLifecycleState::NotInitialized,
        pending_lifecycle_action: None,
    }
}

fn generate_devnet_files(record: &LocalDevnetRecord) -> Result<()> {
    generate_topology_files(record, true)
}

fn generate_topology_files(record: &LocalDevnetRecord, overwrite: bool) -> Result<()> {
    for node in &record.nodes {
        fs::create_dir_all(&node.data_dir)
            .with_context(|| format!("failed to create {}", node.data_dir))?;
        let write_path = node
            .initialization_config_path
            .as_deref()
            .unwrap_or(&node.config_path);
        let config_path = PathBuf::from(write_path);
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        if !overwrite && config_path.is_file() {
            continue;
        }
        let value = generated_node_config(record, node);
        let text = serde_json::to_string_pretty(&value)
            .context("failed to serialize local node config")?;
        fs::write(&config_path, text)
            .with_context(|| format!("failed to write {}", config_path.display()))?;
    }
    Ok(())
}

fn generated_node_config(record: &LocalDevnetRecord, node: &LocalNodeConfigRecord) -> Value {
    adapter_for(node.kind).build_config(NodeConfigContext {
        network_id: &record.id,
        config_path: &node.config_path,
        data_dir: &node.data_dir,
        endpoint: node.endpoint.as_deref(),
        port: node.port,
        public_testnet: record.deployment == LocalNodeDeployment::PublicTestnet,
    })
}

pub(super) fn ensure_testnet_topology(state: &mut LocalNodesState) -> Result<bool> {
    if let Some(record) = state.testnet.as_ref() {
        generate_topology_files(record, false)?;
        return Ok(false);
    }

    let workspace = PathBuf::from(&state.managed_workspace_root).join(TESTNET_ID);
    fs::create_dir_all(&workspace)
        .with_context(|| format!("failed to create workspace {}", workspace.display()))?;
    let now = now_millis();
    let record = LocalDevnetRecord {
        deployment: LocalNodeDeployment::PublicTestnet,
        id: TESTNET_ID.to_owned(),
        label: "Logos Testnet".to_owned(),
        workspace: workspace.display().to_string(),
        manifest_path: workspace.join(MANIFEST_FILE).display().to_string(),
        created_at: now,
        updated_at: now,
        nodes: node_set_for_profile("default")
            .into_iter()
            .map(|kind| default_node_config(&workspace, kind))
            .collect(),
    };
    generate_topology_files(&record, false)?;
    write_devnet_manifest(&record)?;
    state.testnet = Some(record);
    state.version = 2;
    Ok(true)
}

fn write_devnet_manifest(record: &LocalDevnetRecord) -> Result<()> {
    let path = PathBuf::from(&record.manifest_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(record)
        .context("failed to serialize local devnet manifest")?;
    fs::write(&path, text).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn node_config_mut(
    record: &mut LocalDevnetRecord,
    kind: NodeKind,
) -> Option<&mut LocalNodeConfigRecord> {
    record.nodes.iter_mut().find(|node| node.kind == kind)
}

fn stop_all_owned_processes(state: &mut LocalNodesState, network_id: &str) {
    let Some(record) = state.devnet_mut(network_id) else {
        return;
    };
    for node in &mut record.nodes {
        stop_owned_process(node);
    }
}

fn stop_owned_process(node: &mut LocalNodeConfigRecord) {
    let Some(pid) = node.process_id else {
        return;
    };
    if process_is_alive(pid) {
        let _ignored = stop_process(pid);
    }
    node.process_id = None;
}

fn sanitize_network_id(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter_map(|ch| {
            if ch.is_ascii_alphanumeric() {
                Some(ch.to_ascii_lowercase())
            } else if ch == '-' || ch == '_' {
                Some(ch)
            } else if ch.is_ascii_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::{
        fs,
        os::unix::{fs::PermissionsExt as _, process::CommandExt as _},
        process::{self, Command},
        thread,
        time::{Duration, Instant},
    };

    use anyhow::{Context as _, Result, bail};

    use super::*;

    #[cfg(unix)]
    struct ProcessGroupGuard {
        process_id: u32,
    }

    #[cfg(unix)]
    impl Drop for ProcessGroupGuard {
        fn drop(&mut self) {
            if process_group_is_alive(self.process_id) {
                // INTENTIONAL: test cleanup must not mask the assertion that detects an orphan.
                let _ignored = stop_process(self.process_id);
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn runtime_stop_reaps_module_hosts_after_daemon_exit() -> Result<()> {
        runtime_stop_reaps_module_hosts(false)
    }

    #[test]
    fn operation_error_detail_keeps_nested_cause() -> Result<()> {
        let error = anyhow::anyhow!("inner CLI failure").context("outer operation failure");
        let detail = operation_error_detail(&error);
        if !detail.contains("outer operation failure") || !detail.contains("inner CLI failure") {
            bail!("operation error detail lost a cause: {detail}");
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn runtime_stop_reaps_module_hosts_after_daemon_already_exited() -> Result<()> {
        runtime_stop_reaps_module_hosts(true)
    }

    #[cfg(unix)]
    fn runtime_stop_reaps_module_hosts(daemon_already_exited: bool) -> Result<()> {
        // Arrange
        let directory = tempfile::tempdir()?;
        let cli = directory.path().join("logoscore");
        fs::write(
            &cli,
            r#"#!/bin/sh
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
if [ "$1" = "stop" ]; then
    kill -TERM "$(cat "$config_dir/daemon.pid")"
    printf '%s\n' '{"status":"stopped"}'
fi
"#,
        )?;
        let mut permissions = fs::metadata(&cli)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&cli, permissions)?;

        let child_path = directory.path().join("module-host.pid");
        let mut daemon_command = Command::new("/bin/sh");
        daemon_command
            .args([
                "-c",
                "sleep 30 & printf '%s' \"$!\" > \"$1\"; wait",
                "sh",
                child_path
                    .to_str()
                    .context("test child path is not valid UTF-8")?,
            ])
            .process_group(0);
        let mut daemon = daemon_command.spawn()?;
        let daemon_process_id = daemon.id();
        let _cleanup = ProcessGroupGuard {
            process_id: daemon_process_id,
        };
        let child_process_id = wait_for_process_id(&child_path)?;

        if daemon_already_exited {
            let status = Command::new("kill")
                .arg("-TERM")
                .arg(daemon_process_id.to_string())
                .status()
                .context("failed to terminate test daemon without its module host")?;
            if !status.success() {
                bail!("test daemon termination exited with {status}");
            }
            if !wait_until_stopped(daemon_process_id) {
                bail!("test daemon {daemon_process_id} did not stop");
            }
            if !process_is_alive(child_process_id) {
                bail!("test module host {child_process_id} stopped with daemon");
            }
        }

        let mut profile = LogoscoreRuntimeProfile::create_or_restart(
            directory.path(),
            None,
            Some(
                cli.to_str()
                    .context("test LogosCore CLI path is not valid UTF-8")?,
            ),
            Some(
                directory
                    .path()
                    .to_str()
                    .context("test modules path is not valid UTF-8")?,
            ),
        )?;
        fs::create_dir_all(&profile.config_dir)?;
        fs::write(
            Path::new(&profile.config_dir).join("daemon.pid"),
            daemon_process_id.to_string(),
        )?;
        profile.daemon_process_id = Some(daemon_process_id);
        let mut runtime = Some(profile);
        let mut state = LocalNodesState::default_for_config_dir(directory.path());
        let request = LocalNodeActionRequest {
            action: NodeAction::StopRuntime,
            node: None,
            network_id: None,
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            label: None,
        };

        // Act
        let result = runtime_stop(&mut state, &mut runtime, &request, None);

        // Assert
        if result.report.status != "stopped" {
            bail!("runtime stop did not complete: {}", result.report.detail);
        }
        if runtime
            .as_ref()
            .and_then(|profile| profile.daemon_process_id)
            .is_some()
        {
            bail!("runtime stop retained the daemon process id");
        }
        if !wait_until_stopped(child_process_id) {
            bail!("runtime stop left module host {child_process_id} running");
        }
        let _status = daemon.wait()?;
        Ok(())
    }

    #[cfg(unix)]
    fn wait_for_process_id(path: &Path) -> Result<u32> {
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            if let Ok(value) = fs::read_to_string(path)
                && let Ok(process_id) = value.trim().parse::<u32>()
            {
                return Ok(process_id);
            }
            thread::sleep(Duration::from_millis(10));
        }
        bail!("test module host did not publish its process id")
    }

    #[cfg(unix)]
    fn wait_until_stopped(process_id: u32) -> bool {
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            if !process_is_alive(process_id) {
                return true;
            }
            thread::sleep(Duration::from_millis(10));
        }
        false
    }

    #[cfg(unix)]
    #[test]
    fn messaging_initialize_recovers_context_after_lost_create_reply() -> Result<()> {
        // Arrange
        let (result, state, calls) = messaging_initialize_test_case("ready")?;

        // Act & Assert
        if result.report.status != "initialized" {
            bail!(
                "Messaging initialization did not recover: {}",
                result.report.detail
            );
        }
        let messaging = testnet_node(&state, NodeKind::Messaging)?;
        if !messaging.installed || messaging.lifecycle_state != NodeLifecycleState::Stopped {
            bail!("Messaging context was not persisted after recovery: {messaging:?}");
        }
        assert_single_messaging_create(&calls)?;
        if !result
            .report
            .detail
            .contains("createNode response was lost")
        {
            bail!(
                "Messaging recovery detail did not explain reply loss: {}",
                result.report.detail
            );
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn messaging_initialize_preserves_failure_when_context_is_absent() -> Result<()> {
        // Arrange
        let (result, state, calls) = messaging_initialize_test_case("absent")?;

        // Act & Assert
        if result.report.status != "failed" {
            bail!(
                "Messaging initialization unexpectedly recovered: {}",
                result.report.detail
            );
        }
        let messaging = testnet_node(&state, NodeKind::Messaging)?;
        if messaging.installed || messaging.lifecycle_state != NodeLifecycleState::NotInitialized {
            bail!("Messaging persisted an absent context: {messaging:?}");
        }
        assert_single_messaging_create(&calls)?;
        if !result.report.detail.contains("RPC_FAILED")
            || !result
                .report
                .detail
                .contains("Messaging context verification failed")
        {
            bail!(
                "Messaging failure lost original or recovery diagnostics: {}",
                result.report.detail
            );
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn messaging_initialize_restarts_crashed_runtime_without_replaying_create() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir()?;
        let cli = directory.path().join("logoscore");
        fs::write(
            &cli,
            r#"#!/bin/sh
config_dir=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --config-dir)
            config_dir="$2"
            shift 2
            ;;
        --persistence-path|--modules-dir)
            shift 2
            ;;
        *)
            break
            ;;
    esac
done
case "$1" in
    daemon)
        starts_path="$config_dir/daemon-starts"
        starts=0
        if [ -f "$starts_path" ]; then
            starts="$(cat "$starts_path")"
        fi
        starts=$((starts + 1))
        printf '%s' "$starts" > "$starts_path"
        printf '%s' "$$" > "$config_dir/daemon.pid"
        touch "$config_dir/daemon-ready"
        trap 'exit 0' TERM INT
        while :; do sleep 1; done
        ;;
    status)
        printf '%s\n' '{"daemon":{"status":"running"}}'
        ;;
    list-modules)
        printf '%s\n' '{"modules":[{"name":"delivery_module","status":"loaded"}]}'
        ;;
    module-info)
        printf '%s\n' '{"name":"delivery_module","methods":[{"isInvokable":true,"name":"createNode","signature":"createNode(QString)"},{"isInvokable":true,"name":"getNodeInfo","signature":"getNodeInfo(QString)"}]}'
        ;;
    call)
        case "$3" in
            createNode)
                printf '%s\n' createNode >> "$config_dir/calls"
                kill -TERM "$(cat "$config_dir/daemon.pid")"
                printf '%s\n' '{"code":"RPC_FAILED","message":"delivery create response lost","status":"error"}'
                exit 4
                ;;
            getNodeInfo)
                printf '%s\n' getNodeInfo >> "$config_dir/calls"
                if [ "$(cat "$config_dir/daemon-starts")" -ge 2 ]; then
                    printf '%s\n' '{"module":"delivery_module","method":"getNodeInfo","result":{"success":true,"value":"peer-after-restart"},"status":"ok"}'
                else
                    printf '%s\n' '{"code":"CONTEXT_UNAVAILABLE","message":"runtime restart required","status":"error"}'
                    exit 4
                fi
                ;;
        esac
        ;;
esac
"#,
        )?;
        let mut permissions = fs::metadata(&cli)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&cli, permissions)?;

        let mut profile = LogoscoreRuntimeProfile::create_or_restart(
            directory.path(),
            None,
            Some(
                cli.to_str()
                    .context("test LogosCore CLI path is not valid UTF-8")?,
            ),
            Some(
                directory
                    .path()
                    .to_str()
                    .context("test modules path is not valid UTF-8")?,
            ),
        )?;
        fs::create_dir_all(&profile.config_dir)?;
        let initial_process_id = spawn_detached(
            profile.daemon_command()?,
            "test Inspector-managed logoscore daemon",
        )?;
        profile.daemon_process_id = Some(initial_process_id);
        let _initial_cleanup = ProcessGroupGuard {
            process_id: initial_process_id,
        };
        let daemon_pid_path = Path::new(&profile.config_dir).join("daemon.pid");
        let daemon_start_deadline = Instant::now() + Duration::from_secs(1);
        while !daemon_pid_path.is_file() {
            if Instant::now() >= daemon_start_deadline {
                bail!("test Inspector-managed logoscore daemon did not start");
            }
            thread::sleep(Duration::from_millis(10));
        }

        let mut state = LocalNodesState::default_for_config_dir(directory.path());
        ensure_testnet_topology(&mut state)?;
        let storage = state
            .testnet
            .as_mut()
            .and_then(|record| {
                record
                    .nodes
                    .iter_mut()
                    .find(|node| node.kind == NodeKind::Storage)
            })
            .context("missing Storage node for runtime recovery test")?;
        storage.installed = true;
        storage.lifecycle_state = NodeLifecycleState::Running;
        storage.pending_lifecycle_action = Some(NodeAction::Start);
        let request = LocalNodeActionRequest {
            action: NodeAction::Initialize,
            node: Some(NodeKind::Messaging),
            network_id: None,
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            label: None,
        };
        let control = CommandControl::new(
            tokio_util::sync::CancellationToken::new(),
            Instant::now() + Duration::from_secs(10),
        );
        let mut runtime = Some(profile);

        let result = node_initialize(
            &mut state,
            &mut runtime,
            "default",
            &request,
            Some(&control),
        );

        let profile = runtime
            .as_ref()
            .context("Messaging recovery removed the managed runtime profile")?;
        let recovered_process_id = profile
            .daemon_process_id
            .context("Messaging recovery did not record a replacement daemon")?;
        let _replacement_cleanup = ProcessGroupGuard {
            process_id: recovered_process_id,
        };
        if result.report.status != "initialized" {
            bail!(
                "Messaging initialization did not recover after daemon crash: {}",
                result.report.detail
            );
        }
        anyhow::ensure!(
            recovered_process_id != initial_process_id,
            "Messaging recovery retained the crashed daemon process id"
        );
        anyhow::ensure!(
            fs::read_to_string(Path::new(&profile.config_dir).join("daemon-starts"))?.trim() == "2",
            "Messaging recovery did not start exactly one replacement daemon"
        );
        let calls = fs::read_to_string(Path::new(&profile.config_dir).join("calls"))?;
        assert_single_messaging_create(&calls.lines().map(ToOwned::to_owned).collect::<Vec<_>>())?;
        let messaging = testnet_node(&state, NodeKind::Messaging)?;
        anyhow::ensure!(
            messaging.installed && messaging.lifecycle_state == NodeLifecycleState::Stopped,
            "Messaging context was not persisted after restart recovery: {messaging:?}"
        );
        let storage = testnet_node(&state, NodeKind::Storage)?;
        anyhow::ensure!(
            !storage.installed
                && storage.lifecycle_state == NodeLifecycleState::NotInitialized
                && storage.pending_lifecycle_action.is_none(),
            "runtime recovery retained a stale Storage module context: {storage:?}"
        );
        anyhow::ensure!(
            result
                .report
                .detail
                .contains("restarted the Inspector-managed LogosCore runtime"),
            "Messaging recovery did not disclose runtime restart: {}",
            result.report.detail
        );
        Ok(())
    }

    #[cfg(unix)]
    fn messaging_initialize_test_case(
        recovery_mode: &str,
    ) -> Result<(LocalNodeActionResult, LocalNodesState, Vec<String>)> {
        let directory = tempfile::tempdir()?;
        let cli = directory.path().join("logoscore");
        fs::write(
            &cli,
            r#"#!/bin/sh
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
case "$1" in
    list-modules)
        printf '%s\n' '{"modules":[{"name":"delivery_module","status":"loaded"}]}'
        ;;
    module-info)
        printf '%s\n' '{"name":"delivery_module","methods":[{"isInvokable":true,"name":"createNode","signature":"createNode(QString)"},{"isInvokable":true,"name":"getNodeInfo","signature":"getNodeInfo(QString)"}]}'
        ;;
    call)
        case "$3" in
            createNode)
                printf '%s\n' createNode >> "$config_dir/calls"
                printf '%s\n' '{"code":"RPC_FAILED","message":"delivery create response lost","status":"error"}'
                exit 4
                ;;
            getNodeInfo)
                printf '%s\n' getNodeInfo >> "$config_dir/calls"
                if [ "$(cat "$config_dir/recovery-mode")" = ready ]; then
                    printf '%s\n' '{"module":"delivery_module","method":"getNodeInfo","result":{"success":true,"value":"peer-test"},"status":"ok"}'
                else
                    printf '%s\n' '{"code":"CONTEXT_MISSING","message":"no delivery context","status":"error"}'
                    exit 4
                fi
                ;;
        esac
        ;;
esac
"#,
        )?;
        let mut permissions = fs::metadata(&cli)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&cli, permissions)?;

        let mut runtime = LogoscoreRuntimeProfile::create_or_restart(
            directory.path(),
            None,
            Some(
                cli.to_str()
                    .context("test LogosCore CLI path is not valid UTF-8")?,
            ),
            Some(
                directory
                    .path()
                    .to_str()
                    .context("test modules path is not valid UTF-8")?,
            ),
        )?;
        fs::create_dir_all(&runtime.config_dir)?;
        fs::write(
            Path::new(&runtime.config_dir).join("recovery-mode"),
            recovery_mode,
        )?;
        runtime.daemon_process_id = Some(process::id());

        let mut state = LocalNodesState::default_for_config_dir(directory.path());
        ensure_testnet_topology(&mut state)?;
        let request = LocalNodeActionRequest {
            action: NodeAction::Initialize,
            node: Some(NodeKind::Messaging),
            network_id: None,
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            label: None,
        };

        let deadline = Instant::now()
            .checked_add(Duration::from_secs(15))
            .context("Messaging recovery test deadline overflow")?;
        let control = CommandControl::new(tokio_util::sync::CancellationToken::new(), deadline);
        let mut runtime = Some(runtime);
        let result = node_initialize(
            &mut state,
            &mut runtime,
            "default",
            &request,
            Some(&control),
        );
        let config_dir = runtime
            .as_ref()
            .context("Messaging test runtime disappeared")?
            .config_dir
            .clone();
        let calls_path = Path::new(&config_dir).join("calls");
        let calls = calls_path
            .is_file()
            .then(|| fs::read_to_string(calls_path))
            .transpose()?
            .unwrap_or_default()
            .lines()
            .map(ToOwned::to_owned)
            .collect();
        Ok((result, state, calls))
    }

    #[cfg(unix)]
    fn testnet_node(state: &LocalNodesState, kind: NodeKind) -> Result<&LocalNodeConfigRecord> {
        state
            .testnet
            .as_ref()
            .and_then(|record| record.nodes.iter().find(|node| node.kind == kind))
            .with_context(|| format!("missing {kind:?} Testnet node"))
    }

    #[cfg(unix)]
    fn assert_single_messaging_create(calls: &[String]) -> Result<()> {
        let creates = calls
            .iter()
            .filter(|call| call.as_str() == "createNode")
            .count();
        if creates != 1 || !calls.iter().any(|call| call == "getNodeInfo") {
            bail!("unexpected Messaging recovery calls: {calls:?}");
        }
        Ok(())
    }
}
