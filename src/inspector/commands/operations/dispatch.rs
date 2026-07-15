use std::sync::Arc;

use crate::modules::logos_core::{
    BoxedModuleEventSubscription, ModuleCall, ModuleCallFuture, ModuleCallStopReason,
    ModuleCallTerminated, ModuleCallTerminationEvidence, ModuleDiagnosticFuture, ModuleTransport,
    ModuleTransportClosed, ModuleTransportKind, ModuleTransportResult, SharedModuleTransport,
};
use crate::support::command_runner::{
    CommandCleanupUnconfirmed, CommandStopReason, CommandTerminated, CommandTerminationScope,
};
use anyhow::Result;

use super::spec::OperationCommand;
use super::{
    RuntimeOperationRegistry, RuntimeOperationRequest, blockchain, delivery, lez, local_nodes,
    outcome::RuntimeOperationOutcome,
    storage,
    supervisor::{
        OperationAdapterClosed, OperationCleanupUnconfirmed, OperationControl,
        OperationInterrupted, OperationStopReason, TerminationEvidence,
    },
    wallet,
};

struct ControlledModuleTransport {
    transport: SharedModuleTransport,
    control: OperationControl,
}

impl ModuleTransport for ControlledModuleTransport {
    fn kind(&self) -> ModuleTransportKind {
        self.transport.kind()
    }

    fn logoscore_cli_transport(
        &self,
    ) -> Option<&crate::modules::logos_core::LogoscoreCliTransport> {
        self.transport.logoscore_cli_transport()
    }

    fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
        self.transport
            .call_controlled(call, self.control.module_call_control())
    }

    fn subscribe_module_event(
        &self,
        module: &str,
        event: &str,
    ) -> ModuleTransportResult<BoxedModuleEventSubscription> {
        self.transport.subscribe_module_event(module, event)
    }

    fn ingest_module_event(
        &self,
        module: &str,
        event: &str,
        args: &[serde_json::Value],
    ) -> ModuleTransportResult<()> {
        self.transport.ingest_module_event(module, event, args)
    }

    fn supports_shared_file_staging(&self) -> bool {
        self.transport.supports_shared_file_staging()
    }

    fn native_runtime_module_events_ready(&self) -> bool {
        self.transport.native_runtime_module_events_ready()
    }

    fn status(&self) -> ModuleDiagnosticFuture<'_> {
        self.transport.status()
    }

    fn module_info(&self, module: String) -> ModuleDiagnosticFuture<'_> {
        self.transport.module_info(module)
    }
}

pub(super) async fn execute_runtime_operation(
    request: RuntimeOperationRequest,
    registry: &RuntimeOperationRegistry,
    operation_id: &super::identity::RuntimeOperationId,
    control: &OperationControl,
    module_transport: SharedModuleTransport,
) -> Result<RuntimeOperationOutcome> {
    let cleanup_module_transport = Arc::clone(&module_transport);
    let module_transport: SharedModuleTransport = Arc::new(ControlledModuleTransport {
        transport: module_transport,
        control: control.clone(),
    });
    let result = match request.command() {
        OperationCommand::Storage(command) => {
            storage::execute(
                command,
                &request,
                registry,
                operation_id,
                control,
                module_transport,
                cleanup_module_transport,
            )
            .await
        }
        OperationCommand::Delivery(command) => {
            delivery::execute(command, &request, module_transport)
                .await
                .map(Into::into)
        }
        OperationCommand::LocalNodes(command) => local_nodes::execute(command, &request, control)
            .await
            .map(RuntimeOperationOutcome::Completed),
        OperationCommand::Wallet(command) => wallet::execute(command, &request, control)
            .await
            .map(RuntimeOperationOutcome::Completed),
        OperationCommand::Blockchain(command) => {
            blockchain::execute(command, &request, module_transport)
                .await
                .map(RuntimeOperationOutcome::Completed)
        }
        OperationCommand::Execution(command) => lez::execute(command, &request, control)
            .await
            .map(RuntimeOperationOutcome::Completed),
    };
    normalize_execution_result(result)
}

pub(super) fn normalize_command_execution<T>(
    result: Result<T>,
    control: &OperationControl,
    process_evidence: TerminationEvidence,
    no_process_evidence: TerminationEvidence,
) -> Result<T> {
    result.map_err(|error| {
        if error.downcast_ref::<CommandCleanupUnconfirmed>().is_some() {
            return OperationCleanupUnconfirmed::new(error.to_string()).into();
        }
        let Some(terminated) = error.downcast_ref::<CommandTerminated>() else {
            return error;
        };
        let reason = match terminated.reason() {
            CommandStopReason::CancelRequested => control
                .stop_reason()
                .unwrap_or(OperationStopReason::CancelRequested),
            CommandStopReason::DeadlineExceeded => OperationStopReason::DeadlineExceeded,
        };
        let message = error.to_string();
        let evidence = if terminated.scope() == CommandTerminationScope::NoProcess {
            no_process_evidence
        } else {
            process_evidence
        };
        match evidence {
            TerminationEvidence::Confirmed => {
                OperationInterrupted::confirmed(reason, message).into()
            }
            TerminationEvidence::LocalOnly => {
                OperationInterrupted::local_only(reason, message).into()
            }
        }
    })
}

pub(super) async fn interruptible_remote<F, T>(
    control: &OperationControl,
    message: &'static str,
    future: F,
) -> Result<T>
where
    F: std::future::Future<Output = T>,
{
    tokio::select! {
        biased;
        value = future => Ok(value),
        () = control.cancellation().cancelled() => {
            let reason = control
                .stop_reason()
                .unwrap_or(OperationStopReason::CancelRequested);
            Err(OperationInterrupted::local_only(reason, message).into())
        },
        () = tokio::time::sleep_until(control.deadline()) => {
            Err(OperationInterrupted::local_only(
                OperationStopReason::DeadlineExceeded,
                message,
            ).into())
        },
    }
}

fn normalize_execution_result<T>(result: Result<T>) -> Result<T> {
    result.map_err(|error| {
        if error.downcast_ref::<CommandCleanupUnconfirmed>().is_some() {
            return OperationCleanupUnconfirmed::new(error.to_string()).into();
        }
        if let Some(terminated) = error.downcast_ref::<ModuleCallTerminated>() {
            let reason = match terminated.reason() {
                ModuleCallStopReason::CancelRequested => OperationStopReason::CancelRequested,
                ModuleCallStopReason::DeadlineExceeded => OperationStopReason::DeadlineExceeded,
                ModuleCallStopReason::Shutdown => OperationStopReason::Shutdown,
            };
            let message = error.to_string();
            return match terminated.evidence() {
                ModuleCallTerminationEvidence::NotStarted
                | ModuleCallTerminationEvidence::ProcessTerminated
                | ModuleCallTerminationEvidence::RemoteEffectTerminationConfirmed => {
                    OperationInterrupted::confirmed(reason, message).into()
                }
                ModuleCallTerminationEvidence::LocallyAbandoned => {
                    OperationInterrupted::local_only(reason, message).into()
                }
            };
        }
        if error.downcast_ref::<ModuleTransportClosed>().is_some() {
            return OperationAdapterClosed::new(error.to_string()).into();
        }
        error
    })
}

#[cfg(test)]
mod tests {
    use anyhow::{Context as _, Result};

    use super::*;

    #[test]
    fn confirmed_remote_module_effect_normalizes_as_confirmed_interruption() -> Result<()> {
        let error = normalize_execution_result::<()>(Err(ModuleCallTerminated::new(
            ModuleCallStopReason::CancelRequested,
            ModuleCallTerminationEvidence::RemoteEffectTerminationConfirmed,
        )
        .into()))
        .err()
        .context("confirmed remote termination should interrupt the operation")?;
        let interrupted = error
            .downcast_ref::<OperationInterrupted>()
            .context("confirmed remote termination lost operation interruption type")?;

        anyhow::ensure!(
            interrupted.reason() == OperationStopReason::CancelRequested
                && interrupted.evidence() == TerminationEvidence::Confirmed,
            "confirmed remote termination became local-only: {interrupted:?}"
        );
        Ok(())
    }
}
