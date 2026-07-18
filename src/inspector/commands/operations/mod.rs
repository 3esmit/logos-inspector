use std::{
    sync::{Arc, atomic::AtomicU64},
    thread,
    time::Duration,
};

use anyhow::{Context as _, Result, bail};
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{
    inspection::l2::ActiveZoneContext,
    inspector::commands::zone_catalog::ZoneCatalogCommandInterface,
    modules::logos_core::{LogoscoreCliTransport, ModuleTransportKind, SharedModuleTransport},
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
mod supervisor;
mod transition;
mod wallet;
mod wallet_args;

use backup_import::{BackupImportCoordinator, LocalBackupImportStore};
#[cfg(test)]
pub(crate) use entrypoint::operation_bridge_command_names;
pub(crate) use entrypoint::{OperationBridgeCommand, operation_bridge_command};
use entrypoint::{OperationRunner, handle_operation_command};
use identity::{EventCursor, RuntimeOperationId, allocate_sequence};
use record::{RuntimeOperationRegistry, RuntimeOperationStatus, running_runtime_operation_record};
pub(crate) use request::{RuntimeOperationRequest, runtime_operation_request_from_value};
pub(crate) use spec::OperationMethod;
use spec::normalized_operation_method;
use supervisor::RuntimeOperationSupervisor;
use transition::RuntimeOperationTransition;

trait InstructionTargetResolver: Send + Sync {
    fn resolve(
        &self,
        runtime: &Runtime,
        context: &ActiveZoneContext,
        request_revision: u64,
    ) -> Result<lez::BoundInstructionTarget>;
}

struct UnavailableInstructionTargetResolver;

impl InstructionTargetResolver for UnavailableInstructionTargetResolver {
    fn resolve(
        &self,
        _runtime: &Runtime,
        _context: &ActiveZoneContext,
        _request_revision: u64,
    ) -> Result<lez::BoundInstructionTarget> {
        bail!("Active Zone target could not be verified")
    }
}

fn unavailable_instruction_target_resolver() -> Arc<dyn InstructionTargetResolver> {
    Arc::new(UnavailableInstructionTargetResolver)
}

pub(crate) struct RuntimeOperations {
    registry: RuntimeOperationRegistry,
    next_operation_id: AtomicU64,
    backup_import: Arc<BackupImportCoordinator>,
    instruction_target_resolver: Arc<dyn InstructionTargetResolver>,
    module_transport: SharedModuleTransport,
    supervisor: RuntimeOperationSupervisor,
}

impl Default for RuntimeOperations {
    fn default() -> Self {
        Self::new(
            Arc::new(LocalBackupImportStore),
            Arc::new(LogoscoreCliTransport::default()),
            unavailable_instruction_target_resolver(),
        )
    }
}

impl RuntimeOperations {
    #[cfg(test)]
    pub(crate) fn with_test_module_transport(module_transport: SharedModuleTransport) -> Self {
        Self::new(
            Arc::new(LocalBackupImportStore),
            module_transport,
            unavailable_instruction_target_resolver(),
        )
    }

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
        let result = self.start_admitted(runtime, request);
        drop(operation_permit);
        result
    }

    fn start_after_backup_import(
        &self,
        runtime: &Runtime,
        request: RuntimeOperationRequest,
    ) -> Result<Value> {
        self.start_admitted(runtime, request)
    }

    fn start_admitted(
        &self,
        runtime: &Runtime,
        mut request: RuntimeOperationRequest,
    ) -> Result<Value> {
        lez::bind_instruction_target(
            runtime,
            self.instruction_target_resolver.as_ref(),
            &mut request,
        )?;
        let operation_id = RuntimeOperationId::allocated(
            request.domain_name(),
            &normalized_operation_method(request.method_name()),
            allocate_sequence(&self.next_operation_id)?,
        );
        let admission = self
            .supervisor
            .prepare(runtime, operation_id.clone(), &request)?;
        let now = now_millis();
        let record = running_runtime_operation_record(operation_id.clone(), &request, now)?;
        self.registry.insert(record)?;
        if let Err(error) = self
            .registry
            .transition(&operation_id, RuntimeOperationTransition::Started)
        {
            self.registry.remove(&operation_id)?;
            return Err(error);
        }
        if let Err(error) =
            self.supervisor
                .admit(admission, request, Arc::clone(&self.module_transport))
        {
            self.registry.remove(&operation_id)?;
            return Err(error);
        }
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
        self.request_cancel(&operation_id)?;
        self.registry.value(&operation_id)
    }

    fn cancel_for_backup_import(&self, operation_id: &str) -> Result<bool> {
        let operation_id = RuntimeOperationId::parse(operation_id)?;
        self.request_cancel(&operation_id)
    }

    fn request_cancel(&self, operation_id: &RuntimeOperationId) -> Result<bool> {
        let disposition = self
            .registry
            .transition(operation_id, RuntimeOperationTransition::CancelRequested)?;
        let requested = disposition == transition::TransitionDisposition::Applied;
        if requested {
            self.supervisor.cancel(operation_id)?;
        }
        Ok(requested)
    }

    pub(crate) fn run_blocking(
        &self,
        runtime: &Runtime,
        method: OperationMethod,
        args: Value,
        label: &str,
    ) -> Result<Value> {
        let request = RuntimeOperationRequest::from_call(method, args, label)?;
        if request.requested_module_transport()? == Some(ModuleTransportKind::Module) {
            bail!(
                "host-backed operation `{}` requires `runtimeOperationStart`",
                request.method_name()
            );
        }
        let operation = self.start(runtime, request)?;
        let operation_id = operation
            .get("operationId")
            .and_then(Value::as_str)
            .context("runtime operation id is missing")?
            .to_owned();
        let result = self.wait_for_result(&operation_id);
        self.finish_blocking_result(&operation_id, result)
    }

    fn finish_blocking_result(&self, operation_id: &str, result: Result<Value>) -> Result<Value> {
        let _operation_id = RuntimeOperationId::parse(operation_id)?;
        result
    }

    pub(crate) fn wait_for_result(&self, operation_id: &str) -> Result<Value> {
        let operation_id = RuntimeOperationId::parse(operation_id)?;
        loop {
            let (operation, result_purged, acknowledgement_purged) = self
                .registry
                .inspect(|records| {
                    records.get(&operation_id).map(|record| {
                        (
                            record.operation.clone(),
                            record.result_purged,
                            record.acknowledgement_purged,
                        )
                    })
                })?
                .with_context(|| {
                    format!(
                        "runtime operation `{}` was not found",
                        operation_id.as_str()
                    )
                })?;
            match operation.status {
                RuntimeOperationStatus::Completed => {
                    if result_purged {
                        bail!("runtime operation result was purged by bounded history policy");
                    }
                    return Ok(operation.result.unwrap_or(Value::Null));
                }
                RuntimeOperationStatus::AwaitingExternal | RuntimeOperationStatus::Dispatched => {
                    if acknowledgement_purged {
                        bail!(
                            "runtime operation acknowledgement was purged by bounded history policy"
                        );
                    }
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
                RuntimeOperationStatus::Canceling => {
                    if let Some(error) = operation.error {
                        return Err(supervisor::OperationCleanupUnconfirmed::new(error).into());
                    }
                }
                RuntimeOperationStatus::Running => {}
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
        self.ingest_typed_module_event(event)
    }

    pub(crate) fn ingest_module_event_parts(
        &self,
        module: &str,
        event: &str,
        args: Vec<Value>,
    ) -> Result<Value> {
        self.ingest_typed_module_event(ModuleEventEnvelope::new(module, event, args)?)
    }

    fn ingest_typed_module_event(&self, event: ModuleEventEnvelope) -> Result<Value> {
        let transport_ingress = self.module_transport.ingest_module_event(
            event.module_name(),
            event.event_name(),
            event.args(),
        );
        let (value, settled_operation_id) =
            self.registry.ingest_module_event_with_settlement(event)?;
        if let Some(operation_id) = settled_operation_id {
            self.supervisor.settle(&operation_id)?;
        }
        transport_ingress.map(|()| value)
    }

    #[cfg(test)]
    fn with_backup_import_store(store: Arc<dyn backup_import::BackupImportStore>) -> Self {
        Self::new(
            store,
            Arc::new(LogoscoreCliTransport::default()),
            unavailable_instruction_target_resolver(),
        )
    }

    #[cfg(test)]
    pub(crate) fn insert_test_running_operation(
        &self,
        operation_id: &str,
        request: RuntimeOperationRequest,
    ) -> Result<()> {
        let record = running_runtime_operation_record(
            RuntimeOperationId::parse(operation_id)?,
            &request,
            1,
        )?;
        self.registry.insert(record)?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> Result<usize> {
        self.registry.len()
    }

    fn new(
        backup_import_store: Arc<dyn backup_import::BackupImportStore>,
        module_transport: SharedModuleTransport,
        instruction_target_resolver: Arc<dyn InstructionTargetResolver>,
    ) -> Self {
        let registry = RuntimeOperationRegistry::default();
        Self {
            supervisor: RuntimeOperationSupervisor::new(registry.clone()),
            registry,
            next_operation_id: AtomicU64::new(1),
            backup_import: Arc::new(BackupImportCoordinator::new(backup_import_store)),
            instruction_target_resolver,
            module_transport,
        }
    }

    pub(crate) fn close_handle(&self) -> RuntimeOperationCloseHandle {
        RuntimeOperationCloseHandle {
            supervisor: self.supervisor.clone(),
            backup_import: Arc::clone(&self.backup_import),
        }
    }

    pub(crate) fn shutdown(&self, runtime: &Runtime) -> Result<()> {
        self.supervisor.shutdown(runtime)
    }
}

impl Drop for RuntimeOperations {
    fn drop(&mut self) {
        let _result = self.backup_import.begin_close();
        let _result = self.supervisor.begin_close();
    }
}

pub(crate) struct RuntimeOperationInterface {
    operations: RuntimeOperations,
}

#[derive(Clone)]
pub(crate) struct RuntimeOperationCloseHandle {
    supervisor: RuntimeOperationSupervisor,
    backup_import: Arc<BackupImportCoordinator>,
}

impl RuntimeOperationCloseHandle {
    pub(crate) fn begin_close(&self) -> Result<()> {
        let backup_import_result = self.backup_import.begin_close();
        let supervisor_result = self.supervisor.begin_close();
        backup_import_result.and(supervisor_result)
    }
}

impl Default for RuntimeOperationInterface {
    fn default() -> Self {
        Self {
            operations: RuntimeOperations::new(
                Arc::new(LocalBackupImportStore),
                Arc::new(LogoscoreCliTransport::default()),
                unavailable_instruction_target_resolver(),
            ),
        }
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
    pub(crate) fn new(
        module_transport: SharedModuleTransport,
        zone_catalog: Arc<ZoneCatalogCommandInterface>,
    ) -> Self {
        Self {
            operations: RuntimeOperations::new(
                Arc::new(LocalBackupImportStore),
                module_transport,
                Arc::new(lez::ZoneCatalogInstructionTargetResolver::new(zone_catalog)),
            ),
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
    ) -> Result<()> {
        self.operations
            .insert_test_running_operation(operation_id, request)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> Result<usize> {
        self.operations.len()
    }

    pub(crate) fn shutdown(&self, runtime: &Runtime) -> Result<()> {
        self.operations.shutdown(runtime)
    }

    pub(crate) fn close_handle(&self) -> RuntimeOperationCloseHandle {
        self.operations.close_handle()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use anyhow::{Context as _, Result};
    use serde_json::json;

    use super::*;
    use crate::modules::logos_core::{
        ModuleCall, ModuleCallFuture, ModuleTransport, ModuleTransportEvent,
    };
    use crate::source_routing::{
        ModuleCorrelation, ModuleEventCorrelationKind, ModuleSessionId,
        ModuleTerminalEventContract, ObservableOperationAcceptance,
    };

    #[derive(Default)]
    struct RecordingEventTransport {
        events: Mutex<Vec<ModuleTransportEvent>>,
    }

    impl ModuleTransport for RecordingEventTransport {
        fn kind(&self) -> ModuleTransportKind {
            ModuleTransportKind::Module
        }

        fn call(&self, _call: ModuleCall) -> ModuleCallFuture<'_> {
            Box::pin(async { bail!("recording event transport does not support calls") })
        }

        fn ingest_module_event(&self, module: &str, event: &str, args: &[Value]) -> Result<()> {
            self.events
                .lock()
                .map_err(|_| anyhow::anyhow!("recorded event state is unavailable"))?
                .push(ModuleTransportEvent::new(module, event, args.to_vec())?);
            Ok(())
        }
    }

    #[test]
    fn typed_module_event_is_published_to_transport_before_reduction() -> Result<()> {
        let transport = Arc::new(RecordingEventTransport::default());
        let operations = RuntimeOperations::with_test_module_transport(transport.clone());

        operations.ingest_module_event_parts(
            "storage_module",
            "storageDownloadDoneV2",
            vec![json!({ "operationId": "download-7", "status": "completed" })],
        )?;

        let events = transport
            .events
            .lock()
            .map_err(|_| anyhow::anyhow!("recorded event state is unavailable"))?;
        let event = events
            .first()
            .context("typed module event was not published to the transport")?;
        anyhow::ensure!(events.len() == 1);
        anyhow::ensure!(event.module() == "storage_module");
        anyhow::ensure!(event.event() == "storageDownloadDoneV2");
        anyhow::ensure!(
            event.args() == [json!({ "operationId": "download-7", "status": "completed" })]
        );
        Ok(())
    }

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
            RuntimeOperationTransition::Resolved(outcome::RuntimeOperationOutcome::Accepted(
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
            )),
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
            RuntimeOperationTransition::Resolved(outcome::RuntimeOperationOutcome::Accepted(
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
            )),
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

    #[test]
    fn blocking_terminal_result_waits_for_bounded_history_eviction() -> Result<()> {
        let operations = RuntimeOperations::default();
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageManifests",
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://127.0.0.1:8080" }
            },
            "payload": {}
        }))?;
        operations.insert_test_running_operation("terminal-local-operation", request)?;
        let operation_id = RuntimeOperationId::parse("terminal-local-operation")?;
        operations.registry.transition(
            &operation_id,
            RuntimeOperationTransition::Resolved(outcome::RuntimeOperationOutcome::Completed(
                json!({ "manifests": [] }),
            )),
        )?;

        let result = operations.wait_for_result(operation_id.as_str());
        let result = operations.finish_blocking_result(operation_id.as_str(), result)?;

        anyhow::ensure!(result.get("manifests") == Some(&json!([])));
        anyhow::ensure!(operations.len()? == 1);
        Ok(())
    }

    #[test]
    fn blocking_wait_fails_explicitly_when_oversized_result_is_immediately_purged() -> Result<()> {
        let operations = RuntimeOperations::default();
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageManifests",
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://127.0.0.1:8080" }
            },
            "payload": {}
        }))?;
        operations.insert_test_running_operation("oversized-result", request)?;
        let operation_id = RuntimeOperationId::parse("oversized-result")?;
        operations.registry.transition(
            &operation_id,
            RuntimeOperationTransition::Resolved(outcome::RuntimeOperationOutcome::Completed(
                json!({ "payload": "x".repeat(33 * 1024 * 1024) }),
            )),
        )?;

        let error = operations
            .wait_for_result(operation_id.as_str())
            .err()
            .context("purged result should not be returned as null success")?;
        anyhow::ensure!(
            error.to_string() == "runtime operation result was purged by bounded history policy"
        );
        let value = operations.value(operation_id.as_str())?;
        anyhow::ensure!(value.get("status") == Some(&json!("completed")));
        anyhow::ensure!(value.get("result") == Some(&Value::Null));
        anyhow::ensure!(value.get("resultPurged") == Some(&json!(true)));
        Ok(())
    }

    #[test]
    fn blocking_wait_returns_cleanup_uncertainty_without_releasing_operation() -> Result<()> {
        let operations = RuntimeOperations::default();
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageDownloadBackupCatalogEntry",
            "adapter": {
                "source_mode": "logoscore_cli",
                "inputs": {}
            },
            "mutating_enabled": false,
            "payload": { "cid": "cid-cleanup-uncertain" }
        }))?;
        operations.insert_test_running_operation("cleanup-uncertain-operation", request)?;
        let operation_id = RuntimeOperationId::parse("cleanup-uncertain-operation")?;
        let evidence = "storage download cleanup was not confirmed: cancel=failed, watch=ok";
        operations.registry.transition(
            &operation_id,
            RuntimeOperationTransition::CleanupUnconfirmed {
                error: evidence.to_owned(),
            },
        )?;

        let error = operations
            .wait_for_result(operation_id.as_str())
            .err()
            .context("blocking wait should return cleanup uncertainty")?;
        anyhow::ensure!(
            error
                .downcast_ref::<supervisor::OperationCleanupUnconfirmed>()
                .is_some()
        );
        anyhow::ensure!(error.to_string() == evidence);
        let value = operations.value(operation_id.as_str())?;
        anyhow::ensure!(value.get("status") == Some(&json!("canceling")));
        anyhow::ensure!(value.get("terminalReason") == Some(&Value::Null));
        anyhow::ensure!(value.get("error") == Some(&json!(evidence)));
        anyhow::ensure!(operations.len()? == 1);
        Ok(())
    }

    #[test]
    fn blocking_wait_bounds_retained_cleanup_uncertainty() -> Result<()> {
        let operations = RuntimeOperations::default();
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageDownloadBackupCatalogEntry",
            "adapter": {
                "source_mode": "logoscore_cli",
                "inputs": {}
            },
            "mutating_enabled": false,
            "payload": { "cid": "cid-oversized-cleanup-evidence" }
        }))?;
        operations.insert_test_running_operation("oversized-cleanup-evidence", request)?;
        let operation_id = RuntimeOperationId::parse("oversized-cleanup-evidence")?;
        let evidence = "x".repeat(16 * 1024 + 1);
        operations.registry.transition(
            &operation_id,
            RuntimeOperationTransition::CleanupUnconfirmed { error: evidence },
        )?;

        let error = operations
            .wait_for_result(operation_id.as_str())
            .err()
            .context("oversized cleanup uncertainty should remain a blocking failure")?;
        anyhow::ensure!(
            error
                .to_string()
                .contains("runtime operation error redacted")
        );
        let value = operations.value(operation_id.as_str())?;
        anyhow::ensure!(value.get("status") == Some(&json!("canceling")));
        anyhow::ensure!(value.get("errorRedacted") == Some(&json!(true)));
        anyhow::ensure!(value.get("redactedErrorBytes") == Some(&json!(16 * 1024 + 1)));
        anyhow::ensure!(operations.len()? == 1);
        Ok(())
    }
}
