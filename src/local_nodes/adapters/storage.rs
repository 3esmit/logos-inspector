use serde_json::Value;

use crate::source_routing::storage_layer;

use super::{LocalNodeAdapter, NodeAction, NodeConfigContext, NodeKind, NodeLifecycle};

#[derive(Debug)]
pub(super) struct StorageAdapter;

pub(super) static STORAGE_ADAPTER: StorageAdapter = StorageAdapter;

impl LocalNodeAdapter for StorageAdapter {
    fn kind(&self) -> NodeKind {
        NodeKind::Storage
    }

    fn label(&self) -> &'static str {
        "Storage"
    }

    fn default_port(&self) -> Option<u16> {
        None
    }

    fn endpoint(&self, _port: Option<u16>) -> Option<String> {
        None
    }

    fn lifecycle(&self) -> NodeLifecycle {
        NodeLifecycle::InitializedModule(storage_layer::managed_contract())
    }

    fn workflow_actions(&self) -> &'static [NodeAction] {
        &[
            NodeAction::Initialize,
            NodeAction::Start,
            NodeAction::Stop,
            NodeAction::Uninstall,
            NodeAction::Purge,
        ]
    }

    fn build_config(&self, context: NodeConfigContext<'_>) -> Value {
        storage_layer::managed_config(context.data_dir)
    }
}
