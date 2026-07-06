use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::bridge::Args;

pub(crate) trait OperationRunner {
    fn start_from_value(&self, value: Value) -> Result<Value>;
    fn status(&self, operation_id: &str) -> Result<Value>;
    fn events(&self, operation_id: &str, after_seq: u64) -> Result<Value>;
    fn cancel(&self, operation_id: &str) -> Result<Value>;
    fn run_legacy(&self, domain: &str, method: &str, args: Value, label: &str) -> Result<Value>;
    fn start_legacy(&self, domain: &str, method: &str, args: Value, label: &str) -> Result<Value>;
}

pub(crate) fn handle_operation_command(
    runner: &impl OperationRunner,
    method: &str,
    args: &Value,
) -> Result<Option<Value>> {
    let value = match method {
        "nodeOperationStart" => node_operation_start(runner, args)?,
        "nodeOperationStatus" => node_operation_status(runner, args)?,
        "nodeOperationEvents" => node_operation_events(runner, args)?,
        "nodeOperationCancel" => node_operation_cancel(runner, args)?,
        "storageDownloadStart" => runner.start_legacy(
            "storage",
            "storageDownloadToUrl",
            args.clone(),
            "Storage download",
        )?,
        "storageOperationStatus" => storage_operation_status(runner, args)?,
        "storageOperationCancel" => storage_operation_cancel(runner, args)?,
        "localWalletCreateAccount" => runner.run_legacy(
            "wallet",
            "localWalletCreateAccount",
            args.clone(),
            "Wallet account",
        )?,
        "localWalletSendTransaction" => runner.run_legacy(
            "wallet",
            "localWalletSendTransaction",
            args.clone(),
            "Wallet send",
        )?,
        "localWalletInstructionSubmit" => runner.run_legacy(
            "wallet",
            "localWalletInstructionSubmit",
            args.clone(),
            "IDL instruction",
        )?,
        "localWalletCommand" => runner.run_legacy(
            "wallet",
            "localWalletCommand",
            args.clone(),
            "Wallet command",
        )?,
        "localWalletDeployProgram" => runner.run_legacy(
            "wallet",
            "localWalletDeployProgram",
            args.clone(),
            "Program deploy",
        )?,
        "localWalletSyncPrivate" => runner.run_legacy(
            "wallet",
            "localWalletSyncPrivate",
            args.clone(),
            "Private sync",
        )?,
        "localWalletAccounts" => runner.run_legacy(
            "wallet",
            "localWalletAccounts",
            args.clone(),
            "Wallet accounts",
        )?,
        "localNodesAction" => runner.run_legacy(
            "localNodes",
            "localNodesAction",
            args.clone(),
            "Local node action",
        )?,
        "storageManifests" => runner.run_legacy(
            "storage",
            "storageManifests",
            args.clone(),
            "Storage manifests",
        )?,
        "storageDownloadManifest" => runner.run_legacy(
            "storage",
            "storageDownloadManifest",
            args.clone(),
            "Storage manifest",
        )?,
        "storageFetch" => {
            runner.run_legacy("storage", "storageFetch", args.clone(), "Storage fetch")?
        }
        "storageUploadUrl" => runner.run_legacy(
            "storage",
            "storageUploadUrl",
            args.clone(),
            "Storage upload",
        )?,
        "storageDownloadToUrl" => runner.run_legacy(
            "storage",
            "storageDownloadToUrl",
            args.clone(),
            "Storage download",
        )?,
        "storageRemove" => {
            runner.run_legacy("storage", "storageRemove", args.clone(), "Storage remove")?
        }
        "deliveryCreateNode" => runner.run_legacy(
            "delivery",
            "deliveryCreateNode",
            args.clone(),
            "Delivery create node",
        )?,
        "deliveryStart" => {
            runner.run_legacy("delivery", "deliveryStart", args.clone(), "Delivery start")?
        }
        "deliveryStop" => {
            runner.run_legacy("delivery", "deliveryStop", args.clone(), "Delivery stop")?
        }
        "deliverySubscribe" => runner.run_legacy(
            "delivery",
            "deliverySubscribe",
            args.clone(),
            "Delivery subscribe",
        )?,
        "deliveryUnsubscribe" => runner.run_legacy(
            "delivery",
            "deliveryUnsubscribe",
            args.clone(),
            "Delivery unsubscribe",
        )?,
        "deliverySend" => {
            runner.run_legacy("delivery", "deliverySend", args.clone(), "Delivery send")?
        }
        "deliveryStoreQuery" => runner.run_legacy(
            "delivery",
            "deliveryStoreQuery",
            args.clone(),
            "Delivery store query",
        )?,
        _ => return Ok(None),
    };
    Ok(Some(value))
}

fn node_operation_start(runner: &impl OperationRunner, args: &Value) -> Result<Value> {
    let args = Args::new(args.clone())?;
    runner.start_from_value(
        args.value(0)
            .cloned()
            .context("node operation request is required")?,
    )
}

fn node_operation_status(runner: &impl OperationRunner, args: &Value) -> Result<Value> {
    let args = Args::new(args.clone())?;
    runner.status(args.string(0, "node operation id")?)
}

fn node_operation_events(runner: &impl OperationRunner, args: &Value) -> Result<Value> {
    let args = Args::new(args.clone())?;
    let operation_id = args.string(0, "node operation id")?;
    let after_seq = args.value(1).and_then(Value::as_u64).unwrap_or(0);
    runner.events(operation_id, after_seq)
}

fn node_operation_cancel(runner: &impl OperationRunner, args: &Value) -> Result<Value> {
    let args = Args::new(args.clone())?;
    runner.cancel(args.string(0, "node operation id")?)
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

    use anyhow::{Result, bail};
    use serde_json::json;

    use super::*;

    #[derive(Debug, PartialEq)]
    enum RunnerCall {
        StartFromValue(Value),
        Status(String),
        Events(String, u64),
        Cancel(String),
        RunLegacy {
            domain: String,
            method: String,
            args: Value,
            label: String,
        },
        StartLegacy {
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

        fn run_legacy(
            &self,
            domain: &str,
            method: &str,
            args: Value,
            label: &str,
        ) -> Result<Value> {
            self.calls.borrow_mut().push(RunnerCall::RunLegacy {
                domain: domain.to_owned(),
                method: method.to_owned(),
                args,
                label: label.to_owned(),
            });
            Ok(json!({ "legacy": method }))
        }

        fn start_legacy(
            &self,
            domain: &str,
            method: &str,
            args: Value,
            label: &str,
        ) -> Result<Value> {
            self.calls.borrow_mut().push(RunnerCall::StartLegacy {
                domain: domain.to_owned(),
                method: method.to_owned(),
                args,
                label: label.to_owned(),
            });
            Ok(json!({ "operationId": method }))
        }
    }

    #[test]
    fn handle_operation_command_routes_node_operation_start_request() -> Result<()> {
        let runner = FakeRunner::default();
        let request = json!({
            "domain": "delivery",
            "method": "deliverySend",
            "args": ["rest", "http://127.0.0.1:8645", true, "/topic", "hello"]
        });

        let value = handle_operation_command(&runner, "nodeOperationStart", &json!([request]))?;

        if value != Some(json!({ "operationId": "started" })) {
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
    fn handle_operation_command_routes_storage_download_legacy_call() -> Result<()> {
        let runner = FakeRunner::default();
        let args = json!([
            "rest",
            "http://127.0.0.1:8080/api/storage/v1",
            true,
            "cid-a"
        ]);

        let value = handle_operation_command(&runner, "storageDownloadToUrl", &args)?;

        if value != Some(json!({ "legacy": "storageDownloadToUrl" })) {
            bail!("unexpected response: {value:?}");
        }
        if runner.calls()
            != vec![RunnerCall::RunLegacy {
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

        let value = handle_operation_command(&runner, "storageOperationCancel", &json!(["op-1"]))?;

        if value != Some(json!({ "operationId": "op-1", "status": "canceling" })) {
            bail!("unexpected response: {value:?}");
        }
        if runner.calls() != vec![RunnerCall::Cancel("op-1".to_owned())] {
            bail!("unexpected runner calls");
        }
        Ok(())
    }

    #[test]
    fn handle_operation_command_routes_wallet_accounts_through_legacy_runner() -> Result<()> {
        let runner = FakeRunner::default();
        let args = json!([{ "network_profile": "local" }]);

        let value = handle_operation_command(&runner, "localWalletAccounts", &args)?;

        if value != Some(json!({ "legacy": "localWalletAccounts" })) {
            bail!("unexpected response: {value:?}");
        }
        if runner.calls()
            != vec![RunnerCall::RunLegacy {
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

        let value = handle_operation_command(&runner, "storageExists", &json!(["rest", "cid"]))?;

        if value.is_some() {
            bail!("expected non-operation method to be ignored");
        }
        if !runner.calls().is_empty() {
            bail!("unexpected runner calls");
        }
        Ok(())
    }
}
