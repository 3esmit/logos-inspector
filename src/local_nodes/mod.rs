use anyhow::Result;

use crate::support::command_runner::CommandControl;

mod action_engine;
mod action_workspace;
mod adapters;
mod commands;
mod lifecycle;
mod model;
mod paths;
mod presentation;
mod process;
mod runtime;
mod workflow;

pub use model::{
    LocalDevnetListReport, LocalDevnetRecord, LocalNodeActionRequest, LocalNodeConfigRecord,
    LocalNodeOperationReport, LocalNodeProblemCode, LocalNodeReport, LocalNodeStatus,
    LocalNodeSummary, LocalNodeTools, NodeAction, NodeKind, NodeLifecycleState, ToolStatus,
};
pub use runtime::LogoscoreRuntimeStatus;

pub fn local_nodes_status(profile: &str) -> Result<LocalNodeReport> {
    action_engine::LocalNodeActionEngine::system()?.status(profile)
}

pub fn local_devnet_list(profile: &str) -> Result<LocalDevnetListReport> {
    action_engine::LocalNodeActionEngine::system()?.devnets(profile)
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
) -> Result<LocalNodeReport> {
    action_engine::LocalNodeActionEngine::system()?.apply_controlled(
        profile,
        request,
        confirmation,
        control,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Context as _, Result, bail};
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
    fn testnet_profile_excludes_local_sequencer_and_purge() {
        let nodes = workflow::node_set_for_profile("default");
        let actions = workflow::available_actions_for("default", Some(NodeKind::Bedrock), true);

        assert!(!nodes.contains(&NodeKind::Sequencer));
        assert!(!actions.contains(&NodeAction::Purge));
    }

    #[test]
    fn report_exposes_normalized_profile_and_network_actions() {
        let state = LocalNodesState {
            version: 1,
            active_devnet: Some("devnet".to_owned()),
            managed_workspace_root: "/tmp/local-nodes".to_owned(),
            devnets: vec![LocalDevnetRecord {
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
    fn indexer_is_not_actionable_without_a_verified_module_contract() -> Result<()> {
        let state = LocalNodesState::default_for_config_dir(Path::new("/tmp/local-nodes-contract"));

        let report = action_engine::report_for_state("local", &state);
        let indexer = report
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Indexer)
            .context("missing indexer node")?;

        if indexer.install_state != "needs_configuration"
            || !indexer.available_actions.is_empty()
            || !indexer
                .detail
                .contains("no verified logoscore module lifecycle contract")
        {
            bail!("unexpected indexer discovery gate: {indexer:?}");
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
            endpoint: None,
            data_dir: None,
            config_path: None,
            package_path: None,
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
            "/tmp/bedrock.json",
            "local",
        )
        .context("missing bedrock command")?;
        let expected_bedrock = vec![
            "call",
            "blockchain_module",
            "start",
            "/tmp/bedrock.json",
            "",
            "--json",
        ];
        if bedrock.args != expected_bedrock {
            bail!("unexpected bedrock command: {:?}", bedrock.args);
        }

        if command_spec_for(
            NodeKind::Indexer,
            NodeAction::Start,
            "/tmp/indexer.json",
            "local",
        )
        .is_some()
        {
            bail!("indexer start must stay unavailable without a verified module contract");
        }

        let messaging = command_spec_for(
            NodeKind::Messaging,
            NodeAction::Initialize,
            "/tmp/delivery.json",
            "local",
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
            let spec = command_spec_for(kind, action, "/tmp/ignored.json", "local")
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
            "local",
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

        if parsed.version != 1 {
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
