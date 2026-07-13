use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};

use crate::{
    source_routing::{
        BridgeCallbackId, ModuleEventEnvelope, ModuleRequestId, ModuleSessionId,
        ModuleTerminalEventContract,
    },
    support::time::now_millis,
};

use super::{
    identity::{ClientRequestId, EventCursor, RuntimeOperationId},
    policy::RuntimeOperationPolicy,
    request::{
        RuntimeOperationRequest, runtime_operation_backend, runtime_operation_context,
        runtime_operation_pending_module,
    },
    spec::OperationExclusiveGroup,
    transition::{
        ModuleEventIngressResult, RuntimeOperationTransition, TransitionDisposition,
        apply_deferred_module_event, apply_module_event_ingress,
        apply_runtime_operation_transition,
    },
};

const MAX_PENDING_MODULE_EVENTS_PER_MODULE: usize = 32;

#[derive(Debug, Clone, Default)]
pub(super) struct RuntimeOperationRegistry {
    state: Arc<Mutex<RuntimeOperationRegistryState>>,
}

#[derive(Debug, Default)]
struct RuntimeOperationRegistryState {
    records: HashMap<RuntimeOperationId, RuntimeOperationRecord>,
    pending_module_events: HashMap<String, VecDeque<PendingModuleEvent>>,
}

#[derive(Debug, Clone)]
struct PendingModuleEvent {
    event: ModuleEventEnvelope,
    candidate_operation_ids: Vec<RuntimeOperationId>,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeOperation {
    pub(super) operation_id: RuntimeOperationId,
    pub(super) client_request_id: Option<ClientRequestId>,
    pub(super) bridge_callback_id: Option<BridgeCallbackId>,
    pub(super) module_session_id: Option<ModuleSessionId>,
    pub(super) module_request_id: Option<ModuleRequestId>,
    pub(super) terminal_event: Option<ModuleTerminalEventContract>,
    pub(super) event_cursor: EventCursor,
    pub(super) domain: String,
    pub(super) backend: String,
    pub(super) method: String,
    pub(super) status: RuntimeOperationStatus,
    pub(super) label: String,
    pub(super) policy: RuntimeOperationPolicy,
    pub(super) context: Value,
    pub(super) acknowledgement: Option<Value>,
    pub(super) progress: Option<f64>,
    pub(super) bytes_written: u64,
    pub(super) content_length: Option<u64>,
    pub(super) result: Option<Value>,
    pub(super) error: Option<String>,
    pub(super) terminal_reason: Option<String>,
    pub(super) cancellable: bool,
    pub(super) exclusive_group: Option<OperationExclusiveGroup>,
    pub(super) started_at: u64,
    pub(super) updated_at: u64,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeOperationEvent {
    cursor: EventCursor,
    operation_id: RuntimeOperationId,
    client_request_id: Option<ClientRequestId>,
    bridge_callback_id: Option<BridgeCallbackId>,
    module_session_id: Option<ModuleSessionId>,
    module_request_id: Option<ModuleRequestId>,
    domain: String,
    method: String,
    phase: String,
    message: String,
    progress: Option<f64>,
    result: Option<Value>,
    error: Option<String>,
    timestamp: u64,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeOperationRecord {
    pub(super) operation: RuntimeOperation,
    pub(super) restart_request: Option<RuntimeOperationRequest>,
    pub(super) events: Vec<RuntimeOperationEvent>,
    pub(super) cancel_requested: Arc<AtomicBool>,
    pub(super) pending_module_name: Option<&'static str>,
    pub(super) next_event_cursor: EventCursor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeOperationStatus {
    Running,
    AwaitingExternal,
    Canceling,
    Completed,
    Dispatched,
    Failed,
    Canceled,
    TimedOut,
}

impl RuntimeOperationRegistry {
    pub(super) fn insert(&self, record: RuntimeOperationRecord) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        let operation_id = record.operation.operation_id.clone();
        if state.records.contains_key(&operation_id) {
            bail!(
                "runtime operation `{}` already exists",
                operation_id.as_str()
            );
        }
        if let Some(group) = record.operation.exclusive_group
            && state
                .records
                .values()
                .any(|candidate| active_operation_in_exclusive_group(candidate, group))
        {
            bail!("{}", exclusive_operation_message(group));
        }
        state.records.insert(operation_id, record);
        Ok(())
    }

    pub(super) fn transition(
        &self,
        operation_id: &RuntimeOperationId,
        transition: RuntimeOperationTransition,
    ) -> Result<TransitionDisposition> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        let Some(record) = state.records.get(operation_id) else {
            return apply_runtime_operation_transition(
                &mut state.records,
                operation_id,
                transition,
            );
        };
        let pending_module_name = record.pending_module_name;
        let should_replay = {
            !record.operation.status.is_terminal()
                && record.pending_module_name.is_some()
                && transition_settles_pending_module(&transition)
        };
        let replay_count = if should_replay {
            pending_module_name.map_or(0, |module_name| {
                pending_module_replay_count(&state, module_name, operation_id)
            })
        } else {
            0
        };
        ensure_record_event_cursor_capacity(record, replay_count.saturating_add(1))?;
        let previous_record = should_replay.then(|| record.clone());
        let previous_cancel_requested = previous_record
            .as_ref()
            .map(|record| record.cancel_requested.load(Ordering::Relaxed));
        let previous_pending_events = should_replay
            .then(|| {
                pending_module_name
                    .and_then(|module_name| state.pending_module_events.get(module_name).cloned())
            })
            .flatten();

        let result = (|| {
            let disposition =
                apply_runtime_operation_transition(&mut state.records, operation_id, transition)?;
            if should_replay && let Some(module_name) = pending_module_name {
                replay_pending_module_events(&mut state, module_name, operation_id)?;
            }
            Ok(disposition)
        })();
        if result.is_err()
            && let Some(previous_record) = previous_record
        {
            if let Some(previous_cancel_requested) = previous_cancel_requested {
                previous_record
                    .cancel_requested
                    .store(previous_cancel_requested, Ordering::Relaxed);
            }
            state.records.insert(operation_id.clone(), previous_record);
            if let Some(module_name) = pending_module_name {
                match previous_pending_events {
                    Some(previous_pending_events) => {
                        state
                            .pending_module_events
                            .insert(module_name.to_owned(), previous_pending_events);
                    }
                    None => {
                        state.pending_module_events.remove(module_name);
                    }
                }
            }
        }
        result
    }

    pub(super) fn ingest_module_event(&self, event: ModuleEventEnvelope) -> Result<Value> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        let result = apply_module_event_ingress(&mut state.records, &event)?;
        let result = match result {
            unchanged @ (ModuleEventIngressResult::Unknown
            | ModuleEventIngressResult::Uncorrelated { .. }) => {
                let candidate_operation_ids =
                    pending_module_event_candidates(&state.records, event.module_name());
                if candidate_operation_ids.is_empty() {
                    unchanged
                } else {
                    defer_module_event(&mut state, event, candidate_operation_ids.clone())?;
                    ModuleEventIngressResult::Deferred {
                        operation_ids: candidate_operation_ids,
                    }
                }
            }
            unchanged @ (ModuleEventIngressResult::Applied { .. }
            | ModuleEventIngressResult::Stale { .. }
            | ModuleEventIngressResult::Deferred { .. }
            | ModuleEventIngressResult::Ambiguous { .. }) => unchanged,
        };
        Ok(result.as_value(&state.records))
    }

    pub(super) fn value(&self, operation_id: &RuntimeOperationId) -> Result<Value> {
        self.inspect(|records| {
            let record = records.get(operation_id).with_context(|| {
                format!(
                    "runtime operation `{}` was not found",
                    operation_id.as_str()
                )
            })?;
            Ok(runtime_operation_value(&record.operation))
        })?
    }

    pub(super) fn events(
        &self,
        operation_id: &RuntimeOperationId,
        after: EventCursor,
    ) -> Result<Value> {
        self.inspect(|records| {
            let record = records.get(operation_id).with_context(|| {
                format!(
                    "runtime operation `{}` was not found",
                    operation_id.as_str()
                )
            })?;
            let events = record
                .events
                .iter()
                .filter(|event| event.cursor > after)
                .map(runtime_operation_event_value)
                .collect::<Vec<_>>();
            let next = record.events.last().map_or(after, |event| event.cursor);
            Ok(json!({
                "operation": runtime_operation_value(&record.operation),
                "events": events,
                "eventCursor": next.value(),
                "nextSeq": next.value(),
            }))
        })?
    }

    pub(super) fn inspect<T>(
        &self,
        inspect: impl FnOnce(&HashMap<RuntimeOperationId, RuntimeOperationRecord>) -> T,
    ) -> Result<T> {
        let state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        Ok(inspect(&state.records))
    }

    pub(super) fn remove(&self, operation_id: &RuntimeOperationId) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        state.records.remove(operation_id);
        remove_operation_from_pending_module_events(&mut state, operation_id);
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> Result<usize> {
        self.inspect(HashMap::len)
    }

    #[cfg(test)]
    pub(super) fn pending_module_event_count(&self, module_name: &str) -> Result<usize> {
        let state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        Ok(state
            .pending_module_events
            .get(module_name)
            .map_or(0, VecDeque::len))
    }
}

fn pending_module_event_candidates(
    records: &HashMap<RuntimeOperationId, RuntimeOperationRecord>,
    module_name: &str,
) -> Vec<RuntimeOperationId> {
    let mut operation_ids = records
        .iter()
        .filter(|(_, record)| {
            !record.operation.status.is_terminal()
                && record.pending_module_name == Some(module_name)
        })
        .map(|(operation_id, _)| operation_id.clone())
        .collect::<Vec<_>>();
    operation_ids.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    operation_ids
}

fn defer_module_event(
    state: &mut RuntimeOperationRegistryState,
    event: ModuleEventEnvelope,
    candidate_operation_ids: Vec<RuntimeOperationId>,
) -> Result<()> {
    let module_name = event.module_name().to_owned();
    let evicted = state
        .pending_module_events
        .get(&module_name)
        .filter(|events| events.len() >= MAX_PENDING_MODULE_EVENTS_PER_MODULE)
        .and_then(|events| events.front())
        .cloned();
    let mut evidence_counts = HashMap::new();
    for operation_id in &candidate_operation_ids {
        *evidence_counts.entry(operation_id.clone()).or_insert(0) += 1;
    }
    if let Some(evicted) = &evicted {
        for operation_id in &evicted.candidate_operation_ids {
            *evidence_counts.entry(operation_id.clone()).or_insert(0) += 1;
        }
    }
    ensure_event_cursor_capacity(&state.records, &evidence_counts)?;

    if let Some(evicted) = &evicted {
        for operation_id in &evicted.candidate_operation_ids {
            let Some(record) = state.records.get_mut(operation_id) else {
                continue;
            };
            push_runtime_operation_event_locked(
                record,
                "module_event_journal_overflow",
                "oldest deferred module event evicted before correlation registration",
                None,
                Some(evicted.event.result()),
                evicted.event.error(),
            )?;
        }
        state
            .pending_module_events
            .entry(module_name.clone())
            .or_default()
            .pop_front();
    }
    for operation_id in &candidate_operation_ids {
        let record = state
            .records
            .get_mut(operation_id)
            .ok_or_else(|| anyhow::anyhow!("pending runtime operation disappeared"))?;
        push_runtime_operation_event_locked(
            record,
            "module_event_deferred",
            "module event arrived before dispatch correlation registration",
            None,
            Some(event.result()),
            event.error(),
        )?;
    }
    state
        .pending_module_events
        .entry(module_name)
        .or_default()
        .push_back(PendingModuleEvent {
            event,
            candidate_operation_ids,
        });
    Ok(())
}

fn ensure_event_cursor_capacity(
    records: &HashMap<RuntimeOperationId, RuntimeOperationRecord>,
    required_events: &HashMap<RuntimeOperationId, usize>,
) -> Result<()> {
    for (operation_id, required) in required_events {
        let Some(record) = records.get(operation_id) else {
            continue;
        };
        let mut cursor = record.next_event_cursor;
        for _ in 0..*required {
            cursor = cursor.next()?;
        }
    }
    Ok(())
}

fn ensure_record_event_cursor_capacity(
    record: &RuntimeOperationRecord,
    required_events: usize,
) -> Result<()> {
    let mut cursor = record.next_event_cursor;
    for _ in 0..required_events {
        cursor = cursor.next()?;
    }
    Ok(())
}

fn transition_settles_pending_module(transition: &RuntimeOperationTransition) -> bool {
    matches!(
        transition,
        RuntimeOperationTransition::Resolved(_)
            | RuntimeOperationTransition::CancellationConfirmed { .. }
            | RuntimeOperationTransition::TimedOut { .. }
            | RuntimeOperationTransition::Shutdown { .. }
    )
}

fn pending_module_replay_count(
    state: &RuntimeOperationRegistryState,
    module_name: &str,
    operation_id: &RuntimeOperationId,
) -> usize {
    state
        .pending_module_events
        .get(module_name)
        .map_or(0, |pending_events| {
            pending_events
                .iter()
                .filter(|pending| pending.candidate_operation_ids.contains(operation_id))
                .count()
        })
}

fn replay_pending_module_events(
    state: &mut RuntimeOperationRegistryState,
    module_name: &str,
    operation_id: &RuntimeOperationId,
) -> Result<()> {
    let replay_count = pending_module_replay_count(state, module_name, operation_id);
    if let Some(record) = state.records.get(operation_id) {
        ensure_record_event_cursor_capacity(record, replay_count)?;
    }
    let Some(mut pending_events) = state.pending_module_events.remove(module_name) else {
        return Ok(());
    };
    let record = state
        .records
        .get_mut(operation_id)
        .ok_or_else(|| anyhow::anyhow!("pending runtime operation disappeared during replay"))?;
    let mut retained_events = VecDeque::new();
    while let Some(mut pending) = pending_events.pop_front() {
        if !pending.candidate_operation_ids.contains(operation_id) {
            retained_events.push_back(pending);
            continue;
        }
        match apply_deferred_module_event(record, &pending.event) {
            Ok(true) => {}
            Ok(false) => {
                pending
                    .candidate_operation_ids
                    .retain(|candidate| candidate != operation_id);
                if !pending.candidate_operation_ids.is_empty() {
                    retained_events.push_back(pending);
                }
            }
            Err(error) => {
                retained_events.push_back(pending);
                retained_events.append(&mut pending_events);
                state
                    .pending_module_events
                    .insert(module_name.to_owned(), retained_events);
                return Err(error);
            }
        }
    }
    if !retained_events.is_empty() {
        state
            .pending_module_events
            .insert(module_name.to_owned(), retained_events);
    }
    Ok(())
}

fn remove_operation_from_pending_module_events(
    state: &mut RuntimeOperationRegistryState,
    operation_id: &RuntimeOperationId,
) {
    for pending_events in state.pending_module_events.values_mut() {
        for pending in pending_events.iter_mut() {
            pending
                .candidate_operation_ids
                .retain(|candidate| candidate != operation_id);
        }
        pending_events.retain(|pending| !pending.candidate_operation_ids.is_empty());
    }
    state
        .pending_module_events
        .retain(|_, pending_events| !pending_events.is_empty());
}

pub(super) fn running_runtime_operation_record(
    operation_id: RuntimeOperationId,
    request: &RuntimeOperationRequest,
    cancel_requested: Arc<AtomicBool>,
    now: u64,
) -> Result<RuntimeOperationRecord> {
    let context = runtime_operation_context(request)?;
    let policy = RuntimeOperationPolicy::from_request(request, &context)?;
    Ok(RuntimeOperationRecord {
        operation: RuntimeOperation {
            operation_id,
            client_request_id: request.client_request_id().cloned(),
            bridge_callback_id: None,
            module_session_id: None,
            module_request_id: None,
            terminal_event: None,
            event_cursor: EventCursor::new(0),
            domain: request.domain_name().to_owned(),
            backend: runtime_operation_backend(request),
            method: request.method_name().to_owned(),
            status: RuntimeOperationStatus::Running,
            label: request.label().to_owned(),
            policy,
            context,
            acknowledgement: None,
            progress: None,
            bytes_written: 0,
            content_length: None,
            result: None,
            error: None,
            terminal_reason: None,
            cancellable: request.cancellable(),
            exclusive_group: request.exclusive_group(),
            started_at: now,
            updated_at: now,
        },
        restart_request: Some(request.clone()),
        events: Vec::new(),
        cancel_requested,
        pending_module_name: runtime_operation_pending_module(request),
        next_event_cursor: EventCursor::new(1),
    })
}

impl RuntimeOperationStatus {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::AwaitingExternal => "awaiting_external",
            Self::Canceling => "canceling",
            Self::Completed => "completed",
            Self::Dispatched => "dispatched",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
            Self::TimedOut => "timed_out",
        }
    }

    pub(super) const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Dispatched | Self::Failed | Self::Canceled | Self::TimedOut
        )
    }
}

pub(super) fn active_operation_in_exclusive_group(
    record: &RuntimeOperationRecord,
    group: OperationExclusiveGroup,
) -> bool {
    record.operation.exclusive_group == Some(group) && !record.operation.status.is_terminal()
}

pub(super) fn push_runtime_operation_event_locked(
    record: &mut RuntimeOperationRecord,
    phase: &str,
    message: &str,
    progress: Option<f64>,
    result: Option<Value>,
    error: Option<String>,
) -> Result<()> {
    let cursor = record.next_event_cursor;
    let next_event_cursor = cursor.next()?;
    if let Some(value) = progress {
        record.operation.progress = Some(value);
    }
    let timestamp = now_millis();
    record.operation.updated_at = timestamp;
    record.next_event_cursor = next_event_cursor;
    record.operation.event_cursor = cursor;
    record.events.push(RuntimeOperationEvent {
        cursor,
        operation_id: record.operation.operation_id.clone(),
        client_request_id: record.operation.client_request_id.clone(),
        bridge_callback_id: record.operation.bridge_callback_id.clone(),
        module_session_id: record.operation.module_session_id.clone(),
        module_request_id: record.operation.module_request_id.clone(),
        domain: record.operation.domain.clone(),
        method: record.operation.method.clone(),
        phase: phase.to_owned(),
        message: message.to_owned(),
        progress,
        result,
        error,
        timestamp,
    });
    Ok(())
}

pub(super) fn operation_progress(bytes_written: u64, content_length: Option<u64>) -> Option<f64> {
    match content_length {
        Some(total) if total > 0 => Some(bytes_written as f64 / total as f64),
        _ => None,
    }
}

pub(super) fn runtime_operation_value(operation: &RuntimeOperation) -> Value {
    let terminal_event = operation
        .terminal_event
        .as_ref()
        .map(terminal_event_contract_value);
    let mut value = json!({
        "operationId": operation.operation_id.as_str(),
        "clientRequestId": operation.client_request_id.as_ref().map(ClientRequestId::as_str),
        "bridgeCallbackId": operation.bridge_callback_id.as_ref().map(BridgeCallbackId::value),
        "moduleSessionId": operation.module_session_id.as_ref().map(ModuleSessionId::as_str),
        "moduleRequestId": operation.module_request_id.as_ref().map(ModuleRequestId::as_str),
        "externalCorrelation": Value::Null,
        "externalSessionId": operation.module_session_id.as_ref().map(ModuleSessionId::as_str),
        "requestId": operation.module_request_id.as_ref().map(ModuleRequestId::as_str),
        "eventCursor": operation.event_cursor.value(),
        "domain": operation.domain,
        "backend": operation.backend,
        "method": operation.method,
        "status": operation.status.as_str(),
        "label": operation.label,
        "policyFacts": operation.policy.as_value(),
        "terminalEventContract": terminal_event,
        "acknowledgement": operation.acknowledgement,
        "progress": operation.progress,
        "bytesWritten": operation.bytes_written,
        "contentLength": operation.content_length,
        "result": operation.result,
        "error": operation.error,
        "terminalReason": operation.terminal_reason,
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
        for key in ["cid", "path", "endpoint", "source"] {
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
        "seq": event.cursor.value(),
        "eventCursor": event.cursor.value(),
        "operationId": event.operation_id.as_str(),
        "clientRequestId": event.client_request_id.as_ref().map(ClientRequestId::as_str),
        "bridgeCallbackId": event.bridge_callback_id.as_ref().map(BridgeCallbackId::value),
        "moduleSessionId": event.module_session_id.as_ref().map(ModuleSessionId::as_str),
        "moduleRequestId": event.module_request_id.as_ref().map(ModuleRequestId::as_str),
        "externalCorrelation": Value::Null,
        "externalSessionId": event.module_session_id.as_ref().map(ModuleSessionId::as_str),
        "requestId": event.module_request_id.as_ref().map(ModuleRequestId::as_str),
        "domain": event.domain,
        "method": event.method,
        "phase": event.phase,
        "message": event.message,
        "progress": event.progress,
        "result": event.result,
        "error": event.error,
        "timestamp": event.timestamp,
    })
}

fn terminal_event_contract_value(contract: &ModuleTerminalEventContract) -> Value {
    json!({
        "moduleName": contract.module(),
        "progressEvent": contract.progress_event(),
        "successEvent": contract.success_event(),
        "failureEvent": contract.failure_event(),
        "correlation": contract.correlation().as_str(),
        "contextKey": Value::Null,
    })
}

fn exclusive_operation_message(group: OperationExclusiveGroup) -> &'static str {
    match group {
        OperationExclusiveGroup::StorageDownload => {
            "a storage download operation is already running"
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{Context as _, Result, bail};
    use serde_json::json;

    use super::*;
    use crate::{
        inspector::commands::operations::{
            outcome::RuntimeOperationOutcome, runtime_operation_request_from_value,
        },
        source_routing::{
            ModuleCorrelation, ModuleEventCorrelationKind, ModuleSessionId,
            ObservableOperationAcceptance,
        },
    };

    fn operation_id(value: &str) -> Result<RuntimeOperationId> {
        RuntimeOperationId::parse(value)
    }

    fn module_request() -> Result<RuntimeOperationRequest> {
        runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageUploadUrl",
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "path": "/tmp/runtime-operation-journal-test" }
        }))
    }

    fn insert_module_operation(registry: &RuntimeOperationRegistry, id: &str) -> Result<()> {
        registry.insert(running_runtime_operation_record(
            operation_id(id)?,
            &module_request()?,
            Arc::new(AtomicBool::new(false)),
            1,
        )?)
    }

    fn accepted_outcome(session_id: &str) -> Result<RuntimeOperationOutcome> {
        let session_id = ModuleSessionId::parse(session_id).context("module session id")?;
        Ok(RuntimeOperationOutcome::Accepted(Box::new(
            ObservableOperationAcceptance::new(
                json!({ "dispatched": true }),
                ModuleCorrelation::with_session(session_id),
                ModuleTerminalEventContract::new(
                    "storage_module",
                    Some("storageUploadProgress"),
                    "storageUploadDone",
                    Some("storageUploadError"),
                    ModuleEventCorrelationKind::Session,
                ),
            ),
        )))
    }

    fn module_event(session_id: &str, cid: &str) -> Result<ModuleEventEnvelope> {
        ModuleEventEnvelope::from_value(&json!({
            "moduleName": "storage_module",
            "eventName": "storageUploadDone",
            "args": [{ "sessionId": session_id, "cid": cid }]
        }))
    }

    #[test]
    fn runtime_record_serializes_definition_policy_facts() -> Result<()> {
        let request = RuntimeOperationRequest::from_call(
            super::super::spec::OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        let record = running_runtime_operation_record(
            operation_id("wallet-read-1")?,
            &request,
            Arc::new(AtomicBool::new(false)),
            1,
        )?;

        let value = runtime_operation_value(&record.operation);

        if value.get("policyFacts")
            != Some(&json!({
                "operationClass": "read_poll",
                "affectedInputs": [
                    { "key": "domain", "value": "wallet" },
                    { "key": "method", "value": "localWalletAccounts" }
                ],
                "restartPolicy": "safe_read_polling",
                "confirmationRequired": false,
                "provenance": ["runtime_operation_policy"]
            }))
        {
            bail!("runtime record policy facts drifted: {value}");
        }
        if record.restart_request.is_none()
            || record.operation.domain != request.domain_name()
            || record.operation.method != request.method_name()
            || record.operation.cancellable != request.cancellable()
            || record.operation.exclusive_group != request.exclusive_group()
        {
            bail!("running record facts did not originate from request: {value}");
        }
        Ok(())
    }

    #[test]
    fn running_record_keeps_valid_node_request_facts() -> Result<()> {
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageDownloadToUrl",
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://storage.local/api" }
            },
            "mutating_enabled": true,
            "payload": {
                "cid": "cid-a",
                "path": "/tmp/cid-a.bin",
                "local_only": false
            }
        }))?;
        let record = running_runtime_operation_record(
            operation_id("storage-download-1")?,
            &request,
            Arc::new(AtomicBool::new(false)),
            1,
        )?;

        if record.restart_request.is_none()
            || record.operation.backend != "rest"
            || record.operation.context
                != json!({
                    "endpoint": "http://storage.local/api",
                    "source": "network",
                    "mutatingEnabled": true,
                    "cid": "cid-a",
                    "path": "/tmp/cid-a.bin"
                })
            || !record.operation.cancellable
            || record.operation.exclusive_group != Some(OperationExclusiveGroup::StorageDownload)
        {
            bail!("node record facts did not originate from typed request");
        }
        Ok(())
    }

    #[test]
    fn runtime_record_serializes_distinct_conversation_identities() -> Result<()> {
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageUploadUrl",
            "clientRequestId": "client-1",
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "path": "/tmp/a" }
        }))?;
        let mut record = running_runtime_operation_record(
            operation_id("operation-1")?,
            &request,
            Arc::new(AtomicBool::new(false)),
            10,
        )?;
        record.operation.status = RuntimeOperationStatus::AwaitingExternal;
        record.operation.bridge_callback_id = Some(BridgeCallbackId::new(7));
        record.operation.module_session_id =
            Some(ModuleSessionId::parse("session-1").context("module session id")?);
        record.operation.module_request_id =
            Some(ModuleRequestId::parse("request-1").context("module request id")?);
        record.operation.terminal_event = Some(ModuleTerminalEventContract::new(
            "storage_module",
            Some("storageUploadProgress"),
            "storageUploadDone",
            None,
            ModuleEventCorrelationKind::Session,
        ));
        record.operation.acknowledgement = Some(json!({ "dispatched": true }));
        push_runtime_operation_event_locked(
            &mut record,
            "accepted",
            "operation accepted",
            None,
            None,
            None,
        )?;

        let value = runtime_operation_value(&record.operation);

        anyhow::ensure!(
            value.get("operationId") == Some(&json!("operation-1"))
                && value.get("clientRequestId") == Some(&json!("client-1"))
                && value.get("bridgeCallbackId") == Some(&json!(7))
                && value.get("moduleSessionId") == Some(&json!("session-1"))
                && value.get("moduleRequestId") == Some(&json!("request-1"))
                && value.get("externalSessionId") == Some(&json!("session-1"))
                && value.get("requestId") == Some(&json!("request-1"))
                && value.get("eventCursor") == Some(&json!(1))
                && value.get("status") == Some(&json!("awaiting_external"))
                && value.get("terminalEventContract")
                    == Some(&json!({
                        "moduleName": "storage_module",
                        "progressEvent": "storageUploadProgress",
                        "successEvent": "storageUploadDone",
                        "failureEvent": null,
                        "correlation": "module_session",
                        "contextKey": null
                    })),
            "runtime identity wire fields drifted: {value}"
        );
        let event = record.events.first().context("accepted event")?;
        let event_value = runtime_operation_event_value(event);
        anyhow::ensure!(
            event_value.get("eventCursor") == Some(&json!(1))
                && event_value.get("seq") == Some(&json!(1))
                && event_value.get("moduleSessionId") == Some(&json!("session-1"))
                && event_value.get("moduleRequestId") == Some(&json!("request-1")),
            "runtime event identity wire fields drifted: {event_value}"
        );
        Ok(())
    }

    #[test]
    fn registry_rejects_duplicate_operation_id_without_overwrite() -> Result<()> {
        let request = RuntimeOperationRequest::from_call(
            super::super::spec::OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        let registry = RuntimeOperationRegistry::default();
        let id = operation_id("operation-1")?;
        registry.insert(running_runtime_operation_record(
            id.clone(),
            &request,
            Arc::new(AtomicBool::new(false)),
            1,
        )?)?;
        let duplicate = running_runtime_operation_record(
            id.clone(),
            &request,
            Arc::new(AtomicBool::new(false)),
            2,
        )?;

        let Err(error) = registry.insert(duplicate) else {
            bail!("duplicate operation id was accepted");
        };

        anyhow::ensure!(
            error.to_string().contains("already exists")
                && registry.len()? == 1
                && registry
                    .value(&id)?
                    .get("startedAt")
                    .and_then(Value::as_u64)
                    == Some(1),
            "duplicate insertion overwrote the original operation"
        );
        Ok(())
    }

    #[test]
    fn event_cursor_exhaustion_rejects_event_without_partial_record_mutation() -> Result<()> {
        let request = RuntimeOperationRequest::from_call(
            super::super::spec::OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        let mut record = running_runtime_operation_record(
            operation_id("operation-1")?,
            &request,
            Arc::new(AtomicBool::new(false)),
            1,
        )?;
        record.next_event_cursor = EventCursor::new(u64::MAX);

        let Err(error) = push_runtime_operation_event_locked(
            &mut record,
            "progress",
            "operation progress",
            Some(0.5),
            None,
            None,
        ) else {
            bail!("exhausted event cursor accepted another event");
        };

        anyhow::ensure!(
            error.to_string() == "runtime operation event cursor is exhausted"
                && record.events.is_empty()
                && record.operation.event_cursor == EventCursor::new(0)
                && record.operation.progress.is_none()
                && record.operation.updated_at == 1
                && record.next_event_cursor == EventCursor::new(u64::MAX),
            "event cursor exhaustion partially mutated the record"
        );
        Ok(())
    }

    #[test]
    fn early_module_event_is_replayed_after_exact_acceptance() -> Result<()> {
        let registry = RuntimeOperationRegistry::default();
        let id = operation_id("early-event")?;
        insert_module_operation(&registry, id.as_str())?;

        let deferred = registry.ingest_module_event(module_event("session-early", "cid-early")?)?;
        anyhow::ensure!(
            deferred.get("disposition") == Some(&json!("deferred"))
                && deferred.get("operationId") == Some(&json!("early-event"))
                && deferred.get("candidateOperationIds") == Some(&json!(["early-event"]))
                && deferred
                    .get("operation")
                    .and_then(|operation| operation.get("status"))
                    == Some(&json!("running")),
            "deferred event wire evidence drifted: {deferred}"
        );
        anyhow::ensure!(registry.pending_module_event_count("storage_module")? == 1);

        registry.transition(
            &id,
            RuntimeOperationTransition::Resolved(Ok(accepted_outcome("session-early")?)),
        )?;

        let value = registry.value(&id)?;
        anyhow::ensure!(
            value.get("status") == Some(&json!("completed"))
                && value.get("result")
                    == Some(&json!({ "sessionId": "session-early", "cid": "cid-early" })),
            "early terminal event was not replayed: {value}"
        );
        anyhow::ensure!(registry.pending_module_event_count("storage_module")? == 0);
        Ok(())
    }

    #[test]
    fn concurrent_pending_operations_share_journal_until_exact_contract_matches() -> Result<()> {
        let registry = RuntimeOperationRegistry::default();
        let first = operation_id("first")?;
        let second = operation_id("second")?;
        insert_module_operation(&registry, first.as_str())?;
        insert_module_operation(&registry, second.as_str())?;

        let deferred =
            registry.ingest_module_event(module_event("session-second", "cid-second")?)?;
        anyhow::ensure!(
            deferred.get("disposition") == Some(&json!("deferred"))
                && deferred.get("operationId") == Some(&Value::Null)
                && deferred.get("candidateOperationIds") == Some(&json!(["first", "second"])),
            "concurrent candidates were not captured: {deferred}"
        );

        registry.transition(
            &first,
            RuntimeOperationTransition::Resolved(Ok(accepted_outcome("session-first")?)),
        )?;
        anyhow::ensure!(registry.pending_module_event_count("storage_module")? == 1);
        registry.transition(
            &second,
            RuntimeOperationTransition::Resolved(Ok(accepted_outcome("session-second")?)),
        )?;

        anyhow::ensure!(
            registry.value(&first)?.get("status") == Some(&json!("awaiting_external"))
                && registry.value(&second)?.get("status") == Some(&json!("completed")),
            "deferred event completed wrong pending operation"
        );
        anyhow::ensure!(registry.pending_module_event_count("storage_module")? == 0);
        Ok(())
    }

    #[test]
    fn journal_never_replays_event_to_operation_started_after_arrival() -> Result<()> {
        let registry = RuntimeOperationRegistry::default();
        let early = operation_id("early")?;
        let late = operation_id("late")?;
        insert_module_operation(&registry, early.as_str())?;
        registry.ingest_module_event(module_event("session-late", "cid-late")?)?;
        insert_module_operation(&registry, late.as_str())?;

        registry.transition(
            &early,
            RuntimeOperationTransition::Resolved(Ok(RuntimeOperationOutcome::Completed(
                json!({ "completed": true }),
            ))),
        )?;
        registry.transition(
            &late,
            RuntimeOperationTransition::Resolved(Ok(accepted_outcome("session-late")?)),
        )?;

        anyhow::ensure!(
            registry.value(&late)?.get("status") == Some(&json!("awaiting_external")),
            "journal event crossed its arrival-time candidate boundary"
        );
        anyhow::ensure!(registry.pending_module_event_count("storage_module")? == 0);
        Ok(())
    }

    #[test]
    fn journal_overflow_evicts_oldest_and_preserves_newest_terminal_event() -> Result<()> {
        let registry = RuntimeOperationRegistry::default();
        let id = operation_id("overflow")?;
        insert_module_operation(&registry, id.as_str())?;
        for index in 0..=MAX_PENDING_MODULE_EVENTS_PER_MODULE {
            registry.ingest_module_event(module_event(
                &format!("session-{index}"),
                &format!("cid-{index}"),
            )?)?;
        }
        anyhow::ensure!(
            registry.pending_module_event_count("storage_module")?
                == MAX_PENDING_MODULE_EVENTS_PER_MODULE
        );

        registry.transition(
            &id,
            RuntimeOperationTransition::Resolved(Ok(accepted_outcome("session-32")?)),
        )?;

        let value = registry.value(&id)?;
        anyhow::ensure!(
            value.get("status") == Some(&json!("completed"))
                && value.get("result")
                    == Some(&json!({ "sessionId": "session-32", "cid": "cid-32" })),
            "newest deferred terminal event was not retained: {value}"
        );
        let has_overflow_evidence = registry.inspect(|records| {
            records.get(&id).is_some_and(|record| {
                record
                    .events
                    .iter()
                    .any(|event| event.phase == "module_event_journal_overflow")
            })
        })?;
        anyhow::ensure!(
            has_overflow_evidence,
            "journal eviction had no audit evidence"
        );
        anyhow::ensure!(registry.pending_module_event_count("storage_module")? == 0);
        Ok(())
    }

    #[test]
    fn replay_preflight_failure_preserves_operation_and_journal() -> Result<()> {
        let registry = RuntimeOperationRegistry::default();
        let id = operation_id("preflight")?;
        insert_module_operation(&registry, id.as_str())?;
        registry.ingest_module_event(module_event("session-preflight", "cid-preflight")?)?;
        {
            let mut state = registry
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("registry unavailable"))?;
            state
                .records
                .get_mut(&id)
                .context("preflight record")?
                .next_event_cursor = EventCursor::new(u64::MAX);
        }

        let Err(error) = registry.transition(
            &id,
            RuntimeOperationTransition::Resolved(Ok(accepted_outcome("session-preflight")?)),
        ) else {
            bail!("exhausted replay cursor accepted transition");
        };

        let value = registry.value(&id)?;
        anyhow::ensure!(
            error.to_string() == "runtime operation event cursor is exhausted"
                && value.get("status") == Some(&json!("running"))
                && value.get("terminalEventContract") == Some(&Value::Null)
                && registry.pending_module_event_count("storage_module")? == 1,
            "replay preflight failure lost pending conversation state: {value}"
        );
        Ok(())
    }
}
