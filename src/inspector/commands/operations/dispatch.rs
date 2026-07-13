use std::sync::atomic::AtomicBool;

use anyhow::Result;
use serde_json::Value;

use super::spec::OperationCommand;
use super::{
    RuntimeOperationRegistry, RuntimeOperationRequest, blockchain, delivery, lez, local_nodes,
    storage, wallet,
};

pub(super) async fn execute_runtime_operation(
    request: RuntimeOperationRequest,
    registry: &RuntimeOperationRegistry,
    operation_id: &str,
    cancel_requested: &AtomicBool,
) -> Result<Value> {
    match request.command() {
        OperationCommand::Storage(command) => {
            storage::execute(command, &request, registry, operation_id, cancel_requested).await
        }
        OperationCommand::Delivery(command) => delivery::execute(command, &request).await,
        OperationCommand::LocalNodes(command) => local_nodes::execute(command, &request).await,
        OperationCommand::Wallet(command) => wallet::execute(command, &request).await,
        OperationCommand::Blockchain(command) => blockchain::execute(command, &request).await,
        OperationCommand::Execution(command) => lez::execute(command, &request).await,
    }
}
