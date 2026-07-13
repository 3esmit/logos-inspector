use std::sync::atomic::AtomicBool;

use anyhow::Result;

use crate::modules::logos_core::SharedModuleTransport;

use super::spec::OperationCommand;
use super::{
    RuntimeOperationRegistry, RuntimeOperationRequest, blockchain, delivery, lez, local_nodes,
    outcome::RuntimeOperationOutcome, storage, wallet,
};

pub(super) async fn execute_runtime_operation(
    request: RuntimeOperationRequest,
    registry: &RuntimeOperationRegistry,
    operation_id: &super::identity::RuntimeOperationId,
    cancel_requested: &AtomicBool,
    module_transport: SharedModuleTransport,
) -> Result<RuntimeOperationOutcome> {
    match request.command() {
        OperationCommand::Storage(command) => {
            storage::execute(
                command,
                &request,
                registry,
                operation_id,
                cancel_requested,
                module_transport,
            )
            .await
        }
        OperationCommand::Delivery(command) => {
            delivery::execute(command, &request, module_transport)
                .await
                .map(Into::into)
        }
        OperationCommand::LocalNodes(command) => local_nodes::execute(command, &request)
            .await
            .map(RuntimeOperationOutcome::Completed),
        OperationCommand::Wallet(command) => wallet::execute(command, &request)
            .await
            .map(RuntimeOperationOutcome::Completed),
        OperationCommand::Blockchain(command) => {
            blockchain::execute(command, &request, module_transport)
                .await
                .map(RuntimeOperationOutcome::Completed)
        }
        OperationCommand::Execution(command) => lez::execute(command, &request)
            .await
            .map(RuntimeOperationOutcome::Completed),
    }
}
