use std::time::Duration;

use serde_json::Value;

use crate::source_routing::execution_zone_layer;

use super::{
    LocalNodeAdapter, NodeAction, NodeCommandContext, NodeCommandPlan, NodeConfigContext, NodeKind,
    NodeLifecycle, RpcStartupReadiness,
};

#[derive(Debug)]
pub(super) struct IndexerAdapter;

pub(super) static INDEXER_ADAPTER: IndexerAdapter = IndexerAdapter;
const INDEXER_RPC_READINESS: RpcStartupReadiness = RpcStartupReadiness::new(
    "checkHealth",
    Duration::from_secs(120),
    Duration::from_secs(3),
    Duration::from_secs(1),
);

impl LocalNodeAdapter for IndexerAdapter {
    fn kind(&self) -> NodeKind {
        NodeKind::Indexer
    }

    fn label(&self) -> &'static str {
        "Indexer"
    }

    fn default_port(&self) -> Option<u16> {
        Some(8779)
    }

    fn lifecycle(&self) -> NodeLifecycle {
        NodeLifecycle::RegisteredProcess {
            program: execution_zone_layer::managed_indexer_program(),
        }
    }

    fn workflow_actions(&self) -> &'static [NodeAction] {
        &[
            NodeAction::Install,
            NodeAction::Start,
            NodeAction::Stop,
            NodeAction::Uninstall,
            NodeAction::Purge,
        ]
    }

    fn startup_rpc_readiness(&self) -> Option<RpcStartupReadiness> {
        Some(INDEXER_RPC_READINESS)
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
        (action == NodeAction::Start).then(|| NodeCommandPlan::DetachedProcess {
            program: execution_zone_layer::managed_indexer_program(),
            args: vec![
                context.config_path.to_owned(),
                "--port".to_owned(),
                context.port.unwrap_or(8779).to_string(),
                "--data-dir".to_owned(),
                context.data_dir.to_owned(),
            ],
        })
    }
}
