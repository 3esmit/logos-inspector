use std::{any::Any, time::Duration};

use anyhow::{Context as _, Result};

use crate::modules::logos_core::LogoscoreCliRuntime;
use crate::support::command_runner::CommandControl;

mod action_engine;
mod action_workspace;
mod adapters;
mod channel_indexer;
mod commands;
mod lifecycle;
mod messaging_identity;
mod model;
mod module_watcher;
mod package;
mod paths;
mod presentation;
mod process;
mod runtime;
mod workflow;

pub(crate) const INDEXER_PACKAGE_INSTALL_TIMEOUT: Duration = Duration::from_secs(15 * 60);

struct LocalNodePackageCommitControl {
    command: CommandControl,
    _lease: Box<dyn Any + Send>,
}

pub(crate) struct LocalNodePackageCommit {
    begin: Option<Box<dyn FnOnce() -> Result<LocalNodePackageCommitControl> + Send>>,
    active: Option<LocalNodePackageCommitControl>,
}

impl LocalNodePackageCommit {
    pub(crate) fn new<F, L>(begin: F) -> Self
    where
        F: FnOnce() -> Result<(CommandControl, L)> + Send + 'static,
        L: Any + Send,
    {
        Self {
            begin: Some(Box::new(move || {
                let (command, lease) = begin()?;
                Ok(LocalNodePackageCommitControl {
                    command,
                    _lease: Box::new(lease),
                })
            })),
            active: None,
        }
    }

    fn begin(&mut self) -> Result<CommandControl> {
        if self.active.is_none() {
            let begin = self
                .begin
                .take()
                .context("Indexer package commit control is unavailable")?;
            self.active = Some(begin()?);
        }
        self.active
            .as_ref()
            .map(|active| active.command.clone())
            .context("Indexer package commit did not become active")
    }

    #[cfg(test)]
    pub(crate) fn begin_for_test(&mut self) -> Result<CommandControl> {
        self.begin()
    }
}

pub(crate) use channel_indexer::ChannelIndexerActionRequest;
pub use model::{
    LocalDevnetListReport, LocalDevnetRecord, LocalNodeActionRequest, LocalNodeConfigRecord,
    LocalNodeDeployment, LocalNodeOperationReport, LocalNodeProblemCode, LocalNodeReport,
    LocalNodeStatus, LocalNodeSummary, LocalNodeTools, NodeAction, NodeKind, NodeLifecycleState,
    ToolStatus,
};
pub use module_watcher::{LocalNodeModuleSubscription, LocalNodeModuleWatcher};
pub use package::{
    LocalNodeInstalledPackageReport, LocalNodePackageCatalogEntry, LocalNodePackageCatalogReport,
    LocalNodePackageRelease,
};
pub use runtime::LogoscoreRuntimeStatus;

pub fn local_nodes_status(profile: &str) -> Result<LocalNodeReport> {
    action_engine::LocalNodeActionEngine::system()?.status(profile)
}

pub(crate) fn channel_indexer_status(
    profile: &str,
    network_scope: &crate::inspection::NetworkScope,
    channel_id: &str,
) -> Result<LocalNodeReport> {
    action_engine::LocalNodeActionEngine::system()?.channel_indexer_status(
        profile,
        network_scope,
        channel_id,
    )
}

pub fn local_devnet_list(profile: &str) -> Result<LocalDevnetListReport> {
    action_engine::LocalNodeActionEngine::system()?.devnets(profile)
}

pub fn local_node_package_catalog(
    modules_dir: Option<&str>,
) -> Result<LocalNodePackageCatalogReport> {
    package::local_node_package_catalog(modules_dir)
}

pub fn local_nodes_action(
    profile: &str,
    request: LocalNodeActionRequest,
    confirmation: Option<&str>,
) -> Result<LocalNodeReport> {
    action_engine::LocalNodeActionEngine::system()?.apply(profile, request, confirmation)
}

pub(crate) fn local_nodes_action_controlled(
    profile: &str,
    request: LocalNodeActionRequest,
    confirmation: Option<&str>,
    control: CommandControl,
    package_commit: LocalNodePackageCommit,
) -> Result<LocalNodeReport> {
    action_engine::LocalNodeActionEngine::system()?.apply_controlled(
        profile,
        request,
        confirmation,
        control,
        package_commit,
    )
}

pub(crate) fn channel_indexer_action_controlled(
    profile: &str,
    request: ChannelIndexerActionRequest,
    confirmation: Option<&str>,
    control: CommandControl,
) -> Result<LocalNodeReport> {
    action_engine::LocalNodeActionEngine::system()?.channel_indexer_action_controlled(
        profile,
        request,
        confirmation,
        control,
    )
}

pub(crate) fn running_managed_logoscore_runtime() -> Result<Option<LogoscoreCliRuntime>> {
    let config_dir = crate::support::state_store::config_dir()?;
    let profile = runtime::LogoscoreRuntimeStore::system(config_dir).load()?;
    profile
        .filter(runtime::LogoscoreRuntimeProfile::is_running)
        .map(|profile| profile.cli_runtime())
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Result, bail};
    use serde_json::Value;
    use std::{env, fs, path::Path};

    use super::{
        action_engine, action_workspace, commands::command_spec_for, model::LocalNodesState,
        paths::path_is_inside, workflow,
    };
    use crate::support::time::now_millis;

    #[test]
    fn local_profile_includes_sequencer_and_network_actions() {
        let nodes = workflow::node_set_for_profile("local");

        assert!(nodes.contains(&NodeKind::Sequencer));
        assert!(
            workflow::available_actions_for("local", None, false).contains(&NodeAction::NewNetwork)
        );
        assert!(
            !workflow::available_actions_for("default", None, false)
                .contains(&NodeAction::NewNetwork)
        );
        assert_eq!(NodeAction::NewNetwork.label(), "New Local Devnet");
        assert_eq!(NodeAction::ResetNetwork.label(), "Reset Local Devnet");
    }

    #[test]
    fn testnet_profile_excludes_local_sequencer_and_exposes_node_actions() {
        let nodes = workflow::node_set_for_profile("default");
        let actions = workflow::available_actions_for("default", Some(NodeKind::Bedrock), true);

        assert!(!nodes.contains(&NodeKind::Sequencer));
        assert!(actions.contains(&NodeAction::Initialize));
        assert!(actions.contains(&NodeAction::Purge));
    }

    #[test]
    fn report_exposes_normalized_profile_and_network_actions() {
        let state = LocalNodesState {
            version: 3,
            active_devnet: Some("devnet".to_owned()),
            module_context_topology_by_kind: std::collections::BTreeMap::new(),
            testnet: None,
            managed_workspace_root: "/tmp/local-nodes".to_owned(),
            devnets: vec![LocalDevnetRecord {
                deployment: LocalNodeDeployment::LocalDevnet,
                id: "devnet".to_owned(),
                label: "Devnet".to_owned(),
                workspace: "/tmp/local-nodes/devnet".to_owned(),
                manifest_path: "/tmp/local-nodes/devnet/local-network.json".to_owned(),
                created_at: 0,
                updated_at: 0,
                nodes: Vec::new(),
            }],
            operations: Vec::new(),
        };

        let report = action_engine::report_for_state("devnet", &state);

        assert_eq!(report.profile, "local");
        assert_eq!(report.mode, "localnet");
        assert!(
            report
                .available_network_actions
                .contains(&NodeAction::NewNetwork)
        );
        assert!(
            report
                .available_network_actions
                .contains(&NodeAction::ResetNetwork)
        );
        assert!(
            report
                .available_network_actions
                .contains(&NodeAction::DeleteNetwork)
        );
    }

    #[test]
    fn local_node_primary_problem_prefers_missing_logoscore() {
        let nodes = vec![local_node_status(
            NodeKind::Sequencer,
            "needs_configuration",
        )];
        let missing_logoscore = LocalNodeTools {
            logoscore: ToolStatus {
                available: false,
                command: "logoscore".to_owned(),
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
        };
        let configured_tools = LocalNodeTools {
            logoscore: ToolStatus {
                available: true,
                command: "logoscore".to_owned(),
                path: Some("/usr/bin/logoscore".to_owned()),
            },
            lgpd: ToolStatus {
                available: true,
                command: "lgpd".to_owned(),
                path: Some("/usr/bin/lgpd".to_owned()),
            },
            lgpm: ToolStatus {
                available: false,
                command: "lgpm".to_owned(),
                path: None,
            },
        };

        assert_eq!(
            super::presentation::primary_problem("local", &missing_logoscore, &nodes),
            Some(LocalNodeProblemCode::MissingLogoscore)
        );
        assert_eq!(
            super::presentation::primary_problem("local", &configured_tools, &nodes),
            Some(LocalNodeProblemCode::MissingSequencerBinary)
        );
        assert_eq!(
            super::presentation::primary_problem("default", &configured_tools, &nodes),
            None
        );
    }

    #[test]
    fn indexer_uses_package_managed_module_lifecycle() -> Result<()> {
        let adapter = super::adapters::adapter_for(NodeKind::Indexer);
        if !matches!(
            adapter.lifecycle(),
            super::adapters::NodeLifecycle::InitializedModule(contract)
                if contract.module_id() == "lez_indexer_module"
        ) || adapter.workflow_actions()
            != [NodeAction::Install, NodeAction::Start, NodeAction::Stop]
            || adapter.default_port().is_some()
            || adapter.startup_rpc_readiness().is_some()
            || !adapter.package_managed()
            || !adapter.preserve_generated_config_on_runtime_reset()
        {
            bail!("Indexer did not expose its package-managed module contract");
        }
        Ok(())
    }

    fn local_node_status(kind: NodeKind, install_state: &str) -> LocalNodeStatus {
        LocalNodeStatus {
            kind,
            key: kind.as_str().to_owned(),
            label: kind.as_str().to_owned(),
            install_state: install_state.to_owned(),
            run_state: "stopped".to_owned(),
            ownership: "external".to_owned(),
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

    #[test]
    fn command_specs_match_module_adapters() -> Result<()> {
        let bedrock = command_spec_for(
            NodeKind::Bedrock,
            NodeAction::Start,
            "/tmp/bedrock.yaml",
            "/tmp/bedrock",
            Some(8080),
        )
        .context("missing bedrock command")?;
        let expected_bedrock = vec![
            "call",
            "blockchain_module",
            "start",
            "/tmp/bedrock.yaml",
            "",
            "--json",
        ];
        if bedrock.args != expected_bedrock {
            bail!("unexpected bedrock command: {:?}", bedrock.args);
        }

        let indexer = command_spec_for(
            NodeKind::Indexer,
            NodeAction::Start,
            "/tmp/indexer.json",
            "/tmp/indexer-data",
            None,
        )
        .context("missing indexer command")?;
        if indexer.args
            != [
                "call",
                "lez_indexer_module",
                "start_indexer",
                "/tmp/indexer.json",
                "--json",
            ]
        {
            bail!("unexpected indexer command: {:?}", indexer.args);
        }

        let messaging = command_spec_for(
            NodeKind::Messaging,
            NodeAction::Initialize,
            "/tmp/delivery.json",
            "/tmp/delivery",
            Some(8645),
        )
        .context("missing messaging command")?;
        let expected_messaging = vec![
            "call",
            "delivery_module",
            "createNode",
            "@/tmp/delivery.json",
            "--json",
        ];
        if messaging.args != expected_messaging {
            bail!("unexpected messaging command: {:?}", messaging.args);
        }
        Ok(())
    }

    #[test]
    fn local_devnet_writes_native_storage_and_delivery_configs() -> Result<()> {
        let config = env::temp_dir().join(format!(
            "logos-inspector-local-native-config-{}",
            now_millis()
        ));
        let mut state = LocalNodesState::default_for_config_dir(&config);
        let request = LocalNodeActionRequest {
            action: NodeAction::NewNetwork,
            node: None,
            network_id: Some("native-config".to_owned()),
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            package_version: None,
            package_root_hash: None,
            channel_id: None,
            bedrock_endpoint: None,
            allow_identity_rotation: false,
            label: None,
        };
        let mut runtime = None;

        let operation = action_workspace::LocalNodeActionWorkspace::system().apply(
            &mut state,
            &mut runtime,
            &config,
            "local",
            &request,
            None,
        );
        if operation.report.status != "created" {
            bail!("unexpected operation status: {}", operation.report.status);
        }
        let record = state.active_devnet().context("missing active devnet")?;
        let storage = record
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Storage)
            .context("missing storage config")?;
        let storage_config: Value = serde_json::from_str(
            &fs::read_to_string(&storage.config_path)
                .with_context(|| format!("failed to read {}", storage.config_path))?,
        )?;
        if storage_config.pointer("/data-dir").and_then(Value::as_str)
            != Some(storage.data_dir.as_str())
            || storage_config.pointer("/log-level").and_then(Value::as_str) != Some("INFO")
            || storage_config.get("network_id").is_some()
        {
            bail!("unexpected storage config: {storage_config}");
        }

        let delivery = record
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Messaging)
            .context("missing delivery config")?;
        let delivery_config: Value = serde_json::from_str(
            &fs::read_to_string(&delivery.config_path)
                .with_context(|| format!("failed to read {}", delivery.config_path))?,
        )?;
        if delivery_config.pointer("/mode").and_then(Value::as_str) != Some("Core")
            || delivery_config.pointer("/preset").and_then(Value::as_str) != Some("logos.test")
            || delivery_config.pointer("/rest").and_then(Value::as_bool) != Some(true)
            || delivery_config.pointer("/restPort").and_then(Value::as_u64) != Some(8645)
            || delivery_config.get("network_id").is_some()
        {
            bail!("unexpected delivery config: {delivery_config}");
        }
        fs::remove_dir_all(&config)
            .with_context(|| format!("failed to remove {}", config.display()))?;
        Ok(())
    }

    #[test]
    fn controlled_action_stops_before_local_state_mutation() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = LocalNodesState::default_for_config_dir(directory.path());
        let mut runtime = None;
        let request = LocalNodeActionRequest {
            action: NodeAction::NewNetwork,
            node: None,
            network_id: Some("must-not-exist".to_owned()),
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            package_version: None,
            package_root_hash: None,
            channel_id: None,
            bedrock_endpoint: None,
            allow_identity_rotation: false,
            label: None,
        };
        let cancellation = tokio_util::sync::CancellationToken::new();
        cancellation.cancel();
        let deadline = std::time::Instant::now()
            .checked_add(std::time::Duration::from_secs(5))
            .context("local node test deadline overflow")?;
        let control = CommandControl::new(cancellation, deadline);

        let operation = action_workspace::LocalNodeActionWorkspace::system().apply(
            &mut state,
            &mut runtime,
            directory.path(),
            "local",
            &request,
            Some(&control),
        );

        anyhow::ensure!(
            operation
                .interruption
                .as_ref()
                .and_then(|error| {
                    error.downcast_ref::<crate::support::command_runner::CommandTerminated>()
                })
                .is_some(),
            "controlled LocalNodes stop lost typed interruption"
        );
        anyhow::ensure!(
            operation.report.status == "failed"
                && state.active_devnet.is_none()
                && state.devnets.is_empty(),
            "controlled LocalNodes action mutated state: {:?}",
            state.active_devnet
        );
        Ok(())
    }

    #[test]
    fn module_lifecycle_commands_preserve_discovered_arities() -> Result<()> {
        let cases = [
            (
                NodeKind::Bedrock,
                NodeAction::Stop,
                "blockchain_module",
                "stop",
            ),
            (
                NodeKind::Storage,
                NodeAction::Start,
                "storage_module",
                "start",
            ),
            (
                NodeKind::Storage,
                NodeAction::Stop,
                "storage_module",
                "stop",
            ),
            (
                NodeKind::Storage,
                NodeAction::Uninstall,
                "storage_module",
                "destroy",
            ),
            (
                NodeKind::Messaging,
                NodeAction::Start,
                "delivery_module",
                "start",
            ),
            (
                NodeKind::Messaging,
                NodeAction::Stop,
                "delivery_module",
                "stop",
            ),
        ];

        for (kind, action, module, method) in cases {
            let spec = command_spec_for(kind, action, "/tmp/ignored.json", "/tmp/data", None)
                .with_context(|| format!("missing {module}.{method} command"))?;

            if spec.args != ["call", module, method, "--json"] {
                bail!("unexpected {module}.{method} command: {:?}", spec.args);
            }
        }
        Ok(())
    }

    #[test]
    fn storage_init_reads_json_through_cli_file_argument() -> Result<()> {
        let spec = command_spec_for(
            NodeKind::Storage,
            NodeAction::Initialize,
            "/tmp/storage.json",
            "/tmp/storage",
            None,
        )
        .context("missing storage init command")?;

        if spec.args
            != [
                "call",
                "storage_module",
                "init",
                "@/tmp/storage.json",
                "--json",
            ]
        {
            bail!("unexpected storage init command: {:?}", spec.args);
        }
        Ok(())
    }

    #[test]
    fn state_serialization_round_trips() -> Result<()> {
        let config = env::temp_dir().join(format!(
            "logos-inspector-local-nodes-state-{}",
            now_millis()
        ));
        let state = LocalNodesState::default_for_config_dir(&config);

        let text = serde_json::to_string(&state)?;
        let parsed: LocalNodesState = serde_json::from_str(&text)?;

        if parsed.version != model::LOCAL_NODES_STATE_VERSION {
            bail!("unexpected state version");
        }
        if !parsed.managed_workspace_root.ends_with("local-nodes") {
            bail!("managed workspace root was not migrated");
        }
        Ok(())
    }

    #[test]
    fn local_node_store_loads_default_and_round_trips_state() -> Result<()> {
        let config = env::temp_dir().join(format!(
            "logos-inspector-local-nodes-store-{}",
            now_millis()
        ));
        if config.exists() {
            fs::remove_dir_all(&config)
                .with_context(|| format!("failed to clear {}", config.display()))?;
        }
        let store = action_engine::LocalNodeStore::for_config_dir(config.clone());

        let mut state = store.load()?;
        if state.managed_workspace_root != config.join("local-nodes").display().to_string() {
            bail!("unexpected managed workspace root");
        }
        state.active_devnet = Some("devnet-a".to_owned());
        store.save(&state)?;

        let loaded = store.load()?;
        if loaded.active_devnet.as_deref() != Some("devnet-a") {
            bail!("local node state did not round trip");
        }
        fs::remove_dir_all(&config)
            .with_context(|| format!("failed to remove {}", config.display()))?;
        Ok(())
    }

    #[test]
    fn missing_local_node_state_materializes_public_testnet_topology() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = action_engine::LocalNodeStore::for_config_dir(directory.path().to_path_buf());

        let state = store.load()?;
        let testnet = state.testnet.context("missing public Testnet topology")?;
        if testnet.deployment != LocalNodeDeployment::PublicTestnet
            || testnet
                .nodes
                .iter()
                .any(|node| node.kind == NodeKind::Sequencer)
        {
            bail!("unexpected public Testnet topology: {testnet:?}");
        }
        let bedrock = testnet
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Bedrock)
            .context("missing Testnet Bedrock")?;
        let initialization_path = bedrock
            .initialization_config_path
            .as_deref()
            .context("missing Bedrock initialization config")?;
        let bedrock_config: Value =
            serde_json::from_str(&fs::read_to_string(initialization_path)?)?;
        if bedrock_config
            .pointer("/initial_peers/0")
            .and_then(Value::as_str)
            != crate::testnet::LOGOS_TESTNET_BOOTSTRAP_PEERS
                .first()
                .copied()
            || bedrock_config.get("output").and_then(Value::as_str)
                != Some(bedrock.config_path.as_str())
            || Path::new(&bedrock.config_path).exists()
        {
            bail!("unexpected Testnet Bedrock bootstrap config: {bedrock_config}");
        }
        let indexer = testnet
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Indexer)
            .context("missing Testnet Indexer")?;
        let indexer_config: Value =
            serde_json::from_str(&fs::read_to_string(&indexer.config_path)?)?;
        if indexer_config
            .pointer("/bedrock_config/addr")
            .and_then(Value::as_str)
            != Some("http://127.0.0.1:8080")
            || indexer_config.get("channel_id").and_then(Value::as_str)
                != Some(crate::testnet::LOGOS_TESTNET_CHANNEL_ID)
        {
            bail!("unexpected Testnet Indexer config: {indexer_config}");
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn bedrock_reinitialization_reuses_generated_config_without_key_generation() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir()?;
        let binary = directory.path().join("logoscore-fake");
        let calls = directory.path().join("calls.log");
        let config_path = directory.path().join("bedrock.yaml");
        let keystore_path = directory.path().join("keystore.yaml");
        let init_path = directory.path().join("bedrock.init.json");
        let manifest_path = directory.path().join("local-network.json");
        fs::write(&config_path, b"bedrock-sentinel")?;
        fs::write(&keystore_path, b"keystore-sentinel")?;
        fs::write(&init_path, b"{}")?;
        fs::write(
            &binary,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nprintf '%s\\n' '[{{\"name\":\"blockchain_module\",\"status\":\"loaded\"}}]'\n",
                calls.display()
            ),
        )?;
        let mut permissions = fs::metadata(&binary)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&binary, permissions)?;
        let record = LocalDevnetRecord {
            deployment: LocalNodeDeployment::PublicTestnet,
            id: "logos-testnet".to_owned(),
            label: "Logos Testnet".to_owned(),
            workspace: directory.path().display().to_string(),
            manifest_path: manifest_path.display().to_string(),
            created_at: 0,
            updated_at: 0,
            nodes: vec![LocalNodeConfigRecord {
                kind: NodeKind::Bedrock,
                config_path: config_path.display().to_string(),
                initialization_config_path: Some(init_path.display().to_string()),
                data_dir: directory.path().join("data").display().to_string(),
                endpoint: Some(crate::testnet::LOCAL_BEDROCK_ENDPOINT.to_owned()),
                port: Some(8080),
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
        };
        let mut state = LocalNodesState {
            version: 3,
            active_devnet: None,
            module_context_topology_by_kind: std::collections::BTreeMap::new(),
            testnet: Some(record),
            managed_workspace_root: directory.path().display().to_string(),
            devnets: Vec::new(),
            operations: Vec::new(),
        };
        let mut runtime = Some(runtime::LogoscoreRuntimeProfile {
            id: "test-runtime".to_owned(),
            binary_path: binary.display().to_string(),
            config_dir: directory.path().join("runtime").display().to_string(),
            modules_dir: Some(directory.path().display().to_string()),
            persistence_path: Some(directory.path().join("runtime-data").display().to_string()),
            ownership: runtime::LogoscoreRuntimeOwnership::InspectorManaged,
            timeout_profile: runtime::LogoscoreTimeoutProfile::Lifecycle,
            daemon_process_id: Some(std::process::id()),
        });
        let request = LocalNodeActionRequest {
            action: NodeAction::Initialize,
            node: Some(NodeKind::Bedrock),
            network_id: None,
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            package_version: None,
            package_root_hash: None,
            channel_id: None,
            bedrock_endpoint: None,
            allow_identity_rotation: false,
            label: None,
        };

        let operation = action_workspace::LocalNodeActionWorkspace::system().apply(
            &mut state,
            &mut runtime,
            directory.path(),
            "default",
            &request,
            None,
        );

        let calls = fs::read_to_string(&calls)?;
        let node = state
            .testnet
            .as_ref()
            .and_then(|topology| topology.nodes.first())
            .context("missing Bedrock node")?;
        if operation.report.status != "initialized"
            || calls.contains("generate_user_config")
            || !calls.contains("list-modules")
            || !node.installed
            || node.lifecycle_state != NodeLifecycleState::Stopped
            || fs::read(&config_path)? != b"bedrock-sentinel"
            || fs::read(&keystore_path)? != b"keystore-sentinel"
        {
            bail!(
                "Bedrock existing-config attach reran generation: operation={:?}, calls={calls}",
                operation.report
            );
        }
        Ok(())
    }

    #[test]
    fn action_workspace_creates_local_devnet_manifest() -> Result<()> {
        let config = env::temp_dir().join(format!(
            "logos-inspector-local-nodes-action-{}",
            now_millis()
        ));
        if config.exists() {
            fs::remove_dir_all(&config)
                .with_context(|| format!("failed to clear {}", config.display()))?;
        }
        let mut state = LocalNodesState::default_for_config_dir(&config);
        let request = LocalNodeActionRequest {
            action: NodeAction::NewNetwork,
            node: None,
            network_id: Some("Demo Net".to_owned()),
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            package_version: None,
            package_root_hash: None,
            channel_id: None,
            bedrock_endpoint: None,
            allow_identity_rotation: false,
            label: Some("Demo Net".to_owned()),
        };
        let mut runtime = None;

        let operation = action_workspace::LocalNodeActionWorkspace::system().apply(
            &mut state,
            &mut runtime,
            &config,
            "local",
            &request,
            None,
        );

        if operation.report.status != "created" {
            bail!("unexpected operation status: {}", operation.report.status);
        }
        if state.active_devnet.as_deref() != Some("demo-net") {
            bail!("unexpected active devnet: {:?}", state.active_devnet);
        }
        let Some(record) = state.active_devnet() else {
            bail!("created devnet was not active");
        };
        if record.nodes.is_empty() {
            bail!("created devnet has no node configs");
        }
        if !Path::new(&record.manifest_path).is_file() {
            bail!("manifest was not written: {}", record.manifest_path);
        }
        let manifest = fs::read_to_string(&record.manifest_path)
            .with_context(|| format!("failed to read {}", record.manifest_path))?;
        if !manifest.contains("\"id\": \"demo-net\"") {
            bail!("manifest did not contain sanitized devnet id: {manifest}");
        }
        fs::remove_dir_all(&config)
            .with_context(|| format!("failed to remove {}", config.display()))?;
        Ok(())
    }

    #[test]
    fn path_safety_rejects_sibling_and_parent_escape() {
        let root = Path::new("/tmp/logos-inspector/root");

        assert!(path_is_inside(
            root,
            Path::new("/tmp/logos-inspector/root/devnet/data")
        ));
        assert!(!path_is_inside(
            root,
            Path::new("/tmp/logos-inspector/root")
        ));
        assert!(!path_is_inside(
            root,
            Path::new("/tmp/logos-inspector/root/../other")
        ));
        assert!(!path_is_inside(
            root,
            Path::new("/tmp/logos-inspector/root2/data")
        ));
    }
}
