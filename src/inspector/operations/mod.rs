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

use crate::inspector::methods::{normalized_operation_method, operation_cancellable};

mod chain;
mod delivery;
mod dispatch;
mod local_nodes;
mod record;
mod request;
mod storage;
mod wallet;

use dispatch::execute_node_operation;
use record::{
    NodeOperation, NodeOperationRecord, NodeOperationRegistry, NodeOperationStatus,
    active_storage_download_operation, finish_node_operation, node_operation_event_value,
    node_operation_value, now_millis, push_node_operation_event_locked, update_node_operation,
};
pub(crate) use request::{NodeOperationRequest, node_operation_request_from_value};
use request::{node_operation_backend, node_operation_context};

#[cfg(test)]
use record::test_node_operation_record;

#[derive(Debug)]
pub(crate) struct NodeOperations {
    registry: NodeOperationRegistry,
    next_operation_id: AtomicU64,
}

impl Default for NodeOperations {
    fn default() -> Self {
        Self {
            registry: Arc::new(Mutex::new(HashMap::new())),
            next_operation_id: AtomicU64::new(1),
        }
    }
}

impl NodeOperations {
    pub(crate) fn start_from_value(&self, runtime: &Runtime, value: Value) -> Result<Value> {
        let request = node_operation_request_from_value(value)?;
        self.start(runtime, request)
    }

    pub(crate) fn start(&self, runtime: &Runtime, request: NodeOperationRequest) -> Result<Value> {
        let operation_id = format!(
            "{}-{}-{}",
            request.domain,
            normalized_operation_method(&request.method),
            self.next_operation_id.fetch_add(1, Ordering::Relaxed)
        );
        let now = now_millis();
        let cancel_requested = Arc::new(AtomicBool::new(false));
        let operation = NodeOperation {
            operation_id: operation_id.clone(),
            domain: request.domain.clone(),
            backend: node_operation_backend(&request),
            method: request.method.clone(),
            status: NodeOperationStatus::Running,
            label: request.label.clone(),
            context: node_operation_context(&request),
            external_session_id: None,
            progress: None,
            bytes_written: 0,
            content_length: None,
            result: None,
            error: None,
            cancellable: operation_cancellable(&request.method),
            started_at: now,
            updated_at: now,
        };
        {
            let mut operations = self
                .registry
                .lock()
                .map_err(|_| anyhow::anyhow!("node operation registry is unavailable"))?;
            if request.domain == "storage"
                && request.method == "storageDownloadToUrl"
                && operations.values().any(active_storage_download_operation)
            {
                bail!("a storage download operation is already running");
            }
            operations.insert(
                operation_id.clone(),
                NodeOperationRecord {
                    operation,
                    events: Vec::new(),
                    cancel_requested: Arc::clone(&cancel_requested),
                },
            );
        }
        update_node_operation(&self.registry, &operation_id, |record| {
            push_node_operation_event_locked(
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
            let result =
                execute_node_operation(request, &registry, &task_operation_id, &cancel_requested)
                    .await;
            finish_node_operation(&registry, &task_operation_id, &cancel_requested, result);
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
            .map_err(|_| anyhow::anyhow!("node operation registry is unavailable"))?;
        let record = operations
            .get(operation_id)
            .with_context(|| format!("node operation `{operation_id}` was not found"))?;
        let events = record
            .events
            .iter()
            .filter(|event| event.seq > after_seq)
            .map(node_operation_event_value)
            .collect::<Vec<_>>();
        let next_seq = record.events.last().map_or(after_seq, |event| event.seq);
        Ok(json!({
            "operation": node_operation_value(&record.operation),
            "events": events,
            "nextSeq": next_seq,
        }))
    }

    pub(crate) fn cancel(&self, operation_id: &str) -> Result<Value> {
        {
            let mut operations = self
                .registry
                .lock()
                .map_err(|_| anyhow::anyhow!("node operation registry is unavailable"))?;
            let record = operations
                .get_mut(operation_id)
                .with_context(|| format!("node operation `{operation_id}` was not found"))?;
            if !record.operation.status.is_terminal() && record.operation.cancellable {
                record.cancel_requested.store(true, Ordering::Relaxed);
                record.operation.status = NodeOperationStatus::Canceling;
                record.operation.updated_at = now_millis();
                push_node_operation_event_locked(
                    record,
                    "canceling",
                    "cancel requested",
                    None,
                    None,
                    None,
                );
            } else if !record.operation.status.is_terminal() {
                push_node_operation_event_locked(
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

    pub(crate) fn run_legacy(
        &self,
        runtime: &Runtime,
        domain: &str,
        method: &str,
        args: Value,
        label: &str,
    ) -> Result<Value> {
        let operation = self.start(
            runtime,
            NodeOperationRequest::legacy(domain, method, args, label),
        )?;
        let operation_id = operation
            .get("operationId")
            .and_then(Value::as_str)
            .context("node operation id is missing")?
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
                    .map_err(|_| anyhow::anyhow!("node operation registry is unavailable"))?;
                operations
                    .get(operation_id)
                    .with_context(|| format!("node operation `{operation_id}` was not found"))?
                    .operation
                    .clone()
            };
            if operation.status.is_terminal() {
                return match operation.status {
                    NodeOperationStatus::Completed => Ok(operation.result.unwrap_or(Value::Null)),
                    NodeOperationStatus::Canceled => {
                        bail!(
                            "{}",
                            operation
                                .error
                                .unwrap_or_else(|| "node operation canceled".to_owned())
                        )
                    }
                    NodeOperationStatus::Failed => {
                        bail!(
                            "{}",
                            operation
                                .error
                                .unwrap_or_else(|| "node operation failed".to_owned())
                        )
                    }
                    NodeOperationStatus::Running | NodeOperationStatus::Canceling => {
                        bail!("node operation is still running")
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
            .map_err(|_| anyhow::anyhow!("node operation registry is unavailable"))?;
        let record = operations
            .get(operation_id)
            .with_context(|| format!("node operation `{operation_id}` was not found"))?;
        Ok(node_operation_value(&record.operation))
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
        let record = test_node_operation_record(
            operation_id,
            domain,
            method,
            NodeOperationStatus::Running,
            cancellable,
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
            .map_err(|_| anyhow::anyhow!("node operation registry is unavailable"))?
            .len())
    }
}
