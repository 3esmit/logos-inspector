use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};
use tokio::{sync::Notify, time::Instant};

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
const MAX_PENDING_MODULE_EVENT_BYTES_PER_MODULE: usize = 256 * 1024;
const MAX_RETAINED_EVENTS_PER_OPERATION: usize = 256;
const MAX_RETAINED_EVENT_BYTES_PER_OPERATION: usize = 256 * 1024;
const MAX_INLINE_EVENT_PAYLOAD_BYTES: usize = 16 * 1024;
const MAX_RETAINED_TERMINAL_RECORDS: usize = 128;
const MAX_RETAINED_TERMINAL_PAYLOAD_BYTES: usize = 32 * 1024 * 1024;
const TERMINAL_RECORD_MAX_AGE_MILLIS: u64 = 24 * 60 * 60 * 1_000;
const PROGRESS_COALESCE_INTERVAL_MILLIS: u64 = 250;
const PROGRESS_COALESCE_DELTA: f64 = 0.01;
pub(super) const MAX_WIRE_EVENT_CURSOR: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Default)]
pub(super) struct RuntimeOperationRegistry {
    state: Arc<Mutex<RuntimeOperationRegistryState>>,
    retention_changed: Arc<Notify>,
}

#[derive(Debug, Default)]
struct RuntimeOperationRegistryState {
    records: HashMap<RuntimeOperationId, RuntimeOperationRecord>,
    pending_module_events: HashMap<String, PendingModuleEventJournal>,
    terminal_records: VecDeque<RuntimeOperationId>,
    retained_terminal_payload_bytes: usize,
}

#[derive(Debug, Clone)]
struct PendingModuleEvent {
    event: ModuleEventEnvelope,
    candidate_operation_ids: Vec<RuntimeOperationId>,
    event_bytes: usize,
}

#[derive(Debug, Clone, Default)]
struct PendingModuleEventJournal {
    events: VecDeque<PendingModuleEvent>,
    retained_bytes: usize,
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
    class: RuntimeOperationEventClass,
    redacted_payload_bytes: usize,
    serialized_bytes: usize,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeOperationRecord {
    pub(super) operation: RuntimeOperation,
    pub(super) restart_request: Option<RuntimeOperationRequest>,
    pub(super) events: VecDeque<RuntimeOperationEvent>,
    pub(super) pending_module_name: Option<&'static str>,
    pub(super) next_event_cursor: EventCursor,
    retained_event_bytes: usize,
    dropped_event_count: u64,
    coalesced_event_count: u64,
    history_truncated: bool,
    terminal_at: Option<u64>,
    terminal_expires_at: Option<Instant>,
    retained_terminal_payload_bytes: usize,
    pub(super) result_purged: bool,
    pub(super) acknowledgement_purged: bool,
    pub(super) error_redacted_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeOperationEventClass {
    Progress,
    Terminal,
    Evidence,
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

impl PendingModuleEvent {
    fn new(
        event: ModuleEventEnvelope,
        candidate_operation_ids: Vec<RuntimeOperationId>,
    ) -> Result<Self> {
        let event_bytes = event.retained_serialized_bytes()?;
        Ok(Self {
            event,
            candidate_operation_ids,
            event_bytes,
        })
    }

    fn retained_bytes(&self) -> usize {
        self.event_bytes
    }
}

impl PendingModuleEventJournal {
    fn len(&self) -> usize {
        self.events.len()
    }

    fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    fn iter(&self) -> impl Iterator<Item = &PendingModuleEvent> {
        self.events.iter()
    }

    fn pop_front(&mut self) -> Result<Option<PendingModuleEvent>> {
        let Some(pending) = self.events.pop_front() else {
            return Ok(None);
        };
        self.retained_bytes = self
            .retained_bytes
            .checked_sub(pending.retained_bytes())
            .context("pending runtime module event byte accounting underflow")?;
        Ok(Some(pending))
    }

    fn push_back(&mut self, pending: PendingModuleEvent) -> Result<()> {
        if self.events.len() >= MAX_PENDING_MODULE_EVENTS_PER_MODULE {
            bail!("pending runtime module event journal count limit exceeded");
        }
        let retained_bytes = self
            .retained_bytes
            .checked_add(pending.retained_bytes())
            .context("pending runtime module event byte count overflow")?;
        if retained_bytes > MAX_PENDING_MODULE_EVENT_BYTES_PER_MODULE {
            bail!("pending runtime module event journal byte limit exceeded");
        }
        self.events.push_back(pending);
        self.retained_bytes = retained_bytes;
        Ok(())
    }

    fn recalculate_retained_bytes(&mut self) -> Result<()> {
        self.retained_bytes = self.events.iter().try_fold(0usize, |bytes, pending| {
            bytes
                .checked_add(pending.retained_bytes())
                .context("pending runtime module event byte count overflow")
        })?;
        if self.events.len() > MAX_PENDING_MODULE_EVENTS_PER_MODULE
            || self.retained_bytes > MAX_PENDING_MODULE_EVENT_BYTES_PER_MODULE
        {
            bail!("pending runtime module event journal invariant violated");
        }
        Ok(())
    }
}

impl RuntimeOperationRegistry {
    pub(super) fn insert(&self, record: RuntimeOperationRecord) -> Result<()> {
        let now = now_millis();
        let monotonic_now = Instant::now();
        let terminal_expires_at = terminal_history_expiry(monotonic_now)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        sweep_terminal_history(&mut state, now, monotonic_now)?;
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
        state.records.insert(operation_id.clone(), record);
        let retention_changed =
            track_newly_terminal_record(&mut state, &operation_id, now, terminal_expires_at);
        sweep_terminal_history(&mut state, now, monotonic_now)?;
        drop(state);
        if retention_changed {
            self.retention_changed.notify_one();
        }
        Ok(())
    }

    pub(super) fn transition(
        &self,
        operation_id: &RuntimeOperationId,
        transition: RuntimeOperationTransition,
    ) -> Result<TransitionDisposition> {
        let now = now_millis();
        let monotonic_now = Instant::now();
        let terminal_expires_at = terminal_history_expiry(monotonic_now)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        sweep_terminal_history(&mut state, now, monotonic_now)?;
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
        let retention_changed = if result.is_ok() {
            let retention_changed =
                track_newly_terminal_record(&mut state, operation_id, now, terminal_expires_at);
            sweep_terminal_history(&mut state, now, monotonic_now)?;
            retention_changed
        } else {
            false
        };
        drop(state);
        if retention_changed {
            self.retention_changed.notify_one();
        }
        result
    }

    #[cfg(test)]
    pub(super) fn ingest_module_event(&self, event: ModuleEventEnvelope) -> Result<Value> {
        self.ingest_module_event_with_settlement(event)
            .map(|(value, _)| value)
    }

    pub(super) fn ingest_module_event_with_settlement(
        &self,
        event: ModuleEventEnvelope,
    ) -> Result<(Value, Option<RuntimeOperationId>)> {
        let now = now_millis();
        let monotonic_now = Instant::now();
        let terminal_expires_at = terminal_history_expiry(monotonic_now)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        sweep_terminal_history(&mut state, now, monotonic_now)?;
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
        let operation_id = result.operation_id().cloned();
        let retention_changed = operation_id.as_ref().is_some_and(|operation_id| {
            track_newly_terminal_record(&mut state, operation_id, now, terminal_expires_at)
        });
        let settled_operation_id = operation_id.as_ref().and_then(|operation_id| {
            state
                .records
                .get(operation_id)
                .filter(|record| record.operation.status.is_terminal())
                .map(|_| operation_id.clone())
        });
        let value = result.as_value(&state.records);
        sweep_terminal_history(&mut state, now, monotonic_now)?;
        drop(state);
        if retention_changed {
            self.retention_changed.notify_one();
        }
        Ok((value, settled_operation_id))
    }

    pub(super) fn value(&self, operation_id: &RuntimeOperationId) -> Result<Value> {
        self.inspect(|records| {
            let record = records.get(operation_id).with_context(|| {
                format!(
                    "runtime operation `{}` was not found",
                    operation_id.as_str()
                )
            })?;
            Ok(runtime_operation_record_value(record))
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
            let oldest = oldest_retained_event_cursor(record);
            let next = record.next_event_cursor;
            let stale = after
                .value()
                .checked_add(1)
                .is_none_or(|candidate| candidate < oldest.value());
            let future = after >= next;
            let reset_required = stale || future;
            let events = record
                .events
                .iter()
                .filter(|event| reset_required || event.cursor > after)
                .map(runtime_operation_event_value)
                .collect::<Vec<_>>();
            Ok(json!({
                "operation": runtime_operation_record_value(record),
                "events": events,
                "oldestSeq": oldest.value(),
                "nextSeq": next.value(),
                "eventCursor": record.operation.event_cursor.value(),
                "droppedCount": record.dropped_event_count,
                "coalescedCount": record.coalesced_event_count,
                "retainedCount": record.events.len(),
                "retainedBytes": record.retained_event_bytes,
                "historyTruncated": record.history_truncated,
                "resetRequired": reset_required,
            }))
        })?
    }

    pub(super) fn inspect<T>(
        &self,
        inspect: impl FnOnce(&HashMap<RuntimeOperationId, RuntimeOperationRecord>) -> T,
    ) -> Result<T> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        sweep_terminal_history(&mut state, now_millis(), Instant::now())?;
        Ok(inspect(&state.records))
    }

    pub(super) fn next_terminal_expiry(&self) -> Result<Option<Instant>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        sweep_terminal_history(&mut state, now_millis(), Instant::now())?;
        Ok(state
            .terminal_records
            .iter()
            .filter_map(|operation_id| state.records.get(operation_id))
            .filter_map(|record| record.terminal_expires_at)
            .min())
    }

    pub(super) fn sweep_expired_terminal_history(&self) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        sweep_terminal_history(&mut state, now_millis(), Instant::now())?;
        Ok(())
    }

    pub(super) async fn retention_changed(&self) {
        self.retention_changed.notified().await;
    }

    pub(super) fn remove(&self, operation_id: &RuntimeOperationId) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        remove_record(&mut state, operation_id)?;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> Result<usize> {
        self.inspect(HashMap::len)
    }

    #[cfg(test)]
    pub(super) fn pending_module_event_count(&self, module_name: &str) -> Result<usize> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        sweep_terminal_history(&mut state, now_millis(), Instant::now())?;
        Ok(state
            .pending_module_events
            .get(module_name)
            .map_or(0, PendingModuleEventJournal::len))
    }

    #[cfg(test)]
    pub(super) fn pending_module_event_bytes(&self, module_name: &str) -> Result<usize> {
        let state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        Ok(state
            .pending_module_events
            .get(module_name)
            .map_or(0, |journal| journal.retained_bytes))
    }

    #[cfg(test)]
    fn sweep_at(&self, now: u64) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        sweep_terminal_history(&mut state, now, Instant::now())?;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn set_terminal_expiry_after(
        &self,
        operation_id: &RuntimeOperationId,
        duration: Duration,
    ) -> Result<()> {
        let expires_at = Instant::now()
            .checked_add(duration)
            .context("test terminal history expiry overflow")?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        let record = state
            .records
            .get_mut(operation_id)
            .context("test terminal runtime operation is missing")?;
        if !record.operation.status.is_terminal() {
            bail!("test runtime operation is not terminal");
        }
        record.terminal_expires_at = Some(expires_at);
        drop(state);
        self.retention_changed.notify_one();
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn contains_without_sweep(&self, operation_id: &RuntimeOperationId) -> Result<bool> {
        let state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
        Ok(state.records.contains_key(operation_id))
    }
}

fn terminal_history_expiry(now: Instant) -> Result<Instant> {
    now.checked_add(Duration::from_millis(TERMINAL_RECORD_MAX_AGE_MILLIS))
        .context("runtime operation terminal history expiry overflow")
}

fn track_newly_terminal_record(
    state: &mut RuntimeOperationRegistryState,
    operation_id: &RuntimeOperationId,
    terminal_at: u64,
    terminal_expires_at: Instant,
) -> bool {
    let retained_payload_bytes = {
        let Some(record) = state.records.get_mut(operation_id) else {
            return false;
        };
        if !record.operation.status.is_terminal() || record.terminal_at.is_some() {
            return false;
        }
        record.terminal_at = Some(terminal_at);
        record.terminal_expires_at = Some(terminal_expires_at);
        record.restart_request = None;
        record.retained_terminal_payload_bytes =
            retained_operation_payload_bytes(&record.operation);
        record.retained_terminal_payload_bytes
    };
    state.retained_terminal_payload_bytes = state
        .retained_terminal_payload_bytes
        .saturating_add(retained_payload_bytes);
    state.terminal_records.push_back(operation_id.clone());
    true
}

fn retained_operation_payload_bytes(operation: &RuntimeOperation) -> usize {
    operation
        .result
        .as_ref()
        .map_or(0, serialized_value_bytes)
        .saturating_add(
            operation
                .acknowledgement
                .as_ref()
                .map_or(0, serialized_value_bytes),
        )
        .saturating_add(operation.error.as_ref().map_or(0, String::len))
}

fn serialized_value_bytes(value: &Value) -> usize {
    value.to_string().len()
}

fn sweep_terminal_history(
    state: &mut RuntimeOperationRegistryState,
    now: u64,
    monotonic_now: Instant,
) -> Result<()> {
    state.terminal_records.retain(|operation_id| {
        state.records.get(operation_id).is_some_and(|record| {
            record.operation.status.is_terminal() && record.terminal_at.is_some()
        })
    });
    state.retained_terminal_payload_bytes = state
        .terminal_records
        .iter()
        .filter_map(|operation_id| state.records.get(operation_id))
        .fold(0usize, |total, record| {
            total.saturating_add(record.retained_terminal_payload_bytes)
        });

    let expired_operation_ids = state
        .terminal_records
        .iter()
        .filter(|operation_id| {
            state.records.get(*operation_id).is_some_and(|record| {
                record.terminal_at.is_some_and(|terminal_at| {
                    now.saturating_sub(terminal_at) >= TERMINAL_RECORD_MAX_AGE_MILLIS
                }) || record
                    .terminal_expires_at
                    .is_some_and(|expires_at| monotonic_now >= expires_at)
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    for operation_id in expired_operation_ids {
        remove_record(state, &operation_id)?;
    }
    while state.terminal_records.len() > MAX_RETAINED_TERMINAL_RECORDS {
        evict_oldest_terminal_record(state)?;
    }
    while state.retained_terminal_payload_bytes > MAX_RETAINED_TERMINAL_PAYLOAD_BYTES {
        if !purge_oldest_terminal_payload(state) {
            state.retained_terminal_payload_bytes = 0;
            break;
        }
    }
    Ok(())
}

fn evict_oldest_terminal_record(state: &mut RuntimeOperationRegistryState) -> Result<()> {
    let Some(operation_id) = state.terminal_records.pop_front() else {
        return Ok(());
    };
    remove_record(state, &operation_id)?;
    Ok(())
}

fn purge_oldest_terminal_payload(state: &mut RuntimeOperationRegistryState) -> bool {
    let operation_id = state
        .terminal_records
        .iter()
        .find(|operation_id| {
            state.records.get(*operation_id).is_some_and(|record| {
                record.operation.result.is_some() || record.operation.acknowledgement.is_some()
            })
        })
        .cloned();
    let Some(operation_id) = operation_id else {
        return false;
    };
    let Some(record) = state.records.get_mut(&operation_id) else {
        return false;
    };
    if record.operation.acknowledgement.is_some() {
        let bytes = record
            .operation
            .acknowledgement
            .as_ref()
            .map_or(0, serialized_value_bytes);
        record.operation.acknowledgement = None;
        record.acknowledgement_purged = true;
        record.retained_terminal_payload_bytes =
            record.retained_terminal_payload_bytes.saturating_sub(bytes);
        state.retained_terminal_payload_bytes =
            state.retained_terminal_payload_bytes.saturating_sub(bytes);
        return true;
    }
    let bytes = record
        .operation
        .result
        .as_ref()
        .map_or(0, serialized_value_bytes);
    record.operation.result = None;
    record.result_purged = true;
    record.retained_terminal_payload_bytes =
        record.retained_terminal_payload_bytes.saturating_sub(bytes);
    state.retained_terminal_payload_bytes =
        state.retained_terminal_payload_bytes.saturating_sub(bytes);
    true
}

fn remove_record(
    state: &mut RuntimeOperationRegistryState,
    operation_id: &RuntimeOperationId,
) -> Result<Option<RuntimeOperationRecord>> {
    remove_operation_from_pending_module_events(state, operation_id)?;
    state
        .terminal_records
        .retain(|candidate| candidate != operation_id);
    let removed = state.records.remove(operation_id);
    if let Some(record) = &removed {
        state.retained_terminal_payload_bytes = state
            .retained_terminal_payload_bytes
            .saturating_sub(record.retained_terminal_payload_bytes);
    }
    Ok(removed)
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
    let pending = PendingModuleEvent::new(event, candidate_operation_ids.clone())?;
    if pending.retained_bytes() > MAX_PENDING_MODULE_EVENT_BYTES_PER_MODULE {
        bail!(
            "runtime module event requires {} retained bytes, exceeding the {}-byte early-event journal limit",
            pending.retained_bytes(),
            MAX_PENDING_MODULE_EVENT_BYTES_PER_MODULE
        );
    }
    let mut next_journal = state
        .pending_module_events
        .get(&module_name)
        .cloned()
        .unwrap_or_default();
    let mut evicted = Vec::new();
    loop {
        let retained_bytes = next_journal
            .retained_bytes
            .checked_add(pending.retained_bytes())
            .context("pending runtime module event byte count overflow")?;
        if next_journal.len() < MAX_PENDING_MODULE_EVENTS_PER_MODULE
            && retained_bytes <= MAX_PENDING_MODULE_EVENT_BYTES_PER_MODULE
        {
            break;
        }
        let removed = next_journal
            .pop_front()?
            .context("pending runtime module event journal cannot satisfy its byte limit")?;
        evicted.push(removed);
    }
    let mut evidence_counts = HashMap::new();
    for operation_id in &candidate_operation_ids {
        let count = evidence_counts
            .entry(operation_id.clone())
            .or_insert(0usize);
        *count = count
            .checked_add(1)
            .context("pending runtime module event evidence count overflow")?;
    }
    for evicted_event in &evicted {
        for operation_id in &evicted_event.candidate_operation_ids {
            let count = evidence_counts
                .entry(operation_id.clone())
                .or_insert(0usize);
            *count = count
                .checked_add(1)
                .context("pending runtime module event evidence count overflow")?;
        }
    }
    ensure_event_cursor_capacity(&state.records, &evidence_counts)?;

    let mut staged_records = evidence_counts
        .keys()
        .filter_map(|operation_id| {
            state
                .records
                .get(operation_id)
                .cloned()
                .map(|record| (operation_id.clone(), record))
        })
        .collect::<HashMap<_, _>>();
    for evicted_event in &evicted {
        for operation_id in &evicted_event.candidate_operation_ids {
            let Some(record) = staged_records.get_mut(operation_id) else {
                continue;
            };
            push_runtime_operation_event_locked(
                record,
                "module_event_journal_overflow",
                "oldest deferred module event evicted before correlation registration",
                None,
                Some(evicted_event.event.result()),
                evicted_event.event.error(),
            )?;
        }
    }
    for operation_id in &candidate_operation_ids {
        let record = staged_records
            .get_mut(operation_id)
            .ok_or_else(|| anyhow::anyhow!("pending runtime operation disappeared"))?;
        push_runtime_operation_event_locked(
            record,
            "module_event_deferred",
            "module event arrived before dispatch correlation registration",
            None,
            Some(pending.event.result()),
            pending.event.error(),
        )?;
    }
    next_journal.push_back(pending)?;
    for (operation_id, record) in staged_records {
        state.records.insert(operation_id, record);
    }
    state
        .pending_module_events
        .insert(module_name, next_journal);
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
            cursor = checked_next_event_cursor(cursor)?;
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
        cursor = checked_next_event_cursor(cursor)?;
    }
    Ok(())
}

fn transition_settles_pending_module(transition: &RuntimeOperationTransition) -> bool {
    matches!(
        transition,
        RuntimeOperationTransition::Resolved(_)
            | RuntimeOperationTransition::ExecutionFailed { .. }
            | RuntimeOperationTransition::CancellationConfirmed { .. }
            | RuntimeOperationTransition::TimedOut { .. }
            | RuntimeOperationTransition::Shutdown { .. }
            | RuntimeOperationTransition::CleanupFailed { .. }
            | RuntimeOperationTransition::TaskPanicked { .. }
            | RuntimeOperationTransition::TaskAborted { .. }
            | RuntimeOperationTransition::AdapterClosed { .. }
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
        .map_or(0, |journal| {
            journal
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
    let Some(pending_journal) = state.pending_module_events.remove(module_name) else {
        return Ok(());
    };
    let mut pending_events = pending_journal.events;
    let record = state
        .records
        .get_mut(operation_id)
        .ok_or_else(|| anyhow::anyhow!("pending runtime operation disappeared during replay"))?;
    let mut retained_events = PendingModuleEventJournal::default();
    while let Some(mut pending) = pending_events.pop_front() {
        if !pending.candidate_operation_ids.contains(operation_id) {
            retained_events.push_back(pending)?;
            continue;
        }
        match apply_deferred_module_event(record, &pending.event) {
            Ok(true) => {}
            Ok(false) => {
                pending
                    .candidate_operation_ids
                    .retain(|candidate| candidate != operation_id);
                if !pending.candidate_operation_ids.is_empty() {
                    retained_events.push_back(pending)?;
                }
            }
            Err(error) => {
                retained_events.push_back(pending)?;
                for pending in pending_events {
                    retained_events.push_back(pending)?;
                }
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
) -> Result<()> {
    let mut next_pending_module_events = state.pending_module_events.clone();
    for pending_events in next_pending_module_events.values_mut() {
        for pending in &mut pending_events.events {
            pending
                .candidate_operation_ids
                .retain(|candidate| candidate != operation_id);
        }
        pending_events
            .events
            .retain(|pending| !pending.candidate_operation_ids.is_empty());
        pending_events.recalculate_retained_bytes()?;
    }
    next_pending_module_events.retain(|_, pending_events| !pending_events.is_empty());
    state.pending_module_events = next_pending_module_events;
    Ok(())
}

pub(super) fn running_runtime_operation_record(
    operation_id: RuntimeOperationId,
    request: &RuntimeOperationRequest,
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
        events: VecDeque::with_capacity(MAX_RETAINED_EVENTS_PER_OPERATION),
        pending_module_name: runtime_operation_pending_module(request),
        next_event_cursor: EventCursor::new(1),
        retained_event_bytes: 0,
        dropped_event_count: 0,
        coalesced_event_count: 0,
        history_truncated: false,
        terminal_at: None,
        terminal_expires_at: None,
        retained_terminal_payload_bytes: 0,
        result_purged: false,
        acknowledgement_purged: false,
        error_redacted_bytes: 0,
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
    let timestamp = now_millis().max(record.operation.updated_at);
    let progress = progress.map(|candidate| {
        record
            .operation
            .progress
            .map_or(candidate, |previous| previous.max(candidate))
    });
    let class = runtime_operation_event_class(phase);
    if class == RuntimeOperationEventClass::Evidence
        && record.operation.status.is_terminal()
        && record
            .events
            .iter()
            .any(|event| event.class == RuntimeOperationEventClass::Terminal)
    {
        record.operation.updated_at = timestamp;
        record.dropped_event_count = increment_wire_counter(record.dropped_event_count);
        record.history_truncated = true;
        return Ok(());
    }
    if class == RuntimeOperationEventClass::Progress
        && let Some(previous) = record.events.back()
        && should_coalesce_progress(previous, progress, timestamp)
    {
        if let Some(value) = progress {
            record.operation.progress = Some(value);
        }
        record.operation.updated_at = timestamp;
        record.coalesced_event_count = increment_wire_counter(record.coalesced_event_count);
        record.history_truncated = true;
        return Ok(());
    }

    let (result, result_redacted_bytes) = bounded_inline_event_result(
        result,
        event_payload_location(phase).unwrap_or("event.result"),
    );
    let (error, error_redacted_bytes) = bounded_inline_event_error(error);
    let redacted_payload_bytes = result_redacted_bytes.saturating_add(error_redacted_bytes);

    let cursor = record.next_event_cursor;
    let next_event_cursor = checked_next_event_cursor(cursor)?;
    let mut event = RuntimeOperationEvent {
        cursor,
        operation_id: record.operation.operation_id.clone(),
        client_request_id: record.operation.client_request_id.clone(),
        bridge_callback_id: record.operation.bridge_callback_id,
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
        class,
        redacted_payload_bytes,
        serialized_bytes: 0,
    };
    finalize_event_size(&mut event)?;
    let event_was_redacted = event.redacted_payload_bytes > 0;
    if let Some(value) = progress {
        record.operation.progress = Some(value);
    }
    record.operation.updated_at = timestamp;
    record.next_event_cursor = next_event_cursor;
    record.operation.event_cursor = cursor;
    record.retained_event_bytes = record
        .retained_event_bytes
        .saturating_add(event.serialized_bytes);
    if event_was_redacted {
        record.history_truncated = true;
    }
    record.events.push_back(event);
    enforce_event_retention(record);
    Ok(())
}

fn checked_next_event_cursor(cursor: EventCursor) -> Result<EventCursor> {
    let next = cursor.next()?;
    if next.value() > MAX_WIRE_EVENT_CURSOR {
        bail!("runtime operation event cursor exceeds JavaScript safe integer range");
    }
    Ok(next)
}

fn runtime_operation_event_class(phase: &str) -> RuntimeOperationEventClass {
    match phase {
        "progress" | "external_progress" => RuntimeOperationEventClass::Progress,
        "completed"
        | "external_completion"
        | "external_failure"
        | "dispatched"
        | "failed"
        | "canceled"
        | "timed_out" => RuntimeOperationEventClass::Terminal,
        _ => RuntimeOperationEventClass::Evidence,
    }
}

fn should_coalesce_progress(
    previous: &RuntimeOperationEvent,
    progress: Option<f64>,
    timestamp: u64,
) -> bool {
    if previous.class != RuntimeOperationEventClass::Progress {
        return false;
    }
    let elapsed = timestamp.saturating_sub(previous.timestamp);
    let delta = match (previous.progress, progress) {
        (Some(previous), Some(current)) => (current - previous).abs(),
        (None, None) => 0.0,
        (Some(_), None) | (None, Some(_)) => f64::INFINITY,
    };
    elapsed < PROGRESS_COALESCE_INTERVAL_MILLIS && delta < PROGRESS_COALESCE_DELTA
}

fn bounded_inline_event_result(
    result: Option<Value>,
    payload_location: &'static str,
) -> (Option<Value>, usize) {
    let Some(result) = result else {
        return (None, 0);
    };
    let bytes = serialized_value_bytes(&result);
    if bytes <= MAX_INLINE_EVENT_PAYLOAD_BYTES {
        return (Some(result), 0);
    }
    (
        Some(json!({
            "redacted": true,
            "reason": "inline_payload_limit",
            "originalBytes": bytes,
            "limitBytes": MAX_INLINE_EVENT_PAYLOAD_BYTES,
            "payloadLocation": payload_location,
        })),
        bytes,
    )
}

fn bounded_inline_event_error(error: Option<String>) -> (Option<String>, usize) {
    let Some(error) = error else {
        return (None, 0);
    };
    let bytes = error.len();
    if bytes <= MAX_INLINE_EVENT_PAYLOAD_BYTES {
        return (Some(error), 0);
    }
    (
        Some(format!(
            "runtime operation evidence redacted: {bytes} bytes exceeded {MAX_INLINE_EVENT_PAYLOAD_BYTES}-byte inline limit"
        )),
        bytes,
    )
}

pub(super) fn bounded_terminal_operation_error(error: Option<String>) -> (Option<String>, usize) {
    let Some(error) = error else {
        return (None, 0);
    };
    let bytes = error.len();
    if bytes <= MAX_INLINE_EVENT_PAYLOAD_BYTES {
        return (Some(error), 0);
    }
    (
        Some(format!(
            "runtime operation error redacted: {bytes} bytes exceeded {MAX_INLINE_EVENT_PAYLOAD_BYTES}-byte retained limit"
        )),
        bytes,
    )
}

fn serialized_event_bytes(event: &RuntimeOperationEvent) -> Result<usize> {
    serde_json::to_vec(&runtime_operation_event_value(event))
        .map(|value| value.len())
        .context("runtime operation event could not be measured")
}

fn finalize_event_size(event: &mut RuntimeOperationEvent) -> Result<()> {
    event.serialized_bytes = serialized_event_bytes(event)?;
    if event.serialized_bytes <= MAX_RETAINED_EVENT_BYTES_PER_OPERATION {
        return Ok(());
    }

    let original_bytes = event.serialized_bytes;
    event.client_request_id = None;
    event.bridge_callback_id = None;
    event.module_session_id = None;
    event.module_request_id = None;
    event.result = Some(json!({
        "redacted": true,
        "reason": "event_metadata_limit",
        "originalBytes": original_bytes,
        "limitBytes": MAX_RETAINED_EVENT_BYTES_PER_OPERATION,
        "payloadLocation": event_payload_location(&event.phase).unwrap_or("event.metadata"),
    }));
    event.error = None;
    event.redacted_payload_bytes = event.redacted_payload_bytes.saturating_add(original_bytes);
    event.serialized_bytes = serialized_event_bytes(event)?;
    if event.serialized_bytes > MAX_RETAINED_EVENT_BYTES_PER_OPERATION {
        bail!("runtime operation event metadata exceeds retained byte limit");
    }
    Ok(())
}

fn enforce_event_retention(record: &mut RuntimeOperationRecord) {
    while record.events.len() > MAX_RETAINED_EVENTS_PER_OPERATION
        || record.retained_event_bytes > MAX_RETAINED_EVENT_BYTES_PER_OPERATION
    {
        let removed = record.events.pop_front();
        let Some(removed) = removed else {
            break;
        };
        record.retained_event_bytes = record
            .retained_event_bytes
            .saturating_sub(removed.serialized_bytes);
        record.dropped_event_count = increment_wire_counter(record.dropped_event_count);
        record.history_truncated = true;
    }
}

fn increment_wire_counter(value: u64) -> u64 {
    value.saturating_add(1).min(MAX_WIRE_EVENT_CURSOR)
}

fn oldest_retained_event_cursor(record: &RuntimeOperationRecord) -> EventCursor {
    record
        .events
        .front()
        .map_or(record.next_event_cursor, |event| event.cursor)
}

pub(super) fn operation_progress(bytes_written: u64, content_length: Option<u64>) -> Option<f64> {
    match content_length {
        Some(total) if total > 0 => Some((bytes_written as f64 / total as f64).min(1.0)),
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

pub(super) fn runtime_operation_record_value(record: &RuntimeOperationRecord) -> Value {
    let mut value = runtime_operation_value(&record.operation);
    let oldest = oldest_retained_event_cursor(record);
    if let Value::Object(target) = &mut value {
        target.insert("oldestSeq".to_owned(), json!(oldest.value()));
        target.insert(
            "nextSeq".to_owned(),
            json!(record.next_event_cursor.value()),
        );
        target.insert("droppedCount".to_owned(), json!(record.dropped_event_count));
        target.insert(
            "coalescedCount".to_owned(),
            json!(record.coalesced_event_count),
        );
        target.insert("retainedCount".to_owned(), json!(record.events.len()));
        target.insert(
            "retainedBytes".to_owned(),
            json!(record.retained_event_bytes),
        );
        target.insert(
            "historyTruncated".to_owned(),
            json!(record.history_truncated),
        );
        target.insert("terminalAt".to_owned(), json!(record.terminal_at));
        target.insert("resultPurged".to_owned(), json!(record.result_purged));
        target.insert(
            "acknowledgementPurged".to_owned(),
            json!(record.acknowledgement_purged),
        );
        target.insert(
            "retainedPayloadBytes".to_owned(),
            json!(record.retained_terminal_payload_bytes),
        );
        target.insert(
            "errorRedacted".to_owned(),
            json!(record.error_redacted_bytes > 0),
        );
        target.insert(
            "redactedErrorBytes".to_owned(),
            json!(record.error_redacted_bytes),
        );
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
        "payloadLocation": event_payload_location(event.phase.as_str()),
        "progress": event.progress,
        "result": event.result,
        "error": event.error,
        "timestamp": event.timestamp,
        "payloadRedacted": event.redacted_payload_bytes > 0,
        "redactedPayloadBytes": event.redacted_payload_bytes,
    })
}

fn event_payload_location(phase: &str) -> Option<&'static str> {
    match phase {
        "accepted" | "dispatched" => Some("operation.acknowledgement"),
        "completed" | "external_completion" => Some("operation.result"),
        "failed" | "external_failure" | "canceled" | "timed_out" => Some("operation.error"),
        _ => None,
    }
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
        let record = running_runtime_operation_record(operation_id("wallet-read-1")?, &request, 1)?;

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
        let record =
            running_runtime_operation_record(operation_id("storage-download-1")?, &request, 1)?;

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
        let mut record =
            running_runtime_operation_record(operation_id("operation-1")?, &request, 10)?;
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
        let event = record.events.front().context("accepted event")?;
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
        registry.insert(running_runtime_operation_record(id.clone(), &request, 1)?)?;
        let duplicate = running_runtime_operation_record(id.clone(), &request, 2)?;

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
        let mut record =
            running_runtime_operation_record(operation_id("operation-1")?, &request, 1)?;
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
        anyhow::ensure!(registry.pending_module_event_bytes("storage_module")? > 0);

        registry.transition(
            &id,
            RuntimeOperationTransition::Resolved(accepted_outcome("session-early")?),
        )?;

        let value = registry.value(&id)?;
        anyhow::ensure!(
            value.get("status") == Some(&json!("completed"))
                && value.get("result")
                    == Some(&json!({ "sessionId": "session-early", "cid": "cid-early" })),
            "early terminal event was not replayed: {value}"
        );
        anyhow::ensure!(registry.pending_module_event_count("storage_module")? == 0);
        anyhow::ensure!(registry.pending_module_event_bytes("storage_module")? == 0);
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
            RuntimeOperationTransition::Resolved(accepted_outcome("session-first")?),
        )?;
        anyhow::ensure!(registry.pending_module_event_count("storage_module")? == 1);
        registry.transition(
            &second,
            RuntimeOperationTransition::Resolved(accepted_outcome("session-second")?),
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
            RuntimeOperationTransition::Resolved(RuntimeOperationOutcome::Completed(
                json!({ "completed": true }),
            )),
        )?;
        registry.transition(
            &late,
            RuntimeOperationTransition::Resolved(accepted_outcome("session-late")?),
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
        for index in 0..MAX_PENDING_MODULE_EVENTS_PER_MODULE {
            registry.ingest_module_event(module_event(
                &format!("session-{index}"),
                &format!("cid-{index}"),
            )?)?;
        }
        anyhow::ensure!(
            registry.pending_module_event_count("storage_module")?
                == MAX_PENDING_MODULE_EVENTS_PER_MODULE,
            "journal rejected its exact count boundary"
        );
        let newest_index = MAX_PENDING_MODULE_EVENTS_PER_MODULE;
        let newest_session = format!("session-{newest_index}");
        let newest_cid = format!("cid-{newest_index}");
        registry.ingest_module_event(module_event(&newest_session, &newest_cid)?)?;
        anyhow::ensure!(
            registry.pending_module_event_count("storage_module")?
                == MAX_PENDING_MODULE_EVENTS_PER_MODULE
        );
        let retained_bytes = registry.pending_module_event_bytes("storage_module")?;
        anyhow::ensure!(
            retained_bytes > 0 && retained_bytes <= MAX_PENDING_MODULE_EVENT_BYTES_PER_MODULE
        );
        let recomputed_bytes = registry
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("registry unavailable"))?
            .pending_module_events
            .get("storage_module")
            .context("overflow journal")?
            .events
            .iter()
            .try_fold(0usize, |bytes, pending| {
                bytes
                    .checked_add(pending.event.retained_serialized_bytes()?)
                    .context("test journal byte count overflow")
            })?;
        anyhow::ensure!(recomputed_bytes == retained_bytes);

        registry.transition(
            &id,
            RuntimeOperationTransition::Resolved(accepted_outcome(&newest_session)?),
        )?;

        let value = registry.value(&id)?;
        anyhow::ensure!(
            value.get("status") == Some(&json!("completed"))
                && value.get("result")
                    == Some(&json!({ "sessionId": newest_session, "cid": newest_cid })),
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
        anyhow::ensure!(registry.pending_module_event_bytes("storage_module")? == 0);
        Ok(())
    }

    #[test]
    fn journal_byte_pressure_evicts_a_prefix_and_replays_the_newest_event() -> Result<()> {
        let registry = RuntimeOperationRegistry::default();
        let id = operation_id("byte-pressure")?;
        insert_module_operation(&registry, id.as_str())?;
        let cid_payload = "x".repeat(MAX_PENDING_MODULE_EVENT_BYTES_PER_MODULE / 16);
        for index in 0..16 {
            registry.ingest_module_event(module_event(
                &format!("byte-session-{index}"),
                &format!("{index}-{cid_payload}"),
            )?)?;
        }

        let retained_count = registry.pending_module_event_count("storage_module")?;
        let retained_bytes = registry.pending_module_event_bytes("storage_module")?;
        anyhow::ensure!(
            retained_count > 0
                && retained_count < 16
                && retained_bytes <= MAX_PENDING_MODULE_EVENT_BYTES_PER_MODULE,
            "byte-pressure journal exceeded its limits: {retained_count} events, {retained_bytes} bytes"
        );

        registry.transition(
            &id,
            RuntimeOperationTransition::Resolved(accepted_outcome("byte-session-15")?),
        )?;
        let value = registry.value(&id)?;
        anyhow::ensure!(
            value.get("status") == Some(&json!("completed"))
                && value
                    .get("result")
                    .and_then(|result| result.get("cid"))
                    .and_then(Value::as_str)
                    .is_some_and(|cid| cid.starts_with("15-")),
            "byte-pressure journal lost its newest terminal evidence"
        );
        anyhow::ensure!(registry.pending_module_event_count("storage_module")? == 0);
        anyhow::ensure!(registry.pending_module_event_bytes("storage_module")? == 0);
        Ok(())
    }

    #[test]
    fn oversized_early_event_is_rejected_without_record_or_journal_mutation() -> Result<()> {
        let registry = RuntimeOperationRegistry::default();
        let id = operation_id("oversized-early-event")?;
        insert_module_operation(&registry, id.as_str())?;
        let before = registry.value(&id)?;
        let event = module_event(
            "oversized-session",
            &"x".repeat(MAX_PENDING_MODULE_EVENT_BYTES_PER_MODULE),
        )?;
        anyhow::ensure!(
            event.retained_serialized_bytes()? > MAX_PENDING_MODULE_EVENT_BYTES_PER_MODULE
        );

        let Err(error) = registry.ingest_module_event(event) else {
            bail!("oversized early module event was retained");
        };

        anyhow::ensure!(
            error.to_string().contains("early-event journal limit")
                && registry.pending_module_event_count("storage_module")? == 0
                && registry.pending_module_event_bytes("storage_module")? == 0
                && registry.value(&id)? == before,
            "oversized early event changed bounded conversation state: {error:#}"
        );
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
            RuntimeOperationTransition::Resolved(accepted_outcome("session-preflight")?),
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

    #[test]
    fn ten_thousand_progress_updates_keep_a_bounded_contiguous_terminal_window() -> Result<()> {
        let registry = RuntimeOperationRegistry::default();
        let id = operation_id("bounded-progress")?;
        let request = RuntimeOperationRequest::from_call(
            super::super::spec::OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        registry.insert(running_runtime_operation_record(id.clone(), &request, 1)?)?;
        registry.transition(&id, RuntimeOperationTransition::Started)?;
        for bytes_written in 1..=10_000 {
            registry.transition(
                &id,
                RuntimeOperationTransition::Progress {
                    bytes_written,
                    content_length: Some(10_000),
                },
            )?;
        }
        registry.transition(
            &id,
            RuntimeOperationTransition::Resolved(RuntimeOperationOutcome::Completed(
                json!({ "complete": true }),
            )),
        )?;

        registry.inspect(|records| -> Result<()> {
            let record = records.get(&id).context("bounded progress record")?;
            anyhow::ensure!(record.events.len() <= MAX_RETAINED_EVENTS_PER_OPERATION);
            anyhow::ensure!(record.retained_event_bytes <= MAX_RETAINED_EVENT_BYTES_PER_OPERATION);
            anyhow::ensure!(record.coalesced_event_count > 0);
            anyhow::ensure!(record.restart_request.is_none());
            anyhow::ensure!(record.operation.result == Some(json!({ "complete": true })));
            let terminal = record.events.back().context("terminal event")?;
            anyhow::ensure!(terminal.class == RuntimeOperationEventClass::Terminal);
            anyhow::ensure!(terminal.result.is_none());
            anyhow::ensure!(event_payload_location(&terminal.phase) == Some("operation.result"));
            let mut expected = oldest_retained_event_cursor(record).value();
            for event in &record.events {
                anyhow::ensure!(event.cursor.value() == expected);
                expected = expected
                    .checked_add(1)
                    .context("retained cursor overflow")?;
            }
            anyhow::ensure!(expected == record.next_event_cursor.value());
            Ok(())
        })??;
        Ok(())
    }

    #[test]
    fn coalesced_progress_keeps_emitted_event_immutable_until_cumulative_threshold() -> Result<()> {
        let request = RuntimeOperationRequest::from_call(
            super::super::spec::OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        let mut record =
            running_runtime_operation_record(operation_id("coalesced-progress")?, &request, 1)?;
        push_runtime_operation_event_locked(
            &mut record,
            "progress",
            "operation progress",
            Some(0.0),
            None,
            None,
        )?;
        let future_timestamp = now_millis().saturating_add(60_000);
        record.operation.updated_at = future_timestamp;
        record
            .events
            .back_mut()
            .context("initial progress event")?
            .timestamp = future_timestamp;
        let emitted =
            runtime_operation_event_value(record.events.front().context("initial progress event")?);

        push_runtime_operation_event_locked(
            &mut record,
            "progress",
            "operation progress",
            Some(0.005),
            None,
            None,
        )?;
        anyhow::ensure!(record.events.len() == 1);
        anyhow::ensure!(record.next_event_cursor == EventCursor::new(2));
        anyhow::ensure!(record.operation.event_cursor == EventCursor::new(1));
        anyhow::ensure!(record.operation.progress == Some(0.005));
        anyhow::ensure!(record.operation.updated_at == future_timestamp);
        anyhow::ensure!(record.coalesced_event_count == 1);
        anyhow::ensure!(
            runtime_operation_event_value(
                record.events.front().context("immutable progress event")?
            ) == emitted
        );

        push_runtime_operation_event_locked(
            &mut record,
            "progress",
            "operation progress",
            Some(0.02),
            None,
            None,
        )?;
        anyhow::ensure!(record.events.len() == 2);
        anyhow::ensure!(record.operation.event_cursor == EventCursor::new(2));
        anyhow::ensure!(record.next_event_cursor == EventCursor::new(3));
        Ok(())
    }

    #[test]
    fn progress_snapshot_never_regresses_at_the_same_cursor() -> Result<()> {
        let registry = RuntimeOperationRegistry::default();
        let id = operation_id("monotonic-progress")?;
        let request = RuntimeOperationRequest::from_call(
            super::super::spec::OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        registry.insert(running_runtime_operation_record(id.clone(), &request, 1)?)?;
        registry.transition(
            &id,
            RuntimeOperationTransition::Progress {
                bytes_written: 150,
                content_length: Some(100),
            },
        )?;
        let first = registry.value(&id)?;
        registry.transition(
            &id,
            RuntimeOperationTransition::Progress {
                bytes_written: 50,
                content_length: Some(200),
            },
        )?;
        let second = registry.value(&id)?;

        anyhow::ensure!(second.get("bytesWritten") == Some(&json!(150)));
        anyhow::ensure!(second.get("contentLength") == Some(&json!(200)));
        anyhow::ensure!(second.get("progress") == Some(&json!(1.0)));
        anyhow::ensure!(second.get("eventCursor") == first.get("eventCursor"));
        anyhow::ensure!(
            second.get("updatedAt").and_then(Value::as_u64)
                >= first.get("updatedAt").and_then(Value::as_u64)
        );
        Ok(())
    }

    #[test]
    fn terminal_event_survives_progress_prefix_eviction() -> Result<()> {
        let request = RuntimeOperationRequest::from_call(
            super::super::spec::OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        let mut record =
            running_runtime_operation_record(operation_id("terminal-pressure")?, &request, 1)?;
        for index in 0..(MAX_RETAINED_EVENTS_PER_OPERATION + 32) {
            push_runtime_operation_event_locked(
                &mut record,
                "progress",
                "operation progress",
                Some(index as f64 * 0.02),
                None,
                None,
            )?;
        }
        push_runtime_operation_event_locked(
            &mut record,
            "completed",
            "operation completed",
            Some(1.0),
            None,
            None,
        )?;

        anyhow::ensure!(record.events.len() == MAX_RETAINED_EVENTS_PER_OPERATION);
        anyhow::ensure!(record.dropped_event_count > 0);
        anyhow::ensure!(oldest_retained_event_cursor(&record).value() > 1);
        anyhow::ensure!(
            record
                .events
                .back()
                .is_some_and(|event| event.class == RuntimeOperationEventClass::Terminal)
        );
        Ok(())
    }

    #[test]
    fn post_terminal_evidence_pressure_keeps_terminal_marker_and_contiguous_cursor() -> Result<()> {
        let registry = RuntimeOperationRegistry::default();
        let id = operation_id("terminal-stale-pressure")?;
        let request = RuntimeOperationRequest::from_call(
            super::super::spec::OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        registry.insert(running_runtime_operation_record(id.clone(), &request, 1)?)?;
        registry.transition(
            &id,
            RuntimeOperationTransition::Resolved(RuntimeOperationOutcome::Completed(
                json!({ "complete": true }),
            )),
        )?;
        for index in 0..(MAX_RETAINED_EVENTS_PER_OPERATION + 32) {
            let disposition = registry.transition(
                &id,
                RuntimeOperationTransition::Progress {
                    bytes_written: index as u64,
                    content_length: Some(MAX_RETAINED_EVENTS_PER_OPERATION as u64 + 32),
                },
            )?;
            anyhow::ensure!(disposition == TransitionDisposition::Stale);
        }

        let response = registry.events(&id, EventCursor::new(0))?;
        anyhow::ensure!(response.get("eventCursor") == Some(&json!(1)));
        anyhow::ensure!(response.get("nextSeq") == Some(&json!(2)));
        anyhow::ensure!(response.get("oldestSeq") == Some(&json!(1)));
        anyhow::ensure!(response.get("retainedCount") == Some(&json!(1)));
        anyhow::ensure!(response.get("historyTruncated") == Some(&json!(true)));
        anyhow::ensure!(
            response.get("droppedCount") == Some(&json!(MAX_RETAINED_EVENTS_PER_OPERATION + 32))
        );
        let terminal = response
            .get("events")
            .and_then(Value::as_array)
            .and_then(|events| events.first())
            .context("retained terminal marker")?;
        anyhow::ensure!(terminal.get("phase") == Some(&json!("completed")));
        anyhow::ensure!(terminal.get("payloadLocation") == Some(&json!("operation.result")));
        Ok(())
    }

    #[test]
    fn large_terminal_error_is_bounded_and_owned_only_by_operation_record() -> Result<()> {
        let registry = RuntimeOperationRegistry::default();
        let id = operation_id("terminal-error")?;
        let request = RuntimeOperationRequest::from_call(
            super::super::spec::OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        registry.insert(running_runtime_operation_record(id.clone(), &request, 1)?)?;
        let original_error_bytes = MAX_INLINE_EVENT_PAYLOAD_BYTES + 1;
        registry.transition(
            &id,
            RuntimeOperationTransition::ExecutionFailed {
                error: "x".repeat(original_error_bytes),
            },
        )?;

        let value = registry.value(&id)?;
        anyhow::ensure!(value.get("errorRedacted") == Some(&json!(true)));
        anyhow::ensure!(value.get("redactedErrorBytes") == Some(&json!(original_error_bytes)));
        anyhow::ensure!(
            value
                .get("error")
                .and_then(Value::as_str)
                .is_some_and(|error| error.contains("error redacted"))
        );
        registry.inspect(|records| -> Result<()> {
            let record = records.get(&id).context("terminal error record")?;
            let terminal = record.events.back().context("terminal error marker")?;
            anyhow::ensure!(terminal.error.is_none());
            anyhow::ensure!(terminal.result.is_none());
            anyhow::ensure!(event_payload_location(&terminal.phase) == Some("operation.error"));
            anyhow::ensure!(terminal.serialized_bytes <= MAX_RETAINED_EVENT_BYTES_PER_OPERATION);
            Ok(())
        })??;
        Ok(())
    }

    #[test]
    fn event_window_reports_stale_and_future_cursor_resets() -> Result<()> {
        let registry = RuntimeOperationRegistry::default();
        let id = operation_id("cursor-window")?;
        insert_module_operation(&registry, id.as_str())?;
        {
            let mut state = registry
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("registry unavailable"))?;
            let record = state.records.get_mut(&id).context("cursor record")?;
            for index in 0..(MAX_RETAINED_EVENTS_PER_OPERATION + 32) {
                push_runtime_operation_event_locked(
                    record,
                    "audit",
                    &format!("audit event {index}"),
                    None,
                    None,
                    None,
                )?;
            }
        }

        let reset = registry.events(&id, EventCursor::new(0))?;
        let oldest = reset
            .get("oldestSeq")
            .and_then(Value::as_u64)
            .context("oldest sequence")?;
        let next = reset
            .get("nextSeq")
            .and_then(Value::as_u64)
            .context("next sequence")?;
        anyhow::ensure!(oldest > 1);
        anyhow::ensure!(reset.get("resetRequired") == Some(&json!(true)));
        anyhow::ensure!(
            reset.get("events").and_then(Value::as_array).map(Vec::len)
                == Some(MAX_RETAINED_EVENTS_PER_OPERATION)
        );

        let boundary = registry.events(&id, EventCursor::new(oldest - 1))?;
        anyhow::ensure!(boundary.get("resetRequired") == Some(&json!(false)));
        let future = registry.events(&id, EventCursor::new(next))?;
        anyhow::ensure!(future.get("resetRequired") == Some(&json!(true)));
        anyhow::ensure!(future.get("events") == reset.get("events"));
        Ok(())
    }

    #[test]
    fn oversized_event_evidence_is_replaced_by_a_location_marker() -> Result<()> {
        let request = RuntimeOperationRequest::from_call(
            super::super::spec::OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        let mut record =
            running_runtime_operation_record(operation_id("redacted-evidence")?, &request, 1)?;
        push_runtime_operation_event_locked(
            &mut record,
            "audit",
            "oversized evidence",
            None,
            Some(json!({ "payload": "x".repeat(MAX_INLINE_EVENT_PAYLOAD_BYTES + 1) })),
            None,
        )?;

        let event = record.events.front().context("redacted event")?;
        let value = runtime_operation_event_value(event);
        anyhow::ensure!(value.get("payloadRedacted") == Some(&json!(true)));
        anyhow::ensure!(
            value.get("result").and_then(|result| result.get("reason"))
                == Some(&json!("inline_payload_limit"))
        );
        anyhow::ensure!(
            value
                .get("result")
                .and_then(|result| result.get("payloadLocation"))
                == Some(&json!("event.result"))
        );
        anyhow::ensure!(event.serialized_bytes <= MAX_RETAINED_EVENT_BYTES_PER_OPERATION);
        anyhow::ensure!(record.history_truncated);
        Ok(())
    }

    #[test]
    fn terminal_count_and_age_eviction_never_remove_active_records() -> Result<()> {
        let registry = RuntimeOperationRegistry::default();
        let request = RuntimeOperationRequest::from_call(
            super::super::spec::OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        let active = operation_id("active-retained")?;
        registry.insert(running_runtime_operation_record(
            active.clone(),
            &request,
            1,
        )?)?;
        let mut first_terminal = None;
        for index in 0..=MAX_RETAINED_TERMINAL_RECORDS {
            let id = operation_id(&format!("terminal-{index}"))?;
            if first_terminal.is_none() {
                first_terminal = Some(id.clone());
            }
            registry.insert(running_runtime_operation_record(id.clone(), &request, 1)?)?;
            registry.transition(
                &id,
                RuntimeOperationTransition::Resolved(RuntimeOperationOutcome::Completed(
                    json!({ "index": index }),
                )),
            )?;
            if index == 0 {
                let mut state = registry
                    .state
                    .lock()
                    .map_err(|_| anyhow::anyhow!("registry unavailable"))?;
                let pending = PendingModuleEvent::new(
                    module_event("evicted-session", "evicted-cid")?,
                    vec![id.clone()],
                )?;
                state
                    .pending_module_events
                    .entry("storage_module".to_owned())
                    .or_default()
                    .push_back(pending)?;
            }
        }

        let first_terminal = first_terminal.context("first terminal id")?;
        anyhow::ensure!(registry.value(&first_terminal).is_err());
        anyhow::ensure!(registry.value(&active).is_ok());
        anyhow::ensure!(registry.pending_module_event_count("storage_module")? == 0);
        anyhow::ensure!(registry.pending_module_event_bytes("storage_module")? == 0);
        anyhow::ensure!(registry.len()? == MAX_RETAINED_TERMINAL_RECORDS + 1);

        registry.sweep_at(
            now_millis()
                .checked_add(TERMINAL_RECORD_MAX_AGE_MILLIS)
                .and_then(|value| value.checked_add(1))
                .context("age sweep time overflow")?,
        )?;
        anyhow::ensure!(registry.value(&active).is_ok());
        anyhow::ensure!(registry.len()? == 1);
        Ok(())
    }

    #[test]
    fn terminal_payload_pressure_purges_oldest_result_but_keeps_tombstone() -> Result<()> {
        let registry = RuntimeOperationRegistry::default();
        let request = RuntimeOperationRequest::from_call(
            super::super::spec::OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        let first = operation_id("payload-first")?;
        let second = operation_id("payload-second")?;
        registry.insert(running_runtime_operation_record(
            first.clone(),
            &request,
            1,
        )?)?;
        registry.insert(running_runtime_operation_record(
            second.clone(),
            &request,
            1,
        )?)?;
        let payload = || json!({ "payload": "x".repeat(17 * 1024 * 1024) });
        registry.transition(
            &first,
            RuntimeOperationTransition::Resolved(RuntimeOperationOutcome::Completed(payload())),
        )?;
        registry.transition(
            &second,
            RuntimeOperationTransition::Resolved(RuntimeOperationOutcome::Completed(payload())),
        )?;

        let first_value = registry.value(&first)?;
        anyhow::ensure!(first_value.get("result") == Some(&Value::Null));
        anyhow::ensure!(first_value.get("resultPurged") == Some(&json!(true)));
        anyhow::ensure!(first_value.get("status") == Some(&json!("completed")));
        registry.inspect(|records| -> Result<()> {
            let second_record = records.get(&second).context("second payload record")?;
            anyhow::ensure!(second_record.operation.result.is_some());
            Ok(())
        })??;
        let state = registry
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("registry unavailable"))?;
        anyhow::ensure!(
            state.retained_terminal_payload_bytes <= MAX_RETAINED_TERMINAL_PAYLOAD_BYTES
        );
        drop(state);
        anyhow::ensure!(registry.len()? == 2);
        Ok(())
    }

    #[test]
    fn terminal_payload_pressure_purges_oldest_acknowledgement_without_result_alias() -> Result<()>
    {
        let registry = RuntimeOperationRegistry::default();
        let request = RuntimeOperationRequest::from_call(
            super::super::spec::OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        let first = operation_id("ack-first")?;
        let second = operation_id("ack-second")?;
        registry.insert(running_runtime_operation_record(
            first.clone(),
            &request,
            1,
        )?)?;
        registry.insert(running_runtime_operation_record(
            second.clone(),
            &request,
            1,
        )?)?;
        let acknowledgement = || json!({ "dispatch": "x".repeat(17 * 1024 * 1024) });
        registry.transition(
            &first,
            RuntimeOperationTransition::Resolved(RuntimeOperationOutcome::Dispatched(
                acknowledgement(),
            )),
        )?;
        registry.transition(
            &second,
            RuntimeOperationTransition::Resolved(RuntimeOperationOutcome::Dispatched(
                acknowledgement(),
            )),
        )?;

        let first_value = registry.value(&first)?;
        anyhow::ensure!(first_value.get("acknowledgement") == Some(&Value::Null));
        anyhow::ensure!(first_value.get("acknowledgementPurged") == Some(&json!(true)));
        anyhow::ensure!(first_value.get("result") == Some(&Value::Null));
        registry.inspect(|records| -> Result<()> {
            let first_record = records
                .get(&first)
                .context("first acknowledgement record")?;
            let terminal = first_record
                .events
                .back()
                .context("dispatched terminal event")?;
            anyhow::ensure!(terminal.result.is_none());
            anyhow::ensure!(
                event_payload_location(&terminal.phase) == Some("operation.acknowledgement")
            );
            anyhow::ensure!(
                records
                    .get(&second)
                    .context("second acknowledgement record")?
                    .operation
                    .acknowledgement
                    .is_some()
            );
            Ok(())
        })??;
        Ok(())
    }

    #[test]
    fn combined_terminal_payload_purges_acknowledgement_before_authoritative_result() -> Result<()>
    {
        let registry = RuntimeOperationRegistry::default();
        let request = RuntimeOperationRequest::from_call(
            super::super::spec::OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        let id = operation_id("combined-payload")?;
        registry.insert(running_runtime_operation_record(id.clone(), &request, 1)?)?;
        {
            let mut state = registry
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("registry unavailable"))?;
            state
                .records
                .get_mut(&id)
                .context("combined payload record")?
                .operation
                .acknowledgement = Some(json!({ "dispatch": "x".repeat(17 * 1024 * 1024) }));
        }
        registry.transition(
            &id,
            RuntimeOperationTransition::Resolved(RuntimeOperationOutcome::Completed(json!({
                "payload": "x".repeat(17 * 1024 * 1024)
            }))),
        )?;

        let value = registry.value(&id)?;
        anyhow::ensure!(value.get("acknowledgement") == Some(&Value::Null));
        anyhow::ensure!(value.get("acknowledgementPurged") == Some(&json!(true)));
        anyhow::ensure!(value.get("resultPurged") == Some(&json!(false)));
        anyhow::ensure!(value.get("result").is_some_and(Value::is_object));
        Ok(())
    }

    #[test]
    fn event_cursor_allocation_stops_before_unsafe_exclusive_next_value() -> Result<()> {
        let request = RuntimeOperationRequest::from_call(
            super::super::spec::OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        let mut record =
            running_runtime_operation_record(operation_id("safe-cursor")?, &request, 1)?;
        record.next_event_cursor = EventCursor::new(MAX_WIRE_EVENT_CURSOR - 1);
        push_runtime_operation_event_locked(
            &mut record,
            "audit",
            "last safe event",
            None,
            None,
            None,
        )?;
        let previous = record.clone();

        let error = push_runtime_operation_event_locked(
            &mut record,
            "audit",
            "unsafe event",
            None,
            None,
            None,
        )
        .err()
        .context("unsafe cursor should fail")?;

        anyhow::ensure!(
            error.to_string()
                == "runtime operation event cursor exceeds JavaScript safe integer range"
        );
        anyhow::ensure!(record.events.len() == previous.events.len());
        anyhow::ensure!(record.next_event_cursor == previous.next_event_cursor);
        anyhow::ensure!(record.operation.event_cursor == previous.operation.event_cursor);
        Ok(())
    }
}
