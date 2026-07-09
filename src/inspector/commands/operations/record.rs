use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::Result;
use serde_json::{Value, json};

use crate::support::time::now_millis;

use super::policy::RuntimeOperationPolicy;
use super::spec::OperationExclusiveGroup;

pub(super) type RuntimeOperationRegistry = Arc<Mutex<HashMap<String, RuntimeOperationRecord>>>;

#[derive(Debug, Clone)]
pub(super) struct RuntimeOperation {
    pub(super) operation_id: String,
    pub(super) domain: String,
    pub(super) backend: String,
    pub(super) method: String,
    pub(super) status: RuntimeOperationStatus,
    pub(super) label: String,
    pub(super) policy: RuntimeOperationPolicy,
    pub(super) context: Value,
    pub(super) external_session_id: Option<String>,
    pub(super) progress: Option<f64>,
    pub(super) bytes_written: u64,
    pub(super) content_length: Option<u64>,
    pub(super) result: Option<Value>,
    pub(super) error: Option<String>,
    pub(super) cancellable: bool,
    pub(super) exclusive_group: Option<OperationExclusiveGroup>,
    pub(super) started_at: u64,
    pub(super) updated_at: u64,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeOperationEvent {
    pub(super) seq: u64,
    operation_id: String,
    domain: String,
    method: String,
    phase: String,
    external_session_id: Option<String>,
    message: String,
    progress: Option<f64>,
    result: Option<Value>,
    error: Option<String>,
    timestamp: u64,
}

#[derive(Debug)]
pub(super) struct RuntimeOperationRecord {
    pub(super) operation: RuntimeOperation,
    pub(super) events: Vec<RuntimeOperationEvent>,
    pub(super) cancel_requested: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeOperationStatus {
    Running,
    Canceling,
    Completed,
    Failed,
    Canceled,
}

impl RuntimeOperationStatus {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Canceling => "canceling",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
        }
    }

    pub(super) fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Canceled)
    }
}

pub(super) fn update_runtime_operation(
    registry: &RuntimeOperationRegistry,
    operation_id: &str,
    update: impl FnOnce(&mut RuntimeOperationRecord),
) {
    if let Ok(mut operations) = registry.lock()
        && let Some(record) = operations.get_mut(operation_id)
    {
        update(record);
    }
}

pub(super) fn active_operation_in_exclusive_group(
    record: &RuntimeOperationRecord,
    group: OperationExclusiveGroup,
) -> bool {
    record.operation.exclusive_group == Some(group) && !record.operation.status.is_terminal()
}

pub(super) fn update_runtime_operation_progress(
    registry: &RuntimeOperationRegistry,
    operation_id: &str,
    bytes_written: u64,
    content_length: Option<u64>,
) {
    update_runtime_operation(registry, operation_id, |record| {
        record.operation.bytes_written = bytes_written;
        if content_length.is_some() {
            record.operation.content_length = content_length;
        }
        let progress = operation_progress(bytes_written, record.operation.content_length);
        record.operation.progress = progress;
        push_runtime_operation_event_locked(
            record,
            "progress",
            "operation progress",
            progress,
            None,
            None,
        );
    });
}

pub(super) fn finish_runtime_operation(
    registry: &RuntimeOperationRegistry,
    operation_id: &str,
    cancel_requested: &AtomicBool,
    result: Result<Value>,
) {
    update_runtime_operation(registry, operation_id, |record| match result {
        Ok(value) => {
            record.operation.status = RuntimeOperationStatus::Completed;
            record.operation.external_session_id = external_session_id(&value);
            record.operation.result = Some(value.clone());
            record.operation.error = None;
            record.operation.progress = Some(1.0);
            record.operation.updated_at = now_millis();
            push_runtime_operation_event_locked(
                record,
                "completed",
                "operation completed",
                Some(1.0),
                Some(value),
                None,
            );
        }
        Err(error) if cancel_requested.load(Ordering::Relaxed) => {
            let error_text = error.to_string();
            record.operation.status = RuntimeOperationStatus::Canceled;
            record.operation.error = Some(error_text.clone());
            record.operation.updated_at = now_millis();
            push_runtime_operation_event_locked(
                record,
                "canceled",
                "operation canceled",
                record.operation.progress,
                None,
                Some(error_text),
            );
        }
        Err(error) => {
            let error_text = error.to_string();
            record.operation.status = RuntimeOperationStatus::Failed;
            record.operation.error = Some(error_text.clone());
            record.operation.updated_at = now_millis();
            push_runtime_operation_event_locked(
                record,
                "failed",
                "operation failed",
                record.operation.progress,
                None,
                Some(error_text),
            );
        }
    });
}

pub(super) fn push_runtime_operation_event_locked(
    record: &mut RuntimeOperationRecord,
    phase: &str,
    message: &str,
    progress: Option<f64>,
    result: Option<Value>,
    error: Option<String>,
) {
    if let Some(value) = progress {
        record.operation.progress = Some(value);
    }
    record.operation.updated_at = now_millis();
    let seq = u64::try_from(record.events.len())
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    record.events.push(RuntimeOperationEvent {
        seq,
        operation_id: record.operation.operation_id.clone(),
        domain: record.operation.domain.clone(),
        method: record.operation.method.clone(),
        phase: phase.to_owned(),
        external_session_id: record.operation.external_session_id.clone(),
        message: message.to_owned(),
        progress,
        result,
        error,
        timestamp: now_millis(),
    });
}

fn operation_progress(bytes_written: u64, content_length: Option<u64>) -> Option<f64> {
    match content_length {
        Some(total) if total > 0 => Some(bytes_written as f64 / total as f64),
        _ => None,
    }
}

fn external_session_id(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    for key in [
        "sessionId",
        "session_id",
        "operationId",
        "operation_id",
        "requestId",
        "request_id",
    ] {
        if let Some(value) = object
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_owned());
        }
    }
    None
}

pub(super) fn runtime_operation_value(operation: &RuntimeOperation) -> Value {
    let mut value = json!({
        "operationId": operation.operation_id,
        "domain": operation.domain,
        "backend": operation.backend,
        "method": operation.method,
        "status": operation.status.as_str(),
        "label": operation.label,
        "policyFacts": operation.policy.as_value(),
        "externalSessionId": operation.external_session_id,
        "progress": operation.progress,
        "bytesWritten": operation.bytes_written,
        "contentLength": operation.content_length,
        "result": operation.result,
        "error": operation.error,
        "cancellable": operation.cancellable && !operation.status.is_terminal(),
        "startedAt": operation.started_at,
        "updatedAt": operation.updated_at,
        "context": operation.context,
    });
    if let (Value::Object(target), Value::Object(context)) = (&mut value, &operation.context) {
        for key in ["cid", "path", "endpoint", "source"] {
            if let Some(context_value) = context.get(key) {
                target.insert(key.to_owned(), context_value.clone());
            }
        }
    }
    if let (Value::Object(target), Some(Value::Object(result))) = (&mut value, &operation.result) {
        for key in [
            "cid",
            "path",
            "endpoint",
            "source",
            "sessionId",
            "requestId",
        ] {
            if !target.contains_key(key)
                && let Some(result_value) = result.get(key)
            {
                target.insert(key.to_owned(), result_value.clone());
            }
        }
    }
    value
}

pub(super) fn runtime_operation_event_value(event: &RuntimeOperationEvent) -> Value {
    json!({
        "seq": event.seq,
        "operationId": event.operation_id,
        "domain": event.domain,
        "method": event.method,
        "phase": event.phase,
        "externalSessionId": event.external_session_id,
        "message": event.message,
        "progress": event.progress,
        "result": event.result,
        "error": event.error,
        "timestamp": event.timestamp,
    })
}

#[cfg(test)]
pub(super) fn test_runtime_operation_record(
    operation_id: &str,
    domain: &str,
    method: &str,
    status: RuntimeOperationStatus,
    cancellable: bool,
    exclusive_group: Option<OperationExclusiveGroup>,
    cancel_requested: Arc<AtomicBool>,
) -> RuntimeOperationRecord {
    RuntimeOperationRecord {
        operation: RuntimeOperation {
            operation_id: operation_id.to_owned(),
            domain: domain.to_owned(),
            backend: "test".to_owned(),
            method: method.to_owned(),
            status,
            label: "Test operation".to_owned(),
            policy: RuntimeOperationPolicy::from_method(domain, method),
            context: Value::Null,
            external_session_id: None,
            progress: None,
            bytes_written: 0,
            content_length: None,
            result: None,
            error: None,
            cancellable,
            exclusive_group,
            started_at: 1,
            updated_at: 1,
        },
        events: Vec::new(),
        cancel_requested,
    }
}
