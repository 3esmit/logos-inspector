use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, bail};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::support::time::now_millis;

use super::{
    NodeKind, NodeLifecycleState,
    model::{LocalDevnetRecord, LocalNodeConfigRecord, LocalNodesState},
    paths::path_is_inside,
    process::process_group_has_live_members,
    runtime::LogoscoreRuntimeProfile,
};

const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
const MAX_CONFIG_BYTES_USIZE: usize = 1024 * 1024;
const JSON_FORMAT: &str = "json";
const VALIDATION_SCOPE: &str = "JSON syntax and Inspector-managed field checks";
const INDEXER_CONFIGURATION_OWNERSHIP: &str =
    "Channel Indexer configuration is owned by its Zone. Open Zone Sources for that Zone.";
const MESSAGING_IDENTITY_REQUIRED: &str = "Messaging has no persisted peer identity. Initialize Messaging to create one before editing this configuration.";
const UNADOPTED_LOCAL_SERVICE: &str = "The local LogosCore service configuration has not been adopted. Inspector will not read or edit its files.";

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LocalNodeConfigSnapshot {
    pub profile: String,
    pub topology_id: String,
    pub node: NodeKind,
    pub node_label: String,
    pub config_path: String,
    pub config_role: String,
    pub format: String,
    pub raw_text: String,
    pub revision: String,
    pub editable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
    pub validation_scope: String,
    pub common_fields: Vec<LocalNodeConfigField>,
    pub protected_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LocalNodeConfigField {
    pub path: String,
    pub label: String,
    pub section: String,
    pub kind: String,
    pub value: Value,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LocalNodeConfigValidation {
    pub valid: bool,
    pub error: String,
    pub common_fields: Vec<LocalNodeConfigField>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigUse {
    Initialization,
    Runtime,
}

#[derive(Debug, Clone)]
struct ConfigTarget {
    path: PathBuf,
    role: &'static str,
    use_kind: ConfigUse,
}

#[derive(Debug, Clone)]
pub(super) struct LocalNodeConfigSave {
    previous_state: LocalNodesState,
    config_path: PathBuf,
    previous_config: Vec<u8>,
    manifest_path: PathBuf,
    previous_manifest: Option<Vec<u8>>,
}

struct ConfigSaveRequest<'a> {
    runtime: Option<&'a LogoscoreRuntimeProfile>,
    profile: &'a str,
    kind: NodeKind,
    text: &'a str,
    expected_revision: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldKind {
    String,
    Port,
    Boolean,
    StringList,
    LocalPath,
    ChannelId,
    Endpoint,
}

impl FieldKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::String | Self::Endpoint | Self::ChannelId => "string",
            Self::Port => "port",
            Self::Boolean => "boolean",
            Self::StringList => "string_list",
            Self::LocalPath => "path",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct FieldSpec {
    path: &'static str,
    label: &'static str,
    section: &'static str,
    kind: FieldKind,
    required: bool,
}

const BEDROCK_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        path: "/initial_peers",
        label: "Bootstrap peers",
        section: "Connectivity",
        kind: FieldKind::StringList,
        required: true,
    },
    FieldSpec {
        path: "/net_port",
        label: "Network port",
        section: "Connectivity",
        kind: FieldKind::Port,
        required: true,
    },
    FieldSpec {
        path: "/blend_port",
        label: "Blend port",
        section: "Connectivity",
        kind: FieldKind::Port,
        required: true,
    },
    FieldSpec {
        path: "/http_addr",
        label: "HTTP API address",
        section: "API",
        kind: FieldKind::Endpoint,
        required: true,
    },
    FieldSpec {
        path: "/skip_ibd",
        label: "Skip initial block download",
        section: "Protocol",
        kind: FieldKind::Boolean,
        required: true,
    },
    FieldSpec {
        path: "/state_path",
        label: "State directory",
        section: "Local data",
        kind: FieldKind::LocalPath,
        required: true,
    },
    FieldSpec {
        path: "/storage_path",
        label: "Storage directory",
        section: "Local data",
        kind: FieldKind::LocalPath,
        required: true,
    },
    FieldSpec {
        path: "/logs_path",
        label: "Logs directory",
        section: "Logging",
        kind: FieldKind::LocalPath,
        required: true,
    },
    FieldSpec {
        path: "/log_filter",
        label: "Log filter",
        section: "Logging",
        kind: FieldKind::String,
        required: true,
    },
];

const STORAGE_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        path: "/data-dir",
        label: "Data directory",
        section: "Local data",
        kind: FieldKind::LocalPath,
        required: true,
    },
    FieldSpec {
        path: "/listen-ip",
        label: "Listen address",
        section: "Connectivity",
        kind: FieldKind::String,
        required: true,
    },
    FieldSpec {
        path: "/listen-port",
        label: "Listen port",
        section: "Connectivity",
        kind: FieldKind::Port,
        required: true,
    },
    FieldSpec {
        path: "/disc-port",
        label: "Discovery port",
        section: "Connectivity",
        kind: FieldKind::Port,
        required: true,
    },
    FieldSpec {
        path: "/nat",
        label: "NAT mode",
        section: "Connectivity",
        kind: FieldKind::String,
        required: true,
    },
    FieldSpec {
        path: "/network",
        label: "Network preset",
        section: "Protocol",
        kind: FieldKind::String,
        required: true,
    },
    FieldSpec {
        path: "/log-level",
        label: "Log level",
        section: "Logging",
        kind: FieldKind::String,
        required: true,
    },
];

const MESSAGING_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        path: "/mode",
        label: "Node mode",
        section: "Protocol",
        kind: FieldKind::String,
        required: true,
    },
    FieldSpec {
        path: "/preset",
        label: "Network preset",
        section: "Protocol",
        kind: FieldKind::String,
        required: true,
    },
    FieldSpec {
        path: "/tcpPort",
        label: "TCP port",
        section: "Connectivity",
        kind: FieldKind::Port,
        required: true,
    },
    FieldSpec {
        path: "/discv5UdpPort",
        label: "Discovery UDP port",
        section: "Connectivity",
        kind: FieldKind::Port,
        required: true,
    },
    FieldSpec {
        path: "/discv5Discovery",
        label: "Enable discovery",
        section: "Connectivity",
        kind: FieldKind::Boolean,
        required: true,
    },
    FieldSpec {
        path: "/nat",
        label: "NAT mode",
        section: "Connectivity",
        kind: FieldKind::String,
        required: true,
    },
    FieldSpec {
        path: "/rest",
        label: "Enable REST API",
        section: "API",
        kind: FieldKind::Boolean,
        required: true,
    },
    FieldSpec {
        path: "/restAddress",
        label: "REST address",
        section: "API",
        kind: FieldKind::String,
        required: true,
    },
    FieldSpec {
        path: "/restPort",
        label: "REST port",
        section: "API",
        kind: FieldKind::Port,
        required: true,
    },
    FieldSpec {
        path: "/logLevel",
        label: "Log level",
        section: "Logging",
        kind: FieldKind::String,
        required: true,
    },
    FieldSpec {
        path: "/logFormat",
        label: "Log format",
        section: "Logging",
        kind: FieldKind::String,
        required: true,
    },
];

const SEQUENCER_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        path: "/data_dir",
        label: "Data directory",
        section: "Local data",
        kind: FieldKind::LocalPath,
        required: true,
    },
    FieldSpec {
        path: "/endpoint",
        label: "RPC endpoint",
        section: "API",
        kind: FieldKind::Endpoint,
        required: true,
    },
    FieldSpec {
        path: "/port",
        label: "RPC port",
        section: "API",
        kind: FieldKind::Port,
        required: true,
    },
];

const INDEXER_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        path: "/channel_id",
        label: "Zone channel ID",
        section: "Protocol",
        kind: FieldKind::ChannelId,
        required: true,
    },
    FieldSpec {
        path: "/bedrock_config/addr",
        label: "Bedrock API URL",
        section: "API",
        kind: FieldKind::Endpoint,
        required: true,
    },
    FieldSpec {
        path: "/consensus_info_polling_interval",
        label: "Consensus polling interval",
        section: "Protocol",
        kind: FieldKind::String,
        required: true,
    },
];

pub(super) fn snapshot(
    state: &LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    profile: &str,
    kind: NodeKind,
) -> Result<LocalNodeConfigSnapshot> {
    require_configuration_editor_node(kind)?;
    let record = state.active_topology(normalized_profile(profile));
    if attached_service_blocks_configuration(runtime, state, record) {
        return Ok(unadopted_service_snapshot(profile, kind));
    }
    let record = record.context("active local node topology is required")?;
    let node = node_record(record, kind)?;
    let target = config_target(node)?;
    let bytes = read_config_bytes(&record.workspace, &target.path)?;
    let raw_value = parse_json(&bytes)?;
    let editor_value = redact_for_editor(kind, &raw_value)?;
    validate_editor_value(record, node, &editor_value)?;
    let blocked_reason = match edit_blocked_reason(node, kind, &target) {
        Some(reason) => Some(reason),
        None => messaging_identity_blocked_reason(kind, &raw_value)?,
    };
    let raw_text = serde_json::to_string_pretty(&editor_value)
        .context("failed to format node configuration for editing")?;
    Ok(LocalNodeConfigSnapshot {
        profile: normalized_profile(profile).to_owned(),
        topology_id: record.id.clone(),
        node: kind,
        node_label: node_label(kind).to_owned(),
        config_path: target.path.display().to_string(),
        config_role: target.role.to_owned(),
        format: JSON_FORMAT.to_owned(),
        raw_text,
        revision: revision_for(&bytes),
        editable: blocked_reason.is_none(),
        blocked_reason,
        validation_scope: VALIDATION_SCOPE.to_owned(),
        common_fields: project_fields(kind, &editor_value),
        protected_fields: protected_fields(kind),
    })
}

pub(super) fn validate(
    state: &LocalNodesState,
    profile: &str,
    kind: NodeKind,
    text: &str,
) -> Result<LocalNodeConfigValidation> {
    let result: Result<Vec<LocalNodeConfigField>> = (|| {
        require_configuration_editor_node(kind)?;
        let record = active_record(state, profile)?;
        let node = node_record(record, kind)?;
        let value = parse_editor_text(text)?;
        validate_editor_value(record, node, &value)?;
        Ok(project_fields(kind, &value))
    })();
    match result {
        Ok(common_fields) => Ok(LocalNodeConfigValidation {
            valid: true,
            error: String::new(),
            common_fields,
        }),
        Err(error) => Ok(LocalNodeConfigValidation {
            valid: false,
            error: error.to_string(),
            common_fields: Vec::new(),
        }),
    }
}

pub(super) fn save<F>(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    profile: &str,
    kind: NodeKind,
    text: &str,
    expected_revision: &str,
    persist_state: F,
) -> Result<()>
where
    F: FnMut(&LocalNodesState) -> Result<()>,
{
    save_with_writer(
        state,
        ConfigSaveRequest {
            runtime,
            profile,
            kind,
            text,
            expected_revision,
        },
        persist_state,
        write_editor_value,
    )
}

fn unadopted_service_snapshot(profile: &str, kind: NodeKind) -> LocalNodeConfigSnapshot {
    LocalNodeConfigSnapshot {
        profile: normalized_profile(profile).to_owned(),
        topology_id: String::new(),
        node: kind,
        node_label: node_label(kind).to_owned(),
        config_path: String::new(),
        config_role: "Unadopted local service".to_owned(),
        format: JSON_FORMAT.to_owned(),
        raw_text: String::new(),
        revision: String::new(),
        editable: false,
        blocked_reason: Some(UNADOPTED_LOCAL_SERVICE.to_owned()),
        validation_scope: VALIDATION_SCOPE.to_owned(),
        common_fields: Vec::new(),
        protected_fields: Vec::new(),
    }
}

fn save_with_writer<F, W>(
    state: &mut LocalNodesState,
    request: ConfigSaveRequest<'_>,
    mut persist_state: F,
    mut write_config: W,
) -> Result<()>
where
    F: FnMut(&LocalNodesState) -> Result<()>,
    W: FnMut(NodeKind, &str, &Path, &Value, &Value) -> Result<()>,
{
    require_configuration_editor_node(request.kind)?;
    let attached_service_blocks_configuration = {
        let record = state.active_topology(normalized_profile(request.profile));
        attached_service_blocks_configuration(request.runtime, state, record)
    };
    if attached_service_blocks_configuration {
        bail!(UNADOPTED_LOCAL_SERVICE);
    }
    let previous_state = state.clone();
    let (workspace, node, target, manifest_path) = {
        let record = active_record(state, request.profile)?;
        let node = node_record(record, request.kind)?.clone();
        let target = config_target(&node)?;
        (
            record.workspace.clone(),
            node,
            target,
            PathBuf::from(&record.manifest_path),
        )
    };
    if let Some(reason) = edit_blocked_reason(&node, request.kind, &target) {
        bail!("{reason}");
    }
    validate_managed_output_path(&workspace, &manifest_path, "managed topology manifest")?;
    let old_bytes = read_config_bytes(&workspace, &target.path)?;
    if revision_for(&old_bytes) != request.expected_revision {
        bail!("configuration changed on disk; reload it before saving");
    }
    let old_value = parse_json(&old_bytes)?;
    if let Some(reason) = messaging_identity_blocked_reason(request.kind, &old_value)? {
        bail!(reason);
    }
    let editor_value = parse_editor_text(request.text)?;
    validate_editor_value(active_record(state, request.profile)?, &node, &editor_value)?;
    let previous_manifest =
        read_optional_regular_file(&manifest_path, "managed topology manifest")?;
    let transaction = LocalNodeConfigSave {
        previous_state,
        config_path: target.path,
        previous_config: old_bytes,
        manifest_path,
        previous_manifest,
    };
    if let Err(error) = write_config(
        request.kind,
        &workspace,
        &transaction.config_path,
        &editor_value,
        &old_value,
    ) {
        return rollback_failed_save(state, transaction, false, &mut persist_state, error);
    }
    if let Err(error) = (|| {
        let record = active_record_mut(state, request.profile)?;
        project_record(record, request.kind, &editor_value)?;
        write_config_manifest(record)
    })() {
        return rollback_failed_save(state, transaction, false, &mut persist_state, error);
    }
    if let Err(error) = persist_state(state) {
        return rollback_failed_save(state, transaction, true, &mut persist_state, error);
    }
    Ok(())
}

fn rollback_failed_save<F>(
    state: &mut LocalNodesState,
    transaction: LocalNodeConfigSave,
    restore_persisted_state: bool,
    persist_state: &mut F,
    error: anyhow::Error,
) -> Result<()>
where
    F: FnMut(&LocalNodesState) -> Result<()>,
{
    match transaction.rollback(state, restore_persisted_state, persist_state) {
        Ok(()) => {
            let context = if restore_persisted_state {
                "failed to persist local node state; configuration changes were rolled back"
            } else {
                "configuration changes were rolled back"
            };
            Err(error.context(context))
        }
        Err(rollback_error) => Err(error.context(format!(
            "configuration rollback also failed: {rollback_error:#}"
        ))),
    }
}

impl LocalNodeConfigSave {
    fn rollback<F>(
        self,
        state: &mut LocalNodesState,
        restore_persisted_state: bool,
        persist_state: &mut F,
    ) -> Result<()>
    where
        F: FnMut(&LocalNodesState) -> Result<()>,
    {
        *state = self.previous_state;
        let mut failures = Vec::new();
        if let Err(error) = replace_config_bytes(&self.config_path, &self.previous_config) {
            failures.push(format!("configuration file: {error:#}"));
        }
        if let Err(error) = restore_optional_regular_file(
            &self.manifest_path,
            self.previous_manifest.as_deref(),
            "managed topology manifest",
        ) {
            failures.push(format!("topology manifest: {error:#}"));
        }
        if restore_persisted_state && let Err(error) = persist_state(state) {
            failures.push(format!("local node state: {error:#}"));
        }
        if failures.is_empty() {
            Ok(())
        } else {
            bail!(failures.join("; "));
        }
    }
}

fn write_config_manifest(record: &LocalDevnetRecord) -> Result<()> {
    let path = PathBuf::from(&record.manifest_path);
    validate_managed_output_path(&record.workspace, &path, "managed topology manifest")?;
    let bytes = serde_json::to_vec_pretty(record)
        .context("failed to serialize managed topology manifest")?;
    replace_config_bytes(&path, &bytes).with_context(|| {
        format!(
            "failed to write managed topology manifest {}",
            path.display()
        )
    })
}

fn validate_managed_output_path(workspace: &str, path: &Path, label: &str) -> Result<()> {
    let workspace_path = Path::new(workspace);
    if !path_is_inside(workspace_path, path) {
        bail!("{label} is outside its topology workspace");
    }
    let canonical_workspace = fs::canonicalize(workspace_path)
        .with_context(|| format!("failed to resolve topology workspace {workspace}"))?;
    let parent = path
        .parent()
        .context("managed output file has no parent directory")?;
    let canonical_parent = fs::canonicalize(parent)
        .with_context(|| format!("failed to resolve {}", parent.display()))?;
    if canonical_parent != canonical_workspace
        && !canonical_parent.starts_with(&canonical_workspace)
    {
        bail!("{label} directory escapes its topology workspace");
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("{label} must not be a symbolic link")
        }
        Ok(metadata) if !metadata.is_file() => bail!("{label} must be a regular file"),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

fn read_optional_regular_file(path: &Path, label: &str) -> Result<Option<Vec<u8>>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", path.display()));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!("{label} must be a regular file");
    }
    if metadata.len() > MAX_CONFIG_BYTES {
        bail!("{label} exceeds the 1 MiB rollback limit");
    }
    fs::read(path)
        .with_context(|| format!("failed to read {}", path.display()))
        .map(Some)
}

fn restore_optional_regular_file(path: &Path, bytes: Option<&[u8]>, label: &str) -> Result<()> {
    if let Some(bytes) = bytes {
        return replace_config_bytes(path, bytes)
            .with_context(|| format!("failed to restore {label} {}", path.display()));
    }
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to inspect {}", path.display())),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            bail!("{label} must be a regular file")
        }
        Ok(_) => {
            fs::remove_file(path)
                .with_context(|| format!("failed to remove {label} {}", path.display()))?;
            let parent = path
                .parent()
                .context("managed output file has no parent directory")?;
            sync_directory(parent)
        }
    }
}

fn normalized_profile(profile: &str) -> &'static str {
    match profile.trim().to_ascii_lowercase().as_str() {
        "local" | "localnet" | "devnet" => "local",
        _ => "default",
    }
}

fn require_configuration_editor_node(kind: NodeKind) -> Result<()> {
    if kind == NodeKind::Indexer {
        bail!(INDEXER_CONFIGURATION_OWNERSHIP);
    }
    Ok(())
}

fn active_record<'a>(state: &'a LocalNodesState, profile: &str) -> Result<&'a LocalDevnetRecord> {
    state
        .active_topology(normalized_profile(profile))
        .context("active local node topology is required")
}

fn active_record_mut<'a>(
    state: &'a mut LocalNodesState,
    profile: &str,
) -> Result<&'a mut LocalDevnetRecord> {
    state
        .active_topology_mut(normalized_profile(profile))
        .context("active local node topology is required")
}

fn node_record(record: &LocalDevnetRecord, kind: NodeKind) -> Result<&LocalNodeConfigRecord> {
    record
        .nodes
        .iter()
        .find(|node| node.kind == kind)
        .with_context(|| format!("{} configuration is unavailable", node_label(kind)))
}

fn config_target(node: &LocalNodeConfigRecord) -> Result<ConfigTarget> {
    if node.kind == NodeKind::Bedrock {
        return Ok(ConfigTarget {
            path: PathBuf::from(
                node.initialization_config_path
                    .as_deref()
                    .context("Bedrock initialization configuration is unavailable")?,
            ),
            role: "Initialization source",
            use_kind: ConfigUse::Initialization,
        });
    }
    Ok(ConfigTarget {
        path: PathBuf::from(&node.config_path),
        role: "Runtime configuration",
        use_kind: ConfigUse::Runtime,
    })
}

fn edit_blocked_reason(
    node: &LocalNodeConfigRecord,
    kind: NodeKind,
    target: &ConfigTarget,
) -> Option<String> {
    if node.lifecycle_state.is_pending()
        || matches!(
            node.lifecycle_state,
            NodeLifecycleState::Running | NodeLifecycleState::Unknown
        )
        || node.process_id.is_some_and(process_group_has_live_members)
    {
        return Some(
            "Stop this node and wait for its lifecycle state to settle before editing configuration."
                .to_owned(),
        );
    }
    if kind == NodeKind::Bedrock && Path::new(&node.config_path).is_file() {
        return Some(
            "Bedrock has generated a runtime YAML configuration. Its protected runtime material cannot be safely regenerated here; create a fresh topology before changing its initialization source."
                .to_owned(),
        );
    }
    if target.use_kind == ConfigUse::Initialization
        && node.lifecycle_state != NodeLifecycleState::NotInitialized
    {
        return Some(
            "This configuration is consumed during initialization. Clear the existing module context before editing it."
                .to_owned(),
        );
    }
    if kind == NodeKind::Storage && node.lifecycle_state != NodeLifecycleState::NotInitialized {
        return Some(
            "Storage reads this configuration during initialization. Uninstall the stopped Storage context before editing it."
                .to_owned(),
        );
    }
    None
}

fn attached_service_blocks_configuration(
    runtime: Option<&LogoscoreRuntimeProfile>,
    state: &LocalNodesState,
    record: Option<&LocalDevnetRecord>,
) -> bool {
    runtime.is_some_and(LogoscoreRuntimeProfile::is_attached)
        && record.is_none_or(|record| !topology_workspace_is_managed(state, record))
}

fn topology_workspace_is_managed(state: &LocalNodesState, record: &LocalDevnetRecord) -> bool {
    let managed_root = Path::new(&state.managed_workspace_root);
    let workspace = Path::new(&record.workspace);
    if !path_is_inside(managed_root, workspace) {
        return false;
    }
    let Ok(canonical_managed_root) = fs::canonicalize(managed_root) else {
        return false;
    };
    let Ok(canonical_workspace) = fs::canonicalize(workspace) else {
        return false;
    };
    canonical_workspace != canonical_managed_root
        && canonical_workspace.starts_with(&canonical_managed_root)
}

fn redact_for_editor(kind: NodeKind, value: &Value) -> Result<Value> {
    if kind == NodeKind::Messaging {
        return super::messaging_identity::redact_config_for_editor(value);
    }
    Ok(value.clone())
}

fn messaging_identity_blocked_reason(kind: NodeKind, value: &Value) -> Result<Option<String>> {
    if kind != NodeKind::Messaging {
        return Ok(None);
    }
    if super::messaging_identity::has_persisted_identity(value)? {
        return Ok(None);
    }
    Ok(Some(MESSAGING_IDENTITY_REQUIRED.to_owned()))
}

fn protected_fields(kind: NodeKind) -> Vec<String> {
    match kind {
        NodeKind::Messaging => vec!["Messaging peer identity".to_owned()],
        NodeKind::Bedrock => vec!["Generated Bedrock runtime keys".to_owned()],
        NodeKind::Sequencer | NodeKind::Indexer | NodeKind::Storage => Vec::new(),
    }
}

fn fields_for(kind: NodeKind) -> &'static [FieldSpec] {
    match kind {
        NodeKind::Bedrock => BEDROCK_FIELDS,
        NodeKind::Storage => STORAGE_FIELDS,
        NodeKind::Messaging => MESSAGING_FIELDS,
        NodeKind::Sequencer => SEQUENCER_FIELDS,
        NodeKind::Indexer => INDEXER_FIELDS,
    }
}

fn project_fields(kind: NodeKind, value: &Value) -> Vec<LocalNodeConfigField> {
    fields_for(kind)
        .iter()
        .filter_map(|field| {
            value
                .pointer(field.path)
                .map(|field_value| LocalNodeConfigField {
                    path: field.path.to_owned(),
                    label: field.label.to_owned(),
                    section: field.section.to_owned(),
                    kind: field.kind.as_str().to_owned(),
                    value: field_value.clone(),
                    required: field.required,
                })
        })
        .collect()
}

fn validate_editor_value(
    record: &LocalDevnetRecord,
    node: &LocalNodeConfigRecord,
    value: &Value,
) -> Result<()> {
    let object = value
        .as_object()
        .context("node configuration must be a JSON object")?;
    if node.kind == NodeKind::Messaging && object.contains_key("nodekey") {
        bail!("Messaging peer identity is protected and cannot be edited here");
    }
    for field in fields_for(node.kind) {
        let field_value = value.pointer(field.path);
        if field.required && field_value.is_none() {
            bail!("{} is required", field.label);
        }
        if let Some(field_value) = field_value {
            validate_field(record, field, field_value)?;
        }
    }
    validate_protected_values(record, node, value)?;
    Ok(())
}

fn validate_field(record: &LocalDevnetRecord, field: &FieldSpec, value: &Value) -> Result<()> {
    match field.kind {
        FieldKind::String => require_nonempty_string(value, field.label).map(|_| ()),
        FieldKind::Port => {
            let port = value
                .as_u64()
                .with_context(|| format!("{} must be a whole port number", field.label))?;
            if !(1..=u64::from(u16::MAX)).contains(&port) {
                bail!("{} must be between 1 and {}", field.label, u16::MAX);
            }
            Ok(())
        }
        FieldKind::Boolean => value
            .as_bool()
            .with_context(|| format!("{} must be true or false", field.label))
            .map(|_| ()),
        FieldKind::StringList => {
            let values = value
                .as_array()
                .with_context(|| format!("{} must be a JSON array", field.label))?;
            for entry in values {
                require_nonempty_string(entry, field.label)?;
            }
            Ok(())
        }
        FieldKind::LocalPath => {
            let path = require_nonempty_string(value, field.label)?;
            validate_workspace_path(&record.workspace, path, field.label)
        }
        FieldKind::ChannelId => {
            let channel_id = require_nonempty_string(value, field.label)?;
            if channel_id.len() != 64 || !channel_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                bail!(
                    "{} must be a 64-character hexadecimal channel ID",
                    field.label
                );
            }
            Ok(())
        }
        FieldKind::Endpoint => {
            validate_endpoint(require_nonempty_string(value, field.label)?, field.label)
        }
    }
}

fn validate_protected_values(
    record: &LocalDevnetRecord,
    node: &LocalNodeConfigRecord,
    value: &Value,
) -> Result<()> {
    match node.kind {
        NodeKind::Bedrock => {
            let expected = node.config_path.as_str();
            let output = value
                .pointer("/output")
                .and_then(Value::as_str)
                .context("Bedrock generated runtime output is required")?;
            if output != expected {
                bail!("Bedrock generated runtime output must remain {expected}");
            }
        }
        NodeKind::Sequencer => {
            let network_id = value
                .pointer("/network_id")
                .and_then(Value::as_str)
                .context("Sequencer network ID is required")?;
            if network_id != record.id {
                bail!("Sequencer network ID must remain {}", record.id);
            }
            let node_name = value
                .pointer("/node")
                .and_then(Value::as_str)
                .context("Sequencer node role is required")?;
            if node_name != "sequencer" {
                bail!("Sequencer node role must remain sequencer");
            }
        }
        NodeKind::Indexer | NodeKind::Storage | NodeKind::Messaging => {}
    }
    Ok(())
}

fn validate_workspace_path(workspace: &str, value: &str, label: &str) -> Result<()> {
    let path = Path::new(value);
    if !path.is_absolute() || !path_is_inside(Path::new(workspace), path) {
        bail!("{label} must stay inside the active topology workspace");
    }
    let canonical_workspace = fs::canonicalize(workspace)
        .with_context(|| format!("failed to resolve topology workspace {workspace}"))?;
    let mut existing = path;
    while !existing.exists() {
        existing = existing
            .parent()
            .context("configured local path has no existing parent")?;
    }
    let canonical_existing =
        fs::canonicalize(existing).with_context(|| format!("failed to resolve {label}"))?;
    if canonical_existing != canonical_workspace
        && !canonical_existing.starts_with(&canonical_workspace)
    {
        bail!("{label} resolves outside the active topology workspace");
    }
    Ok(())
}

fn validate_endpoint(value: &str, label: &str) -> Result<()> {
    if value.contains("://") {
        let parsed =
            url::Url::parse(value).with_context(|| format!("{label} is not a valid URL"))?;
        if parsed.host_str().is_none() {
            bail!("{label} must include a host");
        }
        return Ok(());
    }
    if value.trim().is_empty() || value.chars().any(char::is_whitespace) {
        bail!("{label} must be a host and port or a URL");
    }
    Ok(())
}

fn require_nonempty_string<'a>(value: &'a Value, label: &str) -> Result<&'a str> {
    value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .with_context(|| format!("{label} must be a non-empty string"))
}

fn parse_json(bytes: &[u8]) -> Result<Value> {
    serde_json::from_slice(bytes).context("node configuration is not valid JSON")
}

fn parse_editor_text(text: &str) -> Result<Value> {
    if text.len() > MAX_CONFIG_BYTES_USIZE {
        bail!("node configuration exceeds the 1 MiB editor limit");
    }
    serde_json::from_str(text).context("node configuration is not valid JSON")
}

fn read_config_bytes(workspace: &str, path: &Path) -> Result<Vec<u8>> {
    validate_config_file_path(workspace, path)?;
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    if bytes.len() > MAX_CONFIG_BYTES_USIZE {
        bail!("node configuration exceeds the 1 MiB editor limit");
    }
    Ok(bytes)
}

fn validate_config_file_path(workspace: &str, path: &Path) -> Result<()> {
    let workspace_path = Path::new(workspace);
    if !path_is_inside(workspace_path, path) {
        bail!("managed node configuration is outside its topology workspace");
    }
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!("managed node configuration must be a regular file");
    }
    if metadata.len() > MAX_CONFIG_BYTES {
        bail!("node configuration exceeds the 1 MiB editor limit");
    }
    let canonical_workspace = fs::canonicalize(workspace_path)
        .with_context(|| format!("failed to resolve topology workspace {workspace}"))?;
    let parent = path
        .parent()
        .context("managed node configuration has no parent directory")?;
    let canonical_parent = fs::canonicalize(parent)
        .with_context(|| format!("failed to resolve {}", parent.display()))?;
    if canonical_parent != canonical_workspace
        && !canonical_parent.starts_with(&canonical_workspace)
    {
        bail!("managed node configuration directory escapes its topology workspace");
    }
    Ok(())
}

fn write_editor_value(
    kind: NodeKind,
    workspace: &str,
    path: &Path,
    editor_value: &Value,
    old_value: &Value,
) -> Result<()> {
    if kind == NodeKind::Messaging {
        return super::messaging_identity::write_editor_config(
            Path::new(workspace),
            path,
            editor_value.clone(),
            old_value,
        );
    }
    let bytes = serde_json::to_vec_pretty(editor_value)
        .context("failed to serialize node configuration")?;
    replace_config_bytes(path, &bytes)
}

fn replace_config_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .context("managed node configuration has no parent directory")?;
    let mut staged = tempfile::Builder::new()
        .prefix(".node-config-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .context("failed to stage node configuration")?;
    staged
        .write_all(bytes)
        .context("failed to write staged node configuration")?;
    staged
        .as_file_mut()
        .flush()
        .context("failed to flush staged node configuration")?;
    staged
        .as_file()
        .sync_all()
        .context("failed to sync staged node configuration")?;
    staged
        .persist(path)
        .map_err(|error| error.error)
        .context("failed to atomically replace node configuration")?;
    sync_directory(parent)
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<()> {
    fs::File::open(path)
        .context("failed to open node configuration directory")?
        .sync_all()
        .context("failed to sync node configuration directory")
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<()> {
    Ok(())
}

fn project_record(record: &mut LocalDevnetRecord, kind: NodeKind, value: &Value) -> Result<()> {
    let node = record
        .nodes
        .iter_mut()
        .find(|node| node.kind == kind)
        .context("managed node configuration is unavailable")?;
    match kind {
        NodeKind::Bedrock => {
            let address = value
                .pointer("/http_addr")
                .and_then(Value::as_str)
                .context("Bedrock HTTP API address is required")?;
            node.endpoint = Some(endpoint_url(address));
        }
        NodeKind::Storage => {
            node.data_dir = value
                .pointer("/data-dir")
                .and_then(Value::as_str)
                .context("Storage data directory is required")?
                .to_owned();
        }
        NodeKind::Messaging => {
            let address = value
                .pointer("/restAddress")
                .and_then(Value::as_str)
                .context("Messaging REST address is required")?;
            let port = value
                .pointer("/restPort")
                .and_then(Value::as_u64)
                .and_then(|port| u16::try_from(port).ok())
                .context("Messaging REST port is required")?;
            node.port = Some(port);
            node.endpoint = Some(format!("http://{address}:{port}/"));
        }
        NodeKind::Sequencer => {
            node.data_dir = value
                .pointer("/data_dir")
                .and_then(Value::as_str)
                .context("Sequencer data directory is required")?
                .to_owned();
            node.endpoint = Some(
                value
                    .pointer("/endpoint")
                    .and_then(Value::as_str)
                    .context("Sequencer RPC endpoint is required")?
                    .to_owned(),
            );
            node.port = value
                .pointer("/port")
                .and_then(Value::as_u64)
                .and_then(|port| u16::try_from(port).ok());
        }
        NodeKind::Indexer => {}
    }
    record.updated_at = now_millis();
    Ok(())
}

fn endpoint_url(address: &str) -> String {
    if address.contains("://") {
        address.to_owned()
    } else {
        format!("http://{address}/")
    }
}

fn revision_for(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn node_label(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Bedrock => "Bedrock",
        NodeKind::Sequencer => "Local Sequencer",
        NodeKind::Indexer => "Indexer",
        NodeKind::Storage => "Storage",
        NodeKind::Messaging => "Messaging",
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic_in_result_fn
)]
mod tests {
    use std::fs;

    use anyhow::Result;
    use serde_json::json;

    use super::*;
    use crate::local_nodes::model::{LocalNodeDeployment, LocalNodesState};
    use crate::local_nodes::runtime::{
        LogoscoreRuntimeOwnership, LogoscoreRuntimeProfile, LogoscoreTimeoutProfile,
    };

    fn record(kind: NodeKind, workspace: &Path, config_path: &Path) -> LocalDevnetRecord {
        LocalDevnetRecord {
            deployment: LocalNodeDeployment::LocalDevnet,
            id: "devnet".to_owned(),
            label: "Devnet".to_owned(),
            workspace: workspace.display().to_string(),
            manifest_path: workspace.join("local-network.json").display().to_string(),
            created_at: 0,
            updated_at: 0,
            nodes: vec![LocalNodeConfigRecord {
                kind,
                config_path: config_path.display().to_string(),
                initialization_config_path: (kind == NodeKind::Bedrock).then(|| {
                    workspace
                        .join("configs/bedrock.init.json")
                        .display()
                        .to_string()
                }),
                data_dir: workspace.join("data").display().to_string(),
                endpoint: Some("http://127.0.0.1:3040/".to_owned()),
                port: Some(3040),
                package_path: None,
                package_version: None,
                package_root_hash: None,
                indexer_state: None,
                indexer_head: None,
                indexer_error: None,
                module_path: None,
                process_id: None,
                installed: false,
                lifecycle_state: NodeLifecycleState::NotInitialized,
                pending_lifecycle_action: None,
            }],
        }
    }

    fn state_for(kind: NodeKind, config: Value) -> Result<(tempfile::TempDir, LocalNodesState)> {
        let directory = tempfile::tempdir()?;
        let workspace = directory.path().join("devnet");
        let config_path = workspace
            .join("configs")
            .join(format!("{}.json", kind.as_str()));
        fs::create_dir_all(config_path.parent().context("config parent")?)?;
        fs::create_dir_all(workspace.join("data"))?;
        fs::write(&config_path, serde_json::to_vec_pretty(&config)?)?;
        let record = record(kind, &workspace, &config_path);
        let state = LocalNodesState {
            version: 4,
            active_devnet: Some("devnet".to_owned()),
            module_context_topology_by_kind: Default::default(),
            testnet: None,
            managed_workspace_root: directory.path().display().to_string(),
            devnets: vec![record],
            operations: Vec::new(),
        };
        Ok((directory, state))
    }

    fn persist_success(_state: &LocalNodesState) -> Result<()> {
        Ok(())
    }

    fn attached_runtime() -> LogoscoreRuntimeProfile {
        LogoscoreRuntimeProfile {
            id: "local-attached".to_owned(),
            binary_path: "/bin/sh".to_owned(),
            config_dir: "/tmp/logoscore".to_owned(),
            modules_dir: None,
            persistence_path: None,
            ownership: LogoscoreRuntimeOwnership::LocalAttached,
            timeout_profile: LogoscoreTimeoutProfile::Probe,
            daemon_process_id: None,
            service_target: None,
        }
    }

    #[test]
    fn messaging_snapshot_hides_peer_identity_and_preserves_it_on_save() -> Result<()> {
        let (directory, mut state) = state_for(
            NodeKind::Messaging,
            json!({
                "mode": "Core",
                "preset": "logos.test",
                "tcpPort": 30303,
                "discv5UdpPort": 9000,
                "discv5Discovery": true,
                "nat": "any",
                "rest": true,
                "restAddress": "127.0.0.1",
                "restPort": 8645,
                "logLevel": "INFO",
                "logFormat": "TEXT",
                "nodekey": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            }),
        )?;
        let initial = snapshot(&state, None, "local", NodeKind::Messaging)?;
        assert!(!initial.raw_text.contains("nodekey"));
        let mut replacement: Value = serde_json::from_str(&initial.raw_text)?;
        replacement["logLevel"] = Value::String("DEBUG".to_owned());
        replacement["nodekey"] = Value::String("replacement-is-not-allowed".to_owned());
        let rejected = validate(
            &state,
            "local",
            NodeKind::Messaging,
            &serde_json::to_string(&replacement)?,
        )?;
        assert!(!rejected.valid);
        assert!(rejected.error.contains("peer identity is protected"));
        replacement
            .as_object_mut()
            .context("replacement configuration must be an object")?
            .remove("nodekey");
        save(
            &mut state,
            None,
            "local",
            NodeKind::Messaging,
            &serde_json::to_string(&replacement)?,
            &initial.revision,
            persist_success,
        )?;
        let path = directory.path().join("devnet/configs/messaging.json");
        let stored: Value = serde_json::from_slice(&fs::read(path)?)?;
        assert_eq!(stored["logLevel"], "DEBUG");
        assert_eq!(
            stored["nodekey"],
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
        Ok(())
    }

    #[test]
    fn messaging_without_identity_is_read_only_and_never_writes() -> Result<()> {
        let (directory, mut state) = state_for(
            NodeKind::Messaging,
            json!({
                "mode": "Core",
                "preset": "logos.test",
                "tcpPort": 30303,
                "discv5UdpPort": 9000,
                "discv5Discovery": true,
                "nat": "any",
                "rest": true,
                "restAddress": "127.0.0.1",
                "restPort": 8645,
                "logLevel": "INFO",
                "logFormat": "TEXT"
            }),
        )?;
        let path = directory.path().join("devnet/configs/messaging.json");
        let before = fs::read(&path)?;
        let snapshot = snapshot(&state, None, "local", NodeKind::Messaging)?;

        assert!(!snapshot.editable);
        assert_eq!(
            snapshot.blocked_reason.as_deref(),
            Some(MESSAGING_IDENTITY_REQUIRED)
        );
        let error = save(
            &mut state,
            None,
            "local",
            NodeKind::Messaging,
            &snapshot.raw_text,
            &snapshot.revision,
            persist_success,
        )
        .expect_err("Messaging config without an identity must not be saved");
        assert!(error.to_string().contains("no persisted peer identity"));
        assert_eq!(fs::read(path)?, before);
        Ok(())
    }

    #[test]
    fn indexer_configuration_is_explicitly_zone_owned() -> Result<()> {
        let (directory, state) = state_for(NodeKind::Indexer, json!({}))?;
        let _keep = directory;

        let error = snapshot(&state, None, "local", NodeKind::Indexer)
            .expect_err("global Indexer editor must stay unavailable");
        assert_eq!(error.to_string(), INDEXER_CONFIGURATION_OWNERSHIP);
        Ok(())
    }

    #[test]
    fn stale_revision_and_invalid_json_leave_config_unchanged() -> Result<()> {
        let (directory, mut state) = state_for(
            NodeKind::Storage,
            json!({
                "data-dir": directory_placeholder(),
                "listen-ip": "0.0.0.0",
                "listen-port": 8091,
                "disc-port": 8090,
                "nat": "any",
                "network": "logos.test",
                "log-level": "INFO"
            }),
        )?;
        let workspace = directory.path().join("devnet");
        let config_path = workspace.join("configs/storage.json");
        let mut value: Value = serde_json::from_slice(&fs::read(&config_path)?)?;
        value["data-dir"] = Value::String(workspace.join("data/storage").display().to_string());
        fs::write(&config_path, serde_json::to_vec_pretty(&value)?)?;
        let initial = snapshot(&state, None, "local", NodeKind::Storage)?;
        let before = fs::read(&config_path)?;
        let invalid = save(
            &mut state,
            None,
            "local",
            NodeKind::Storage,
            "{",
            &initial.revision,
            persist_success,
        );
        assert!(invalid.is_err());
        assert_eq!(fs::read(&config_path)?, before);
        let stale = save(
            &mut state,
            None,
            "local",
            NodeKind::Storage,
            &initial.raw_text,
            "stale",
            persist_success,
        );
        assert!(stale.is_err());
        assert_eq!(fs::read(&config_path)?, before);
        Ok(())
    }

    #[test]
    fn failed_state_persistence_rolls_back_config_manifest_and_memory() -> Result<()> {
        let (directory, mut state) = state_for(
            NodeKind::Storage,
            json!({
                "data-dir": directory_placeholder(),
                "listen-ip": "0.0.0.0",
                "listen-port": 8091,
                "disc-port": 8090,
                "nat": "any",
                "network": "logos.test",
                "log-level": "INFO"
            }),
        )?;
        let workspace = directory.path().join("devnet");
        let config_path = workspace.join("configs/storage.json");
        let mut original: Value = serde_json::from_slice(&fs::read(&config_path)?)?;
        original["data-dir"] = Value::String(workspace.join("data/storage").display().to_string());
        fs::write(&config_path, serde_json::to_vec_pretty(&original)?)?;
        let snapshot = snapshot(&state, None, "local", NodeKind::Storage)?;
        let before_config = fs::read(&config_path)?;
        let before_state = serde_json::to_vec(&state)?;
        let manifest_path = workspace.join("local-network.json");
        assert!(!manifest_path.exists());
        let mut replacement: Value = serde_json::from_str(&snapshot.raw_text)?;
        replacement["log-level"] = Value::String("DEBUG".to_owned());
        let mut persist_attempts = 0;

        let error = save(
            &mut state,
            None,
            "local",
            NodeKind::Storage,
            &serde_json::to_string(&replacement)?,
            &snapshot.revision,
            |_state| {
                persist_attempts += 1;
                if persist_attempts == 1 {
                    return Err(anyhow::anyhow!("injected state-store failure"));
                }
                Ok(())
            },
        )
        .expect_err("failed state persistence must fail the configuration save");

        assert!(format!("{error:#}").contains("injected state-store failure"));
        assert_eq!(persist_attempts, 2);
        assert_eq!(fs::read(&config_path)?, before_config);
        assert!(!manifest_path.exists());
        assert_eq!(serde_json::to_vec(&state)?, before_state);
        Ok(())
    }

    #[test]
    fn failure_after_replacing_config_rolls_back_before_state_persistence() -> Result<()> {
        let (directory, mut state) = state_for(
            NodeKind::Storage,
            json!({
                "data-dir": directory_placeholder(),
                "listen-ip": "0.0.0.0",
                "listen-port": 8091,
                "disc-port": 8090,
                "nat": "any",
                "network": "logos.test",
                "log-level": "INFO"
            }),
        )?;
        let workspace = directory.path().join("devnet");
        let config_path = workspace.join("configs/storage.json");
        let mut original: Value = serde_json::from_slice(&fs::read(&config_path)?)?;
        original["data-dir"] = Value::String(workspace.join("data/storage").display().to_string());
        fs::write(&config_path, serde_json::to_vec_pretty(&original)?)?;
        let manifest_path = workspace.join("local-network.json");
        let manifest_before = b"{\"preserve\":true}\n";
        fs::write(&manifest_path, manifest_before)?;
        let snapshot = snapshot(&state, None, "local", NodeKind::Storage)?;
        let config_before = fs::read(&config_path)?;
        let state_before = serde_json::to_vec(&state)?;
        let mut replacement: Value = serde_json::from_str(&snapshot.raw_text)?;
        replacement["log-level"] = Value::String("DEBUG".to_owned());
        let replacement_text = serde_json::to_string(&replacement)?;

        let error = save_with_writer(
            &mut state,
            ConfigSaveRequest {
                runtime: None,
                profile: "local",
                kind: NodeKind::Storage,
                text: &replacement_text,
                expected_revision: &snapshot.revision,
            },
            persist_success,
            |node_kind, workspace, path, editor_value, old_value| {
                write_editor_value(node_kind, workspace, path, editor_value, old_value)?;
                bail!("injected post-replacement failure");
            },
        )
        .expect_err("a post-replacement write failure must roll back");

        assert!(format!("{error:#}").contains("injected post-replacement failure"));
        assert_eq!(fs::read(&config_path)?, config_before);
        assert_eq!(fs::read(&manifest_path)?, manifest_before);
        assert_eq!(serde_json::to_vec(&state)?, state_before);
        Ok(())
    }

    #[test]
    fn validation_rejects_paths_outside_the_topology_workspace() -> Result<()> {
        let (directory, state) = state_for(
            NodeKind::Storage,
            json!({
                "data-dir": "/tmp/outside",
                "listen-ip": "0.0.0.0",
                "listen-port": 8091,
                "disc-port": 8090,
                "nat": "any",
                "network": "logos.test",
                "log-level": "INFO"
            }),
        )?;
        let _keep = directory;
        let result = validate(
            &state,
            "local",
            NodeKind::Storage,
            &serde_json::to_string(&json!({
                "data-dir": "/tmp/outside",
                "listen-ip": "0.0.0.0",
                "listen-port": 8091,
                "disc-port": 8090,
                "nat": "any",
                "network": "logos.test",
                "log-level": "INFO"
            }))?,
        )?;
        assert!(!result.valid);
        assert!(result.error.contains("Data directory must stay inside"));
        Ok(())
    }

    #[test]
    fn save_rejects_manifest_path_outside_the_topology_before_writing_config() -> Result<()> {
        let (directory, mut state) = state_for(
            NodeKind::Storage,
            json!({
                "data-dir": directory_placeholder(),
                "listen-ip": "0.0.0.0",
                "listen-port": 8091,
                "disc-port": 8090,
                "nat": "any",
                "network": "logos.test",
                "log-level": "INFO"
            }),
        )?;
        let workspace = directory.path().join("devnet");
        let config_path = workspace.join("configs/storage.json");
        let mut original: Value = serde_json::from_slice(&fs::read(&config_path)?)?;
        original["data-dir"] = Value::String(workspace.join("data/storage").display().to_string());
        fs::write(&config_path, serde_json::to_vec_pretty(&original)?)?;
        let snapshot = snapshot(&state, None, "local", NodeKind::Storage)?;
        let before = fs::read(&config_path)?;
        state.devnets[0].manifest_path = directory
            .path()
            .join("outside-manifest.json")
            .display()
            .to_string();

        let error = save(
            &mut state,
            None,
            "local",
            NodeKind::Storage,
            &snapshot.raw_text,
            &snapshot.revision,
            persist_success,
        )
        .expect_err("an out-of-workspace manifest must block a configuration save");

        assert!(error.to_string().contains("outside its topology workspace"));
        assert_eq!(fs::read(&config_path)?, before);
        Ok(())
    }

    #[test]
    fn attached_service_allows_owned_stopped_storage_configuration() -> Result<()> {
        let (directory, mut state) = state_for(
            NodeKind::Storage,
            json!({
                "data-dir": directory_placeholder(),
                "listen-ip": "0.0.0.0",
                "listen-port": 8091,
                "disc-port": 8090,
                "nat": "any",
                "network": "logos.test",
                "log-level": "INFO"
            }),
        )?;
        let workspace = directory.path().join("devnet");
        let config_path = workspace.join("configs/storage.json");
        let mut original: Value = serde_json::from_slice(&fs::read(&config_path)?)?;
        original["data-dir"] = Value::String(workspace.join("data/storage").display().to_string());
        fs::write(&config_path, serde_json::to_vec_pretty(&original)?)?;
        let runtime = attached_runtime();

        let snapshot = snapshot(&state, Some(&runtime), "local", NodeKind::Storage)?;
        assert!(snapshot.editable);
        let mut replacement: Value = serde_json::from_str(&snapshot.raw_text)?;
        replacement["log-level"] = Value::String("DEBUG".to_owned());
        save(
            &mut state,
            Some(&runtime),
            "local",
            NodeKind::Storage,
            &serde_json::to_string(&replacement)?,
            &snapshot.revision,
            persist_success,
        )?;

        let stored: Value = serde_json::from_slice(&fs::read(config_path)?)?;
        assert_eq!(stored["log-level"], "DEBUG");
        Ok(())
    }

    #[test]
    fn attached_service_never_reads_or_writes_unmanaged_configuration() -> Result<()> {
        let (directory, mut state) = state_for(NodeKind::Storage, json!({}))?;
        let config_path = directory.path().join("devnet/configs/storage.json");
        let before = fs::read(&config_path)?;
        state.managed_workspace_root = directory
            .path()
            .join("unmanaged-workspace")
            .display()
            .to_string();
        let runtime = attached_runtime();

        let snapshot = snapshot(&state, Some(&runtime), "local", NodeKind::Storage)?;
        assert!(!snapshot.editable);
        assert!(snapshot.topology_id.is_empty());
        assert!(snapshot.raw_text.is_empty());
        assert!(snapshot.config_path.is_empty());
        assert_eq!(snapshot.config_role, "Unadopted local service");
        assert_eq!(
            snapshot.blocked_reason.as_deref(),
            Some(UNADOPTED_LOCAL_SERVICE)
        );
        let error = save(
            &mut state,
            Some(&runtime),
            "local",
            NodeKind::Storage,
            "{}",
            "revision",
            persist_success,
        )
        .expect_err("an attached service must not write an unmanaged configuration");
        assert_eq!(error.to_string(), UNADOPTED_LOCAL_SERVICE);
        assert_eq!(fs::read(config_path)?, before);
        Ok(())
    }

    #[test]
    fn attached_service_without_topology_never_reads_or_writes_configuration() -> Result<()> {
        let (directory, mut state) = state_for(NodeKind::Storage, json!({}))?;
        let config_path = directory.path().join("devnet/configs/storage.json");
        let before = fs::read(&config_path)?;
        state.active_devnet = None;
        state.devnets.clear();
        let runtime = attached_runtime();

        let snapshot = snapshot(&state, Some(&runtime), "local", NodeKind::Storage)?;
        assert!(!snapshot.editable);
        assert!(snapshot.topology_id.is_empty());
        assert!(snapshot.raw_text.is_empty());
        assert!(snapshot.config_path.is_empty());
        assert_eq!(snapshot.config_role, "Unadopted local service");
        assert_eq!(
            snapshot.blocked_reason.as_deref(),
            Some(UNADOPTED_LOCAL_SERVICE)
        );
        let error = save(
            &mut state,
            Some(&runtime),
            "local",
            NodeKind::Storage,
            "{}",
            "revision",
            persist_success,
        )
        .expect_err("an attached service without a topology must not write a configuration");
        assert_eq!(error.to_string(), UNADOPTED_LOCAL_SERVICE);
        assert_eq!(fs::read(config_path)?, before);
        Ok(())
    }

    fn directory_placeholder() -> String {
        "/tmp/placeholder".to_owned()
    }
}
