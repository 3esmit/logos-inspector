use serde_json::Value;

use crate::source_routing::execution_zone_layer;

use super::{LocalNodeAdapter, NodeAction, NodeConfigContext, NodeKind, NodeLifecycle};

#[derive(Debug)]
pub(super) struct SequencerAdapter;

pub(super) static SEQUENCER_ADAPTER: SequencerAdapter = SequencerAdapter;

impl LocalNodeAdapter for SequencerAdapter {
    fn kind(&self) -> NodeKind {
        NodeKind::Sequencer
    }

    fn label(&self) -> &'static str {
        "Local Sequencer"
    }

    fn default_port(&self) -> Option<u16> {
        Some(3040)
    }

    fn lifecycle(&self) -> NodeLifecycle {
        NodeLifecycle::RegisteredProcess {
            program: execution_zone_layer::managed_sequencer_program(),
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

    fn available_in_profile(&self, profile: &str) -> bool {
        profile == "local"
    }

    fn build_config(&self, context: NodeConfigContext<'_>) -> Value {
        execution_zone_layer::managed_sequencer_config(
            context.network_id,
            context.data_dir,
            context.endpoint,
            context.port,
        )
    }
}
