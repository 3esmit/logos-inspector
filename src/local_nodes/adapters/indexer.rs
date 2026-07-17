use serde_json::Value;

use crate::source_routing::execution_zone_layer;

use super::{
    LocalNodeAdapter, LocalNodeConfigRecord, ManagedCommandResultContract, NodeAction,
    NodeCommandContext, NodeCommandPlan, NodeConfigContext, NodeKind, NodeLifecycle,
    managed_action,
};

#[derive(Debug)]
pub(super) struct IndexerAdapter;

pub(super) static INDEXER_ADAPTER: IndexerAdapter = IndexerAdapter;

impl LocalNodeAdapter for IndexerAdapter {
    fn kind(&self) -> NodeKind {
        NodeKind::Indexer
    }

    fn label(&self) -> &'static str {
        "Indexer"
    }

    fn default_port(&self) -> Option<u16> {
        None
    }

    fn endpoint(&self, _port: Option<u16>) -> Option<String> {
        None
    }

    fn lifecycle(&self) -> NodeLifecycle {
        NodeLifecycle::InitializedModule(execution_zone_layer::managed_indexer_contract())
    }

    fn workflow_actions(&self) -> &'static [NodeAction] {
        &[NodeAction::Install, NodeAction::Start, NodeAction::Stop]
    }

    fn preserve_generated_config_on_runtime_reset(&self) -> bool {
        true
    }

    fn installation_survives_runtime_reset(&self, config: &LocalNodeConfigRecord) -> bool {
        std::path::Path::new(&config.config_path).is_file()
            && config
                .package_path
                .as_deref()
                .is_some_and(|path| std::path::Path::new(path).is_file())
    }

    fn ensure_loaded_before_start(&self) -> bool {
        true
    }

    fn package_managed(&self) -> bool {
        true
    }

    fn package_installation_matches_runtime(
        &self,
        config: &LocalNodeConfigRecord,
        runtime: Option<&super::super::runtime::LogoscoreRuntimeProfile>,
    ) -> bool {
        let Some(package_modules_dir) = config
            .package_path
            .as_deref()
            .and_then(super::super::package::package_path_modules_dir)
        else {
            return false;
        };
        let Some(runtime_modules_dir) = runtime.and_then(|profile| profile.modules_dir.as_deref())
        else {
            return true;
        };
        let Ok(runtime_modules_dir) =
            super::super::package::canonical_modules_dir(std::path::Path::new(runtime_modules_dir))
        else {
            return false;
        };
        package_modules_dir == runtime_modules_dir
    }

    fn build_config(&self, context: NodeConfigContext<'_>) -> Value {
        execution_zone_layer::managed_indexer_config(
            context.network_id,
            context.data_dir,
            context.endpoint,
            context.port,
            context.public_testnet,
        )
    }

    fn command_plan(
        &self,
        action: NodeAction,
        context: NodeCommandContext<'_>,
    ) -> Option<NodeCommandPlan> {
        let contract = execution_zone_layer::managed_indexer_contract();
        let call = if action == NodeAction::Purge {
            execution_zone_layer::managed_indexer_reset_storage_call(context.config_path)
        } else {
            contract.call_spec(managed_action(action)?, context.config_path)?
        };
        Some(NodeCommandPlan::ManagedModule {
            contract,
            call,
            result_contract: ManagedCommandResultContract::OperationStatusZero,
        })
    }
}
