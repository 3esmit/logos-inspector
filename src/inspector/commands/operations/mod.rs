use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64},
    },
    thread,
    time::Duration,
};

use anyhow::{Context as _, Result, bail};
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{
    modules::logos_core::{LogoscoreCliTransport, SharedModuleTransport},
    source_routing::ModuleEventEnvelope,
    support::time::now_millis,
};

mod backup_import;
mod blockchain;
mod delivery;
mod dispatch;
mod entrypoint;
mod identity;
mod lez;
mod local_nodes;
mod outcome;
mod policy;
mod record;
mod request;
mod spec;
mod storage;
mod transition;
mod wallet;
mod wallet_args;

use backup_import::{BackupImportCoordinator, LocalBackupImportStore};
use dispatch::execute_runtime_operation;
#[cfg(test)]
pub(crate) use entrypoint::operation_bridge_command_names;
pub(crate) use entrypoint::{OperationBridgeCommand, operation_bridge_command};
use entrypoint::{OperationRunner, handle_operation_command};
use identity::{EventCursor, RuntimeOperationId, allocate_sequence};
use record::{RuntimeOperationRegistry, RuntimeOperationStatus, running_runtime_operation_record};
pub(crate) use request::{RuntimeOperationRequest, runtime_operation_request_from_value};
pub(crate) use spec::OperationMethod;
use spec::normalized_operation_method;
use transition::RuntimeOperationTransition;

pub(crate) struct RuntimeOperations {
    registry: RuntimeOperationRegistry,
    next_operation_id: AtomicU64,
    backup_import: BackupImportCoordinator,
    module_transport: SharedModuleTransport,
}

impl Default for RuntimeOperations {
    fn default() -> Self {
        Self {
            registry: RuntimeOperationRegistry::default(),
            next_operation_id: AtomicU64::new(1),
            backup_import: BackupImportCoordinator::new(Arc::new(LocalBackupImportStore)),
            module_transport: Arc::new(LogoscoreCliTransport::default()),
        }
    }
}

impl RuntimeOperations {
    pub(crate) fn start_from_value(&self, runtime: &Runtime, value: Value) -> Result<Value> {
        let request = runtime_operation_request_from_value(value)?;
        self.start(runtime, request)
    }

    pub(crate) fn start(
        &self,
        runtime: &Runtime,
        request: RuntimeOperationRequest,
    ) -> Result<Value> {
        let operation_permit = self.backup_import.operation_permit(&request)?;
        let operation_id = RuntimeOperationId::allocated(
            request.domain_name(),
            &normalized_operation_method(request.method_name()),
            allocate_sequence(&self.next_operation_id)?,
        );
        let now = now_millis();
        let cancel_requested = Arc::new(AtomicBool::new(false));
        let record = running_runtime_operation_record(
            operation_id.clone(),
            &request,
            Arc::clone(&cancel_requested),
            now,
        )?;
        self.registry.insert(record)?;
        drop(operation_permit);
        self.registry
            .transition(&operation_id, RuntimeOperationTransition::Started)?;

        let registry = self.registry.clone();
        let task_operation_id = operation_id.clone();
        let module_transport = Arc::clone(&self.module_transport);
        let _detached_task = runtime.spawn(async move {
            let result = execute_runtime_operation(
                request,
                &registry,
                &task_operation_id,
                &cancel_requested,
                module_transport,
            )
            .await;
            registry.transition(
                &task_operation_id,
                RuntimeOperationTransition::Resolved(result.map_err(|error| error.to_string())),
            )
        });

        self.registry.value(&operation_id)
    }

    pub(crate) fn status(&self, operation_id: &str) -> Result<Value> {
        self.value(operation_id)
    }

    fn preview_backup_import(
        &self,
        backup_catalog_id: &str,
        wallet_profile: Option<&Value>,
        options: Option<&Value>,
    ) -> Result<Value> {
        self.backup_import
            .preview(self, backup_catalog_id, wallet_profile, options)
    }

    fn apply_backup_import(
        &self,
        runtime: &Runtime,
        backup_catalog_id: &str,
        wallet_profile: Option<&Value>,
        options: Option<&Value>,
    ) -> Result<Value> {
        self.backup_import
            .apply(runtime, self, backup_catalog_id, wallet_profile, options)
    }

    pub(crate) fn events(&self, operation_id: &str, after_seq: u64) -> Result<Value> {
        self.registry.events(
            &RuntimeOperationId::parse(operation_id)?,
            EventCursor::new(after_seq),
        )
    }

    pub(crate) fn cancel(&self, operation_id: &str) -> Result<Value> {
        let operation_id = RuntimeOperationId::parse(operation_id)?;
        self.registry
            .transition(&operation_id, RuntimeOperationTransition::CancelRequested)?;
        self.registry.value(&operation_id)
    }

    pub(crate) fn run_blocking(
        &self,
        runtime: &Runtime,
        method: OperationMethod,
        args: Value,
        label: &str,
    ) -> Result<Value> {
        let operation = self.start(
            runtime,
            RuntimeOperationRequest::from_call(method, args, label)?,
        )?;
        let operation_id = operation
            .get("operationId")
            .and_then(Value::as_str)
            .context("runtime operation id is missing")?
            .to_owned();
        let result = self.wait_for_result(&operation_id);
        self.finish_blocking_result(&operation_id, result)
    }

    fn finish_blocking_result(&self, operation_id: &str, result: Result<Value>) -> Result<Value> {
        let operation_id = RuntimeOperationId::parse(operation_id)?;
        let should_remove = self.registry.inspect(|records| {
            records.get(&operation_id).is_some_and(|record| {
                record.operation.status.is_terminal()
                    && record.operation.bridge_callback_id.is_none()
                    && record.operation.module_session_id.is_none()
                    && record.operation.module_request_id.is_none()
                    && record.operation.terminal_event.is_none()
            })
        })?;
        if should_remove {
            self.registry.remove(&operation_id)?;
        }
        result
    }

    pub(crate) fn wait_for_result(&self, operation_id: &str) -> Result<Value> {
        let operation_id = RuntimeOperationId::parse(operation_id)?;
        loop {
            let operation = self
                .registry
                .inspect(|records| {
                    records
                        .get(&operation_id)
                        .map(|record| record.operation.clone())
                })?
                .with_context(|| {
                    format!(
                        "runtime operation `{}` was not found",
                        operation_id.as_str()
                    )
                })?;
            match operation.status {
                RuntimeOperationStatus::Completed => {
                    return Ok(operation.result.unwrap_or(Value::Null));
                }
                RuntimeOperationStatus::AwaitingExternal | RuntimeOperationStatus::Dispatched => {
                    return Ok(operation.acknowledgement.unwrap_or(Value::Null));
                }
                RuntimeOperationStatus::Canceled => {
                    bail!(
                        "{}",
                        operation
                            .error
                            .unwrap_or_else(|| "runtime operation canceled".to_owned())
                    )
                }
                RuntimeOperationStatus::Failed => {
                    bail!(
                        "{}",
                        operation
                            .error
                            .unwrap_or_else(|| "runtime operation failed".to_owned())
                    )
                }
                RuntimeOperationStatus::TimedOut => {
                    bail!(
                        "{}",
                        operation
                            .error
                            .unwrap_or_else(|| "runtime operation timed out".to_owned())
                    )
                }
                RuntimeOperationStatus::Running | RuntimeOperationStatus::Canceling => {}
            }
            thread::sleep(Duration::from_millis(25));
        }
    }

    pub(crate) fn value(&self, operation_id: &str) -> Result<Value> {
        self.registry
            .value(&RuntimeOperationId::parse(operation_id)?)
    }

    pub(crate) fn ingest_module_event(&self, value: Value) -> Result<Value> {
        let event = ModuleEventEnvelope::from_value(&value)?;
        self.registry.ingest_module_event(event)
    }

    pub(crate) fn ingest_module_event_parts(
        &self,
        module: &str,
        event: &str,
        args: Vec<Value>,
    ) -> Result<Value> {
        self.registry
            .ingest_module_event(ModuleEventEnvelope::new(module, event, args)?)
    }

    #[cfg(test)]
    fn with_backup_import_store(store: Arc<dyn backup_import::BackupImportStore>) -> Self {
        Self {
            registry: RuntimeOperationRegistry::default(),
            next_operation_id: AtomicU64::new(1),
            backup_import: BackupImportCoordinator::new(store),
            module_transport: Arc::new(LogoscoreCliTransport::default()),
        }
    }

    #[cfg(test)]
    pub(crate) fn insert_test_running_operation(
        &self,
        operation_id: &str,
        request: RuntimeOperationRequest,
    ) -> Result<Arc<AtomicBool>> {
        let cancel_requested = Arc::new(AtomicBool::new(false));
        let record = running_runtime_operation_record(
            RuntimeOperationId::parse(operation_id)?,
            &request,
            Arc::clone(&cancel_requested),
            1,
        )?;
        self.registry.insert(record)?;
        Ok(cancel_requested)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> Result<usize> {
        self.registry.len()
    }
}

pub(crate) struct RuntimeOperationInterface {
    operations: RuntimeOperations,
}

impl Default for RuntimeOperationInterface {
    fn default() -> Self {
        Self::new(Arc::new(LogoscoreCliTransport::default()))
    }
}

struct RuntimeOperationRunner<'a> {
    runtime: &'a Runtime,
    operations: &'a RuntimeOperations,
}

impl OperationRunner for RuntimeOperationRunner<'_> {
    fn start_from_value(&self, value: Value) -> Result<Value> {
        self.operations.start_from_value(self.runtime, value)
    }

    fn ingest_module_event(&self, event: Value) -> Result<Value> {
        self.operations.ingest_module_event(event)
    }

    fn status(&self, operation_id: &str) -> Result<Value> {
        self.operations.status(operation_id)
    }

    fn events(&self, operation_id: &str, after_seq: u64) -> Result<Value> {
        self.operations.events(operation_id, after_seq)
    }

    fn cancel(&self, operation_id: &str) -> Result<Value> {
        self.operations.cancel(operation_id)
    }

    fn run_operation(&self, method: OperationMethod, args: Value, label: &str) -> Result<Value> {
        self.operations
            .run_blocking(self.runtime, method, args, label)
    }

    fn start_operation(&self, method: OperationMethod, args: Value, label: &str) -> Result<Value> {
        self.operations.start(
            self.runtime,
            RuntimeOperationRequest::from_call(method, args, label)?,
        )
    }

    fn preview_backup_import(
        &self,
        backup_catalog_id: &str,
        wallet_profile: Option<&Value>,
        options: Option<&Value>,
    ) -> Result<Value> {
        self.operations
            .preview_backup_import(backup_catalog_id, wallet_profile, options)
    }

    fn apply_backup_import(
        &self,
        backup_catalog_id: &str,
        wallet_profile: Option<&Value>,
        options: Option<&Value>,
    ) -> Result<Value> {
        self.operations.apply_backup_import(
            self.runtime,
            backup_catalog_id,
            wallet_profile,
            options,
        )
    }
}

impl RuntimeOperationInterface {
    pub(crate) fn new(module_transport: SharedModuleTransport) -> Self {
        Self {
            operations: RuntimeOperations {
                registry: RuntimeOperationRegistry::default(),
                next_operation_id: AtomicU64::new(1),
                backup_import: BackupImportCoordinator::new(Arc::new(LocalBackupImportStore)),
                module_transport,
            },
        }
    }

    pub(crate) fn bridge_call(
        &self,
        runtime: &Runtime,
        command: OperationBridgeCommand,
        args: &Value,
    ) -> Result<Value> {
        let runner = RuntimeOperationRunner {
            runtime,
            operations: &self.operations,
        };
        handle_operation_command(&runner, command, args)
    }

    pub(crate) fn ingest_module_event(
        &self,
        module: &str,
        event: &str,
        args: Vec<Value>,
    ) -> Result<Value> {
        self.operations
            .ingest_module_event_parts(module, event, args)
    }

    #[cfg(test)]
    pub(crate) fn insert_test_running_operation(
        &self,
        operation_id: &str,
        request: RuntimeOperationRequest,
    ) -> Result<Arc<AtomicBool>> {
        self.operations
            .insert_test_running_operation(operation_id, request)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> Result<usize> {
        self.operations.len()
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{Context as _, Result};
    use serde_json::json;

    use super::*;
    use crate::source_routing::{
        ModuleCorrelation, ModuleEventCorrelationKind, ModuleSessionId,
        ModuleTerminalEventContract, ObservableOperationAcceptance,
    };

    #[test]
    fn blocking_acknowledgement_retains_observable_conversation() -> Result<()> {
        let operations = RuntimeOperations::default();
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageUploadUrl",
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "path": "/tmp/runtime-operation-retention-test" }
        }))?;
        operations.insert_test_running_operation("retained-operation", request)?;
        let operation_id = RuntimeOperationId::parse("retained-operation")?;
        let session_id = ModuleSessionId::parse("session-retained").context("session id")?;
        operations.registry.transition(
            &operation_id,
            RuntimeOperationTransition::Resolved(Ok(outcome::RuntimeOperationOutcome::Accepted(
                Box::new(ObservableOperationAcceptance::new(
                    json!({ "dispatched": true, "value": "session-retained" }),
                    ModuleCorrelation::with_session(session_id),
                    ModuleTerminalEventContract::new(
                        "storage_module",
                        Some("storageUploadProgress"),
                        "storageUploadDone",
                        None,
                        ModuleEventCorrelationKind::Session,
                    ),
                )),
            ))),
        )?;

        let acknowledgement = operations.wait_for_result(operation_id.as_str());
        let acknowledgement =
            operations.finish_blocking_result(operation_id.as_str(), acknowledgement)?;

        anyhow::ensure!(acknowledgement.get("dispatched") == Some(&json!(true)));
        anyhow::ensure!(operations.len()? == 1);
        Ok(())
    }

    #[test]
    fn blocking_terminal_result_retains_external_correlation_tombstone() -> Result<()> {
        let operations = RuntimeOperations::default();
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageUploadUrl",
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "path": "/tmp/runtime-operation-terminal-retention-test" }
        }))?;
        operations.insert_test_running_operation("terminal-operation", request)?;
        let operation_id = RuntimeOperationId::parse("terminal-operation")?;
        let session_id = ModuleSessionId::parse("session-terminal").context("session id")?;
        operations.registry.transition(
            &operation_id,
            RuntimeOperationTransition::Resolved(Ok(outcome::RuntimeOperationOutcome::Accepted(
                Box::new(ObservableOperationAcceptance::new(
                    json!({ "dispatched": true }),
                    ModuleCorrelation::with_session(session_id),
                    ModuleTerminalEventContract::new(
                        "storage_module",
                        Some("storageUploadProgress"),
                        "storageUploadDone",
                        None,
                        ModuleEventCorrelationKind::Session,
                    ),
                )),
            ))),
        )?;
        operations.ingest_module_event(json!({
            "moduleName": "storage_module",
            "eventName": "storageUploadDone",
            "args": [{ "sessionId": "session-terminal", "cid": "cid-terminal" }]
        }))?;

        let result = operations.wait_for_result(operation_id.as_str());
        let result = operations.finish_blocking_result(operation_id.as_str(), result)?;

        anyhow::ensure!(result.get("cid") == Some(&json!("cid-terminal")));
        anyhow::ensure!(operations.len()? == 1);
        Ok(())
    }
}
