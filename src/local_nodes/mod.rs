use anyhow::Result;

mod action_engine;
mod commands;
mod local_indexer;
mod model;
mod paths;
mod presentation;
mod process;
mod workflow;

pub use local_indexer::{
    bootstrap_default_local_indexer, bootstrap_default_local_indexer_for_saved_settings,
    default_local_indexer_requested_by_saved_settings, is_default_local_indexer_endpoint,
};
pub use model::{
    LocalDevnetListReport, LocalDevnetRecord, LocalNodeActionRequest, LocalNodeConfigRecord,
    LocalNodeOperationReport, LocalNodeProblemCode, LocalNodeReport, LocalNodeStatus,
    LocalNodeSummary, LocalNodeTools, NodeAction, NodeKind, ToolStatus,
};

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

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Context as _, Result, bail};
    use std::{env, fs, path::Path};

    use super::{
        action_engine, commands::command_spec_for, model::LocalNodesState, paths::path_is_inside,
        workflow,
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
            "local",
            "--json",
        ];
        if bedrock.args != expected_bedrock {
            bail!("unexpected bedrock command: {:?}", bedrock.args);
        }

        let indexer = command_spec_for(
            NodeKind::Indexer,
            NodeAction::Start,
            "/tmp/indexer.json",
            "local",
        )
        .context("missing indexer command")?;
        let expected_indexer = vec![
            "call",
            "lez_indexer_module",
            "start_indexer",
            "/tmp/indexer.json",
            "--json",
        ];
        if indexer.args != expected_indexer {
            bail!("unexpected indexer command: {:?}", indexer.args);
        }

        let messaging = command_spec_for(
            NodeKind::Messaging,
            NodeAction::Install,
            "/tmp/delivery.json",
            "local",
        )
        .context("missing messaging command")?;
        let expected_messaging = vec![
            "call",
            "delivery_module",
            "createNode",
            "/tmp/delivery.json",
            "--json",
        ];
        if messaging.args != expected_messaging {
            bail!("unexpected messaging command: {:?}", messaging.args);
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
