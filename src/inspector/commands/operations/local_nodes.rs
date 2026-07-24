use std::time::Instant;

use anyhow::{Context as _, Result};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::{
    LocalNodeActionRequest,
    local_nodes::{
        ChannelIndexerActionRequest, INDEXER_PACKAGE_INSTALL_TIMEOUT, LocalNodePackageCommit,
        basecamp_local_nodes_action, channel_indexer_action_controlled,
        local_nodes_action_controlled,
    },
    modules::logos_core::{ModuleTransportKind, SharedModuleTransport},
    support::{args::Args, command_runner::CommandControl},
};

use super::super::value::{blocking_value, to_value};
use super::RuntimeOperationRequest;
use super::dispatch::normalize_command_execution;
use super::spec::{OperationClass, OperationCommand, OperationDefinition, OperationMethod};
use super::supervisor::{OperationControl, TerminationEvidence};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LocalNodesCommand {
    Action,
    ChannelIndexerAction,
}

impl LocalNodesCommand {
    pub(super) const fn method(self) -> OperationMethod {
        match self {
            Self::Action => OperationMethod::LocalNodesAction,
            Self::ChannelIndexerAction => OperationMethod::ChannelIndexerAction,
        }
    }
}

pub(super) const OPERATION_DEFINITIONS: &[OperationDefinition] = &[
    OperationDefinition::new(
        OperationCommand::LocalNodes(LocalNodesCommand::Action),
        "localNodesAction",
        "Local node action",
        OperationClass::Lifecycle,
    ),
    OperationDefinition::new(
        OperationCommand::LocalNodes(LocalNodesCommand::ChannelIndexerAction),
        "channelIndexerAction",
        "Channel Indexer action",
        OperationClass::Lifecycle,
    ),
];

pub(super) async fn execute(
    command: LocalNodesCommand,
    request: &RuntimeOperationRequest,
    control: &OperationControl,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    match command {
        LocalNodesCommand::Action => {
            execute_local_nodes_action(request, control, module_transport).await
        }
        LocalNodesCommand::ChannelIndexerAction => {
            execute_channel_indexer_action(request, control).await
        }
    }
}

async fn execute_channel_indexer_action(
    request: &RuntimeOperationRequest,
    control: &OperationControl,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let action_request = serde_json::from_value::<ChannelIndexerActionRequest>(
        args.value(1)
            .cloned()
            .context("Channel Indexer action request is required")?,
    )
    .context("failed to parse Channel Indexer action request")?;
    let profile = args.optional_string(0).unwrap_or("default").to_owned();
    let confirmation = args.optional_string(2).map(ToOwned::to_owned);
    let command_control = command_control(control);
    let worker_guard = control.blocking_worker_guard()?;
    let result = blocking_value("Channel Indexer action", move || {
        let _worker_guard = worker_guard;
        to_value(channel_indexer_action_controlled(
            &profile,
            action_request,
            confirmation.as_deref(),
            command_control,
        )?)
    })
    .await;
    normalize_command_execution(
        result,
        control,
        TerminationEvidence::LocalOnly,
        TerminationEvidence::LocalOnly,
    )
}

pub(super) async fn execute_local_nodes_action(
    request: &RuntimeOperationRequest,
    control: &OperationControl,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let action_request = serde_json::from_value::<LocalNodeActionRequest>(
        args.value(1)
            .cloned()
            .context("local node action request is required")?,
    )
    .context("failed to parse local node action request")?;
    let profile = args.optional_string(0).unwrap_or("default").to_owned();
    let confirmation = args.optional_string(2).map(ToOwned::to_owned);
    if module_transport.kind() == ModuleTransportKind::Module {
        return to_value(
            basecamp_local_nodes_action(
                &profile,
                action_request,
                confirmation.as_deref(),
                &module_transport,
            )
            .await?,
        );
    }
    let command_control = command_control(control);
    let package_commit = package_install_commit(control, &command_control);
    let worker_guard = control.blocking_worker_guard()?;
    let result = blocking_value("local node action", move || {
        let _worker_guard = worker_guard;
        to_value(local_nodes_action_controlled(
            &profile,
            action_request,
            confirmation.as_deref(),
            command_control,
            package_commit,
        )?)
    })
    .await;
    normalize_command_execution(
        result,
        control,
        TerminationEvidence::LocalOnly,
        TerminationEvidence::LocalOnly,
    )
}

fn command_control(control: &OperationControl) -> CommandControl {
    control.command_control()
}

fn package_install_commit(
    control: &OperationControl,
    command_control: &CommandControl,
) -> LocalNodePackageCommit {
    let operation_control = control.clone();
    let command_budget = command_control.command_budget();
    LocalNodePackageCommit::new(move || {
        let lease = operation_control.begin_non_cancellable_commit()?;
        let deadline = Instant::now()
            .checked_add(INDEXER_PACKAGE_INSTALL_TIMEOUT)
            .context("Indexer package commit deadline overflow")?;
        let mut control = CommandControl::new(CancellationToken::new(), deadline)
            .with_blocking_work_tracker(operation_control.blocking_work_tracker());
        if let Some(command_budget) = command_budget {
            control = control.with_command_budget(command_budget);
        }
        Ok((control, lease))
    })
}

pub(super) fn is_indexer_package_install(args: &Value) -> bool {
    args.as_array()
        .and_then(|values| values.get(1))
        .and_then(Value::as_object)
        .is_some_and(|request| {
            request.get("action").and_then(Value::as_str) == Some("install")
                && request.get("node").and_then(Value::as_str) == Some("indexer")
        })
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use serde_json::json;

    use super::*;
    use crate::inspector::commands::operations::supervisor::test_operation_control;

    #[test]
    fn indexer_package_commit_ignores_outer_cancellation_and_holds_lease() -> Result<()> {
        let control = test_operation_control(std::time::Duration::from_secs(30));
        let outer_command = control.command_control();
        let mut package_commit = package_install_commit(&control, &outer_command);

        let commit_command = package_commit.begin_for_test()?;
        anyhow::ensure!(control.commit_is_active());
        anyhow::ensure!(commit_command.shares_command_budget_with(&outer_command));
        control.cancellation().cancel();
        anyhow::ensure!(outer_command.check_active().is_err());
        commit_command.check_active()?;

        drop(package_commit);
        anyhow::ensure!(!control.commit_is_active());
        Ok(())
    }

    #[test]
    fn indexer_package_install_detection_requires_node_and_action() {
        assert!(is_indexer_package_install(&json!([
            "default",
            { "action": "install", "node": "indexer" }
        ])));
        assert!(!is_indexer_package_install(&json!([
            "default",
            { "action": "start", "node": "indexer" }
        ])));
        assert!(!is_indexer_package_install(&json!([
            "default",
            { "action": "install", "node": "storage" }
        ])));
    }
}
