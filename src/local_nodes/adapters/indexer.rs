use serde_json::Value;

use crate::source_routing::execution_zone_layer;

use super::{LocalNodeAdapter, NodeAction, NodeConfigContext, NodeKind, NodeLifecycle};

const UNAVAILABLE_REASON: &str = "no verified logoscore module lifecycle contract";

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
        Some(8779)
    }

    fn lifecycle(&self) -> NodeLifecycle {
        NodeLifecycle::Unavailable {
            reason: UNAVAILABLE_REASON,
        }
    }

    fn workflow_actions(&self) -> &'static [NodeAction] {
        &[]
    }

    fn build_config(&self, context: NodeConfigContext<'_>) -> Value {
        execution_zone_layer::managed_indexer_config(
            context.network_id,
            context.data_dir,
            context.endpoint,
            context.port,
        )
    }
}
