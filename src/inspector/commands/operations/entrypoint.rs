use anyhow::{Context as _, Result};
use serde_json::Value;

use super::spec::{OperationMethod, OperationRoute, operation_route};
use crate::source_routing::Args;

pub(crate) trait OperationRunner {
    fn start_from_value(&self, value: Value) -> Result<Value>;
    fn status(&self, operation_id: &str) -> Result<Value>;
    fn events(&self, operation_id: &str, after_seq: u64) -> Result<Value>;
    fn cancel(&self, operation_id: &str) -> Result<Value>;
    fn run_operation(&self, method: OperationMethod, args: Value, label: &str) -> Result<Value>;
    fn start_operation(&self, method: OperationMethod, args: Value, label: &str) -> Result<Value>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperationBridgeCommand {
    RuntimeOperationStart,
    RuntimeOperationStatus,
    RuntimeOperationEvents,
    RuntimeOperationCancel,
    StorageOperationStatus,
    StorageOperationCancel,
    Route(OperationRoute),
}

const OPERATION_CONTROL_METHODS: &[(&str, OperationBridgeCommand)] = &[
    (
        "nodeOperationStart",
        OperationBridgeCommand::RuntimeOperationStart,
    ),
    (
        "runtimeOperationStart",
        OperationBridgeCommand::RuntimeOperationStart,
    ),
    (
        "nodeOperationStatus",
        OperationBridgeCommand::RuntimeOperationStatus,
    ),
    (
        "runtimeOperationStatus",
        OperationBridgeCommand::RuntimeOperationStatus,
    ),
    (
        "nodeOperationEvents",
        OperationBridgeCommand::RuntimeOperationEvents,
    ),
    (
        "runtimeOperationEvents",
        OperationBridgeCommand::RuntimeOperationEvents,
    ),
    (
        "nodeOperationCancel",
        OperationBridgeCommand::RuntimeOperationCancel,
    ),
    (
        "runtimeOperationCancel",
        OperationBridgeCommand::RuntimeOperationCancel,
    ),
    (
        "storageOperationStatus",
        OperationBridgeCommand::StorageOperationStatus,
    ),
    (
        "storageOperationCancel",
        OperationBridgeCommand::StorageOperationCancel,
    ),
];

pub(crate) fn operation_bridge_command(method: &str) -> Option<OperationBridgeCommand> {
    OPERATION_CONTROL_METHODS
        .iter()
        .find(|(name, _)| *name == method)
        .map(|(_, command)| *command)
        .or_else(|| operation_route(method).map(OperationBridgeCommand::Route))
}

#[cfg(test)]
pub(crate) fn operation_bridge_command_names() -> impl Iterator<Item = &'static str> {
    OPERATION_CONTROL_METHODS
        .iter()
        .map(|(name, _)| *name)
        .chain(super::spec::operation_method_names())
}

pub(crate) fn handle_operation_command(
    runner: &impl OperationRunner,
    command: OperationBridgeCommand,
    args: &Value,
) -> Result<Value> {
    let value = match command {
        OperationBridgeCommand::RuntimeOperationStart => runtime_operation_start(runner, args)?,
        OperationBridgeCommand::RuntimeOperationStatus => runtime_operation_status(runner, args)?,
        OperationBridgeCommand::RuntimeOperationEvents => runtime_operation_events(runner, args)?,
        OperationBridgeCommand::RuntimeOperationCancel => runtime_operation_cancel(runner, args)?,
        OperationBridgeCommand::StorageOperationStatus => storage_operation_status(runner, args)?,
        OperationBridgeCommand::StorageOperationCancel => storage_operation_cancel(runner, args)?,
        OperationBridgeCommand::Route(route) => {
            if route.start_async {
                runner.start_operation(route.method, args.clone(), route.label)?
            } else {
                runner.run_operation(route.method, args.clone(), route.label)?
            }
        }
    };
    Ok(value)
}

fn runtime_operation_start(runner: &impl OperationRunner, args: &Value) -> Result<Value> {
    let args = Args::new(args.clone())?;
    runner.start_from_value(
        args.value(0)
            .cloned()
            .context("runtime operation request is required")?,
    )
}

fn runtime_operation_status(runner: &impl OperationRunner, args: &Value) -> Result<Value> {
    let args = Args::new(args.clone())?;
    runner.status(args.string(0, "runtime operation id")?)
}

fn runtime_operation_events(runner: &impl OperationRunner, args: &Value) -> Result<Value> {
    let args = Args::new(args.clone())?;
    let operation_id = args.string(0, "runtime operation id")?;
    let after_seq = args.value(1).and_then(Value::as_u64).unwrap_or(0);
    runner.events(operation_id, after_seq)
}

fn runtime_operation_cancel(runner: &impl OperationRunner, args: &Value) -> Result<Value> {
    let args = Args::new(args.clone())?;
    runner.cancel(args.string(0, "runtime operation id")?)
}

fn storage_operation_status(runner: &impl OperationRunner, args: &Value) -> Result<Value> {
    let args = Args::new(args.clone())?;
    runner.status(args.string(0, "storage operation id")?)
}

fn storage_operation_cancel(runner: &impl OperationRunner, args: &Value) -> Result<Value> {
    let args = Args::new(args.clone())?;
    runner.cancel(args.string(0, "storage operation id")?)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use anyhow::{Context as _, Result, bail};
    use serde_json::json;

    use super::*;

    #[derive(Debug, PartialEq)]
    enum RunnerCall {
        StartFromValue(Value),
        Status(String),
        Events(String, u64),
        Cancel(String),
        RunOperation {
            domain: String,
            method: String,
            args: Value,
            label: String,
        },
        StartOperation {
            domain: String,
            method: String,
            args: Value,
            label: String,
        },
    }

    #[derive(Default)]
    struct FakeRunner {
        calls: RefCell<Vec<RunnerCall>>,
    }

    impl FakeRunner {
        fn calls(&self) -> Vec<RunnerCall> {
            self.calls.take()
        }
    }

    impl OperationRunner for FakeRunner {
        fn start_from_value(&self, value: Value) -> Result<Value> {
            self.calls
                .borrow_mut()
                .push(RunnerCall::StartFromValue(value));
            Ok(json!({ "operationId": "started" }))
        }

        fn status(&self, operation_id: &str) -> Result<Value> {
            self.calls
                .borrow_mut()
                .push(RunnerCall::Status(operation_id.to_owned()));
            Ok(json!({ "operationId": operation_id, "status": "running" }))
        }

        fn events(&self, operation_id: &str, after_seq: u64) -> Result<Value> {
            self.calls
                .borrow_mut()
                .push(RunnerCall::Events(operation_id.to_owned(), after_seq));
            Ok(json!({ "operation": { "operationId": operation_id }, "nextSeq": after_seq }))
        }

        fn cancel(&self, operation_id: &str) -> Result<Value> {
            self.calls
                .borrow_mut()
                .push(RunnerCall::Cancel(operation_id.to_owned()));
            Ok(json!({ "operationId": operation_id, "status": "canceling" }))
        }

        fn run_operation(
            &self,
            method: OperationMethod,
            args: Value,
            label: &str,
        ) -> Result<Value> {
            self.calls.borrow_mut().push(RunnerCall::RunOperation {
                domain: method.domain().as_str().to_owned(),
                method: method.as_str().to_owned(),
                args,
                label: label.to_owned(),
            });
            Ok(json!({ "operation": method.as_str() }))
        }

        fn start_operation(
            &self,
            method: OperationMethod,
            args: Value,
            label: &str,
        ) -> Result<Value> {
            self.calls.borrow_mut().push(RunnerCall::StartOperation {
                domain: method.domain().as_str().to_owned(),
                method: method.as_str().to_owned(),
                args,
                label: label.to_owned(),
            });
            Ok(json!({ "operationId": method.as_str() }))
        }
    }

    #[test]
    fn handle_operation_command_routes_runtime_operation_start_request() -> Result<()> {
        let runner = FakeRunner::default();
        let request = json!({
            "domain": "delivery",
            "method": "deliverySend",
            "args": ["rest", "http://127.0.0.1:8645", true, "/topic", "hello"]
        });

        let command = operation_bridge_command("nodeOperationStart")
            .context("runtime operation start command")?;
        let value = handle_operation_command(&runner, command, &json!([request]))?;

        if value != json!({ "operationId": "started" }) {
            bail!("unexpected response: {value:?}");
        }
        if runner.calls()
            != vec![RunnerCall::StartFromValue(json!({
                "domain": "delivery",
                "method": "deliverySend",
                "args": ["rest", "http://127.0.0.1:8645", true, "/topic", "hello"]
            }))]
        {
            bail!("unexpected runner calls");
        }
        Ok(())
    }

    #[test]
    fn handle_operation_command_routes_storage_download_call() -> Result<()> {
        let runner = FakeRunner::default();
        let args = json!([
            "rest",
            "http://127.0.0.1:8080/api/storage/v1",
            true,
            "cid-a"
        ]);

        let command =
            operation_bridge_command("storageDownloadToUrl").context("storage download command")?;
        let value = handle_operation_command(&runner, command, &args)?;

        if value != json!({ "operation": "storageDownloadToUrl" }) {
            bail!("unexpected response: {value:?}");
        }
        if runner.calls()
            != vec![RunnerCall::RunOperation {
                domain: "storage".to_owned(),
                method: "storageDownloadToUrl".to_owned(),
                args,
                label: "Storage download".to_owned(),
            }]
        {
            bail!("unexpected runner calls");
        }
        Ok(())
    }

    #[test]
    fn handle_operation_command_routes_storage_cancel_alias() -> Result<()> {
        let runner = FakeRunner::default();

        let command =
            operation_bridge_command("storageOperationCancel").context("storage cancel command")?;
        let value = handle_operation_command(&runner, command, &json!(["op-1"]))?;

        if value != json!({ "operationId": "op-1", "status": "canceling" }) {
            bail!("unexpected response: {value:?}");
        }
        if runner.calls() != vec![RunnerCall::Cancel("op-1".to_owned())] {
            bail!("unexpected runner calls");
        }
        Ok(())
    }

    #[test]
    fn handle_operation_command_routes_wallet_accounts_through_runner() -> Result<()> {
        let runner = FakeRunner::default();
        let args = json!([{ "network_profile": "local" }]);

        let command =
            operation_bridge_command("localWalletAccounts").context("wallet accounts command")?;
        let value = handle_operation_command(&runner, command, &args)?;

        if value != json!({ "operation": "localWalletAccounts" }) {
            bail!("unexpected response: {value:?}");
        }
        if runner.calls()
            != vec![RunnerCall::RunOperation {
                domain: "wallet".to_owned(),
                method: "localWalletAccounts".to_owned(),
                args,
                label: "Wallet accounts".to_owned(),
            }]
        {
            bail!("unexpected runner calls");
        }
        Ok(())
    }

    #[test]
    fn handle_operation_command_returns_none_for_non_operation_method() -> Result<()> {
        let runner = FakeRunner::default();

        let value = operation_bridge_command("storageExists");

        if value.is_some() {
            bail!("expected non-operation method to be ignored");
        }
        if !runner.calls().is_empty() {
            bail!("unexpected runner calls");
        }
        Ok(())
    }
}
