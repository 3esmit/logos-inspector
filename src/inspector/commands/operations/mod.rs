use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};
use tokio::runtime::Runtime;

use crate::support::time::now_millis;

mod blockchain;
mod delivery;
mod dispatch;
mod entrypoint;
mod lez;
mod local_nodes;
mod policy;
mod record;
mod request;
mod spec;
mod storage;
mod wallet;
mod wallet_args;

use dispatch::execute_runtime_operation;
#[cfg(test)]
pub(crate) use entrypoint::operation_bridge_command_names;
pub(crate) use entrypoint::{OperationBridgeCommand, operation_bridge_command};
use entrypoint::{OperationRunner, handle_operation_command};
use policy::RuntimeOperationPolicy;
use record::{
    RuntimeOperation, RuntimeOperationRecord, RuntimeOperationRegistry, RuntimeOperationStatus,
    active_operation_in_exclusive_group, finish_runtime_operation,
    push_runtime_operation_event_locked, runtime_operation_event_value, runtime_operation_value,
    update_runtime_operation,
};
pub(crate) use request::{RuntimeOperationRequest, runtime_operation_request_from_value};
use request::{runtime_operation_backend, runtime_operation_context};
pub(crate) use spec::OperationMethod;
use spec::{OperationExclusiveGroup, normalized_operation_method};

#[cfg(test)]
use record::test_runtime_operation_record;

#[derive(Debug)]
pub(crate) struct RuntimeOperations {
    registry: RuntimeOperationRegistry,
    next_operation_id: AtomicU64,
}

impl Default for RuntimeOperations {
    fn default() -> Self {
        Self {
            registry: Arc::new(Mutex::new(HashMap::new())),
            next_operation_id: AtomicU64::new(1),
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
        let operation_id = format!(
            "{}-{}-{}",
            request.domain,
            normalized_operation_method(request.method_name()),
            self.next_operation_id.fetch_add(1, Ordering::Relaxed)
        );
        let now = now_millis();
        let cancel_requested = Arc::new(AtomicBool::new(false));
        let operation = RuntimeOperation {
            operation_id: operation_id.clone(),
            domain: request.domain.clone(),
            backend: runtime_operation_backend(&request),
            method: request.method_name().to_owned(),
            status: RuntimeOperationStatus::Running,
            label: request.label().to_owned(),
            policy: RuntimeOperationPolicy::from_request(&request),
            context: runtime_operation_context(&request),
            external_session_id: None,
            progress: None,
            bytes_written: 0,
            content_length: None,
            result: None,
            error: None,
            cancellable: request.cancellable(),
            exclusive_group: request.exclusive_group(),
            started_at: now,
            updated_at: now,
        };
        {
            let mut operations = self
                .registry
                .lock()
                .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
            if let Some(group) = request.exclusive_group()
                && operations
                    .values()
                    .any(|record| active_operation_in_exclusive_group(record, group))
            {
                bail!("{}", exclusive_operation_message(group));
            }
            operations.insert(
                operation_id.clone(),
                RuntimeOperationRecord {
                    operation,
                    events: Vec::new(),
                    cancel_requested: Arc::clone(&cancel_requested),
                },
            );
        }
        update_runtime_operation(&self.registry, &operation_id, |record| {
            push_runtime_operation_event_locked(
                record,
                "started",
                "operation started",
                Some(0.0),
                None,
                None,
            );
        });

        let registry = Arc::clone(&self.registry);
        let task_operation_id = operation_id.clone();
        runtime.spawn(async move {
            let result = execute_runtime_operation(
                request,
                &registry,
                &task_operation_id,
                &cancel_requested,
            )
            .await;
            finish_runtime_operation(&registry, &task_operation_id, &cancel_requested, result);
        });

        self.value(&operation_id)
    }

    pub(crate) fn status(&self, operation_id: &str) -> Result<Value> {
        self.value(operation_id)
    }

    pub(crate) fn events(&self, operation_id: &str, after_seq: u64) -> Result<Value> {
        let operations = self
            .registry
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        let record = operations
            .get(operation_id)
            .with_context(|| format!("runtime operation `{operation_id}` was not found"))?;
        let events = record
            .events
            .iter()
            .filter(|event| event.seq > after_seq)
            .map(runtime_operation_event_value)
            .collect::<Vec<_>>();
        let next_seq = record.events.last().map_or(after_seq, |event| event.seq);
        Ok(json!({
            "operation": runtime_operation_value(&record.operation),
            "events": events,
            "nextSeq": next_seq,
        }))
    }

    pub(crate) fn cancel(&self, operation_id: &str) -> Result<Value> {
        {
            let mut operations = self
                .registry
                .lock()
                .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
            let record = operations
                .get_mut(operation_id)
                .with_context(|| format!("runtime operation `{operation_id}` was not found"))?;
            if !record.operation.status.is_terminal() && record.operation.cancellable {
                record.cancel_requested.store(true, Ordering::Relaxed);
                record.operation.status = RuntimeOperationStatus::Canceling;
                record.operation.updated_at = now_millis();
                push_runtime_operation_event_locked(
                    record,
                    "canceling",
                    "cancel requested",
                    None,
                    None,
                    None,
                );
            } else if !record.operation.status.is_terminal() {
                push_runtime_operation_event_locked(
                    record,
                    "cancel_ignored",
                    "operation is not cancellable",
                    None,
                    None,
                    None,
                );
            }
        }
        self.value(operation_id)
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
            RuntimeOperationRequest::from_call(method, args, label),
        )?;
        let operation_id = operation
            .get("operationId")
            .and_then(Value::as_str)
            .context("runtime operation id is missing")?
            .to_owned();
        let result = self.wait_for_result(&operation_id);
        self.remove(&operation_id);
        result
    }

    pub(crate) fn wait_for_result(&self, operation_id: &str) -> Result<Value> {
        loop {
            let operation = {
                let operations = self
                    .registry
                    .lock()
                    .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
                operations
                    .get(operation_id)
                    .with_context(|| format!("runtime operation `{operation_id}` was not found"))?
                    .operation
                    .clone()
            };
            if operation.status.is_terminal() {
                return match operation.status {
                    RuntimeOperationStatus::Completed => {
                        Ok(operation.result.unwrap_or(Value::Null))
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
                    RuntimeOperationStatus::Running | RuntimeOperationStatus::Canceling => {
                        bail!("runtime operation is still running")
                    }
                };
            }
            thread::sleep(Duration::from_millis(25));
        }
    }

    pub(crate) fn value(&self, operation_id: &str) -> Result<Value> {
        let operations = self
            .registry
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        let record = operations
            .get(operation_id)
            .with_context(|| format!("runtime operation `{operation_id}` was not found"))?;
        Ok(runtime_operation_value(&record.operation))
    }

    fn remove(&self, operation_id: &str) {
        if let Ok(mut operations) = self.registry.lock() {
            operations.remove(operation_id);
        }
    }

    #[cfg(test)]
    pub(crate) fn insert_test_running_operation(
        &self,
        operation_id: &str,
        domain: &str,
        method: &str,
        cancellable: bool,
    ) -> Arc<AtomicBool> {
        let cancel_requested = Arc::new(AtomicBool::new(false));
        let record = test_runtime_operation_record(
            operation_id,
            domain,
            method,
            RuntimeOperationStatus::Running,
            cancellable,
            OperationMethod::from_str(method).and_then(OperationMethod::exclusive_group),
            Arc::clone(&cancel_requested),
        );
        if let Ok(mut operations) = self.registry.lock() {
            operations.insert(operation_id.to_owned(), record);
        }
        cancel_requested
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> Result<usize> {
        Ok(self
            .registry
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?
            .len())
    }
}

fn exclusive_operation_message(group: OperationExclusiveGroup) -> &'static str {
    match group {
        OperationExclusiveGroup::StorageDownload => {
            "a storage download operation is already running"
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct RuntimeOperationInterface {
    operations: RuntimeOperations,
}

struct RuntimeOperationRunner<'a> {
    runtime: &'a Runtime,
    operations: &'a RuntimeOperations,
}

impl OperationRunner for RuntimeOperationRunner<'_> {
    fn start_from_value(&self, value: Value) -> Result<Value> {
        self.operations.start_from_value(self.runtime, value)
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
            RuntimeOperationRequest::from_call(method, args, label),
        )
    }
}

impl RuntimeOperationInterface {
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

    #[cfg(test)]
    pub(crate) fn insert_test_running_operation(
        &self,
        operation_id: &str,
        domain: &str,
        method: &str,
        cancellable: bool,
    ) -> Arc<AtomicBool> {
        self.operations
            .insert_test_running_operation(operation_id, domain, method, cancellable)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> Result<usize> {
        self.operations.len()
    }
}
