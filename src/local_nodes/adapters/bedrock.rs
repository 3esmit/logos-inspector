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
        NodeLifecycle::InitializedModule(bedrock_layer::managed_contract())
    }

    fn workflow_actions(&self) -> &'static [NodeAction] {
        &[
            NodeAction::Initialize,
            NodeAction::Start,
            NodeAction::Stop,
            NodeAction::Purge,
        ]
    }

    fn preserve_generated_config_on_runtime_reset(&self) -> bool {
        true
    }

    fn ensure_loaded_before_start(&self) -> bool {
        true
    }

    fn build_config(&self, context: NodeConfigContext<'_>) -> Value {
        bedrock_layer::managed_config(
            context.network_id,
            context.data_dir,
            context.endpoint,
            context.port,
            context.config_path,
            context.public_testnet,
        )
    }
}
