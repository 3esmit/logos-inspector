use std::sync::{Mutex, MutexGuard, OnceLock};

use anyhow::Result;

use crate::support::time::now_millis;

use super::{NodeLifecycleState, adapters::adapter_for, model::LocalNodesState};

static STATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub(super) fn acquire_state_lock() -> Result<MutexGuard<'static, ()>> {
    STATE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| anyhow::anyhow!("local node state lock is poisoned"))
}

pub(super) fn reset_module_contexts(state: &mut LocalNodesState) {
    state.clear_module_context_topologies();
    if let Some(record) = state.testnet.as_mut() {
        reset_record_contexts(record);
    }
    for record in &mut state.devnets {
        reset_record_contexts(record);
    }
}

fn reset_record_contexts(record: &mut super::model::LocalDevnetRecord) {
    for node in &mut record.nodes {
        let adapter = adapter_for(node.kind);
        if !adapter.resets_with_runtime() {
            continue;
        }
        let generated_config_available = adapter.preserve_generated_config_on_runtime_reset()
            && std::path::Path::new(&node.config_path).is_file();
        node.installed = generated_config_available;
        node.process_id = None;
        node.lifecycle_state = if generated_config_available {
            NodeLifecycleState::Stopped
        } else {
            NodeLifecycleState::NotInitialized
        };
        node.pending_lifecycle_action = None;
    }
    record.updated_at = now_millis();
}

#[cfg(test)]
mod tests {
    use anyhow::{Context as _, Result, bail};

    use super::*;
    use crate::local_nodes::{LocalNodeConfigRecord, NodeKind};

    #[test]
    fn runtime_restart_preserves_generated_bedrock_config_for_direct_start() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let config_path = directory.path().join("bedrock.yaml");
        let keystore_path = directory.path().join("keystore.yaml");
        std::fs::write(&config_path, b"bedrock-sentinel")?;
        std::fs::write(&keystore_path, b"keystore-sentinel")?;
        let mut state = LocalNodesState {
            version: 3,
            active_devnet: None,
            module_context_topology_by_kind: std::collections::BTreeMap::new(),
            testnet: Some(super::super::model::LocalDevnetRecord {
                deployment: super::super::model::LocalNodeDeployment::PublicTestnet,
                id: "logos-testnet".to_owned(),
                label: "Logos Testnet".to_owned(),
                workspace: directory.path().display().to_string(),
                manifest_path: directory
                    .path()
                    .join("local-network.json")
                    .display()
                    .to_string(),
                created_at: 0,
                updated_at: 0,
                nodes: vec![LocalNodeConfigRecord {
                    kind: NodeKind::Bedrock,
                    config_path: config_path.display().to_string(),
                    initialization_config_path: Some(
                        directory
                            .path()
                            .join("bedrock.init.json")
                            .display()
                            .to_string(),
                    ),
                    data_dir: directory.path().join("data").display().to_string(),
                    endpoint: Some(crate::testnet::LOCAL_BEDROCK_ENDPOINT.to_owned()),
                    port: Some(8080),
                    package_path: Some("logoscore".to_owned()),
                    module_path: None,
                    process_id: None,
                    installed: true,
                    lifecycle_state: NodeLifecycleState::Running,
                    pending_lifecycle_action: None,
                }],
            }),
            managed_workspace_root: directory.path().display().to_string(),
            devnets: Vec::new(),
            operations: Vec::new(),
        };

        reset_module_contexts(&mut state);

        let node = state
            .testnet
            .as_ref()
            .and_then(|record| record.nodes.first())
            .context("missing Bedrock node")?;
        let policy = adapter_for(NodeKind::Bedrock).action_policy(super::super::NodeAction::Start);
        if !node.installed
            || node.lifecycle_state != NodeLifecycleState::Stopped
            || !matches!(
                policy,
                super::super::adapters::NodeActionPolicy::ExecuteManaged {
                    ensure_loaded: true,
                    requires_installed_context: true,
                }
            )
            || std::fs::read(&config_path)? != b"bedrock-sentinel"
            || std::fs::read(&keystore_path)? != b"keystore-sentinel"
        {
            bail!("Bedrock restart did not preserve generated configuration: {node:?}");
        }
        Ok(())
    }
}
