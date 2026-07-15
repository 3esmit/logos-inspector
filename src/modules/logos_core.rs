use std::{
    collections::VecDeque,
    env, fs,
    future::Future,
    io::{ErrorKind, Read as _},
    path::{Path, PathBuf},
    pin::Pin,
    process::{Child, Command, Stdio},
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant as StdInstant},
};

use anyhow::{Context as _, Result, bail};
use serde::{Serialize, Serializer};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::support::command_runner::{
    CommandControl, CommandRunPolicy, CommandStopReason, CommandTerminated,
    CommandTerminationScope, StreamingCommandPermit, acquire_streaming_command_permit, output_text,
    run_command, run_command_controlled,
};
use crate::support::settings_backup::SETTINGS_BACKUP_MAX_BYTES;
use crate::support::work_tracker::{BlockingWorkGuard, BlockingWorkTracker};

const LOGOSCORE_POLL_INTERVAL: Duration = Duration::from_millis(25);
const LOGOSCORE_OUTPUT_LIMIT: usize = 4096;
const LOGOSCORE_JSON_OUTPUT_LIMIT: usize = 16 * 1024 * 1024;
const LOGOSCORE_EVENT_LINE_LIMIT: usize = 1024 * 1024;
const LOGOSCORE_EVENT_FIELD_LIMIT: usize = 64;
const LOGOSCORE_EVENT_QUEUE_CAPACITY: usize = 64;
const LOGOSCORE_WATCH_STOP_GRACE: Duration = Duration::from_millis(250);
const LOGOSCORE_WATCH_PROTOCOL: &str = "logoscore.watch";
const LOGOSCORE_WATCH_PROTOCOL_VERSION: u64 = 1;
static LOGOSCORE_WATCH_RECOVERY: LazyLock<
    std::result::Result<mpsc::Sender<LogoscoreWatchRecovery>, String>,
> = LazyLock::new(start_watch_recovery_worker);

#[derive(Debug, Clone, Serialize)]
pub struct LogosCoreOutput {
    pub runner: String,
    pub value: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogoscoreModuleMethod {
    name: String,
    signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogoscoreModuleDiscovery {
    module: String,
    methods: Vec<LogoscoreModuleMethod>,
    events: Vec<LogoscoreModuleMethod>,
}

impl LogoscoreModuleDiscovery {
    pub(crate) fn require_method(&self, method: &str, signature: &str) -> Result<()> {
        let Some(found) = self
            .methods
            .iter()
            .find(|candidate| candidate.name == method)
        else {
            bail!(
                "logoscore module `{}` does not expose invokable method `{method}`",
                self.module
            );
        };
        if found.signature != signature {
            bail!(
                "logoscore module `{}` method `{method}` signature mismatch: expected `{signature}`, found `{}`",
                self.module,
                found.signature
            );
        }
        Ok(())
    }

    pub(crate) fn require_event(&self, event: &str, signature: &str) -> Result<()> {
        if self
            .events
            .iter()
            .any(|candidate| candidate.name == event && candidate.signature == signature)
        {
            return Ok(());
        }
        bail!(
            "logoscore module `{}` does not expose event `{signature}`",
            self.module,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleTransportKind {
    Module,
    LogoscoreCli,
}

impl ModuleTransportKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::LogoscoreCli => "logoscore_cli",
        }
    }
}

impl Serialize for ModuleTransportKind {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BridgeCallbackId(u64);

impl BridgeCallbackId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModuleCall {
    transport: ModuleTransportKind,
    module: String,
    method: String,
    args: Vec<Value>,
}

impl ModuleCall {
    pub fn new(
        transport: ModuleTransportKind,
        module: impl Into<String>,
        method: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<Self> {
        let module = module.into();
        let method = method.into();
        if module.trim().is_empty() {
            bail!("module name is required");
        }
        if method.trim().is_empty() {
            bail!("method name is required");
        }
        Ok(Self {
            transport,
            module,
            method,
            args,
        })
    }

    #[must_use]
    pub const fn transport(&self) -> ModuleTransportKind {
        self.transport
    }

    #[must_use]
    pub fn module(&self) -> &str {
        &self.module
    }

    #[must_use]
    pub fn method(&self) -> &str {
        &self.method
    }

    #[must_use]
    pub fn args(&self) -> &[Value] {
        &self.args
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModuleCallReply {
    transport: ModuleTransportKind,
    value: Value,
    bridge_callback_id: Option<BridgeCallbackId>,
}

impl ModuleCallReply {
    #[must_use]
    pub const fn new(transport: ModuleTransportKind, value: Value) -> Self {
        Self {
            transport,
            value,
            bridge_callback_id: None,
        }
    }

    #[must_use]
    pub const fn with_bridge_callback(mut self, bridge_callback_id: BridgeCallbackId) -> Self {
        self.bridge_callback_id = Some(bridge_callback_id);
        self
    }

    #[must_use]
    pub const fn transport(&self) -> ModuleTransportKind {
        self.transport
    }

    #[must_use]
    pub const fn bridge_callback_id(&self) -> Option<BridgeCallbackId> {
        self.bridge_callback_id
    }

    #[must_use]
    pub fn into_value(self) -> Value {
        self.value
    }
}

pub type ModuleCallFuture<'a> = Pin<Box<dyn Future<Output = Result<ModuleCallReply>> + Send + 'a>>;
pub type ModuleDiagnosticFuture<'a> = Pin<Box<dyn Future<Output = Result<Value>> + Send + 'a>>;
pub type ModuleTransportResult<T> = Result<T>;
pub type BoxedModuleEventSubscription = Box<dyn ModuleEventSubscription>;
pub type SharedModuleTransport = Arc<dyn ModuleTransport>;

#[derive(Debug, Clone, PartialEq)]
pub struct ModuleTransportEvent {
    module: String,
    event: String,
    args: Vec<Value>,
}

impl ModuleTransportEvent {
    pub fn new(
        module: impl Into<String>,
        event: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<Self> {
        let module = module.into();
        let event = event.into();
        if module.trim().is_empty() {
            bail!("module event module name is required");
        }
        if event.trim().is_empty() {
            bail!("module event name is required");
        }
        Ok(Self {
            module,
            event,
            args,
        })
    }

    #[must_use]
    pub fn module(&self) -> &str {
        &self.module
    }

    #[must_use]
    pub fn event(&self) -> &str {
        &self.event
    }

    #[must_use]
    pub fn args(&self) -> &[Value] {
        &self.args
    }
}

pub trait ModuleEventSubscription: Send {
    fn next_within(&mut self, timeout: Duration) -> Result<Option<ModuleTransportEvent>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleCallStopReason {
    CancelRequested,
    DeadlineExceeded,
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleCallTerminationEvidence {
    ProcessTerminated,
    RemoteEffectTerminationConfirmed,
    LocallyAbandoned,
    NotStarted,
}

#[derive(Clone)]
pub struct ModuleCallControl {
    cancellation: CancellationToken,
    deadline: Instant,
    stop_reason: Arc<AtomicU8>,
    blocking_work: BlockingWorkTracker,
}

impl ModuleCallControl {
    pub(crate) fn new(
        cancellation: CancellationToken,
        deadline: Instant,
        stop_reason: Arc<AtomicU8>,
    ) -> Self {
        Self {
            cancellation,
            deadline,
            stop_reason,
            blocking_work: BlockingWorkTracker::new(),
        }
    }

    #[must_use]
    pub(crate) fn with_blocking_work_tracker(mut self, tracker: BlockingWorkTracker) -> Self {
        self.blocking_work = tracker;
        self
    }

    pub(crate) fn blocking_worker_guard(&self) -> Result<BlockingWorkGuard> {
        self.blocking_work.worker_guard()
    }

    #[must_use]
    pub fn cancellation(&self) -> &CancellationToken {
        &self.cancellation
    }

    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.deadline
    }

    #[must_use]
    pub fn stop_reason(&self) -> ModuleCallStopReason {
        match self.stop_reason.load(Ordering::Acquire) {
            2 => ModuleCallStopReason::DeadlineExceeded,
            3 => ModuleCallStopReason::Shutdown,
            _ => ModuleCallStopReason::CancelRequested,
        }
    }

    fn check_active(&self) -> std::result::Result<(), ModuleCallTerminated> {
        if self.cancellation.is_cancelled() {
            return Err(ModuleCallTerminated::new(
                self.stop_reason(),
                ModuleCallTerminationEvidence::NotStarted,
            ));
        }
        if Instant::now() >= self.deadline {
            return Err(ModuleCallTerminated::new(
                ModuleCallStopReason::DeadlineExceeded,
                ModuleCallTerminationEvidence::NotStarted,
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ModuleCallTerminated {
    reason: ModuleCallStopReason,
    evidence: ModuleCallTerminationEvidence,
}

impl ModuleCallTerminated {
    #[must_use]
    pub const fn new(
        reason: ModuleCallStopReason,
        evidence: ModuleCallTerminationEvidence,
    ) -> Self {
        Self { reason, evidence }
    }

    #[must_use]
    pub const fn reason(&self) -> ModuleCallStopReason {
        self.reason
    }

    #[must_use]
    pub const fn evidence(&self) -> ModuleCallTerminationEvidence {
        self.evidence
    }
}

impl std::fmt::Display for ModuleCallTerminated {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reason = match self.reason {
            ModuleCallStopReason::CancelRequested => "cancellation requested",
            ModuleCallStopReason::DeadlineExceeded => "deadline exceeded",
            ModuleCallStopReason::Shutdown => "shutdown requested",
        };
        let evidence = match self.evidence {
            ModuleCallTerminationEvidence::ProcessTerminated => "process terminated and reaped",
            ModuleCallTerminationEvidence::RemoteEffectTerminationConfirmed => {
                "remote effect termination confirmed"
            }
            ModuleCallTerminationEvidence::LocallyAbandoned => {
                "local work stopped; remote termination unknown"
            }
            ModuleCallTerminationEvidence::NotStarted => "external process was not started",
        };
        write!(formatter, "module call stopped after {reason}: {evidence}")
    }
}

impl std::error::Error for ModuleCallTerminated {}

#[derive(Debug)]
pub struct ModuleTransportClosed {
    message: String,
}

impl ModuleTransportClosed {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ModuleTransportClosed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ModuleTransportClosed {}

pub trait ModuleTransport: Send + Sync {
    fn kind(&self) -> ModuleTransportKind;

    fn logoscore_cli_transport(&self) -> Option<&LogoscoreCliTransport> {
        None
    }

    fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_>;

    fn subscribe_module_event(
        &self,
        _module: &str,
        _event: &str,
    ) -> ModuleTransportResult<BoxedModuleEventSubscription> {
        bail!("module event subscriptions are unavailable through this adapter")
    }

    fn ingest_module_event(
        &self,
        _module: &str,
        _event: &str,
        _args: &[Value],
    ) -> ModuleTransportResult<()> {
        Ok(())
    }

    fn supports_shared_file_staging(&self) -> bool {
        false
    }

    /// Reports whether the Basecamp host owns a healthy native runtime-event
    /// ingress path. Local Rust subscription registration alone is not upstream
    /// event-delivery evidence.
    fn native_runtime_module_events_ready(&self) -> bool {
        false
    }

    fn call_controlled(
        &self,
        call: ModuleCall,
        control: ModuleCallControl,
    ) -> ModuleCallFuture<'_> {
        Box::pin(async move {
            control.check_active()?;
            let call = self.call(call);
            tokio::select! {
                biased;
                result = call => result,
                () = control.cancellation.cancelled() => Err(ModuleCallTerminated::new(
                    control.stop_reason(),
                    ModuleCallTerminationEvidence::LocallyAbandoned,
                ).into()),
                () = tokio::time::sleep_until(control.deadline) => Err(ModuleCallTerminated::new(
                    ModuleCallStopReason::DeadlineExceeded,
                    ModuleCallTerminationEvidence::LocallyAbandoned,
                ).into()),
            }
        })
    }

    fn status(&self) -> ModuleDiagnosticFuture<'_> {
        let adapter = self.kind();
        Box::pin(async move {
            Ok(unsupported_diagnostic(
                adapter,
                "transport status is unavailable through this adapter",
            ))
        })
    }

    fn module_info(&self, module: String) -> ModuleDiagnosticFuture<'_> {
        let adapter = self.kind();
        Box::pin(async move {
            Ok(unsupported_diagnostic(
                adapter,
                format!("module metadata for `{module}` is unavailable through this adapter"),
            ))
        })
    }
}

fn unsupported_diagnostic(adapter: ModuleTransportKind, reason: impl Into<String>) -> Value {
    serde_json::json!({
        "supported": false,
        "adapter": adapter,
        "reason": reason.into(),
    })
}

#[derive(Debug, Clone)]
pub struct UnavailableModuleTransport {
    reason: String,
}

impl UnavailableModuleTransport {
    #[must_use]
    pub fn basecamp_host_not_configured() -> Self {
        Self {
            reason: "Basecamp host module transport is unavailable: no host transport was configured for this core handle".to_owned(),
        }
    }
}

impl ModuleTransport for UnavailableModuleTransport {
    fn kind(&self) -> ModuleTransportKind {
        ModuleTransportKind::Module
    }

    fn call(&self, _call: ModuleCall) -> ModuleCallFuture<'_> {
        Box::pin(async move { bail!(self.reason.clone()) })
    }
}

pub async fn dispatch_module_call(
    transport: &dyn ModuleTransport,
    call: ModuleCall,
) -> Result<ModuleCallReply> {
    let expected = call.transport();
    let actual = transport.kind();
    if expected != actual {
        bail!(
            "resolved module transport `{}` is unavailable; active transport is `{}`",
            expected.as_str(),
            actual.as_str()
        );
    }
    let reply = transport.call(call).await?;
    if reply.transport() != actual {
        bail!(
            "module transport `{}` returned reply identity `{}`",
            actual.as_str(),
            reply.transport().as_str()
        );
    }
    Ok(reply)
}

#[derive(Debug, Clone)]
pub struct LogoscoreCliTransport {
    runtime: LogoscoreCliRuntime,
}

impl Default for LogoscoreCliTransport {
    fn default() -> Self {
        Self {
            runtime: configured_runtime(),
        }
    }
}

impl LogoscoreCliTransport {
    #[cfg(test)]
    pub(crate) fn managed(binary_path: String, config_dir: String) -> Self {
        Self {
            runtime: LogoscoreCliRuntime::managed(binary_path, config_dir),
        }
    }

    pub(crate) fn runtime(&self) -> LogoscoreCliRuntime {
        self.runtime.clone()
    }
}

impl ModuleTransport for LogoscoreCliTransport {
    fn kind(&self) -> ModuleTransportKind {
        ModuleTransportKind::LogoscoreCli
    }

    fn logoscore_cli_transport(&self) -> Option<&LogoscoreCliTransport> {
        Some(self)
    }

    fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
        let runtime = self.runtime.clone();
        Box::pin(async move {
            let transport = call.transport();
            if transport != ModuleTransportKind::LogoscoreCli {
                bail!(
                    "LogosCore CLI transport cannot execute `{}` calls",
                    transport.as_str()
                );
            }
            let module = call.module().to_owned();
            let method = call.method().to_owned();
            let args = call
                .args()
                .iter()
                .cloned()
                .map(|value| match value {
                    Value::String(value) => value,
                    value => value.to_string(),
                })
                .collect::<Vec<_>>();
            let module_label = module.clone();
            let method_label = method.clone();
            let output = tokio::task::spawn_blocking(move || runtime.call(&module, &method, &args))
                .await
                .context("LogosCore CLI module-call worker failed")??;
            let value = normalize_module_call_value(&module_label, &method_label, output.value)?;
            Ok(ModuleCallReply::new(
                ModuleTransportKind::LogoscoreCli,
                value,
            ))
        })
    }

    fn call_controlled(
        &self,
        call: ModuleCall,
        control: ModuleCallControl,
    ) -> ModuleCallFuture<'_> {
        let runtime = self.runtime.clone();
        Box::pin(async move {
            let transport = call.transport();
            if transport != ModuleTransportKind::LogoscoreCli {
                bail!(
                    "LogosCore CLI transport cannot execute `{}` calls",
                    transport.as_str()
                );
            }
            let module = call.module().to_owned();
            let method = call.method().to_owned();
            let args = call
                .args()
                .iter()
                .cloned()
                .map(|value| match value {
                    Value::String(value) => value,
                    value => value.to_string(),
                })
                .collect::<Vec<_>>();
            let module_label = module.clone();
            let method_label = method.clone();
            let command_control = CommandControl::new(
                control.cancellation().clone(),
                control.deadline().into_std(),
            );
            control.check_active()?;
            let worker_guard = control.blocking_worker_guard()?;
            let output = tokio::task::spawn_blocking(move || {
                let _worker_guard = worker_guard;
                runtime.call_controlled(&module, &method, &args, command_control)
            })
            .await
            .context("LogosCore CLI module-call worker failed")?;
            let output = match output {
                Ok(output) => output,
                Err(error) => {
                    if let Some(terminated) = error.downcast_ref::<CommandTerminated>() {
                        let reason = match terminated.reason() {
                            CommandStopReason::CancelRequested => control.stop_reason(),
                            CommandStopReason::DeadlineExceeded => {
                                ModuleCallStopReason::DeadlineExceeded
                            }
                        };
                        let evidence = match terminated.scope() {
                            CommandTerminationScope::NoProcess => {
                                ModuleCallTerminationEvidence::NotStarted
                            }
                            CommandTerminationScope::DirectChild
                            | CommandTerminationScope::ProcessGroup => {
                                ModuleCallTerminationEvidence::LocallyAbandoned
                            }
                        };
                        return Err(ModuleCallTerminated::new(reason, evidence).into());
                    }
                    return Err(error);
                }
            };
            let value = normalize_module_call_value(&module_label, &method_label, output.value)?;
            Ok(ModuleCallReply::new(
                ModuleTransportKind::LogoscoreCli,
                value,
            ))
        })
    }

    fn status(&self) -> ModuleDiagnosticFuture<'_> {
        let runtime = self.runtime.clone();
        Box::pin(async move {
            let output = tokio::task::spawn_blocking(move || runtime.status())
                .await
                .context("LogosCore CLI status worker failed")??;
            serde_json::to_value(output).context("failed to serialize LogosCore CLI status")
        })
    }

    fn module_info(&self, module: String) -> ModuleDiagnosticFuture<'_> {
        let runtime = self.runtime.clone();
        Box::pin(async move {
            let module_label = module.clone();
            let output = tokio::task::spawn_blocking(move || runtime.module_info(&module))
                .await
                .with_context(|| {
                    format!("LogosCore CLI module-info worker failed for `{module_label}`")
                })??;
            serde_json::to_value(output)
                .with_context(|| format!("failed to serialize module info for `{module_label}`"))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogoscoreCliRuntime {
    runner: LogosCoreRunner,
}

pub(crate) struct LogoscoreSharedFile {
    _directory: TempDir,
    path: PathBuf,
}

impl LogoscoreSharedFile {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

pub(crate) struct LogoscoreSharedDownload {
    directory: TempDir,
    path: PathBuf,
}

impl LogoscoreSharedDownload {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn read_bounded(&self, max_bytes: usize) -> Result<Vec<u8>> {
        let metadata = fs::symlink_metadata(&self.path).with_context(|| {
            format!(
                "failed to inspect logoscore download staging file `{}`",
                self.path.display()
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            bail!("logoscore download staging path is not a regular file");
        }
        if metadata.len() > max_bytes as u64 {
            bail!("logoscore download exceeded {max_bytes} byte limit");
        }
        let capacity = usize::try_from(metadata.len())
            .context("logoscore download length does not fit in memory")?;
        let mut bytes = Vec::with_capacity(capacity);
        fs::File::open(&self.path)
            .with_context(|| {
                format!(
                    "failed to open logoscore download staging file `{}`",
                    self.path.display()
                )
            })?
            .take(max_bytes.saturating_add(1) as u64)
            .read_to_end(&mut bytes)
            .with_context(|| {
                format!(
                    "failed to read logoscore download staging file `{}`",
                    self.path.display()
                )
            })?;
        if bytes.len() > max_bytes {
            bail!("logoscore download exceeded {max_bytes} byte limit");
        }
        Ok(bytes)
    }

    pub(crate) fn close(self) -> Result<()> {
        let path = self.directory.path().to_path_buf();
        self.directory.close().with_context(|| {
            format!(
                "failed to remove logoscore download workspace `{}`",
                path.display()
            )
        })
    }
}

enum LogoscoreWatchOutput {
    Value(Value),
    Error(String),
    Eof,
}

enum LogoscoreWatchReadiness {
    Ready,
    Error(String),
    Eof,
}

#[derive(Debug)]
pub(crate) struct LogoscoreWatchCleanupUnconfirmed {
    message: String,
}

impl LogoscoreWatchCleanupUnconfirmed {
    fn new(message: String) -> Self {
        Self { message }
    }
}

impl std::fmt::Display for LogoscoreWatchCleanupUnconfirmed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for LogoscoreWatchCleanupUnconfirmed {}

pub(crate) struct LogoscoreEventWatch {
    child: Option<Child>,
    output: mpsc::Receiver<LogoscoreWatchOutput>,
    output_failure: Arc<Mutex<Option<String>>>,
    readiness: mpsc::Receiver<LogoscoreWatchReadiness>,
    reader: Option<thread::JoinHandle<()>>,
    stderr_reader: Option<thread::JoinHandle<()>>,
    reader_stop: Arc<AtomicBool>,
    process_permit: Option<StreamingCommandPermit>,
    recovery: Option<mpsc::Sender<LogoscoreWatchRecovery>>,
    label: String,
}

impl LogoscoreEventWatch {
    pub(crate) fn wait_ready(&mut self, control: &CommandControl) -> Result<()> {
        loop {
            control.check_active()?;
            if let Some(error) = take_watch_output_failure(&self.output_failure) {
                bail!("{error}");
            }
            let wait = LOGOSCORE_POLL_INTERVAL.min(
                control
                    .deadline()
                    .saturating_duration_since(StdInstant::now()),
            );
            match self.readiness.recv_timeout(wait) {
                Ok(LogoscoreWatchReadiness::Ready) => return Ok(()),
                Ok(LogoscoreWatchReadiness::Error(error)) => bail!("{error}"),
                Ok(LogoscoreWatchReadiness::Eof) => {
                    bail!("{} ended before its subscription became ready", self.label)
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    bail!(
                        "{} readiness channel closed before subscription",
                        self.label
                    )
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn next_value(&mut self, control: &CommandControl) -> Result<Value> {
        loop {
            if let Some(value) = self.next_value_within(control, LOGOSCORE_POLL_INTERVAL)? {
                return Ok(value);
            }
        }
    }

    pub(crate) fn next_value_within(
        &mut self,
        control: &CommandControl,
        timeout: Duration,
    ) -> Result<Option<Value>> {
        if let Some(error) = take_watch_output_failure(&self.output_failure) {
            bail!("{error}");
        }
        match self.output.try_recv() {
            Ok(LogoscoreWatchOutput::Value(value)) => return Ok(Some(value)),
            Ok(LogoscoreWatchOutput::Error(error)) => bail!("{error}"),
            Ok(LogoscoreWatchOutput::Eof) => {
                bail!("{} ended before a terminal event", self.label)
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                if let Some(error) = take_watch_output_failure(&self.output_failure) {
                    bail!("{error}");
                }
                bail!("{} output closed before a terminal event", self.label)
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        control.check_active()?;
        let wait = timeout.min(
            control
                .deadline()
                .saturating_duration_since(StdInstant::now()),
        );
        match self.output.recv_timeout(wait) {
            Ok(LogoscoreWatchOutput::Value(value)) => Ok(Some(value)),
            Ok(LogoscoreWatchOutput::Error(error)) => bail!("{error}"),
            Ok(LogoscoreWatchOutput::Eof) => {
                bail!("{} ended before a terminal event", self.label)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Some(error) = take_watch_output_failure(&self.output_failure) {
                    bail!("{error}");
                }
                bail!("{} output closed before a terminal event", self.label)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                control.check_active()?;
                Ok(None)
            }
        }
    }

    pub(crate) fn stop(&mut self) -> Result<()> {
        self.reader_stop.store(true, Ordering::Release);
        let child_result = match self.child.as_mut() {
            Some(child) => stop_watch_child_with_retry(child, &self.label),
            None => Ok(()),
        };
        child_result?;
        self.child = None;
        let reader_result = match self.reader.take() {
            Some(reader) => reader
                .join()
                .map_err(|_| anyhow::anyhow!("{} output reader panicked", self.label)),
            None => Ok(()),
        };
        let stderr_result = match self.stderr_reader.take() {
            Some(reader) => reader
                .join()
                .map_err(|_| anyhow::anyhow!("{} stderr reader panicked", self.label)),
            None => Ok(()),
        };
        self.process_permit = None;
        reader_result.and(stderr_result)
    }
}

impl Drop for LogoscoreEventWatch {
    fn drop(&mut self) {
        if self.stop().is_err() {
            self.handoff_failed_cleanup();
        }
    }
}

impl LogoscoreEventWatch {
    fn handoff_failed_cleanup(&mut self) {
        let Some(child) = self.child.take() else {
            return;
        };
        let recovery = LogoscoreWatchRecovery {
            child,
            reader: self.reader.take(),
            stderr_reader: self.stderr_reader.take(),
            reader_stop: Arc::clone(&self.reader_stop),
            process_permit: self.process_permit.take(),
            label: self.label.clone(),
        };
        handoff_watch_recovery(self.recovery.take(), recovery);
    }
}

struct LogoscoreWatchRecovery {
    child: Child,
    reader: Option<thread::JoinHandle<()>>,
    stderr_reader: Option<thread::JoinHandle<()>>,
    reader_stop: Arc<AtomicBool>,
    process_permit: Option<StreamingCommandPermit>,
    label: String,
}

fn start_watch_recovery_worker() -> std::result::Result<mpsc::Sender<LogoscoreWatchRecovery>, String>
{
    let (sender, receiver) = mpsc::channel();
    thread::Builder::new()
        .name("logoscore-watch-recovery".to_owned())
        .spawn(move || run_watch_recovery_queue(&receiver))
        .map_err(|error| format!("failed to start logoscore watch recovery worker: {error}"))?;
    Ok(sender)
}

fn watch_recovery_sender() -> Result<mpsc::Sender<LogoscoreWatchRecovery>> {
    match &*LOGOSCORE_WATCH_RECOVERY {
        Ok(sender) => Ok(sender.clone()),
        Err(error) => bail!(error.clone()),
    }
}

fn run_watch_recovery_queue(receiver: &mpsc::Receiver<LogoscoreWatchRecovery>) {
    run_watch_recovery_queue_with(receiver, LOGOSCORE_WATCH_STOP_GRACE, |recovery| {
        stop_watch_child_with_retry(&mut recovery.child, &recovery.label).is_ok()
    });
}

fn run_watch_recovery_queue_with<F>(
    receiver: &mpsc::Receiver<LogoscoreWatchRecovery>,
    retry_interval: Duration,
    mut cleanup: F,
) where
    F: FnMut(&mut LogoscoreWatchRecovery) -> bool,
{
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
            recovery.reader_stop.store(true, Ordering::Release);
            if cleanup(&mut recovery) {
                finish_watch_recovery(recovery);
            } else {
                pending.push_back(recovery);
            }
        }
        if !pending.is_empty() {
            thread::sleep(retry_interval);
        }
    }
}

fn run_watch_recovery(mut recovery: LogoscoreWatchRecovery) {
    recovery.reader_stop.store(true, Ordering::Release);
    while stop_watch_child_with_retry(&mut recovery.child, &recovery.label).is_err() {
        thread::sleep(LOGOSCORE_WATCH_STOP_GRACE);
    }
    finish_watch_recovery(recovery);
}

fn finish_watch_recovery(mut recovery: LogoscoreWatchRecovery) {
    if let Some(reader) = recovery.reader.take() {
        let _join_result = reader.join();
    }
    if let Some(reader) = recovery.stderr_reader.take() {
        let _join_result = reader.join();
    }
    recovery.process_permit = None;
}

impl LogoscoreCliRuntime {
    #[must_use]
    pub(crate) fn managed(binary_path: String, config_dir: String) -> Self {
        Self {
            runner: LogosCoreRunner {
                program: binary_path,
                sudo_user: None,
                home: None,
                config_dir: Some(config_dir),
                label: "Inspector-managed logoscore".to_owned(),
            },
        }
    }

    pub(crate) fn status(&self) -> Result<LogosCoreOutput> {
        self.run_json(["status", "--json"], command_timeout())
    }

    pub(crate) fn status_with_timeout(&self, timeout: Duration) -> Result<LogosCoreOutput> {
        self.run_json(["status", "--json"], timeout)
    }

    pub(crate) fn status_controlled(&self, control: CommandControl) -> Result<LogosCoreOutput> {
        self.run_json_controlled(["status", "--json"], control)
    }

    pub(crate) fn list_modules(&self) -> Result<LogosCoreOutput> {
        self.run_json(["list-modules", "--json"], command_timeout())
    }

    pub(crate) fn list_modules_controlled(
        &self,
        control: CommandControl,
    ) -> Result<LogosCoreOutput> {
        self.run_json_controlled(["list-modules", "--json"], control)
    }

    pub(crate) fn module_info(&self, module: &str) -> Result<LogosCoreOutput> {
        if module.trim().is_empty() {
            bail!("module name is required");
        }
        self.run_json(["module-info", module, "--json"], command_timeout())
    }

    pub(crate) fn module_info_controlled(
        &self,
        module: &str,
        control: CommandControl,
    ) -> Result<LogosCoreOutput> {
        if module.trim().is_empty() {
            bail!("module name is required");
        }
        self.run_json_controlled(["module-info", module, "--json"], control)
    }

    pub(crate) fn require_module_method(
        &self,
        module: &str,
        method: &str,
        signature: &str,
    ) -> Result<()> {
        let modules = self
            .list_modules()
            .context("failed to list logoscore modules")?;
        let module_info = self
            .module_info(module)
            .with_context(|| format!("failed to inspect logoscore module `{module}`"))?;
        module_discovery(module, &modules.value, &module_info.value)?
            .require_method(method, signature)
    }

    pub(crate) fn require_module_method_controlled(
        &self,
        module: &str,
        method: &str,
        signature: &str,
        control: CommandControl,
    ) -> Result<()> {
        let modules = self
            .list_modules_controlled(control.clone())
            .context("failed to list logoscore modules")?;
        let module_info = self
            .module_info_controlled(module, control)
            .with_context(|| format!("failed to inspect logoscore module `{module}`"))?;
        module_discovery(module, &modules.value, &module_info.value)?
            .require_method(method, signature)
    }

    pub(crate) fn require_module_contract_controlled(
        &self,
        module: &str,
        methods: &[(&str, &str)],
        events: &[(&str, &str)],
        control: CommandControl,
    ) -> Result<()> {
        let modules = self
            .list_modules_controlled(control.clone())
            .context("failed to list logoscore modules")?;
        let module_info = self
            .module_info_controlled(module, control)
            .with_context(|| format!("failed to inspect logoscore module `{module}`"))?;
        let discovery = module_discovery(module, &modules.value, &module_info.value)?;
        for (method, signature) in methods {
            discovery.require_method(method, signature)?;
        }
        for (event, signature) in events {
            discovery.require_event(event, signature)?;
        }
        Ok(())
    }

    pub(crate) fn ensure_module_loaded(&self, module: &str) -> Result<()> {
        let modules = self
            .list_modules()
            .context("failed to list logoscore modules")?;
        let rows = module_rows(&modules.value)?;
        let Some(row) = rows
            .iter()
            .find(|candidate| candidate.get("name").and_then(Value::as_str) == Some(module))
        else {
            bail!("logoscore module `{module}` is not listed");
        };
        if row.get("status").and_then(Value::as_str) == Some("loaded") {
            return Ok(());
        }

        self.run_json(["load-module", module, "--json"], command_timeout())
            .with_context(|| format!("failed to load logoscore module `{module}`"))?;
        Ok(())
    }

    pub(crate) fn ensure_module_loaded_controlled(
        &self,
        module: &str,
        control: CommandControl,
    ) -> Result<()> {
        let modules = self
            .list_modules_controlled(control.clone())
            .context("failed to list logoscore modules")?;
        let rows = module_rows(&modules.value)?;
        let Some(row) = rows
            .iter()
            .find(|candidate| candidate.get("name").and_then(Value::as_str) == Some(module))
        else {
            bail!("logoscore module `{module}` is not listed");
        };
        if row.get("status").and_then(Value::as_str) == Some("loaded") {
            return Ok(());
        }

        self.run_json_controlled(["load-module", module, "--json"], control)
            .with_context(|| format!("failed to load logoscore module `{module}`"))?;
        Ok(())
    }

    pub(crate) fn call(
        &self,
        module: &str,
        method: &str,
        args: &[String],
    ) -> Result<LogosCoreOutput> {
        let command_args = call_arguments(module, method, args)?;
        let mut output = self.run_json(command_args, command_timeout())?;
        normalize_call_value(&mut output.value);
        Ok(output)
    }

    pub(crate) fn call_controlled(
        &self,
        module: &str,
        method: &str,
        args: &[String],
        control: CommandControl,
    ) -> Result<LogosCoreOutput> {
        let command_args = call_arguments(module, method, args)?;
        let mut output = self.run_json_controlled(command_args, control)?;
        normalize_call_value(&mut output.value);
        Ok(output)
    }

    pub(crate) fn call_checked(
        &self,
        module: &str,
        method: &str,
        signature: &str,
        args: &[String],
    ) -> Result<Value> {
        self.require_module_method(module, method, signature)?;
        serde_json::to_value(self.call(module, method, args)?)
            .context("failed to serialize logoscore call output")
    }

    pub(crate) fn call_checked_controlled(
        &self,
        module: &str,
        method: &str,
        signature: &str,
        args: &[String],
        control: CommandControl,
    ) -> Result<Value> {
        self.require_module_method_controlled(module, method, signature, control.clone())?;
        serde_json::to_value(self.call_controlled(module, method, args, control)?)
            .context("failed to serialize logoscore call output")
    }

    #[must_use]
    pub(crate) fn daemon_command(&self, persistence_path: &str, modules_dir: &str) -> Command {
        command_for_runner(
            &self.runner,
            [
                "--persistence-path",
                persistence_path,
                "daemon",
                "--modules-dir",
                modules_dir,
            ],
        )
    }

    #[must_use]
    pub(crate) fn watch_command(&self, module: &str, event: &str) -> Command {
        command_for_runner(
            &self.runner,
            [
                "watch",
                module,
                "--event",
                event,
                "--json",
                "--watch-protocol",
                "v1",
            ],
        )
    }

    pub(crate) fn start_event_watch(
        &self,
        module: &str,
        event: &str,
        control: &CommandControl,
    ) -> Result<LogoscoreEventWatch> {
        ensure_logoscore_event_watch_supported()?;
        if module.trim().is_empty() {
            bail!("module name is required");
        }
        if event.trim().is_empty() {
            bail!("module event name is required");
        }
        control.check_active()?;
        let recovery = watch_recovery_sender()?;
        let label = format!("logoscore watch {module}.{event}");
        let process_permit = acquire_streaming_command_permit(&label, control)?;
        let mut command = self.watch_command(module, event);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;

            command.process_group(0);
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("failed to start {label}"))?;
        let Some(stdout) = child.stdout.take() else {
            let error = anyhow::anyhow!("{label} did not expose stdout");
            return Err(cleanup_failed_watch_start(
                error,
                FailedWatchStart::new(child, None, None, process_permit, recovery, &label),
            ));
        };
        let Some(stderr) = child.stderr.take() else {
            let error = anyhow::anyhow!("{label} did not expose stderr");
            return Err(cleanup_failed_watch_start(
                error,
                FailedWatchStart::new(child, None, None, process_permit, recovery, &label),
            ));
        };
        #[cfg(unix)]
        if let Err(error) = configure_watch_pipe_nonblocking(&stdout)
            .and_then(|()| configure_watch_pipe_nonblocking(&stderr))
        {
            let error = anyhow::Error::new(error)
                .context(format!("failed to configure {label} output capture"));
            return Err(cleanup_failed_watch_start(
                error,
                FailedWatchStart::new(child, None, None, process_permit, recovery, &label),
            ));
        }
        let (sender, output) = mpsc::sync_channel(LOGOSCORE_EVENT_QUEUE_CAPACITY);
        let output_failure = Arc::new(Mutex::new(None));
        let (readiness_sender, readiness) = mpsc::channel();
        let reader_stop = Arc::new(AtomicBool::new(false));
        let reader_label = label.clone();
        let expected_module = module.to_owned();
        let expected_event = event.to_owned();
        let reader_failure = Arc::clone(&output_failure);
        let stdout_stop = Arc::clone(&reader_stop);
        let reader = match thread::Builder::new()
            .name("logoscore-event-watch-reader".to_owned())
            .spawn(move || {
                read_json_watch_output(
                    stdout,
                    &reader_label,
                    (&expected_module, &expected_event),
                    &readiness_sender,
                    &sender,
                    &reader_failure,
                    &stdout_stop,
                );
            }) {
            Ok(reader) => reader,
            Err(error) => {
                let error = anyhow::Error::new(error)
                    .context(format!("failed to start {label} output reader"));
                return Err(cleanup_failed_watch_start(
                    error,
                    FailedWatchStart::new(
                        child,
                        None,
                        Some(reader_stop),
                        process_permit,
                        recovery,
                        &label,
                    ),
                ));
            }
        };
        let stderr_label = label.clone();
        let stderr_failure = Arc::clone(&output_failure);
        let stderr_stop = Arc::clone(&reader_stop);
        let stderr_reader = match thread::Builder::new()
            .name("logoscore-event-watch-stderr".to_owned())
            .spawn(move || {
                read_watch_stderr(stderr, &stderr_label, &stderr_failure, &stderr_stop);
            }) {
            Ok(stderr_reader) => stderr_reader,
            Err(error) => {
                let error = anyhow::Error::new(error)
                    .context(format!("failed to start {label} stderr reader"));
                return Err(cleanup_failed_watch_start(
                    error,
                    FailedWatchStart::new(
                        child,
                        Some(reader),
                        Some(reader_stop),
                        process_permit,
                        recovery,
                        &label,
                    ),
                ));
            }
        };
        Ok(LogoscoreEventWatch {
            child: Some(child),
            output,
            output_failure,
            readiness,
            reader: Some(reader),
            stderr_reader: Some(stderr_reader),
            reader_stop,
            process_permit: Some(process_permit),
            recovery: Some(recovery),
            label,
        })
    }

    pub(crate) fn stop(&self) -> Result<LogosCoreOutput> {
        self.run_json(["stop", "--json"], command_timeout())
    }

    pub(crate) fn stop_controlled(&self, control: CommandControl) -> Result<LogosCoreOutput> {
        self.run_json_controlled(["stop", "--json"], control)
    }

    pub(crate) fn stage_shared_file(
        &self,
        filename: &str,
        bytes: &[u8],
    ) -> Result<LogoscoreSharedFile> {
        let shared_transport = SharedFilesystemTransport::from_runner(&self.runner, "uploadUrl")?;
        let directory = tempfile::Builder::new()
            .prefix("logos-inspector-upload-")
            .tempdir()
            .context("failed to create logoscore upload workspace")?;
        shared_transport.share_directory(directory.path())?;
        let path = directory.path().join(filename);
        fs::write(&path, bytes).context("failed to write logoscore upload payload")?;
        shared_transport.share_file(&path, 0o640)?;
        Ok(LogoscoreSharedFile {
            _directory: directory,
            path,
        })
    }

    pub(crate) fn stage_shared_download(&self, filename: &str) -> Result<LogoscoreSharedDownload> {
        let safe_filename = Path::new(filename)
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty() && *value == filename)
            .context("logoscore download filename is invalid")?;
        let shared_transport =
            SharedFilesystemTransport::from_runner(&self.runner, "downloadToUrl")?;
        let directory = tempfile::Builder::new()
            .prefix("logos-inspector-download-")
            .tempdir()
            .context("failed to create logoscore download workspace")?;
        shared_transport.share_directory(directory.path())?;
        let path = directory.path().join(safe_filename);
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .context("failed to create logoscore download staging file")?;
        shared_transport.share_file(&path, 0o660)?;
        Ok(LogoscoreSharedDownload { directory, path })
    }

    pub(crate) fn storage_backup_download_readiness(&self) -> Result<Value> {
        ensure_logoscore_event_watch_supported()?;
        let deadline = StdInstant::now()
            .checked_add(command_timeout())
            .context("storage backup readiness deadline overflow")?;
        let control = CommandControl::new(CancellationToken::new(), deadline);
        self.require_module_contract_controlled(
            "storage_module",
            &[
                ("downloadProtocol", "downloadProtocol()"),
                (
                    "downloadToUrlV2",
                    "downloadToUrlV2(QString,QString,bool,int,QString,int)",
                ),
                ("downloadCancelV2", "downloadCancelV2(QString)"),
            ],
            &[("storageDownloadDoneV2", "storageDownloadDoneV2(QString)")],
            control.clone(),
        )?;
        let protocol =
            self.call_controlled("storage_module", "downloadProtocol", &[], control.clone())?;
        let protocol =
            normalize_module_call_value("storage_module", "downloadProtocol", protocol.value)?;
        anyhow::ensure!(
            protocol.get("protocol").and_then(Value::as_str) == Some("logos.storage.download")
                && protocol.get("version").and_then(Value::as_u64) == Some(2)
                && protocol
                    .get("moduleOperationIdOwner")
                    .and_then(Value::as_str)
                    == Some("caller")
                && protocol.get("cancelTimeoutMs").and_then(Value::as_u64) == Some(15_000)
                && protocol
                    .get("maxDownloadBytes")
                    .and_then(Value::as_u64)
                    .is_some_and(|max_bytes| max_bytes >= SETTINGS_BACKUP_MAX_BYTES as u64),
            "storage_module returned an incompatible download protocol"
        );
        let staged = self.stage_shared_download("backup-readiness.json")?;
        let watch_result = self
            .start_event_watch("storage_module", "storageDownloadDoneV2", &control)
            .and_then(|mut watch| {
                let ready = watch.wait_ready(&control);
                let cleanup = watch.stop();
                match (ready, cleanup) {
                    (Ok(()), Ok(())) => Ok(()),
                    (Err(error), Ok(())) => Err(error),
                    (Ok(()), Err(cleanup)) => Err(cleanup),
                    (Err(error), Err(cleanup)) => Err(anyhow::anyhow!(
                        "{error}; readiness watch cleanup failed: {cleanup:#}"
                    )),
                }
            });
        let staging_cleanup = staged.close();
        match (watch_result, staging_cleanup) {
            (Ok(()), Ok(())) => Ok(json!({
                "contract": protocol,
                "shared_staging": true,
                "watch_protocol": {
                    "protocol": LOGOSCORE_WATCH_PROTOCOL,
                    "version": LOGOSCORE_WATCH_PROTOCOL_VERSION,
                    "ready": true,
                },
            })),
            (Err(error), Ok(())) => Err(error),
            (Ok(()), Err(cleanup)) => Err(cleanup),
            (Err(error), Err(cleanup)) => Err(anyhow::anyhow!(
                "{error}; readiness staging cleanup failed: {cleanup:#}"
            )),
        }
    }

    fn run_json<I, S>(&self, args: I, timeout: Duration) -> Result<LogosCoreOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        run_json_with(&self.runner, args, timeout)
    }

    fn run_json_controlled<I, S>(&self, args: I, control: CommandControl) -> Result<LogosCoreOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        run_json_with_controlled(&self.runner, args, control)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LogosCoreRunner {
    program: String,
    sudo_user: Option<String>,
    home: Option<String>,
    config_dir: Option<String>,
    label: String,
}

pub fn status() -> Result<LogosCoreOutput> {
    configured_runtime().status()
}

pub fn module_info(module: &str) -> Result<LogosCoreOutput> {
    if module.trim().is_empty() {
        bail!("module name is required");
    }
    configured_runtime().module_info(module)
}

struct SharedFilesystemTransport {
    #[cfg(unix)]
    group: u32,
}

impl SharedFilesystemTransport {
    fn from_runner(runner: &LogosCoreRunner, method: &str) -> Result<Self> {
        let config_dir = runner_config_dir(runner)?;
        let config_path = config_dir.join("client").join("config.json");
        let config_bytes = fs::read(&config_path).with_context(|| {
            format!(
                "failed to read logoscore client config `{}`",
                config_path.display()
            )
        })?;
        let config: Value = serde_json::from_slice(&config_bytes)
            .context("logoscore client config contains invalid JSON")?;
        let instance_id = local_transport_instance_id(&config, method)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;

            let socket = env::temp_dir().join(format!("logos_core_service_{instance_id}"));
            let group = fs::metadata(&socket)
                .with_context(|| {
                    format!(
                        "logoscore local transport socket is unavailable at `{}`",
                        socket.display()
                    )
                })?
                .gid();
            Ok(Self { group })
        }
        #[cfg(not(unix))]
        {
            let _validated_instance_id = instance_id;
            Ok(Self {})
        }
    }

    fn share_directory(&self, path: &Path) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::{PermissionsExt as _, chown};

            chown(path, None, Some(self.group))
                .context("failed to assign logoscore shared directory group")?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o750))
                .context("failed to secure logoscore shared directory")?;
        }
        #[cfg(not(unix))]
        let _path = path;
        Ok(())
    }

    fn share_file(&self, path: &Path, mode: u32) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::{PermissionsExt as _, chown};

            chown(path, None, Some(self.group))
                .context("failed to assign logoscore shared file group")?;
            fs::set_permissions(path, fs::Permissions::from_mode(mode))
                .context("failed to secure logoscore shared file")?;
        }
        #[cfg(not(unix))]
        let (_path, _mode) = (path, mode);
        Ok(())
    }
}

fn runner_config_dir(runner: &LogosCoreRunner) -> Result<PathBuf> {
    if let Some(config_dir) = runner.config_dir.as_deref() {
        return Ok(PathBuf::from(config_dir));
    }
    let home = runner
        .home
        .clone()
        .or_else(|| env::var("HOME").ok())
        .filter(|value| !value.trim().is_empty())
        .context("HOME is required to locate logoscore client config")?;
    Ok(PathBuf::from(home).join(".logoscore"))
}

fn local_transport_instance_id<'a>(config: &'a Value, method: &str) -> Result<&'a str> {
    let transport = config
        .pointer("/daemon/core_service/transport")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if transport != "local" {
        bail!(
            "storage_module {method} requires local logoscore transport with a shared filesystem"
        );
    }
    config
        .get("instance_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("logoscore client config has no instance_id")
}

fn module_discovery(
    module: &str,
    modules_value: &Value,
    module_info_value: &Value,
) -> Result<LogoscoreModuleDiscovery> {
    if module.trim().is_empty() {
        bail!("module name is required");
    }
    let modules = module_rows(modules_value)?;
    let Some(module_row) = modules
        .iter()
        .find(|candidate| candidate.get("name").and_then(Value::as_str) == Some(module))
    else {
        bail!("logoscore module `{module}` is not listed");
    };
    let status = module_row
        .get("status")
        .and_then(Value::as_str)
        .context("logoscore module listing has no status")?;
    if status != "loaded" {
        bail!("logoscore module `{module}` is not loaded (status `{status}`)");
    }
    if module_info_value.get("name").and_then(Value::as_str) != Some(module) {
        bail!("logoscore module-info did not identify module `{module}`");
    }
    let methods = module_info_value
        .get("methods")
        .and_then(Value::as_array)
        .context("logoscore module-info response does not contain a methods array")?
        .iter()
        .filter(|method| method.get("isInvokable").and_then(Value::as_bool) == Some(true))
        .filter_map(|method| {
            Some(LogoscoreModuleMethod {
                name: method.get("name")?.as_str()?.to_owned(),
                signature: method.get("signature")?.as_str()?.to_owned(),
            })
        })
        .collect();
    let events = module_info_value
        .get("events")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|event| {
            Some(LogoscoreModuleMethod {
                name: event.get("name")?.as_str()?.to_owned(),
                signature: event.get("signature")?.as_str()?.to_owned(),
            })
        })
        .collect();
    Ok(LogoscoreModuleDiscovery {
        module: module.to_owned(),
        methods,
        events,
    })
}

fn module_rows(modules_value: &Value) -> Result<&Vec<Value>> {
    modules_value
        .as_array()
        .or_else(|| modules_value.get("modules").and_then(Value::as_array))
        .context("logoscore list-modules response does not contain a modules array")
}

pub fn call(module: &str, method: &str, args: &[String]) -> Result<LogosCoreOutput> {
    configured_runtime().call(module, method, args)
}

fn call_arguments(module: &str, method: &str, args: &[String]) -> Result<Vec<String>> {
    if module.trim().is_empty() {
        bail!("module name is required");
    }
    if method.trim().is_empty() {
        bail!("method name is required");
    }

    let mut command_args = Vec::with_capacity(args.len() + 4);
    command_args.push("call".to_owned());
    command_args.push(module.to_owned());
    command_args.push(method.to_owned());
    command_args.extend(args.iter().cloned());
    command_args.push("--json".to_owned());
    Ok(command_args)
}

fn run_json_with<I, S>(
    runner: &LogosCoreRunner,
    args: I,
    timeout: Duration,
) -> Result<LogosCoreOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let command = command_for_runner(runner, args);
    let output = run_command(
        command,
        CommandRunPolicy {
            label: &runner.label,
            timeout,
            poll_interval: LOGOSCORE_POLL_INTERVAL,
            redactions: &[],
            output_limit: LOGOSCORE_OUTPUT_LIMIT,
        },
    )?;
    let stderr = output_text(&output.stderr, &[], LOGOSCORE_OUTPUT_LIMIT);
    let value = parse_json_stdout(&runner.label, &output.stdout)?;
    let stderr = (!stderr.is_empty()).then_some(stderr);
    Ok(LogosCoreOutput {
        runner: runner.label.clone(),
        value,
        stderr,
    })
}

fn run_json_with_controlled<I, S>(
    runner: &LogosCoreRunner,
    args: I,
    control: CommandControl,
) -> Result<LogosCoreOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let command = command_for_runner(runner, args);
    let output = run_command_controlled(
        command,
        CommandRunPolicy {
            label: &runner.label,
            // Controlled commands have one authority: CommandControl's absolute deadline.
            timeout: Duration::ZERO,
            poll_interval: LOGOSCORE_POLL_INTERVAL,
            redactions: &[],
            output_limit: LOGOSCORE_OUTPUT_LIMIT,
        },
        control,
    )?;
    logos_core_output(runner, output)
}

fn logos_core_output(
    runner: &LogosCoreRunner,
    output: std::process::Output,
) -> Result<LogosCoreOutput> {
    let stderr = output_text(&output.stderr, &[], LOGOSCORE_OUTPUT_LIMIT);
    let value = parse_json_stdout(&runner.label, &output.stdout)?;
    let stderr = (!stderr.is_empty()).then_some(stderr);
    Ok(LogosCoreOutput {
        runner: runner.label.clone(),
        value,
        stderr,
    })
}

fn parse_json_stdout(label: &str, stdout: &[u8]) -> Result<Value> {
    if stdout.len() > LOGOSCORE_JSON_OUTPUT_LIMIT {
        bail!(
            "{label} JSON output exceeded {} bytes",
            LOGOSCORE_JSON_OUTPUT_LIMIT
        );
    }
    let text = std::str::from_utf8(stdout).with_context(|| {
        format!(
            "{label} returned non-UTF-8 output: {}",
            output_text(stdout, &[], 400)
        )
    })?;
    serde_json::from_str(text.trim()).with_context(|| {
        format!(
            "{label} returned non-json output: {}",
            text.chars().take(400).collect::<String>()
        )
    })
}

fn command_timeout() -> Duration {
    env::var("LOGOSCORE_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_secs(5))
}

fn command_for_runner<I, S>(runner: &LogosCoreRunner, args: I) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    if let Some(user) = &runner.sudo_user {
        let mut command = Command::new("sudo");
        command.arg("-n").arg("-u").arg(user).arg("env");
        if let Some(home) = &runner.home {
            command.arg(format!("HOME={home}"));
        }
        command.arg(&runner.program);
        if let Some(config_dir) = &runner.config_dir {
            command.arg("--config-dir").arg(config_dir);
        }
        for arg in args {
            command.arg(arg.as_ref());
        }
        command
    } else {
        let mut command = Command::new(&runner.program);
        if let Some(home) = &runner.home {
            command.env("HOME", home);
        }
        if let Some(config_dir) = &runner.config_dir {
            command.arg("--config-dir").arg(config_dir);
        }
        for arg in args {
            command.arg(arg.as_ref());
        }
        command
    }
}

fn read_json_watch_output(
    stdout: std::process::ChildStdout,
    label: &str,
    expected: (&str, &str),
    readiness: &mpsc::Sender<LogoscoreWatchReadiness>,
    sender: &mpsc::SyncSender<LogoscoreWatchOutput>,
    failure: &Arc<Mutex<Option<String>>>,
    stop: &AtomicBool,
) {
    let mut reader = WatchLineReader::new(stdout);
    let mut ready = false;
    loop {
        let line = match reader.next_line(label, stop) {
            Ok(Some(line)) if line.trim().is_empty() => continue,
            Ok(Some(line)) => line,
            Ok(None) => {
                if ready {
                    send_watch_output(sender, failure, label, LogoscoreWatchOutput::Eof);
                } else {
                    let _result = readiness.send(LogoscoreWatchReadiness::Eof);
                }
                return;
            }
            Err(error) => {
                send_watch_protocol_error(ready, readiness, sender, failure, label, error);
                return;
            }
        };
        let value = match serde_json::from_str::<Value>(line.trim()) {
            Ok(value) => value,
            Err(error) => {
                send_watch_protocol_error(
                    ready,
                    readiness,
                    sender,
                    failure,
                    label,
                    format!("{label} returned malformed JSON watch frame: {error}"),
                );
                return;
            }
        };
        if !ready {
            if let Err(error) = validate_watch_ready_frame(&value, expected.0, expected.1) {
                let _result = readiness.send(LogoscoreWatchReadiness::Error(format!(
                    "{label} returned invalid subscription-ready frame: {error:#}"
                )));
                return;
            }
            if readiness.send(LogoscoreWatchReadiness::Ready).is_err() {
                return;
            }
            ready = true;
            continue;
        }
        if let Err(error) = validate_watch_event_frame(&value, expected.0, expected.1) {
            send_watch_output(
                sender,
                failure,
                label,
                LogoscoreWatchOutput::Error(format!(
                    "{label} returned invalid event frame: {error:#}"
                )),
            );
            return;
        }
        if !send_watch_output(sender, failure, label, LogoscoreWatchOutput::Value(value)) {
            return;
        }
    }
}

fn validate_watch_ready_frame(value: &Value, module: &str, event: &str) -> Result<()> {
    let object = value
        .as_object()
        .context("subscription-ready frame must be an object")?;
    anyhow::ensure!(
        object.len() == 5
            && value.get("type").and_then(Value::as_str) == Some("subscription_ready")
            && value.get("protocol").and_then(Value::as_str) == Some(LOGOSCORE_WATCH_PROTOCOL)
            && value.get("version").and_then(Value::as_u64)
                == Some(LOGOSCORE_WATCH_PROTOCOL_VERSION)
            && value.get("module").and_then(Value::as_str) == Some(module)
            && value.get("event").and_then(Value::as_str) == Some(event),
        "expected exact {LOGOSCORE_WATCH_PROTOCOL} v{LOGOSCORE_WATCH_PROTOCOL_VERSION} readiness for {module}.{event}"
    );
    Ok(())
}

fn validate_watch_event_frame(value: &Value, module: &str, event: &str) -> Result<()> {
    let object = value.as_object().context("event frame must be an object")?;
    anyhow::ensure!(
        object.len() == 7
            && value.get("type").and_then(Value::as_str) == Some("event")
            && value.get("protocol").and_then(Value::as_str) == Some(LOGOSCORE_WATCH_PROTOCOL)
            && value.get("version").and_then(Value::as_u64)
                == Some(LOGOSCORE_WATCH_PROTOCOL_VERSION)
            && value.get("timestamp").and_then(Value::as_str).is_some(),
        "event frame must exactly declare typed {LOGOSCORE_WATCH_PROTOCOL} v{LOGOSCORE_WATCH_PROTOCOL_VERSION} fields"
    );
    anyhow::ensure!(
        value.get("module").and_then(Value::as_str) == Some(module),
        "event module does not match `{module}`"
    );
    anyhow::ensure!(
        value.get("event").and_then(Value::as_str) == Some(event),
        "event name does not match `{event}`"
    );
    let data = value
        .get("data")
        .and_then(Value::as_object)
        .context("event data must be an object")?;
    anyhow::ensure!(
        data.len() <= LOGOSCORE_EVENT_FIELD_LIMIT,
        "event exceeded {LOGOSCORE_EVENT_FIELD_LIMIT} field limit"
    );
    Ok(())
}

fn send_watch_protocol_error(
    ready: bool,
    readiness: &mpsc::Sender<LogoscoreWatchReadiness>,
    sender: &mpsc::SyncSender<LogoscoreWatchOutput>,
    failure: &Arc<Mutex<Option<String>>>,
    label: &str,
    error: String,
) {
    if ready {
        send_watch_output(sender, failure, label, LogoscoreWatchOutput::Error(error));
    } else {
        let _result = readiness.send(LogoscoreWatchReadiness::Error(error));
    }
}

fn send_watch_output(
    sender: &mpsc::SyncSender<LogoscoreWatchOutput>,
    failure: &Arc<Mutex<Option<String>>>,
    label: &str,
    output: LogoscoreWatchOutput,
) -> bool {
    match sender.try_send(output) {
        Ok(()) => true,
        Err(mpsc::TrySendError::Full(_)) => {
            record_watch_output_failure(
                failure,
                format!(
                    "{label} exceeded bounded event queue capacity {LOGOSCORE_EVENT_QUEUE_CAPACITY}"
                ),
            );
            false
        }
        Err(mpsc::TrySendError::Disconnected(_)) => false,
    }
}

fn record_watch_output_failure(failure: &Arc<Mutex<Option<String>>>, error: String) {
    if let Ok(mut failure) = failure.lock()
        && failure.is_none()
    {
        *failure = Some(error);
    }
}

fn take_watch_output_failure(failure: &Arc<Mutex<Option<String>>>) -> Option<String> {
    failure.lock().ok().and_then(|mut failure| failure.take())
}

fn read_watch_stderr(
    stderr: std::process::ChildStderr,
    label: &str,
    failure: &Arc<Mutex<Option<String>>>,
    stop: &AtomicBool,
) {
    let mut reader = WatchLineReader::new(stderr);
    loop {
        match reader.next_line(label, stop) {
            Ok(Some(line)) if line.trim().is_empty() => {}
            Ok(Some(line)) => {
                record_watch_output_failure(
                    failure,
                    format!("{label} wrote to stderr: {}", line.trim()),
                );
                return;
            }
            Ok(None) => return,
            Err(error) => {
                record_watch_output_failure(failure, error);
                return;
            }
        }
    }
}

struct WatchLineReader<R> {
    reader: R,
    pending: Vec<u8>,
    eof: bool,
}

impl<R> WatchLineReader<R>
where
    R: std::io::Read,
{
    const fn new(reader: R) -> Self {
        Self {
            reader,
            pending: Vec::new(),
            eof: false,
        }
    }

    fn next_line(
        &mut self,
        label: &str,
        stop: &AtomicBool,
    ) -> std::result::Result<Option<String>, String> {
        loop {
            if let Some(newline) = self.pending.iter().position(|byte| *byte == b'\n') {
                let line_end = newline.saturating_add(1);
                if line_end > LOGOSCORE_EVENT_LINE_LIMIT {
                    return Err(watch_line_limit_error(label));
                }
                let remaining = self.pending.split_off(line_end);
                let line = std::mem::replace(&mut self.pending, remaining);
                return decode_watch_line(line, label).map(Some);
            }
            if self.pending.len() > LOGOSCORE_EVENT_LINE_LIMIT {
                return Err(watch_line_limit_error(label));
            }
            if self.eof {
                if self.pending.is_empty() {
                    return Ok(None);
                }
                let line = std::mem::take(&mut self.pending);
                return decode_watch_line(line, label).map(Some);
            }
            if stop.load(Ordering::Acquire) {
                return Ok(None);
            }

            let mut buffer = [0_u8; 8192];
            match self.reader.read(&mut buffer) {
                Ok(0) => self.eof = true,
                Ok(read) => self.pending.extend_from_slice(
                    buffer
                        .get(..read)
                        .ok_or_else(|| format!("{label} watch read exceeded its buffer"))?,
                ),
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    thread::sleep(LOGOSCORE_POLL_INTERVAL);
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) => return Err(format!("failed to read {label} output: {error}")),
            }
        }
    }
}

fn decode_watch_line(bytes: Vec<u8>, label: &str) -> std::result::Result<String, String> {
    String::from_utf8(bytes).map_err(|error| format!("{label} output is not UTF-8: {error}"))
}

fn watch_line_limit_error(label: &str) -> String {
    format!("{label} event exceeded {LOGOSCORE_EVENT_LINE_LIMIT} byte line limit")
}

#[cfg(unix)]
fn configure_watch_pipe_nonblocking<F>(descriptor: &F) -> std::io::Result<()>
where
    F: std::os::fd::AsFd,
{
    use nix::fcntl::{FcntlArg, OFlag, fcntl};

    let current = fcntl(descriptor, FcntlArg::F_GETFL).map_err(std::io::Error::from)?;
    let flags = OFlag::from_bits_truncate(current) | OFlag::O_NONBLOCK;
    fcntl(descriptor, FcntlArg::F_SETFL(flags))
        .map(drop)
        .map_err(std::io::Error::from)
}

struct FailedWatchStart {
    child: Child,
    reader: Option<thread::JoinHandle<()>>,
    reader_stop: Option<Arc<AtomicBool>>,
    process_permit: StreamingCommandPermit,
    recovery: mpsc::Sender<LogoscoreWatchRecovery>,
    label: String,
}

impl FailedWatchStart {
    fn new(
        child: Child,
        reader: Option<thread::JoinHandle<()>>,
        reader_stop: Option<Arc<AtomicBool>>,
        process_permit: StreamingCommandPermit,
        recovery: mpsc::Sender<LogoscoreWatchRecovery>,
        label: &str,
    ) -> Self {
        Self {
            child,
            reader,
            reader_stop,
            process_permit,
            recovery,
            label: label.to_owned(),
        }
    }
}

fn cleanup_failed_watch_start(primary: anyhow::Error, state: FailedWatchStart) -> anyhow::Error {
    cleanup_failed_watch_start_with(primary, state, stop_watch_child_with_retry)
}

fn cleanup_failed_watch_start_with<F>(
    primary: anyhow::Error,
    state: FailedWatchStart,
    cleanup: F,
) -> anyhow::Error
where
    F: FnOnce(&mut Child, &str) -> Result<()>,
{
    let FailedWatchStart {
        mut child,
        reader,
        reader_stop,
        process_permit,
        recovery,
        label,
    } = state;
    let reader_stop = reader_stop.unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
    reader_stop.store(true, Ordering::Release);
    let stop = cleanup(&mut child, &label);
    if let Err(stop) = stop {
        handoff_watch_recovery(
            Some(recovery),
            LogoscoreWatchRecovery {
                child,
                reader,
                stderr_reader: None,
                reader_stop,
                process_permit: Some(process_permit),
                label: label.clone(),
            },
        );
        return LogoscoreWatchCleanupUnconfirmed::new(format!(
            "{primary}; failed watch-start process cleanup: {stop:#}"
        ))
        .into();
    }
    drop(process_permit);
    let join = reader.map_or(Ok(()), |reader| {
        reader
            .join()
            .map_err(|_| anyhow::anyhow!("{label} output reader panicked during cleanup"))
    });
    match join {
        Ok(()) => primary,
        Err(join) => LogoscoreWatchCleanupUnconfirmed::new(format!(
            "{primary}; failed watch-start reader cleanup: {join:#}"
        ))
        .into(),
    }
}

fn handoff_watch_recovery(
    sender: Option<mpsc::Sender<LogoscoreWatchRecovery>>,
    recovery: LogoscoreWatchRecovery,
) {
    let Some(sender) = sender else {
        run_watch_recovery(recovery);
        return;
    };
    if let Err(error) = sender.send(recovery) {
        run_watch_recovery(error.0);
    }
}

fn ensure_logoscore_event_watch_supported() -> Result<()> {
    #[cfg(unix)]
    {
        Ok(())
    }
    #[cfg(not(unix))]
    {
        bail!(
            "logoscore event watch is unsupported on this platform because bounded process-group cleanup is unavailable"
        )
    }
}

fn stop_watch_child_with_retry(child: &mut Child, label: &str) -> Result<()> {
    match stop_watch_child(child, label) {
        Ok(()) => Ok(()),
        Err(first) => stop_watch_child(child, label).map_err(|second| {
            LogoscoreWatchCleanupUnconfirmed::new(format!(
                "{label} cleanup remained unconfirmed after retry: first={first:#}; second={second:#}"
            ))
            .into()
        }),
    }
}

fn stop_watch_child(child: &mut Child, label: &str) -> Result<()> {
    match child.try_wait() {
        Ok(Some(_)) => return kill_remaining_watch_group(child, label),
        Ok(None) => {}
        Err(error) => {
            return force_stop_watch_child(
                child,
                label,
                anyhow::Error::new(error).context(format!("failed to poll {label} during cleanup")),
            );
        }
    }
    if let Err(error) = terminate_watch_child(child) {
        return force_stop_watch_child(child, label, error);
    }
    let deadline = StdInstant::now()
        .checked_add(LOGOSCORE_WATCH_STOP_GRACE)
        .context("logoscore event watch cleanup deadline overflow")?;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return kill_remaining_watch_group(child, label),
            Ok(None) => {}
            Err(error) => {
                return force_stop_watch_child(
                    child,
                    label,
                    anyhow::Error::new(error)
                        .context(format!("failed to poll {label} during cleanup")),
                );
            }
        }
        if StdInstant::now() >= deadline {
            break;
        }
        thread::sleep(LOGOSCORE_POLL_INTERVAL);
    }
    force_stop_watch_child(
        child,
        label,
        anyhow::anyhow!("{label} did not stop after graceful termination"),
    )
}

fn force_stop_watch_child(child: &mut Child, label: &str, primary: anyhow::Error) -> Result<()> {
    let group_kill = kill_watch_child(child);
    let direct_kill = child
        .kill()
        .with_context(|| format!("failed to kill direct {label} process"));
    let deadline = StdInstant::now()
        .checked_add(LOGOSCORE_WATCH_STOP_GRACE)
        .context("logoscore event watch forced-cleanup deadline overflow")?;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                if let Err(group_error) = &group_kill {
                    return Err(anyhow::anyhow!(
                        "{primary}; direct process reaped but process-group cleanup failed: {group_error:#}"
                    ));
                }
                return Ok(());
            }
            Ok(None) => {}
            Err(error) => {
                return Err(anyhow::anyhow!(
                    "{primary}; forced cleanup failed: group={}, direct={}, reap={error}",
                    watch_cleanup_status(group_kill),
                    watch_cleanup_status(direct_kill),
                ));
            }
        }
        if StdInstant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "{primary}; forced cleanup timed out: group={}, direct={}",
                watch_cleanup_status(group_kill),
                watch_cleanup_status(direct_kill),
            ));
        }
        thread::sleep(LOGOSCORE_POLL_INTERVAL);
    }
}

#[cfg(unix)]
fn kill_remaining_watch_group(child: &mut Child, label: &str) -> Result<()> {
    kill_watch_child(child)
        .with_context(|| format!("failed to kill remaining {label} process-group members"))
}

#[cfg(not(unix))]
fn kill_remaining_watch_group(_child: &mut Child, _label: &str) -> Result<()> {
    Ok(())
}

fn watch_cleanup_status(result: Result<()>) -> String {
    match result {
        Ok(()) => "ok".to_owned(),
        Err(error) => format!("{error:#}"),
    }
}

#[cfg(unix)]
fn terminate_watch_child(child: &mut Child) -> Result<()> {
    signal_watch_process_group(child, nix::sys::signal::Signal::SIGTERM)
}

#[cfg(not(unix))]
fn terminate_watch_child(child: &mut Child) -> Result<()> {
    child
        .kill()
        .context("failed to terminate logoscore event watch")
}

#[cfg(unix)]
fn kill_watch_child(child: &mut Child) -> Result<()> {
    signal_watch_process_group(child, nix::sys::signal::Signal::SIGKILL)
}

#[cfg(not(unix))]
fn kill_watch_child(child: &mut Child) -> Result<()> {
    child.kill().context("failed to kill logoscore event watch")
}

#[cfg(unix)]
fn signal_watch_process_group(child: &Child, signal: nix::sys::signal::Signal) -> Result<()> {
    use nix::{errno::Errno, sys::signal::killpg, unistd::Pid};

    let process_group = i32::try_from(child.id()).context("logoscore watch PID is too large")?;
    match killpg(Pid::from_raw(process_group), signal) {
        Ok(()) | Err(Errno::ESRCH) => Ok(()),
        Err(error) => Err(error).context("failed to signal logoscore event watch process group"),
    }
}

fn configured_runtime() -> LogoscoreCliRuntime {
    let env_program = env::var("LOGOSCORE_BIN")
        .ok()
        .filter(|value| !value.is_empty());
    let program = env_program
        .clone()
        .unwrap_or_else(|| "logoscore".to_owned());
    let env_user = env::var("LOGOSCORE_USER")
        .ok()
        .filter(|value| !value.is_empty());
    let env_home = env::var("LOGOSCORE_HOME")
        .ok()
        .filter(|value| !value.is_empty());
    let config_dir = env::var("LOGOSCORE_CONFIG_DIR")
        .ok()
        .filter(|value| !value.is_empty());
    let configured =
        env_program.is_some() || env_user.is_some() || env_home.is_some() || config_dir.is_some();

    LogoscoreCliRuntime {
        runner: LogosCoreRunner {
            program,
            sudo_user: env_user,
            home: env_home,
            config_dir,
            label: if configured {
                "configured logoscore".to_owned()
            } else {
                "plain logoscore".to_owned()
            },
        },
    }
}

fn normalize_call_value(value: &mut Value) {
    let Some(call_value) = value
        .get_mut("result")
        .and_then(|result| result.get_mut("value"))
    else {
        return;
    };
    let Some(raw) = call_value.as_str() else {
        return;
    };
    let trimmed = raw.trim();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return;
    }
    let Ok(parsed) = serde_json::from_str::<Value>(trimmed) else {
        return;
    };
    *call_value = parsed;
}

pub(crate) fn normalize_module_call_value(
    module: &str,
    method: &str,
    value: Value,
) -> Result<Value> {
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !status.is_empty() && status != "ok" {
        bail!(
            "{module}.{method} returned status `{status}`: {}",
            crate::response_excerpt(&value.to_string())
        );
    }

    let Some(result) = value.get("result") else {
        return Ok(parse_module_json_string(value));
    };
    if let Some(object) = result.as_object()
        && let Some(success) = object.get("success").and_then(Value::as_bool)
    {
        if !success {
            let error = object
                .get("error")
                .map(module_value_error_text)
                .filter(|error| !error.is_empty())
                .unwrap_or_else(|| "module call failed".to_owned());
            bail!("{module}.{method} failed: {error}");
        }
        return Ok(object
            .get("value")
            .cloned()
            .map(parse_module_json_string)
            .unwrap_or(Value::Null));
    }
    Ok(parse_module_json_string(result.clone()))
}

fn parse_module_json_string(value: Value) -> Value {
    let Value::String(text) = value else {
        return value;
    };
    let trimmed = text.trim();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return Value::String(text);
    }
    serde_json::from_str(trimmed).unwrap_or(Value::String(text))
}

fn module_value_error_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => String::new(),
        value => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use super::*;
    use serde_json::json;

    struct RecordingTransport {
        kind: ModuleTransportKind,
        reply_kind: ModuleTransportKind,
        calls: AtomicUsize,
        last_call: Mutex<Option<ModuleCall>>,
    }

    impl RecordingTransport {
        fn new(kind: ModuleTransportKind, reply_kind: ModuleTransportKind) -> Self {
            Self {
                kind,
                reply_kind,
                calls: AtomicUsize::new(0),
                last_call: Mutex::new(None),
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn unix_event_watch_contract_requires_process_group_cleanup() -> Result<()> {
        ensure_logoscore_event_watch_supported()
    }

    #[cfg(unix)]
    #[test]
    fn watch_recovery_queue_retries_without_head_of_line_blocking() -> Result<()> {
        fn recovery(label: &str) -> Result<LogoscoreWatchRecovery> {
            let child = Command::new("sh")
                .arg("-c")
                .arg("while :; do sleep 1; done")
                .spawn()
                .with_context(|| format!("failed to start {label} recovery fixture"))?;
            Ok(LogoscoreWatchRecovery {
                child,
                reader: None,
                stderr_reader: None,
                reader_stop: Arc::new(AtomicBool::new(false)),
                process_permit: None,
                label: label.to_owned(),
            })
        }

        let (sender, receiver) = mpsc::channel();
        sender.send(recovery("first")?)?;
        sender.send(recovery("second")?)?;
        drop(sender);
        let mut attempts = Vec::new();
        let mut first_attempts = 0_u8;
        run_watch_recovery_queue_with(&receiver, Duration::ZERO, |recovery| {
            attempts.push(recovery.label.clone());
            if recovery.label == "first" {
                first_attempts = first_attempts.saturating_add(1);
                if first_attempts == 1 {
                    return false;
                }
            }
            recovery.child.kill().is_ok() && recovery.child.wait().is_ok()
        });

        anyhow::ensure!(
            attempts == ["first", "second", "first"],
            "watch recovery queue blocked later cleanup behind a retry: {attempts:?}"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn failed_watch_start_hands_process_handle_to_recovery() -> Result<()> {
        use std::os::unix::process::CommandExt as _;

        use nix::{errno::Errno, sys::signal::kill, unistd::Pid};

        let control = CommandControl::new(
            CancellationToken::new(),
            StdInstant::now() + Duration::from_secs(2),
        );
        let permit = acquire_streaming_command_permit("failed watch start fixture", &control)?;
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("while :; do sleep 1; done")
            .process_group(0);
        let child = command.spawn()?;
        let pid = i32::try_from(child.id()).context("watch fixture PID is too large")?;
        let error = cleanup_failed_watch_start_with(
            anyhow::anyhow!("injected watch-start failure"),
            FailedWatchStart::new(
                child,
                None,
                None,
                permit,
                watch_recovery_sender()?,
                "injected failed watch",
            ),
            |_child, _label| bail!("injected cleanup uncertainty"),
        );
        anyhow::ensure!(
            error
                .downcast_ref::<LogoscoreWatchCleanupUnconfirmed>()
                .is_some(),
            "failed watch start lost cleanup-uncertain classification: {error:#}"
        );

        let deadline = StdInstant::now() + Duration::from_secs(2);
        loop {
            match kill(Pid::from_raw(pid), None) {
                Err(Errno::ESRCH) => break,
                Ok(()) => {}
                Err(error) => return Err(error).context("failed to inspect recovered watch"),
            }
            if StdInstant::now() >= deadline {
                bail!("failed watch-start recovery left PID {pid} running");
            }
            thread::sleep(LOGOSCORE_POLL_INTERVAL);
        }
        Ok(())
    }

    #[cfg(not(unix))]
    #[test]
    fn non_unix_backup_readiness_fails_before_spawning_logoscore() -> Result<()> {
        let runtime = LogoscoreCliRuntime::managed(
            "program-that-must-not-be-spawned".to_owned(),
            "config-that-must-not-be-read".to_owned(),
        );
        let error = runtime
            .storage_backup_download_readiness()
            .err()
            .context("non-Unix backup readiness unexpectedly claimed event-watch support")?;
        anyhow::ensure!(
            error
                .to_string()
                .contains("bounded process-group cleanup is unavailable"),
            "non-Unix readiness did not fail closed: {error:#}"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn json_event_reader_bounds_queue_and_event_fields() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let burst_path = directory.path().join("burst.ndjson");
        let ready = json!({
            "type": "subscription_ready",
            "protocol": "logoscore.watch",
            "version": 1,
            "module": "storage_module",
            "event": "storageDownloadDone",
        });
        let event = json!({
            "type": "event",
            "protocol": "logoscore.watch",
            "version": 1,
            "timestamp": "2026-07-14T12:00:00Z",
            "module": "storage_module",
            "event": "storageDownloadDone",
            "data": { "arg0": "{}" },
        });
        let mut burst_frames = format!("{}\n", serde_json::to_string(&ready)?);
        for _ in 0..70 {
            burst_frames.push_str(&serde_json::to_string(&event)?);
            burst_frames.push('\n');
        }
        fs::write(&burst_path, burst_frames)?;
        let mut burst = Command::new("cat")
            .arg(&burst_path)
            .stdout(Stdio::piped())
            .spawn()?;
        let stdout = burst.stdout.take().context("burst fixture has no stdout")?;
        let (sender, receiver) = mpsc::sync_channel(LOGOSCORE_EVENT_QUEUE_CAPACITY);
        let (readiness_sender, readiness) = mpsc::channel();
        let failure = Arc::new(Mutex::new(None));
        let stop = AtomicBool::new(false);
        read_json_watch_output(
            stdout,
            "burst watch",
            ("storage_module", "storageDownloadDone"),
            &readiness_sender,
            &sender,
            &failure,
            &stop,
        );
        burst.wait()?;
        anyhow::ensure!(
            matches!(readiness.recv()?, LogoscoreWatchReadiness::Ready),
            "JSON readiness frame was not accepted"
        );
        let queued = receiver.try_iter().count();
        anyhow::ensure!(
            queued == LOGOSCORE_EVENT_QUEUE_CAPACITY,
            "event queue exceeded or underfilled its bound: {queued}"
        );
        anyhow::ensure!(
            take_watch_output_failure(&failure)
                .is_some_and(|error| error.contains("bounded event queue capacity")),
            "event queue overflow was not explicit"
        );

        let fields_path = directory.path().join("fields.ndjson");
        let mut data = serde_json::Map::new();
        for index in 0..=LOGOSCORE_EVENT_FIELD_LIMIT {
            data.insert(format!("arg{index}"), Value::String("value".to_owned()));
        }
        fs::write(
            &fields_path,
            format!(
                "{}\n{}\n",
                serde_json::to_string(&ready)?,
                serde_json::to_string(&json!({
                    "type": "event",
                    "protocol": "logoscore.watch",
                    "version": 1,
                    "timestamp": "2026-07-14T12:00:00Z",
                    "module": "storage_module",
                    "event": "storageDownloadDone",
                    "data": data,
                }))?,
            ),
        )?;
        let mut fields = Command::new("cat")
            .arg(&fields_path)
            .stdout(Stdio::piped())
            .spawn()?;
        let stdout = fields
            .stdout
            .take()
            .context("field fixture has no stdout")?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let (readiness_sender, readiness) = mpsc::channel();
        let failure = Arc::new(Mutex::new(None));
        let stop = AtomicBool::new(false);
        read_json_watch_output(
            stdout,
            "field watch",
            ("storage_module", "storageDownloadDone"),
            &readiness_sender,
            &sender,
            &failure,
            &stop,
        );
        fields.wait()?;
        anyhow::ensure!(
            matches!(readiness.recv()?, LogoscoreWatchReadiness::Ready),
            "JSON readiness frame was not accepted"
        );
        match receiver.recv()? {
            LogoscoreWatchOutput::Error(error) => anyhow::ensure!(
                error.contains("field limit"),
                "unexpected field-bound error: {error}"
            ),
            _ => bail!("over-field event did not return a parser error"),
        }
        Ok(())
    }

    #[test]
    fn watch_protocol_rejects_legacy_or_inexact_frames() -> Result<()> {
        for frame in [
            json!({
                "module": "storage_module",
                "event": "storageDownloadDoneV2",
                "data": {}
            }),
            json!({
                "type": "subscription_ready",
                "protocol": "logoscore.watch",
                "version": 0,
                "module": "storage_module",
                "event": "storageDownloadDoneV2"
            }),
            json!({
                "type": "subscription_ready",
                "protocol": "logoscore.watch",
                "version": 1,
                "module": "storage_module",
                "event": "storageDownloadDoneV2",
                "legacy": true
            }),
        ] {
            anyhow::ensure!(
                validate_watch_ready_frame(&frame, "storage_module", "storageDownloadDoneV2")
                    .is_err(),
                "inexact watch readiness was accepted: {frame}"
            );
        }
        let untyped_event = json!({
            "module": "storage_module",
            "event": "storageDownloadDoneV2",
            "data": { "arg0": "{}" }
        });
        anyhow::ensure!(
            validate_watch_event_frame(&untyped_event, "storage_module", "storageDownloadDoneV2")
                .is_err(),
            "legacy watch event was accepted"
        );
        for inexact_event in [
            json!({
                "type": "event",
                "protocol": "logoscore.watch",
                "version": 1,
                "module": "storage_module",
                "event": "storageDownloadDoneV2",
                "data": { "arg0": "{}" }
            }),
            json!({
                "type": "event",
                "protocol": "logoscore.watch",
                "version": 1,
                "timestamp": 1,
                "module": "storage_module",
                "event": "storageDownloadDoneV2",
                "data": { "arg0": "{}" }
            }),
            json!({
                "type": "event",
                "protocol": "logoscore.watch",
                "version": 1,
                "timestamp": "2026-07-14T12:00:00Z",
                "module": "storage_module",
                "event": "storageDownloadDoneV2",
                "data": { "arg0": "{}" },
                "legacy": true
            }),
        ] {
            anyhow::ensure!(
                validate_watch_event_frame(
                    &inexact_event,
                    "storage_module",
                    "storageDownloadDoneV2",
                )
                .is_err(),
                "inexact typed watch event was accepted: {inexact_event}"
            );
        }
        Ok(())
    }

    #[test]
    fn shared_staging_requires_local_transport_on_every_platform() -> Result<()> {
        let local = json!({
            "instance_id": "instance-local",
            "daemon": { "core_service": { "transport": "local" } }
        });
        anyhow::ensure!(
            local_transport_instance_id(&local, "downloadToUrl")? == "instance-local",
            "local shared-filesystem transport identity drifted"
        );

        for incompatible in [
            json!({
                "instance_id": "instance-remote",
                "daemon": { "core_service": { "transport": "tcp" } }
            }),
            json!({
                "daemon": { "core_service": { "transport": "local" } }
            }),
        ] {
            anyhow::ensure!(
                local_transport_instance_id(&incompatible, "downloadToUrl").is_err(),
                "shared staging accepted incompatible client config: {incompatible}"
            );
        }
        Ok(())
    }

    #[test]
    fn queued_watch_terminal_wins_over_concurrent_cancellation() -> Result<()> {
        let terminal = json!({
            "type": "event",
            "protocol": "logoscore.watch",
            "version": 1,
            "timestamp": "2026-07-14T12:00:00Z",
            "module": "storage_module",
            "event": "storageDownloadDoneV2",
            "data": { "arg0": "{}" },
        });
        let (sender, output) = mpsc::sync_channel(1);
        sender.send(LogoscoreWatchOutput::Value(terminal.clone()))?;
        drop(sender);
        let (_readiness_sender, readiness) = mpsc::channel();
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let control = CommandControl::new(cancellation, StdInstant::now() + Duration::from_secs(1));
        let mut watch = LogoscoreEventWatch {
            child: None,
            output,
            output_failure: Arc::new(Mutex::new(None)),
            readiness,
            reader: None,
            stderr_reader: None,
            reader_stop: Arc::new(AtomicBool::new(false)),
            process_permit: None,
            recovery: None,
            label: "queued terminal watch".to_owned(),
        };

        anyhow::ensure!(
            watch.next_value(&control)? == terminal,
            "queued terminal lost to concurrent cancellation"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn event_watch_drains_terminal_emitted_immediately_before_exit() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-watch-exit");
        fs::write(
            &program,
            "#!/bin/sh\n\
             if [ \"$1\" = \"--config-dir\" ]; then shift 2; fi\n\
             printf '%s\\n' '{\"type\":\"subscription_ready\",\"protocol\":\"logoscore.watch\",\"version\":1,\"module\":\"storage_module\",\"event\":\"storageDownloadDone\"}'\n\
             printf '%s\\n' '{\"type\":\"event\",\"protocol\":\"logoscore.watch\",\"version\":1,\"timestamp\":\"2026-07-14T12:00:00Z\",\"module\":\"storage_module\",\"event\":\"storageDownloadDone\",\"data\":{\"arg0\":\"{\\\"success\\\":true,\\\"sessionId\\\":\\\"session-exit\\\"}\"}}'\n",
        )?;
        let mut permissions = fs::metadata(&program)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&program, permissions)?;
        let runtime = LogoscoreCliRuntime::managed(
            program.display().to_string(),
            directory.path().display().to_string(),
        );

        for _ in 0..20 {
            let control = CommandControl::new(
                CancellationToken::new(),
                StdInstant::now() + Duration::from_secs(2),
            );
            let mut watch =
                runtime.start_event_watch("storage_module", "storageDownloadDone", &control)?;
            watch.wait_ready(&control)?;
            let value = watch.next_value(&control)?;
            anyhow::ensure!(
                value.pointer("/data/arg0").and_then(Value::as_str)
                    == Some(r#"{"success":true,"sessionId":"session-exit"}"#),
                "terminal emitted before watcher exit was lost: {value}"
            );
            watch.stop()?;
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn event_watch_stop_kills_pipe_holding_process_group_descendant() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        use nix::{sys::signal::Signal, unistd::Pid};

        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-watch-descendant");
        let descendant_path = directory.path().join("descendant.pid");
        fs::write(
            &program,
            "#!/bin/sh\n\
             state_dir=$2\n\
             (trap '' TERM; while :; do sleep 0.05; done) &\n\
             printf '%s' \"$!\" > \"$state_dir/descendant.pid\"\n\
             trap 'exit 0' TERM\n\
             printf '%s\\n' '{\"type\":\"subscription_ready\",\"protocol\":\"logoscore.watch\",\"version\":1,\"module\":\"storage_module\",\"event\":\"storageDownloadDone\"}'\n\
             while :; do sleep 0.05; done\n",
        )?;
        let mut permissions = fs::metadata(&program)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&program, permissions)?;
        let runtime = LogoscoreCliRuntime::managed(
            program.display().to_string(),
            directory.path().display().to_string(),
        );
        let control = CommandControl::new(
            CancellationToken::new(),
            StdInstant::now() + Duration::from_secs(2),
        );
        let mut watch =
            runtime.start_event_watch("storage_module", "storageDownloadDone", &control)?;
        watch.wait_ready(&control)?;
        let process_group = i32::try_from(
            watch
                .child
                .as_ref()
                .context("watch fixture has no child")?
                .id(),
        )?;
        let descendant = fs::read_to_string(&descendant_path)?
            .trim()
            .parse::<i32>()
            .context("descendant fixture wrote an invalid PID")?;

        let (stop_sender, stop_receiver) = mpsc::channel();
        let started = StdInstant::now();
        let stopper = thread::spawn(move || {
            let _result = stop_sender.send(watch.stop());
        });
        let stop_result = match stop_receiver.recv_timeout(Duration::from_secs(1)) {
            Ok(result) => result,
            Err(error) => {
                let _result =
                    nix::sys::signal::killpg(Pid::from_raw(process_group), Signal::SIGKILL);
                let _result = stopper.join();
                bail!("watch stop blocked on inherited output pipes: {error}");
            }
        };
        stopper
            .join()
            .map_err(|_| anyhow::anyhow!("watch stopper panicked"))?;
        stop_result?;
        anyhow::ensure!(
            started.elapsed() < Duration::from_secs(1),
            "watch stop exceeded its bounded cleanup window"
        );

        let status_path = PathBuf::from(format!("/proc/{descendant}/stat"));
        let deadline = StdInstant::now() + Duration::from_secs(1);
        loop {
            let live = match fs::read_to_string(&status_path) {
                Ok(status) => status
                    .rsplit_once(')')
                    .and_then(|(_, fields)| fields.split_whitespace().next())
                    .is_none_or(|state| state != "Z"),
                Err(error) if error.kind() == ErrorKind::NotFound => false,
                Err(error) => return Err(error).context("failed to inspect watch descendant"),
            };
            if !live {
                break;
            }
            if StdInstant::now() >= deadline {
                let _result =
                    nix::sys::signal::killpg(Pid::from_raw(process_group), Signal::SIGKILL);
                bail!("watch cleanup left descendant PID {descendant} running");
            }
            thread::sleep(LOGOSCORE_POLL_INTERVAL);
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn shared_download_close_surfaces_workspace_removal_failure() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir()?;
        let root = directory.path().to_path_buf();
        let path = root.join("backup.json");
        fs::write(&path, b"payload")?;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o500))?;
        let staged = LogoscoreSharedDownload { directory, path };

        let error = staged
            .close()
            .err()
            .context("non-writable download workspace should not report clean removal")?;
        anyhow::ensure!(
            error
                .to_string()
                .contains("failed to remove logoscore download workspace"),
            "workspace cleanup lost its error: {error:#}"
        );
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        fs::remove_dir_all(root)?;
        Ok(())
    }

    impl ModuleTransport for RecordingTransport {
        fn kind(&self) -> ModuleTransportKind {
            self.kind
        }

        fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            if let Ok(mut last_call) = self.last_call.lock() {
                *last_call = Some(call.clone());
            }
            let reply_kind = self.reply_kind;
            Box::pin(async move {
                Ok(ModuleCallReply::new(
                    reply_kind,
                    json!({
                        "module": call.module(),
                        "method": call.method(),
                        "args": call.args(),
                    }),
                ))
            })
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn controlled_cli_call_does_not_overclaim_remote_termination() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-test");
        let pid_path = directory.path().join("logoscore-test.pid");
        fs::write(
            &program,
            "#!/bin/sh\nprintf '%s' \"$$\" > \"${0}.pid\"\nwhile :; do :; done\n",
        )?;
        let mut permissions = fs::metadata(&program)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&program, permissions)?;
        let transport = LogoscoreCliTransport {
            runtime: LogoscoreCliRuntime {
                runner: LogosCoreRunner {
                    program: program.to_string_lossy().into_owned(),
                    sudo_user: None,
                    home: None,
                    config_dir: None,
                    label: "test logoscore".to_owned(),
                },
            },
        };
        let cancellation = CancellationToken::new();
        let cancel_request = cancellation.clone();
        let pid_for_cancel = pid_path.clone();
        let canceler = tokio::spawn(async move {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !pid_for_cancel.exists() {
                if Instant::now() >= deadline {
                    bail!("timed out waiting for CLI child process");
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            cancel_request.cancel();
            Ok::<(), anyhow::Error>(())
        });
        let control = ModuleCallControl::new(
            cancellation,
            Instant::now() + Duration::from_secs(5),
            Arc::new(AtomicU8::new(1)),
        );
        let call = ModuleCall::new(
            ModuleTransportKind::LogoscoreCli,
            "storage_module",
            "get",
            vec![],
        )?;

        let Err(error) = transport.call_controlled(call, control).await else {
            bail!("canceled CLI module call unexpectedly completed");
        };
        canceler.await.context("CLI canceler task failed")??;
        let terminated = error
            .downcast_ref::<ModuleCallTerminated>()
            .context("CLI cancellation lost typed termination evidence")?;
        anyhow::ensure!(
            terminated.reason() == ModuleCallStopReason::CancelRequested
                && terminated.evidence() == ModuleCallTerminationEvidence::LocallyAbandoned,
            "unexpected CLI termination evidence: {terminated:?}"
        );
        anyhow::ensure!(
            terminated
                .to_string()
                .contains("remote termination unknown"),
            "CLI termination message overclaimed remote effect: {terminated}"
        );
        let pid = fs::read_to_string(&pid_path)?;
        let alive = Command::new("sh")
            .arg("-c")
            .arg("kill -0 \"$1\" 2>/dev/null")
            .arg("logoscore-reap-probe")
            .arg(pid.trim())
            .status()?;
        anyhow::ensure!(!alive.success(), "CLI child was not reaped");
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn pre_canceled_cli_call_reports_that_no_external_process_started() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let marker = directory.path().join("unexpected-start");
        let transport = LogoscoreCliTransport {
            runtime: LogoscoreCliRuntime {
                runner: LogosCoreRunner {
                    program: marker.to_string_lossy().into_owned(),
                    sudo_user: None,
                    home: None,
                    config_dir: None,
                    label: "test logoscore".to_owned(),
                },
            },
        };
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let control = ModuleCallControl::new(
            cancellation,
            Instant::now() + Duration::from_secs(5),
            Arc::new(AtomicU8::new(1)),
        );
        let call = ModuleCall::new(
            ModuleTransportKind::LogoscoreCli,
            "storage_module",
            "get",
            vec![],
        )?;

        let Err(error) = transport.call_controlled(call, control).await else {
            bail!("pre-canceled CLI module call unexpectedly completed");
        };
        let terminated = error
            .downcast_ref::<ModuleCallTerminated>()
            .context("pre-canceled CLI call lost typed termination evidence")?;
        anyhow::ensure!(
            terminated.reason() == ModuleCallStopReason::CancelRequested
                && terminated.evidence() == ModuleCallTerminationEvidence::NotStarted,
            "unexpected pre-canceled CLI evidence: {terminated:?}"
        );
        anyhow::ensure!(!marker.exists(), "pre-canceled CLI call started a process");
        Ok(())
    }

    #[tokio::test]
    async fn default_controlled_transport_preflights_before_call_invocation() -> Result<()> {
        let transport =
            RecordingTransport::new(ModuleTransportKind::Module, ModuleTransportKind::Module);
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let control = ModuleCallControl::new(
            cancellation,
            Instant::now() + Duration::from_secs(5),
            Arc::new(AtomicU8::new(1)),
        );
        let call = ModuleCall::new(ModuleTransportKind::Module, "storage_module", "get", vec![])?;

        let controlled = transport.call_controlled(call, control);
        anyhow::ensure!(
            transport.calls.load(Ordering::Acquire) == 0,
            "controlled transport invoked call while constructing a queued future"
        );
        let Err(error) = controlled.await else {
            bail!("pre-canceled default transport call unexpectedly completed");
        };
        let terminated = error
            .downcast_ref::<ModuleCallTerminated>()
            .context("default transport preflight lost typed termination evidence")?;
        anyhow::ensure!(
            terminated.reason() == ModuleCallStopReason::CancelRequested
                && terminated.evidence() == ModuleCallTerminationEvidence::NotStarted,
            "unexpected default transport preflight evidence: {terminated:?}"
        );
        anyhow::ensure!(
            transport.calls.load(Ordering::Acquire) == 0,
            "pre-canceled default transport invoked call"
        );
        Ok(())
    }

    #[tokio::test]
    async fn dispatch_preserves_json_arguments_and_transport_identity() -> Result<()> {
        let transport = RecordingTransport::new(
            ModuleTransportKind::LogoscoreCli,
            ModuleTransportKind::LogoscoreCli,
        );
        let args = vec![json!({ "nested": [true, 7] }), json!("0")];
        let call = ModuleCall::new(
            ModuleTransportKind::LogoscoreCli,
            "storage_module",
            "get",
            args.clone(),
        )?;

        let reply = dispatch_module_call(&transport, call).await?;

        anyhow::ensure!(reply.transport() == ModuleTransportKind::LogoscoreCli);
        anyhow::ensure!(reply.into_value().get("args") == Some(&json!(args)));
        let recorded = transport
            .last_call
            .lock()
            .map_err(|error| anyhow::anyhow!("recording transport lock failed: {error}"))?
            .clone()
            .context("recording transport did not receive call")?;
        anyhow::ensure!(recorded.transport() == ModuleTransportKind::LogoscoreCli);
        anyhow::ensure!(recorded.args() == args);
        Ok(())
    }

    #[tokio::test]
    async fn dispatch_rejects_transport_mismatch_before_invocation() -> Result<()> {
        let transport = RecordingTransport::new(
            ModuleTransportKind::LogoscoreCli,
            ModuleTransportKind::LogoscoreCli,
        );
        let call = ModuleCall::new(ModuleTransportKind::Module, "storage_module", "get", vec![])?;

        let Err(error) = dispatch_module_call(&transport, call).await else {
            bail!("transport mismatch unexpectedly succeeded");
        };

        anyhow::ensure!(
            error
                .to_string()
                .contains("resolved module transport `module` is unavailable")
        );
        anyhow::ensure!(transport.calls.load(Ordering::Relaxed) == 0);
        Ok(())
    }

    #[tokio::test]
    async fn dispatch_rejects_reply_identity_mismatch() -> Result<()> {
        let transport = RecordingTransport::new(
            ModuleTransportKind::LogoscoreCli,
            ModuleTransportKind::Module,
        );
        let call = ModuleCall::new(
            ModuleTransportKind::LogoscoreCli,
            "storage_module",
            "get",
            vec![],
        )?;

        let Err(error) = dispatch_module_call(&transport, call).await else {
            bail!("reply identity mismatch unexpectedly succeeded");
        };

        anyhow::ensure!(
            error
                .to_string()
                .contains("returned reply identity `module`")
        );
        anyhow::ensure!(transport.calls.load(Ordering::Relaxed) == 1);
        Ok(())
    }

    #[test]
    fn module_call_value_unwraps_logos_result_json_string() -> Result<()> {
        let value = normalize_module_call_value(
            "module",
            "method",
            json!({
                "status": "ok",
                "result": {
                    "success": true,
                    "value": "{\"slot\":7}",
                    "error": null
                }
            }),
        )?;

        anyhow::ensure!(value.get("slot").and_then(Value::as_u64) == Some(7));
        Ok(())
    }

    #[test]
    fn module_call_value_unwraps_plain_json_string_result() -> Result<()> {
        let value = normalize_module_call_value(
            "module",
            "method",
            json!({
                "status": "ok",
                "result": "[{\"id\":1}]"
            }),
        )?;

        anyhow::ensure!(value.as_array().map(Vec::len) == Some(1));
        Ok(())
    }

    #[test]
    fn module_call_value_reports_module_failure() {
        let result = normalize_module_call_value(
            "module",
            "method",
            json!({
                "status": "ok",
                "result": {
                    "success": false,
                    "value": null,
                    "error": "not started"
                }
            }),
        );

        assert!(result.is_err_and(|error| error.to_string().contains("not started")));
    }

    #[test]
    fn normalizes_nested_json_call_value() {
        let mut value = json!({
            "result": {
                "value": "{\"height\":1}"
            }
        });

        normalize_call_value(&mut value);

        let height = value
            .pointer("/result/value/height")
            .and_then(Value::as_u64);
        assert_eq!(height, Some(1));
    }

    #[test]
    fn keeps_non_json_call_value() {
        let mut value = json!({
            "result": {
                "value": "@[Version, Metrics]"
            }
        });

        normalize_call_value(&mut value);

        let value = value.pointer("/result/value").and_then(Value::as_str);
        assert_eq!(value, Some("@[Version, Metrics]"));
    }

    #[test]
    fn keeps_scalar_json_text_as_module_string() {
        let mut value = json!({
            "result": {
                "value": "0"
            }
        });

        normalize_call_value(&mut value);

        let value = value.pointer("/result/value").and_then(Value::as_str);
        assert_eq!(value, Some("0"));
    }

    #[test]
    fn parses_json_larger_than_error_excerpt_limit() -> Result<()> {
        let expected = json!({ "payload": "x".repeat(LOGOSCORE_OUTPUT_LIMIT * 3) });
        let encoded = serde_json::to_vec(&expected)?;

        let parsed = parse_json_stdout("logoscore test", &encoded)?;

        anyhow::ensure!(parsed == expected, "large logoscore JSON was truncated");
        Ok(())
    }

    #[test]
    fn cli_transport_builds_logoscore_call_arguments() -> Result<()> {
        let args = vec!["alpha".to_owned(), "42".to_owned()];

        let command_args = call_arguments("storage_module", "get", &args)?;

        if command_args != ["call", "storage_module", "get", "alpha", "42", "--json"] {
            bail!("unexpected logoscore call arguments: {command_args:?}");
        }
        Ok(())
    }

    #[test]
    fn configured_runtime_arguments_precede_call_arguments() {
        let runner = LogosCoreRunner {
            program: "logoscore".to_owned(),
            sudo_user: None,
            home: Some("/tmp/home".to_owned()),
            config_dir: Some("/tmp/logoscore".to_owned()),
            label: "configured logoscore".to_owned(),
        };
        let command = command_for_runner(&runner, ["call", "storage_module", "get", "--json"]);
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            args,
            [
                "--config-dir",
                "/tmp/logoscore",
                "call",
                "storage_module",
                "get",
                "--json"
            ]
        );
    }

    #[test]
    fn module_discovery_accepts_matching_loaded_method_contract() -> Result<()> {
        let modules = json!([{"name": "storage_module", "status": "loaded"}]);
        let info = json!({
            "name": "storage_module",
            "methods": [
                {"isInvokable": true, "name": "init", "signature": "init(QString)"},
                {"isInvokable": true, "name": "start", "signature": "start()"}
            ]
        });

        let discovery = module_discovery("storage_module", &modules, &info)?;

        discovery.require_method("init", "init(QString)")
    }

    #[test]
    fn module_discovery_rejects_missing_unloaded_and_mismatched_contracts() -> Result<()> {
        let missing = module_discovery("storage_module", &json!([]), &json!({}));
        let Err(error) = missing else {
            bail!("missing module discovery unexpectedly succeeded");
        };
        if !error.to_string().contains("is not listed") {
            bail!("unexpected missing module error: {error:#}");
        }

        let unloaded = module_discovery(
            "storage_module",
            &json!([{"name": "storage_module", "status": "not_loaded"}]),
            &json!({}),
        );
        let Err(error) = unloaded else {
            bail!("unloaded module discovery unexpectedly succeeded");
        };
        if !error.to_string().contains("is not loaded") {
            bail!("unexpected unloaded module error: {error:#}");
        }

        let methods = json!({
            "name": "storage_module",
            "methods": [
                {"isInvokable": true, "name": "start", "signature": "start(QString)"}
            ]
        });
        let discovery = module_discovery(
            "storage_module",
            &json!([{"name": "storage_module", "status": "loaded"}]),
            &methods,
        )?;
        let mismatch = discovery.require_method("start", "start()");
        let Err(error) = mismatch else {
            bail!("signature mismatch unexpectedly succeeded");
        };
        if !error.to_string().contains("signature mismatch") {
            bail!("unexpected signature mismatch error: {error:#}");
        }

        let absent = discovery.require_method("stop", "stop()");
        let Err(error) = absent else {
            bail!("missing method unexpectedly succeeded");
        };
        if !error
            .to_string()
            .contains("does not expose invokable method")
        {
            bail!("unexpected missing method error: {error:#}");
        }
        Ok(())
    }
}
