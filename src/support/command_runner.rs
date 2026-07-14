use std::{
    collections::VecDeque,
    io::{self, ErrorKind},
    process::{Child, Command, ExitStatus, Output, Stdio},
    sync::{Arc, Condvar, LazyLock, Mutex, MutexGuard, mpsc},
    thread,
    time::{Duration, Instant},
};

#[cfg(all(unix, not(target_os = "fuchsia")))]
use std::process::{ChildStderr, ChildStdout};

use anyhow::{Context as _, Result, bail};
use tokio_util::sync::CancellationToken;

use super::work_tracker::{BlockingWorkGuard, BlockingWorkTracker};

const COMMAND_CAPTURE_LIMIT: usize = 16 * 1024 * 1024;
#[cfg(all(unix, not(target_os = "fuchsia")))]
const COMMAND_CAPTURE_POLL_BUDGET: usize = 256 * 1024;
#[cfg(all(unix, not(target_os = "fuchsia")))]
const COMMAND_CAPTURE_FINAL_BUDGET: usize = COMMAND_CAPTURE_LIMIT + 8192;
pub(crate) const MAX_CONCURRENT_COMMANDS: usize = 4;
const TERMINATION_RETRY_INTERVAL: Duration = Duration::from_millis(5);
const TERMINATION_RETRY_WINDOW: Duration = Duration::from_millis(250);
const TERMINATION_REAP_WINDOW: Duration = Duration::from_millis(250);
static COMMAND_BUDGET: LazyLock<CommandBudget> =
    LazyLock::new(|| CommandBudget::new(MAX_CONCURRENT_COMMANDS));
static COMMAND_RECOVERY: LazyLock<std::result::Result<mpsc::SyncSender<CommandRecovery>, String>> =
    LazyLock::new(start_command_recovery_worker);

pub(crate) struct CommandRunPolicy<'a> {
    pub(crate) label: &'a str,
    pub(crate) timeout: Duration,
    pub(crate) poll_interval: Duration,
    pub(crate) redactions: &'a [&'a str],
    pub(crate) output_limit: usize,
}

#[derive(Clone)]
pub(crate) struct CommandControl {
    cancellation: CancellationToken,
    deadline: Instant,
    blocking_work: Option<BlockingWorkTracker>,
}

impl CommandControl {
    pub(crate) fn new(cancellation: CancellationToken, deadline: Instant) -> Self {
        Self {
            cancellation,
            deadline,
            blocking_work: None,
        }
    }

    #[must_use]
    pub(crate) fn cancellation(&self) -> &CancellationToken {
        &self.cancellation
    }

    #[must_use]
    pub(crate) const fn deadline(&self) -> Instant {
        self.deadline
    }

    #[must_use]
    pub(crate) fn with_deadline(&self, deadline: Instant) -> Self {
        Self {
            cancellation: self.cancellation.clone(),
            deadline: self.deadline.min(deadline),
            blocking_work: self.blocking_work.clone(),
        }
    }

    #[must_use]
    pub(crate) fn with_blocking_work_tracker(mut self, tracker: BlockingWorkTracker) -> Self {
        self.blocking_work = Some(tracker);
        self
    }

    pub(crate) fn blocking_worker_guard(&self) -> Result<Option<BlockingWorkGuard>> {
        self.blocking_work
            .as_ref()
            .map(BlockingWorkTracker::worker_guard)
            .transpose()
    }

    pub(crate) fn check_active(&self) -> std::result::Result<(), CommandTerminated> {
        if self.cancellation.is_cancelled() {
            return Err(CommandTerminated::without_process(
                CommandStopReason::CancelRequested,
            ));
        }
        if Instant::now() >= self.deadline {
            return Err(CommandTerminated::without_process(
                CommandStopReason::DeadlineExceeded,
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandStopReason {
    CancelRequested,
    DeadlineExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandTerminationScope {
    NoProcess,
    DirectChild,
    ProcessGroup,
}

#[derive(Debug)]
pub(crate) struct CommandTerminated {
    reason: CommandStopReason,
    scope: CommandTerminationScope,
}

impl CommandTerminated {
    const fn without_process(reason: CommandStopReason) -> Self {
        Self {
            reason,
            scope: CommandTerminationScope::NoProcess,
        }
    }

    const fn after_reap(reason: CommandStopReason, scope: CommandTerminationScope) -> Self {
        Self { reason, scope }
    }

    #[must_use]
    pub(crate) const fn reason(&self) -> CommandStopReason {
        self.reason
    }

    #[must_use]
    pub(crate) const fn scope(&self) -> CommandTerminationScope {
        self.scope
    }
}

impl std::fmt::Display for CommandTerminated {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reason = match self.reason {
            CommandStopReason::CancelRequested => "cancellation requested",
            CommandStopReason::DeadlineExceeded => "deadline exceeded",
        };
        let evidence = match self.scope {
            CommandTerminationScope::NoProcess => "no child process was started",
            CommandTerminationScope::DirectChild => {
                "direct child terminated and reaped; descendant termination is not guaranteed"
            }
            CommandTerminationScope::ProcessGroup => {
                "local process group terminated and direct child reaped; remote effects are not guaranteed"
            }
        };
        write!(formatter, "command stopped after {reason}; {evidence}")
    }
}

impl std::error::Error for CommandTerminated {}

#[derive(Debug)]
pub(crate) struct CommandCleanupUnconfirmed {
    reason: Option<CommandStopReason>,
    scope: CommandTerminationScope,
    message: String,
}

impl CommandCleanupUnconfirmed {
    pub(crate) fn new(
        reason: Option<CommandStopReason>,
        scope: CommandTerminationScope,
        message: String,
    ) -> Self {
        Self {
            reason,
            scope,
            message,
        }
    }

    #[must_use]
    pub(crate) const fn reason(&self) -> Option<CommandStopReason> {
        self.reason
    }
}

impl std::fmt::Display for CommandCleanupUnconfirmed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}; cleanup scope: {:?}",
            self.message, self.scope
        )
    }
}

impl std::error::Error for CommandCleanupUnconfirmed {}

struct CommandBudget {
    inner: Arc<CommandBudgetInner>,
}

struct CommandBudgetInner {
    limit: usize,
    state: Mutex<CommandBudgetState>,
    available: Condvar,
}

struct CommandBudgetState {
    active: usize,
}

struct CommandPermit {
    budget: Arc<CommandBudgetInner>,
}

pub(crate) struct StreamingCommandPermit {
    _permit: CommandPermit,
}

pub(crate) fn acquire_streaming_command_permit(
    label: &str,
    control: &CommandControl,
) -> Result<StreamingCommandPermit> {
    let policy = CommandRunPolicy {
        label,
        timeout: Duration::ZERO,
        poll_interval: TERMINATION_RETRY_INTERVAL,
        redactions: &[],
        output_limit: 0,
    };
    let permit = COMMAND_BUDGET.acquire(&policy, Some(control), None)?;
    Ok(StreamingCommandPermit { _permit: permit })
}

impl CommandBudget {
    fn new(limit: usize) -> Self {
        Self {
            inner: Arc::new(CommandBudgetInner {
                limit,
                state: Mutex::new(CommandBudgetState { active: 0 }),
                available: Condvar::new(),
            }),
        }
    }

    fn acquire(
        &self,
        policy: &CommandRunPolicy<'_>,
        control: Option<&CommandControl>,
        relative_deadline: Option<Instant>,
    ) -> Result<CommandPermit> {
        let mut state = self.lock_state();
        loop {
            check_pre_spawn_control(policy, control, relative_deadline)?;
            if state.active < self.inner.limit {
                state.active += 1;
                return Ok(CommandPermit {
                    budget: Arc::clone(&self.inner),
                });
            }

            let wait_for = pre_spawn_wait_duration(policy, control, relative_deadline);
            let (next_state, _wait_result) =
                match self.inner.available.wait_timeout(state, wait_for) {
                    Ok(result) => result,
                    Err(poisoned) => poisoned.into_inner(),
                };
            state = next_state;
        }
    }

    fn lock_state(&self) -> MutexGuard<'_, CommandBudgetState> {
        match self.inner.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl Drop for CommandPermit {
    fn drop(&mut self) {
        let mut state = match self.budget.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        if state.active != 0 {
            state.active -= 1;
        }
        self.budget.available.notify_one();
    }
}

fn check_pre_spawn_control(
    policy: &CommandRunPolicy<'_>,
    control: Option<&CommandControl>,
    relative_deadline: Option<Instant>,
) -> Result<()> {
    if let Some(control) = control {
        control.check_active()?;
    } else if relative_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        bail!(
            "{} timed out after {} ms before process start",
            policy.label,
            policy.timeout.as_millis()
        );
    }
    Ok(())
}

fn pre_spawn_wait_duration(
    policy: &CommandRunPolicy<'_>,
    control: Option<&CommandControl>,
    relative_deadline: Option<Instant>,
) -> Duration {
    let cadence = if policy.poll_interval.is_zero() {
        Duration::from_millis(1)
    } else {
        policy.poll_interval
    };
    let deadline = control.map(CommandControl::deadline).or(relative_deadline);
    deadline.map_or(cadence, |deadline| {
        cadence.min(deadline.saturating_duration_since(Instant::now()))
    })
}

pub(crate) fn run_command(command: Command, policy: CommandRunPolicy<'_>) -> Result<Output> {
    run_command_inner(command, policy, None)
}

pub(crate) fn run_command_controlled(
    command: Command,
    policy: CommandRunPolicy<'_>,
    control: CommandControl,
) -> Result<Output> {
    run_command_inner(command, policy, Some(control))
}

fn run_command_inner(
    command: Command,
    policy: CommandRunPolicy<'_>,
    control: Option<CommandControl>,
) -> Result<Output> {
    run_command_inner_with(command, policy, control, &COMMAND_BUDGET, Child::try_wait)
}

fn run_command_inner_with<P>(
    command: Command,
    policy: CommandRunPolicy<'_>,
    control: Option<CommandControl>,
    budget: &CommandBudget,
    poll_child: P,
) -> Result<Output>
where
    P: FnMut(&mut Child) -> io::Result<Option<ExitStatus>>,
{
    run_command_inner_with_termination(
        command,
        policy,
        control,
        budget,
        poll_child,
        request_termination,
        Child::try_wait,
    )
}

fn run_command_inner_with_termination<P, R, W>(
    mut command: Command,
    policy: CommandRunPolicy<'_>,
    control: Option<CommandControl>,
    budget: &CommandBudget,
    mut poll_child: P,
    mut request_stop: R,
    mut poll_reap: W,
) -> Result<Output>
where
    P: FnMut(&mut Child) -> io::Result<Option<ExitStatus>>,
    R: FnMut(&mut Child, CommandTerminationScope) -> io::Result<CommandTerminationScope>,
    W: FnMut(&mut Child) -> io::Result<Option<ExitStatus>>,
{
    let started = Instant::now();
    let relative_deadline = if control.is_none() {
        Some(
            started
                .checked_add(policy.timeout)
                .context("command timeout overflow")?,
        )
    } else {
        None
    };
    let permit = budget.acquire(&policy, control.as_ref(), relative_deadline)?;
    let recovery = command_recovery_sender()?;
    check_pre_spawn_control(&policy, control.as_ref(), relative_deadline)?;
    let termination_scope = configure_termination_scope(&mut command);
    let capture_setup = CaptureSetup::configure(&mut command, policy.label)?;
    check_pre_spawn_control(&policy, control.as_ref(), relative_deadline)?;
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to run {}", policy.label))?;
    let mut capture = match capture_setup.start(&mut child, policy.label, termination_scope) {
        Ok(capture) => capture,
        Err(error) => {
            let cleanup = match terminate_and_reap_bounded_with(
                &mut child,
                policy.label,
                termination_scope,
                &mut request_stop,
                &mut poll_reap,
            ) {
                Ok(cleanup) => cleanup,
                Err(cleanup) => {
                    return Err(error).with_context(|| {
                        format!(
                            "failed to initialize {} output capture; termination also failed: {cleanup:#}",
                            policy.label
                        )
                    });
                }
            };
            return match cleanup {
                BoundedReapOutcome::Reaped(_) => Err(error),
                BoundedReapOutcome::CleanupUnconfirmed { scope, detail } => {
                    Err(cleanup_unconfirmed_error(
                        recovery,
                        child,
                        None,
                        permit,
                        None,
                        scope,
                        policy.label,
                        format!("output capture initialization failed: {error}; {detail}"),
                    ))
                }
            };
        }
    };

    loop {
        if let Err(error) = capture.drain_available() {
            let cleanup = match terminate_and_collect_bounded_with(
                &mut child,
                capture,
                policy.label,
                termination_scope,
                &mut request_stop,
                &mut poll_reap,
            ) {
                Ok(cleanup) => cleanup,
                Err(cleanup) => {
                    return Err(error).with_context(|| {
                        format!(
                            "failed to capture output from {}; termination also failed: {cleanup:#}",
                            policy.label
                        )
                    });
                }
            };
            return match cleanup {
                StoppedOutput::Completed(_) | StoppedOutput::Terminated { .. } => Err(error)
                    .with_context(|| format!("failed to capture output from {}", policy.label)),
                StoppedOutput::CleanupUnconfirmed {
                    capture,
                    scope,
                    detail,
                } => Err(cleanup_unconfirmed_error(
                    recovery,
                    child,
                    Some(capture),
                    permit,
                    None,
                    scope,
                    policy.label,
                    format!("output capture failed: {error}; {detail}"),
                )),
            };
        }

        let status = match poll_child(&mut child) {
            Ok(status) => status,
            Err(error) => {
                let cleanup = match terminate_and_collect_bounded_with(
                    &mut child,
                    capture,
                    policy.label,
                    termination_scope,
                    &mut request_stop,
                    &mut poll_reap,
                ) {
                    Ok(cleanup) => cleanup,
                    Err(cleanup) => {
                        return Err(error).with_context(|| {
                            format!(
                                "failed to poll {}; termination also failed: {cleanup:#}",
                                policy.label
                            )
                        });
                    }
                };
                return match cleanup {
                    StoppedOutput::Completed(_) | StoppedOutput::Terminated { .. } => {
                        Err(error).with_context(|| format!("failed to poll {}", policy.label))
                    }
                    StoppedOutput::CleanupUnconfirmed {
                        capture,
                        scope,
                        detail,
                    } => Err(cleanup_unconfirmed_error(
                        recovery,
                        child,
                        Some(capture),
                        permit,
                        None,
                        scope,
                        policy.label,
                        format!("child polling failed: {error}; {detail}"),
                    )),
                };
            }
        };
        if let Some(status) = status {
            let output = collect_exited_child(status, capture, policy.label)?;
            return validate_output(output, &policy);
        }

        if let Some(control) = control.as_ref() {
            let stop_reason = if control.cancellation().is_cancelled() {
                Some(CommandStopReason::CancelRequested)
            } else if Instant::now() >= control.deadline() {
                Some(CommandStopReason::DeadlineExceeded)
            } else {
                None
            };
            if let Some(reason) = stop_reason {
                return match terminate_and_collect_bounded_with(
                    &mut child,
                    capture,
                    policy.label,
                    termination_scope,
                    &mut request_stop,
                    &mut poll_reap,
                )? {
                    StoppedOutput::Completed(output) => validate_output(output, &policy),
                    StoppedOutput::Terminated { scope, .. } => {
                        Err(CommandTerminated::after_reap(reason, scope).into())
                    }
                    StoppedOutput::CleanupUnconfirmed {
                        capture,
                        scope,
                        detail,
                    } => Err(cleanup_unconfirmed_error(
                        recovery,
                        child,
                        Some(capture),
                        permit,
                        Some(reason),
                        scope,
                        policy.label,
                        detail,
                    )),
                };
            }
        }

        if relative_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            match terminate_and_collect_bounded_with(
                &mut child,
                capture,
                policy.label,
                termination_scope,
                &mut request_stop,
                &mut poll_reap,
            )? {
                StoppedOutput::Completed(output) => return validate_output(output, &policy),
                StoppedOutput::Terminated { output, .. } => {
                    let message = process_message(&output, policy.redactions, policy.output_limit);
                    bail!(
                        "{} timed out after {} ms: {}",
                        policy.label,
                        policy.timeout.as_millis(),
                        message
                    );
                }
                StoppedOutput::CleanupUnconfirmed {
                    capture,
                    scope,
                    detail,
                } => {
                    return Err(cleanup_unconfirmed_error(
                        recovery,
                        child,
                        Some(capture),
                        permit,
                        Some(CommandStopReason::DeadlineExceeded),
                        scope,
                        policy.label,
                        detail,
                    ));
                }
            }
        }
        thread::sleep(policy.poll_interval);
    }
}

fn validate_output(output: Output, policy: &CommandRunPolicy<'_>) -> Result<Output> {
    if !output.status.success() {
        let message = process_message(&output, policy.redactions, policy.output_limit);
        bail!("{} exited with {}: {message}", policy.label, output.status);
    }
    Ok(output)
}

struct CapturedOutput {
    bytes: Vec<u8>,
    truncated: bool,
}

impl CapturedOutput {
    #[cfg(all(unix, not(target_os = "fuchsia")))]
    const fn new() -> Self {
        Self {
            bytes: Vec::new(),
            truncated: false,
        }
    }

    #[cfg(all(unix, not(target_os = "fuchsia")))]
    fn retain(&mut self, bytes: &[u8]) {
        let remaining = COMMAND_CAPTURE_LIMIT.saturating_sub(self.bytes.len());
        self.bytes.extend(bytes.iter().take(remaining).copied());
        self.truncated |= bytes.len() > remaining;
    }
}

fn captured_bytes(
    stdout: CapturedOutput,
    stderr: CapturedOutput,
    label: &str,
) -> Result<(Vec<u8>, Vec<u8>)> {
    if stdout.truncated || stderr.truncated {
        bail!(
            "{label} output exceeded the {} byte capture limit",
            COMMAND_CAPTURE_LIMIT
        );
    }
    Ok((stdout.bytes, stderr.bytes))
}

#[cfg(any(not(unix), target_os = "fuchsia", test))]
fn enforce_capture_length(length: u64) -> io::Result<()> {
    let limit = u64::try_from(COMMAND_CAPTURE_LIMIT)
        .map_err(|_| io::Error::other("command capture limit cannot be represented as u64"))?;
    if length > limit {
        return Err(io::Error::other(format!(
            "command output exceeded the {COMMAND_CAPTURE_LIMIT} byte capture limit"
        )));
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "fuchsia")))]
struct CaptureSetup;

#[cfg(all(unix, not(target_os = "fuchsia")))]
impl CaptureSetup {
    fn configure(command: &mut Command, _label: &str) -> Result<Self> {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        Ok(Self)
    }

    fn start(
        self,
        child: &mut Child,
        label: &str,
        termination_scope: CommandTerminationScope,
    ) -> Result<OutputCapture> {
        OutputCapture::start(child, label, termination_scope)
    }
}

#[cfg(all(unix, not(target_os = "fuchsia")))]
struct OutputCapture {
    stdout: PipeCapture<ChildStdout>,
    stderr: PipeCapture<ChildStderr>,
}

#[cfg(all(unix, not(target_os = "fuchsia")))]
impl OutputCapture {
    fn start(
        child: &mut Child,
        label: &str,
        termination_scope: CommandTerminationScope,
    ) -> Result<Self> {
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                return capture_start_failure(
                    child,
                    label,
                    termination_scope,
                    anyhow::anyhow!("failed to capture {label} stdout"),
                );
            }
        };
        let stderr = match child.stderr.take() {
            Some(stderr) => stderr,
            None => {
                return capture_start_failure(
                    child,
                    label,
                    termination_scope,
                    anyhow::anyhow!("failed to capture {label} stderr"),
                );
            }
        };
        if let Err(error) = set_nonblocking(&stdout) {
            return capture_start_failure(
                child,
                label,
                termination_scope,
                anyhow::Error::from(error)
                    .context(format!("failed to configure {label} stdout capture")),
            );
        }
        if let Err(error) = set_nonblocking(&stderr) {
            return capture_start_failure(
                child,
                label,
                termination_scope,
                anyhow::Error::from(error)
                    .context(format!("failed to configure {label} stderr capture")),
            );
        }
        Ok(Self {
            stdout: PipeCapture::new(stdout),
            stderr: PipeCapture::new(stderr),
        })
    }

    fn drain_available(&mut self) -> io::Result<()> {
        self.stdout.drain(COMMAND_CAPTURE_POLL_BUDGET)?;
        self.stderr.drain(COMMAND_CAPTURE_POLL_BUDGET)
    }

    fn finish(mut self, label: &str) -> Result<(Vec<u8>, Vec<u8>)> {
        self.stdout
            .drain(COMMAND_CAPTURE_FINAL_BUDGET)
            .with_context(|| format!("failed to read {label} stdout"))?;
        self.stderr
            .drain(COMMAND_CAPTURE_FINAL_BUDGET)
            .with_context(|| format!("failed to read {label} stderr"))?;
        captured_bytes(self.stdout.output, self.stderr.output, label)
    }
}

#[cfg(all(unix, not(target_os = "fuchsia")))]
struct PipeCapture<R> {
    reader: R,
    output: CapturedOutput,
    eof: bool,
}

#[cfg(all(unix, not(target_os = "fuchsia")))]
impl<R> PipeCapture<R>
where
    R: io::Read,
{
    const fn new(reader: R) -> Self {
        Self {
            reader,
            output: CapturedOutput::new(),
            eof: false,
        }
    }

    fn drain(&mut self, budget: usize) -> io::Result<()> {
        let mut remaining = budget;
        let mut buffer = [0_u8; 8192];
        while !self.eof && remaining != 0 {
            let read_limit = remaining.min(buffer.len());
            let target = buffer.get_mut(..read_limit).ok_or_else(|| {
                io::Error::other("command capture budget exceeded its read buffer")
            })?;
            match self.reader.read(target) {
                Ok(0) => self.eof = true,
                Ok(read) => {
                    self.output.retain(target.get(..read).ok_or_else(|| {
                        io::Error::other("command capture read exceeded its buffer")
                    })?);
                    remaining = remaining.saturating_sub(read);
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

#[cfg(all(unix, not(target_os = "fuchsia")))]
fn set_nonblocking<F>(descriptor: &F) -> io::Result<()>
where
    F: std::os::fd::AsFd,
{
    use nix::fcntl::{FcntlArg, OFlag, fcntl};

    let current = fcntl(descriptor, FcntlArg::F_GETFL).map_err(io::Error::from)?;
    let flags = OFlag::from_bits_truncate(current) | OFlag::O_NONBLOCK;
    fcntl(descriptor, FcntlArg::F_SETFL(flags))
        .map(drop)
        .map_err(io::Error::from)
}

#[cfg(all(unix, not(target_os = "fuchsia")))]
fn capture_start_failure<T>(
    _child: &mut Child,
    _label: &str,
    _termination_scope: CommandTerminationScope,
    error: anyhow::Error,
) -> Result<T> {
    Err(error)
}

#[cfg(any(not(unix), target_os = "fuchsia"))]
struct CaptureSetup {
    stdout: tempfile::NamedTempFile,
    stderr: tempfile::NamedTempFile,
}

#[cfg(any(not(unix), target_os = "fuchsia"))]
impl CaptureSetup {
    fn configure(command: &mut Command, label: &str) -> Result<Self> {
        let stdout = tempfile::NamedTempFile::new()
            .with_context(|| format!("failed to create {label} stdout capture"))?;
        let stderr = tempfile::NamedTempFile::new()
            .with_context(|| format!("failed to create {label} stderr capture"))?;
        command.stdout(Stdio::from(
            stdout
                .reopen()
                .with_context(|| format!("failed to open {label} stdout capture"))?,
        ));
        command.stderr(Stdio::from(
            stderr
                .reopen()
                .with_context(|| format!("failed to open {label} stderr capture"))?,
        ));
        Ok(Self { stdout, stderr })
    }

    fn start(
        self,
        _child: &mut Child,
        _label: &str,
        _termination_scope: CommandTerminationScope,
    ) -> Result<OutputCapture> {
        Ok(OutputCapture {
            stdout: self.stdout,
            stderr: self.stderr,
        })
    }
}

#[cfg(any(not(unix), target_os = "fuchsia"))]
struct OutputCapture {
    stdout: tempfile::NamedTempFile,
    stderr: tempfile::NamedTempFile,
}

#[cfg(any(not(unix), target_os = "fuchsia"))]
impl OutputCapture {
    fn drain_available(&mut self) -> io::Result<()> {
        enforce_capture_length(self.stdout.as_file().metadata()?.len())?;
        enforce_capture_length(self.stderr.as_file().metadata()?.len())
    }

    fn finish(mut self, label: &str) -> Result<(Vec<u8>, Vec<u8>)> {
        let stdout = read_file_capture(&mut self.stdout)
            .with_context(|| format!("failed to read {label} stdout"))?;
        let stderr = read_file_capture(&mut self.stderr)
            .with_context(|| format!("failed to read {label} stderr"))?;
        captured_bytes(stdout, stderr, label)
    }
}

#[cfg(any(not(unix), target_os = "fuchsia"))]
fn read_file_capture(file: &mut tempfile::NamedTempFile) -> io::Result<CapturedOutput> {
    use std::io::{Read as _, Seek as _, SeekFrom};

    let limit = u64::try_from(COMMAND_CAPTURE_LIMIT)
        .map_err(|_| io::Error::other("command capture limit cannot be represented as u64"))?;
    let initial_length = file.as_file().metadata()?.len();
    file.as_file_mut().seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::with_capacity(COMMAND_CAPTURE_LIMIT.min(64 * 1024));
    {
        let mut bounded = file.as_file_mut().take(limit);
        bounded.read_to_end(&mut bytes)?;
    }
    let final_length = file.as_file().metadata()?.len();
    Ok(CapturedOutput {
        bytes,
        truncated: initial_length > limit || final_length > limit,
    })
}

fn collect_exited_child(status: ExitStatus, capture: OutputCapture, label: &str) -> Result<Output> {
    output_from_capture(status, capture, label)
}

#[cfg(all(unix, not(target_os = "fuchsia")))]
fn configure_termination_scope(command: &mut Command) -> CommandTerminationScope {
    use std::os::unix::process::CommandExt as _;

    command.process_group(0);
    CommandTerminationScope::ProcessGroup
}

#[cfg(any(not(unix), target_os = "fuchsia"))]
fn configure_termination_scope(_command: &mut Command) -> CommandTerminationScope {
    CommandTerminationScope::DirectChild
}

enum ReapOutcome {
    Terminated {
        status: ExitStatus,
        scope: CommandTerminationScope,
    },
    Completed(ExitStatus),
}

impl ReapOutcome {
    const fn status(&self) -> ExitStatus {
        match self {
            Self::Terminated { status, .. } | Self::Completed(status) => *status,
        }
    }
}

enum StoppedOutput {
    Terminated {
        output: Output,
        scope: CommandTerminationScope,
    },
    Completed(Output),
    CleanupUnconfirmed {
        capture: OutputCapture,
        scope: CommandTerminationScope,
        detail: String,
    },
}

enum BoundedReapOutcome {
    Reaped(ReapOutcome),
    CleanupUnconfirmed {
        scope: CommandTerminationScope,
        detail: String,
    },
}

struct CommandRecovery {
    child: Child,
    capture: Option<OutputCapture>,
    termination_scope: CommandTerminationScope,
    _permit: CommandPermit,
}

impl Drop for CommandRecovery {
    fn drop(&mut self) {
        if let Some(capture) = self.capture.as_mut() {
            let _capture_result = capture.drain_available();
        }
        let _termination_result = request_termination(&mut self.child, self.termination_scope);
        let _reap_result = self.child.try_wait();
    }
}

fn start_command_recovery_worker() -> std::result::Result<mpsc::SyncSender<CommandRecovery>, String>
{
    let (sender, receiver) = mpsc::sync_channel(MAX_CONCURRENT_COMMANDS);
    thread::Builder::new()
        .name("command-process-recovery".to_owned())
        .spawn(move || run_command_recovery_queue(&receiver))
        .map_err(|error| format!("failed to start command recovery worker: {error}"))?;
    Ok(sender)
}

fn command_recovery_sender() -> Result<mpsc::SyncSender<CommandRecovery>> {
    match &*COMMAND_RECOVERY {
        Ok(sender) => Ok(sender.clone()),
        Err(error) => bail!(error.clone()),
    }
}

fn handoff_command_recovery(
    sender: mpsc::SyncSender<CommandRecovery>,
    recovery: CommandRecovery,
) -> std::result::Result<(), CommandRecovery> {
    sender.try_send(recovery).map_err(|error| match error {
        mpsc::TrySendError::Full(recovery) | mpsc::TrySendError::Disconnected(recovery) => recovery,
    })
}

fn run_command_recovery_queue(receiver: &mpsc::Receiver<CommandRecovery>) {
    let mut pending = VecDeque::new();
    loop {
        if pending.is_empty() {
            match receiver.recv() {
                Ok(recovery) => pending.push_back(recovery),
                Err(_) => return,
            }
        }
        pending.extend(receiver.try_iter());
        let pass_count = pending.len();
        for _ in 0..pass_count {
            let Some(mut recovery) = pending.pop_front() else {
                break;
            };
            if !recover_command_process(&mut recovery) {
                pending.push_back(recovery);
            }
        }
        if !pending.is_empty() {
            thread::sleep(TERMINATION_RETRY_INTERVAL);
        }
    }
}

fn recover_command_process(recovery: &mut CommandRecovery) -> bool {
    if let Some(capture) = recovery.capture.as_mut() {
        let _capture_result = capture.drain_available();
    }
    let _termination_result = request_termination(&mut recovery.child, recovery.termination_scope);
    match recovery.child.try_wait() {
        Ok(Some(_)) => true,
        Ok(None) | Err(_) => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn cleanup_unconfirmed_error(
    sender: mpsc::SyncSender<CommandRecovery>,
    child: Child,
    capture: Option<OutputCapture>,
    permit: CommandPermit,
    reason: Option<CommandStopReason>,
    scope: CommandTerminationScope,
    label: &str,
    detail: String,
) -> anyhow::Error {
    let recovery_status = match handoff_command_recovery(
        sender,
        CommandRecovery {
            child,
            capture,
            termination_scope: scope,
            _permit: permit,
        },
    ) {
        Ok(()) => "process recovery ownership accepted",
        Err(_recovery) => {
            "process recovery ownership was unavailable; one final nonblocking cleanup attempt was made"
        }
    };
    CommandCleanupUnconfirmed::new(
        reason,
        scope,
        format!(
            "{label} cleanup was not confirmed within {} ms after command stop: {detail}; {recovery_status}",
            TERMINATION_REAP_WINDOW.as_millis(),
        ),
    )
    .into()
}

fn terminate_and_collect_bounded_with<R, W>(
    child: &mut Child,
    capture: OutputCapture,
    label: &str,
    termination_scope: CommandTerminationScope,
    request_stop: &mut R,
    poll_reap: &mut W,
) -> Result<StoppedOutput>
where
    R: FnMut(&mut Child, CommandTerminationScope) -> io::Result<CommandTerminationScope>,
    W: FnMut(&mut Child) -> io::Result<Option<ExitStatus>>,
{
    match terminate_and_reap_bounded_with(child, label, termination_scope, request_stop, poll_reap)?
    {
        BoundedReapOutcome::Reaped(outcome) => {
            let output = output_from_capture(outcome.status(), capture, label)?;
            match outcome {
                ReapOutcome::Terminated { scope, .. } => {
                    Ok(StoppedOutput::Terminated { output, scope })
                }
                ReapOutcome::Completed(_) => Ok(StoppedOutput::Completed(output)),
            }
        }
        BoundedReapOutcome::CleanupUnconfirmed { scope, detail } => {
            Ok(StoppedOutput::CleanupUnconfirmed {
                capture,
                scope,
                detail,
            })
        }
    }
}

fn terminate_and_reap_bounded_with<R, W>(
    child: &mut Child,
    label: &str,
    termination_scope: CommandTerminationScope,
    request_stop: &mut R,
    poll_reap: &mut W,
) -> Result<BoundedReapOutcome>
where
    R: FnMut(&mut Child, CommandTerminationScope) -> io::Result<CommandTerminationScope>,
    W: FnMut(&mut Child) -> io::Result<Option<ExitStatus>>,
{
    let mut last_error = match poll_reap(child) {
        Ok(Some(status)) => {
            return Ok(BoundedReapOutcome::Reaped(ReapOutcome::Completed(status)));
        }
        Ok(None) => None,
        Err(error) => Some(error),
    };
    let started = Instant::now();
    let deadline = started
        .checked_add(TERMINATION_REAP_WINDOW)
        .unwrap_or(started);
    let request_retry_window = TERMINATION_RETRY_WINDOW.min(TERMINATION_REAP_WINDOW / 2);
    let request_retry_deadline = started.checked_add(request_retry_window).unwrap_or(started);
    let mut requested_scope = None;
    loop {
        if requested_scope.is_none() {
            match request_stop(child, termination_scope) {
                Ok(scope) => requested_scope = Some(scope),
                Err(error) => last_error = Some(error),
            }
            if requested_scope.is_none() && Instant::now() >= request_retry_deadline {
                match child.kill() {
                    Ok(()) => requested_scope = Some(CommandTerminationScope::DirectChild),
                    Err(error) => last_error = Some(error),
                }
            }
        }
        match poll_reap(child) {
            Ok(Some(status)) => {
                let outcome = requested_scope.map_or_else(
                    || ReapOutcome::Completed(status),
                    |scope| requested_reap_outcome(status, scope),
                );
                return Ok(BoundedReapOutcome::Reaped(outcome));
            }
            Ok(None) => {}
            Err(error) => last_error = Some(error),
        }
        if Instant::now() >= deadline {
            break;
        }
        thread::sleep(TERMINATION_RETRY_INTERVAL);
    }
    if requested_scope.is_none() {
        match child.kill() {
            Ok(()) => requested_scope = Some(CommandTerminationScope::DirectChild),
            Err(error) => last_error = Some(error),
        }
        if let Ok(Some(status)) = poll_reap(child) {
            let outcome = requested_scope.map_or_else(
                || ReapOutcome::Completed(status),
                |scope| requested_reap_outcome(status, scope),
            );
            return Ok(BoundedReapOutcome::Reaped(outcome));
        }
    }
    let scope = requested_scope.unwrap_or(termination_scope);
    let detail = last_error.map_or_else(
        || format!("{label} direct child was not reaped after termination request"),
        |error| format!("{label} direct child was not reaped; last cleanup error: {error}"),
    );
    Ok(BoundedReapOutcome::CleanupUnconfirmed { scope, detail })
}

fn output_from_capture(status: ExitStatus, capture: OutputCapture, label: &str) -> Result<Output> {
    let (stdout, stderr) = capture.finish(label)?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

#[cfg(test)]
fn terminate_and_reap_with<F>(
    child: &mut Child,
    label: &str,
    termination_scope: CommandTerminationScope,
    mut request: F,
) -> Result<ReapOutcome>
where
    F: FnMut(&mut Child, CommandTerminationScope) -> io::Result<CommandTerminationScope>,
{
    let mut poll = Child::try_wait;
    match terminate_and_reap_bounded_with(child, label, termination_scope, &mut request, &mut poll)?
    {
        BoundedReapOutcome::Reaped(outcome) => Ok(outcome),
        BoundedReapOutcome::CleanupUnconfirmed { detail, .. } => bail!(detail),
    }
}

fn requested_reap_outcome(status: ExitStatus, scope: CommandTerminationScope) -> ReapOutcome {
    #[cfg(unix)]
    if status.code().is_some() {
        return ReapOutcome::Completed(status);
    }
    ReapOutcome::Terminated { status, scope }
}

#[cfg(all(unix, not(target_os = "fuchsia")))]
fn request_termination(
    child: &mut Child,
    scope: CommandTerminationScope,
) -> io::Result<CommandTerminationScope> {
    use nix::{
        errno::Errno,
        sys::signal::{Signal, killpg},
        unistd::Pid,
    };

    if scope != CommandTerminationScope::ProcessGroup {
        child.kill()?;
        return Ok(CommandTerminationScope::DirectChild);
    }
    let process_group = i32::try_from(child.id()).map_err(|_| {
        io::Error::new(
            ErrorKind::InvalidData,
            "child process id cannot be represented as a Unix process group",
        )
    })?;
    match killpg(Pid::from_raw(process_group), Signal::SIGKILL) {
        Ok(()) => Ok(CommandTerminationScope::ProcessGroup),
        Err(Errno::ESRCH) => Err(io::Error::new(
            ErrorKind::NotFound,
            "child process group no longer exists",
        )),
        Err(error) => Err(io::Error::from(error)),
    }
}

#[cfg(any(not(unix), target_os = "fuchsia"))]
fn request_termination(
    child: &mut Child,
    _scope: CommandTerminationScope,
) -> io::Result<CommandTerminationScope> {
    child.kill()?;
    Ok(CommandTerminationScope::DirectChild)
}

pub(crate) fn process_message(output: &Output, redactions: &[&str], limit: usize) -> String {
    let message = if output.stderr.is_empty() {
        output_text(&output.stdout, redactions, limit)
    } else {
        output_text(&output.stderr, redactions, limit)
    };
    if message.is_empty() {
        "no output".to_owned()
    } else {
        message
    }
}

pub(crate) fn output_text(output: &[u8], redactions: &[&str], limit: usize) -> String {
    let text = String::from_utf8_lossy(output).trim().to_owned();
    let mut redacted = text;
    for value in redactions {
        let value = value.trim();
        if !value.is_empty() {
            redacted = redacted.replace(value, "...");
        }
    }
    redacted.chars().take(limit).collect()
}

#[cfg(all(test, unix, not(target_os = "fuchsia")))]
mod tests {
    use std::{
        fs,
        path::Path,
        process::Stdio,
        sync::{Arc, mpsc},
        thread,
        time::{Duration, Instant},
    };

    use anyhow::{Context as _, Result, bail};

    #[cfg(target_os = "linux")]
    use std::path::PathBuf;

    use super::*;

    const TEST_TIMEOUT: Duration = Duration::from_secs(5);

    fn test_policy(poll_interval: Duration) -> CommandRunPolicy<'static> {
        CommandRunPolicy {
            label: "test command",
            timeout: TEST_TIMEOUT,
            poll_interval,
            redactions: &[],
            output_limit: 256,
        }
    }

    fn shell_command(script: &str, arguments: &[&Path]) -> Command {
        let mut command = Command::new("sh");
        command.arg("-c").arg(script).arg("command-runner-test");
        command.args(arguments);
        command
    }

    fn wait_for_path(path: &Path, timeout: Duration) -> Result<()> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .context("test wait deadline overflow")?;
        while !path.exists() {
            if Instant::now() >= deadline {
                bail!("timed out waiting for {}", path.display());
            }
            thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }

    fn assert_process_gone(pid_path: &Path, timeout: Duration) -> Result<()> {
        let pid = fs::read_to_string(pid_path)
            .with_context(|| format!("failed to read process pid from {}", pid_path.display()))?;
        let deadline = Instant::now()
            .checked_add(timeout)
            .context("process probe deadline overflow")?;
        loop {
            let status = Command::new("sh")
                .arg("-c")
                .arg("kill -0 \"$1\" 2>/dev/null")
                .arg("command-runner-reap-probe")
                .arg(pid.trim())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .context("failed to probe process pid")?;
            if !status.success() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                bail!("process {} still exists after command return", pid.trim());
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[cfg(target_os = "linux")]
    fn process_exists(pid_path: &Path) -> Result<bool> {
        let pid = fs::read_to_string(pid_path)
            .with_context(|| format!("failed to read process pid from {}", pid_path.display()))?;
        Command::new("sh")
            .arg("-c")
            .arg("kill -0 \"$1\" 2>/dev/null")
            .arg("command-runner-existence-probe")
            .arg(pid.trim())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("failed to probe process pid")
            .map(|status| status.success())
    }

    #[cfg(target_os = "linux")]
    struct EscapedProcessCleanup {
        pid_path: PathBuf,
    }

    #[cfg(target_os = "linux")]
    impl Drop for EscapedProcessCleanup {
        fn drop(&mut self) {
            let Ok(pid) = fs::read_to_string(&self.pid_path) else {
                return;
            };
            let process_group = format!("-{}", pid.trim());
            let _cleanup_status = Command::new("kill")
                .arg("-KILL")
                .arg("--")
                .arg(process_group)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }

    #[cfg(target_os = "linux")]
    fn escaped_pipe_holder_command(ready: &Path, direct_pid: &Path, escaped_pid: &Path) -> Command {
        let script = r#"
            printf '%s' "$$" > "$2"
            setsid sh -c 'printf "%s" "$$" > "$1"; sleep 30' command-runner-escaped "$3" &
            while [ ! -f "$3" ]; do :; done
            printf ready > "$1"
            while :; do :; done
        "#;
        shell_command(script, &[ready, direct_pid, escaped_pid])
    }

    fn assert_termination(error: &anyhow::Error, expected_reason: CommandStopReason) -> Result<()> {
        let terminated = error
            .downcast_ref::<CommandTerminated>()
            .context("expected typed command termination")?;
        if terminated.reason() != expected_reason {
            bail!(
                "unexpected stop reason: expected {expected_reason:?}, got {:?}",
                terminated.reason()
            );
        }
        if terminated.scope() != CommandTerminationScope::ProcessGroup {
            bail!(
                "unexpected Unix termination scope: {:?}",
                terminated.scope()
            );
        }
        let message = terminated.to_string();
        if !message.contains("local process group terminated and direct child reaped")
            || !message.contains("remote effects are not guaranteed")
        {
            bail!("termination scope missing from error: {message}");
        }
        Ok(())
    }

    fn command_error(result: Result<Output>, unexpected_success: &str) -> Result<anyhow::Error> {
        match result {
            Ok(_) => bail!("{unexpected_success}"),
            Err(error) => Ok(error),
        }
    }

    #[test]
    fn concurrent_large_stdout_and_stderr_are_drained_without_pipe_deadlock() -> Result<()> {
        const STREAM_BYTES: usize = 256 * 1024;
        let script = r#"
            (head -c 262144 /dev/zero | tr '\000' o) &
            stdout_pid=$!
            (head -c 262144 /dev/zero | tr '\000' e >&2) &
            stderr_pid=$!
            wait "$stdout_pid"
            wait "$stderr_pid"
        "#;

        let output = run_command(
            shell_command(script, &[]),
            test_policy(Duration::from_millis(2)),
        )?;

        if output.stdout.len() != STREAM_BYTES
            || output.stderr.len() != STREAM_BYTES
            || output.stdout.iter().any(|byte| *byte != b'o')
            || output.stderr.iter().any(|byte| *byte != b'e')
        {
            bail!(
                "unexpected concurrent capture sizes/content: stdout={}, stderr={}",
                output.stdout.len(),
                output.stderr.len()
            );
        }
        Ok(())
    }

    #[test]
    fn cancellation_terminates_pipe_inheriting_process_group_without_hanging() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let ready = directory.path().join("ready");
        let direct_pid = directory.path().join("direct-pid");
        let descendant_pid = directory.path().join("descendant-pid");
        let direct_late_effect = directory.path().join("direct-late-effect");
        let descendant_effect = directory.path().join("descendant-effect");
        let script = r#"
            printf '%s' "$$" > "$2"
            sh -c 'printf "%s" "$$" > "$1"; sleep 2; printf descendant > "$2"' command-runner-descendant "$5" "$4" &
            while [ ! -f "$5" ]; do :; done
            printf ready > "$1"
            while :; do :; done
            printf direct > "$3"
        "#;
        let cancellation = CancellationToken::new();
        let cancellation_request = cancellation.clone();
        let ready_for_request = ready.clone();
        let requester = thread::spawn(move || -> Result<()> {
            wait_for_path(&ready_for_request, TEST_TIMEOUT)?;
            cancellation_request.cancel();
            Ok(())
        });
        let control = CommandControl::new(
            cancellation,
            Instant::now()
                .checked_add(TEST_TIMEOUT)
                .context("test command deadline overflow")?,
        );

        let started = Instant::now();
        let result = run_command_controlled(
            shell_command(
                script,
                &[
                    &ready,
                    &direct_pid,
                    &direct_late_effect,
                    &descendant_effect,
                    &descendant_pid,
                ],
            ),
            test_policy(Duration::from_millis(1)),
            control,
        );
        let elapsed = started.elapsed();
        requester
            .join()
            .map_err(|_| anyhow::anyhow!("cancellation requester panicked"))??;
        let error = command_error(result, "canceled command unexpectedly completed")?;

        assert_termination(&error, CommandStopReason::CancelRequested)?;
        assert_process_gone(&direct_pid, Duration::from_secs(2))?;
        assert_process_gone(&descendant_pid, Duration::from_secs(2))?;
        if elapsed >= Duration::from_secs(1) {
            bail!("pipe-inheriting descendant delayed cancellation by {elapsed:?}");
        }
        if direct_late_effect.exists() || descendant_effect.exists() {
            bail!("process group performed late side effect after cancellation");
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn escaped_pipe_holder_cannot_delay_cancellation_capture_teardown() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let ready = directory.path().join("ready");
        let direct_pid = directory.path().join("direct-pid");
        let escaped_pid = directory.path().join("escaped-pid");
        let _escaped_cleanup = EscapedProcessCleanup {
            pid_path: escaped_pid.clone(),
        };
        let cancellation = CancellationToken::new();
        let cancellation_request = cancellation.clone();
        let ready_for_request = ready.clone();
        let requester = thread::spawn(move || -> Result<()> {
            wait_for_path(&ready_for_request, TEST_TIMEOUT)?;
            cancellation_request.cancel();
            Ok(())
        });
        let control = CommandControl::new(
            cancellation,
            Instant::now()
                .checked_add(TEST_TIMEOUT)
                .context("escaped cancellation deadline overflow")?,
        );

        let started = Instant::now();
        let result = run_command_controlled(
            escaped_pipe_holder_command(&ready, &direct_pid, &escaped_pid),
            test_policy(Duration::from_millis(1)),
            control,
        );
        let elapsed = started.elapsed();
        requester
            .join()
            .map_err(|_| anyhow::anyhow!("escaped cancellation requester panicked"))??;
        let error = command_error(result, "escaped-holder command unexpectedly completed")?;

        assert_termination(&error, CommandStopReason::CancelRequested)?;
        assert_process_gone(&direct_pid, Duration::from_secs(2))?;
        if !process_exists(&escaped_pid)? {
            bail!("escaped pipe holder did not survive the direct process-group teardown");
        }
        if elapsed >= Duration::from_secs(1) {
            bail!("escaped pipe holder delayed cancellation capture teardown by {elapsed:?}");
        }
        Ok(())
    }

    #[test]
    fn absolute_deadline_reaps_direct_child_with_deadline_evidence() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let pid = directory.path().join("pid");
        let script = r#"
            printf '%s' "$$" > "$1"
            while :; do :; done
        "#;
        let cancellation = CancellationToken::new();
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(500))
            .context("test command deadline overflow")?;
        let started = Instant::now();

        let result = run_command_controlled(
            shell_command(script, &[&pid]),
            test_policy(Duration::from_millis(1)),
            CommandControl::new(cancellation, deadline),
        );
        let elapsed = started.elapsed();
        let error = command_error(result, "deadline-bound command unexpectedly completed")?;

        assert_termination(&error, CommandStopReason::DeadlineExceeded)?;
        assert_process_gone(&pid, Duration::from_secs(2))?;
        if elapsed >= Duration::from_secs(2) {
            bail!("absolute deadline termination took {elapsed:?}");
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn escaped_pipe_holder_cannot_delay_deadline_capture_teardown() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let ready = directory.path().join("ready");
        let direct_pid = directory.path().join("direct-pid");
        let escaped_pid = directory.path().join("escaped-pid");
        let _escaped_cleanup = EscapedProcessCleanup {
            pid_path: escaped_pid.clone(),
        };
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(400))
            .context("escaped command deadline overflow")?;
        let started = Instant::now();

        // This regression owns its command budget: the assertion below is
        // specifically about post-spawn process-group teardown, while the
        // shared production budget may legitimately expire this short control
        // deadline before spawning when unrelated command tests run in parallel.
        let budget = CommandBudget::new(1);
        let result = run_command_inner_with(
            escaped_pipe_holder_command(&ready, &direct_pid, &escaped_pid),
            test_policy(Duration::from_millis(1)),
            Some(CommandControl::new(CancellationToken::new(), deadline)),
            &budget,
            Child::try_wait,
        );
        let elapsed = started.elapsed();
        let error = command_error(
            result,
            "escaped-holder deadline command unexpectedly completed",
        )?;

        assert_termination(&error, CommandStopReason::DeadlineExceeded)?;
        assert_process_gone(&direct_pid, Duration::from_secs(2))?;
        if !process_exists(&escaped_pid)? {
            bail!("escaped deadline pipe holder did not survive process-group teardown");
        }
        if elapsed >= Duration::from_secs(1) {
            bail!("escaped pipe holder delayed deadline capture teardown by {elapsed:?}");
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn poll_error_with_escaped_pipe_holder_leaves_no_reader_work() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let ready = directory.path().join("ready");
        let direct_pid = directory.path().join("direct-pid");
        let escaped_pid = directory.path().join("escaped-pid");
        let _escaped_cleanup = EscapedProcessCleanup {
            pid_path: escaped_pid.clone(),
        };
        let ready_for_poll = ready.clone();
        let mut injected = false;
        let budget = CommandBudget::new(1);
        let started = Instant::now();

        let result = run_command_inner_with(
            escaped_pipe_holder_command(&ready, &direct_pid, &escaped_pid),
            test_policy(Duration::from_millis(1)),
            None,
            &budget,
            move |child| {
                if ready_for_poll.exists() && !injected {
                    injected = true;
                    Err(io::Error::other("injected child poll failure"))
                } else {
                    child.try_wait()
                }
            },
        );
        let elapsed = started.elapsed();
        let error = command_error(result, "poll-error command unexpectedly completed")?;

        if !error.to_string().contains("failed to poll test command") {
            bail!("poll error lost its primary context: {error:#}");
        }
        assert_process_gone(&direct_pid, Duration::from_secs(2))?;
        if !process_exists(&escaped_pid)? {
            bail!("escaped poll-error pipe holder did not survive process-group teardown");
        }
        if elapsed >= Duration::from_secs(1) {
            bail!("escaped pipe holder left capture work running for {elapsed:?}");
        }
        Ok(())
    }

    #[test]
    fn controlled_absolute_deadline_supersedes_relative_policy_timeout() -> Result<()> {
        let cancellation = CancellationToken::new();
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(2))
            .context("test command deadline overflow")?;
        let mut policy = test_policy(Duration::from_millis(1));
        policy.timeout = Duration::from_millis(1);

        let output = run_command_controlled(
            shell_command("sleep 0.05; printf success", &[]),
            policy,
            CommandControl::new(cancellation, deadline),
        )?;

        if output.stdout != b"success" {
            bail!("unexpected controlled output: {:?}", output.stdout);
        }
        Ok(())
    }

    #[test]
    fn derived_command_control_preserves_parent_deadline_token_and_work_tracker() -> Result<()> {
        let cancellation = CancellationToken::new();
        let parent_deadline = Instant::now()
            .checked_add(Duration::from_secs(2))
            .context("parent control deadline overflow")?;
        let tracker = BlockingWorkTracker::new();
        let control = CommandControl::new(cancellation.clone(), parent_deadline)
            .with_blocking_work_tracker(tracker.clone());
        let derived = control.with_deadline(
            Instant::now()
                .checked_add(Duration::from_secs(10))
                .context("derived control deadline overflow")?,
        );
        let guard = derived
            .blocking_worker_guard()?
            .context("derived control lost blocking-work tracker")?;

        if derived.deadline() != parent_deadline {
            bail!("derived control extended its parent deadline");
        }
        cancellation.cancel();
        let stopped = derived
            .check_active()
            .err()
            .context("derived control lost shared cancellation")?;
        if stopped.reason() != CommandStopReason::CancelRequested
            || stopped.scope() != CommandTerminationScope::NoProcess
        {
            bail!("unexpected pre-spawn stop evidence: {stopped}");
        }
        tracker.stop_accepting();
        drop(guard);
        tracker.wait_idle();
        if control.blocking_worker_guard().is_ok() {
            bail!("parent control did not share the closed work tracker");
        }
        Ok(())
    }

    #[test]
    fn fifth_command_canceled_behind_global_limit_never_spawns() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let budget = Arc::new(CommandBudget::new(MAX_CONCURRENT_COMMANDS));
        let mut ready_paths = Vec::new();
        let mut release_paths = Vec::new();
        let mut workers = Vec::new();

        for index in 0..MAX_CONCURRENT_COMMANDS {
            let ready = directory.path().join(format!("ready-{index}"));
            let release = directory.path().join(format!("release-{index}"));
            ready_paths.push(ready.clone());
            release_paths.push(release.clone());
            let worker_budget = Arc::clone(&budget);
            workers.push(thread::spawn(move || {
                run_command_inner_with(
                    shell_command(
                        "printf ready > \"$1\"; while [ ! -f \"$2\" ]; do sleep 0.01; done; printf released",
                        &[&ready, &release],
                    ),
                    test_policy(Duration::from_millis(1)),
                    None,
                    &worker_budget,
                    Child::try_wait,
                )
            }));
        }
        for ready in &ready_paths {
            wait_for_path(ready, TEST_TIMEOUT)?;
        }

        let marker = directory.path().join("fifth-spawned");
        let cancellation = CancellationToken::new();
        let cancellation_request = cancellation.clone();
        let fifth_budget = Arc::clone(&budget);
        let marker_for_worker = marker.clone();
        let (result_sender, result_receiver) = mpsc::channel();
        let fifth = thread::spawn(move || {
            let deadline_started = Instant::now();
            let control = CommandControl::new(
                cancellation,
                deadline_started
                    .checked_add(TEST_TIMEOUT)
                    .unwrap_or(deadline_started),
            );
            let result = run_command_inner_with(
                shell_command("printf spawned > \"$1\"", &[&marker_for_worker]),
                test_policy(Duration::from_millis(1)),
                Some(control),
                &fifth_budget,
                Child::try_wait,
            );
            let _result_sent = result_sender.send(result);
        });
        thread::sleep(Duration::from_millis(50));
        let spawned_before_cancel = marker.exists();
        cancellation_request.cancel();
        let fifth_result = result_receiver.recv_timeout(Duration::from_secs(1));

        for release in &release_paths {
            fs::write(release, b"release")?;
        }
        for worker in workers {
            let output = worker
                .join()
                .map_err(|_| anyhow::anyhow!("budget-holder command panicked"))??;
            if output.stdout != b"released" {
                bail!("budget-holder command returned unexpected output");
            }
        }
        fifth
            .join()
            .map_err(|_| anyhow::anyhow!("fifth command worker panicked"))?;
        let fifth_result = fifth_result.context(
            "fifth command did not observe cancellation while waiting for command budget",
        )?;
        let error = command_error(fifth_result, "fifth command unexpectedly completed")?;
        let terminated = error
            .downcast_ref::<CommandTerminated>()
            .context("budget cancellation lost typed command termination")?;

        if spawned_before_cancel || marker.exists() {
            bail!("fifth command spawned while all command permits were occupied");
        }
        if terminated.reason() != CommandStopReason::CancelRequested
            || terminated.scope() != CommandTerminationScope::NoProcess
        {
            bail!("unexpected queued-command termination: {terminated}");
        }
        Ok(())
    }

    #[test]
    fn termination_request_failure_is_retried_before_direct_child_fallback() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let pid = directory.path().join("pid");
        let mut command =
            shell_command("printf '%s' \"$$\" > \"$1\"; while :; do :; done", &[&pid]);
        let termination_scope = configure_termination_scope(&mut command);
        command.stdout(Stdio::null()).stderr(Stdio::null());
        let mut child = command
            .spawn()
            .context("failed to spawn termination test child")?;
        wait_for_path(&pid, TEST_TIMEOUT)?;
        let mut attempts = 0_usize;

        let outcome = terminate_and_reap_with(
            &mut child,
            "termination test child",
            termination_scope,
            |_child, _scope| {
                attempts = attempts.saturating_add(1);
                Err(io::Error::new(
                    ErrorKind::PermissionDenied,
                    "injected termination failure",
                ))
            },
        )?;

        match outcome {
            ReapOutcome::Terminated {
                scope: CommandTerminationScope::DirectChild,
                ..
            } => {}
            _ => bail!("termination failure did not use direct-child fallback"),
        }
        if attempts < 2 {
            bail!("termination request was not retried: {attempts} attempt(s)");
        }
        assert_process_gone(&pid, Duration::from_secs(2))?;
        Ok(())
    }

    #[test]
    fn stalled_reap_is_handed_off_with_typed_cleanup_uncertainty() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let pid = directory.path().join("pid");
        let budget = CommandBudget::new(1);
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(50))
            .context("stalled-reap command deadline overflow")?;
        let started = Instant::now();

        let result = run_command_inner_with_termination(
            shell_command("printf '%s' \"$$\" > \"$1\"; while :; do :; done", &[&pid]),
            test_policy(Duration::from_millis(1)),
            Some(CommandControl::new(CancellationToken::new(), deadline)),
            &budget,
            Child::try_wait,
            request_termination,
            |_child| Ok(None),
        );
        let elapsed = started.elapsed();
        let error = command_error(result, "stalled-reap command unexpectedly completed")?;

        let cleanup = error
            .downcast_ref::<CommandCleanupUnconfirmed>()
            .context("stalled reap lost typed cleanup uncertainty")?;
        if cleanup.reason() != Some(CommandStopReason::DeadlineExceeded) {
            bail!("stalled reap lost its stop reason: {cleanup}");
        }
        if elapsed >= Duration::from_secs(1) {
            bail!("stalled reap exceeded bounded handoff budget: {elapsed:?}");
        }
        assert_process_gone(&pid, Duration::from_secs(2))?;
        Ok(())
    }

    #[test]
    fn initial_reap_poll_error_is_killed_and_handed_to_recovery() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let pid = directory.path().join("pid");
        let budget = CommandBudget::new(1);
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(50))
            .context("poll-error command deadline overflow")?;

        let result = run_command_inner_with_termination(
            shell_command("printf '%s' \"$$\" > \"$1\"; while :; do :; done", &[&pid]),
            test_policy(Duration::from_millis(1)),
            Some(CommandControl::new(CancellationToken::new(), deadline)),
            &budget,
            Child::try_wait,
            request_termination,
            |_child| Err(io::Error::other("injected initial reap poll failure")),
        );
        let error = command_error(result, "reap-poll-error command unexpectedly completed")?;
        let cleanup = error
            .downcast_ref::<CommandCleanupUnconfirmed>()
            .context("initial reap poll error lost typed cleanup uncertainty")?;

        if cleanup.reason() != Some(CommandStopReason::DeadlineExceeded)
            || !cleanup
                .to_string()
                .contains("injected initial reap poll failure")
        {
            bail!("initial reap poll error lost cleanup evidence: {cleanup}");
        }
        assert_process_gone(&pid, Duration::from_secs(2))?;
        Ok(())
    }

    #[test]
    fn recovery_owner_retains_command_admission_until_reap() -> Result<()> {
        let budget = CommandBudget::new(1);
        let policy = test_policy(Duration::from_millis(1));
        let permit = budget.acquire(&policy, None, Some(Instant::now() + TEST_TIMEOUT))?;
        let mut command = shell_command("while :; do :; done", &[]);
        let termination_scope = configure_termination_scope(&mut command);
        let capture_setup = CaptureSetup::configure(&mut command, policy.label)?;
        let mut child = command.spawn()?;
        let capture = capture_setup.start(&mut child, policy.label, termination_scope)?;
        let mut recovery = CommandRecovery {
            child,
            capture: Some(capture),
            termination_scope,
            _permit: permit,
        };
        let blocked_deadline = Instant::now()
            .checked_add(Duration::from_millis(50))
            .context("recovery admission deadline overflow")?;

        let blocked = budget
            .acquire(
                &policy,
                Some(&CommandControl::new(
                    CancellationToken::new(),
                    blocked_deadline,
                )),
                None,
            )
            .err()
            .context("recovery owner released command admission before reap")?;
        let blocked = blocked
            .downcast_ref::<CommandTerminated>()
            .context("recovery admission wait lost typed termination")?;
        if blocked.reason() != CommandStopReason::DeadlineExceeded
            || blocked.scope() != CommandTerminationScope::NoProcess
        {
            bail!("unexpected recovery admission evidence: {blocked}");
        }

        let reap_deadline = Instant::now()
            .checked_add(Duration::from_secs(2))
            .context("recovery owner reap deadline overflow")?;
        while !recover_command_process(&mut recovery) {
            if Instant::now() >= reap_deadline {
                bail!("recovery owner did not reap its retained command");
            }
            thread::sleep(TERMINATION_RETRY_INTERVAL);
        }
        drop(recovery);
        let available = budget.acquire(
            &policy,
            Some(&CommandControl::new(
                CancellationToken::new(),
                Instant::now() + TEST_TIMEOUT,
            )),
            None,
        )?;
        drop(available);
        Ok(())
    }

    #[test]
    fn natural_exit_after_successful_termination_request_is_completed() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let ready = directory.path().join("ready");
        let release = directory.path().join("release");
        let mut command = shell_command(
            "printf ready > \"$1\"; while [ ! -f \"$2\" ]; do :; done; exit 23",
            &[&ready, &release],
        );
        let termination_scope = configure_termination_scope(&mut command);
        command.stdout(Stdio::null()).stderr(Stdio::null());
        let mut child = command
            .spawn()
            .context("failed to spawn natural-exit race child")?;
        wait_for_path(&ready, TEST_TIMEOUT)?;

        let outcome = terminate_and_reap_with(
            &mut child,
            "natural-exit race child",
            termination_scope,
            |_child, scope| {
                fs::write(&release, b"release")?;
                Ok(scope)
            },
        )?;

        let ReapOutcome::Completed(status) = outcome else {
            bail!("natural child exit was misclassified as requested termination");
        };
        if status.code() != Some(23) {
            bail!("natural child exit status was not preserved: {status}");
        }
        let output = Output {
            status,
            stdout: Vec::new(),
            stderr: b"natural failure".to_vec(),
        };
        let error = command_error(
            validate_output(output, &test_policy(Duration::from_millis(1))),
            "natural nonzero child exit unexpectedly validated",
        )?;
        if !error.to_string().contains("natural failure") {
            bail!("natural child error output was not preserved: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn completed_child_wins_over_concurrent_cancellation() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let ready = directory.path().join("ready");
        let release = directory.path().join("release");
        let completed = directory.path().join("completed");
        let script = r#"
            sleep 0.1
            printf ready > "$1"
            while [ ! -f "$2" ]; do :; done
            printf completed > "$3"
            printf success
        "#;
        let cancellation = CancellationToken::new();
        let cancellation_request = cancellation.clone();
        let ready_for_request = ready.clone();
        let release_for_request = release.clone();
        let completed_for_request = completed.clone();
        let requester = thread::spawn(move || -> Result<()> {
            wait_for_path(&ready_for_request, TEST_TIMEOUT)?;
            fs::write(&release_for_request, b"release")?;
            wait_for_path(&completed_for_request, TEST_TIMEOUT)?;
            cancellation_request.cancel();
            Ok(())
        });
        let control = CommandControl::new(
            cancellation,
            Instant::now()
                .checked_add(TEST_TIMEOUT)
                .context("test command deadline overflow")?,
        );

        let result = run_command_controlled(
            shell_command(script, &[&ready, &release, &completed]),
            test_policy(Duration::from_millis(300)),
            control,
        );
        requester
            .join()
            .map_err(|_| anyhow::anyhow!("completion requester panicked"))??;
        let output = result?;

        if output.stdout != b"success" {
            bail!("unexpected completion output: {:?}", output.stdout);
        }
        Ok(())
    }

    #[test]
    fn output_beyond_capture_limit_is_drained_then_rejected() -> Result<()> {
        let count = (COMMAND_CAPTURE_LIMIT + 1).to_string();
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("head -c \"$1\" /dev/zero")
            .arg("command-runner-test")
            .arg(count);

        let error = command_error(
            run_command(command, test_policy(Duration::from_millis(2))),
            "oversized command output unexpectedly succeeded",
        )?;

        let expected =
            format!("test command output exceeded the {COMMAND_CAPTURE_LIMIT} byte capture limit");
        if error.to_string() != expected {
            bail!("unexpected capture-limit error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn file_capture_length_guard_rejects_first_byte_beyond_limit() -> Result<()> {
        let limit = u64::try_from(COMMAND_CAPTURE_LIMIT)
            .context("test capture limit cannot be represented as u64")?;
        enforce_capture_length(limit)?;
        let error = enforce_capture_length(limit.saturating_add(1))
            .err()
            .context("capture length guard accepted oversized output")?;
        let expected =
            format!("command output exceeded the {COMMAND_CAPTURE_LIMIT} byte capture limit");
        if error.to_string() != expected {
            bail!("unexpected capture length error: {error}");
        }
        Ok(())
    }
}
