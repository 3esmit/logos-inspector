use std::collections::HashMap;

use anyhow::Result;
use serde_json::{Value, json};

use crate::source_routing::{
    ModuleCorrelation, ModuleEventCorrelationKind, ModuleEventEnvelope, ModuleTerminalEventContract,
};

use super::{
    identity::RuntimeOperationId,
    outcome::RuntimeOperationOutcome,
    record::{
        RuntimeOperation, RuntimeOperationRecord, RuntimeOperationStatus,
        bounded_terminal_operation_error, operation_progress, push_runtime_operation_event_locked,
        runtime_operation_record_value,
    },
};

#[derive(Debug)]
pub(super) enum RuntimeOperationTransition {
    Started,
    Progress {
        bytes_written: u64,
        content_length: Option<u64>,
    },
    Resolved(RuntimeOperationOutcome),
    ExecutionFailed {
        error: String,
    },
    CancelRequested,
    CancellationConfirmed {
        error: Option<String>,
    },
    CancellationUnconfirmed {
        error: String,
    },
    CleanupUnconfirmed {
        error: String,
    },
    TimedOut {
        error: String,
    },
    Shutdown {
        error: String,
    },
    CleanupFailed {
        error: String,
    },
    TaskPanicked {
        error: String,
    },
    TaskAborted {
        error: String,
    },
    AdapterClosed {
        error: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TransitionDisposition {
    Applied,
    EvidenceOnly,
    Stale,
    DuplicateCorrelation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ModuleEventIngressResult {
    Applied {
        operation_id: RuntimeOperationId,
    },
    Stale {
        operation_id: RuntimeOperationId,
    },
    Deferred {
        operation_ids: Vec<RuntimeOperationId>,
    },
    Unknown,
    Uncorrelated {
        operation_ids: Vec<RuntimeOperationId>,
    },
    Ambiguous {
        operation_ids: Vec<RuntimeOperationId>,
    },
}

impl ModuleEventIngressResult {
    #[must_use]
    pub(super) fn as_value(
        &self,
        records: &HashMap<RuntimeOperationId, RuntimeOperationRecord>,
    ) -> Value {
        let operation_id = self.operation_id();
        let operation = operation_id
            .and_then(|operation_id| records.get(operation_id))
            .map(runtime_operation_record_value);
        let operation_ids = match self {
            Self::Deferred { operation_ids }
            | Self::Uncorrelated { operation_ids }
            | Self::Ambiguous { operation_ids } => operation_ids
                .iter()
                .map(RuntimeOperationId::as_str)
                .collect::<Vec<_>>(),
            Self::Applied { .. } | Self::Stale { .. } | Self::Unknown => Vec::new(),
        };
        json!({
            "disposition": self.as_str(),
            "operationId": operation_id.map(RuntimeOperationId::as_str),
            "candidateOperationIds": operation_ids,
            "operation": operation,
        })
    }

    #[must_use]
    pub(super) const fn as_str(&self) -> &'static str {
        match self {
            Self::Applied { .. } => "applied",
            Self::Stale { .. } => "stale",
            Self::Deferred { .. } => "deferred",
            Self::Unknown => "unknown",
            Self::Uncorrelated { .. } => "uncorrelated",
            Self::Ambiguous { .. } => "ambiguous",
        }
    }

    pub(super) fn operation_id(&self) -> Option<&RuntimeOperationId> {
        match self {
            Self::Applied { operation_id } | Self::Stale { operation_id } => Some(operation_id),
            Self::Deferred { operation_ids } if operation_ids.len() == 1 => operation_ids.first(),
            Self::Deferred { .. } => None,
            Self::Unknown | Self::Uncorrelated { .. } | Self::Ambiguous { .. } => None,
        }
    }
}

pub(super) fn apply_runtime_operation_transition(
    records: &mut HashMap<RuntimeOperationId, RuntimeOperationRecord>,
    operation_id: &RuntimeOperationId,
    transition: RuntimeOperationTransition,
) -> Result<TransitionDisposition> {
    let Some(record) = records.get(operation_id) else {
        return Ok(TransitionDisposition::EvidenceOnly);
    };
    if record.operation.status.is_terminal() {
        let record = records
            .get_mut(operation_id)
            .ok_or_else(|| anyhow::anyhow!("runtime operation disappeared during transition"))?;
        push_runtime_operation_event_locked(
            record,
            "stale_transition",
            stale_transition_message(&transition),
            None,
            None,
            None,
        )?;
        return Ok(TransitionDisposition::Stale);
    }

    let acceptance_problem = match &transition {
        RuntimeOperationTransition::Resolved(RuntimeOperationOutcome::Accepted(acceptance)) => {
            accepted_correlation_problem(
                records,
                operation_id,
                acceptance.correlation(),
                acceptance.terminal_event(),
            )
        }
        _ => None,
    };
    let record = records
        .get_mut(operation_id)
        .ok_or_else(|| anyhow::anyhow!("runtime operation disappeared during transition"))?;

    if let Some(problem) = acceptance_problem {
        return apply_record_mutation(record, |record| {
            let acknowledgement = match transition {
                RuntimeOperationTransition::Resolved(RuntimeOperationOutcome::Accepted(
                    acceptance,
                )) => (*acceptance).into_parts().0,
                _ => Value::Null,
            };
            record.operation.acknowledgement = Some(acknowledgement);
            record.pending_module_name = None;
            let (error, disposition) = match problem {
                AcceptedCorrelationProblem::Missing => (
                    "accepted operation has no correlation required by its terminal event contract"
                        .to_owned(),
                    TransitionDisposition::Applied,
                ),
                AcceptedCorrelationProblem::Duplicate => (
                    "accepted operation reused an external correlation".to_owned(),
                    TransitionDisposition::DuplicateCorrelation,
                ),
            };
            terminalize(
                record,
                RuntimeOperationStatus::Failed,
                None,
                Some(error),
                "invalid_external_correlation",
                "failed",
                "operation correlation rejected",
                TerminalProgress::Preserve,
            )?;
            Ok(disposition)
        });
    }

    apply_record_mutation(record, |record| match transition {
        RuntimeOperationTransition::Started => {
            push_runtime_operation_event_locked(
                record,
                "started",
                "operation started",
                Some(0.0),
                None,
                None,
            )?;
            Ok(TransitionDisposition::Applied)
        }
        RuntimeOperationTransition::Progress {
            bytes_written,
            content_length,
        } => {
            record.operation.bytes_written = record.operation.bytes_written.max(bytes_written);
            if let Some(content_length) = content_length {
                record.operation.content_length = Some(
                    record
                        .operation
                        .content_length
                        .map_or(content_length, |current| current.max(content_length)),
                );
            }
            let progress = monotonic_progress(
                record.operation.progress,
                operation_progress(
                    record.operation.bytes_written,
                    record.operation.content_length,
                ),
            );
            push_runtime_operation_event_locked(
                record,
                "progress",
                "operation progress",
                progress,
                None,
                None,
            )?;
            Ok(TransitionDisposition::Applied)
        }
        RuntimeOperationTransition::Resolved(outcome) => apply_outcome(record, outcome),
        RuntimeOperationTransition::ExecutionFailed { error } => {
            record.pending_module_name = None;
            terminalize(
                record,
                RuntimeOperationStatus::Failed,
                None,
                Some(error),
                "execution_failed",
                "failed",
                "operation failed",
                TerminalProgress::Preserve,
            )?;
            Ok(TransitionDisposition::Applied)
        }
        RuntimeOperationTransition::CancelRequested => apply_cancel_requested(record),
        RuntimeOperationTransition::CancellationConfirmed { error } => {
            let error = error.unwrap_or_else(|| "runtime operation canceled".to_owned());
            terminalize(
                record,
                RuntimeOperationStatus::Canceled,
                None,
                Some(error),
                "canceled",
                "canceled",
                "operation cancellation confirmed",
                TerminalProgress::Preserve,
            )?;
            Ok(TransitionDisposition::Applied)
        }
        RuntimeOperationTransition::CancellationUnconfirmed { error } => {
            let disposition = if record.operation.status == RuntimeOperationStatus::Canceling {
                TransitionDisposition::EvidenceOnly
            } else {
                apply_cancel_requested(record)?
            };
            push_runtime_operation_event_locked(
                record,
                "cancellation_unconfirmed",
                "adapter cleanup lacks remote termination evidence",
                record.operation.progress,
                None,
                Some(error),
            )?;
            Ok(disposition)
        }
        RuntimeOperationTransition::CleanupUnconfirmed { error } => {
            let disposition = if record.operation.status == RuntimeOperationStatus::Canceling {
                TransitionDisposition::EvidenceOnly
            } else {
                record.operation.status = RuntimeOperationStatus::Canceling;
                TransitionDisposition::Applied
            };
            if record.operation.error.is_none() {
                let (retained, redacted_bytes) =
                    bounded_terminal_operation_error(Some(error.clone()));
                record.operation.error = retained;
                record.error_redacted_bytes = redacted_bytes;
            }
            push_runtime_operation_event_locked(
                record,
                "cleanup_unconfirmed",
                "adapter cleanup lacks remote termination evidence",
                record.operation.progress,
                None,
                Some(error),
            )?;
            Ok(disposition)
        }
        RuntimeOperationTransition::TimedOut { error } => {
            terminalize(
                record,
                RuntimeOperationStatus::TimedOut,
                None,
                Some(error),
                "timeout",
                "timed_out",
                "operation timed out",
                TerminalProgress::Preserve,
            )?;
            Ok(TransitionDisposition::Applied)
        }
        RuntimeOperationTransition::Shutdown { error } => {
            terminalize(
                record,
                RuntimeOperationStatus::Failed,
                None,
                Some(error),
                "shutdown",
                "failed",
                "operation stopped during shutdown",
                TerminalProgress::Preserve,
            )?;
            Ok(TransitionDisposition::Applied)
        }
        RuntimeOperationTransition::CleanupFailed { error } => {
            terminalize(
                record,
                RuntimeOperationStatus::Failed,
                None,
                Some(error),
                "cleanup_failed",
                "failed",
                "operation cleanup failed",
                TerminalProgress::Preserve,
            )?;
            Ok(TransitionDisposition::Applied)
        }
        RuntimeOperationTransition::TaskPanicked { error } => {
            terminalize(
                record,
                RuntimeOperationStatus::Failed,
                None,
                Some(error),
                "task_panicked",
                "failed",
                "operation task panicked",
                TerminalProgress::Preserve,
            )?;
            Ok(TransitionDisposition::Applied)
        }
        RuntimeOperationTransition::TaskAborted { error } => {
            terminalize(
                record,
                RuntimeOperationStatus::Failed,
                None,
                Some(error),
                "task_aborted",
                "failed",
                "operation task was aborted",
                TerminalProgress::Preserve,
            )?;
            Ok(TransitionDisposition::Applied)
        }
        RuntimeOperationTransition::AdapterClosed { error } => {
            terminalize(
                record,
                RuntimeOperationStatus::Failed,
                None,
                Some(error),
                "adapter_closed",
                "failed",
                "operation adapter closed",
                TerminalProgress::Preserve,
            )?;
            Ok(TransitionDisposition::Applied)
        }
    })
}

fn monotonic_progress(previous: Option<f64>, candidate: Option<f64>) -> Option<f64> {
    match (previous, candidate) {
        (Some(previous), Some(candidate)) => Some(previous.max(candidate)),
        (Some(previous), None) => Some(previous),
        (None, candidate) => candidate,
    }
}

pub(super) fn apply_module_event_ingress(
    records: &mut HashMap<RuntimeOperationId, RuntimeOperationRecord>,
    event: &ModuleEventEnvelope,
) -> Result<ModuleEventIngressResult> {
    let mut recognized = Vec::new();
    let mut correlated = Vec::new();
    for (operation_id, record) in records.iter() {
        let Some(contract) = record.operation.terminal_event.as_ref() else {
            continue;
        };
        if !contract.recognizes(event) {
            continue;
        }
        recognized.push(operation_id.clone());
        let Some(expected) = operation_correlation_value(&record.operation, contract.correlation())
        else {
            continue;
        };
        let Some(actual) = event.correlation_value(contract.correlation()) else {
            continue;
        };
        if expected == actual {
            correlated.push(operation_id.clone());
        }
    }
    sort_operation_ids(&mut recognized);
    sort_operation_ids(&mut correlated);

    if correlated.len() > 1 {
        return Ok(ModuleEventIngressResult::Ambiguous {
            operation_ids: correlated,
        });
    }
    let Some(operation_id) = correlated.pop() else {
        if recognized.is_empty() {
            return Ok(ModuleEventIngressResult::Unknown);
        }
        return Ok(ModuleEventIngressResult::Uncorrelated {
            operation_ids: recognized,
        });
    };
    let record = records
        .get_mut(&operation_id)
        .ok_or_else(|| anyhow::anyhow!("correlated runtime operation disappeared"))?;
    if record.operation.status.is_terminal() {
        push_runtime_operation_event_locked(
            record,
            "stale_module_event",
            "module event arrived after terminal state",
            None,
            Some(event.result()),
            event.error(),
        )?;
        return Ok(ModuleEventIngressResult::Stale { operation_id });
    }

    apply_record_mutation(record, |record| {
        apply_correlated_module_event(record, event)
    })?;
    Ok(ModuleEventIngressResult::Applied { operation_id })
}

fn apply_record_mutation<T>(
    record: &mut RuntimeOperationRecord,
    apply: impl FnOnce(&mut RuntimeOperationRecord) -> Result<T>,
) -> Result<T> {
    let previous_record = record.clone();
    match apply(record) {
        Ok(value) => Ok(value),
        Err(error) => {
            *record = previous_record;
            Err(error)
        }
    }
}

fn apply_outcome(
    record: &mut RuntimeOperationRecord,
    outcome: RuntimeOperationOutcome,
) -> Result<TransitionDisposition> {
    record.pending_module_name = None;
    match outcome {
        RuntimeOperationOutcome::Completed(result) => {
            terminalize(
                record,
                RuntimeOperationStatus::Completed,
                Some(result),
                None,
                "completed",
                "completed",
                "operation completed",
                TerminalProgress::Complete,
            )?;
        }
        RuntimeOperationOutcome::Accepted(acceptance) => {
            let (acknowledgement, correlation, terminal_event) = (*acceptance).into_parts();
            record.operation.bridge_callback_id = correlation.bridge_callback_id().copied();
            record.operation.module_session_id = correlation.session_id().cloned();
            record.operation.module_request_id = correlation.request_id().cloned();
            record.operation.terminal_event = Some(terminal_event);
            record.operation.acknowledgement = Some(acknowledgement);
            record.operation.result = None;
            record.operation.error = None;
            record.operation.terminal_reason = None;
            if record.operation.status != RuntimeOperationStatus::Canceling {
                record.operation.status = RuntimeOperationStatus::AwaitingExternal;
            }
            let progress = record.operation.progress;
            push_runtime_operation_event_locked(
                record,
                "accepted",
                "operation accepted; awaiting correlated module event",
                progress,
                None,
                None,
            )?;
        }
        RuntimeOperationOutcome::Dispatched(acknowledgement) => {
            record.operation.acknowledgement = Some(acknowledgement);
            terminalize(
                record,
                RuntimeOperationStatus::Dispatched,
                None,
                None,
                "completion_unobservable",
                "dispatched",
                "operation dispatched without observable completion",
                TerminalProgress::Preserve,
            )?;
        }
    }
    Ok(TransitionDisposition::Applied)
}

pub(super) fn apply_deferred_module_event(
    record: &mut RuntimeOperationRecord,
    event: &ModuleEventEnvelope,
) -> Result<bool> {
    apply_record_mutation(record, |record| {
        let Some(contract) = record.operation.terminal_event.clone() else {
            push_runtime_operation_event_locked(
                record,
                "deferred_module_event_ignored",
                "deferred module event has no observable completion contract",
                None,
                Some(event.result()),
                event.error(),
            )?;
            return Ok(false);
        };
        let expected = operation_correlation_value(&record.operation, contract.correlation());
        let actual = event.correlation_value(contract.correlation());
        if !contract.recognizes(event) || expected != actual.as_deref() {
            push_runtime_operation_event_locked(
                record,
                "deferred_module_event_uncorrelated",
                "deferred module event did not match registered correlation",
                None,
                Some(event.result()),
                event.error(),
            )?;
            return Ok(false);
        }
        if record.operation.status.is_terminal() {
            push_runtime_operation_event_locked(
                record,
                "stale_module_event",
                "deferred module event arrived after terminal state",
                None,
                Some(event.result()),
                event.error(),
            )?;
            return Ok(true);
        }
        apply_correlated_module_event(record, event)?;
        Ok(true)
    })
}

fn apply_cancel_requested(record: &mut RuntimeOperationRecord) -> Result<TransitionDisposition> {
    if !record.operation.cancellable {
        push_runtime_operation_event_locked(
            record,
            "cancel_ignored",
            "operation is not cancellable",
            None,
            None,
            None,
        )?;
        return Ok(TransitionDisposition::EvidenceOnly);
    }
    if record.operation.status == RuntimeOperationStatus::Canceling {
        push_runtime_operation_event_locked(
            record,
            "cancel_duplicate",
            "operation cancellation was already requested",
            None,
            None,
            None,
        )?;
        return Ok(TransitionDisposition::EvidenceOnly);
    }
    record.operation.status = RuntimeOperationStatus::Canceling;
    push_runtime_operation_event_locked(record, "canceling", "cancel requested", None, None, None)?;
    Ok(TransitionDisposition::Applied)
}

fn apply_correlated_module_event(
    record: &mut RuntimeOperationRecord,
    event: &ModuleEventEnvelope,
) -> Result<()> {
    let contract =
        record.operation.terminal_event.clone().ok_or_else(|| {
            anyhow::anyhow!("correlated operation has no terminal event contract")
        })?;
    let result = event.result();
    if contract.progress_event() == Some(event.event_name()) {
        apply_module_event_progress(record, event, true);
        let progress = record.operation.progress;
        push_runtime_operation_event_locked(
            record,
            "external_progress",
            "correlated module progress",
            progress,
            Some(result),
            event.error(),
        )?;
        return Ok(());
    }

    apply_module_event_progress(record, event, false);
    if event.failed(&contract) {
        let error = event
            .error()
            .unwrap_or_else(|| format!("module event `{}` reported failure", event.event_name()));
        terminalize(
            record,
            RuntimeOperationStatus::Failed,
            None,
            Some(error),
            "external_failure",
            "failed",
            "correlated module event failed",
            TerminalProgress::Preserve,
        )?;
    } else {
        terminalize(
            record,
            RuntimeOperationStatus::Completed,
            Some(result),
            None,
            "external_completion",
            "completed",
            "correlated module event completed operation",
            TerminalProgress::Complete,
        )?;
    }
    Ok(())
}

fn apply_module_event_progress(
    record: &mut RuntimeOperationRecord,
    event: &ModuleEventEnvelope,
    accumulate: bool,
) {
    let (bytes, content_length) = event.progress();
    if let Some(bytes) = bytes {
        record.operation.bytes_written = if accumulate {
            record.operation.bytes_written.saturating_add(bytes)
        } else {
            record.operation.bytes_written.max(bytes)
        };
    }
    if let Some(content_length) = content_length {
        record.operation.content_length = Some(
            record
                .operation
                .content_length
                .map_or(content_length, |current| current.max(content_length)),
        );
    }
    record.operation.progress = monotonic_progress(
        record.operation.progress,
        operation_progress(
            record.operation.bytes_written,
            record.operation.content_length,
        ),
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalProgress {
    Preserve,
    Complete,
}

#[allow(clippy::too_many_arguments)]
fn terminalize(
    record: &mut RuntimeOperationRecord,
    status: RuntimeOperationStatus,
    result: Option<Value>,
    error: Option<String>,
    terminal_reason: &str,
    phase: &str,
    message: &str,
    terminal_progress: TerminalProgress,
) -> Result<()> {
    let progress = match terminal_progress {
        TerminalProgress::Preserve => record.operation.progress,
        TerminalProgress::Complete => Some(1.0),
    };
    let (error, error_redacted_bytes) = bounded_terminal_operation_error(error);
    record.operation.status = status;
    record.operation.result = result;
    record.operation.error = error;
    record.error_redacted_bytes = error_redacted_bytes;
    record.operation.terminal_reason = Some(terminal_reason.to_owned());
    record.restart_request = None;
    record.pending_module_name = None;
    push_runtime_operation_event_locked(record, phase, message, progress, None, None)
}

fn accepted_correlation_problem(
    records: &HashMap<RuntimeOperationId, RuntimeOperationRecord>,
    operation_id: &RuntimeOperationId,
    correlation: &ModuleCorrelation,
    terminal_event: &ModuleTerminalEventContract,
) -> Option<AcceptedCorrelationProblem> {
    let Some(identity) = correlation.identity_for(terminal_event.correlation()) else {
        return Some(AcceptedCorrelationProblem::Missing);
    };
    let duplicate = records.iter().any(|(candidate_id, candidate)| {
        candidate_id != operation_id
            && candidate
                .operation
                .terminal_event
                .as_ref()
                .is_some_and(|candidate_contract| {
                    candidate_contract.module() == terminal_event.module()
                        && candidate_contract.correlation() == terminal_event.correlation()
                        && operation_correlation_value(
                            &candidate.operation,
                            candidate_contract.correlation(),
                        ) == Some(identity)
                })
    });
    duplicate.then_some(AcceptedCorrelationProblem::Duplicate)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AcceptedCorrelationProblem {
    Missing,
    Duplicate,
}

fn operation_correlation_value<'a>(
    operation: &'a RuntimeOperation,
    kind: &ModuleEventCorrelationKind,
) -> Option<&'a str> {
    match kind {
        ModuleEventCorrelationKind::Session => operation
            .module_session_id
            .as_ref()
            .map(crate::source_routing::ModuleSessionId::as_str),
        ModuleEventCorrelationKind::Request => operation
            .module_request_id
            .as_ref()
            .map(crate::source_routing::ModuleRequestId::as_str),
    }
}

fn stale_transition_message(transition: &RuntimeOperationTransition) -> &'static str {
    match transition {
        RuntimeOperationTransition::Started => "start arrived after terminal state",
        RuntimeOperationTransition::Progress { .. } => "progress arrived after terminal state",
        RuntimeOperationTransition::Resolved(_) => "execution result arrived after terminal state",
        RuntimeOperationTransition::ExecutionFailed { .. } => {
            "execution failure arrived after terminal state"
        }
        RuntimeOperationTransition::CancelRequested => {
            "cancel request arrived after terminal state"
        }
        RuntimeOperationTransition::CancellationConfirmed { .. } => {
            "cancellation confirmation arrived after terminal state"
        }
        RuntimeOperationTransition::CancellationUnconfirmed { .. } => {
            "unconfirmed cancellation arrived after terminal state"
        }
        RuntimeOperationTransition::CleanupUnconfirmed { .. } => {
            "unconfirmed cleanup arrived after terminal state"
        }
        RuntimeOperationTransition::TimedOut { .. } => "timeout arrived after terminal state",
        RuntimeOperationTransition::Shutdown { .. } => "shutdown arrived after terminal state",
        RuntimeOperationTransition::CleanupFailed { .. } => {
            "cleanup failure arrived after terminal state"
        }
        RuntimeOperationTransition::TaskPanicked { .. } => {
            "task panic arrived after terminal state"
        }
        RuntimeOperationTransition::TaskAborted { .. } => "task abort arrived after terminal state",
        RuntimeOperationTransition::AdapterClosed { .. } => {
            "adapter close arrived after terminal state"
        }
    }
}

fn sort_operation_ids(operation_ids: &mut [RuntimeOperationId]) {
    operation_ids.sort_by(|left, right| left.as_str().cmp(right.as_str()));
}

#[cfg(test)]
mod tests {
    use anyhow::{Context as _, Result};
    use serde_json::json;

    use super::*;
    use crate::{
        inspector::commands::operations::{
            identity::EventCursor,
            record::{running_runtime_operation_record, runtime_operation_event_value},
            runtime_operation_request_from_value,
        },
        source_routing::{
            BridgeCallbackId, ModuleEventCorrelationKind, ModuleRequestId, ModuleSessionId,
            NodeOperationOutcome, ObservableOperationAcceptance,
        },
    };

    fn operation_id(value: &str) -> Result<RuntimeOperationId> {
        RuntimeOperationId::parse(value)
    }

    fn running_record(id: &str) -> Result<RuntimeOperationRecord> {
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageUploadUrl",
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "path": "/tmp/runtime-operation-transition-test" }
        }))?;
        running_runtime_operation_record(operation_id(id)?, &request, 1)
    }

    fn records(ids: &[&str]) -> Result<HashMap<RuntimeOperationId, RuntimeOperationRecord>> {
        ids.iter()
            .map(|id| Ok((operation_id(id)?, running_record(id)?)))
            .collect()
    }

    fn accepted_node_outcome(session_id: &str) -> Result<NodeOperationOutcome> {
        let session_id = ModuleSessionId::parse(session_id).context("module session id")?;
        Ok(NodeOperationOutcome::Accepted(Box::new(
            ObservableOperationAcceptance::new(
                json!({ "dispatched": true }),
                ModuleCorrelation::with_session(session_id)
                    .with_bridge_callback(BridgeCallbackId::new(7)),
                terminal_contract(),
            ),
        )))
    }

    fn accepted_outcome(session_id: &str) -> Result<RuntimeOperationOutcome> {
        accepted_node_outcome(session_id).map(Into::into)
    }

    fn fake_adapter(outcome: NodeOperationOutcome) -> RuntimeOperationOutcome {
        outcome.into()
    }

    fn terminal_contract() -> ModuleTerminalEventContract {
        ModuleTerminalEventContract::new(
            "storage_module",
            Some("storageUploadProgress"),
            "storageUploadDone",
            Some("storageUploadError"),
            ModuleEventCorrelationKind::Session,
        )
    }

    fn request_outcome(request_id: &str) -> Result<RuntimeOperationOutcome> {
        let request_id = ModuleRequestId::parse(request_id).context("module request id")?;
        Ok(RuntimeOperationOutcome::Accepted(Box::new(
            ObservableOperationAcceptance::new(
                json!({ "dispatched": true }),
                ModuleCorrelation::with_request(request_id),
                ModuleTerminalEventContract::new(
                    "delivery_module",
                    Some("messagePropagated"),
                    "messageSent",
                    Some("messageError"),
                    ModuleEventCorrelationKind::Request,
                ),
            ),
        )))
    }

    fn module_event(
        event_name: &str,
        session_id: &str,
        extra: Value,
    ) -> Result<ModuleEventEnvelope> {
        let mut payload = json!({ "sessionId": session_id });
        if let (Some(target), Some(extra)) = (payload.as_object_mut(), extra.as_object()) {
            target.extend(extra.clone());
        }
        ModuleEventEnvelope::from_value(&json!({
            "moduleName": "storage_module",
            "eventName": event_name,
            "args": [payload]
        }))
    }

    fn status(
        records: &HashMap<RuntimeOperationId, RuntimeOperationRecord>,
        operation_id: &RuntimeOperationId,
    ) -> Result<RuntimeOperationStatus> {
        records
            .get(operation_id)
            .map(|record| record.operation.status)
            .context("runtime operation record")
    }

    fn event_cursors(record: &RuntimeOperationRecord) -> Vec<u64> {
        record
            .events
            .iter()
            .filter_map(|event| {
                runtime_operation_event_value(event)
                    .get("eventCursor")
                    .and_then(Value::as_u64)
            })
            .collect()
    }

    #[test]
    fn resolved_outcomes_follow_distinct_conversation_paths() -> Result<()> {
        let mut records = records(&["completed", "accepted", "dispatched"])?;
        let completed = operation_id("completed")?;
        let accepted = operation_id("accepted")?;
        let dispatched = operation_id("dispatched")?;

        apply_runtime_operation_transition(
            &mut records,
            &completed,
            RuntimeOperationTransition::Resolved(fake_adapter(NodeOperationOutcome::Completed(
                json!({ "value": 1 }),
            ))),
        )?;
        apply_runtime_operation_transition(
            &mut records,
            &accepted,
            RuntimeOperationTransition::Resolved(fake_adapter(accepted_node_outcome("session-1")?)),
        )?;
        apply_runtime_operation_transition(
            &mut records,
            &dispatched,
            RuntimeOperationTransition::Resolved(fake_adapter(NodeOperationOutcome::Dispatched(
                json!({ "dispatched": true }),
            ))),
        )?;

        anyhow::ensure!(status(&records, &completed)? == RuntimeOperationStatus::Completed);
        anyhow::ensure!(status(&records, &accepted)? == RuntimeOperationStatus::AwaitingExternal);
        anyhow::ensure!(status(&records, &dispatched)? == RuntimeOperationStatus::Dispatched);
        anyhow::ensure!(
            records
                .get(&dispatched)
                .context("dispatched operation")?
                .operation
                .terminal_reason
                .as_deref()
                == Some("completion_unobservable")
        );
        anyhow::ensure!(
            records
                .get(&accepted)
                .context("accepted operation")?
                .operation
                .module_session_id
                .as_ref()
                .map(ModuleSessionId::as_str)
                == Some("session-1")
        );
        anyhow::ensure!(
            records
                .get(&accepted)
                .context("accepted operation")?
                .operation
                .bridge_callback_id
                .as_ref()
                .map(BridgeCallbackId::value)
                == Some(7),
            "Accepted outcome dropped bridge callback identity"
        );
        Ok(())
    }

    #[test]
    fn module_event_ingress_requires_exact_typed_correlation_and_keeps_cursors_monotonic()
    -> Result<()> {
        let mut records = records(&["accepted"])?;
        let operation_id = operation_id("accepted")?;
        apply_runtime_operation_transition(
            &mut records,
            &operation_id,
            RuntimeOperationTransition::Resolved(accepted_outcome("session-1")?),
        )?;

        let wrong = apply_module_event_ingress(
            &mut records,
            &module_event("storageUploadDone", "session-2", json!({ "cid": "wrong" }))?,
        )?;
        anyhow::ensure!(matches!(
            &wrong,
            ModuleEventIngressResult::Uncorrelated { .. }
        ));
        anyhow::ensure!(
            wrong.as_value(&records)
                == json!({
                    "disposition": "uncorrelated",
                    "operationId": null,
                    "candidateOperationIds": ["accepted"],
                    "operation": null
                }),
            "uncorrelated ingress wire evidence drifted"
        );
        anyhow::ensure!(
            status(&records, &operation_id)? == RuntimeOperationStatus::AwaitingExternal
        );

        let progress = apply_module_event_ingress(
            &mut records,
            &module_event(
                "storageUploadProgress",
                "session-1",
                json!({ "bytes": 4, "totalBytes": 8 }),
            )?,
        )?;
        anyhow::ensure!(matches!(progress, ModuleEventIngressResult::Applied { .. }));
        anyhow::ensure!(
            status(&records, &operation_id)? == RuntimeOperationStatus::AwaitingExternal
        );

        let completed = apply_module_event_ingress(
            &mut records,
            &module_event(
                "storageUploadDone",
                "session-1",
                json!({ "cid": "cid-1", "bytes": 8, "totalBytes": 8 }),
            )?,
        )?;
        anyhow::ensure!(matches!(
            &completed,
            ModuleEventIngressResult::Applied { .. }
        ));
        anyhow::ensure!(status(&records, &operation_id)? == RuntimeOperationStatus::Completed);
        let completed_wire = completed.as_value(&records);
        anyhow::ensure!(
            completed_wire.get("disposition") == Some(&json!("applied"))
                && completed_wire.get("operationId") == Some(&json!("accepted"))
                && completed_wire
                    .get("operation")
                    .and_then(|operation| operation.get("status"))
                    == Some(&json!("completed")),
            "applied ingress wire response drifted: {completed_wire}"
        );

        let stale = apply_module_event_ingress(
            &mut records,
            &module_event("storageUploadDone", "session-1", json!({ "cid": "late" }))?,
        )?;
        anyhow::ensure!(matches!(stale, ModuleEventIngressResult::Stale { .. }));
        let record = records.get(&operation_id).context("completed operation")?;
        anyhow::ensure!(
            record.operation.result
                == Some(json!({
                    "sessionId": "session-1",
                    "cid": "cid-1",
                    "bytes": 8,
                    "totalBytes": 8
                }))
        );
        anyhow::ensure!(event_cursors(record) == vec![1, 2, 3]);
        anyhow::ensure!(record.operation.event_cursor == EventCursor::new(3));
        let record_value = runtime_operation_record_value(record);
        anyhow::ensure!(record_value.get("droppedCount") == Some(&json!(1)));
        anyhow::ensure!(record_value.get("historyTruncated") == Some(&json!(true)));
        Ok(())
    }

    #[test]
    fn retained_correlation_cannot_be_reused_after_terminal_state() -> Result<()> {
        let mut records = records(&["first", "second"])?;
        let first = operation_id("first")?;
        let second = operation_id("second")?;
        apply_runtime_operation_transition(
            &mut records,
            &first,
            RuntimeOperationTransition::Resolved(accepted_outcome("shared-session")?),
        )?;
        apply_module_event_ingress(
            &mut records,
            &module_event(
                "storageUploadDone",
                "shared-session",
                json!({ "cid": "cid-first" }),
            )?,
        )?;

        let disposition = apply_runtime_operation_transition(
            &mut records,
            &second,
            RuntimeOperationTransition::Resolved(accepted_outcome("shared-session")?),
        )?;

        anyhow::ensure!(disposition == TransitionDisposition::DuplicateCorrelation);
        anyhow::ensure!(status(&records, &first)? == RuntimeOperationStatus::Completed);
        anyhow::ensure!(status(&records, &second)? == RuntimeOperationStatus::Failed);
        Ok(())
    }

    #[test]
    fn request_events_use_only_their_declared_identity_role() -> Result<()> {
        let mut records = records(&["request-success", "request-failure"])?;
        let request_success = operation_id("request-success")?;
        let request_failure = operation_id("request-failure")?;
        apply_runtime_operation_transition(
            &mut records,
            &request_success,
            RuntimeOperationTransition::Resolved(request_outcome("request-1")?),
        )?;
        apply_runtime_operation_transition(
            &mut records,
            &request_failure,
            RuntimeOperationTransition::Resolved(request_outcome("request-2")?),
        )?;
        let request_result = apply_module_event_ingress(
            &mut records,
            &ModuleEventEnvelope::from_value(&json!({
                "moduleName": "delivery_module",
                "eventName": "messageSent",
                "args": ["request-1", "hash-1"]
            }))?,
        )?;
        let failure_result = apply_module_event_ingress(
            &mut records,
            &ModuleEventEnvelope::from_value(&json!({
                "moduleName": "delivery_module",
                "eventName": "messageError",
                "args": ["request-2", "hash-2", "delivery failed"]
            }))?,
        )?;
        anyhow::ensure!(matches!(
            request_result,
            ModuleEventIngressResult::Applied { operation_id } if operation_id == request_success
        ));
        anyhow::ensure!(matches!(
            failure_result,
            ModuleEventIngressResult::Applied { operation_id } if operation_id == request_failure
        ));
        anyhow::ensure!(
            status(&records, &request_success)? == RuntimeOperationStatus::Completed
                && status(&records, &request_failure)? == RuntimeOperationStatus::Failed
        );
        anyhow::ensure!(
            records
                .get(&request_failure)
                .context("failed request operation")?
                .operation
                .error
                .as_deref()
                == Some("delivery failed")
        );
        Ok(())
    }

    #[test]
    fn ambiguous_module_event_is_evidence_only() -> Result<()> {
        let mut records = records(&["first", "second"])?;
        for id in [operation_id("first")?, operation_id("second")?] {
            let record = records.get_mut(&id).context("ambiguous operation")?;
            record.operation.status = RuntimeOperationStatus::AwaitingExternal;
            record.operation.module_session_id =
                Some(ModuleSessionId::parse("shared-session").context("session id")?);
            record.operation.terminal_event = Some(terminal_contract());
        }

        let result = apply_module_event_ingress(
            &mut records,
            &module_event(
                "storageUploadDone",
                "shared-session",
                json!({ "cid": "cid-1" }),
            )?,
        )?;

        let ModuleEventIngressResult::Ambiguous { operation_ids } = result else {
            anyhow::bail!("ambiguous event was not rejected");
        };
        anyhow::ensure!(operation_ids.len() == 2);
        anyhow::ensure!(records.values().all(|record| {
            record.operation.status == RuntimeOperationStatus::AwaitingExternal
                && record.events.is_empty()
        }));
        Ok(())
    }

    #[test]
    fn first_terminal_transition_wins_every_later_race() -> Result<()> {
        let mut records = records(&[
            "event-first",
            "cancel-first",
            "timeout-first",
            "shutdown-first",
        ])?;
        let event_first = operation_id("event-first")?;
        let cancel_first = operation_id("cancel-first")?;
        let timeout_first = operation_id("timeout-first")?;
        let shutdown_first = operation_id("shutdown-first")?;

        apply_runtime_operation_transition(
            &mut records,
            &event_first,
            RuntimeOperationTransition::Resolved(accepted_outcome("session-1")?),
        )?;
        apply_module_event_ingress(
            &mut records,
            &module_event("storageUploadDone", "session-1", json!({ "cid": "winner" }))?,
        )?;
        for transition in [
            RuntimeOperationTransition::CancellationConfirmed { error: None },
            RuntimeOperationTransition::TimedOut {
                error: "late timeout".to_owned(),
            },
            RuntimeOperationTransition::Shutdown {
                error: "late shutdown".to_owned(),
            },
            RuntimeOperationTransition::Resolved(RuntimeOperationOutcome::Completed(
                json!({ "cid": "late" }),
            )),
        ] {
            anyhow::ensure!(
                apply_runtime_operation_transition(&mut records, &event_first, transition)?
                    == TransitionDisposition::Stale
            );
        }
        anyhow::ensure!(status(&records, &event_first)? == RuntimeOperationStatus::Completed);

        apply_runtime_operation_transition(
            &mut records,
            &cancel_first,
            RuntimeOperationTransition::CancellationConfirmed { error: None },
        )?;
        anyhow::ensure!(
            apply_runtime_operation_transition(
                &mut records,
                &cancel_first,
                RuntimeOperationTransition::Resolved(RuntimeOperationOutcome::Completed(
                    json!({ "late": true }),
                )),
            )? == TransitionDisposition::Stale
        );
        anyhow::ensure!(status(&records, &cancel_first)? == RuntimeOperationStatus::Canceled);

        apply_runtime_operation_transition(
            &mut records,
            &timeout_first,
            RuntimeOperationTransition::TimedOut {
                error: "deadline elapsed".to_owned(),
            },
        )?;
        anyhow::ensure!(
            apply_runtime_operation_transition(
                &mut records,
                &timeout_first,
                RuntimeOperationTransition::Shutdown {
                    error: "late shutdown".to_owned(),
                },
            )? == TransitionDisposition::Stale
        );
        anyhow::ensure!(status(&records, &timeout_first)? == RuntimeOperationStatus::TimedOut);

        apply_runtime_operation_transition(
            &mut records,
            &shutdown_first,
            RuntimeOperationTransition::Shutdown {
                error: "runtime stopped".to_owned(),
            },
        )?;
        anyhow::ensure!(
            apply_runtime_operation_transition(
                &mut records,
                &shutdown_first,
                RuntimeOperationTransition::Resolved(RuntimeOperationOutcome::Completed(
                    json!({ "late": true }),
                )),
            )? == TransitionDisposition::Stale
        );
        let shutdown_record = records.get(&shutdown_first).context("shutdown operation")?;
        anyhow::ensure!(shutdown_record.operation.status == RuntimeOperationStatus::Failed);
        anyhow::ensure!(shutdown_record.operation.terminal_reason.as_deref() == Some("shutdown"));
        Ok(())
    }

    #[test]
    fn unknown_operation_and_event_remain_evidence_only() -> Result<()> {
        let mut records = records(&["known"])?;
        let missing = operation_id("missing")?;

        anyhow::ensure!(
            apply_runtime_operation_transition(
                &mut records,
                &missing,
                RuntimeOperationTransition::Shutdown {
                    error: "ignored".to_owned(),
                },
            )? == TransitionDisposition::EvidenceOnly
        );
        let unknown_event = ModuleEventEnvelope::from_value(&json!({
            "moduleName": "other_module",
            "eventName": "otherEvent",
            "args": []
        }))?;
        anyhow::ensure!(
            apply_module_event_ingress(&mut records, &unknown_event)?
                == ModuleEventIngressResult::Unknown
        );
        Ok(())
    }
}
