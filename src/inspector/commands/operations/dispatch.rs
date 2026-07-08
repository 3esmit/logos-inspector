use std::sync::atomic::AtomicBool;

use anyhow::Result;
use serde_json::Value;

use super::{
    OperationDomain, RuntimeOperationRegistry, RuntimeOperationRequest, blockchain, delivery, lez,
    local_nodes, storage, wallet,
};

pub(super) async fn execute_runtime_operation(
    request: RuntimeOperationRequest,
    registry: &RuntimeOperationRegistry,
    operation_id: &str,
    cancel_requested: &AtomicBool,
) -> Result<Value> {
    match request.method().domain() {
        OperationDomain::Storage => {
            storage::execute(&request, registry, operation_id, cancel_requested).await
        }
        OperationDomain::Delivery => delivery::execute(&request).await,
        OperationDomain::LocalNodes => local_nodes::execute(&request).await,
        OperationDomain::Wallet => wallet::execute(&request).await,
        OperationDomain::Blockchain => blockchain::execute(&request).await,
        OperationDomain::Execution | OperationDomain::Indexer => lez::execute(&request).await,
    }
}
