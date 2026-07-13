use serde_json::Value;

use crate::source_routing::messaging_layer;

use super::{LocalNodeAdapter, NodeAction, NodeConfigContext, NodeKind, NodeLifecycle};

#[derive(Debug)]
pub(super) struct MessagingAdapter;

pub(super) static MESSAGING_ADAPTER: MessagingAdapter = MessagingAdapter;

impl LocalNodeAdapter for MessagingAdapter {
    fn kind(&self) -> NodeKind {
        NodeKind::Messaging
    }

    fn label(&self) -> &'static str {
        "Messaging"
    }

    fn default_port(&self) -> Option<u16> {
        Some(8645)
    }

    fn lifecycle(&self) -> NodeLifecycle {
        NodeLifecycle::InitializedModule(messaging_layer::managed_contract())
    }

    fn workflow_actions(&self) -> &'static [NodeAction] {
        &[
            NodeAction::Initialize,
            NodeAction::Start,
            NodeAction::Stop,
            NodeAction::Purge,
        ]
    }

    fn build_config(&self, context: NodeConfigContext<'_>) -> Value {
        messaging_layer::managed_config(context.port)
    }
}
