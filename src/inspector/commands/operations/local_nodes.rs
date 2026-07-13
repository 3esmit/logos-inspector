use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::{LocalNodeActionRequest, local_nodes_action, support::args::Args};

use super::super::value::{blocking_value, to_value};
use super::RuntimeOperationRequest;
use super::spec::{OperationClass, OperationCommand, OperationDefinition, OperationMethod};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LocalNodesCommand {
    Action,
}

impl LocalNodesCommand {
    pub(super) const fn method(self) -> OperationMethod {
        match self {
            Self::Action => OperationMethod::LocalNodesAction,
        }
    }
}

pub(super) const OPERATION_DEFINITIONS: &[OperationDefinition] = &[OperationDefinition::new(
    OperationCommand::LocalNodes(LocalNodesCommand::Action),
    "localNodesAction",
    "Local node action",
    OperationClass::Lifecycle,
)];

pub(super) async fn execute(
    command: LocalNodesCommand,
    request: &RuntimeOperationRequest,
) -> Result<Value> {
    match command {
        LocalNodesCommand::Action => execute_local_nodes_action(request).await,
    }
}

pub(super) async fn execute_local_nodes_action(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let action_request = serde_json::from_value::<LocalNodeActionRequest>(
        args.value(1)
            .cloned()
            .context("local node action request is required")?,
    )
    .context("failed to parse local node action request")?;
    let profile = args.optional_string(0).unwrap_or("default").to_owned();
    let confirmation = args.optional_string(2).map(ToOwned::to_owned);
    blocking_value("local node action", move || {
        to_value(local_nodes_action(
            &profile,
            action_request,
            confirmation.as_deref(),
        )?)
    })
    .await
}
