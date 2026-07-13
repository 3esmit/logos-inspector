use serde_json::Value;

use crate::source_routing::bedrock_layer;

use super::{LocalNodeAdapter, NodeAction, NodeConfigContext, NodeKind, NodeLifecycle};

#[derive(Debug)]
pub(super) struct BedrockAdapter;

pub(super) static BEDROCK_ADAPTER: BedrockAdapter = BedrockAdapter;

impl LocalNodeAdapter for BedrockAdapter {
    fn kind(&self) -> NodeKind {
        NodeKind::Bedrock
    }

    fn label(&self) -> &'static str {
        "Bedrock"
    }

    fn default_port(&self) -> Option<u16> {
        Some(8080)
    }

    fn lifecycle(&self) -> NodeLifecycle {
        NodeLifecycle::RuntimeOwnedModule(bedrock_layer::managed_contract())
    }

    fn workflow_actions(&self) -> &'static [NodeAction] {
        &[NodeAction::Start, NodeAction::Stop, NodeAction::Purge]
    }

    fn build_config(&self, context: NodeConfigContext<'_>) -> Value {
        bedrock_layer::managed_config(
            context.network_id,
            context.data_dir,
            context.endpoint,
            context.port,
        )
    }
}
