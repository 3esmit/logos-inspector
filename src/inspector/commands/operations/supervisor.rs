use std::{
    collections::{HashMap, hash_map::Entry},
    future::{Future, pending},
    path::PathBuf,
    pin::Pin,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    time::Duration,
};

use anyhow::{Context as _, Result, bail};
use futures_util::FutureExt as _;
use tokio::{
    runtime::Runtime,
    sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot},
    task::{AbortHandle, Id, JoinHandle, JoinSet},
    time::{Instant, sleep_until},
};
use tokio_util::sync::CancellationToken;

use crate::{
    modules::logos_core::{ModuleCallControl, SharedModuleTransport},
    support::{
        command_runner::CommandControl,
        work_tracker::{BlockingWorkGuard, BlockingWorkTracker},
    },
};

use super::{
    dispatch::execute_runtime_operation,
    identity::RuntimeOperationId,
    outcome::RuntimeOperationOutcome,
    record::{RuntimeOperationRegistry, RuntimeOperationStatus},
    request::RuntimeOperationRequest,
    transition::RuntimeOperationTransition,
};

const MAX_ACTIVE_OPERATIONS: usize = 64;
const SUPERVISOR_COMMAND_CAPACITY: usize = MAX_ACTIVE_OPERATIONS;
// Longer than ordinary adapters' bounded terminate-and-reap retry window.
const DEFAULT_TERMINATION_HANDSHAKE_GRACE: Duration = Duration::from_secs(1);
const EXECUTION_QUEUED: u8 = 0;
const EXECUTION_STARTED: u8 = 1;
const EXECUTION_STOPPED_BEFORE_START: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SupervisorPhase {
    Open,
    Closing,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OperationStopReason {
    CancelRequested = 1,
    DeadlineExceeded = 2,
    Shutdown = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TerminationEvidence {
    Confirmed,
    LocalOnly,
}

#[derive(Debug)]
pub(super) struct OperationInterrupted {
    reason: OperationStopReason,
    evidence: TerminationEvidence,
    message: String,
}

impl OperationInterrupted {
    pub(super) fn confirmed(reason: OperationStopReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            evidence: TerminationEvidence::Confirmed,
            message: message.into(),
        }
    }

    pub(super) fn local_only(reason: OperationStopReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            evidence: TerminationEvidence::LocalOnly,
            message: message.into(),
        }
    }

    #[cfg(test)]
    pub(super) const fn reason(&self) -> OperationStopReason {
        self.reason
    }

    #[cfg(test)]
    pub(super) const fn evidence(&self) -> TerminationEvidence {
        self.evidence
    }
}

impl std::fmt::Display for OperationInterrupted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for OperationInterrupted {}

#[derive(Debug)]
pub(super) struct OperationCleanupUnconfirmed {
    message: String,
}

impl OperationCleanupUnconfirmed {
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for OperationCleanupUnconfirmed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for OperationCleanupUnconfirmed {}

#[derive(Debug)]
pub(super) struct OperationAdapterClosed {
    message: String,
}

impl OperationAdapterClosed {
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for OperationAdapterClosed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for OperationAdapterClosed {}

#[derive(Clone)]
pub(super) struct OperationControl {
    cancellation: CancellationToken,
    settled: CancellationToken,
    deadline: Instant,
    stop_reason: Arc<AtomicU8>,
    disposable_file: Option<OperationDisposableFile>,
    blocking_work: BlockingWorkTracker,
    commit_active: Arc<AtomicBool>,
    execution_state: Arc<AtomicU8>,
    termination_handshake_grace: Duration,
}

pub(super) struct OperationCommitGuard {
    commit_active: Arc<AtomicBool>,
}

impl Drop for OperationCommitGuard {
    fn drop(&mut self) {
        self.commit_active.store(false, Ordering::Release);
    }
}

impl OperationControl {
    fn new(
        root: &CancellationToken,
        deadline: Instant,
        disposable_file: Option<OperationDisposableFile>,
    ) -> Self {
        Self {
            cancellation: root.child_token(),
            settled: CancellationToken::new(),
            deadline,
            stop_reason: Arc::new(AtomicU8::new(0)),
            disposable_file,
            blocking_work: BlockingWorkTracker::new(),
            commit_active: Arc::new(AtomicBool::new(false)),
            execution_state: Arc::new(AtomicU8::new(EXECUTION_QUEUED)),
            termination_handshake_grace: DEFAULT_TERMINATION_HANDSHAKE_GRACE,
        }
    }

    fn with_termination_handshake_grace(mut self, grace: Duration) -> Self {
        self.termination_handshake_grace = grace;
        self
    }

    pub(super) fn cancellation(&self) -> &CancellationToken {
        &self.cancellation
    }

    pub(super) const fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(super) fn stop_reason(&self) -> Option<OperationStopReason> {
        match self.stop_reason.load(Ordering::Acquire) {
            1 => Some(OperationStopReason::CancelRequested),
            2 => Some(OperationStopReason::DeadlineExceeded),
            3 => Some(OperationStopReason::Shutdown),
            _ => None,
        }
    }

    pub(super) fn module_call_control(&self) -> ModuleCallControl {
        ModuleCallControl::new(
            self.cancellation.clone(),
            self.deadline,
            Arc::clone(&self.stop_reason),
        )
        .with_blocking_work_tracker(self.blocking_work.clone())
    }

    pub(super) fn command_control(&self) -> CommandControl {
        CommandControl::new(self.cancellation.clone(), self.deadline.into_std())
            .with_blocking_work_tracker(self.blocking_work.clone())
    }

    pub(super) fn blocking_worker_guard(&self) -> Result<BlockingWorkGuard> {
        self.blocking_work.worker_guard()
    }

    pub(super) fn blocking_work_tracker(&self) -> BlockingWorkTracker {
        self.blocking_work.clone()
    }

    pub(super) fn begin_non_cancellable_commit(&self) -> Result<OperationCommitGuard> {
        self.commit_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| anyhow::anyhow!("runtime operation commit phase is already active"))?;
        Ok(OperationCommitGuard {
            commit_active: Arc::clone(&self.commit_active),
        })
    }

    fn commit_is_active(&self) -> bool {
        self.commit_active.load(Ordering::Acquire)
    }

    fn begin_execution(&self) -> Result<()> {
        if Instant::now() >= self.deadline {
            self.request_deadline();
        } else if self.cancellation.is_cancelled() {
            self.stop_before_execution();
        }
        match self.execution_state.compare_exchange(
            EXECUTION_QUEUED,
            EXECUTION_STARTED,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(()),
            Err(EXECUTION_STOPPED_BEFORE_START) => {
                let reason = self
                    .stop_reason()
                    .unwrap_or(OperationStopReason::CancelRequested);
                Err(not_started_interruption(reason).into())
            }
            Err(_) => bail!("runtime operation execution was already started"),
        }
    }

    fn stop_before_execution(&self) {
        let _result = self.execution_state.compare_exchange(
            EXECUTION_QUEUED,
            EXECUTION_STOPPED_BEFORE_START,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    pub(super) fn disposable_file_path(&self) -> Option<&std::path::Path> {
        self.disposable_file
            .as_ref()
            .map(|file| file.path.as_path())
    }

    pub(super) fn mark_disposable_file_created(&self) {
        if let Some(file) = &self.disposable_file {
            file.created.store(true, Ordering::Release);
        }
    }

    fn request_cancel(&self) {
        let _result = self.stop_reason.compare_exchange(
            0,
            OperationStopReason::CancelRequested as u8,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        self.stop_before_execution();
        self.cancellation.cancel();
    }

    fn request_deadline(&self) {
        let mut current = self.stop_reason.load(Ordering::Acquire);
        loop {
            if matches!(
                current,
                value if value == OperationStopReason::DeadlineExceeded as u8
                    || value == OperationStopReason::Shutdown as u8
            ) {
                break;
            }
            match self.stop_reason.compare_exchange_weak(
                current,
                OperationStopReason::DeadlineExceeded as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(updated) => current = updated,
            }
        }
        self.stop_before_execution();
        self.cancellation.cancel();
    }

    fn request_shutdown(&self) {
        let mut current = self.stop_reason.load(Ordering::Acquire);
        loop {
            if matches!(
                current,
                value if value == OperationStopReason::DeadlineExceeded as u8
                    || value == OperationStopReason::Shutdown as u8
            ) {
                break;
            }
            match self.stop_reason.compare_exchange_weak(
                current,
                OperationStopReason::Shutdown as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(updated) => current = updated,
            }
        }
        self.stop_before_execution();
        self.cancellation.cancel();
    }

    fn settle(&self) {
        self.settled.cancel();
    }
}

#[cfg(test)]
pub(super) fn test_operation_control(deadline: std::time::Duration) -> OperationControl {
    let root = CancellationToken::new();
    OperationControl::new(&root, Instant::now() + deadline, None)
}

pub(super) struct OperationAdmission {
    operation_id: RuntimeOperationId,
    control: OperationControl,
    cleanup: OperationCleanup,
    permit: OwnedSemaphorePermit,
}

#[derive(Debug, Clone)]
struct OperationDisposableFile {
    path: PathBuf,
    created: Arc<AtomicBool>,
}

#[derive(Debug, Default)]
struct OperationCleanup {
    disposable_files: Vec<OperationDisposableFile>,
}

impl OperationCleanup {
    async fn run(&self) -> Result<()> {
        for file in &self.disposable_files {
            if !file.created.load(Ordering::Acquire) {
                continue;
            }
            match tokio::fs::remove_file(&file.path).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!(
                            "failed to remove disposable operation file `{}`",
                            file.path.display()
                        )
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(super) struct RuntimeOperationSupervisor {
    inner: Arc<SupervisorInner>,
}

struct SupervisorInner {
    controller: Mutex<ControllerState>,
    closed: Condvar,
    shared: Arc<SupervisorShared>,
    admission: Arc<Semaphore>,
}

enum ControllerState {
    Unstarted,
    Running(ControllerRuntime),
    Draining,
    Stopped,
}

struct ControllerRuntime {
    commands: mpsc::Sender<SupervisorCommand>,
    task: JoinHandle<Result<()>>,
    reducer_abort: AbortHandle,
    reducer_completion: oneshot::Receiver<()>,
}

struct SupervisorShared {
    state: Mutex<SupervisorSharedState>,
    root: CancellationToken,
    registry: RuntimeOperationRegistry,
}

struct SupervisorSharedState {
    phase: SupervisorPhase,
    controls: HashMap<RuntimeOperationId, OperationControl>,
    live_tasks: HashMap<RuntimeOperationId, LiveOperationTask>,
    shutdown_error: Option<String>,
}

struct LiveOperationTask {
    task_id: Id,
    abort: AbortHandle,
    completion: oneshot::Receiver<()>,
}

struct OperationTaskCompletion(Option<oneshot::Sender<()>>);

impl Drop for OperationTaskCompletion {
    fn drop(&mut self) {
        if let Some(completion) = self.0.take() {
            let _completion_observed = completion.send(()).is_ok();
        }
    }
}

enum SupervisorCommand {
    Admit {
        admission: OperationAdmission,
        request: Box<RuntimeOperationRequest>,
        module_transport: SharedModuleTransport,
    },
    #[cfg(test)]
    InjectControllerError,
    #[cfg(test)]
    InjectControllerPanic,
}

impl RuntimeOperationSupervisor {
    pub(super) fn new(registry: RuntimeOperationRegistry) -> Self {
        Self {
            inner: Arc::new(SupervisorInner {
                controller: Mutex::new(ControllerState::Unstarted),
                closed: Condvar::new(),
                shared: Arc::new(SupervisorShared {
                    state: Mutex::new(SupervisorSharedState {
                        phase: SupervisorPhase::Open,
                        controls: HashMap::new(),
                        live_tasks: HashMap::new(),
                        shutdown_error: None,
                    }),
                    root: CancellationToken::new(),
                    registry,
                }),
                admission: Arc::new(Semaphore::new(MAX_ACTIVE_OPERATIONS)),
            }),
        }
    }

    pub(super) fn prepare(
        &self,
        runtime: &Runtime,
        operation_id: RuntimeOperationId,
        request: &RuntimeOperationRequest,
    ) -> Result<OperationAdmission> {
        self.ensure_controller(runtime)?;
        self.ensure_open()?;
        let permit = Arc::clone(&self.inner.admission)
            .try_acquire_owned()
            .map_err(|_| anyhow::anyhow!("runtime operation capacity is exhausted"))?;
        let deadline = Instant::now()
            .checked_add(request.deadline())
            .context("runtime operation deadline overflow")?;
        let disposable_file = super::storage::operation_disposable_file(request, &operation_id)?
            .map(|path| OperationDisposableFile {
                path,
                created: Arc::new(AtomicBool::new(false)),
            });
        let termination_handshake_grace = super::storage::termination_handshake_grace(request)?
            .unwrap_or(DEFAULT_TERMINATION_HANDSHAKE_GRACE);
        Ok(OperationAdmission {
            operation_id,
            control: OperationControl::new(
                &self.inner.shared.root,
                deadline,
                disposable_file.clone(),
            )
            .with_termination_handshake_grace(termination_handshake_grace),
            cleanup: OperationCleanup {
                disposable_files: disposable_file.into_iter().collect(),
            },
            permit,
        })
    }

    pub(super) fn admit(
        &self,
        admission: OperationAdmission,
        request: RuntimeOperationRequest,
        module_transport: SharedModuleTransport,
    ) -> Result<()> {
        let operation_id = admission.operation_id.clone();
        let control = admission.control.clone();
        {
            let mut state = self
                .inner
                .shared
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("runtime operation supervisor is unavailable"))?;
            if state.phase != SupervisorPhase::Open {
                bail!("runtime operation supervisor is shutting down");
            }
            match state.controls.entry(operation_id.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(control);
                }
                Entry::Occupied(_) => {
                    bail!(
                        "runtime operation `{}` is already supervised",
                        operation_id.as_str()
                    );
                }
            }
        }
        if let Err(error) = self.synchronize_admitted_control(&operation_id) {
            self.remove_control(&operation_id)?;
            return Err(error);
        }
        let sender = match self.controller_sender() {
            Ok(sender) => sender,
            Err(error) => {
                self.remove_control(&operation_id)?;
                return Err(error);
            }
        };
        let send_result = sender.try_send(SupervisorCommand::Admit {
            admission,
            request: Box::new(request),
            module_transport,
        });
        if let Err(error) = send_result {
            self.remove_control(&operation_id)?;
            match error {
                mpsc::error::TrySendError::Full(_) => {
                    bail!("runtime operation supervisor command capacity is exhausted")
                }
                mpsc::error::TrySendError::Closed(_) => {
                    bail!("runtime operation supervisor is stopped")
                }
            }
        }
        Ok(())
    }

    pub(super) fn cancel(&self, operation_id: &RuntimeOperationId) -> Result<()> {
        let state = self
            .inner
            .shared
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation supervisor is unavailable"))?;
        if let Some(control) = state.controls.get(operation_id) {
            control.request_cancel();
        }
        Ok(())
    }

    pub(super) fn settle(&self, operation_id: &RuntimeOperationId) -> Result<()> {
        let state = self
            .inner
            .shared
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation supervisor is unavailable"))?;
        if let Some(control) = state.controls.get(operation_id) {
            control.settle();
        }
        Ok(())
    }

    pub(super) fn begin_close(&self) -> Result<()> {
        let should_cancel =
            {
                let mut state =
                    self.inner.shared.state.lock().map_err(|_| {
                        anyhow::anyhow!("runtime operation supervisor is unavailable")
                    })?;
                match state.phase {
                    SupervisorPhase::Open => {
                        state.phase = SupervisorPhase::Closing;
                        for control in state.controls.values() {
                            control.request_shutdown();
                        }
                        true
                    }
                    SupervisorPhase::Closing | SupervisorPhase::Closed => false,
                }
            };
        if should_cancel {
            self.inner.shared.root.cancel();
        }
        Ok(())
    }

    pub(super) fn shutdown(&self, runtime: &Runtime) -> Result<()> {
        self.begin_close()?;
        let (owns_drain, controller) = {
            let mut state = self.inner.controller.lock().map_err(|_| {
                anyhow::anyhow!("runtime operation supervisor controller is unavailable")
            })?;
            match std::mem::replace(&mut *state, ControllerState::Draining) {
                ControllerState::Unstarted => (true, None),
                ControllerState::Running(controller) => (true, Some(controller)),
                ControllerState::Draining => {
                    *state = ControllerState::Draining;
                    (false, None)
                }
                ControllerState::Stopped => {
                    *state = ControllerState::Stopped;
                    (false, None)
                }
            }
        };
        if !owns_drain {
            return self.wait_for_closed();
        }
        let result = controller.map_or_else(
            || Ok(()),
            |controller| self.drain_controller(runtime, controller),
        );
        self.complete_shutdown(result)
    }

    fn ensure_open(&self) -> Result<()> {
        let state = self
            .inner
            .shared
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation supervisor is unavailable"))?;
        if state.phase != SupervisorPhase::Open {
            bail!("runtime operation supervisor is shutting down");
        }
        Ok(())
    }

    fn ensure_controller(&self, runtime: &Runtime) -> Result<()> {
        self.ensure_open()?;
        let mut controller = self.inner.controller.lock().map_err(|_| {
            anyhow::anyhow!("runtime operation supervisor controller is unavailable")
        })?;
        match &*controller {
            ControllerState::Running(_) => return Ok(()),
            ControllerState::Unstarted => {}
            ControllerState::Draining | ControllerState::Stopped => {
                bail!("runtime operation supervisor is stopped")
            }
        }
        self.ensure_open()?;
        let (commands, receiver) = mpsc::channel(SUPERVISOR_COMMAND_CAPACITY);
        let shared = Arc::clone(&self.inner.shared);
        let (reducer_completion, completed_reducer) = oneshot::channel();
        let reducer = runtime.spawn(async move {
            let _completion = OperationTaskCompletion(Some(reducer_completion));
            run_supervisor(receiver, Arc::clone(&shared)).await
        });
        let reducer_abort = reducer.abort_handle();
        let shared = Arc::clone(&self.inner.shared);
        let admission = Arc::clone(&self.inner.admission);
        let task = runtime.spawn(watch_supervisor(reducer, shared, admission));
        *controller = ControllerState::Running(ControllerRuntime {
            commands,
            task,
            reducer_abort,
            reducer_completion: completed_reducer,
        });
        Ok(())
    }

    fn controller_sender(&self) -> Result<mpsc::Sender<SupervisorCommand>> {
        let controller = self.inner.controller.lock().map_err(|_| {
            anyhow::anyhow!("runtime operation supervisor controller is unavailable")
        })?;
        match &*controller {
            ControllerState::Running(controller) => Ok(controller.commands.clone()),
            ControllerState::Unstarted | ControllerState::Draining | ControllerState::Stopped => {
                bail!("runtime operation supervisor is stopped")
            }
        }
    }

    fn remove_control(&self, operation_id: &RuntimeOperationId) -> Result<()> {
        self.inner
            .shared
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation supervisor is unavailable"))?
            .controls
            .remove(operation_id);
        Ok(())
    }

    fn synchronize_admitted_control(&self, operation_id: &RuntimeOperationId) -> Result<()> {
        let state = self
            .inner
            .shared
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation supervisor is unavailable"))?;
        let control = state.controls.get(operation_id).with_context(|| {
            format!(
                "runtime operation `{}` lost supervisor control during admission",
                operation_id.as_str()
            )
        })?;
        if state.phase != SupervisorPhase::Open {
            control.request_shutdown();
        } else if operation_status(&self.inner.shared.registry, operation_id)?
            == RuntimeOperationStatus::Canceling
        {
            control.request_cancel();
        }
        Ok(())
    }

    fn drain_controller(&self, runtime: &Runtime, controller: ControllerRuntime) -> Result<()> {
        let ControllerRuntime {
            commands,
            task,
            reducer_abort,
            reducer_completion,
        } = controller;
        drop(commands);
        match runtime.block_on(task) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                let message = format!("runtime operation supervisor stopped: {error}");
                let classification = self
                    .abort_and_await_live_tasks(runtime)
                    .and_then(|()| self.fail_remaining_after_controller_exit(runtime, &message));
                combine_shutdown_errors(Err(error), classification)
            }
            Err(error) => {
                let message = format!("runtime operation supervisor task failed: {error}");
                await_reducer_after_watcher_exit(runtime, reducer_abort, reducer_completion);
                let classification = self
                    .abort_and_await_live_tasks(runtime)
                    .and_then(|()| self.fail_remaining_after_controller_exit(runtime, &message));
                combine_shutdown_errors(Err(anyhow::anyhow!(message)), classification)
            }
        }
    }

    fn abort_and_await_live_tasks(&self, runtime: &Runtime) -> Result<()> {
        runtime.block_on(abort_and_await_live_tasks(&self.inner.shared))
    }

    fn complete_shutdown(&self, result: Result<()>) -> Result<()> {
        let shutdown_error = result.as_ref().err().map(ToString::to_string);
        {
            let mut controller = self.inner.controller.lock().map_err(|_| {
                anyhow::anyhow!("runtime operation supervisor controller is unavailable")
            })?;
            *controller = ControllerState::Stopped;
        }
        {
            let mut state = self
                .inner
                .shared
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("runtime operation supervisor is unavailable"))?;
            state.phase = SupervisorPhase::Closed;
            state.shutdown_error = shutdown_error;
        }
        self.inner.closed.notify_all();
        result
    }

    fn wait_for_closed(&self) -> Result<()> {
        let mut state = self
            .inner
            .shared
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation supervisor is unavailable"))?;
        while state.phase != SupervisorPhase::Closed {
            state = self
                .inner
                .closed
                .wait(state)
                .map_err(|_| anyhow::anyhow!("runtime operation supervisor is unavailable"))?;
        }
        match &state.shutdown_error {
            Some(error) => bail!(error.clone()),
            None => Ok(()),
        }
    }

    fn fail_remaining_after_controller_exit(&self, runtime: &Runtime, error: &str) -> Result<()> {
        runtime.block_on(classify_remaining_after_controller_exit(
            &self.inner.shared,
            error,
        ))
    }
}

async fn watch_supervisor(
    reducer: JoinHandle<Result<()>>,
    shared: Arc<SupervisorShared>,
    admission: Arc<Semaphore>,
) -> Result<()> {
    let primary = match reducer.await {
        Ok(Ok(())) => {
            let phase = shared
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("runtime operation supervisor is unavailable"))?
                .phase;
            if phase != SupervisorPhase::Open {
                return Ok(());
            }
            anyhow::anyhow!("runtime operation supervisor stopped unexpectedly")
        }
        Ok(Err(error)) => anyhow::anyhow!("runtime operation supervisor stopped: {error}"),
        Err(error) => anyhow::anyhow!("runtime operation supervisor task failed: {error}"),
    };
    let failure_message = primary.to_string();
    admission.close();
    let fail_close = fail_close_supervisor(&shared);
    let recovery = match fail_close {
        Ok(()) => match abort_and_await_live_tasks(&shared).await {
            Ok(()) => classify_remaining_after_controller_exit(&shared, &failure_message).await,
            Err(error) => Err(error),
        },
        Err(error) => Err(error),
    };
    let result = combine_shutdown_errors(Err(primary), recovery);
    let shutdown_error = result
        .as_ref()
        .err()
        .map(ToString::to_string)
        .unwrap_or_else(|| failure_message.clone());
    let persistence = persist_controller_failure(&shared, shutdown_error);
    combine_shutdown_errors(result, persistence)
}

fn fail_close_supervisor(shared: &SupervisorShared) -> Result<()> {
    {
        let mut state = shared
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation supervisor is unavailable"))?;
        if state.phase != SupervisorPhase::Closed {
            state.phase = SupervisorPhase::Closing;
        }
        for control in state.controls.values() {
            control.request_shutdown();
        }
    }
    shared.root.cancel();
    Ok(())
}

fn await_reducer_after_watcher_exit(
    runtime: &Runtime,
    reducer_task: AbortHandle,
    reducer_completion: oneshot::Receiver<()>,
) {
    runtime.block_on(async move {
        let _completion_result = reducer_completion.await;
        while !reducer_task.is_finished() {
            tokio::task::yield_now().await;
        }
    });
}

fn persist_controller_failure(shared: &SupervisorShared, error: String) -> Result<()> {
    let mut state = shared
        .state
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime operation supervisor is unavailable"))?;
    state.phase = SupervisorPhase::Closed;
    state.shutdown_error = Some(error);
    Ok(())
}

async fn abort_and_await_live_tasks(shared: &SupervisorShared) -> Result<()> {
    let (live_tasks, blocking_work) = {
        let mut state = shared
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation supervisor is unavailable"))?;
        let blocking_work = state
            .controls
            .values()
            .map(OperationControl::blocking_work_tracker)
            .collect::<Vec<_>>();
        let live_tasks = state
            .live_tasks
            .drain()
            .map(|(_, task)| task)
            .collect::<Vec<_>>();
        (live_tasks, blocking_work)
    };
    for task in &live_tasks {
        task.abort.abort();
    }
    for task in live_tasks {
        let _completion_result = task.completion.await;
        while !task.abort.is_finished() {
            tokio::task::yield_now().await;
        }
    }
    for tracker in &blocking_work {
        tracker.stop_accepting();
    }
    tokio::task::spawn_blocking(move || {
        for tracker in blocking_work {
            tracker.wait_idle();
        }
    })
    .await
    .context("runtime operation blocking-work drain failed")?;
    Ok(())
}

async fn classify_remaining_after_controller_exit(
    shared: &SupervisorShared,
    error: &str,
) -> Result<()> {
    let operations = {
        let mut state = shared
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime operation supervisor is unavailable"))?;
        state.controls.drain().collect::<Vec<_>>()
    };
    let mut first_error = None;
    for (operation_id, control) in operations {
        let cleanup = OperationCleanup {
            disposable_files: control.disposable_file.into_iter().collect(),
        };
        let transition = match cleanup.run().await {
            Ok(()) => RuntimeOperationTransition::TaskAborted {
                error: error.to_owned(),
            },
            Err(cleanup_error) => RuntimeOperationTransition::CleanupFailed {
                error: cleanup_error.to_string(),
            },
        };
        if let Err(transition_error) = shared.registry.transition(&operation_id, transition)
            && first_error.is_none()
        {
            first_error = Some(transition_error);
        }
    }
    if let Some(error) = first_error {
        return Err(error).context("failed to classify remaining runtime operations");
    }
    Ok(())
}

fn combine_shutdown_errors(primary: Result<()>, classification: Result<()>) -> Result<()> {
    match (primary, classification) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(primary), Err(classification)) => Err(anyhow::anyhow!(
            "{primary}; residual operation classification also failed: {classification}"
        )),
    }
}

enum OperationTaskExit {
    Execution {
        operation_id: RuntimeOperationId,
        result: OperationTaskResult,
    },
    External {
        operation_id: RuntimeOperationId,
        result: ExternalWaitResult,
    },
}

enum OperationTaskResult {
    Resolved(RuntimeOperationOutcome),
    ExecutionFailed(String),
    Interrupted(OperationInterrupted),
    CleanupUnconfirmed(String),
    AdapterClosed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExternalWaitResult {
    Settled,
    Deadline,
    CleanupSettlementGraceElapsed,
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskKind {
    Execution,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExternalWaitPolicy {
    OperationDeadline,
    CleanupSettlement { deadline: Instant },
    CleanupEvidence,
}

struct TaskDescriptor {
    operation_id: RuntimeOperationId,
    control: OperationControl,
    cleanup: OperationCleanup,
    _permit: OwnedSemaphorePermit,
    kind: TaskKind,
    external_wait_policy: ExternalWaitPolicy,
    cleanup_uncertainty: Option<String>,
}

fn spawn_operation_task<F>(
    tasks: &mut JoinSet<OperationTaskExit>,
    shared: &SupervisorShared,
    operation_id: &RuntimeOperationId,
    future: F,
) -> Result<AbortHandle>
where
    F: Future<Output = OperationTaskExit> + Send + 'static,
{
    let (completion, completed) = oneshot::channel();
    let completion = OperationTaskCompletion(Some(completion));
    let abort = tasks.spawn(async move {
        let _completion = completion;
        future.await
    });
    let live_task = LiveOperationTask {
        task_id: abort.id(),
        abort: abort.clone(),
        completion: completed,
    };
    let mut state = shared
        .state
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime operation supervisor is unavailable"))?;
    match state.live_tasks.entry(operation_id.clone()) {
        Entry::Vacant(entry) => {
            entry.insert(live_task);
        }
        Entry::Occupied(_) => {
            abort.abort();
            bail!(
                "runtime operation `{}` already has a live task",
                operation_id.as_str()
            );
        }
    }
    Ok(abort)
}

fn unregister_operation_task(
    shared: &SupervisorShared,
    operation_id: &RuntimeOperationId,
    task_id: Id,
) -> Result<()> {
    let mut state = shared
        .state
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime operation supervisor is unavailable"))?;
    let live_task = state.live_tasks.get(operation_id).with_context(|| {
        format!(
            "runtime operation `{}` exited without live-task ownership",
            operation_id.as_str()
        )
    })?;
    if live_task.task_id != task_id {
        bail!(
            "runtime operation `{}` exited with mismatched live-task identity",
            operation_id.as_str()
        );
    }
    state.live_tasks.remove(operation_id);
    Ok(())
}

async fn run_supervisor(
    mut commands: mpsc::Receiver<SupervisorCommand>,
    shared: Arc<SupervisorShared>,
) -> Result<()> {
    let mut tasks = JoinSet::new();
    let mut descriptors = HashMap::new();
    let reducer = run_supervisor_loop(&mut commands, &mut tasks, &mut descriptors, &shared);
    let reducer_result = std::panic::AssertUnwindSafe(reducer).catch_unwind().await;
    let failure = match reducer_result {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error),
        Err(_) => Some(anyhow::anyhow!(
            "runtime operation supervisor reducer panicked"
        )),
    };
    if let Some(error) = failure {
        let recovery =
            drain_after_controller_failure(&mut commands, &mut tasks, &mut descriptors, &shared)
                .await;
        return combine_controller_failure(error, recovery);
    }

    let state = shared
        .state
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime operation supervisor is unavailable"))?;
    if !state.controls.is_empty() || !state.live_tasks.is_empty() {
        bail!(
            "runtime operation supervisor exited with {} retained operation(s) and {} live task(s)",
            state.controls.len(),
            state.live_tasks.len()
        );
    }
    Ok(())
}

async fn run_supervisor_loop(
    commands: &mut mpsc::Receiver<SupervisorCommand>,
    tasks: &mut JoinSet<OperationTaskExit>,
    descriptors: &mut HashMap<Id, TaskDescriptor>,
    shared: &Arc<SupervisorShared>,
) -> Result<()> {
    let mut shutting_down = shared.root.is_cancelled();

    loop {
        if shutting_down {
            commands.close();
            drain_pending_admissions(commands, shared).await?;
            if tasks.is_empty() {
                break;
            }
        }
        let deadline = next_deadline(descriptors);
        let terminal_history_expiry = shared.registry.next_terminal_expiry()?;
        tokio::select! {
            biased;
            () = sleep_to(terminal_history_expiry), if !shutting_down => {
                shared.registry.sweep_expired_terminal_history()?;
            }
            joined = tasks.join_next_with_id(), if !tasks.is_empty() => {
                if let Some(joined) = joined {
                    handle_joined_task(
                        joined,
                        tasks,
                        descriptors,
                        shared,
                    ).await?;
                }
            }
            command = commands.recv(), if !shutting_down => {
                match command {
                    Some(command) => launch_command(command, tasks, descriptors, shared)?,
                    None => {
                        shutting_down = true;
                        request_shutdown_for_all(shared)?;
                    }
                }
            }
            () = shared.root.cancelled(), if !shutting_down => {
                shutting_down = true;
                request_shutdown_for_all(shared)?;
            }
            () = shared.registry.retention_changed(), if !shutting_down => {}
            () = sleep_to(deadline) => {
                request_expired_deadlines(descriptors);
            }
        }
    }
    Ok(())
}

async fn drain_after_controller_failure(
    commands: &mut mpsc::Receiver<SupervisorCommand>,
    tasks: &mut JoinSet<OperationTaskExit>,
    descriptors: &mut HashMap<Id, TaskDescriptor>,
    shared: &Arc<SupervisorShared>,
) -> Result<()> {
    commands.close();
    let mut failures = Vec::new();
    if let Err(error) = fail_close_supervisor(shared) {
        failures.push(format!("failed to close supervisor admission: {error:#}"));
    }
    shared.root.cancel();
    if let Err(error) = drain_pending_admissions(commands, shared).await {
        failures.push(format!("failed to drain pending admissions: {error:#}"));
    }
    while let Some(joined) = tasks.join_next_with_id().await {
        if let Err(error) = handle_joined_task(joined, tasks, descriptors, shared).await {
            failures.push(format!("failed to reduce a live operation task: {error:#}"));
        }
    }
    if !failures.is_empty() {
        bail!(failures.join("; "));
    }
    Ok(())
}

fn combine_controller_failure(primary: anyhow::Error, recovery: Result<()>) -> Result<()> {
    match recovery {
        Ok(()) => Err(primary),
        Err(recovery) => Err(anyhow::anyhow!(
            "{primary}; runtime operation failure drain also failed: {recovery:#}"
        )),
    }
}

fn launch_command(
    command: SupervisorCommand,
    tasks: &mut JoinSet<OperationTaskExit>,
    descriptors: &mut HashMap<Id, TaskDescriptor>,
    shared: &Arc<SupervisorShared>,
) -> Result<()> {
    let (admission, request, module_transport) = match command {
        SupervisorCommand::Admit {
            admission,
            request,
            module_transport,
        } => (admission, *request, module_transport),
        #[cfg(test)]
        SupervisorCommand::InjectControllerError => {
            bail!("injected runtime operation reducer failure")
        }
        #[cfg(test)]
        SupervisorCommand::InjectControllerPanic => {
            std::panic::resume_unwind(Box::new("injected runtime operation reducer panic"));
        }
    };
    let task_operation_id = admission.operation_id.clone();
    let task_registry = shared.registry.clone();
    let task_control = admission.control.clone();
    let task = spawn_operation_task(tasks, shared, &admission.operation_id, async move {
        let result = begin_supervised_execution(&task_control, || {
            execute_runtime_operation(
                request,
                &task_registry,
                &task_operation_id,
                &task_control,
                module_transport,
            )
        })
        .await;
        let result = match result {
            Ok(outcome) => OperationTaskResult::Resolved(outcome),
            Err(error) => match error.downcast::<OperationCleanupUnconfirmed>() {
                Ok(unconfirmed) => OperationTaskResult::CleanupUnconfirmed(unconfirmed.to_string()),
                Err(error) => match error.downcast::<OperationInterrupted>() {
                    Ok(interrupted) => OperationTaskResult::Interrupted(interrupted),
                    Err(error) => match error.downcast::<OperationAdapterClosed>() {
                        Ok(closed) => OperationTaskResult::AdapterClosed(closed.to_string()),
                        Err(error) => OperationTaskResult::ExecutionFailed(error.to_string()),
                    },
                },
            },
        };
        OperationTaskExit::Execution {
            operation_id: task_operation_id,
            result,
        }
    })?;
    descriptors.insert(
        task.id(),
        TaskDescriptor {
            operation_id: admission.operation_id,
            control: admission.control,
            cleanup: admission.cleanup,
            _permit: admission.permit,
            kind: TaskKind::Execution,
            external_wait_policy: ExternalWaitPolicy::OperationDeadline,
            cleanup_uncertainty: None,
        },
    );
    Ok(())
}

async fn begin_supervised_execution<C, F>(
    control: &OperationControl,
    create_execution: C,
) -> Result<RuntimeOperationOutcome>
where
    C: FnOnce() -> F,
    F: Future<Output = Result<RuntimeOperationOutcome>>,
{
    control.begin_execution()?;
    supervise_execution(control, create_execution()).await
}

async fn supervise_execution<F>(
    control: &OperationControl,
    execution: F,
) -> Result<RuntimeOperationOutcome>
where
    F: Future<Output = Result<RuntimeOperationOutcome>>,
{
    tokio::pin!(execution);
    tokio::select! {
        biased;
        result = &mut execution => result,
        () = control.cancellation().cancelled() => {
            if control.commit_is_active() {
                execution.as_mut().await
            } else {
                let reason = control
                    .stop_reason()
                    .unwrap_or(OperationStopReason::CancelRequested);
                await_termination_handshake(control, execution.as_mut(), reason).await
            }
        }
        () = sleep_until(control.deadline()) => {
            control.request_deadline();
            if control.commit_is_active() {
                execution.as_mut().await
            } else {
                await_termination_handshake(
                    control,
                    execution.as_mut(),
                    OperationStopReason::DeadlineExceeded,
                )
                .await
            }
        }
    }
}

async fn await_termination_handshake<F>(
    control: &OperationControl,
    mut execution: Pin<&mut F>,
    reason: OperationStopReason,
) -> Result<RuntimeOperationOutcome>
where
    F: Future<Output = Result<RuntimeOperationOutcome>>,
{
    tokio::select! {
        biased;
        result = &mut execution => result,
        () = tokio::time::sleep(control.termination_handshake_grace) => {
            if control.commit_is_active() {
                execution.await
            } else {
                Err(local_only_interruption(reason).into())
            }
        }
    }
}

fn local_only_interruption(reason: OperationStopReason) -> OperationInterrupted {
    let message = match reason {
        OperationStopReason::CancelRequested => {
            "runtime operation stopped after cancellation; adapter termination is unconfirmed"
        }
        OperationStopReason::DeadlineExceeded => {
            "runtime operation deadline elapsed; adapter termination is unconfirmed"
        }
        OperationStopReason::Shutdown => {
            "runtime operation stopped during shutdown; adapter termination is unconfirmed"
        }
    };
    OperationInterrupted::local_only(reason, message)
}

fn not_started_interruption(reason: OperationStopReason) -> OperationInterrupted {
    let message = match reason {
        OperationStopReason::CancelRequested => {
            "runtime operation was canceled before adapter execution started"
        }
        OperationStopReason::DeadlineExceeded => {
            "runtime operation deadline elapsed before adapter execution started"
        }
        OperationStopReason::Shutdown => {
            "runtime operation stopped during shutdown before adapter execution started"
        }
    };
    OperationInterrupted::confirmed(reason, message)
}

async fn handle_joined_task(
    joined: std::result::Result<(Id, OperationTaskExit), tokio::task::JoinError>,
    tasks: &mut JoinSet<OperationTaskExit>,
    descriptors: &mut HashMap<Id, TaskDescriptor>,
    shared: &Arc<SupervisorShared>,
) -> Result<()> {
    let task_id = match &joined {
        Ok((task_id, _)) => *task_id,
        Err(error) => error.id(),
    };
    let Some(descriptor) = descriptors.remove(&task_id) else {
        bail!("runtime operation task exited without a descriptor");
    };
    unregister_operation_task(shared, &descriptor.operation_id, task_id)?;
    await_blocking_work_idle(&descriptor.control).await?;
    if let Err(error) = descriptor.cleanup.run().await {
        shared.registry.transition(
            &descriptor.operation_id,
            RuntimeOperationTransition::CleanupFailed {
                error: error.to_string(),
            },
        )?;
        finish_operation(shared, &descriptor.operation_id)?;
        return Ok(());
    }

    match joined {
        Ok((_, exit)) => {
            handle_task_exit(exit, descriptor, tasks, descriptors, shared).await?;
        }
        Err(error) => {
            let transition = if error.is_panic() {
                RuntimeOperationTransition::TaskPanicked {
                    error: format!("runtime operation task panicked: {error}"),
                }
            } else {
                RuntimeOperationTransition::TaskAborted {
                    error: format!("runtime operation task was aborted: {error}"),
                }
            };
            shared
                .registry
                .transition(&descriptor.operation_id, transition)?;
            finish_operation(shared, &descriptor.operation_id)?;
        }
    }
    Ok(())
}

async fn await_blocking_work_idle(control: &OperationControl) -> Result<()> {
    let tracker = control.blocking_work_tracker();
    tracker.stop_accepting();
    tokio::task::spawn_blocking(move || tracker.wait_idle())
        .await
        .context("runtime operation blocking-work barrier failed")?;
    Ok(())
}

async fn handle_task_exit(
    exit: OperationTaskExit,
    mut descriptor: TaskDescriptor,
    tasks: &mut JoinSet<OperationTaskExit>,
    descriptors: &mut HashMap<Id, TaskDescriptor>,
    shared: &Arc<SupervisorShared>,
) -> Result<()> {
    let (operation_id, transition, retain_external) = match exit {
        OperationTaskExit::Execution {
            operation_id,
            result,
        } => {
            ensure_task_identity(&descriptor, &operation_id, TaskKind::Execution)?;
            match &result {
                OperationTaskResult::CleanupUnconfirmed(error) => {
                    let deadline = Instant::now()
                        .checked_add(descriptor.control.termination_handshake_grace)
                        .context("runtime operation cleanup settlement deadline overflow")?;
                    descriptor.external_wait_policy =
                        ExternalWaitPolicy::CleanupSettlement { deadline };
                    descriptor.cleanup_uncertainty = Some(error.clone());
                }
                OperationTaskResult::Interrupted(interrupted)
                    if interrupted.evidence == TerminationEvidence::LocalOnly =>
                {
                    descriptor.cleanup_uncertainty = Some(interrupted.message.clone());
                }
                OperationTaskResult::Resolved(_)
                | OperationTaskResult::ExecutionFailed(_)
                | OperationTaskResult::Interrupted(_)
                | OperationTaskResult::AdapterClosed(_) => {}
            }
            let transition = transition_for_execution_result(result);
            (operation_id, transition, true)
        }
        OperationTaskExit::External {
            operation_id,
            result,
        } => {
            ensure_task_identity(&descriptor, &operation_id, TaskKind::External)?;
            let transition = match result {
                ExternalWaitResult::Settled => None,
                ExternalWaitResult::Deadline => Some(RuntimeOperationTransition::TimedOut {
                    error: "runtime operation deadline elapsed".to_owned(),
                }),
                ExternalWaitResult::CleanupSettlementGraceElapsed => {
                    descriptor.external_wait_policy = ExternalWaitPolicy::CleanupEvidence;
                    Some(RuntimeOperationTransition::CleanupUnconfirmed {
                        error:
                            "cleanup settlement grace elapsed without external termination evidence"
                                .to_owned(),
                    })
                }
                ExternalWaitResult::Shutdown => Some(RuntimeOperationTransition::Shutdown {
                    error: descriptor.cleanup_uncertainty.as_ref().map_or_else(
                        || "runtime operation stopped during shutdown".to_owned(),
                        |uncertainty| {
                            format!(
                                "runtime operation stopped during shutdown; cleanup remains unconfirmed: {uncertainty}"
                            )
                        },
                    ),
                }),
            };
            let retain_external =
                matches!(result, ExternalWaitResult::CleanupSettlementGraceElapsed);
            (operation_id, transition, retain_external)
        }
    };

    if let Some(transition) = transition {
        shared.registry.transition(&operation_id, transition)?;
    }
    let status = operation_status(&shared.registry, &operation_id)?;
    if retain_external
        && matches!(
            status,
            RuntimeOperationStatus::AwaitingExternal | RuntimeOperationStatus::Canceling
        )
    {
        descriptor.kind = TaskKind::External;
        descriptor.cleanup = OperationCleanup::default();
        let task_control = descriptor.control.clone();
        let wait_policy = descriptor.external_wait_policy;
        let task_operation_id = operation_id.clone();
        let root = shared.root.clone();
        let task = spawn_operation_task(tasks, shared, &operation_id, async move {
            let result = wait_for_external_completion(&task_control, &root, wait_policy).await;
            OperationTaskExit::External {
                operation_id: task_operation_id,
                result,
            }
        })?;
        descriptors.insert(task.id(), descriptor);
    } else {
        finish_operation(shared, &operation_id)?;
    }
    Ok(())
}

fn transition_for_execution_result(
    result: OperationTaskResult,
) -> Option<RuntimeOperationTransition> {
    match result {
        OperationTaskResult::Resolved(outcome) => {
            Some(RuntimeOperationTransition::Resolved(outcome))
        }
        OperationTaskResult::ExecutionFailed(error) => {
            Some(RuntimeOperationTransition::ExecutionFailed { error })
        }
        OperationTaskResult::CleanupUnconfirmed(error) => {
            Some(RuntimeOperationTransition::CleanupUnconfirmed { error })
        }
        OperationTaskResult::Interrupted(interrupted) => match interrupted.evidence {
            TerminationEvidence::Confirmed => Some(match interrupted.reason {
                OperationStopReason::CancelRequested => {
                    RuntimeOperationTransition::CancellationConfirmed {
                        error: Some(interrupted.message),
                    }
                }
                OperationStopReason::DeadlineExceeded => RuntimeOperationTransition::TimedOut {
                    error: interrupted.message,
                },
                OperationStopReason::Shutdown => RuntimeOperationTransition::Shutdown {
                    error: interrupted.message,
                },
            }),
            TerminationEvidence::LocalOnly => Some(match interrupted.reason {
                OperationStopReason::CancelRequested => {
                    RuntimeOperationTransition::CancellationUnconfirmed {
                        error: interrupted.message,
                    }
                }
                OperationStopReason::DeadlineExceeded => RuntimeOperationTransition::TimedOut {
                    error: interrupted.message,
                },
                OperationStopReason::Shutdown => RuntimeOperationTransition::Shutdown {
                    error: interrupted.message,
                },
            }),
        },
        OperationTaskResult::AdapterClosed(error) => {
            Some(RuntimeOperationTransition::AdapterClosed { error })
        }
    }
}

async fn wait_for_external_completion(
    control: &OperationControl,
    root: &CancellationToken,
    policy: ExternalWaitPolicy,
) -> ExternalWaitResult {
    match policy {
        ExternalWaitPolicy::OperationDeadline => tokio::select! {
            biased;
            () = control.settled.cancelled() => ExternalWaitResult::Settled,
            () = sleep_until(control.deadline) => ExternalWaitResult::Deadline,
            () = root.cancelled() => ExternalWaitResult::Shutdown,
        },
        ExternalWaitPolicy::CleanupSettlement { deadline } => tokio::select! {
            biased;
            () = control.settled.cancelled() => ExternalWaitResult::Settled,
            () = root.cancelled() => ExternalWaitResult::Shutdown,
            () = sleep_until(deadline) => ExternalWaitResult::CleanupSettlementGraceElapsed,
        },
        ExternalWaitPolicy::CleanupEvidence => tokio::select! {
            biased;
            () = control.settled.cancelled() => ExternalWaitResult::Settled,
            () = root.cancelled() => ExternalWaitResult::Shutdown,
        },
    }
}

fn ensure_task_identity(
    descriptor: &TaskDescriptor,
    operation_id: &RuntimeOperationId,
    kind: TaskKind,
) -> Result<()> {
    if descriptor.operation_id != *operation_id || descriptor.kind != kind {
        bail!("runtime operation task identity did not match its descriptor");
    }
    Ok(())
}

fn operation_status(
    registry: &RuntimeOperationRegistry,
    operation_id: &RuntimeOperationId,
) -> Result<RuntimeOperationStatus> {
    registry
        .inspect(|records| {
            records
                .get(operation_id)
                .map(|record| record.operation.status)
        })?
        .with_context(|| {
            format!(
                "runtime operation `{}` disappeared while supervised",
                operation_id.as_str()
            )
        })
}

fn finish_operation(shared: &SupervisorShared, operation_id: &RuntimeOperationId) -> Result<()> {
    shared
        .state
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime operation supervisor is unavailable"))?
        .controls
        .remove(operation_id);
    Ok(())
}

fn next_deadline(descriptors: &HashMap<Id, TaskDescriptor>) -> Option<Instant> {
    descriptors
        .values()
        .filter(|descriptor| {
            descriptor.external_wait_policy == ExternalWaitPolicy::OperationDeadline
                && matches!(
                    descriptor.control.stop_reason(),
                    None | Some(OperationStopReason::CancelRequested)
                )
        })
        .map(|descriptor| descriptor.control.deadline)
        .min()
}

async fn sleep_to(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => sleep_until(deadline).await,
        None => pending::<()>().await,
    }
}

fn request_expired_deadlines(descriptors: &HashMap<Id, TaskDescriptor>) {
    let now = Instant::now();
    for descriptor in descriptors.values() {
        if descriptor.external_wait_policy == ExternalWaitPolicy::OperationDeadline
            && descriptor.control.deadline <= now
        {
            descriptor.control.request_deadline();
        }
    }
}

fn request_shutdown_for_all(shared: &SupervisorShared) -> Result<()> {
    let state = shared
        .state
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime operation supervisor is unavailable"))?;
    for control in state.controls.values() {
        control.request_shutdown();
    }
    Ok(())
}

async fn drain_pending_admissions(
    commands: &mut mpsc::Receiver<SupervisorCommand>,
    shared: &SupervisorShared,
) -> Result<()> {
    while let Ok(command) = commands.try_recv() {
        let admission = match command {
            SupervisorCommand::Admit { admission, .. } => admission,
            #[cfg(test)]
            SupervisorCommand::InjectControllerError | SupervisorCommand::InjectControllerPanic => {
                continue;
            }
        };
        shared.registry.transition(
            &admission.operation_id,
            RuntimeOperationTransition::Shutdown {
                error: "runtime operation stopped before execution during shutdown".to_owned(),
            },
        )?;
        finish_operation(shared, &admission.operation_id)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        path::Path,
        sync::{Arc, Barrier, mpsc as sync_mpsc},
        time::Duration,
    };

    use anyhow::{Context as _, Result, bail};
    use serde_json::{Value, json};
    use tokio::{runtime::Runtime, sync::oneshot, task::JoinSet, time::Instant};

    use crate::modules::logos_core::LogoscoreCliTransport;
    use crate::source_routing::{
        ModuleCorrelation, ModuleEventCorrelationKind, ModuleEventEnvelope, ModuleSessionId,
        ModuleTerminalEventContract, ObservableOperationAcceptance,
    };

    use super::*;
    use crate::inspector::commands::operations::{
        identity::EventCursor, record::running_runtime_operation_record,
        request::runtime_operation_request_from_value, spec::OperationMethod,
    };

    struct TestOperation {
        registry: RuntimeOperationRegistry,
        shared: Arc<SupervisorShared>,
        operation_id: RuntimeOperationId,
        control: OperationControl,
    }

    struct LiveFileUse {
        path: PathBuf,
        dropped: Arc<AtomicBool>,
        cleanup_raced: Arc<AtomicBool>,
    }

    impl Drop for LiveFileUse {
        fn drop(&mut self) {
            if !self.path.exists() {
                self.cleanup_raced.store(true, Ordering::Release);
            }
            self.dropped.store(true, Ordering::Release);
        }
    }

    struct DropSignal(Option<sync_mpsc::Sender<()>>);

    impl Drop for DropSignal {
        fn drop(&mut self) {
            if let Some(signal) = self.0.take() {
                let _signal_observed = signal.send(()).is_ok();
            }
        }
    }

    impl TestOperation {
        fn new(
            id: &str,
            request: &RuntimeOperationRequest,
            deadline: Instant,
            disposable_file: Option<OperationDisposableFile>,
        ) -> Result<Self> {
            let registry = RuntimeOperationRegistry::default();
            let operation_id = RuntimeOperationId::parse(id)?;
            registry.insert(running_runtime_operation_record(
                operation_id.clone(),
                request,
                1,
            )?)?;
            registry.transition(&operation_id, RuntimeOperationTransition::Started)?;

            let root = CancellationToken::new();
            let control = OperationControl::new(&root, deadline, disposable_file);
            let mut controls = HashMap::new();
            controls.insert(operation_id.clone(), control.clone());
            let shared = Arc::new(SupervisorShared {
                state: Mutex::new(SupervisorSharedState {
                    phase: SupervisorPhase::Open,
                    controls,
                    live_tasks: HashMap::new(),
                    shutdown_error: None,
                }),
                root,
                registry: registry.clone(),
            });
            Ok(Self {
                registry,
                shared,
                operation_id,
                control,
            })
        }

        fn descriptor(&self, cleanup: OperationCleanup, kind: TaskKind) -> Result<TaskDescriptor> {
            let permit = Arc::new(Semaphore::new(1))
                .try_acquire_owned()
                .context("test operation permit")?;
            Ok(TaskDescriptor {
                operation_id: self.operation_id.clone(),
                control: self.control.clone(),
                cleanup,
                _permit: permit,
                kind,
                external_wait_policy: ExternalWaitPolicy::OperationDeadline,
                cleanup_uncertainty: None,
            })
        }

        fn value(&self) -> Result<Value> {
            self.registry.value(&self.operation_id)
        }

        fn is_retained(&self) -> Result<bool> {
            let state = self
                .shared
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("test supervisor state unavailable"))?;
            Ok(state.controls.contains_key(&self.operation_id))
        }
    }

    fn storage_download_request(path: &Path) -> Result<RuntimeOperationRequest> {
        runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageDownloadToUrl",
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://storage.test/api" }
            },
            "mutating_enabled": true,
            "payload": {
                "cid": "cid-supervisor-test",
                "path": path.display().to_string(),
                "local_only": false
            }
        }))
    }

    #[tokio::test]
    async fn terminal_history_expires_while_registry_is_idle() -> Result<()> {
        let registry = RuntimeOperationRegistry::default();
        let operation_id = RuntimeOperationId::parse("idle-terminal-history")?;
        let request = RuntimeOperationRequest::from_call(
            OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        registry.insert(running_runtime_operation_record(
            operation_id.clone(),
            &request,
            1,
        )?)?;
        registry.transition(&operation_id, RuntimeOperationTransition::Started)?;
        registry.transition(
            &operation_id,
            RuntimeOperationTransition::Resolved(RuntimeOperationOutcome::Completed(json!({
                "accounts": []
            }))),
        )?;
        registry.set_terminal_expiry_after(&operation_id, Duration::from_millis(20))?;

        let root = CancellationToken::new();
        let shared = Arc::new(SupervisorShared {
            state: Mutex::new(SupervisorSharedState {
                phase: SupervisorPhase::Open,
                controls: HashMap::new(),
                live_tasks: HashMap::new(),
                shutdown_error: None,
            }),
            root: root.clone(),
            registry: registry.clone(),
        });
        let (commands, receiver) = mpsc::channel(1);
        let controller = tokio::spawn(run_supervisor(receiver, shared));

        let expiry_result = tokio::time::timeout(Duration::from_secs(1), async {
            while registry.contains_without_sweep(&operation_id)? {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            Ok::<(), anyhow::Error>(())
        })
        .await
        .context("terminal history expiry worker did not wake")
        .and_then(|result| result);

        root.cancel();
        drop(commands);
        let controller_result = controller
            .await
            .context("terminal history supervisor task failed")
            .and_then(|result| result);
        expiry_result?;
        controller_result
    }

    fn accepted_request(path: &Path) -> Result<RuntimeOperationRequest> {
        runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageUploadUrl",
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "path": path.display().to_string() }
        }))
    }

    fn accepted_outcome(session_id: &str) -> Result<RuntimeOperationOutcome> {
        Ok(RuntimeOperationOutcome::Accepted(Box::new(
            ObservableOperationAcceptance::new(
                json!({ "dispatched": true, "sessionId": session_id }),
                ModuleCorrelation::with_session(
                    ModuleSessionId::parse(session_id).context("test module session id")?,
                ),
                ModuleTerminalEventContract::new(
                    "storage_module",
                    Some("storageUploadProgress"),
                    "storageUploadDone",
                    None,
                    ModuleEventCorrelationKind::Session,
                ),
            ),
        )))
    }

    fn module_terminal_event(session_id: &str) -> Result<ModuleEventEnvelope> {
        ModuleEventEnvelope::from_value(&json!({
            "moduleName": "storage_module",
            "eventName": "storageUploadDone",
            "args": [{ "sessionId": session_id, "cid": "cid-complete" }]
        }))
    }

    fn assert_terminal(
        operation: &TestOperation,
        expected_status: &str,
        expected_reason: &str,
    ) -> Result<()> {
        let value = operation.value()?;
        if value.get("status").and_then(Value::as_str) != Some(expected_status)
            || value.get("terminalReason").and_then(Value::as_str) != Some(expected_reason)
        {
            bail!(
                "unexpected terminal evidence, expected {expected_status}/{expected_reason}: {value}"
            );
        }
        Ok(())
    }

    fn assert_shutdown_preserves_cleanup_uncertainty(
        operation: &TestOperation,
        expected: &str,
    ) -> Result<()> {
        let value = operation.value()?;
        let terminal_error = value
            .get("error")
            .and_then(Value::as_str)
            .context("cleanup-uncertain shutdown omitted terminal error")?;
        let events = operation
            .registry
            .events(&operation.operation_id, EventCursor::new(0))?;
        let shutdown_event = events
            .get("events")
            .and_then(Value::as_array)
            .context("cleanup-uncertain shutdown omitted event history")?
            .iter()
            .find(|event| event.get("phase").and_then(Value::as_str) == Some("failed"))
            .context("cleanup-uncertain shutdown omitted shutdown event")?;
        if !terminal_error.contains("cleanup remains unconfirmed")
            || !terminal_error.contains(expected)
            || shutdown_event.get("message").and_then(Value::as_str)
                != Some("operation stopped during shutdown")
            || shutdown_event
                .get("payloadLocation")
                .and_then(Value::as_str)
                != Some("operation.error")
        {
            bail!(
                "shutdown lost cleanup uncertainty: terminal={terminal_error}, event={shutdown_event}"
            );
        }
        Ok(())
    }

    fn assert_local_interruption(
        result: Result<RuntimeOperationOutcome>,
        expected_reason: OperationStopReason,
    ) -> Result<()> {
        let Err(error) = result else {
            bail!("supervised execution completed instead of stopping");
        };
        let interrupted = error
            .downcast::<OperationInterrupted>()
            .map_err(|error| anyhow::anyhow!("execution returned untyped interruption: {error}"))?;
        if interrupted.reason != expected_reason
            || interrupted.evidence != TerminationEvidence::LocalOnly
        {
            bail!("supervised execution returned incorrect interruption evidence");
        }
        Ok(())
    }

    async fn join_and_handle(
        tasks: &mut JoinSet<OperationTaskExit>,
        descriptors: &mut HashMap<Id, TaskDescriptor>,
        shared: &Arc<SupervisorShared>,
    ) -> Result<()> {
        let joined = tasks
            .join_next_with_id()
            .await
            .context("supervised test task did not exit")?;
        handle_joined_task(joined, tasks, descriptors, shared).await
    }

    async fn retain_accepted(
        operation: &TestOperation,
        session_id: &str,
        tasks: &mut JoinSet<OperationTaskExit>,
        descriptors: &mut HashMap<Id, TaskDescriptor>,
    ) -> Result<()> {
        handle_task_exit(
            OperationTaskExit::Execution {
                operation_id: operation.operation_id.clone(),
                result: OperationTaskResult::Resolved(accepted_outcome(session_id)?),
            },
            operation.descriptor(OperationCleanup::default(), TaskKind::Execution)?,
            tasks,
            descriptors,
            &operation.shared,
        )
        .await
    }

    #[test]
    fn admission_is_bounded_and_releases_capacity() -> Result<()> {
        let runtime = Runtime::new().context("test runtime")?;
        let registry = RuntimeOperationRegistry::default();
        let supervisor = RuntimeOperationSupervisor::new(registry);
        let directory = tempfile::tempdir()?;
        let request = storage_download_request(&directory.path().join("download.bin"))?;
        let mut admissions = Vec::with_capacity(MAX_ACTIVE_OPERATIONS);

        for sequence in 0..MAX_ACTIVE_OPERATIONS {
            admissions.push(supervisor.prepare(
                &runtime,
                RuntimeOperationId::parse(&format!("bounded-{sequence}"))?,
                &request,
            )?);
        }
        let Err(error) = supervisor.prepare(
            &runtime,
            RuntimeOperationId::parse("bounded-overflow")?,
            &request,
        ) else {
            bail!("supervisor admitted work beyond its active-operation bound");
        };
        if !error.to_string().contains("capacity is exhausted") {
            bail!("unexpected capacity error: {error}");
        }

        let released = admissions.pop().context("bounded admission fixture")?;
        drop(released);
        admissions.push(supervisor.prepare(
            &runtime,
            RuntimeOperationId::parse("bounded-replacement")?,
            &request,
        )?);
        drop(admissions);
        supervisor.shutdown(&runtime)
    }

    #[test]
    fn close_gate_rejects_start_after_an_earlier_open_check() -> Result<()> {
        let runtime = Runtime::new().context("test runtime")?;
        let supervisor = RuntimeOperationSupervisor::new(RuntimeOperationRegistry::default());
        let directory = tempfile::tempdir()?;
        let request = storage_download_request(&directory.path().join("download.bin"))?;

        supervisor.ensure_open()?;
        supervisor.begin_close()?;
        let Err(error) =
            supervisor.prepare(&runtime, RuntimeOperationId::parse("close-race")?, &request)
        else {
            bail!("operation start crossed the supervisor close gate");
        };
        if !error.to_string().contains("shutting down") {
            bail!("unexpected close-gate error: {error}");
        }
        {
            let controller = supervisor
                .inner
                .controller
                .lock()
                .map_err(|_| anyhow::anyhow!("test supervisor controller state unavailable"))?;
            if !matches!(&*controller, ControllerState::Unstarted) {
                bail!("close race started a controller after the close gate");
            }
        }
        supervisor.shutdown(&runtime)
    }

    #[test]
    fn admission_rolls_back_control_when_controller_sender_is_unavailable() -> Result<()> {
        let runtime = Runtime::new().context("test runtime")?;
        let registry = RuntimeOperationRegistry::default();
        let supervisor = RuntimeOperationSupervisor::new(registry.clone());
        let directory = tempfile::tempdir()?;
        let request = storage_download_request(&directory.path().join("download.bin"))?;
        let operation_id = RuntimeOperationId::parse("admission-sender-failure")?;
        let admission = supervisor.prepare(&runtime, operation_id.clone(), &request)?;
        registry.insert(running_runtime_operation_record(
            operation_id.clone(),
            &request,
            1,
        )?)?;
        registry.transition(&operation_id, RuntimeOperationTransition::Started)?;

        let controller = {
            let mut state = supervisor
                .inner
                .controller
                .lock()
                .map_err(|_| anyhow::anyhow!("test supervisor controller state unavailable"))?;
            match std::mem::replace(&mut *state, ControllerState::Draining) {
                ControllerState::Running(controller) => controller,
                ControllerState::Unstarted
                | ControllerState::Draining
                | ControllerState::Stopped => {
                    bail!("test supervisor controller was not running")
                }
            }
        };
        let result = supervisor.admit(
            admission,
            request,
            Arc::new(LogoscoreCliTransport::default()),
        );
        let Err(error) = result else {
            bail!("admission succeeded without a controller sender");
        };
        if !error.to_string().contains("supervisor is stopped") {
            bail!("unexpected controller sender error: {error}");
        }
        {
            let state = supervisor
                .inner
                .shared
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("test supervisor state unavailable"))?;
            if state.controls.contains_key(&operation_id) {
                bail!("failed admission retained supervisor control");
            }
        }
        {
            let mut state = supervisor
                .inner
                .controller
                .lock()
                .map_err(|_| anyhow::anyhow!("test supervisor controller state unavailable"))?;
            *state = ControllerState::Running(controller);
        }
        supervisor.shutdown(&runtime)
    }

    #[test]
    fn cancellation_before_admit_is_applied_to_inserted_control() -> Result<()> {
        let runtime = Runtime::new().context("test runtime")?;
        let registry = RuntimeOperationRegistry::default();
        let supervisor = RuntimeOperationSupervisor::new(registry.clone());
        let directory = tempfile::tempdir()?;
        let request = storage_download_request(&directory.path().join("download.bin"))?;
        let operation_id = RuntimeOperationId::parse("cancel-before-admit")?;
        let admission = supervisor.prepare(&runtime, operation_id.clone(), &request)?;
        let control = admission.control.clone();
        registry.insert(running_runtime_operation_record(
            operation_id.clone(),
            &request,
            1,
        )?)?;
        registry.transition(&operation_id, RuntimeOperationTransition::Started)?;
        registry.transition(&operation_id, RuntimeOperationTransition::CancelRequested)?;
        supervisor.cancel(&operation_id)?;
        if control.stop_reason().is_some() {
            bail!("cancel-before-admit fixture unexpectedly found supervisor control");
        }

        supervisor.admit(
            admission,
            request,
            Arc::new(LogoscoreCliTransport::default()),
        )?;
        if control.stop_reason() != Some(OperationStopReason::CancelRequested)
            || !control.cancellation().is_cancelled()
        {
            bail!("admission lost cancellation requested before control insertion");
        }
        supervisor.shutdown(&runtime)
    }

    #[tokio::test]
    async fn supervisor_deadline_stops_never_resolving_execution() -> Result<()> {
        let root = CancellationToken::new();
        let control = OperationControl::new(&root, Instant::now(), None);
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            supervise_execution(&control, pending::<Result<RuntimeOperationOutcome>>()),
        )
        .await
        .context("supervisor deadline did not stop a pending execution")?;

        assert_local_interruption(result, OperationStopReason::DeadlineExceeded)?;
        if control.stop_reason() != Some(OperationStopReason::DeadlineExceeded)
            || !control.cancellation().is_cancelled()
        {
            bail!("supervisor deadline did not update the authoritative operation control");
        }
        Ok(())
    }

    #[tokio::test]
    async fn supervisor_cancellation_and_shutdown_use_local_only_fallback() -> Result<()> {
        let root = CancellationToken::new();
        let canceled = OperationControl::new(&root, Instant::now() + Duration::from_secs(30), None);
        canceled.request_cancel();
        assert_local_interruption(
            supervise_execution(&canceled, pending::<Result<RuntimeOperationOutcome>>()).await,
            OperationStopReason::CancelRequested,
        )?;

        let shutdown = OperationControl::new(&root, Instant::now() + Duration::from_secs(30), None);
        shutdown.request_shutdown();
        assert_local_interruption(
            supervise_execution(&shutdown, pending::<Result<RuntimeOperationOutcome>>()).await,
            OperationStopReason::Shutdown,
        )
    }

    #[tokio::test]
    async fn queued_stop_prevents_adapter_dispatch() -> Result<()> {
        let root = CancellationToken::new();
        let canceled = OperationControl::new(&root, Instant::now() + Duration::from_secs(30), None);
        let cancel_dispatches = Arc::new(AtomicU8::new(0));
        let cancel_dispatch_count = Arc::clone(&cancel_dispatches);
        let queued_execution = begin_supervised_execution(&canceled, move || {
            cancel_dispatch_count.fetch_add(1, Ordering::AcqRel);
            std::future::ready(Ok(RuntimeOperationOutcome::Completed(json!({
                "dispatched": true
            }))))
        });
        canceled.request_cancel();
        let Err(cancel_error) = queued_execution.await else {
            bail!("queued cancellation dispatched adapter work");
        };
        let cancel_interruption = cancel_error
            .downcast::<OperationInterrupted>()
            .map_err(|error| anyhow::anyhow!("queued cancellation was untyped: {error}"))?;
        if cancel_dispatches.load(Ordering::Acquire) != 0
            || cancel_interruption.reason != OperationStopReason::CancelRequested
            || cancel_interruption.evidence != TerminationEvidence::Confirmed
        {
            bail!("queued cancellation crossed the adapter start gate");
        }

        let expired = OperationControl::new(&root, Instant::now(), None);
        let deadline_dispatches = Arc::new(AtomicU8::new(0));
        let deadline_dispatch_count = Arc::clone(&deadline_dispatches);
        let deadline_result = begin_supervised_execution(&expired, move || {
            deadline_dispatch_count.fetch_add(1, Ordering::AcqRel);
            std::future::ready(Ok(RuntimeOperationOutcome::Completed(json!({
                "dispatched": true
            }))))
        })
        .await;
        let Err(deadline_error) = deadline_result else {
            bail!("expired queued operation dispatched adapter work");
        };
        let deadline_interruption = deadline_error
            .downcast::<OperationInterrupted>()
            .map_err(|error| anyhow::anyhow!("queued deadline was untyped: {error}"))?;
        if deadline_dispatches.load(Ordering::Acquire) != 0
            || deadline_interruption.reason != OperationStopReason::DeadlineExceeded
            || deadline_interruption.evidence != TerminationEvidence::Confirmed
            || expired.stop_reason() != Some(OperationStopReason::DeadlineExceeded)
        {
            bail!("queued deadline crossed the adapter start gate");
        }
        Ok(())
    }

    #[tokio::test]
    async fn supervisor_completion_wins_a_ready_deadline_race() -> Result<()> {
        let root = CancellationToken::new();
        let control = OperationControl::new(&root, Instant::now(), None);
        let expected = RuntimeOperationOutcome::Completed(json!({ "committed": true }));
        let actual =
            supervise_execution(&control, std::future::ready(Ok(expected.clone()))).await?;

        if actual != expected || control.stop_reason().is_some() {
            bail!("ready completion lost the supervisor deadline race");
        }
        Ok(())
    }

    #[tokio::test]
    async fn supervisor_preserves_confirmed_prestart_worker_interruption() -> Result<()> {
        let root = CancellationToken::new();
        let control = OperationControl::new(&root, Instant::now() + Duration::from_secs(30), None);
        let worker_guard = control.blocking_worker_guard()?;
        let command_control = control.command_control();
        let (release_worker, observe_release) = sync_mpsc::channel();
        control.request_cancel();
        let execution = async move {
            tokio::task::spawn_blocking(move || {
                let _worker_guard = worker_guard;
                let _release_result = observe_release.recv();
                match command_control.check_active() {
                    Ok(()) => Ok(RuntimeOperationOutcome::Completed(json!({ "ran": true }))),
                    Err(error) => Err(OperationInterrupted::confirmed(
                        OperationStopReason::CancelRequested,
                        error.to_string(),
                    )
                    .into()),
                }
            })
            .await
            .context("queued operation worker failed")?
        };
        let supervised = supervise_execution(&control, execution);
        let release = async {
            tokio::task::yield_now().await;
            if release_worker.send(()).is_err() {
                bail!("queued worker stopped before its handshake was released");
            }
            Ok::<_, anyhow::Error>(())
        };
        let (result, release_result) = tokio::join!(biased; supervised, release);
        release_result?;

        let Err(error) = result else {
            bail!("pre-start cancellation executed queued blocking work");
        };
        let interrupted = error
            .downcast::<OperationInterrupted>()
            .map_err(|error| anyhow::anyhow!("worker returned untyped interruption: {error}"))?;
        if interrupted.reason != OperationStopReason::CancelRequested
            || interrupted.evidence != TerminationEvidence::Confirmed
        {
            bail!("supervisor replaced confirmed worker interruption evidence");
        }
        Ok(())
    }

    #[tokio::test]
    async fn non_cancellable_commit_survives_cancel_and_deadline() -> Result<()> {
        let root = CancellationToken::new();
        let canceled = OperationControl::new(&root, Instant::now() + Duration::from_secs(30), None);
        let canceled_execution_control = canceled.clone();
        let cancel_effects = Arc::new(AtomicU8::new(0));
        let cancel_execution_effects = Arc::clone(&cancel_effects);
        let (cancel_commit_entered, observed_cancel_commit) = oneshot::channel();
        let (release_cancel_commit, cancel_commit_release) = oneshot::channel();
        let cancel_execution = async move {
            let _commit = canceled_execution_control.begin_non_cancellable_commit()?;
            let _entered_result = cancel_commit_entered.send(());
            cancel_commit_release
                .await
                .context("cancel commit release disappeared")?;
            cancel_execution_effects.fetch_add(1, Ordering::AcqRel);
            Ok(RuntimeOperationOutcome::Completed(
                json!({ "cancelCommit": true }),
            ))
        };
        let cancel_supervised = supervise_execution(&canceled, cancel_execution);
        let trigger_cancel = async {
            observed_cancel_commit
                .await
                .context("cancel commit did not start")?;
            canceled.request_cancel();
            tokio::task::yield_now().await;
            if release_cancel_commit.send(()).is_err() {
                bail!("cancel commit future was dropped after entering commit phase");
            }
            Ok::<_, anyhow::Error>(())
        };
        let (cancel_result, trigger_result) =
            tokio::join!(biased; cancel_supervised, trigger_cancel);
        trigger_result?;
        if cancel_result? != RuntimeOperationOutcome::Completed(json!({ "cancelCommit": true }))
            || cancel_effects.load(Ordering::Acquire) != 1
            || canceled.commit_is_active()
        {
            bail!("cancellation dropped or duplicated an entered commit phase");
        }

        let deadline = OperationControl::new(&root, Instant::now(), None);
        let deadline_execution_control = deadline.clone();
        let deadline_effects = Arc::new(AtomicU8::new(0));
        let deadline_execution_effects = Arc::clone(&deadline_effects);
        let (deadline_commit_entered, observed_deadline_commit) = oneshot::channel();
        let (release_deadline_commit, deadline_commit_release) = oneshot::channel();
        let deadline_execution = async move {
            let _commit = deadline_execution_control.begin_non_cancellable_commit()?;
            let _entered_result = deadline_commit_entered.send(());
            deadline_commit_release
                .await
                .context("deadline commit release disappeared")?;
            deadline_execution_effects.fetch_add(1, Ordering::AcqRel);
            Ok(RuntimeOperationOutcome::Completed(
                json!({ "deadlineCommit": true }),
            ))
        };
        let deadline_supervised = supervise_execution(&deadline, deadline_execution);
        let release_deadline = async {
            observed_deadline_commit
                .await
                .context("deadline commit did not start")?;
            tokio::task::yield_now().await;
            if release_deadline_commit.send(()).is_err() {
                bail!("deadline commit future was dropped after entering commit phase");
            }
            Ok::<_, anyhow::Error>(())
        };
        let (deadline_result, release_result) =
            tokio::join!(biased; deadline_supervised, release_deadline);
        release_result?;
        if deadline_result? != RuntimeOperationOutcome::Completed(json!({ "deadlineCommit": true }))
            || deadline_effects.load(Ordering::Acquire) != 1
            || deadline.commit_is_active()
        {
            bail!("deadline dropped or duplicated an entered commit phase");
        }
        Ok(())
    }

    #[tokio::test]
    async fn controller_failure_drain_preserves_entered_commit_outcome() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let request = storage_download_request(&directory.path().join("download.bin"))?;
        let operation = TestOperation::new(
            "controller-failure-commit",
            &request,
            Instant::now() + Duration::from_secs(30),
            None,
        )?;
        let mut tasks = JoinSet::new();
        let mut descriptors = HashMap::new();
        let task_control = operation.control.clone();
        let execution_control = operation.control.clone();
        let task_operation_id = operation.operation_id.clone();
        let effects = Arc::new(AtomicU8::new(0));
        let worker_effects = Arc::clone(&effects);
        let (commit_entered, observed_commit) = oneshot::channel();
        let (release_commit, observe_release) = sync_mpsc::channel();
        let task = spawn_operation_task(
            &mut tasks,
            &operation.shared,
            &operation.operation_id,
            async move {
                let execution = async move {
                    let _commit = execution_control.begin_non_cancellable_commit()?;
                    let worker_guard = execution_control.blocking_worker_guard()?;
                    tokio::task::spawn_blocking(move || {
                        let _worker_guard = worker_guard;
                        let _entered_result = commit_entered.send(());
                        observe_release
                            .recv()
                            .context("commit release disappeared")?;
                        worker_effects.fetch_add(1, Ordering::AcqRel);
                        Ok(RuntimeOperationOutcome::Completed(json!({
                            "committed": true
                        })))
                    })
                    .await
                    .context("commit worker failed")?
                };
                let result = match supervise_execution(&task_control, execution).await {
                    Ok(outcome) => OperationTaskResult::Resolved(outcome),
                    Err(error) => OperationTaskResult::ExecutionFailed(error.to_string()),
                };
                OperationTaskExit::Execution {
                    operation_id: task_operation_id,
                    result,
                }
            },
        )?;
        descriptors.insert(
            task.id(),
            operation.descriptor(OperationCleanup::default(), TaskKind::Execution)?,
        );
        observed_commit
            .await
            .context("non-cancellable commit did not start")?;

        let (sender, mut commands) = mpsc::channel(1);
        drop(sender);
        let recovery = drain_after_controller_failure(
            &mut commands,
            &mut tasks,
            &mut descriptors,
            &operation.shared,
        );
        let release = async move {
            tokio::task::yield_now().await;
            release_commit
                .send(())
                .map_err(|_| anyhow::anyhow!("failure drain dropped the entered commit"))
        };
        let (recovery_result, release_result) = tokio::join!(biased; recovery, release);
        release_result?;
        recovery_result?;

        assert_terminal(&operation, "completed", "completed")?;
        if effects.load(Ordering::Acquire) != 1
            || operation.is_retained()?
            || !tasks.is_empty()
            || !descriptors.is_empty()
        {
            bail!("controller failure drain lost or duplicated an entered commit");
        }
        Ok(())
    }

    #[test]
    fn watcher_abort_waits_for_entered_commit_reducer() -> Result<()> {
        let runtime = Arc::new(Runtime::new().context("test runtime")?);
        let root = CancellationToken::new();
        let control = OperationControl::new(&root, Instant::now() + Duration::from_secs(30), None);
        let effects = Arc::new(AtomicU8::new(0));
        let worker_effects = Arc::clone(&effects);
        let (commit_entered, observed_commit) = sync_mpsc::channel();
        let (release_commit, observe_release) = sync_mpsc::channel();
        let (reducer_completion, completed_reducer) = oneshot::channel();
        let reducer = runtime.spawn(async move {
            let _completion = OperationTaskCompletion(Some(reducer_completion));
            let _commit = control.begin_non_cancellable_commit()?;
            let worker_guard = control.blocking_worker_guard()?;
            let worker = tokio::task::spawn_blocking(move || {
                let _worker_guard = worker_guard;
                let _entered_result = commit_entered.send(());
                let _release_result = observe_release.recv();
                worker_effects.fetch_add(1, Ordering::AcqRel);
            });
            worker.await.context("commit worker failed")?;
            Ok::<_, anyhow::Error>(())
        });
        let reducer_task = reducer.abort_handle();
        drop(reducer);
        observed_commit
            .recv_timeout(Duration::from_secs(1))
            .context("commit reducer did not enter its commit phase")?;

        let wait_runtime = Arc::clone(&runtime);
        let waiter = std::thread::spawn(move || {
            await_reducer_after_watcher_exit(
                wait_runtime.as_ref(),
                reducer_task,
                completed_reducer,
            );
        });
        std::thread::sleep(Duration::from_millis(25));
        let returned_before_release = waiter.is_finished();
        let worker_was_released = release_commit.send(()).is_ok();
        waiter
            .join()
            .map_err(|_| anyhow::anyhow!("reducer waiter panicked"))?;

        if returned_before_release || !worker_was_released || effects.load(Ordering::Acquire) != 1 {
            bail!("watcher abort crossed an entered commit reducer");
        }
        Ok(())
    }

    #[tokio::test]
    async fn completion_wins_a_ready_completion_cancel_race() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let request = storage_download_request(&directory.path().join("download.bin"))?;
        let operation = TestOperation::new(
            "completion-cancel-race",
            &request,
            Instant::now() + Duration::from_secs(30),
            None,
        )?;
        operation.registry.transition(
            &operation.operation_id,
            RuntimeOperationTransition::CancelRequested,
        )?;
        operation.control.request_cancel();
        let mut tasks = JoinSet::new();
        let mut descriptors = HashMap::new();

        handle_task_exit(
            OperationTaskExit::Execution {
                operation_id: operation.operation_id.clone(),
                result: OperationTaskResult::Resolved(RuntimeOperationOutcome::Completed(json!({
                    "completed": true
                }))),
            },
            operation.descriptor(OperationCleanup::default(), TaskKind::Execution)?,
            &mut tasks,
            &mut descriptors,
            &operation.shared,
        )
        .await?;

        assert_terminal(&operation, "completed", "completed")?;
        if operation.is_retained()? || !tasks.is_empty() {
            bail!("completed operation remained supervised after winning cancellation race");
        }
        Ok(())
    }

    #[tokio::test]
    async fn completion_wins_a_deadline_racing_non_cancellable_commit() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let request = storage_download_request(&directory.path().join("download.bin"))?;
        let operation =
            TestOperation::new("completion-deadline-race", &request, Instant::now(), None)?;
        operation.control.request_deadline();
        let mut tasks = JoinSet::new();
        let mut descriptors = HashMap::new();

        handle_task_exit(
            OperationTaskExit::Execution {
                operation_id: operation.operation_id.clone(),
                result: OperationTaskResult::Resolved(RuntimeOperationOutcome::Completed(json!({
                    "committed": true
                }))),
            },
            operation.descriptor(OperationCleanup::default(), TaskKind::Execution)?,
            &mut tasks,
            &mut descriptors,
            &operation.shared,
        )
        .await?;

        assert_terminal(&operation, "completed", "completed")?;
        if operation.is_retained()? || !tasks.is_empty() {
            bail!("committed operation was reclassified after a raced deadline");
        }
        Ok(())
    }

    #[tokio::test]
    async fn execution_failure_wins_a_deadline_race() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let request = storage_download_request(&directory.path().join("download.bin"))?;
        let operation =
            TestOperation::new("failure-deadline-race", &request, Instant::now(), None)?;
        operation.control.request_deadline();
        let mut tasks = JoinSet::new();
        let mut descriptors = HashMap::new();

        handle_task_exit(
            OperationTaskExit::Execution {
                operation_id: operation.operation_id.clone(),
                result: OperationTaskResult::ExecutionFailed(
                    "commit failed after deadline request".to_owned(),
                ),
            },
            operation.descriptor(OperationCleanup::default(), TaskKind::Execution)?,
            &mut tasks,
            &mut descriptors,
            &operation.shared,
        )
        .await?;

        assert_terminal(&operation, "failed", "execution_failed")?;
        if operation.is_retained()? || !tasks.is_empty() {
            bail!("failed operation remained supervised after winning deadline race");
        }
        Ok(())
    }

    #[tokio::test]
    async fn canceling_remains_nonterminal_until_task_exit_and_cleanup() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let disposable_path = directory.path().join("download.part");
        tokio::fs::write(&disposable_path, b"partial").await?;
        let disposable_file = OperationDisposableFile {
            path: disposable_path.clone(),
            created: Arc::new(AtomicBool::new(true)),
        };
        let request = storage_download_request(&directory.path().join("download.bin"))?;
        let operation = TestOperation::new(
            "cancel-cleanup",
            &request,
            Instant::now() + Duration::from_secs(30),
            Some(disposable_file.clone()),
        )?;
        let (release, released) = oneshot::channel();
        let mut tasks = JoinSet::new();
        let mut descriptors = HashMap::new();
        let task_operation_id = operation.operation_id.clone();
        let task = spawn_operation_task(
            &mut tasks,
            &operation.shared,
            &operation.operation_id,
            async move {
                let _release_result = released.await;
                OperationTaskExit::Execution {
                    operation_id: task_operation_id,
                    result: OperationTaskResult::Interrupted(OperationInterrupted::confirmed(
                        OperationStopReason::CancelRequested,
                        "adapter termination confirmed",
                    )),
                }
            },
        )?;
        descriptors.insert(
            task.id(),
            operation.descriptor(
                OperationCleanup {
                    disposable_files: vec![disposable_file],
                },
                TaskKind::Execution,
            )?,
        );

        operation.registry.transition(
            &operation.operation_id,
            RuntimeOperationTransition::CancelRequested,
        )?;
        operation.control.request_cancel();
        let canceling = operation.value()?;
        if canceling.get("status").and_then(Value::as_str) != Some("canceling")
            || canceling.get("terminalReason") != Some(&Value::Null)
            || !tokio::fs::try_exists(&disposable_path).await?
            || !operation.is_retained()?
        {
            bail!("cancel request became terminal before task exit and cleanup: {canceling}");
        }

        if release.send(()).is_err() {
            bail!("supervised task stopped before the cleanup barrier was released");
        }
        join_and_handle(&mut tasks, &mut descriptors, &operation.shared).await?;

        assert_terminal(&operation, "canceled", "canceled")?;
        if tokio::fs::try_exists(&disposable_path).await? || operation.is_retained()? {
            bail!("canceled operation terminalized before disposable cleanup completed");
        }
        Ok(())
    }

    #[tokio::test]
    async fn cleanup_failure_overrides_confirmed_cancellation_evidence() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let disposable_path = directory.path().join("not-a-file");
        tokio::fs::create_dir(&disposable_path).await?;
        let disposable_file = OperationDisposableFile {
            path: disposable_path.clone(),
            created: Arc::new(AtomicBool::new(true)),
        };
        let request = storage_download_request(&directory.path().join("download.bin"))?;
        let operation = TestOperation::new(
            "cleanup-failure",
            &request,
            Instant::now() + Duration::from_secs(30),
            Some(disposable_file.clone()),
        )?;
        operation.registry.transition(
            &operation.operation_id,
            RuntimeOperationTransition::CancelRequested,
        )?;
        operation.control.request_cancel();
        let mut tasks = JoinSet::new();
        let mut descriptors = HashMap::new();
        let task_operation_id = operation.operation_id.clone();
        let task = spawn_operation_task(
            &mut tasks,
            &operation.shared,
            &operation.operation_id,
            async move {
                OperationTaskExit::Execution {
                    operation_id: task_operation_id,
                    result: OperationTaskResult::Interrupted(OperationInterrupted::confirmed(
                        OperationStopReason::CancelRequested,
                        "adapter termination confirmed",
                    )),
                }
            },
        )?;
        descriptors.insert(
            task.id(),
            operation.descriptor(
                OperationCleanup {
                    disposable_files: vec![disposable_file],
                },
                TaskKind::Execution,
            )?,
        );

        join_and_handle(&mut tasks, &mut descriptors, &operation.shared).await?;

        assert_terminal(&operation, "failed", "cleanup_failed")?;
        let value = operation.value()?;
        if !value
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|error| error.contains("failed to remove disposable operation file"))
        {
            bail!("cleanup failure lost typed error evidence: {value}");
        }
        Ok(())
    }

    #[test]
    fn operation_control_keeps_one_fixed_absolute_deadline() -> Result<()> {
        let root = CancellationToken::new();
        let deadline = Instant::now() + Duration::from_secs(60);
        let control = OperationControl::new(&root, deadline, None);
        let module_control = control.module_call_control();

        control.request_cancel();
        if control.deadline() != deadline || module_control.deadline() != deadline {
            bail!("cancellation reset the operation's absolute deadline");
        }
        control.request_deadline();
        if control.deadline() != deadline || module_control.deadline() != deadline {
            bail!("deadline request replaced the operation's absolute deadline");
        }
        Ok(())
    }

    #[tokio::test]
    async fn panic_and_runtime_abort_have_distinct_terminal_evidence() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let request = storage_download_request(&directory.path().join("download.bin"))?;

        let panicked = TestOperation::new(
            "task-panic",
            &request,
            Instant::now() + Duration::from_secs(30),
            None,
        )?;
        let mut tasks: JoinSet<OperationTaskExit> = JoinSet::new();
        let mut descriptors = HashMap::new();
        let task = spawn_operation_task(
            &mut tasks,
            &panicked.shared,
            &panicked.operation_id,
            async move {
                std::panic::resume_unwind(Box::new("supervisor task panic fixture"));
            },
        )?;
        descriptors.insert(
            task.id(),
            panicked.descriptor(OperationCleanup::default(), TaskKind::Execution)?,
        );
        join_and_handle(&mut tasks, &mut descriptors, &panicked.shared).await?;
        assert_terminal(&panicked, "failed", "task_panicked")?;

        let aborted = TestOperation::new(
            "task-abort",
            &request,
            Instant::now() + Duration::from_secs(30),
            None,
        )?;
        let mut tasks: JoinSet<OperationTaskExit> = JoinSet::new();
        let mut descriptors = HashMap::new();
        let task = spawn_operation_task(
            &mut tasks,
            &aborted.shared,
            &aborted.operation_id,
            async move { pending::<OperationTaskExit>().await },
        )?;
        descriptors.insert(
            task.id(),
            aborted.descriptor(OperationCleanup::default(), TaskKind::Execution)?,
        );
        task.abort();
        join_and_handle(&mut tasks, &mut descriptors, &aborted.shared).await?;
        assert_terminal(&aborted, "failed", "task_aborted")
    }

    #[tokio::test]
    async fn task_abort_waits_for_blocking_worker_before_cleanup() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let disposable_path = directory.path().join("blocking-worker.part");
        tokio::fs::write(&disposable_path, b"partial").await?;
        let disposable_file = OperationDisposableFile {
            path: disposable_path.clone(),
            created: Arc::new(AtomicBool::new(true)),
        };
        let request = storage_download_request(&directory.path().join("download.bin"))?;
        let operation = TestOperation::new(
            "task-abort-blocking-worker",
            &request,
            Instant::now() + Duration::from_secs(30),
            Some(disposable_file.clone()),
        )?;
        let worker_guard = operation.control.blocking_worker_guard()?;
        let worker_dropped = Arc::new(AtomicBool::new(false));
        let cleanup_raced_worker = Arc::new(AtomicBool::new(false));
        let live_file_use = LiveFileUse {
            path: disposable_path.clone(),
            dropped: Arc::clone(&worker_dropped),
            cleanup_raced: Arc::clone(&cleanup_raced_worker),
        };
        let (release_worker, observe_release) = sync_mpsc::channel();
        let (worker_started, observed_start) = oneshot::channel();
        let worker = tokio::task::spawn_blocking(move || {
            let _worker_guard = worker_guard;
            let _live_file_use = live_file_use;
            let _started_result = worker_started.send(());
            let _release_result = observe_release.recv();
        });
        observed_start
            .await
            .context("blocking operation worker did not start")?;

        let (outer_dropped, observed_outer_drop) = oneshot::channel();
        let mut tasks = JoinSet::new();
        let mut descriptors = HashMap::new();
        let task_operation_id = operation.operation_id.clone();
        let task = spawn_operation_task(
            &mut tasks,
            &operation.shared,
            &operation.operation_id,
            async move {
                let _outer_drop = OperationTaskCompletion(Some(outer_dropped));
                let _worker_result = worker.await;
                OperationTaskExit::Execution {
                    operation_id: task_operation_id,
                    result: OperationTaskResult::ExecutionFailed(
                        "aborted worker fixture completed unexpectedly".to_owned(),
                    ),
                }
            },
        )?;
        descriptors.insert(
            task.id(),
            operation.descriptor(
                OperationCleanup {
                    disposable_files: vec![disposable_file],
                },
                TaskKind::Execution,
            )?,
        );
        task.abort();

        let handling = join_and_handle(&mut tasks, &mut descriptors, &operation.shared);
        let release_after_outer_abort = async {
            let _outer_drop_result = observed_outer_drop.await;
            let file_existed = tokio::fs::try_exists(&disposable_path).await?;
            let worker_was_active = !worker_dropped.load(Ordering::Acquire);
            let release_succeeded = release_worker.send(()).is_ok();
            Ok::<_, anyhow::Error>((file_existed, worker_was_active, release_succeeded))
        };
        let (handling_result, release_result) = tokio::join!(handling, release_after_outer_abort);
        let (file_existed, worker_was_active, release_succeeded) = release_result?;
        handling_result?;

        if !file_existed || !worker_was_active || !release_succeeded {
            bail!("task cleanup crossed the blocking-worker termination barrier");
        }
        if !worker_dropped.load(Ordering::Acquire)
            || cleanup_raced_worker.load(Ordering::Acquire)
            || tokio::fs::try_exists(&disposable_path).await?
        {
            bail!("blocking worker did not finish before disposable-file cleanup");
        }
        assert_terminal(&operation, "failed", "task_aborted")
    }

    #[tokio::test]
    async fn normal_interruption_waits_for_blocking_worker_before_cleanup() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let disposable_path = directory.path().join("normal-interruption-worker.part");
        tokio::fs::write(&disposable_path, b"partial").await?;
        let disposable_file = OperationDisposableFile {
            path: disposable_path.clone(),
            created: Arc::new(AtomicBool::new(true)),
        };
        let request = storage_download_request(&directory.path().join("download.bin"))?;
        let operation = TestOperation::new(
            "normal-interruption-blocking-worker",
            &request,
            Instant::now() + Duration::from_secs(30),
            Some(disposable_file.clone()),
        )?;
        let worker_guard = operation.control.blocking_worker_guard()?;
        let worker_dropped = Arc::new(AtomicBool::new(false));
        let cleanup_raced_worker = Arc::new(AtomicBool::new(false));
        let live_file_use = LiveFileUse {
            path: disposable_path.clone(),
            dropped: Arc::clone(&worker_dropped),
            cleanup_raced: Arc::clone(&cleanup_raced_worker),
        };
        let (release_worker, observe_release) = sync_mpsc::channel();
        let (worker_started, observed_start) = oneshot::channel();
        let worker = tokio::task::spawn_blocking(move || {
            let _worker_guard = worker_guard;
            let _live_file_use = live_file_use;
            let _started_result = worker_started.send(());
            let _release_result = observe_release.recv();
        });
        observed_start
            .await
            .context("blocking operation worker did not start")?;
        drop(worker);

        operation.control.request_deadline();
        let mut tasks = JoinSet::new();
        let mut descriptors = HashMap::new();
        let task_operation_id = operation.operation_id.clone();
        let (task_finished, observed_task_finish) = oneshot::channel();
        let task = spawn_operation_task(
            &mut tasks,
            &operation.shared,
            &operation.operation_id,
            async move {
                let _finished_result = task_finished.send(());
                OperationTaskExit::Execution {
                    operation_id: task_operation_id,
                    result: OperationTaskResult::Interrupted(OperationInterrupted::local_only(
                        OperationStopReason::DeadlineExceeded,
                        "supervisor deadline interrupted execution",
                    )),
                }
            },
        )?;
        descriptors.insert(
            task.id(),
            operation.descriptor(
                OperationCleanup {
                    disposable_files: vec![disposable_file],
                },
                TaskKind::Execution,
            )?,
        );
        observed_task_finish
            .await
            .context("normal interruption task did not resolve")?;
        while !task.is_finished() {
            tokio::task::yield_now().await;
        }

        let handling = join_and_handle(&mut tasks, &mut descriptors, &operation.shared);
        let release_after_join = async {
            let file_existed = tokio::fs::try_exists(&disposable_path).await?;
            let worker_was_active = !worker_dropped.load(Ordering::Acquire);
            let release_succeeded = release_worker.send(()).is_ok();
            Ok::<_, anyhow::Error>((file_existed, worker_was_active, release_succeeded))
        };
        let (handling_result, release_result) = tokio::join!(handling, release_after_join);
        let (file_existed, worker_was_active, release_succeeded) = release_result?;
        handling_result?;

        if !file_existed || !worker_was_active || !release_succeeded {
            bail!("normal interruption crossed the blocking-worker termination barrier");
        }
        if !worker_dropped.load(Ordering::Acquire)
            || cleanup_raced_worker.load(Ordering::Acquire)
            || tokio::fs::try_exists(&disposable_path).await?
        {
            bail!("normal interruption cleaned before its blocking worker stopped");
        }
        assert_terminal(&operation, "timed_out", "timeout")
    }

    #[tokio::test]
    async fn accepted_operation_is_retained_until_correlated_event() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let request = accepted_request(&directory.path().join("upload.bin"))?;
        let operation = TestOperation::new(
            "accepted-event",
            &request,
            Instant::now() + Duration::from_secs(30),
            None,
        )?;
        let mut tasks = JoinSet::new();
        let mut descriptors = HashMap::new();

        retain_accepted(&operation, "session-event", &mut tasks, &mut descriptors).await?;
        let retained = operation.value()?;
        if retained.get("status").and_then(Value::as_str) != Some("awaiting_external")
            || !operation.is_retained()?
            || tasks.len() != 1
        {
            bail!("accepted operation was not retained for its external event: {retained}");
        }

        let (_, settled) = operation
            .registry
            .ingest_module_event_with_settlement(module_terminal_event("session-event")?)?;
        if settled.as_ref() != Some(&operation.operation_id) {
            bail!("correlated terminal event did not settle the accepted operation");
        }
        operation.control.settle();
        join_and_handle(&mut tasks, &mut descriptors, &operation.shared).await?;

        assert_terminal(&operation, "completed", "external_completion")?;
        if operation.is_retained()? {
            bail!("externally completed operation remained supervised");
        }
        Ok(())
    }

    #[tokio::test]
    async fn accepted_operation_is_retained_until_deadline_or_shutdown() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let request = accepted_request(&directory.path().join("upload.bin"))?;

        let timed_out = TestOperation::new("accepted-timeout", &request, Instant::now(), None)?;
        let mut timeout_tasks = JoinSet::new();
        let mut timeout_descriptors = HashMap::new();
        retain_accepted(
            &timed_out,
            "session-timeout",
            &mut timeout_tasks,
            &mut timeout_descriptors,
        )
        .await?;
        if !timed_out.is_retained()? {
            bail!("accepted operation was released before its absolute deadline");
        }
        join_and_handle(
            &mut timeout_tasks,
            &mut timeout_descriptors,
            &timed_out.shared,
        )
        .await?;
        assert_terminal(&timed_out, "timed_out", "timeout")?;

        let shutdown = TestOperation::new(
            "accepted-shutdown",
            &request,
            Instant::now() + Duration::from_secs(30),
            None,
        )?;
        let mut shutdown_tasks = JoinSet::new();
        let mut shutdown_descriptors = HashMap::new();
        retain_accepted(
            &shutdown,
            "session-shutdown",
            &mut shutdown_tasks,
            &mut shutdown_descriptors,
        )
        .await?;
        request_shutdown_for_all(&shutdown.shared)?;
        shutdown.shared.root.cancel();
        join_and_handle(
            &mut shutdown_tasks,
            &mut shutdown_descriptors,
            &shutdown.shared,
        )
        .await?;
        assert_terminal(&shutdown, "failed", "shutdown")
    }

    #[tokio::test]
    async fn local_only_cancellation_remains_canceling_with_exact_evidence() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let request = storage_download_request(&directory.path().join("download.bin"))?;
        let operation = TestOperation::new(
            "local-only-cancel",
            &request,
            Instant::now() + Duration::from_secs(30),
            None,
        )?;
        operation.registry.transition(
            &operation.operation_id,
            RuntimeOperationTransition::CancelRequested,
        )?;
        operation.control.request_cancel();
        let mut tasks = JoinSet::new();
        let mut descriptors = HashMap::new();

        handle_task_exit(
            OperationTaskExit::Execution {
                operation_id: operation.operation_id.clone(),
                result: OperationTaskResult::Interrupted(OperationInterrupted::local_only(
                    OperationStopReason::CancelRequested,
                    "remote termination is unknown",
                )),
            },
            operation.descriptor(OperationCleanup::default(), TaskKind::Execution)?,
            &mut tasks,
            &mut descriptors,
            &operation.shared,
        )
        .await?;

        let value = operation.value()?;
        let event_page = operation
            .registry
            .events(&operation.operation_id, EventCursor::new(0))?;
        let has_unconfirmed_evidence = event_page
            .get("events")
            .and_then(Value::as_array)
            .is_some_and(|events| {
                events.iter().any(|event| {
                    event.get("phase").and_then(Value::as_str) == Some("cancellation_unconfirmed")
                })
            });
        if value.get("status").and_then(Value::as_str) != Some("canceling")
            || value.get("terminalReason") != Some(&Value::Null)
            || !has_unconfirmed_evidence
            || !operation.is_retained()?
        {
            bail!("local abandonment was promoted to terminal cancellation: {value}");
        }

        request_shutdown_for_all(&operation.shared)?;
        operation.shared.root.cancel();
        join_and_handle(&mut tasks, &mut descriptors, &operation.shared).await?;
        assert_terminal(&operation, "failed", "shutdown")?;
        assert_shutdown_preserves_cleanup_uncertainty(&operation, "remote termination is unknown")
    }

    #[tokio::test]
    async fn system_cleanup_unknown_retains_canceling_operation_lease() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let request = storage_download_request(&directory.path().join("download.bin"))?;
        let operation = TestOperation::new(
            "system-cleanup-unknown",
            &request,
            Instant::now() + Duration::from_secs(30),
            None,
        )?;
        let mut tasks = JoinSet::new();
        let mut descriptors = HashMap::new();

        handle_task_exit(
            OperationTaskExit::Execution {
                operation_id: operation.operation_id.clone(),
                result: OperationTaskResult::CleanupUnconfirmed(
                    "download cleanup could not prove remote termination".to_owned(),
                ),
            },
            operation.descriptor(OperationCleanup::default(), TaskKind::Execution)?,
            &mut tasks,
            &mut descriptors,
            &operation.shared,
        )
        .await?;

        let value = operation.value()?;
        let events = operation
            .registry
            .events(&operation.operation_id, EventCursor::new(0))?;
        let event_phases = events
            .get("events")
            .and_then(Value::as_array)
            .context("cleanup-unknown operation has no event history")?
            .iter()
            .filter_map(|event| event.get("phase").and_then(Value::as_str))
            .collect::<Vec<_>>();
        if value.get("status").and_then(Value::as_str) != Some("canceling")
            || value.get("terminalReason") != Some(&Value::Null)
            || !operation.is_retained()?
            || !event_phases.contains(&"cleanup_unconfirmed")
            || event_phases.contains(&"canceling")
        {
            bail!("cleanup-unknown operation released its lease: {value}");
        }

        request_shutdown_for_all(&operation.shared)?;
        operation.shared.root.cancel();
        join_and_handle(&mut tasks, &mut descriptors, &operation.shared).await?;
        assert_terminal(&operation, "failed", "shutdown")?;
        assert_shutdown_preserves_cleanup_uncertainty(
            &operation,
            "download cleanup could not prove remote termination",
        )
    }

    #[tokio::test]
    async fn cleanup_unknown_after_deadline_retains_exclusive_lease_until_shutdown() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let request = storage_download_request(&directory.path().join("download.bin"))?;
        let mut operation =
            TestOperation::new("expired-cleanup-unknown", &request, Instant::now(), None)?;
        operation.control.termination_handshake_grace = Duration::from_millis(1);
        let mut tasks = JoinSet::new();
        let mut descriptors = HashMap::new();

        handle_task_exit(
            OperationTaskExit::Execution {
                operation_id: operation.operation_id.clone(),
                result: OperationTaskResult::CleanupUnconfirmed(
                    "download cleanup could not prove remote termination after deadline".to_owned(),
                ),
            },
            operation.descriptor(OperationCleanup::default(), TaskKind::Execution)?,
            &mut tasks,
            &mut descriptors,
            &operation.shared,
        )
        .await?;
        join_and_handle(&mut tasks, &mut descriptors, &operation.shared).await?;

        let competing_id = RuntimeOperationId::parse("competing-storage-download")?;
        let competing = running_runtime_operation_record(competing_id.clone(), &request, 2)?;
        let admission_error = operation
            .registry
            .insert(competing.clone())
            .err()
            .context("cleanup uncertainty released the storage download exclusive group")?;
        let events = operation
            .registry
            .events(&operation.operation_id, EventCursor::new(0))?;
        let cleanup_evidence_count = events
            .get("events")
            .and_then(Value::as_array)
            .context("cleanup-uncertain operation has no event history")?
            .iter()
            .filter(|event| {
                event.get("phase").and_then(Value::as_str) == Some("cleanup_unconfirmed")
            })
            .count();
        if !admission_error
            .to_string()
            .contains("storage download operation is already running")
            || operation.value()?.get("status").and_then(Value::as_str) != Some("canceling")
            || !operation.is_retained()?
            || cleanup_evidence_count != 2
        {
            bail!("expired cleanup uncertainty lost its exclusive lease: {admission_error:#}");
        }

        request_shutdown_for_all(&operation.shared)?;
        operation.shared.root.cancel();
        join_and_handle(&mut tasks, &mut descriptors, &operation.shared).await?;
        assert_terminal(&operation, "failed", "shutdown")?;
        assert_shutdown_preserves_cleanup_uncertainty(
            &operation,
            "download cleanup could not prove remote termination after deadline",
        )?;
        operation.registry.insert(competing)?;
        if operation
            .registry
            .value(&competing_id)?
            .get("status")
            .and_then(Value::as_str)
            != Some("running")
        {
            bail!("shutdown did not release the cleanup-uncertain exclusive lease");
        }
        Ok(())
    }

    #[tokio::test]
    async fn adapter_close_retains_its_exact_terminal_reason() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let request = storage_download_request(&directory.path().join("download.bin"))?;
        let operation = TestOperation::new(
            "adapter-close",
            &request,
            Instant::now() + Duration::from_secs(30),
            None,
        )?;
        let mut tasks = JoinSet::new();
        let mut descriptors = HashMap::new();

        handle_task_exit(
            OperationTaskExit::Execution {
                operation_id: operation.operation_id.clone(),
                result: OperationTaskResult::AdapterClosed("host adapter closed".to_owned()),
            },
            operation.descriptor(OperationCleanup::default(), TaskKind::Execution)?,
            &mut tasks,
            &mut descriptors,
            &operation.shared,
        )
        .await?;

        assert_terminal(&operation, "failed", "adapter_closed")
    }

    #[test]
    fn controller_failures_terminalize_records_without_surface_shutdown() -> Result<()> {
        for (case, command) in [
            ("error", SupervisorCommand::InjectControllerError),
            ("panic", SupervisorCommand::InjectControllerPanic),
        ] {
            let runtime = Runtime::new().context("test runtime")?;
            let supervisor = RuntimeOperationSupervisor::new(RuntimeOperationRegistry::default());
            supervisor.ensure_controller(&runtime)?;
            let directory = tempfile::tempdir()?;
            let request = accepted_request(&directory.path().join("upload.bin"))?;
            let operation_id = RuntimeOperationId::parse(&format!("watcher-{case}"))?;
            let residual_path = directory.path().join(format!("watcher-{case}.part"));
            std::fs::write(&residual_path, b"partial")?;
            supervisor
                .inner
                .shared
                .registry
                .insert(running_runtime_operation_record(
                    operation_id.clone(),
                    &request,
                    1,
                )?)?;
            supervisor
                .inner
                .shared
                .registry
                .transition(&operation_id, RuntimeOperationTransition::Started)?;
            let control = OperationControl::new(
                &supervisor.inner.shared.root,
                Instant::now() + Duration::from_secs(30),
                Some(OperationDisposableFile {
                    path: residual_path.clone(),
                    created: Arc::new(AtomicBool::new(true)),
                }),
            );
            supervisor
                .inner
                .shared
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("test supervisor state unavailable"))?
                .controls
                .insert(operation_id.clone(), control);
            supervisor
                .controller_sender()?
                .try_send(command)
                .map_err(|_| anyhow::anyhow!("failed to inject controller failure"))?;

            runtime.block_on(async {
                tokio::time::timeout(Duration::from_secs(1), async {
                    loop {
                        let value = supervisor.inner.shared.registry.value(&operation_id)?;
                        if value.get("status").and_then(Value::as_str) == Some("failed") {
                            return Ok::<_, anyhow::Error>(value);
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .context("controller watcher did not terminalize residual operation")?
            })?;

            let value = supervisor.inner.shared.registry.value(&operation_id)?;
            if value.get("terminalReason").and_then(Value::as_str) != Some("task_aborted")
                || residual_path.try_exists()?
            {
                bail!("controller watcher exposed terminal state before cleanup: {value}");
            }
            {
                let state = supervisor
                    .inner
                    .shared
                    .state
                    .lock()
                    .map_err(|_| anyhow::anyhow!("test supervisor state unavailable"))?;
                if state.phase != SupervisorPhase::Closed
                    || state.shutdown_error.is_none()
                    || !state.controls.is_empty()
                    || !state.live_tasks.is_empty()
                {
                    bail!("controller watcher did not fail closed after {case}");
                }
            }
            let Err(admission_error) = supervisor.prepare(
                &runtime,
                RuntimeOperationId::parse(&format!("watcher-{case}-late"))?,
                &request,
            ) else {
                bail!("controller watcher admitted work after {case}");
            };
            if !admission_error.to_string().contains("shutting down") {
                bail!("controller watcher returned unexpected admission error: {admission_error}");
            }
            if supervisor.shutdown(&runtime).is_ok() {
                bail!("surface shutdown lost the retained controller {case}");
            }
        }
        Ok(())
    }

    #[test]
    fn concurrent_shutdown_waits_for_owner_and_propagates_controller_abort() -> Result<()> {
        let runtime = Arc::new(Runtime::new().context("test runtime")?);
        let supervisor = RuntimeOperationSupervisor::new(RuntimeOperationRegistry::default());
        supervisor.ensure_controller(runtime.as_ref())?;
        let directory = tempfile::tempdir()?;
        let request = accepted_request(&directory.path().join("upload.bin"))?;
        let residual_path = directory.path().join("controller-abort.part");
        std::fs::write(&residual_path, b"partial")?;

        for id in ["controller-abort-a", "controller-abort-b"] {
            let operation_id = RuntimeOperationId::parse(id)?;
            supervisor
                .inner
                .shared
                .registry
                .insert(running_runtime_operation_record(
                    operation_id.clone(),
                    &request,
                    1,
                )?)?;
            supervisor
                .inner
                .shared
                .registry
                .transition(&operation_id, RuntimeOperationTransition::Started)?;
            let control = OperationControl::new(
                &supervisor.inner.shared.root,
                Instant::now() + Duration::from_secs(30),
                (id == "controller-abort-a").then(|| OperationDisposableFile {
                    path: residual_path.clone(),
                    created: Arc::new(AtomicBool::new(true)),
                }),
            );
            supervisor
                .inner
                .shared
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("test supervisor state unavailable"))?
                .controls
                .insert(operation_id, control);
        }

        let worker_guard = {
            let state = supervisor
                .inner
                .shared
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("test supervisor state unavailable"))?;
            state
                .controls
                .get(&RuntimeOperationId::parse("controller-abort-a")?)
                .context("controller-abort worker control")?
                .blocking_worker_guard()?
        };
        let live_task_dropped = Arc::new(AtomicBool::new(false));
        let cleanup_raced_live_task = Arc::new(AtomicBool::new(false));
        let (release_worker, observe_release) = sync_mpsc::channel();
        let (worker_started, observed_worker_start) = oneshot::channel();
        let live_file_use = LiveFileUse {
            path: residual_path.clone(),
            dropped: Arc::clone(&live_task_dropped),
            cleanup_raced: Arc::clone(&cleanup_raced_live_task),
        };
        let blocking_worker = runtime.spawn_blocking(move || {
            let _worker_guard = worker_guard;
            let _live_file_use = live_file_use;
            let _started_result = worker_started.send(());
            let _release_result = observe_release.recv();
        });
        runtime
            .block_on(observed_worker_start)
            .context("blocking operation worker did not start")?;

        let (completion, completed) = oneshot::channel();
        let (wrapper_started, observed_wrapper_start) = oneshot::channel();
        let (wrapper_dropped, observed_wrapper_drop) = sync_mpsc::channel();
        let live_task = runtime.spawn(async move {
            let _completion = OperationTaskCompletion(Some(completion));
            let _wrapper_drop = DropSignal(Some(wrapper_dropped));
            let _started_result = wrapper_started.send(());
            let _worker_result = blocking_worker.await;
        });
        runtime
            .block_on(observed_wrapper_start)
            .context("live operation wrapper did not start")?;
        {
            let mut state = supervisor
                .inner
                .shared
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("test supervisor state unavailable"))?;
            state.live_tasks.insert(
                RuntimeOperationId::parse("controller-abort-a")?,
                LiveOperationTask {
                    task_id: live_task.id(),
                    abort: live_task.abort_handle(),
                    completion: completed,
                },
            );
        }

        let controller_abort = {
            let controller = supervisor
                .inner
                .controller
                .lock()
                .map_err(|_| anyhow::anyhow!("test supervisor controller state unavailable"))?;
            match &*controller {
                ControllerState::Running(controller) => controller.task.abort_handle(),
                ControllerState::Unstarted
                | ControllerState::Draining
                | ControllerState::Stopped => {
                    bail!("test supervisor controller was not running")
                }
            }
        };
        controller_abort.abort();

        let barrier = Arc::new(Barrier::new(3));
        let first_supervisor = supervisor.clone();
        let first_runtime = Arc::clone(&runtime);
        let first_barrier = Arc::clone(&barrier);
        let second_supervisor = supervisor.clone();
        let second_runtime = Arc::clone(&runtime);
        let second_barrier = Arc::clone(&barrier);
        let (
            first,
            second,
            wrapper_stopped_before_release,
            file_existed_before_release,
            worker_was_active_before_release,
            shutdown_waited_for_worker,
            worker_was_released,
        ) = std::thread::scope(|scope| {
            let first = scope.spawn(move || {
                let _barrier_leader = first_barrier.wait();
                first_supervisor
                    .shutdown(first_runtime.as_ref())
                    .map_err(|error| error.to_string())
            });
            let second = scope.spawn(move || {
                let _barrier_leader = second_barrier.wait();
                second_supervisor
                    .shutdown(second_runtime.as_ref())
                    .map_err(|error| error.to_string())
            });
            let _barrier_leader = barrier.wait();
            let wrapper_stopped_before_release = observed_wrapper_drop
                .recv_timeout(Duration::from_secs(1))
                .is_ok();
            let file_existed_before_release = residual_path.try_exists();
            let worker_was_active_before_release = !live_task_dropped.load(Ordering::Acquire);
            let shutdown_waited_for_worker = !first.is_finished() && !second.is_finished();
            let worker_was_released = release_worker.send(()).is_ok();
            let first = first
                .join()
                .map_err(|_| anyhow::anyhow!("first shutdown thread panicked"))?;
            let second = second
                .join()
                .map_err(|_| anyhow::anyhow!("second shutdown thread panicked"))?;
            Ok::<_, anyhow::Error>((
                first,
                second,
                wrapper_stopped_before_release,
                file_existed_before_release?,
                worker_was_active_before_release,
                shutdown_waited_for_worker,
                worker_was_released,
            ))
        })?;

        if !wrapper_stopped_before_release
            || !file_existed_before_release
            || !worker_was_active_before_release
            || !shutdown_waited_for_worker
            || !worker_was_released
        {
            bail!("controller abort crossed its blocking-worker termination barrier");
        }

        let Err(first_error) = first else {
            bail!("controller abort was hidden from first shutdown caller");
        };
        let Err(second_error) = second else {
            bail!("controller abort was hidden from concurrent shutdown caller");
        };
        if !first_error.contains("supervisor task failed")
            || !second_error.contains("supervisor task failed")
        {
            bail!("concurrent shutdown callers observed different evidence");
        }
        for id in ["controller-abort-a", "controller-abort-b"] {
            let value = supervisor
                .inner
                .shared
                .registry
                .value(&RuntimeOperationId::parse(id)?)?;
            if value.get("status").and_then(Value::as_str) != Some("failed")
                || value.get("terminalReason").and_then(Value::as_str) != Some("task_aborted")
            {
                bail!("controller abort left operation without terminal evidence: {value}");
            }
        }
        if residual_path.try_exists()? {
            bail!("controller abort terminalized before residual cleanup");
        }
        if !live_task_dropped.load(Ordering::Acquire) {
            bail!("controller abort detached a live operation task");
        }
        if cleanup_raced_live_task.load(Ordering::Acquire) {
            bail!("controller abort cleaned a disposable file before its live task stopped");
        }
        let Err(live_task_error) = runtime.block_on(live_task) else {
            bail!("controller abort did not abort its live operation task");
        };
        if !live_task_error.is_cancelled() {
            bail!("live operation task ended with unexpected evidence: {live_task_error}");
        }
        {
            let state = supervisor
                .inner
                .shared
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("test supervisor state unavailable"))?;
            if state.phase != SupervisorPhase::Closed
                || !state.controls.is_empty()
                || !state.live_tasks.is_empty()
            {
                bail!("controller abort did not close and drain supervisor state");
            }
        }
        let controller = supervisor
            .inner
            .controller
            .lock()
            .map_err(|_| anyhow::anyhow!("test supervisor controller state unavailable"))?;
        if !matches!(&*controller, ControllerState::Stopped) {
            bail!("controller abort did not leave stopped drain ownership");
        }
        Ok(())
    }
}
