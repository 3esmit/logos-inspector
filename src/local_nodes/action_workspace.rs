use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};

use crate::support::time::now_millis;

use super::commands::{command_spec_for, execute_command_spec, operation_detail_from_value};
use super::model::{
    LocalDevnetRecord, LocalNodeActionRequest, LocalNodeConfigRecord, LocalNodeOperationReport,
    LocalNodeTools, LocalNodesState, NodeAction, NodeKind, ToolStatus,
};
use super::paths::{path_is_inside, remove_dir_inside};
use super::process::{find_command, process_is_alive, stop_process};
use super::workflow::node_set_for_profile;

const MANIFEST_FILE: &str = "local-network.json";
const DEFAULT_DEPLOYMENT: &str = "local";

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct LocalNodeActionWorkspace;

impl LocalNodeActionWorkspace {
    pub(super) fn system() -> Self {
        Self
    }

    pub(super) fn apply(
        self,
        state: &mut LocalNodesState,
        normalized_profile: &str,
        request: &LocalNodeActionRequest,
    ) -> LocalNodeOperationReport {
        dispatch_action(state, normalized_profile, request)
    }
}

fn dispatch_action(
    state: &mut LocalNodesState,
    normalized_profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    match request.action {
        NodeAction::NewNetwork => new_network(state, request),
        NodeAction::LoadNetwork => load_network(state, request),
        NodeAction::DeleteNetwork => delete_network(state, request),
        NodeAction::ResetNetwork => reset_network(state, request),
        NodeAction::Install => node_install(state, normalized_profile, request),
        NodeAction::Uninstall => node_uninstall(state, normalized_profile, request),
        NodeAction::Start => node_start(state, normalized_profile, request),
        NodeAction::Stop => node_stop(state, normalized_profile, request),
        NodeAction::Purge => node_purge(state, normalized_profile, request),
    }
}

fn new_network(
    state: &mut LocalNodesState,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    operation_result(request, None, || {
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
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    operation_result(request, None, || {
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
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    operation_result(request, None, || {
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
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    operation_result(request, None, || {
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

fn node_install(
    state: &mut LocalNodesState,
    profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        if kind == NodeKind::Sequencer {
            let Some(binary) = find_command("sequencer_service") else {
                return Ok(OperationOutcome {
                    status: "needs_configuration".to_owned(),
                    detail: "sequencer_service not found".to_owned(),
                    command: None,
                });
            };
            if let Some(record) = state.active_devnet_mut()
                && let Some(config) = node_config_mut(record, kind)
            {
                config.package_path = Some(binary);
                config.installed = true;
                record.updated_at = now_millis();
                write_devnet_manifest(record)?;
            }
            return Ok(OperationOutcome {
                status: "installed".to_owned(),
                detail: "sequencer_service registered".to_owned(),
                command: None,
            });
        }
        if !tool_statuses().logoscore.available {
            return Ok(OperationOutcome {
                status: "needs_configuration".to_owned(),
                detail: "logoscore not found".to_owned(),
                command: None,
            });
        }
        if let Some(record) = state.active_devnet_mut()
            && let Some(config) = node_config_mut(record, kind)
        {
            if profile == "local"
                && let Some(spec) = command_spec_for(
                    kind,
                    NodeAction::Install,
                    &config.config_path,
                    DEFAULT_DEPLOYMENT,
                )
            {
                match execute_command_spec(&spec) {
                    Ok(value) => {
                        config.installed = true;
                        config.package_path = Some(spec.program.clone());
                        record.updated_at = now_millis();
                        write_devnet_manifest(record)?;
                        return Ok(OperationOutcome {
                            status: "installed".to_owned(),
                            detail: operation_detail_from_value(&value),
                            command: Some(spec.display),
                        });
                    }
                    Err(error) => {
                        return Ok(OperationOutcome {
                            status: "failed".to_owned(),
                            detail: error.to_string(),
                            command: Some(spec.display),
                        });
                    }
                }
            }
            config.installed = true;
            config.package_path = Some("logoscore".to_owned());
            record.updated_at = now_millis();
            write_devnet_manifest(record)?;
        }
        Ok(OperationOutcome {
            status: "installed".to_owned(),
            detail: "logoscore module available".to_owned(),
            command: None,
        })
    })
}

fn node_uninstall(
    state: &mut LocalNodesState,
    _profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let mut command = None;
        let mut detail = "node registration removed".to_owned();
        if let Some(record) = state.active_devnet_mut()
            && let Some(config) = node_config_mut(record, kind)
        {
            stop_owned_process(config);
            if let Some(spec) = command_spec_for(
                kind,
                NodeAction::Uninstall,
                &config.config_path,
                DEFAULT_DEPLOYMENT,
            ) {
                command = Some(spec.display.clone());
                if let Err(error) = execute_command_spec(&spec) {
                    detail = error.to_string();
                }
            }
            config.installed = false;
            config.package_path = None;
            config.module_path = None;
            config.process_id = None;
            record.updated_at = now_millis();
            write_devnet_manifest(record)?;
        }
        Ok(OperationOutcome {
            status: if detail == "node registration removed" {
                "uninstalled"
            } else {
                "failed"
            }
            .to_owned(),
            detail,
            command,
        })
    })
}

fn node_start(
    state: &mut LocalNodesState,
    profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let Some(record) = state.active_devnet_mut() else {
            if profile == "local" {
                bail!("active devnet is required");
            }
            return start_external_node(kind);
        };
        let Some(config) = node_config_mut(record, kind) else {
            bail!("{} config is not available", kind.label());
        };
        fs::create_dir_all(&config.data_dir)
            .with_context(|| format!("failed to create {}", config.data_dir))?;
        let spec = command_spec_for(
            kind,
            NodeAction::Start,
            &config.config_path,
            DEFAULT_DEPLOYMENT,
        )
        .with_context(|| format!("{} start is not implemented", kind.label()))?;
        match execute_command_spec(&spec) {
            Ok(value) => {
                if kind == NodeKind::Sequencer {
                    config.process_id = value
                        .get("pid")
                        .and_then(Value::as_u64)
                        .and_then(|pid| u32::try_from(pid).ok());
                }
                config.installed = true;
                record.updated_at = now_millis();
                write_devnet_manifest(record)?;
                Ok(OperationOutcome {
                    status: "started".to_owned(),
                    detail: operation_detail_from_value(&value),
                    command: Some(spec.display),
                })
            }
            Err(error) => Ok(OperationOutcome {
                status: "failed".to_owned(),
                detail: error.to_string(),
                command: Some(spec.display),
            }),
        }
    })
}

fn start_external_node(kind: NodeKind) -> Result<OperationOutcome> {
    if !tool_statuses().logoscore.available {
        return Ok(OperationOutcome {
            status: "needs_configuration".to_owned(),
            detail: "logoscore not found".to_owned(),
            command: None,
        });
    }
    let config = "";
    let spec = command_spec_for(kind, NodeAction::Start, config, DEFAULT_DEPLOYMENT)
        .with_context(|| format!("{} start is not implemented", kind.label()))?;
    match execute_command_spec(&spec) {
        Ok(value) => Ok(OperationOutcome {
            status: "started".to_owned(),
            detail: operation_detail_from_value(&value),
            command: Some(spec.display),
        }),
        Err(error) => Ok(OperationOutcome {
            status: "failed".to_owned(),
            detail: error.to_string(),
            command: Some(spec.display),
        }),
    }
}

fn node_stop(
    state: &mut LocalNodesState,
    profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let Some(record) = state.active_devnet_mut() else {
            if profile == "local" {
                bail!("active devnet is required");
            }
            return stop_external_node(kind);
        };
        let Some(config) = node_config_mut(record, kind) else {
            bail!("{} config is not available", kind.label());
        };
        let mut command = None;
        if kind == NodeKind::Sequencer {
            stop_owned_process(config);
            record.updated_at = now_millis();
            write_devnet_manifest(record)?;
            return Ok(OperationOutcome {
                status: "stopped".to_owned(),
                detail: "stopped recorded sequencer process".to_owned(),
                command,
            });
        }
        if let Some(spec) = command_spec_for(
            kind,
            NodeAction::Stop,
            &config.config_path,
            DEFAULT_DEPLOYMENT,
        ) {
            command = Some(spec.display.clone());
            match execute_command_spec(&spec) {
                Ok(value) => {
                    config.process_id = None;
                    record.updated_at = now_millis();
                    write_devnet_manifest(record)?;
                    return Ok(OperationOutcome {
                        status: "stopped".to_owned(),
                        detail: operation_detail_from_value(&value),
                        command,
                    });
                }
                Err(error) => {
                    return Ok(OperationOutcome {
                        status: "failed".to_owned(),
                        detail: error.to_string(),
                        command,
                    });
                }
            }
        }
        Ok(OperationOutcome {
            status: "stopped".to_owned(),
            detail: "no stop adapter configured".to_owned(),
            command,
        })
    })
}

fn stop_external_node(kind: NodeKind) -> Result<OperationOutcome> {
    if !tool_statuses().logoscore.available {
        return Ok(OperationOutcome {
            status: "needs_configuration".to_owned(),
            detail: "logoscore not found".to_owned(),
            command: None,
        });
    }
    let spec = command_spec_for(kind, NodeAction::Stop, "", DEFAULT_DEPLOYMENT)
        .with_context(|| format!("{} stop is not implemented", kind.label()))?;
    match execute_command_spec(&spec) {
        Ok(value) => Ok(OperationOutcome {
            status: "stopped".to_owned(),
            detail: operation_detail_from_value(&value),
            command: Some(spec.display),
        }),
        Err(error) => Ok(OperationOutcome {
            status: "failed".to_owned(),
            detail: error.to_string(),
            command: Some(spec.display),
        }),
    }
}

fn node_purge(
    state: &mut LocalNodesState,
    _profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let workspace_root = PathBuf::from(&state.managed_workspace_root);
        let Some(record) = state.active_devnet_mut() else {
            bail!("active devnet is required");
        };
        let Some(config) = node_config_mut(record, kind) else {
            bail!("{} config is not available", kind.label());
        };
        stop_owned_process(config);
        remove_dir_inside(&workspace_root, Path::new(&config.data_dir))?;
        fs::create_dir_all(&config.data_dir)
            .with_context(|| format!("failed to recreate {}", config.data_dir))?;
        config.process_id = None;
        record.updated_at = now_millis();
        write_devnet_manifest(record)?;
        Ok(OperationOutcome {
            status: "purged".to_owned(),
            detail: format!("purged {} data directory", kind.label()),
            command: None,
        })
    })
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
) -> LocalNodeOperationReport {
    let timestamp = now_millis();
    match operation() {
        Ok(outcome) => LocalNodeOperationReport {
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
        Err(error) => LocalNodeOperationReport {
            id: format!("op-{timestamp}"),
            time: timestamp.to_string(),
            timestamp_millis: timestamp,
            action: request.action,
            node,
            network_id: request.network_id.clone(),
            status: "failed".to_owned(),
            detail: error.to_string(),
            command: None,
        },
    }
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
    let port = kind.default_port();
    LocalNodeConfigRecord {
        kind,
        config_path: workspace
            .join("configs")
            .join(format!("{}.json", kind.as_str()))
            .display()
            .to_string(),
        data_dir: workspace
            .join("data")
            .join(kind.as_str())
            .display()
            .to_string(),
        endpoint: kind.endpoint(port),
        port,
        package_path: None,
        module_path: None,
        process_id: None,
        installed: false,
    }
}

fn generate_devnet_files(record: &LocalDevnetRecord) -> Result<()> {
    for node in &record.nodes {
        fs::create_dir_all(&node.data_dir)
            .with_context(|| format!("failed to create {}", node.data_dir))?;
        let config_path = PathBuf::from(&node.config_path);
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let value = json!({
            "network_id": record.id,
            "node": node.kind.as_str(),
            "data_dir": node.data_dir,
            "endpoint": node.endpoint,
            "port": node.port,
        });
        let text = serde_json::to_string_pretty(&value)
            .context("failed to serialize local node config")?;
        fs::write(&node.config_path, text)
            .with_context(|| format!("failed to write {}", node.config_path))?;
    }
    Ok(())
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

fn tool_statuses() -> LocalNodeTools {
    LocalNodeTools {
        logoscore: tool_status("logoscore"),
        lgpm: tool_status("lgpm"),
    }
}

fn tool_status(command: &str) -> ToolStatus {
    ToolStatus {
        available: find_command(command).is_some(),
        command: command.to_owned(),
        path: find_command(command),
    }
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
