use std::sync::atomic::AtomicBool;

use anyhow::Result;
use serde_json::Value;

use super::spec::OperationExecutor;
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
    match request.method().executor() {
        OperationExecutor::Storage => {
            storage::execute(&request, registry, operation_id, cancel_requested).await
        }
        OperationExecutor::Delivery => delivery::execute(&request).await,
        OperationExecutor::LocalNodes => local_nodes::execute(&request).await,
        OperationExecutor::Wallet => wallet::execute(&request).await,
        OperationExecutor::Blockchain => blockchain::execute(&request).await,
        OperationExecutor::Lez => lez::execute(&request).await,
    }
}
