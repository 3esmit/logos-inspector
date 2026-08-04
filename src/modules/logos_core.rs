use std::{
    collections::{HashMap, VecDeque},
    env, fs,
    future::Future,
    io::{ErrorKind, Read as _, Write as _},
    path::{Path, PathBuf},
    pin::Pin,
    process::{Child, Command, Stdio},
    sync::{
        Arc, LazyLock, Mutex, MutexGuard, TryLockError,
        atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant as StdInstant},
};

use anyhow::{Context as _, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose};
#[cfg(target_os = "linux")]
use serde::Deserialize;
use serde::{Serialize, Serializer};
use serde_json::{Value, json};
use tempfile::{NamedTempFile, TempDir};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::support::command_runner::{
    CommandControl, CommandRunPolicy, CommandStopReason, CommandTerminated,
    CommandTerminationScope, DEFAULT_COMMAND_CAPTURE_LIMIT, StreamingCommandPermit,
    acquire_streaming_command_permit, output_text, process_message, run_command,
    run_command_allow_failure, run_command_controlled, run_command_controlled_allow_failure,
};
use crate::support::settings_backup::SETTINGS_BACKUP_MAX_BYTES;
use crate::support::storage_download_contract::{
    STORAGE_DOWNLOAD_V2_METHOD, STORAGE_DOWNLOAD_V2_METHOD_SIGNATURES,
};
use crate::support::work_tracker::{BlockingWorkGuard, BlockingWorkTracker};

const LOGOSCORE_POLL_INTERVAL: Duration = Duration::from_millis(25);
const LOGOSCORE_OUTPUT_LIMIT: usize = 4096;
const LOGOSCORE_JSON_OUTPUT_LIMIT: usize = 16 * 1024 * 1024;
const LOGOSCORE_MAX_JSON_OUTPUT_LIMIT: usize = 64 * 1024 * 1024;
const LOGOSCORE_CLIENT_CONFIG_LIMIT: usize = 64 * 1024;
const LOGOSCORE_EVENT_LINE_LIMIT: usize = 1024 * 1024;
const LOGOSCORE_EVENT_FIELD_LIMIT: usize = 64;
const LOGOSCORE_EVENT_NAME_LIMIT: usize = 256;
const LOGOSCORE_EVENT_QUEUE_CAPACITY: usize = 64;
const LOGOSCORE_WATCH_STOP_GRACE: Duration = Duration::from_millis(250);
const LOGOSCORE_CLI_COMMAND_GATE_POLL_INTERVAL: Duration = Duration::from_millis(10);
const LOGOSCORE_CLI_STATUS_SNAPSHOT_FRESHNESS: Duration = Duration::from_secs(20);
const LOGOSCORE_CLI_MODULES_SNAPSHOT_FRESHNESS: Duration = Duration::from_secs(30);
const LOGOSCORE_CLI_FAILURE_BACKOFF: Duration = Duration::from_secs(1);
const LOGOSCORE_MODULE_DISCOVERY_ATTEMPTS: usize = 3;
const LOGOSCORE_MODULE_DISCOVERY_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(30);
const LOGOSCORE_MODULE_DISCOVERY_RETRY_DELAY: Duration = Duration::from_secs(5);
const LOGOSCORE_WATCH_PROTOCOL: &str = "logoscore.watch";
const LOGOSCORE_WATCH_PROTOCOL_VERSION: u64 = 1;
const LOGOSCORE_WATCH_CLEANUP_TOKEN_ENV: &str = "LOGOS_INSPECTOR_WATCH_TOKEN";
const LOGOSCORE_WATCH_OWNER_PID_ENV: &str = "LOGOS_INSPECTOR_WATCH_OWNER_PID";
const LOGOSCORE_WATCH_OWNER_START_ENV: &str = "LOGOS_INSPECTOR_WATCH_OWNER_START";
const LOGOSCORE_WATCH_OWNER_NONCE_ENV: &str = "LOGOS_INSPECTOR_WATCH_OWNER_NONCE";
#[cfg(target_os = "linux")]
const LOGOSCORE_WATCH_LEASE_DIRECTORY: &str = "runtime/watch-leases";
#[cfg(target_os = "linux")]
const LOGOSCORE_WATCH_LEASE_SCHEMA_VERSION: u8 = 1;
static LOGOSCORE_WATCH_RECOVERY: LazyLock<
    std::result::Result<mpsc::Sender<LogoscoreWatchRecovery>, String>,
> = LazyLock::new(start_watch_recovery_worker);
static LOGOSCORE_WATCH_CLEANUP_SERIAL: AtomicU64 = AtomicU64::new(0);

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

    pub(crate) fn require_method_with_signatures(
        &self,
        method: &str,
        signatures: &[&str],
    ) -> Result<()> {
        anyhow::ensure!(
            !signatures.is_empty(),
            "logoscore module method contract requires at least one signature"
        );
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
        if signatures
            .iter()
            .any(|signature| *signature == found.signature)
        {
            return Ok(());
        }
        bail!(
            "logoscore module `{}` method `{method}` signature mismatch: expected one of `{}`, found `{}`",
            self.module,
            signatures.join("`, `"),
            found.signature
        )
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
    instance_id: Option<String>,
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
        Self::new_with_instance(transport, module, None, method, args)
    }

    /// Creates a call for one explicitly named module instance.
    ///
    /// A scoped call must never be silently redirected to a transport's
    /// default instance. Callers use this for independently configured
    /// Basecamp module instances, such as one Indexer per Zone channel.
    pub fn new_instance(
        transport: ModuleTransportKind,
        module: impl Into<String>,
        instance_id: impl Into<String>,
        method: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<Self> {
        Self::new_with_instance(transport, module, Some(instance_id.into()), method, args)
    }

    fn new_with_instance(
        transport: ModuleTransportKind,
        module: impl Into<String>,
        instance_id: Option<String>,
        method: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<Self> {
        let module = module.into();
        let method = method.into();
        if module.trim().is_empty() {
            bail!("module name is required");
        }
        if instance_id
            .as_deref()
            .is_some_and(|instance_id| instance_id.trim().is_empty())
        {
            bail!("module instance id is required");
        }
        if method.trim().is_empty() {
            bail!("method name is required");
        }
        Ok(Self {
            transport,
            module,
            instance_id,
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
    pub fn instance_id(&self) -> Option<&str> {
        self.instance_id.as_deref()
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
    instance_id: Option<String>,
    event: String,
    args: Vec<Value>,
}

impl ModuleTransportEvent {
    pub fn new(
        module: impl Into<String>,
        event: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<Self> {
        Self::new_with_instance(module, None, event, args)
    }

    /// Creates an event emitted by one explicitly named module instance.
    pub fn new_instance(
        module: impl Into<String>,
        instance_id: impl Into<String>,
        event: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<Self> {
        Self::new_with_instance(module, Some(instance_id.into()), event, args)
    }

    fn new_with_instance(
        module: impl Into<String>,
        instance_id: Option<String>,
        event: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<Self> {
        let module = module.into();
        let event = event.into();
        if module.trim().is_empty() {
            bail!("module event module name is required");
        }
        if instance_id
            .as_deref()
            .is_some_and(|instance_id| instance_id.trim().is_empty())
        {
            bail!("module event instance id is required");
        }
        if event.trim().is_empty() {
            bail!("module event name is required");
        }
        Ok(Self {
            module,
            instance_id,
            event,
            args,
        })
    }

    #[must_use]
    pub fn module(&self) -> &str {
        &self.module
    }

    #[must_use]
    pub fn instance_id(&self) -> Option<&str> {
        self.instance_id.as_deref()
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
    json_output_limit: usize,
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
            json_output_limit: LOGOSCORE_JSON_OUTPUT_LIMIT,
        }
    }

    #[must_use]
    pub(crate) fn with_blocking_work_tracker(mut self, tracker: BlockingWorkTracker) -> Self {
        self.blocking_work = tracker;
        self
    }

    pub(crate) fn with_json_output_limit(mut self, json_output_limit: usize) -> Result<Self> {
        ensure!(
            json_output_limit > 0 && json_output_limit <= LOGOSCORE_MAX_JSON_OUTPUT_LIMIT,
            "LogosCore CLI JSON output limit must be between 1 and {LOGOSCORE_MAX_JSON_OUTPUT_LIMIT} bytes"
        );
        self.json_output_limit = json_output_limit;
        Ok(self)
    }

    pub(crate) fn blocking_worker_guard(&self) -> Result<BlockingWorkGuard> {
        self.blocking_work.worker_guard()
    }

    #[must_use]
    pub(crate) fn command_control(&self) -> CommandControl {
        CommandControl::new(self.cancellation.clone(), self.deadline.into_std())
            .with_blocking_work_tracker(self.blocking_work.clone())
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
    pub(crate) const fn json_output_limit(&self) -> usize {
        self.json_output_limit
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

    /// Requests that transport-owned local work stop during host shutdown.
    ///
    /// Transports without cancellable local work retain the no-op default.
    fn begin_close(&self) {}

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

    fn subscribe_module_instance_event(
        &self,
        module: &str,
        instance_id: &str,
        event: &str,
    ) -> ModuleTransportResult<BoxedModuleEventSubscription> {
        if instance_id.trim().is_empty() {
            return self.subscribe_module_event(module, event);
        }
        bail!("scoped module event subscriptions are unavailable through this adapter")
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

/// Binds one transport to exactly one explicitly named module instance.
///
/// The wrapper exists for Basecamp modules that are independently configured
/// by the Inspector. It rejects a different module or instance instead of
/// allowing a caller to fall back to a transport default instance.
#[derive(Clone)]
pub(crate) struct ScopedModuleTransport {
    inner: SharedModuleTransport,
    module: String,
    instance_id: String,
}

impl ScopedModuleTransport {
    pub(crate) fn new(
        inner: SharedModuleTransport,
        module: impl Into<String>,
        instance_id: impl Into<String>,
    ) -> Result<Self> {
        let module = module.into();
        let instance_id = instance_id.into();
        ensure!(!module.trim().is_empty(), "scoped module name is required");
        ensure!(
            !instance_id.trim().is_empty(),
            "scoped module instance id is required"
        );
        Ok(Self {
            inner,
            module,
            instance_id,
        })
    }

    fn require_module(&self, module: &str) -> Result<()> {
        ensure!(
            module == self.module,
            "scoped module transport is bound to `{}`, not `{module}`",
            self.module
        );
        Ok(())
    }

    fn scoped_call(&self, call: ModuleCall) -> Result<ModuleCall> {
        self.require_module(call.module())?;
        if let Some(instance_id) = call.instance_id() {
            ensure!(
                instance_id == self.instance_id,
                "scoped module transport is bound to instance `{}`, not `{instance_id}`",
                self.instance_id
            );
        }
        ModuleCall::new_instance(
            call.transport(),
            self.module.clone(),
            self.instance_id.clone(),
            call.method().to_owned(),
            call.args().to_vec(),
        )
    }
}

impl ModuleTransport for ScopedModuleTransport {
    fn kind(&self) -> ModuleTransportKind {
        self.inner.kind()
    }

    fn begin_close(&self) {
        self.inner.begin_close();
    }

    fn logoscore_cli_transport(&self) -> Option<&LogoscoreCliTransport> {
        self.inner.logoscore_cli_transport()
    }

    fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
        let inner = Arc::clone(&self.inner);
        let call = self.scoped_call(call);
        Box::pin(async move {
            let call = call?;
            inner.call(call).await
        })
    }

    fn call_controlled(
        &self,
        call: ModuleCall,
        control: ModuleCallControl,
    ) -> ModuleCallFuture<'_> {
        let inner = Arc::clone(&self.inner);
        let call = self.scoped_call(call);
        Box::pin(async move {
            let call = call?;
            inner.call_controlled(call, control).await
        })
    }

    fn subscribe_module_event(
        &self,
        module: &str,
        event: &str,
    ) -> ModuleTransportResult<BoxedModuleEventSubscription> {
        self.require_module(module)?;
        self.inner
            .subscribe_module_instance_event(&self.module, &self.instance_id, event)
    }

    fn subscribe_module_instance_event(
        &self,
        module: &str,
        instance_id: &str,
        event: &str,
    ) -> ModuleTransportResult<BoxedModuleEventSubscription> {
        self.require_module(module)?;
        ensure!(
            instance_id == self.instance_id,
            "scoped module transport is bound to instance `{}`, not `{instance_id}`",
            self.instance_id
        );
        self.inner
            .subscribe_module_instance_event(&self.module, &self.instance_id, event)
    }

    fn ingest_module_event(
        &self,
        _module: &str,
        _event: &str,
        _args: &[Value],
    ) -> ModuleTransportResult<()> {
        bail!(
            "scoped module event ingress must use the owning host transport with an explicit instance id"
        )
    }

    fn supports_shared_file_staging(&self) -> bool {
        self.inner.supports_shared_file_staging()
    }

    fn native_runtime_module_events_ready(&self) -> bool {
        self.inner.native_runtime_module_events_ready()
    }

    fn status(&self) -> ModuleDiagnosticFuture<'_> {
        self.inner.status()
    }

    fn module_info(&self, module: String) -> ModuleDiagnosticFuture<'_> {
        let required = self.require_module(&module);
        Box::pin(async move {
            required?;
            bail!(
                "scoped module metadata is unavailable; query the explicit Basecamp module instance interface"
            )
        })
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

pub(crate) async fn dispatch_module_call_controlled(
    transport: &dyn ModuleTransport,
    call: ModuleCall,
    control: ModuleCallControl,
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
    let reply = transport.call_controlled(call, control).await?;
    if reply.transport() != actual {
        bail!(
            "module transport `{}` returned reply identity `{}`",
            actual.as_str(),
            reply.transport().as_str()
        );
    }
    Ok(reply)
}

type LogoscoreRuntimeResolver =
    Arc<dyn Fn() -> Result<Option<LogoscoreCliRuntime>> + Send + Sync + 'static>;

#[derive(Clone)]
enum LogoscoreRuntimeBinding {
    Fixed(LogoscoreCliRuntime),
    ConfiguredWithFallback(LogoscoreRuntimeResolver),
}

impl std::fmt::Debug for LogoscoreRuntimeBinding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fixed(_) => formatter.write_str("Fixed"),
            Self::ConfiguredWithFallback(_) => formatter.write_str("ConfiguredWithFallback"),
        }
    }
}

impl LogoscoreRuntimeBinding {
    fn resolve(&self) -> Result<LogoscoreCliRuntime> {
        let explicitly_configured = LogoscoreCliTransport::configured_runtime_from_environment();
        self.resolve_with_explicit(explicitly_configured)
    }

    fn resolve_with_explicit(
        &self,
        explicitly_configured: Option<LogoscoreCliRuntime>,
    ) -> Result<LogoscoreCliRuntime> {
        match self {
            Self::Fixed(runtime) => Ok(runtime.clone()),
            Self::ConfiguredWithFallback(resolver) => {
                if let Some(runtime) = explicitly_configured {
                    return Ok(runtime);
                }
                Ok(resolver()?.unwrap_or_else(configured_runtime))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogoscoreCliTransport {
    runtime: LogoscoreRuntimeBinding,
    close_cancellation: CancellationToken,
}

impl Default for LogoscoreCliTransport {
    fn default() -> Self {
        let runtime = if let Some(runtime) = Self::configured_runtime_from_environment() {
            LogoscoreRuntimeBinding::Fixed(runtime)
        } else {
            LogoscoreRuntimeBinding::ConfiguredWithFallback(Arc::new(
                crate::local_nodes::running_local_logoscore_runtime,
            ))
        };
        Self {
            runtime,
            close_cancellation: CancellationToken::new(),
        }
    }
}

impl LogoscoreCliTransport {
    #[must_use]
    pub(crate) fn configured_runtime_from_environment() -> Option<LogoscoreCliRuntime> {
        logoscore_environment_is_configured().then(configured_runtime)
    }

    #[must_use]
    pub(crate) fn fixed_runtime(runtime: LogoscoreCliRuntime) -> Self {
        Self {
            runtime: LogoscoreRuntimeBinding::Fixed(runtime),
            close_cancellation: CancellationToken::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn managed(binary_path: String, config_dir: String) -> Self {
        Self::fixed_runtime(LogoscoreCliRuntime::managed(binary_path, config_dir))
    }

    pub(crate) fn runtime(&self) -> Result<LogoscoreCliRuntime> {
        self.runtime.resolve()
    }

    fn pinned(&self) -> Result<Self> {
        Ok(Self {
            runtime: LogoscoreRuntimeBinding::Fixed(self.runtime.resolve()?),
            close_cancellation: self.close_cancellation.clone(),
        })
    }
}

/// Freezes dynamic LogosCore selection for one Inspector request so a
/// multi-call report cannot migrate to a runtime that started mid-request.
pub(crate) fn pin_module_transport(
    module_transport: SharedModuleTransport,
) -> Result<SharedModuleTransport> {
    if module_transport.kind() != ModuleTransportKind::LogoscoreCli {
        return Ok(module_transport);
    }
    let pinned = module_transport
        .logoscore_cli_transport()
        .map(LogoscoreCliTransport::pinned)
        .transpose()?;
    match pinned {
        Some(transport) => Ok(Arc::new(transport)),
        None => Ok(module_transport),
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
        let close_cancellation = self.close_cancellation.clone();
        Box::pin(async move {
            let runtime = runtime.resolve()?;
            let transport = call.transport();
            if transport != ModuleTransportKind::LogoscoreCli {
                bail!(
                    "LogosCore CLI transport cannot execute `{}` calls",
                    transport.as_str()
                );
            }
            if call.instance_id().is_some() {
                bail!("LogosCore CLI transport cannot execute scoped module calls");
            }
            let module = call.module().to_owned();
            let method = call.method().to_owned();
            let args = call.args().to_vec();
            let module_label = module.clone();
            let method_label = method.clone();
            let deadline = StdInstant::now()
                .checked_add(compound_command_timeout())
                .context("LogosCore CLI call deadline overflowed")?;
            let command_control = CommandControl::new(close_cancellation, deadline);
            let output = tokio::task::spawn_blocking(move || {
                runtime.call_typed_controlled_with_output_limit(
                    &module,
                    &method,
                    &args,
                    command_control,
                    LOGOSCORE_JSON_OUTPUT_LIMIT,
                )
            })
            .await
            .context("LogosCore CLI module-call worker failed")??;
            let value = normalize_module_call_value(&module_label, &method_label, output.value)?;
            Ok(ModuleCallReply::new(
                ModuleTransportKind::LogoscoreCli,
                value,
            ))
        })
    }

    fn begin_close(&self) {
        self.close_cancellation.cancel();
    }

    fn call_controlled(
        &self,
        call: ModuleCall,
        control: ModuleCallControl,
    ) -> ModuleCallFuture<'_> {
        let runtime = self.runtime.clone();
        Box::pin(async move {
            let runtime = runtime.resolve()?;
            let transport = call.transport();
            if transport != ModuleTransportKind::LogoscoreCli {
                bail!(
                    "LogosCore CLI transport cannot execute `{}` calls",
                    transport.as_str()
                );
            }
            if call.instance_id().is_some() {
                bail!("LogosCore CLI transport cannot execute scoped module calls");
            }
            let module = call.module().to_owned();
            let method = call.method().to_owned();
            let args = call.args().to_vec();
            let module_label = module.clone();
            let method_label = method.clone();
            let json_output_limit = control.json_output_limit();
            let command_control = CommandControl::new(
                control.cancellation().clone(),
                control.deadline().into_std(),
            );
            control.check_active()?;
            let worker_guard = control.blocking_worker_guard()?;
            let output = tokio::task::spawn_blocking(move || {
                let _worker_guard = worker_guard;
                runtime.call_typed_controlled_with_output_limit(
                    &module,
                    &method,
                    &args,
                    command_control,
                    json_output_limit,
                )
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
        let close_cancellation = self.close_cancellation.clone();
        Box::pin(async move {
            let runtime = runtime.resolve()?;
            let deadline = StdInstant::now()
                .checked_add(command_timeout())
                .context("LogosCore CLI status deadline overflowed")?;
            let command_control = CommandControl::new(close_cancellation, deadline);
            let output =
                tokio::task::spawn_blocking(move || runtime.status_controlled(command_control))
                    .await
                    .context("LogosCore CLI status worker failed")??;
            serde_json::to_value(output).context("failed to serialize LogosCore CLI status")
        })
    }

    fn module_info(&self, module: String) -> ModuleDiagnosticFuture<'_> {
        let runtime = self.runtime.clone();
        let close_cancellation = self.close_cancellation.clone();
        Box::pin(async move {
            let runtime = runtime.resolve()?;
            let module_label = module.clone();
            let deadline = StdInstant::now()
                .checked_add(compound_command_timeout())
                .context("LogosCore CLI module-info deadline overflowed")?;
            let command_control = CommandControl::new(close_cancellation, deadline);
            let output = tokio::task::spawn_blocking(move || {
                runtime.module_info_controlled(&module, command_control)
            })
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

    pub(crate) fn copy_to_new(&self, target: &Path) -> Result<u64> {
        let metadata = fs::symlink_metadata(&self.path).with_context(|| {
            format!(
                "failed to inspect logoscore download staging file `{}`",
                self.path.display()
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            bail!("logoscore download staging path is not a regular file");
        }
        let parent = target.parent().with_context(|| {
            format!(
                "storage download target has no parent: `{}`",
                target.display()
            )
        })?;
        anyhow::ensure!(
            fs::metadata(parent)
                .with_context(|| {
                    format!(
                        "failed to inspect storage download target directory `{}`",
                        parent.display()
                    )
                })?
                .is_dir(),
            "storage download target parent is not a directory: `{}`",
            parent.display()
        );
        anyhow::ensure!(
            !target.exists(),
            "storage download target already exists: `{}`",
            target.display()
        );

        let mut source = fs::File::open(&self.path).with_context(|| {
            format!(
                "failed to open logoscore download staging file `{}`",
                self.path.display()
            )
        })?;
        let mut pending = NamedTempFile::new_in(parent).with_context(|| {
            format!(
                "failed to create storage download commit file in `{}`",
                parent.display()
            )
        })?;
        let bytes = std::io::copy(&mut source, pending.as_file_mut()).with_context(|| {
            format!(
                "failed to copy logoscore download into `{}`",
                parent.display()
            )
        })?;
        anyhow::ensure!(
            bytes == metadata.len(),
            "logoscore download staging file changed while it was copied"
        );
        pending
            .as_file_mut()
            .flush()
            .context("failed to flush storage download commit file")?;
        pending
            .as_file()
            .sync_all()
            .context("failed to sync storage download commit file")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            pending
                .as_file()
                .set_permissions(fs::Permissions::from_mode(0o640))
                .context("failed to secure storage download commit file")?;
        }
        pending.persist_noclobber(target).map_err(|error| {
            anyhow::anyhow!(
                "failed to commit storage download to `{}`: {}",
                target.display(),
                error.error
            )
        })?;
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum WatchCleanupAuthority {
    Direct,
    #[cfg(target_os = "linux")]
    ServiceIdentity {
        user: String,
    },
}

impl WatchCleanupAuthority {
    fn for_runner(runner: &LogosCoreRunner) -> Self {
        #[cfg(target_os = "linux")]
        if let Some(user) = &runner.sudo_user {
            return Self::ServiceIdentity { user: user.clone() };
        }
        #[cfg(not(target_os = "linux"))]
        let _ = runner;
        Self::Direct
    }
}

/// Per-launch ownership for persistent LogosCore event-watch leases.
///
/// A lease is recorded before a watch is spawned, so a later Inspector launch
/// can distinguish an abandoned watch from one belonging to a live process.
#[derive(Debug, Clone)]
pub(crate) struct LogoscoreWatchOwner {
    #[cfg(target_os = "linux")]
    lease_directory: PathBuf,
    #[cfg(target_os = "linux")]
    process: u32,
    #[cfg(target_os = "linux")]
    process_start_marker: u64,
    #[cfg(target_os = "linux")]
    launch_nonce: String,
}

impl LogoscoreWatchOwner {
    pub(crate) fn start() -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            let lease_directory = watch_lease_directory()?;
            recover_abandoned_watch_leases(&lease_directory)?;
            let process = std::process::id();
            let process_i32 = i32::try_from(process)
                .context("standalone watcher owner PID does not fit Linux process identity")?;
            let process_start_marker = linux_process_start_marker(process_i32)?
                .context("standalone watcher owner process is not live")?;
            Ok(Self {
                lease_directory,
                process,
                process_start_marker,
                launch_nonce: new_logoscore_watch_launch_nonce()?,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            Ok(Self {})
        }
    }

    fn register(
        &self,
        token: &str,
        authority: &WatchCleanupAuthority,
    ) -> Result<Option<LogoscoreWatchLease>> {
        #[cfg(target_os = "linux")]
        {
            let record = WatchLeaseRecord {
                schema_version: LOGOSCORE_WATCH_LEASE_SCHEMA_VERSION,
                token: token.to_owned(),
                owner_pid: self.process,
                owner_start_marker: self.process_start_marker,
                launch_nonce: self.launch_nonce.clone(),
                cleanup_user: match authority {
                    WatchCleanupAuthority::Direct => None,
                    WatchCleanupAuthority::ServiceIdentity { user } => Some(user.clone()),
                },
            };
            let path = write_watch_lease_record(&self.lease_directory, &record)?;
            Ok(Some(LogoscoreWatchLease { path, record }))
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (token, authority);
            Ok(None)
        }
    }
}

#[derive(Debug)]
struct LogoscoreWatchLease {
    #[cfg(target_os = "linux")]
    path: PathBuf,
    #[cfg(target_os = "linux")]
    record: WatchLeaseRecord,
}

impl LogoscoreWatchLease {
    fn owner_pid(&self) -> u32 {
        #[cfg(target_os = "linux")]
        {
            self.record.owner_pid
        }
        #[cfg(not(target_os = "linux"))]
        {
            0
        }
    }

    fn owner_start_marker(&self) -> u64 {
        #[cfg(target_os = "linux")]
        {
            self.record.owner_start_marker
        }
        #[cfg(not(target_os = "linux"))]
        {
            0
        }
    }

    fn launch_nonce(&self) -> &str {
        #[cfg(target_os = "linux")]
        {
            &self.record.launch_nonce
        }
        #[cfg(not(target_os = "linux"))]
        {
            ""
        }
    }

    fn release(&self) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            match fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error).with_context(|| {
                    format!(
                        "failed to remove LogosCore watch lease `{}`",
                        self.path.display()
                    )
                }),
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            Ok(())
        }
    }
}

fn release_watch_lease(lease: &mut Option<LogoscoreWatchLease>) -> Result<()> {
    if let Some(current) = lease.as_ref() {
        current.release()?;
    }
    *lease = None;
    Ok(())
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WatchLeaseRecord {
    schema_version: u8,
    token: String,
    owner_pid: u32,
    owner_start_marker: u64,
    launch_nonce: String,
    cleanup_user: Option<String>,
}

#[cfg(target_os = "linux")]
fn watch_lease_directory() -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt as _;

    let directory =
        crate::support::config_path::config_dir()?.join(LOGOSCORE_WATCH_LEASE_DIRECTORY);
    fs::create_dir_all(&directory).with_context(|| {
        format!(
            "failed to create LogosCore watch lease directory `{}`",
            directory.display()
        )
    })?;
    let metadata = fs::symlink_metadata(&directory).with_context(|| {
        format!(
            "failed to inspect LogosCore watch lease directory `{}`",
            directory.display()
        )
    })?;
    anyhow::ensure!(
        metadata.file_type().is_dir() && !metadata.file_type().is_symlink(),
        "LogosCore watch lease path is not a directory: `{}`",
        directory.display()
    );
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).with_context(|| {
        format!(
            "failed to secure LogosCore watch lease directory `{}`",
            directory.display()
        )
    })?;
    Ok(directory)
}

#[cfg(target_os = "linux")]
fn watch_lease_path(directory: &Path, token: &str) -> Result<PathBuf> {
    anyhow::ensure!(
        !token.is_empty()
            && token
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'),
        "LogosCore watch lease token is invalid"
    );
    Ok(directory.join(format!("{token}.json")))
}

#[cfg(target_os = "linux")]
fn write_watch_lease_record(directory: &Path, record: &WatchLeaseRecord) -> Result<PathBuf> {
    use std::os::unix::fs::OpenOptionsExt as _;

    let path = watch_lease_path(directory, &record.token)?;
    let bytes = serde_json::to_vec(record).context("failed to encode LogosCore watch lease")?;
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&path)
        .with_context(|| {
            format!(
                "failed to create LogosCore watch lease `{}`",
                path.display()
            )
        })?;
    if let Err(error) = file.write_all(&bytes).and_then(|()| file.write_all(b"\n")) {
        let _remove_result = fs::remove_file(&path);
        return Err(error).with_context(|| {
            format!("failed to write LogosCore watch lease `{}`", path.display())
        });
    }
    if let Err(error) = file.sync_all() {
        let _remove_result = fs::remove_file(&path);
        return Err(error).with_context(|| {
            format!(
                "failed to persist LogosCore watch lease `{}`",
                path.display()
            )
        });
    }
    Ok(path)
}

#[cfg(target_os = "linux")]
fn recover_abandoned_watch_leases(directory: &Path) -> Result<()> {
    for entry in fs::read_dir(directory).with_context(|| {
        format!(
            "failed to inspect LogosCore watch lease directory `{}`",
            directory.display()
        )
    })? {
        let entry = entry.context("failed to read LogosCore watch lease entry")?;
        let file_type = entry
            .file_type()
            .context("failed to inspect LogosCore watch lease entry type")?;
        if !file_type.is_file() || file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        let record = match fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<WatchLeaseRecord>(&bytes).ok())
        {
            Some(record) => record,
            None => continue,
        };
        if record.schema_version != LOGOSCORE_WATCH_LEASE_SCHEMA_VERSION
            || record.launch_nonce.is_empty()
            || watch_lease_path(directory, &record.token).ok().as_deref() != Some(path.as_path())
        {
            continue;
        }
        let owner_pid = match i32::try_from(record.owner_pid) {
            Ok(owner_pid) if owner_pid > 0 => owner_pid,
            _ => continue,
        };
        if linux_process_start_marker(owner_pid)? == Some(record.owner_start_marker) {
            continue;
        }
        let authority = match record.cleanup_user.as_deref() {
            Some(user) if !user.trim().is_empty() => WatchCleanupAuthority::ServiceIdentity {
                user: user.to_owned(),
            },
            _ => WatchCleanupAuthority::Direct,
        };
        stop_tagged_watch_processes(&record.token, &authority, "abandoned logoscore watch")
            .with_context(|| {
                format!(
                    "failed to reap abandoned LogosCore watch lease `{}`",
                    path.display()
                )
            })?;
        fs::remove_file(&path).with_context(|| {
            format!(
                "failed to remove recovered LogosCore watch lease `{}`",
                path.display()
            )
        })?;
    }
    Ok(())
}

#[derive(Debug)]
struct WatchCleanup {
    token: String,
    authority: WatchCleanupAuthority,
    lease: Option<LogoscoreWatchLease>,
}

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
    cleanup_token: String,
    cleanup_authority: WatchCleanupAuthority,
    lease: Option<LogoscoreWatchLease>,
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
            Some(child) => stop_watch_child_with_retry(
                child,
                &self.cleanup_token,
                &self.cleanup_authority,
                &self.label,
            ),
            None => Ok(()),
        };
        child_result?;
        self.child = None;
        release_watch_lease(&mut self.lease)?;
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
            cleanup_token: self.cleanup_token.clone(),
            cleanup_authority: self.cleanup_authority.clone(),
            lease: self.lease.take(),
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
    cleanup_token: String,
    cleanup_authority: WatchCleanupAuthority,
    lease: Option<LogoscoreWatchLease>,
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
        stop_watch_child_with_retry(
            &mut recovery.child,
            &recovery.cleanup_token,
            &recovery.cleanup_authority,
            &recovery.label,
        )
        .is_ok()
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
    while stop_watch_child_with_retry(
        &mut recovery.child,
        &recovery.cleanup_token,
        &recovery.cleanup_authority,
        &recovery.label,
    )
    .is_err()
    {
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
    let _lease_release = release_watch_lease(&mut recovery.lease);
}

impl LogoscoreCliRuntime {
    #[must_use]
    pub(crate) fn local(binary_path: String, config_dir: String) -> Self {
        Self {
            runner: LogosCoreRunner {
                program: binary_path,
                sudo_user: None,
                home: None,
                config_dir: Some(config_dir),
                label: "local LogosCore".to_owned(),
            },
        }
    }

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
        self.cached_status(command_timeout())
    }

    pub(crate) fn status_with_timeout(&self, timeout: Duration) -> Result<LogosCoreOutput> {
        self.cached_status(timeout)
    }

    pub(crate) fn status_probe_with_timeout(&self, timeout: Duration) -> Result<LogosCoreOutput> {
        self.run_status_json(timeout)
    }

    pub(crate) fn status_controlled(&self, control: CommandControl) -> Result<LogosCoreOutput> {
        self.cached_status_controlled(control)
    }

    fn cached_status(&self, timeout: Duration) -> Result<LogosCoreOutput> {
        if let Some(snapshot) =
            fresh_logoscore_cli_snapshot(&self.runner, LogoscoreCliSnapshotKind::Status)?
        {
            return Ok(snapshot);
        }
        self.with_command_gate(timeout, move |runner, deadline| {
            cached_logoscore_cli_snapshot(runner, LogoscoreCliSnapshotKind::Status, || {
                run_status_json_before_deadline(runner, deadline)
            })
        })
    }

    fn cached_status_controlled(&self, control: CommandControl) -> Result<LogosCoreOutput> {
        let gate_control = control.clone();
        self.with_controlled_command_gate(&gate_control, move |runner| {
            cached_logoscore_cli_snapshot(runner, LogoscoreCliSnapshotKind::Status, || {
                run_status_json_with_controlled(runner, control)
            })
        })
    }

    fn run_status_json(&self, timeout: Duration) -> Result<LogosCoreOutput> {
        self.with_command_gate(timeout, move |runner, deadline| {
            run_status_json_before_deadline(runner, deadline)
        })
    }

    pub(crate) fn list_modules(&self) -> Result<LogosCoreOutput> {
        self.cached_json(
            LogoscoreCliSnapshotKind::Modules,
            ["list-modules", "--json"],
            command_timeout(),
        )
    }

    pub(crate) fn list_modules_controlled(
        &self,
        control: CommandControl,
    ) -> Result<LogosCoreOutput> {
        self.cached_json_controlled(
            LogoscoreCliSnapshotKind::Modules,
            ["list-modules", "--json"],
            control,
        )
    }

    pub(crate) fn module_info(&self, module: &str) -> Result<LogosCoreOutput> {
        if module.trim().is_empty() {
            bail!("module name is required");
        }
        self.with_command_gate(compound_command_timeout(), |runner, deadline| {
            let modules =
                cached_logoscore_cli_snapshot(runner, LogoscoreCliSnapshotKind::Modules, || {
                    run_json_before_deadline(runner, ["list-modules", "--json"], deadline)
                })
                .context("failed to list logoscore modules")?;
            require_listed_module_loaded(module, &modules.value)?;
            run_json_before_deadline(runner, ["module-info", module, "--json"], deadline)
        })
    }

    pub(crate) fn module_info_controlled(
        &self,
        module: &str,
        control: CommandControl,
    ) -> Result<LogosCoreOutput> {
        if module.trim().is_empty() {
            bail!("module name is required");
        }
        let gate_control = control.clone();
        self.with_controlled_command_gate(&gate_control, |runner| {
            let modules =
                cached_logoscore_cli_snapshot(runner, LogoscoreCliSnapshotKind::Modules, || {
                    run_json_with_controlled(runner, ["list-modules", "--json"], control.clone())
                })
                .context("failed to list logoscore modules")?;
            require_listed_module_loaded(module, &modules.value)?;
            run_json_with_controlled(runner, ["module-info", module, "--json"], control)
        })
    }

    pub(crate) fn require_module_method(
        &self,
        module: &str,
        method: &str,
        signature: &str,
    ) -> Result<()> {
        self.discover_module(module)?
            .require_method(method, signature)
    }

    fn discover_module(&self, module: &str) -> Result<LogoscoreModuleDiscovery> {
        self.with_command_gate(compound_command_timeout(), |runner, deadline| {
            let modules =
                cached_logoscore_cli_snapshot(runner, LogoscoreCliSnapshotKind::Modules, || {
                    run_json_before_deadline(runner, ["list-modules", "--json"], deadline)
                })
                .context("failed to list logoscore modules")?;
            require_listed_module_loaded(module, &modules.value)?;
            let module_info =
                run_json_before_deadline(runner, ["module-info", module, "--json"], deadline)
                    .with_context(|| format!("failed to inspect logoscore module `{module}`"))?;
            module_discovery(module, &modules.value, &module_info.value)
        })
    }

    pub(crate) fn require_module_method_controlled(
        &self,
        module: &str,
        method: &str,
        signature: &str,
        control: CommandControl,
    ) -> Result<()> {
        self.discover_module_controlled(module, control)?
            .require_method(method, signature)
    }

    pub(crate) fn require_module_method_controlled_once(
        &self,
        module: &str,
        method: &str,
        signature: &str,
        control: CommandControl,
    ) -> Result<()> {
        self.discover_module_controlled_once(module, control)?
            .require_method(method, signature)
    }

    pub(crate) fn require_module_contract_controlled(
        &self,
        module: &str,
        methods: &[(&str, &str)],
        events: &[(&str, &str)],
        control: CommandControl,
    ) -> Result<()> {
        let discovery = self.discover_module_controlled(module, control)?;
        for (method, signature) in methods {
            discovery.require_method(method, signature)?;
        }
        for (event, signature) in events {
            discovery.require_event(event, signature)?;
        }
        Ok(())
    }

    pub(crate) fn require_module_contract_with_method_signatures_controlled(
        &self,
        module: &str,
        methods: &[(&str, &[&str])],
        events: &[(&str, &str)],
        control: CommandControl,
    ) -> Result<()> {
        let discovery = self.discover_module_controlled(module, control)?;
        for (method, signatures) in methods {
            discovery.require_method_with_signatures(method, signatures)?;
        }
        for (event, signature) in events {
            discovery.require_event(event, signature)?;
        }
        Ok(())
    }

    fn discover_module_controlled(
        &self,
        module: &str,
        control: CommandControl,
    ) -> Result<LogoscoreModuleDiscovery> {
        self.discover_module_controlled_with(
            module,
            control,
            LOGOSCORE_MODULE_DISCOVERY_ATTEMPT_TIMEOUT,
            LOGOSCORE_MODULE_DISCOVERY_RETRY_DELAY,
        )
    }

    fn discover_module_controlled_once(
        &self,
        module: &str,
        control: CommandControl,
    ) -> Result<LogoscoreModuleDiscovery> {
        self.discover_module_controlled_with_attempts(
            module,
            control,
            LOGOSCORE_MODULE_DISCOVERY_ATTEMPT_TIMEOUT,
            Duration::ZERO,
            1,
        )
    }

    fn discover_module_controlled_with(
        &self,
        module: &str,
        control: CommandControl,
        attempt_timeout: Duration,
        retry_delay: Duration,
    ) -> Result<LogoscoreModuleDiscovery> {
        self.discover_module_controlled_with_attempts(
            module,
            control,
            attempt_timeout,
            retry_delay,
            LOGOSCORE_MODULE_DISCOVERY_ATTEMPTS,
        )
    }

    fn discover_module_controlled_with_attempts(
        &self,
        module: &str,
        control: CommandControl,
        attempt_timeout: Duration,
        retry_delay: Duration,
        attempts: usize,
    ) -> Result<LogoscoreModuleDiscovery> {
        if attempts == 0 {
            bail!("logoscore module discovery requires at least one attempt");
        }
        for attempt in 0..attempts {
            control.check_active()?;
            let attempt_deadline = StdInstant::now()
                .checked_add(attempt_timeout)
                .unwrap_or(control.deadline());
            let attempt_control = control.with_deadline(attempt_deadline);
            let result = self.with_controlled_command_gate(&attempt_control, |runner| {
                let modules = cached_logoscore_cli_snapshot(
                    runner,
                    LogoscoreCliSnapshotKind::Modules,
                    || {
                        run_json_with_controlled(
                            runner,
                            ["list-modules", "--json"],
                            attempt_control.clone(),
                        )
                    },
                )
                .context("failed to list logoscore modules")?;
                require_listed_module_loaded(module, &modules.value)?;
                let module_info = run_json_with_controlled(
                    runner,
                    ["module-info", module, "--json"],
                    attempt_control.clone(),
                )
                .with_context(|| format!("failed to inspect logoscore module `{module}`"))?;
                module_discovery(module, &modules.value, &module_info.value)
            });
            match result {
                Ok(discovery) => return Ok(discovery),
                Err(error)
                    if attempt + 1 < attempts && is_transient_module_discovery_error(&error) =>
                {
                    control.check_active()?;
                    thread::sleep(retry_delay);
                }
                Err(error)
                    if is_module_discovery_attempt_timeout(&error, &control, attempt_deadline) =>
                {
                    control.check_active()?;
                    bail!(
                        "logoscore module `{module}` discovery attempt {}/{} exceeded its bounded deadline: {error:#}",
                        attempt + 1,
                        attempts,
                    );
                }
                Err(error) => return Err(error),
            }
        }
        bail!("logoscore module `{module}` discovery completed without an attempt result")
    }

    pub(crate) fn ensure_module_loaded(&self, module: &str) -> Result<()> {
        let modules = self
            .list_modules()
            .context("failed to list logoscore modules")?;
        if listed_module_status(module, &modules.value)? == "loaded" {
            return Ok(());
        }

        let result = self
            .run_json(["load-module", module, "--json"], command_timeout())
            .with_context(|| format!("failed to load logoscore module `{module}`"));
        self.invalidate_cli_snapshot()?;
        result?;
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
        if listed_module_status(module, &modules.value)? == "loaded" {
            return Ok(());
        }

        let result = self
            .run_json_controlled(["load-module", module, "--json"], control)
            .with_context(|| format!("failed to load logoscore module `{module}`"));
        self.invalidate_cli_snapshot()?;
        result?;
        Ok(())
    }

    pub(crate) fn unload_module(&self, module: &str) -> Result<LogosCoreOutput> {
        if module.trim().is_empty() {
            bail!("module name is required");
        }
        let result = self.run_json(["unload-module", module, "--json"], command_timeout());
        self.invalidate_cli_snapshot()?;
        result
    }

    pub(crate) fn unload_module_controlled(
        &self,
        module: &str,
        control: CommandControl,
    ) -> Result<LogosCoreOutput> {
        if module.trim().is_empty() {
            bail!("module name is required");
        }
        let result = self.run_json_controlled(["unload-module", module, "--json"], control);
        self.invalidate_cli_snapshot()?;
        result
    }

    pub(crate) fn call(
        &self,
        module: &str,
        method: &str,
        args: &[String],
    ) -> Result<LogosCoreOutput> {
        let command_args = call_arguments(module, method, args)?;
        self.call_with_arguments(module, command_args)
    }

    fn call_with_arguments(
        &self,
        module: &str,
        command_args: Vec<String>,
    ) -> Result<LogosCoreOutput> {
        let mut output =
            self.with_command_gate(compound_command_timeout(), |runner, deadline| {
                let modules = cached_logoscore_cli_snapshot(
                    runner,
                    LogoscoreCliSnapshotKind::Modules,
                    || run_json_before_deadline(runner, ["list-modules", "--json"], deadline),
                )
                .context("failed to list logoscore modules")?;
                require_listed_module_loaded(module, &modules.value)?;
                run_json_before_deadline(runner, command_args, deadline)
            })?;
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
        self.call_with_arguments_controlled(module, command_args, control)
    }

    pub(crate) fn call_typed_controlled_with_output_limit(
        &self,
        module: &str,
        method: &str,
        args: &[Value],
        control: CommandControl,
        json_output_limit: usize,
    ) -> Result<LogosCoreOutput> {
        validate_json_output_limit(json_output_limit)?;
        let command_args = typed_call_arguments(module, method, args)?;
        self.call_with_arguments_controlled_with_output_limit(
            module,
            command_args,
            control,
            json_output_limit,
        )
    }

    fn call_with_arguments_controlled(
        &self,
        module: &str,
        command_args: Vec<String>,
        control: CommandControl,
    ) -> Result<LogosCoreOutput> {
        self.call_with_arguments_controlled_with_output_limit(
            module,
            command_args,
            control,
            LOGOSCORE_JSON_OUTPUT_LIMIT,
        )
    }

    fn call_with_arguments_controlled_with_output_limit(
        &self,
        module: &str,
        command_args: Vec<String>,
        control: CommandControl,
        json_output_limit: usize,
    ) -> Result<LogosCoreOutput> {
        let gate_control = control.clone();
        let mut output = self.with_controlled_command_gate(&gate_control, |runner| {
            let modules =
                cached_logoscore_cli_snapshot(runner, LogoscoreCliSnapshotKind::Modules, || {
                    run_json_with_controlled(runner, ["list-modules", "--json"], control.clone())
                })
                .context("failed to list logoscore modules")?;
            require_listed_module_loaded(module, &modules.value)?;
            run_json_with_controlled_with_output_limit(
                runner,
                command_args,
                control,
                json_output_limit,
            )
        })?;
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

    pub(crate) fn start_event_watch(
        &self,
        module: &str,
        event: &str,
        control: &CommandControl,
    ) -> Result<LogoscoreEventWatch> {
        if event.trim().is_empty() {
            bail!("module event name is required");
        }
        self.start_event_watch_inner(module, Some(event), control, None)
    }

    pub(crate) fn start_event_watch_for_owner(
        &self,
        module: &str,
        event: &str,
        control: &CommandControl,
        owner: &LogoscoreWatchOwner,
    ) -> Result<LogoscoreEventWatch> {
        if event.trim().is_empty() {
            bail!("module event name is required");
        }
        self.start_event_watch_inner(module, Some(event), control, Some(owner))
    }

    pub(crate) fn start_all_event_watch_for_owner(
        &self,
        module: &str,
        control: &CommandControl,
        owner: &LogoscoreWatchOwner,
    ) -> Result<LogoscoreEventWatch> {
        self.start_event_watch_inner(module, None, control, Some(owner))
    }

    fn start_event_watch_inner(
        &self,
        module: &str,
        event: Option<&str>,
        control: &CommandControl,
        owner: Option<&LogoscoreWatchOwner>,
    ) -> Result<LogoscoreEventWatch> {
        ensure_logoscore_event_watch_supported()?;
        if module.trim().is_empty() {
            bail!("module name is required");
        }
        control.check_active()?;
        let recovery = watch_recovery_sender()?;
        let cleanup_authority = WatchCleanupAuthority::for_runner(&self.runner);
        let event_label = event.unwrap_or("*");
        let label = format!("logoscore watch {module}.{event_label}");
        let process_permit = acquire_streaming_command_permit(&label, control)?;
        // Some launchers re-exec the real CLI in a separate process group.
        // The inherited token lets shutdown find only this watch's escaped child.
        let cleanup_token = new_logoscore_watch_cleanup_token()?;
        let mut watch_lease = owner
            .map(|owner| owner.register(&cleanup_token, &cleanup_authority))
            .transpose()?
            .flatten();
        let owner_pid = watch_lease
            .as_ref()
            .map(|lease| lease.owner_pid().to_string());
        let owner_start_marker = watch_lease
            .as_ref()
            .map(|lease| lease.owner_start_marker().to_string());
        let owner_nonce = watch_lease
            .as_ref()
            .map(|lease| lease.launch_nonce().to_owned());
        let mut watch_environment =
            vec![(LOGOSCORE_WATCH_CLEANUP_TOKEN_ENV, cleanup_token.as_str())];
        if let (Some(owner_pid), Some(owner_start_marker), Some(owner_nonce)) = (
            owner_pid.as_deref(),
            owner_start_marker.as_deref(),
            owner_nonce.as_deref(),
        ) {
            watch_environment.extend([
                (LOGOSCORE_WATCH_OWNER_PID_ENV, owner_pid),
                (LOGOSCORE_WATCH_OWNER_START_ENV, owner_start_marker),
                (LOGOSCORE_WATCH_OWNER_NONCE_ENV, owner_nonce),
            ]);
        }
        let mut command = event.map_or_else(
            || {
                command_for_runner_with_environment(
                    &self.runner,
                    ["watch", module, "--json", "--watch-protocol", "v1"],
                    &watch_environment,
                )
            },
            |event| {
                command_for_runner_with_environment(
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
                    &watch_environment,
                )
            },
        );
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;

            command.process_group(0);
        }
        let mut child =
            match crate::support::command_runner::spawn_command_with_executable_busy_retry(
                &mut command,
                Some(control.deadline()),
                || {
                    control.check_active()?;
                    Ok(())
                },
            )
            .with_context(|| format!("failed to start {label}"))
            {
                Ok(child) => child,
                Err(error) => {
                    let lease_result = release_watch_lease(&mut watch_lease);
                    return match lease_result {
                        Ok(()) => Err(error),
                        Err(lease_error) => Err(error.context(format!(
                            "failed to release rejected LogosCore watch lease: {lease_error:#}"
                        ))),
                    };
                }
            };
        let Some(stdout) = child.stdout.take() else {
            let error = anyhow::anyhow!("{label} did not expose stdout");
            return Err(cleanup_failed_watch_start(
                error,
                FailedWatchStart::new(
                    child,
                    None,
                    None,
                    process_permit,
                    recovery,
                    WatchCleanup {
                        token: cleanup_token,
                        authority: cleanup_authority,
                        lease: watch_lease,
                    },
                    &label,
                ),
            ));
        };
        let Some(stderr) = child.stderr.take() else {
            let error = anyhow::anyhow!("{label} did not expose stderr");
            return Err(cleanup_failed_watch_start(
                error,
                FailedWatchStart::new(
                    child,
                    None,
                    None,
                    process_permit,
                    recovery,
                    WatchCleanup {
                        token: cleanup_token,
                        authority: cleanup_authority,
                        lease: watch_lease,
                    },
                    &label,
                ),
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
                FailedWatchStart::new(
                    child,
                    None,
                    None,
                    process_permit,
                    recovery,
                    WatchCleanup {
                        token: cleanup_token,
                        authority: cleanup_authority,
                        lease: watch_lease,
                    },
                    &label,
                ),
            ));
        }
        let (sender, output) = mpsc::sync_channel(LOGOSCORE_EVENT_QUEUE_CAPACITY);
        let output_failure = Arc::new(Mutex::new(None));
        let (readiness_sender, readiness) = mpsc::channel();
        let reader_stop = Arc::new(AtomicBool::new(false));
        let reader_label = label.clone();
        let expected_module = module.to_owned();
        let expected_event = event.unwrap_or_default().to_owned();
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
                        WatchCleanup {
                            token: cleanup_token,
                            authority: cleanup_authority,
                            lease: watch_lease,
                        },
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
                        WatchCleanup {
                            token: cleanup_token,
                            authority: cleanup_authority,
                            lease: watch_lease,
                        },
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
            cleanup_token,
            cleanup_authority,
            lease: watch_lease,
            label,
        })
    }

    pub(crate) fn stop(&self) -> Result<LogosCoreOutput> {
        let result = self.run_json(["stop", "--json"], command_timeout());
        self.invalidate_cli_snapshot()?;
        result
    }

    pub(crate) fn stop_controlled(&self, control: CommandControl) -> Result<LogosCoreOutput> {
        let result = self.run_json_controlled(["stop", "--json"], control);
        self.invalidate_cli_snapshot()?;
        result
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
        shared_transport.share_directory(directory.path(), 0o750)?;
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
        // Storage V2 creates a sibling `.partial` file before atomically
        // replacing this destination, so the module's shared group needs
        // write access to the workspace itself.
        shared_transport.share_directory(directory.path(), 0o770)?;
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
        self.require_module_contract_with_method_signatures_controlled(
            "storage_module",
            &[
                ("downloadProtocol", &["downloadProtocol()"] as &[&str]),
                (
                    STORAGE_DOWNLOAD_V2_METHOD,
                    &STORAGE_DOWNLOAD_V2_METHOD_SIGNATURES,
                ),
                (
                    "downloadCancelV2",
                    &["downloadCancelV2(QString)"] as &[&str],
                ),
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

    fn cached_json<I, S>(
        &self,
        kind: LogoscoreCliSnapshotKind,
        args: I,
        timeout: Duration,
    ) -> Result<LogosCoreOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        if kind == LogoscoreCliSnapshotKind::Status
            && let Some(snapshot) = fresh_logoscore_cli_snapshot(&self.runner, kind)?
        {
            return Ok(snapshot);
        }
        self.with_command_gate(timeout, move |runner, deadline| {
            cached_logoscore_cli_snapshot(runner, kind, || {
                run_json_before_deadline(runner, args, deadline)
            })
        })
    }

    fn cached_json_controlled<I, S>(
        &self,
        kind: LogoscoreCliSnapshotKind,
        args: I,
        control: CommandControl,
    ) -> Result<LogosCoreOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let gate_control = control.clone();
        self.with_controlled_command_gate(&gate_control, move |runner| {
            cached_logoscore_cli_snapshot(runner, kind, || {
                run_json_with_controlled(runner, args, control)
            })
        })
    }

    fn invalidate_cli_snapshot(&self) -> Result<()> {
        invalidate_logoscore_cli_snapshot(&self.runner)
    }

    pub(crate) fn invalidate_observation_snapshot(&self) -> Result<()> {
        self.invalidate_cli_snapshot()
    }

    fn run_json<I, S>(&self, args: I, timeout: Duration) -> Result<LogosCoreOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.with_command_gate(timeout, move |runner, deadline| {
            run_json_before_deadline(runner, args, deadline)
        })
    }

    fn run_json_controlled<I, S>(&self, args: I, control: CommandControl) -> Result<LogosCoreOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let gate_control = control.clone();
        self.with_controlled_command_gate(&gate_control, move |runner| {
            run_json_with_controlled(runner, args, control)
        })
    }

    fn with_command_gate<T>(
        &self,
        timeout: Duration,
        operation: impl FnOnce(&LogosCoreRunner, StdInstant) -> Result<T>,
    ) -> Result<T> {
        let deadline = StdInstant::now()
            .checked_add(timeout)
            .context("LogosCore CLI command deadline overflowed")?;
        let gate = logoscore_cli_command_gate(&self.runner)?;
        let _permit = acquire_logoscore_cli_command_gate(&gate, None, Some(deadline))?;
        operation(&self.runner, deadline)
    }

    fn with_controlled_command_gate<T>(
        &self,
        control: &CommandControl,
        operation: impl FnOnce(&LogosCoreRunner) -> Result<T>,
    ) -> Result<T> {
        let gate = logoscore_cli_command_gate(&self.runner)?;
        let _permit = acquire_logoscore_cli_command_gate(&gate, Some(control), None)?;
        operation(&self.runner)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogoscoreCliSnapshotKind {
    Status,
    Modules,
}

#[derive(Debug, Clone)]
enum LogoscoreCliSnapshotResult {
    Output(LogosCoreOutput),
    Error(String),
}

impl LogoscoreCliSnapshotResult {
    fn into_result(self) -> Result<LogosCoreOutput> {
        match self {
            Self::Output(output) => Ok(output),
            Self::Error(error) => Err(anyhow::anyhow!("{error}")),
        }
    }
}

#[derive(Debug, Clone)]
struct LogoscoreCliSnapshotEntry {
    observed_at: StdInstant,
    result: LogoscoreCliSnapshotResult,
}

impl LogoscoreCliSnapshotEntry {
    fn is_fresh(&self, kind: LogoscoreCliSnapshotKind, now: StdInstant) -> bool {
        let freshness = match self.result {
            LogoscoreCliSnapshotResult::Output(_) => match kind {
                LogoscoreCliSnapshotKind::Status => LOGOSCORE_CLI_STATUS_SNAPSHOT_FRESHNESS,
                LogoscoreCliSnapshotKind::Modules => LOGOSCORE_CLI_MODULES_SNAPSHOT_FRESHNESS,
            },
            LogoscoreCliSnapshotResult::Error(_) => LOGOSCORE_CLI_FAILURE_BACKOFF,
        };
        now.saturating_duration_since(self.observed_at) <= freshness
    }
}

#[derive(Debug, Default)]
struct LogoscoreCliSnapshot {
    generation: u64,
    status: Option<LogoscoreCliSnapshotEntry>,
    modules: Option<LogoscoreCliSnapshotEntry>,
}

impl LogoscoreCliSnapshot {
    fn entry(&self, kind: LogoscoreCliSnapshotKind) -> Option<&LogoscoreCliSnapshotEntry> {
        match kind {
            LogoscoreCliSnapshotKind::Status => self.status.as_ref(),
            LogoscoreCliSnapshotKind::Modules => self.modules.as_ref(),
        }
    }

    fn set_entry(&mut self, kind: LogoscoreCliSnapshotKind, entry: LogoscoreCliSnapshotEntry) {
        match kind {
            LogoscoreCliSnapshotKind::Status => self.status = Some(entry),
            LogoscoreCliSnapshotKind::Modules => self.modules = Some(entry),
        }
    }

    fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.status = None;
        self.modules = None;
    }
}

#[derive(Debug, Default)]
struct LogoscoreCliCommandGate {
    lock: Mutex<()>,
    controlled_waiters: AtomicUsize,
    snapshot: Mutex<LogoscoreCliSnapshot>,
}

fn fresh_logoscore_cli_snapshot(
    runner: &LogosCoreRunner,
    kind: LogoscoreCliSnapshotKind,
) -> Result<Option<LogosCoreOutput>> {
    let gate = logoscore_cli_command_gate(runner)?;
    let cached = gate
        .snapshot
        .lock()
        .map_err(|_| anyhow::anyhow!("logoscore CLI snapshot is poisoned"))?
        .entry(kind)
        .filter(|entry| entry.is_fresh(kind, StdInstant::now()))
        .cloned();
    cached.map(|entry| entry.result.into_result()).transpose()
}

fn cached_logoscore_cli_snapshot(
    runner: &LogosCoreRunner,
    kind: LogoscoreCliSnapshotKind,
    operation: impl FnOnce() -> Result<LogosCoreOutput>,
) -> Result<LogosCoreOutput> {
    let gate = logoscore_cli_command_gate(runner)?;
    let (generation, cached) = {
        let snapshot = gate
            .snapshot
            .lock()
            .map_err(|_| anyhow::anyhow!("logoscore CLI snapshot is poisoned"))?;
        let cached = snapshot
            .entry(kind)
            .filter(|entry| entry.is_fresh(kind, StdInstant::now()))
            .cloned();
        (snapshot.generation, cached)
    };
    if let Some(cached) = cached {
        return cached.result.into_result();
    }

    let result = operation();
    let cached_result = match &result {
        Ok(output) if cacheable_logoscore_cli_snapshot(kind, output) => {
            Some(LogoscoreCliSnapshotResult::Output(output.clone()))
        }
        Ok(_) => None,
        Err(error) if error.downcast_ref::<CommandTerminated>().is_none() => {
            Some(LogoscoreCliSnapshotResult::Error(format!("{error:#}")))
        }
        Err(_) => None,
    };
    if let Some(cached_result) = cached_result {
        let mut snapshot = gate
            .snapshot
            .lock()
            .map_err(|_| anyhow::anyhow!("logoscore CLI snapshot is poisoned"))?;
        if snapshot.generation == generation {
            let observed_at = StdInstant::now();
            let status_inventory = match (&kind, &cached_result) {
                (LogoscoreCliSnapshotKind::Status, LogoscoreCliSnapshotResult::Output(output))
                    if module_rows(&output.value).is_ok() =>
                {
                    Some(cached_result.clone())
                }
                _ => None,
            };
            snapshot.set_entry(
                kind,
                LogoscoreCliSnapshotEntry {
                    observed_at,
                    result: cached_result,
                },
            );
            if let Some(result) = status_inventory {
                snapshot.set_entry(
                    LogoscoreCliSnapshotKind::Modules,
                    LogoscoreCliSnapshotEntry {
                        observed_at,
                        result,
                    },
                );
            }
        }
    }
    result
}

fn cacheable_logoscore_cli_snapshot(
    kind: LogoscoreCliSnapshotKind,
    output: &LogosCoreOutput,
) -> bool {
    kind != LogoscoreCliSnapshotKind::Status
        || matches!(
            output
                .value
                .pointer("/daemon/status")
                .and_then(Value::as_str),
            Some("running" | "stopped" | "not_running")
        )
}

fn invalidate_logoscore_cli_snapshot(runner: &LogosCoreRunner) -> Result<()> {
    let gate = logoscore_cli_command_gate(runner)?;
    gate.snapshot
        .lock()
        .map_err(|_| anyhow::anyhow!("logoscore CLI snapshot is poisoned"))?
        .invalidate();
    Ok(())
}

fn logoscore_cli_command_gate(runner: &LogosCoreRunner) -> Result<Arc<LogoscoreCliCommandGate>> {
    let mut gates = LOGOSCORE_CLI_COMMAND_GATES
        .lock()
        .map_err(|_| anyhow::anyhow!("logoscore CLI command gate registry is poisoned"))?;
    Ok(Arc::clone(
        gates
            .entry(LogoscoreCliCommandGateKey::from(runner))
            .or_insert_with(|| Arc::new(LogoscoreCliCommandGate::default())),
    ))
}

fn acquire_logoscore_cli_command_gate<'gate>(
    gate: &'gate LogoscoreCliCommandGate,
    control: Option<&CommandControl>,
    deadline: Option<StdInstant>,
) -> Result<MutexGuard<'gate, ()>> {
    let controlled_waiter = control.is_some();
    if controlled_waiter {
        gate.controlled_waiters.fetch_add(1, Ordering::SeqCst);
    }
    let result = (|| {
        loop {
            if let Some(control) = control {
                control.check_active()?;
            }
            if deadline.is_some_and(|deadline| StdInstant::now() >= deadline) {
                bail!("logoscore CLI request timed out waiting for another request");
            }
            if !controlled_waiter && gate.controlled_waiters.load(Ordering::SeqCst) > 0 {
                let sleep_duration = deadline
                    .map(|deadline| {
                        LOGOSCORE_CLI_COMMAND_GATE_POLL_INTERVAL
                            .min(deadline.saturating_duration_since(StdInstant::now()))
                    })
                    .unwrap_or(LOGOSCORE_CLI_COMMAND_GATE_POLL_INTERVAL);
                if sleep_duration == Duration::ZERO {
                    bail!("logoscore CLI request timed out waiting for another request");
                }
                thread::sleep(sleep_duration);
                continue;
            }
            match gate.lock.try_lock() {
                Ok(permit) => return Ok(permit),
                Err(TryLockError::Poisoned(_)) => {
                    bail!("logoscore CLI command gate is poisoned");
                }
                Err(TryLockError::WouldBlock) => {
                    let sleep_duration = deadline
                        .map(|deadline| {
                            LOGOSCORE_CLI_COMMAND_GATE_POLL_INTERVAL
                                .min(deadline.saturating_duration_since(StdInstant::now()))
                        })
                        .unwrap_or(LOGOSCORE_CLI_COMMAND_GATE_POLL_INTERVAL);
                    if sleep_duration == Duration::ZERO {
                        bail!("logoscore CLI request timed out waiting for another request");
                    }
                    thread::sleep(sleep_duration);
                }
            }
        }
    })();
    if controlled_waiter {
        gate.controlled_waiters.fetch_sub(1, Ordering::SeqCst);
    }
    result
}

fn is_transient_module_discovery_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        let detail = cause.to_string();
        detail.contains("RPC_FAILED")
            || detail.contains("command stopped after deadline exceeded")
            || detail.contains("not loaded")
    })
}

fn is_module_discovery_attempt_timeout(
    error: &anyhow::Error,
    control: &CommandControl,
    attempt_deadline: StdInstant,
) -> bool {
    attempt_deadline < control.deadline()
        && error
            .downcast_ref::<CommandTerminated>()
            .is_some_and(|termination| termination.reason() == CommandStopReason::DeadlineExceeded)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LogosCoreRunner {
    program: String,
    sudo_user: Option<String>,
    home: Option<String>,
    config_dir: Option<String>,
    label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct LogoscoreCliCommandGateKey {
    program: String,
    sudo_user: Option<String>,
    home: Option<String>,
    config_dir: Option<String>,
}

impl From<&LogosCoreRunner> for LogoscoreCliCommandGateKey {
    fn from(runner: &LogosCoreRunner) -> Self {
        Self {
            program: runner.program.clone(),
            sudo_user: runner.sudo_user.clone(),
            home: runner.home.clone(),
            config_dir: runner.config_dir.clone(),
        }
    }
}

static LOGOSCORE_CLI_COMMAND_GATES: LazyLock<
    Mutex<HashMap<LogoscoreCliCommandGateKey, Arc<LogoscoreCliCommandGate>>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));

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
        let config_bytes = read_runner_client_config(runner, &config_path)?;
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

    fn share_directory(&self, path: &Path, mode: u32) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::{PermissionsExt as _, chown};

            chown(path, None, Some(self.group))
                .context("failed to assign logoscore shared directory group")?;
            fs::set_permissions(path, fs::Permissions::from_mode(mode))
                .context("failed to secure logoscore shared directory")?;
        }
        #[cfg(not(unix))]
        let (_path, _mode) = (path, mode);
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

fn read_runner_client_config(runner: &LogosCoreRunner, config_path: &Path) -> Result<Vec<u8>> {
    let config_bytes = if let Some(command) = runner_client_config_read_command(runner, config_path)
    {
        let output = run_command(
            command,
            CommandRunPolicy {
                label: &runner.label,
                timeout: command_timeout(),
                poll_interval: LOGOSCORE_POLL_INTERVAL,
                redactions: &[],
                output_limit: 0,
                capture_limit: DEFAULT_COMMAND_CAPTURE_LIMIT,
            },
        )
        .with_context(|| {
            format!(
                "failed to read logoscore client config `{}` through configured service identity",
                config_path.display()
            )
        })?;
        output.stdout
    } else {
        fs::read(config_path).with_context(|| {
            format!(
                "failed to read logoscore client config `{}`",
                config_path.display()
            )
        })?
    };
    anyhow::ensure!(
        config_bytes.len() <= LOGOSCORE_CLIENT_CONFIG_LIMIT,
        "logoscore client config exceeds {LOGOSCORE_CLIENT_CONFIG_LIMIT} byte limit"
    );
    Ok(config_bytes)
}

fn runner_client_config_read_command(
    runner: &LogosCoreRunner,
    config_path: &Path,
) -> Option<Command> {
    let user = runner.sudo_user.as_deref()?;
    let mut command = Command::new("sudo");
    command.arg("-n").arg("-u").arg(user).arg("env");
    if let Some(home) = &runner.home {
        command.arg(format!("HOME={home}"));
    }
    command.arg("/bin/cat").arg("--").arg(config_path);
    Some(command)
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

fn listed_module_status<'value>(module: &str, modules_value: &'value Value) -> Result<&'value str> {
    if module.trim().is_empty() {
        bail!("module name is required");
    }
    let rows = module_rows(modules_value)?;
    let row = rows
        .iter()
        .find(|candidate| candidate.get("name").and_then(Value::as_str) == Some(module))
        .with_context(|| format!("logoscore module `{module}` is not listed"))?;
    row.get("status")
        .and_then(Value::as_str)
        .with_context(|| format!("logoscore module `{module}` listing has no status"))
}

fn require_listed_module_loaded(module: &str, modules_value: &Value) -> Result<()> {
    let status = listed_module_status(module, modules_value)?;
    if status != "loaded" {
        bail!("logoscore module `{module}` is not loaded (status `{status}`)");
    }
    Ok(())
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

fn typed_call_arguments(module: &str, method: &str, args: &[Value]) -> Result<Vec<String>> {
    if module.trim().is_empty() {
        bail!("module name is required");
    }
    if method.trim().is_empty() {
        bail!("method name is required");
    }

    let encoded = serde_json::to_string(args)
        .context("failed to encode exact LogosCore module-call arguments")?;
    Ok(vec![
        "call".to_owned(),
        module.to_owned(),
        method.to_owned(),
        "--args-json".to_owned(),
        encoded,
        "--json".to_owned(),
    ])
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
            capture_limit: DEFAULT_COMMAND_CAPTURE_LIMIT,
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

fn run_status_json_with(runner: &LogosCoreRunner, timeout: Duration) -> Result<LogosCoreOutput> {
    let command = command_for_runner(runner, ["status", "--json"]);
    let output = run_command_allow_failure(
        command,
        CommandRunPolicy {
            label: &runner.label,
            timeout,
            poll_interval: LOGOSCORE_POLL_INTERVAL,
            redactions: &[],
            output_limit: LOGOSCORE_OUTPUT_LIMIT,
            capture_limit: DEFAULT_COMMAND_CAPTURE_LIMIT,
        },
    )?;
    logos_core_status_output_with_limit(runner, output, LOGOSCORE_JSON_OUTPUT_LIMIT)
}

fn run_json_before_deadline<I, S>(
    runner: &LogosCoreRunner,
    args: I,
    deadline: StdInstant,
) -> Result<LogosCoreOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let remaining_timeout = deadline.saturating_duration_since(StdInstant::now());
    if remaining_timeout == Duration::ZERO {
        bail!(
            "{} request timed out waiting for another LogosCore CLI request",
            runner.label
        );
    }
    run_json_with(runner, args, remaining_timeout)
}

fn run_status_json_before_deadline(
    runner: &LogosCoreRunner,
    deadline: StdInstant,
) -> Result<LogosCoreOutput> {
    let remaining_timeout = deadline.saturating_duration_since(StdInstant::now());
    if remaining_timeout == Duration::ZERO {
        bail!(
            "{} request timed out waiting for another LogosCore CLI request",
            runner.label
        );
    }
    run_status_json_with(runner, remaining_timeout)
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
    run_json_with_controlled_with_output_limit(runner, args, control, LOGOSCORE_JSON_OUTPUT_LIMIT)
}

fn run_json_with_controlled_with_output_limit<I, S>(
    runner: &LogosCoreRunner,
    args: I,
    control: CommandControl,
    json_output_limit: usize,
) -> Result<LogosCoreOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    validate_json_output_limit(json_output_limit)?;
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
            capture_limit: json_output_limit,
        },
        control,
    )?;
    logos_core_output_with_limit(runner, output, json_output_limit)
}

fn run_status_json_with_controlled(
    runner: &LogosCoreRunner,
    control: CommandControl,
) -> Result<LogosCoreOutput> {
    let command = command_for_runner(runner, ["status", "--json"]);
    let output = run_command_controlled_allow_failure(
        command,
        CommandRunPolicy {
            label: &runner.label,
            // Controlled commands have one authority: CommandControl's absolute deadline.
            timeout: Duration::ZERO,
            poll_interval: LOGOSCORE_POLL_INTERVAL,
            redactions: &[],
            output_limit: LOGOSCORE_OUTPUT_LIMIT,
            capture_limit: LOGOSCORE_JSON_OUTPUT_LIMIT,
        },
        control,
    )?;
    logos_core_status_output_with_limit(runner, output, LOGOSCORE_JSON_OUTPUT_LIMIT)
}

fn logos_core_status_output_with_limit(
    runner: &LogosCoreRunner,
    output: std::process::Output,
    json_output_limit: usize,
) -> Result<LogosCoreOutput> {
    let exit_succeeded = output.status.success();
    let exit_status = output.status.to_string();
    let failure_message =
        (!exit_succeeded).then(|| process_message(&output, &[], LOGOSCORE_OUTPUT_LIMIT));
    let result = logos_core_output_with_limit(runner, output, json_output_limit)?;
    if exit_succeeded || status_reports_not_running(&result.value) {
        return Ok(result);
    }
    bail!(
        "{} exited with {exit_status}: {}",
        runner.label,
        failure_message.unwrap_or_else(|| "no output".to_owned())
    )
}

fn status_reports_not_running(value: &Value) -> bool {
    value.pointer("/daemon/status").and_then(Value::as_str) == Some("not_running")
}

fn logos_core_output_with_limit(
    runner: &LogosCoreRunner,
    output: std::process::Output,
    json_output_limit: usize,
) -> Result<LogosCoreOutput> {
    let stderr = output_text(&output.stderr, &[], LOGOSCORE_OUTPUT_LIMIT);
    let value = parse_json_stdout_with_limit(&runner.label, &output.stdout, json_output_limit)?;
    let stderr = (!stderr.is_empty()).then_some(stderr);
    Ok(LogosCoreOutput {
        runner: runner.label.clone(),
        value,
        stderr,
    })
}

fn parse_json_stdout(label: &str, stdout: &[u8]) -> Result<Value> {
    parse_json_stdout_with_limit(label, stdout, LOGOSCORE_JSON_OUTPUT_LIMIT)
}

fn parse_json_stdout_with_limit(
    label: &str,
    stdout: &[u8],
    json_output_limit: usize,
) -> Result<Value> {
    validate_json_output_limit(json_output_limit)?;
    if stdout.len() > json_output_limit {
        bail!("{label} JSON output exceeded {} bytes", json_output_limit);
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

fn validate_json_output_limit(json_output_limit: usize) -> Result<()> {
    ensure!(
        json_output_limit > 0 && json_output_limit <= LOGOSCORE_MAX_JSON_OUTPUT_LIMIT,
        "LogosCore CLI JSON output limit must be between 1 and {LOGOSCORE_MAX_JSON_OUTPUT_LIMIT} bytes"
    );
    Ok(())
}

fn command_timeout() -> Duration {
    env::var("LOGOSCORE_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_secs(5))
}

fn compound_command_timeout() -> Duration {
    command_timeout().saturating_mul(2)
}

fn command_for_runner<I, S>(runner: &LogosCoreRunner, args: I) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    command_for_runner_with_environment(runner, args, &[])
}

fn command_for_runner_with_environment<I, S>(
    runner: &LogosCoreRunner,
    args: I,
    environment: &[(&str, &str)],
) -> Command
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
        for (key, value) in environment {
            command.arg(format!("{key}={value}"));
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
        for (key, value) in environment {
            command.env(key, value);
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
    let actual_event = value
        .get("event")
        .and_then(Value::as_str)
        .context("event name must be a string")?;
    anyhow::ensure!(
        !actual_event.trim().is_empty(),
        "event name must not be empty"
    );
    anyhow::ensure!(
        actual_event.len() <= LOGOSCORE_EVENT_NAME_LIMIT,
        "event name exceeded {LOGOSCORE_EVENT_NAME_LIMIT} byte limit"
    );
    if !event.is_empty() {
        anyhow::ensure!(actual_event == event, "event name does not match `{event}`");
    }
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

pub(crate) fn module_transport_event_from_watch_frame(
    value: &Value,
    module: &str,
) -> Result<ModuleTransportEvent> {
    validate_watch_event_frame(value, module, "")?;
    let event = value
        .get("event")
        .and_then(Value::as_str)
        .context("event name must be a string")?;
    let data = value
        .get("data")
        .and_then(Value::as_object)
        .context("event data must be an object")?;
    let mut args = Vec::with_capacity(data.len());
    for index in 0..data.len() {
        let key = format!("arg{index}");
        let arg = data
            .get(&key)
            .with_context(|| format!("event data must contain consecutive `{key}` fields"))?;
        args.push(normalize_watch_event_arg(arg)?);
    }
    ModuleTransportEvent::new(module, event, args)
}

fn normalize_watch_event_arg(value: &Value) -> Result<Value> {
    let Some(object) = value.as_object() else {
        return Ok(value.clone());
    };
    if object.len() != 1 || !object.contains_key("_bytes") {
        return Ok(value.clone());
    }
    let encoded = object
        .get("_bytes")
        .and_then(Value::as_str)
        .context("typed event byte payload must be a base64url string")?;
    let bytes = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .context("typed event byte payload is not valid base64url")?;
    match String::from_utf8(bytes) {
        Ok(text) => Ok(Value::String(text)),
        Err(_) => Ok(value.clone()),
    }
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
    cleanup_token: String,
    cleanup_authority: WatchCleanupAuthority,
    lease: Option<LogoscoreWatchLease>,
    label: String,
}

impl FailedWatchStart {
    fn new(
        child: Child,
        reader: Option<thread::JoinHandle<()>>,
        reader_stop: Option<Arc<AtomicBool>>,
        process_permit: StreamingCommandPermit,
        recovery: mpsc::Sender<LogoscoreWatchRecovery>,
        cleanup: WatchCleanup,
        label: &str,
    ) -> Self {
        let WatchCleanup {
            token: cleanup_token,
            authority: cleanup_authority,
            lease,
        } = cleanup;
        Self {
            child,
            reader,
            reader_stop,
            process_permit,
            recovery,
            cleanup_token,
            cleanup_authority,
            lease,
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
    F: FnOnce(&mut Child, &str, &WatchCleanupAuthority, &str) -> Result<()>,
{
    let FailedWatchStart {
        mut child,
        reader,
        reader_stop,
        process_permit,
        recovery,
        cleanup_token,
        cleanup_authority,
        mut lease,
        label,
    } = state;
    let reader_stop = reader_stop.unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
    reader_stop.store(true, Ordering::Release);
    let stop = cleanup(&mut child, &cleanup_token, &cleanup_authority, &label);
    if let Err(stop) = stop {
        handoff_watch_recovery(
            Some(recovery),
            LogoscoreWatchRecovery {
                child,
                reader,
                stderr_reader: None,
                reader_stop,
                process_permit: Some(process_permit),
                cleanup_token,
                cleanup_authority,
                lease,
                label: label.clone(),
            },
        );
        return LogoscoreWatchCleanupUnconfirmed::new(format!(
            "{primary}; failed watch-start process cleanup: {stop:#}"
        ))
        .into();
    }
    if let Err(lease_error) = release_watch_lease(&mut lease) {
        return LogoscoreWatchCleanupUnconfirmed::new(format!(
            "{primary}; failed to release rejected LogosCore watch lease: {lease_error:#}"
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

fn new_logoscore_watch_cleanup_token() -> Result<String> {
    let mut nonce = [0_u8; 16];
    getrandom::fill(&mut nonce).context("failed to generate logoscore watch cleanup token")?;
    let serial = LOGOSCORE_WATCH_CLEANUP_SERIAL.fetch_add(1, Ordering::Relaxed);
    Ok(format!(
        "logos-inspector-watch-{serial}-{}",
        hex::encode(nonce)
    ))
}

#[cfg(target_os = "linux")]
fn new_logoscore_watch_launch_nonce() -> Result<String> {
    let mut nonce = [0_u8; 16];
    getrandom::fill(&mut nonce).context("failed to generate LogosCore watch launch nonce")?;
    Ok(hex::encode(nonce))
}

fn stop_watch_child_with_retry(
    child: &mut Child,
    cleanup_token: &str,
    cleanup_authority: &WatchCleanupAuthority,
    label: &str,
) -> Result<()> {
    let direct = match cleanup_authority {
        WatchCleanupAuthority::Direct => stop_direct_watch_child(child, label),
        #[cfg(target_os = "linux")]
        WatchCleanupAuthority::ServiceIdentity { user } => {
            stop_service_watch_child(child, user, label)
        }
    };
    let direct = match direct {
        Ok(()) => Ok(()),
        Err(first) => stop_watch_child_once(child, cleanup_authority, label).map_err(|second| {
            LogoscoreWatchCleanupUnconfirmed::new(format!(
                "{label} primary cleanup remained unconfirmed after retry: first={first:#}; second={second:#}"
            ))
            .into()
        }),
    };
    let tagged = stop_tagged_watch_processes(cleanup_token, cleanup_authority, label);
    match (direct, tagged) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(direct), Ok(())) => Err(direct),
        (Ok(()), Err(tagged)) => Err(tagged),
        (Err(direct), Err(tagged)) => Err(LogoscoreWatchCleanupUnconfirmed::new(format!(
            "{label} cleanup failed: direct={direct:#}; token-tagged={tagged:#}"
        ))
        .into()),
    }
}

fn stop_watch_child_once(
    child: &mut Child,
    cleanup_authority: &WatchCleanupAuthority,
    label: &str,
) -> Result<()> {
    match cleanup_authority {
        WatchCleanupAuthority::Direct => stop_direct_watch_child(child, label),
        #[cfg(target_os = "linux")]
        WatchCleanupAuthority::ServiceIdentity { user } => {
            stop_service_watch_child(child, user, label)
        }
    }
}

fn stop_direct_watch_child(child: &mut Child, label: &str) -> Result<()> {
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

#[cfg(target_os = "linux")]
fn stop_service_watch_child(child: &mut Child, user: &str, label: &str) -> Result<()> {
    match child.try_wait() {
        Ok(Some(_)) => return Ok(()),
        Ok(None) => {}
        Err(error) => {
            return force_stop_service_watch_child(
                child,
                user,
                label,
                anyhow::Error::new(error)
                    .context(format!("failed to poll {label} during service cleanup")),
            );
        }
    }
    if let Err(error) = signal_service_watch_child(child, user, "TERM", label) {
        return force_stop_service_watch_child(child, user, label, error);
    }
    let deadline = StdInstant::now()
        .checked_add(LOGOSCORE_WATCH_STOP_GRACE)
        .context("logoscore service watch cleanup deadline overflow")?;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Ok(()),
            Ok(None) => {}
            Err(error) => {
                return force_stop_service_watch_child(
                    child,
                    user,
                    label,
                    anyhow::Error::new(error)
                        .context(format!("failed to poll {label} during service cleanup")),
                );
            }
        }
        if StdInstant::now() >= deadline {
            break;
        }
        thread::sleep(LOGOSCORE_POLL_INTERVAL);
    }
    force_stop_service_watch_child(
        child,
        user,
        label,
        anyhow::anyhow!("{label} did not stop after graceful service termination"),
    )
}

#[cfg(target_os = "linux")]
fn force_stop_service_watch_child(
    child: &mut Child,
    user: &str,
    label: &str,
    primary: anyhow::Error,
) -> Result<()> {
    let force = signal_service_watch_child(child, user, "KILL", label);
    let deadline = StdInstant::now()
        .checked_add(LOGOSCORE_WATCH_STOP_GRACE)
        .context("logoscore service watch forced-cleanup deadline overflow")?;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Ok(()),
            Ok(None) => {}
            Err(error) => {
                return Err(anyhow::anyhow!(
                    "{primary}; forced service cleanup failed: signal={}, reap={error}",
                    watch_cleanup_status(force),
                ));
            }
        }
        if StdInstant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "{primary}; forced service cleanup timed out: signal={}",
                watch_cleanup_status(force),
            ));
        }
        thread::sleep(LOGOSCORE_POLL_INTERVAL);
    }
}

#[cfg(target_os = "linux")]
fn signal_service_watch_child(
    child: &mut Child,
    user: &str,
    signal: &str,
    label: &str,
) -> Result<()> {
    let process = child.id();
    match signal_service_watch_child_as_root(process, signal, label) {
        Ok(()) => Ok(()),
        Err(root_error) => match signal_service_watch_child_as_user(process, user, signal, label) {
            Ok(()) => Ok(()),
            Err(user_error) => match child.try_wait() {
                Ok(Some(_)) => Ok(()),
                Ok(None) => Err(anyhow::anyhow!(
                    "could not signal {label} through configured service cleanup: elevated={root_error:#}; service={user_error:#}"
                )),
                Err(error) => Err(error).context(format!(
                    "could not signal {label} through configured service cleanup; child status is unavailable"
                )),
            },
        },
    }
}

#[cfg(target_os = "linux")]
fn signal_service_watch_child_as_root(process: u32, signal: &str, label: &str) -> Result<()> {
    let mut command = elevated_watch_child_signal_command(process, signal);
    let output = command
        .output()
        .with_context(|| format!("failed to signal {label} through elevated cleanup"))?;
    anyhow::ensure!(
        output.status.success(),
        "elevated cleanup could not signal {label}"
    );
    Ok(())
}

#[cfg(target_os = "linux")]
fn elevated_watch_child_signal_command(process: u32, signal: &str) -> Command {
    let mut command = Command::new("sudo");
    command
        .arg("-n")
        .arg("/bin/kill")
        .arg(format!("-{signal}"))
        .arg("--")
        .arg(process.to_string());
    command
}

#[cfg(target_os = "linux")]
fn signal_service_watch_child_as_user(
    process: u32,
    user: &str,
    signal: &str,
    label: &str,
) -> Result<()> {
    let mut command = Command::new("sudo");
    command
        .arg("-n")
        .arg("-u")
        .arg(user)
        .arg("/bin/kill")
        .arg(format!("-{signal}"))
        .arg("--")
        .arg(process.to_string());
    let output = command
        .output()
        .with_context(|| format!("failed to signal {label} through configured service identity"))?;
    if output.status.success() {
        return Ok(());
    }
    bail!("configured service identity could not signal {label}")
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

#[cfg(target_os = "linux")]
fn stop_tagged_watch_processes(
    cleanup_token: &str,
    cleanup_authority: &WatchCleanupAuthority,
    label: &str,
) -> Result<()> {
    match cleanup_authority {
        WatchCleanupAuthority::Direct => stop_direct_tagged_watch_processes(cleanup_token, label),
        WatchCleanupAuthority::ServiceIdentity { user } => {
            stop_service_tagged_watch_processes(cleanup_token, user, label)
        }
    }
}

#[cfg(target_os = "linux")]
fn stop_direct_tagged_watch_processes(cleanup_token: &str, label: &str) -> Result<()> {
    use nix::{
        errno::Errno,
        sys::signal::{Signal, kill},
        unistd::Pid,
    };

    fn signal(processes: &[i32], signal: Signal, label: &str) -> Result<()> {
        for process in processes {
            match kill(Pid::from_raw(*process), signal) {
                Ok(()) | Err(Errno::ESRCH) => {}
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!(
                            "failed to send {signal:?} to token-tagged {label} process {process}"
                        )
                    });
                }
            }
        }
        Ok(())
    }

    let mut processes = tagged_watch_processes(cleanup_token)?;
    if processes.is_empty() {
        return Ok(());
    }
    signal(&processes, Signal::SIGTERM, label)?;
    let graceful_deadline = StdInstant::now()
        .checked_add(LOGOSCORE_WATCH_STOP_GRACE)
        .context("token-tagged logoscore watch cleanup deadline overflow")?;
    while !processes.is_empty() && StdInstant::now() < graceful_deadline {
        thread::sleep(LOGOSCORE_POLL_INTERVAL);
        processes = tagged_watch_processes(cleanup_token)?;
    }
    if processes.is_empty() {
        return Ok(());
    }
    signal(&processes, Signal::SIGKILL, label)?;
    let forced_deadline = StdInstant::now()
        .checked_add(LOGOSCORE_WATCH_STOP_GRACE)
        .context("forced token-tagged logoscore watch cleanup deadline overflow")?;
    while !processes.is_empty() && StdInstant::now() < forced_deadline {
        thread::sleep(LOGOSCORE_POLL_INTERVAL);
        processes = tagged_watch_processes(cleanup_token)?;
    }
    anyhow::ensure!(
        processes.is_empty(),
        "{label} cleanup left token-tagged descendant processes running: {processes:?}"
    );
    Ok(())
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
enum ServiceWatchTokenAction {
    Check,
    Terminate,
    Kill,
}

#[cfg(target_os = "linux")]
impl ServiceWatchTokenAction {
    const fn name(self) -> &'static str {
        match self {
            Self::Check => "CHECK",
            Self::Terminate => "TERM",
            Self::Kill => "KILL",
        }
    }
}

#[cfg(target_os = "linux")]
const SERVICE_WATCH_TOKEN_CLEANUP_SCRIPT: &str = r#"
token=$1
action=$2
marker="LOGOS_INSPECTOR_WATCH_TOKEN=$token"

case "$action" in
  CHECK) signal="" ;;
  TERM) signal="-TERM" ;;
  KILL) signal="-KILL" ;;
  *) exit 64 ;;
esac

for entry in /proc/[0-9]*; do
  pid=${entry#/proc/}
  [ "$pid" = "$$" ] && continue
  [ -r "$entry/environ" ] || continue
  if /usr/bin/grep -Fzx -- "$marker" "$entry/environ" >/dev/null 2>&1; then
    printf '%s\n' "$pid"
    [ -z "$signal" ] || /bin/kill "$signal" "$pid" 2>/dev/null || :
  fi
done
"#;

#[cfg(target_os = "linux")]
fn stop_service_tagged_watch_processes(cleanup_token: &str, user: &str, label: &str) -> Result<()> {
    let mut processes = service_tagged_watch_processes(
        user,
        cleanup_token,
        ServiceWatchTokenAction::Terminate,
        label,
    )?;
    let graceful_deadline = StdInstant::now()
        .checked_add(LOGOSCORE_WATCH_STOP_GRACE)
        .context("service token-tagged logoscore watch cleanup deadline overflow")?;
    while !processes.is_empty() && StdInstant::now() < graceful_deadline {
        thread::sleep(LOGOSCORE_POLL_INTERVAL);
        processes = service_tagged_watch_processes(
            user,
            cleanup_token,
            ServiceWatchTokenAction::Check,
            label,
        )?;
    }
    if processes.is_empty() {
        return Ok(());
    }
    processes =
        service_tagged_watch_processes(user, cleanup_token, ServiceWatchTokenAction::Kill, label)?;
    let forced_deadline = StdInstant::now()
        .checked_add(LOGOSCORE_WATCH_STOP_GRACE)
        .context("forced service token-tagged logoscore watch cleanup deadline overflow")?;
    while !processes.is_empty() && StdInstant::now() < forced_deadline {
        thread::sleep(LOGOSCORE_POLL_INTERVAL);
        processes = service_tagged_watch_processes(
            user,
            cleanup_token,
            ServiceWatchTokenAction::Check,
            label,
        )?;
    }
    anyhow::ensure!(
        processes.is_empty(),
        "{label} cleanup left token-tagged service descendants running: {processes:?}"
    );
    Ok(())
}

#[cfg(target_os = "linux")]
fn service_tagged_watch_processes(
    user: &str,
    cleanup_token: &str,
    action: ServiceWatchTokenAction,
    label: &str,
) -> Result<Vec<i32>> {
    let mut command = service_watch_cleanup_command(user, cleanup_token, action);
    let output = command.output().with_context(|| {
        format!(
            "failed to inspect token-tagged {label} processes through configured service identity"
        )
    })?;
    anyhow::ensure!(
        output.status.success(),
        "configured service identity could not inspect token-tagged {label} processes"
    );
    parse_service_watch_processes(&output.stdout, label)
}

#[cfg(target_os = "linux")]
fn service_watch_cleanup_command(
    user: &str,
    cleanup_token: &str,
    action: ServiceWatchTokenAction,
) -> Command {
    let mut command = Command::new("sudo");
    command
        .arg("-n")
        .arg("-u")
        .arg(user)
        .arg("/bin/sh")
        .arg("-c")
        .arg(SERVICE_WATCH_TOKEN_CLEANUP_SCRIPT)
        .arg("logos-inspector-watch-cleanup")
        .arg(cleanup_token)
        .arg(action.name());
    command
}

#[cfg(target_os = "linux")]
fn parse_service_watch_processes(output: &[u8], label: &str) -> Result<Vec<i32>> {
    let text = std::str::from_utf8(output).with_context(|| {
        format!("configured service cleanup returned non-UTF-8 output for {label}")
    })?;
    let mut processes = Vec::new();
    for line in text.lines() {
        let process = line.parse::<i32>().with_context(|| {
            format!("configured service cleanup returned an invalid PID for {label}")
        })?;
        anyhow::ensure!(
            process > 0,
            "configured service cleanup returned a nonpositive PID for {label}"
        );
        processes.push(process);
    }
    processes.sort_unstable();
    processes.dedup();
    Ok(processes)
}

#[cfg(not(target_os = "linux"))]
fn stop_tagged_watch_processes(
    _cleanup_token: &str,
    _cleanup_authority: &WatchCleanupAuthority,
    _label: &str,
) -> Result<()> {
    Ok(())
}

#[cfg(target_os = "linux")]
fn tagged_watch_processes(cleanup_token: &str) -> Result<Vec<i32>> {
    let mut marker = Vec::with_capacity(
        LOGOSCORE_WATCH_CLEANUP_TOKEN_ENV
            .len()
            .saturating_add(cleanup_token.len())
            .saturating_add(1),
    );
    marker.extend_from_slice(LOGOSCORE_WATCH_CLEANUP_TOKEN_ENV.as_bytes());
    marker.push(b'=');
    marker.extend_from_slice(cleanup_token.as_bytes());

    let entries =
        fs::read_dir("/proc").context("failed to inspect Linux processes for watch cleanup")?;
    let mut processes = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(process) = name.parse::<i32>() else {
            continue;
        };
        if process <= 0 || process == i32::try_from(std::process::id()).unwrap_or_default() {
            continue;
        }
        let environment = match fs::read(entry.path().join("environ")) {
            Ok(environment) => environment,
            Err(error)
                if error.kind() == ErrorKind::NotFound
                    || error.kind() == ErrorKind::PermissionDenied =>
            {
                continue;
            }
            Err(_) => continue,
        };
        if !environment
            .split(|byte| *byte == b'\0')
            .any(|value| value == marker)
        {
            continue;
        }
        if linux_process_is_live(process)? {
            processes.push(process);
        }
    }
    Ok(processes)
}

#[cfg(target_os = "linux")]
fn linux_process_is_live(process: i32) -> Result<bool> {
    Ok(linux_process_start_marker(process)?.is_some())
}

#[cfg(target_os = "linux")]
fn linux_process_start_marker(process: i32) -> Result<Option<u64>> {
    let status_path = PathBuf::from(format!("/proc/{process}/stat"));
    match fs::read_to_string(status_path) {
        Ok(status) => {
            let Some((_, fields)) = status.rsplit_once(')') else {
                return Ok(None);
            };
            let mut fields = fields.split_whitespace();
            if fields.next() == Some("Z") {
                return Ok(None);
            }
            // `/proc/<pid>/stat` field 22 is process start time. `fields`
            // begins at field 3 after the parenthesized command name, and the
            // state field has already been consumed above.
            let Some(start_marker) = fields.nth(18) else {
                return Ok(None);
            };
            start_marker.parse::<u64>().map(Some).with_context(|| {
                format!("failed to parse Linux process start marker for {process}")
            })
        }
        Err(error)
            if error.kind() == ErrorKind::NotFound
                || error.raw_os_error() == Some(nix::libc::ESRCH) =>
        {
            Ok(None)
        }
        Err(error) => Err(error).context("failed to inspect token-tagged logoscore watch process"),
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

fn logoscore_environment_is_configured() -> bool {
    [
        "LOGOSCORE_BIN",
        "LOGOSCORE_USER",
        "LOGOSCORE_HOME",
        "LOGOSCORE_CONFIG_DIR",
    ]
    .into_iter()
    .any(|key| env::var(key).is_ok_and(|value| !value.trim().is_empty()))
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
    normalize_module_call_value_inner(module, method, value, 0)
}

const MODULE_RESULT_UNWRAP_MAX_DEPTH: usize = 8;

fn normalize_module_call_value_inner(
    module: &str,
    method: &str,
    value: Value,
    depth: usize,
) -> Result<Value> {
    if depth > MODULE_RESULT_UNWRAP_MAX_DEPTH {
        bail!("{module}.{method} returned excessively nested result envelopes");
    }
    let value = parse_module_json_string(value);
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

    let Some(object) = value.as_object() else {
        return Ok(value);
    };

    // Logos Protocol and Basecamp can each add a result envelope. Accept
    // repeated exact envelopes, but keep application payloads such as
    // {"result": ..., "kind": ...} opaque.
    if object.len() == 1 && object.contains_key("result") {
        return normalize_module_call_value_inner(
            module,
            method,
            object.get("result").cloned().unwrap_or(Value::Null),
            depth + 1,
        );
    }

    if status == "ok" && object.contains_key("result") {
        return normalize_module_call_value_inner(
            module,
            method,
            object.get("result").cloned().unwrap_or(Value::Null),
            depth + 1,
        );
    }

    // LogosCore CLI omits a null `error` member from successful results, while
    // the Basecamp bridge serializes it. Accept both wire shapes, but keep
    // objects with additional application fields opaque.
    let canonical_result_shape =
        (object.len() == 2 && object.contains_key("success") && object.contains_key("value"))
            || (object.len() == 3
                && object.contains_key("success")
                && object.contains_key("value")
                && object.contains_key("error"));
    if canonical_result_shape && let Some(success) = object.get("success").and_then(Value::as_bool)
    {
        if !success {
            let error = object
                .get("error")
                .map(module_value_error_text)
                .filter(|error| !error.is_empty())
                .unwrap_or_else(|| "module call failed".to_owned());
            bail!("{module}.{method} failed: {error}");
        }
        return normalize_module_call_value_inner(
            module,
            method,
            object.get("value").cloned().unwrap_or(Value::Null),
            depth + 1,
        );
    }

    Ok(value)
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

    #[cfg(unix)]
    fn write_executable_script(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;
        use std::time::{SystemTime, UNIX_EPOCH};

        // Write via a new inode then rename. In-place rewrite/exec races under
        // parallel test load can fail with ETXTBSY on Linux.
        let temp = path.with_extension(format!(
            "tmp-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::write(&temp, contents)?;
        let mut permissions = fs::metadata(&temp)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&temp, permissions)?;
        fs::rename(&temp, path)?;
        Ok(())
    }

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
        subscriptions: Mutex<Vec<(String, String, String)>>,
    }

    impl RecordingTransport {
        fn new(kind: ModuleTransportKind, reply_kind: ModuleTransportKind) -> Self {
            Self {
                kind,
                reply_kind,
                calls: AtomicUsize::new(0),
                last_call: Mutex::new(None),
                subscriptions: Mutex::new(Vec::new()),
            }
        }
    }

    #[test]
    fn scoped_module_calls_and_events_require_and_preserve_instance_ids() -> Result<()> {
        let call = ModuleCall::new_instance(
            ModuleTransportKind::Module,
            "lez_indexer_module",
            "indexer-testnet-0101010101010101",
            "nodeStatus",
            vec![json!({ "verbose": true })],
        )?;
        anyhow::ensure!(
            call.instance_id() == Some("indexer-testnet-0101010101010101"),
            "scoped module call lost its instance identifier"
        );
        anyhow::ensure!(
            ModuleCall::new_instance(
                ModuleTransportKind::Module,
                "lez_indexer_module",
                "  ",
                "nodeStatus",
                Vec::new(),
            )
            .is_err(),
            "blank scoped module call instance identifier was accepted"
        );

        let event = ModuleTransportEvent::new_instance(
            "lez_indexer_module",
            "indexer-testnet-0101010101010101",
            "nodeChanged",
            vec![json!({ "running": true })],
        )?;
        anyhow::ensure!(
            event.instance_id() == Some("indexer-testnet-0101010101010101"),
            "scoped module event lost its instance identifier"
        );
        anyhow::ensure!(
            ModuleTransportEvent::new_instance(
                "lez_indexer_module",
                "",
                "nodeChanged",
                Vec::new(),
            )
            .is_err(),
            "blank scoped module event instance identifier was accepted"
        );
        Ok(())
    }

    #[tokio::test]
    async fn scoped_module_transport_never_uses_a_default_instance() -> Result<()> {
        let inner = Arc::new(RecordingTransport::new(
            ModuleTransportKind::Module,
            ModuleTransportKind::Module,
        ));
        let transport: SharedModuleTransport = inner.clone();
        let scoped = ScopedModuleTransport::new(
            transport,
            "lez_indexer_module",
            "indexer-network-0101010101010101",
        )?;

        dispatch_module_call(
            &scoped,
            ModuleCall::new(
                ModuleTransportKind::Module,
                "lez_indexer_module",
                "getStatus",
                Vec::new(),
            )?,
        )
        .await?;
        let call = inner
            .last_call
            .lock()
            .map_err(|error| anyhow::anyhow!("recording call lock failed: {error}"))?
            .clone()
            .context("scoped transport did not forward its call")?;
        anyhow::ensure!(
            call.instance_id() == Some("indexer-network-0101010101010101"),
            "scoped transport dispatched through a default module instance"
        );

        let _subscription = scoped.subscribe_module_event("lez_indexer_module", "nodeChanged")?;
        let subscriptions = inner
            .subscriptions
            .lock()
            .map_err(|error| anyhow::anyhow!("recording subscriptions lock failed: {error}"))?
            .clone();
        anyhow::ensure!(
            subscriptions
                == vec![(
                    "lez_indexer_module".to_owned(),
                    "indexer-network-0101010101010101".to_owned(),
                    "nodeChanged".to_owned(),
                )],
            "scoped transport did not subscribe to its exact module instance"
        );

        let wrong_module = ModuleCall::new(
            ModuleTransportKind::Module,
            "storage_module",
            "getStatus",
            Vec::new(),
        )?;
        anyhow::ensure!(
            dispatch_module_call(&scoped, wrong_module).await.is_err(),
            "scoped transport accepted a different module"
        );
        anyhow::ensure!(
            scoped
                .subscribe_module_instance_event(
                    "lez_indexer_module",
                    "other-instance",
                    "nodeChanged",
                )
                .is_err(),
            "scoped transport accepted a different module instance"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unix_event_watch_contract_requires_process_group_cleanup() -> Result<()> {
        ensure_logoscore_event_watch_supported()
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn configured_service_runner_uses_service_owned_watch_cleanup() -> Result<()> {
        let runner = LogosCoreRunner {
            program: "logoscore".to_owned(),
            sudo_user: Some("service-account".to_owned()),
            home: Some("/var/lib/logos-node".to_owned()),
            config_dir: Some("/var/lib/logos-node/.logoscore".to_owned()),
            label: "configured logoscore".to_owned(),
        };

        let authority = WatchCleanupAuthority::for_runner(&runner);
        anyhow::ensure!(
            authority
                == WatchCleanupAuthority::ServiceIdentity {
                    user: "service-account".to_owned()
                },
            "configured service runtime lost its watch cleanup authority"
        );
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn service_watch_cleanup_passes_token_as_positional_argument() -> Result<()> {
        use std::ffi::OsStr;

        let token = "cleanup-token";
        let command =
            service_watch_cleanup_command("service-account", token, ServiceWatchTokenAction::Check);
        let args = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        anyhow::ensure!(command.get_program() == OsStr::new("sudo"));
        anyhow::ensure!(
            args == [
                "-n",
                "-u",
                "service-account",
                "/bin/sh",
                "-c",
                SERVICE_WATCH_TOKEN_CLEANUP_SCRIPT,
                "logos-inspector-watch-cleanup",
                token,
                "CHECK",
            ],
            "service watch cleanup command changed its scoped argument contract: {args:?}"
        );
        anyhow::ensure!(
            command
                .get_envs()
                .all(|(name, _)| name != OsStr::new(LOGOSCORE_WATCH_CLEANUP_TOKEN_ENV)),
            "service watch cleanup leaked its token through the command environment"
        );
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn service_watch_child_signal_targets_only_the_owned_supervisor() -> Result<()> {
        use std::ffi::OsStr;

        let command = elevated_watch_child_signal_command(431, "TERM");
        let args = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        anyhow::ensure!(command.get_program() == OsStr::new("sudo"));
        anyhow::ensure!(
            args == ["-n", "/bin/kill", "-TERM", "--", "431"],
            "service watch supervisor signal changed its scoped argument contract: {args:?}"
        );
        anyhow::ensure!(
            command
                .get_envs()
                .all(|(name, _)| name != OsStr::new(LOGOSCORE_WATCH_CLEANUP_TOKEN_ENV)),
            "service watch supervisor signal leaked its token through the command environment"
        );
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn service_watch_cleanup_rejects_invalid_process_output() -> Result<()> {
        let processes = parse_service_watch_processes(b"17\n5\n17\n", "fixture watch")?;
        anyhow::ensure!(processes == [5, 17]);
        anyhow::ensure!(
            parse_service_watch_processes(b"not-a-pid\n", "fixture watch").is_err(),
            "invalid service cleanup PID was accepted"
        );
        anyhow::ensure!(
            parse_service_watch_processes(b"0\n", "fixture watch").is_err(),
            "nonpositive service cleanup PID was accepted"
        );
        Ok(())
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
                cleanup_token: new_logoscore_watch_cleanup_token()?,
                cleanup_authority: WatchCleanupAuthority::Direct,
                lease: None,
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
                WatchCleanup {
                    token: new_logoscore_watch_cleanup_token()?,
                    authority: WatchCleanupAuthority::Direct,
                    lease: None,
                },
                "injected failed watch",
            ),
            |_child, _cleanup_token, _cleanup_authority, _label| {
                bail!("injected cleanup uncertainty")
            },
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
    fn wildcard_watch_accepts_any_nonempty_event_from_the_selected_module() -> Result<()> {
        let ready = json!({
            "type": "subscription_ready",
            "protocol": "logoscore.watch",
            "version": 1,
            "module": "delivery_module",
            "event": ""
        });
        let event = json!({
            "type": "event",
            "protocol": "logoscore.watch",
            "version": 1,
            "timestamp": "2026-07-17T12:00:00Z",
            "module": "delivery_module",
            "event": "messageSent",
            "data": {
                "arg0": "request-1",
                "arg1": "hash-1",
                "arg2": 1_784_426_733_600_168_769_u64
            }
        });

        validate_watch_ready_frame(&ready, "delivery_module", "")?;
        validate_watch_event_frame(&event, "delivery_module", "")?;
        let converted = module_transport_event_from_watch_frame(&event, "delivery_module")?;
        anyhow::ensure!(converted.module() == "delivery_module");
        anyhow::ensure!(converted.event() == "messageSent");
        anyhow::ensure!(
            converted.args()
                == [
                    Value::String("request-1".to_owned()),
                    Value::String("hash-1".to_owned()),
                    json!(1_784_426_733_600_168_769_u64),
                ]
        );
        Ok(())
    }

    #[test]
    fn wildcard_watch_decodes_utf8_qbytearray_event_arguments() -> Result<()> {
        let event = json!({
            "type": "event",
            "protocol": "logoscore.watch",
            "version": 1,
            "timestamp": "2026-07-19T03:47:07Z",
            "module": "delivery_module",
            "event": "messageReceived",
            "data": {
                "arg0": "hash-1",
                "arg1": "/test/topic",
                "arg2": { "_bytes": "eyJraW5kIjoiY29tbWVudCJ9" },
                "arg3": 1_784_432_827_841_750_528_u64
            }
        });

        let converted = module_transport_event_from_watch_frame(&event, "delivery_module")?;

        anyhow::ensure!(
            converted.args().get(2) == Some(&Value::String("{\"kind\":\"comment\"}".to_owned()))
        );
        Ok(())
    }

    #[test]
    fn wildcard_watch_rejects_malformed_qbytearray_event_arguments() {
        let event = json!({
            "type": "event",
            "protocol": "logoscore.watch",
            "version": 1,
            "timestamp": "2026-07-19T03:47:07Z",
            "module": "delivery_module",
            "event": "messageReceived",
            "data": {
                "arg0": "hash-1",
                "arg1": "/test/topic",
                "arg2": { "_bytes": "%%%" },
                "arg3": 1
            }
        });

        assert!(
            module_transport_event_from_watch_frame(&event, "delivery_module").is_err(),
            "malformed typed byte payload was accepted"
        );
    }

    #[test]
    fn wildcard_watch_command_omits_event_filter() {
        let runtime = LogoscoreCliRuntime::managed(
            "logoscore".to_owned(),
            "/tmp/logoscore-config".to_owned(),
        );
        let command = command_for_runner(
            &runtime.runner,
            [
                "watch",
                "delivery_module",
                "--json",
                "--watch-protocol",
                "v1",
            ],
        );
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            args,
            [
                "--config-dir",
                "/tmp/logoscore-config",
                "watch",
                "delivery_module",
                "--json",
                "--watch-protocol",
                "v1",
            ]
        );
    }

    #[test]
    fn wildcard_watch_rejects_inexact_readiness_and_unbounded_or_sparse_events() -> Result<()> {
        let inexact_ready = json!({
            "type": "subscription_ready",
            "protocol": "logoscore.watch",
            "version": 1,
            "module": "delivery_module",
            "event": "messageSent"
        });
        anyhow::ensure!(validate_watch_ready_frame(&inexact_ready, "delivery_module", "").is_err());

        let event_frame = |event: &str, data: Value| {
            json!({
                "type": "event",
                "protocol": "logoscore.watch",
                "version": 1,
                "timestamp": "2026-07-17T12:00:00Z",
                "module": "delivery_module",
                "event": event,
                "data": data,
            })
        };
        anyhow::ensure!(
            validate_watch_event_frame(&event_frame("", json!({})), "delivery_module", "",)
                .is_err()
        );
        anyhow::ensure!(
            validate_watch_event_frame(
                &event_frame(&"x".repeat(LOGOSCORE_EVENT_NAME_LIMIT + 1), json!({})),
                "delivery_module",
                "",
            )
            .is_err()
        );
        anyhow::ensure!(
            module_transport_event_from_watch_frame(
                &event_frame("messageSent", json!({ "arg0": 1, "arg2": 3 })),
                "delivery_module",
            )
            .is_err()
        );
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
    fn configured_service_config_reader_uses_sudo_without_shell() -> Result<()> {
        use std::ffi::OsStr;

        let runner = LogosCoreRunner {
            program: "/usr/local/bin/logoscore".to_owned(),
            sudo_user: Some("logos".to_owned()),
            home: Some("/var/lib/logos-node".to_owned()),
            config_dir: Some("/var/lib/logos-node/.logoscore".to_owned()),
            label: "configured logoscore".to_owned(),
        };
        let config_path = Path::new("/var/lib/logos-node/.logoscore/client/config.json");
        let command = runner_client_config_read_command(&runner, config_path)
            .context("configured service runner did not build config reader")?;

        anyhow::ensure!(
            command.get_program() == OsStr::new("sudo"),
            "configured service config reader bypassed sudo"
        );
        let args = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        anyhow::ensure!(
            args == [
                "-n",
                "-u",
                "logos",
                "env",
                "HOME=/var/lib/logos-node",
                "/bin/cat",
                "--",
                "/var/lib/logos-node/.logoscore/client/config.json",
            ],
            "configured service config reader arguments drifted: {args:?}"
        );
        Ok(())
    }

    #[test]
    fn client_config_reader_rejects_oversized_local_file() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let config_path = directory.path().join("config.json");
        fs::write(&config_path, vec![b'x'; LOGOSCORE_CLIENT_CONFIG_LIMIT + 1])?;
        let runner = LogosCoreRunner {
            program: "logoscore".to_owned(),
            sudo_user: None,
            home: None,
            config_dir: None,
            label: "test logoscore".to_owned(),
        };

        let error = read_runner_client_config(&runner, &config_path)
            .err()
            .context("oversized client config was accepted")?;
        anyhow::ensure!(
            error
                .to_string()
                .contains("logoscore client config exceeds 65536 byte limit"),
            "unexpected oversized client config error: {error:#}"
        );
        Ok(())
    }

    #[test]
    fn local_client_config_reader_keeps_direct_file_path() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let config_path = directory.path().join("config.json");
        let expected = br#"{"instance_id":"local"}"#;
        fs::write(&config_path, expected)?;
        let runner = LogosCoreRunner {
            program: "logoscore".to_owned(),
            sudo_user: None,
            home: None,
            config_dir: None,
            label: "test logoscore".to_owned(),
        };

        let read = read_runner_client_config(&runner, &config_path)?;
        anyhow::ensure!(read == expected, "local client config content drifted");
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
            cleanup_token: new_logoscore_watch_cleanup_token()?,
            cleanup_authority: WatchCleanupAuthority::Direct,
            lease: None,
            label: "queued terminal watch".to_owned(),
        };

        anyhow::ensure!(
            watch.next_value(&control)? == terminal,
            "queued terminal lost to concurrent cancellation"
        );
        Ok(())
    }

    #[test]
    fn event_watch_timeout_is_idle_but_closed_output_is_terminal() -> Result<()> {
        let (sender, output) = mpsc::sync_channel(1);
        let (_readiness_sender, readiness) = mpsc::channel();
        let control = CommandControl::new(
            CancellationToken::new(),
            StdInstant::now() + Duration::from_secs(1),
        );
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
            cleanup_token: new_logoscore_watch_cleanup_token()?,
            cleanup_authority: WatchCleanupAuthority::Direct,
            lease: None,
            label: "idle watch".to_owned(),
        };

        anyhow::ensure!(
            watch
                .next_value_within(&control, Duration::from_millis(1))?
                .is_none(),
            "idle event watch was treated as terminal"
        );
        drop(sender);
        let error = watch
            .next_value_within(&control, Duration::from_millis(1))
            .err()
            .context("closed event output was treated as idle")?;
        anyhow::ensure!(error.to_string().contains("output closed"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn event_watch_drains_terminal_emitted_immediately_before_exit() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-watch-exit");
        write_executable_script(
            &program,
            "#!/bin/sh\n\
             if [ \"$1\" = \"--config-dir\" ]; then shift 2; fi\n\
             printf '%s\\n' '{\"type\":\"subscription_ready\",\"protocol\":\"logoscore.watch\",\"version\":1,\"module\":\"storage_module\",\"event\":\"storageDownloadDone\"}'\n\
             printf '%s\\n' '{\"type\":\"event\",\"protocol\":\"logoscore.watch\",\"version\":1,\"timestamp\":\"2026-07-14T12:00:00Z\",\"module\":\"storage_module\",\"event\":\"storageDownloadDone\",\"data\":{\"arg0\":\"{\\\"success\\\":true,\\\"sessionId\\\":\\\"session-exit\\\"}\"}}'\n",
        )?;
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
        use nix::{sys::signal::Signal, unistd::Pid};

        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-watch-descendant");
        let descendant_path = directory.path().join("descendant.pid");
        write_executable_script(
            &program,
            "#!/bin/sh\n\
             state_dir=$2\n\
             (trap '' TERM; while :; do sleep 0.05; done) &\n\
             printf '%s' \"$!\" > \"$state_dir/descendant.pid\"\n\
             trap 'exit 0' TERM\n\
             printf '%s\\n' '{\"type\":\"subscription_ready\",\"protocol\":\"logoscore.watch\",\"version\":1,\"module\":\"storage_module\",\"event\":\"storageDownloadDone\"}'\n\
             while :; do sleep 0.05; done\n",
        )?;
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
                // ENOENT and ESRCH both mean the process is gone. Stop can
                // reap the descendant between kill and this inspection.
                Err(error)
                    if error.kind() == ErrorKind::NotFound
                        || error.raw_os_error() == Some(nix::libc::ESRCH) =>
                {
                    false
                }
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

    #[cfg(target_os = "linux")]
    #[test]
    fn event_watch_stop_kills_token_tagged_detached_descendant() -> Result<()> {
        use nix::{
            sys::signal::{Signal, kill},
            unistd::Pid,
        };

        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-watch-detached-descendant");
        let descendant_path = directory.path().join("detached.pid");
        write_executable_script(
            &program,
            "#!/bin/sh\n\
             state_dir=$2\n\
             setsid sh -c '\n\
               printf \"%s\" \"$$\" > \"$1/detached.pid\"\n\
               trap \"\" TERM\n\
               while :; do sleep 0.05; done\n\
             ' sh \"$state_dir\" &\n\
             trap 'exit 0' TERM\n\
             printf '%s\\n' '{\"type\":\"subscription_ready\",\"protocol\":\"logoscore.watch\",\"version\":1,\"module\":\"storage_module\",\"event\":\"storageDownloadDone\"}'\n\
             while :; do sleep 0.05; done\n",
        )?;
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

        let deadline = StdInstant::now() + Duration::from_secs(1);
        let descendant = loop {
            match fs::read_to_string(&descendant_path) {
                Ok(value) => {
                    break value
                        .trim()
                        .parse::<i32>()
                        .context("detached descendant fixture wrote an invalid PID")?;
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).context("failed to read detached watch descendant PID");
                }
            }
            if StdInstant::now() >= deadline {
                bail!("detached watch descendant did not report its PID");
            }
            thread::sleep(LOGOSCORE_POLL_INTERVAL);
        };
        let cleanup_token = watch.cleanup_token.clone();
        let tagged = tagged_watch_processes(&cleanup_token)?;
        anyhow::ensure!(
            tagged.contains(&descendant),
            "detached watch descendant did not retain its cleanup token: {tagged:?}"
        );

        if let Err(error) = watch.stop() {
            let _result = kill(Pid::from_raw(descendant), Signal::SIGKILL);
            return Err(error);
        }
        let deadline = StdInstant::now() + Duration::from_secs(1);
        while linux_process_is_live(descendant)? {
            if StdInstant::now() >= deadline {
                let _result = kill(Pid::from_raw(descendant), Signal::SIGKILL);
                bail!("watch cleanup left detached descendant PID {descendant} running");
            }
            thread::sleep(LOGOSCORE_POLL_INTERVAL);
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_process_start_marker_reads_current_process_start_time() -> Result<()> {
        let process = i32::try_from(std::process::id())
            .context("test process PID does not fit Linux process identity")?;
        let start_marker = linux_process_start_marker(process)?
            .context("test process did not expose a Linux start marker")?;
        anyhow::ensure!(
            start_marker > 0,
            "Linux process start marker must be the positive field 22 value"
        );
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn test_watch_owner(lease_directory: PathBuf) -> Result<LogoscoreWatchOwner> {
        fs::create_dir_all(&lease_directory)?;
        let process = std::process::id();
        let process_i32 = i32::try_from(process)
            .context("test watcher owner PID does not fit Linux process identity")?;
        let process_start_marker = linux_process_start_marker(process_i32)?
            .context("test watcher owner process is not live")?;
        anyhow::ensure!(
            process_start_marker > 0,
            "test watcher owner must use a positive Linux start marker"
        );
        Ok(LogoscoreWatchOwner {
            lease_directory,
            process,
            process_start_marker,
            launch_nonce: new_logoscore_watch_launch_nonce()?,
        })
    }

    #[cfg(target_os = "linux")]
    fn stale_watch_lease(token: String) -> WatchLeaseRecord {
        WatchLeaseRecord {
            schema_version: LOGOSCORE_WATCH_LEASE_SCHEMA_VERSION,
            token,
            // A live PID with a mismatched start marker models PID reuse.
            owner_pid: std::process::id(),
            owner_start_marker: 0,
            launch_nonce: "stale-watch-owner".to_owned(),
            cleanup_user: None,
        }
    }

    #[cfg(target_os = "linux")]
    fn wait_for_watch_exit(process: i32) -> Result<()> {
        let deadline = StdInstant::now() + Duration::from_secs(2);
        while linux_process_is_live(process)? {
            if StdInstant::now() >= deadline {
                bail!("watch process {process} remained live after cleanup");
            }
            thread::sleep(LOGOSCORE_POLL_INTERVAL);
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn owner_watch_stop_removes_persisted_lease() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let lease_directory = directory.path().join("watch-leases");
        let owner = test_watch_owner(lease_directory)?;
        let program = directory.path().join("logoscore-watch-lease");
        let owner_marker_path = directory.path().join("watch-owner");
        write_executable_script(
            &program,
            "#!/bin/sh\n\
             state_dir=$2\n\
             if [ \"$1\" = \"--config-dir\" ]; then shift 2; fi\n\
             printf '%s\\n%s\\n%s\\n%s\\n' \"$LOGOS_INSPECTOR_WATCH_TOKEN\" \"$LOGOS_INSPECTOR_WATCH_OWNER_PID\" \"$LOGOS_INSPECTOR_WATCH_OWNER_START\" \"$LOGOS_INSPECTOR_WATCH_OWNER_NONCE\" > \"$state_dir/watch-owner\"\n\
             printf '%s\\n' '{\"type\":\"subscription_ready\",\"protocol\":\"logoscore.watch\",\"version\":1,\"module\":\"storage_module\",\"event\":\"storageDownloadDone\"}'\n\
             while :; do sleep 0.05; done\n",
        )?;
        let runtime = LogoscoreCliRuntime::managed(
            program.display().to_string(),
            directory.path().display().to_string(),
        );
        let control = CommandControl::new(
            CancellationToken::new(),
            StdInstant::now() + Duration::from_secs(2),
        );
        let mut watch = runtime.start_event_watch_for_owner(
            "storage_module",
            "storageDownloadDone",
            &control,
            &owner,
        )?;
        watch.wait_ready(&control)?;
        let owner_marker = fs::read_to_string(&owner_marker_path)?;
        let mut owner_fields = owner_marker.lines();
        let (
            Some(cleanup_token),
            Some(owner_process),
            Some(owner_start_marker),
            Some(owner_nonce),
            None,
        ) = (
            owner_fields.next(),
            owner_fields.next(),
            owner_fields.next(),
            owner_fields.next(),
            owner_fields.next(),
        )
        else {
            bail!(
                "owner-backed watch did not inherit its complete owner identity: {owner_marker:?}"
            );
        };
        anyhow::ensure!(
            cleanup_token.starts_with("logos-inspector-watch-")
                && owner_process == owner.process.to_string()
                && owner_start_marker == owner.process_start_marker.to_string()
                && owner_nonce == owner.launch_nonce,
            "owner-backed watch did not inherit its complete owner identity: {owner_marker:?}"
        );
        let lease_path = watch
            .lease
            .as_ref()
            .context("owner-backed watch did not create a persisted lease")?
            .path
            .clone();
        anyhow::ensure!(
            lease_path.is_file(),
            "watch lease was not written before spawn"
        );
        watch.stop()?;
        anyhow::ensure!(
            !lease_path.exists(),
            "normal watch stop left its persisted lease behind"
        );
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn startup_recovery_reaps_stale_direct_watch() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let lease_directory = directory.path().join("watch-leases");
        fs::create_dir_all(&lease_directory)?;
        let token = new_logoscore_watch_cleanup_token()?;
        let lease_path =
            write_watch_lease_record(&lease_directory, &stale_watch_lease(token.clone()))?;
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("trap '' TERM; while :; do sleep 0.05; done")
            .env(LOGOSCORE_WATCH_CLEANUP_TOKEN_ENV, &token)
            .spawn()
            .context("failed to start stale direct watch fixture")?;
        let process = i32::try_from(child.id()).context("stale direct watch PID is too large")?;
        let result = recover_abandoned_watch_leases(&lease_directory);
        if result.is_err() || linux_process_is_live(process)? {
            let _kill_result = child.kill();
        }
        let _wait_result = child.wait();
        result?;
        wait_for_watch_exit(process)?;
        anyhow::ensure!(
            !lease_path.exists(),
            "stale direct watch recovery left its lease behind"
        );
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn startup_recovery_reaps_stale_detached_watch_descendant() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let lease_directory = directory.path().join("watch-leases");
        fs::create_dir_all(&lease_directory)?;
        let token = new_logoscore_watch_cleanup_token()?;
        let lease_path =
            write_watch_lease_record(&lease_directory, &stale_watch_lease(token.clone()))?;
        let descendant_path = directory.path().join("descendant.pid");
        let mut launcher = Command::new("sh")
            .arg("-c")
            .arg(
                "setsid sh -c 'printf \"%s\" \"$$\" > \"$1\"; trap \"\" TERM; while :; do sleep 0.05; done' sh \"$1\" &",
            )
            .arg("sh")
            .arg(&descendant_path)
            .env(LOGOSCORE_WATCH_CLEANUP_TOKEN_ENV, &token)
            .spawn()
            .context("failed to start stale detached watch launcher")?;
        let launcher_status = launcher.wait()?;
        anyhow::ensure!(launcher_status.success(), "stale watch launcher failed");
        let deadline = StdInstant::now() + Duration::from_secs(1);
        let descendant = loop {
            match fs::read_to_string(&descendant_path) {
                Ok(value) => {
                    break value
                        .trim()
                        .parse::<i32>()
                        .context("stale detached watch wrote an invalid PID")?;
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("failed to read stale detached watch PID"),
            }
            if StdInstant::now() >= deadline {
                bail!("stale detached watch did not report its PID");
            }
            thread::sleep(LOGOSCORE_POLL_INTERVAL);
        };
        let result = recover_abandoned_watch_leases(&lease_directory);
        if result.is_err() || linux_process_is_live(descendant)? {
            let _kill_result = nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(descendant),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
        result?;
        wait_for_watch_exit(descendant)?;
        anyhow::ensure!(
            !lease_path.exists(),
            "stale detached watch recovery left its lease behind"
        );
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn startup_recovery_preserves_live_owner_watch() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let lease_directory = directory.path().join("watch-leases");
        fs::create_dir_all(&lease_directory)?;
        let owner = test_watch_owner(lease_directory.clone())?;
        let token = new_logoscore_watch_cleanup_token()?;
        let lease = owner
            .register(&token, &WatchCleanupAuthority::Direct)?
            .context("live owner did not create a watch lease")?;
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("trap '' TERM; while :; do sleep 0.05; done")
            .env(LOGOSCORE_WATCH_CLEANUP_TOKEN_ENV, &token)
            .spawn()
            .context("failed to start live owner watch fixture")?;
        let process = i32::try_from(child.id()).context("live owner watch PID is too large")?;
        let result = recover_abandoned_watch_leases(&lease_directory);
        let retained = linux_process_is_live(process)? && lease.path.exists();
        let cleanup_result = stop_direct_tagged_watch_processes(&token, "live owner watch fixture");
        let _wait_result = child.wait();
        let release_result = lease.release();
        result?;
        cleanup_result?;
        release_result?;
        anyhow::ensure!(
            retained,
            "startup recovery interrupted a watch belonging to a live owner"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn shared_download_workspace_is_group_writable() -> Result<()> {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

        let directory = tempfile::tempdir()?;
        let group = fs::metadata(directory.path())?.gid();
        let shared_transport = SharedFilesystemTransport { group };

        shared_transport.share_directory(directory.path(), 0o770)?;

        let mode = fs::metadata(directory.path())?.permissions().mode() & 0o777;
        anyhow::ensure!(
            mode == 0o770,
            "Storage V2 download workspace must grant shared-group write access, got {mode:o}"
        );
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

        fn subscribe_module_instance_event(
            &self,
            module: &str,
            instance_id: &str,
            event: &str,
        ) -> ModuleTransportResult<BoxedModuleEventSubscription> {
            self.subscriptions
                .lock()
                .map_err(|error| anyhow::anyhow!("recording subscriptions lock failed: {error}"))?
                .push((module.to_owned(), instance_id.to_owned(), event.to_owned()));
            Ok(Box::new(EmptyModuleEventSubscription))
        }
    }

    struct EmptyModuleEventSubscription;

    impl ModuleEventSubscription for EmptyModuleEventSubscription {
        fn next_within(&mut self, _timeout: Duration) -> Result<Option<ModuleTransportEvent>> {
            Ok(None)
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn controlled_cli_call_does_not_overclaim_remote_termination() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-test");
        let pid_path = directory.path().join("logoscore-test.pid");
        write_executable_script(
            &program,
            r#"#!/bin/sh
case "$1" in
    list-modules)
        printf '%s\n' '{"modules":[{"name":"storage_module","status":"loaded"}]}'
        ;;
    call)
        printf '%s' "$$" > "${0}.pid"
        while :; do :; done
        ;;
esac
"#,
        )?;
        let transport = LogoscoreCliTransport {
            runtime: LogoscoreRuntimeBinding::Fixed(LogoscoreCliRuntime {
                runner: LogosCoreRunner {
                    program: program.to_string_lossy().into_owned(),
                    sudo_user: None,
                    home: None,
                    config_dir: None,
                    label: "test logoscore".to_owned(),
                },
            }),
            close_cancellation: CancellationToken::new(),
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
    async fn closing_cli_transport_cancels_an_ordinary_pinned_call() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-close-test");
        let pid_path = directory.path().join("logoscore-close-test.pid");
        write_executable_script(
            &program,
            r#"#!/bin/sh
case "$1" in
    list-modules)
        printf '%s\n' '{"modules":[{"name":"storage_module","status":"loaded"}]}'
        ;;
    call)
        printf '%s' "$$" > "${0}.pid"
        while :; do :; done
        ;;
esac
"#,
        )?;
        let transport = LogoscoreCliTransport {
            runtime: LogoscoreRuntimeBinding::Fixed(LogoscoreCliRuntime {
                runner: LogosCoreRunner {
                    program: program.to_string_lossy().into_owned(),
                    sudo_user: None,
                    home: None,
                    config_dir: None,
                    label: "test logoscore".to_owned(),
                },
            }),
            close_cancellation: CancellationToken::new(),
        };
        let pinned = pin_module_transport(Arc::new(transport.clone()))?;
        let close_request = transport.clone();
        let pid_for_close = pid_path.clone();
        let closer = tokio::spawn(async move {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !pid_for_close.exists() {
                if Instant::now() >= deadline {
                    bail!("timed out waiting for ordinary CLI call process");
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            close_request.begin_close();
            Ok::<(), anyhow::Error>(())
        });
        let call = ModuleCall::new(
            ModuleTransportKind::LogoscoreCli,
            "storage_module",
            "get",
            vec![],
        )?;
        let started = Instant::now();

        let Err(error) = pinned.call(call).await else {
            bail!("closing transport let ordinary CLI call complete");
        };
        closer.await.context("CLI closer task failed")??;
        anyhow::ensure!(
            started.elapsed() < Duration::from_secs(3),
            "ordinary CLI call did not stop promptly after close: {:?}",
            started.elapsed()
        );
        anyhow::ensure!(
            format!("{error:#}").contains("cancellation requested"),
            "ordinary CLI close lost cancellation evidence: {error:#}"
        );
        let pid = fs::read_to_string(&pid_path)?;
        let alive = Command::new("sh")
            .arg("-c")
            .arg("kill -0 \"$1\" 2>/dev/null")
            .arg("logoscore-reap-probe")
            .arg(pid.trim())
            .status()?;
        anyhow::ensure!(!alive.success(), "ordinary CLI child was not reaped");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn controlled_call_retries_transient_module_metadata_before_single_invocation() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-retry-metadata");
        write_executable_script(
            &program,
            r#"#!/bin/sh
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
case "$1" in
    list-modules)
        printf '%s\n' '{"modules":[{"name":"storage_module","status":"loaded"}]}'
        ;;
    module-info)
        count_path="$config_dir/module-info-count"
        count=0
        if [ -f "$count_path" ]; then
            count="$(cat "$count_path")"
        fi
        count=$((count + 1))
        printf '%s' "$count" > "$count_path"
        if [ "$count" -eq 1 ]; then
            printf '%s\n' '{"code":"RPC_FAILED","message":"storage replica is starting","status":"error"}'
            exit 4
        fi
        printf '%s\n' '{"name":"storage_module","methods":[{"isInvokable":true,"name":"init","signature":"init(QString)"}]}'
        ;;
    call)
        printf '%s\n' "$3" >> "$config_dir/calls"
        printf '%s\n' '{"module":"storage_module","method":"init","result":{"success":true,"value":"ready"},"status":"ok"}'
        ;;
esac
"#,
        )?;
        let runtime = LogoscoreCliRuntime::managed(
            program.display().to_string(),
            directory.path().display().to_string(),
        );
        let control = CommandControl::new(
            CancellationToken::new(),
            StdInstant::now() + Duration::from_secs(15),
        );

        let result = runtime.call_checked_controlled(
            "storage_module",
            "init",
            "init(QString)",
            &["@/tmp/storage.json".to_owned()],
            control,
        )?;

        anyhow::ensure!(
            result
                .pointer("/value/result/value")
                .and_then(Value::as_str)
                == Some("ready"),
            "retrying metadata did not return the single Storage invocation result: {result}"
        );
        anyhow::ensure!(
            fs::read_to_string(directory.path().join("module-info-count"))?.trim() == "2",
            "module metadata was not retried exactly once"
        );
        anyhow::ensure!(
            fs::read_to_string(directory.path().join("calls"))?
                .lines()
                .eq(["init"]),
            "Storage init was retried after metadata recovery"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn controlled_module_discovery_retries_a_timed_out_metadata_probe() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-timeout-metadata");
        write_executable_script(
            &program,
            r#"#!/bin/sh
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
case "$1" in
    list-modules)
        printf '%s\n' '{"modules":[{"name":"storage_module","status":"loaded"}]}'
        ;;
    module-info)
        count_path="$config_dir/module-info-count"
        count=0
        if [ -f "$count_path" ]; then
            count="$(cat "$count_path")"
        fi
        count=$((count + 1))
        printf '%s' "$count" > "$count_path"
        if [ "$count" -eq 1 ]; then
            sleep 1
        fi
        printf '%s\n' '{"name":"storage_module","methods":[{"isInvokable":true,"name":"init","signature":"init(QString)"}]}'
        ;;
esac
"#,
        )?;
        let runtime = LogoscoreCliRuntime::managed(
            program.display().to_string(),
            directory.path().display().to_string(),
        );
        let control = CommandControl::new(
            CancellationToken::new(),
            StdInstant::now() + Duration::from_secs(2),
        )
        .with_isolated_test_budget();

        let discovery = runtime.discover_module_controlled_with(
            "storage_module",
            control,
            Duration::from_millis(100),
            Duration::from_millis(10),
        )?;

        discovery.require_method("init", "init(QString)")?;
        anyhow::ensure!(
            fs::read_to_string(directory.path().join("module-info-count"))?.trim() == "2",
            "timed-out metadata probe was not retried"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn controlled_discovery_never_queries_unloaded_module_metadata() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-unloaded-metadata");
        write_executable_script(
            &program,
            r#"#!/bin/sh
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
case "$1" in
    list-modules)
        printf '%s\n' '{"modules":[{"name":"lez_indexer_module","status":"not_loaded"}]}'
        ;;
    module-info)
        touch "$config_dir/unsafe-module-info"
        printf '%s\n' '{"name":"lez_indexer_module","methods":[{"isInvokable":true,"name":"getStatus","signature":"getStatus()"}]}'
        ;;
esac
"#,
        )?;
        let runtime = LogoscoreCliRuntime::managed(
            program.display().to_string(),
            directory.path().display().to_string(),
        );
        let control = CommandControl::new(
            CancellationToken::new(),
            StdInstant::now() + Duration::from_secs(5),
        );

        let error = runtime
            .require_module_method_controlled_once(
                "lez_indexer_module",
                "getStatus",
                "getStatus()",
                control,
            )
            .err()
            .context("unloaded module discovery unexpectedly succeeded")?;

        anyhow::ensure!(
            error.to_string().contains("not loaded"),
            "unloaded module failure lost status: {error:#}"
        );
        anyhow::ensure!(
            !directory.path().join("unsafe-module-info").exists(),
            "discovery queried metadata from an unloaded module"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cli_requests_for_one_runtime_do_not_overlap() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-serialized-requests");
        write_executable_script(
            &program,
            r#"#!/bin/sh
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
if [ "$1" != "status" ]; then
    printf '%s\n' '{"code":"UNEXPECTED","status":"error"}'
    exit 2
fi
if mkdir "$config_dir/in-flight" 2>/dev/null; then
    touch "$config_dir/entered"
    while [ ! -e "$config_dir/release" ]; do
        sleep 0.01
    done
    rmdir "$config_dir/in-flight"
    printf '%s\n' '{"status":"ok"}'
else
    touch "$config_dir/concurrent"
    printf '%s\n' '{"code":"CONCURRENT","status":"error"}'
    exit 3
fi
"#,
        )?;
        let runtime = LogoscoreCliRuntime::managed(
            program.display().to_string(),
            directory.path().display().to_string(),
        );
        let first_runtime = runtime.clone();
        let first =
            thread::spawn(move || first_runtime.status_with_timeout(Duration::from_secs(3)));

        let entered = directory.path().join("entered");
        let entered_deadline = StdInstant::now() + Duration::from_secs(1);
        while !entered.exists() {
            anyhow::ensure!(
                StdInstant::now() < entered_deadline,
                "first CLI request did not enter the fake runtime"
            );
            thread::sleep(Duration::from_millis(5));
        }

        let second_runtime = runtime.clone();
        let second =
            thread::spawn(move || second_runtime.status_with_timeout(Duration::from_secs(3)));
        thread::sleep(Duration::from_millis(100));
        fs::write(directory.path().join("release"), "release")?;

        first
            .join()
            .map_err(|_| anyhow::anyhow!("first CLI request thread panicked"))??;
        second
            .join()
            .map_err(|_| anyhow::anyhow!("second CLI request thread panicked"))??;
        anyhow::ensure!(
            !directory.path().join("concurrent").exists(),
            "same-runtime CLI requests overlapped"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn loaded_preflight_and_call_hold_one_runtime_gate() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-atomic-loaded-call");
        write_executable_script(
            &program,
            r#"#!/bin/sh
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
case "$1" in
    list-modules)
        printf '%s\n' list-modules >> "$config_dir/sequence"
        touch "$config_dir/listed"
        while [ ! -f "$config_dir/release-list" ]; do sleep 0.01; done
        printf '%s\n' '{"modules":[{"name":"lez_indexer_module","status":"loaded"}]}'
        ;;
    call)
        printf '%s\n' call >> "$config_dir/sequence"
        if [ -f "$config_dir/unloaded" ]; then touch "$config_dir/unsafe-call"; fi
        printf '%s\n' '{"method":"getStatus","module":"lez_indexer_module","result":"{\"state\":\"stopped\"}","status":"ok"}'
        ;;
    unload-module)
        printf '%s\n' unload-module >> "$config_dir/sequence"
        touch "$config_dir/unloaded"
        printf '%s\n' '{"module":"lez_indexer_module","status":"ok"}'
        ;;
esac
"#,
        )?;
        let runtime = LogoscoreCliRuntime::managed(
            program.display().to_string(),
            directory.path().display().to_string(),
        );
        let call_runtime = runtime.clone();
        let call = thread::spawn(move || call_runtime.call("lez_indexer_module", "getStatus", &[]));

        let listed = directory.path().join("listed");
        let listed_deadline = StdInstant::now() + Duration::from_secs(2);
        while !listed.exists() {
            anyhow::ensure!(
                StdInstant::now() < listed_deadline,
                "loaded-state preflight did not start"
            );
            thread::sleep(Duration::from_millis(5));
        }
        let unload_runtime = runtime.clone();
        let unload = thread::spawn(move || {
            let control = CommandControl::new(
                CancellationToken::new(),
                StdInstant::now() + Duration::from_secs(5),
            );
            unload_runtime.unload_module_controlled("lez_indexer_module", control)
        });
        thread::sleep(Duration::from_millis(100));
        anyhow::ensure!(
            !directory.path().join("unloaded").exists(),
            "unload interleaved while loaded-state call gate was held"
        );
        fs::write(directory.path().join("release-list"), "release")?;

        call.join()
            .map_err(|_| anyhow::anyhow!("loaded module call thread panicked"))??;
        unload
            .join()
            .map_err(|_| anyhow::anyhow!("module unload thread panicked"))??;
        anyhow::ensure!(
            fs::read_to_string(directory.path().join("sequence"))?
                .lines()
                .eq(["list-modules", "call", "unload-module"]),
            "module unload interleaved between loaded preflight and call"
        );
        anyhow::ensure!(
            !directory.path().join("unsafe-call").exists(),
            "module call ran after the checked module was unloaded"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn controlled_module_discovery_surfaces_exhausted_attempt_timeout() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-exhausted-metadata");
        write_executable_script(
            &program,
            r#"#!/bin/sh
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
case "$1" in
    list-modules)
        printf '%s\n' '{"modules":[{"name":"storage_module","status":"loaded"}]}'
        ;;
    module-info)
        count_path="$config_dir/module-info-count"
        count=0
        if [ -f "$count_path" ]; then
            count="$(cat "$count_path")"
        fi
        printf '%s' "$((count + 1))" > "$count_path"
        sleep 1
        printf '%s\n' '{"name":"storage_module","methods":[{"isInvokable":true,"name":"init","signature":"init(QString)"}]}'
        ;;
esac
"#,
        )?;
        let runtime = LogoscoreCliRuntime::managed(
            program.display().to_string(),
            directory.path().display().to_string(),
        );
        let control = CommandControl::new(
            CancellationToken::new(),
            StdInstant::now() + Duration::from_secs(5),
        )
        .with_isolated_test_budget();

        let Err(error) = runtime.discover_module_controlled_with(
            "storage_module",
            control.clone(),
            Duration::from_millis(250),
            Duration::from_millis(10),
        ) else {
            bail!("exhausted module metadata probes unexpectedly succeeded");
        };

        control.check_active()?;
        anyhow::ensure!(
            error.downcast_ref::<CommandTerminated>().is_none(),
            "child metadata timeout was exposed as a parent interruption: {error:#}"
        );
        let detail = format!("{error:#}");
        anyhow::ensure!(
            detail.contains("storage_module")
                && detail.contains("command stopped after deadline exceeded"),
            "exhausted metadata failure lost diagnostics: {detail}"
        );
        anyhow::ensure!(
            fs::read_to_string(directory.path().join("module-info-count"))?.trim() == "3",
            "module metadata did not use all bounded probes"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn controlled_module_discovery_preserves_parent_deadline() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-parent-metadata-deadline");
        write_executable_script(
            &program,
            r#"#!/bin/sh
if [ "$1" = "--config-dir" ]; then
    shift 2
fi
case "$1" in
    list-modules)
        printf '%s\n' '{"modules":[{"name":"storage_module","status":"loaded"}]}'
        ;;
    module-info)
        sleep 1
        ;;
esac
"#,
        )?;
        let runtime = LogoscoreCliRuntime::managed(
            program.display().to_string(),
            directory.path().display().to_string(),
        );
        let control = CommandControl::new(
            CancellationToken::new(),
            StdInstant::now() + Duration::from_millis(100),
        );

        let Err(error) = runtime.discover_module_controlled_with(
            "storage_module",
            control,
            Duration::from_secs(1),
            Duration::from_millis(10),
        ) else {
            bail!("parent-deadline module metadata probe unexpectedly succeeded");
        };

        let termination = error
            .downcast_ref::<CommandTerminated>()
            .context("parent deadline was converted into a normal metadata failure")?;
        anyhow::ensure!(
            termination.reason() == CommandStopReason::DeadlineExceeded,
            "module metadata ended for the wrong reason"
        );
        Ok(())
    }

    #[test]
    fn controlled_cli_request_waiting_for_gate_preserves_deadline() -> Result<()> {
        let gate = LogoscoreCliCommandGate::default();
        let held = gate
            .lock
            .lock()
            .map_err(|_| anyhow::anyhow!("test command gate is poisoned"))?;
        let control = CommandControl::new(
            CancellationToken::new(),
            StdInstant::now() + Duration::from_millis(10),
        );

        let Err(error) = acquire_logoscore_cli_command_gate(&gate, Some(&control), None) else {
            bail!("controlled CLI request acquired an occupied command gate");
        };
        drop(held);
        let termination = error
            .downcast_ref::<CommandTerminated>()
            .context("controlled CLI gate wait lost typed termination evidence")?;
        anyhow::ensure!(
            termination.reason() == CommandStopReason::DeadlineExceeded,
            "controlled CLI gate wait ended for the wrong reason"
        );
        Ok(())
    }

    #[test]
    fn controlled_cli_request_is_not_starved_by_uncontrolled_barging() -> Result<()> {
        let gate = Arc::new(LogoscoreCliCommandGate::default());
        let held = gate
            .lock
            .lock()
            .map_err(|_| anyhow::anyhow!("test command gate is poisoned"))?;
        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(1);
        let controlled_gate = Arc::clone(&gate);
        let controlled = thread::spawn(move || -> Result<()> {
            let control = CommandControl::new(
                CancellationToken::new(),
                StdInstant::now() + Duration::from_millis(120),
            );
            started_tx
                .send(())
                .map_err(|_| anyhow::anyhow!("controlled gate fixture did not start"))?;
            let permit =
                acquire_logoscore_cli_command_gate(&controlled_gate, Some(&control), None)?;
            drop(permit);
            Ok(())
        });
        started_rx
            .recv_timeout(Duration::from_millis(50))
            .context("controlled gate fixture did not report startup")?;
        thread::sleep(Duration::from_millis(25));
        drop(held);

        let uncontrolled = acquire_logoscore_cli_command_gate(
            &gate,
            None,
            Some(StdInstant::now() + Duration::from_millis(500)),
        )?;
        thread::sleep(Duration::from_millis(150));
        drop(uncontrolled);

        controlled
            .join()
            .map_err(|_| anyhow::anyhow!("controlled gate fixture panicked"))??;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cli_snapshot_coalesces_concurrent_status_and_module_inventory_queries() -> Result<()> {
        use std::sync::{Arc, Barrier};

        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-snapshot");
        write_executable_script(
            &program,
            r#"#!/bin/sh
set -eu
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
case "$1" in
    status)
        printf '%s\n' status >> "$config_dir/commands"
        printf '%s\n' '{"daemon":{"status":"running"},"modules":[{"name":"storage_module","status":"loaded"},{"name":"delivery_module","status":"not_loaded"}]}'
        ;;
    list-modules)
        printf '%s\n' list-modules >> "$config_dir/commands"
        printf '%s\n' '{"modules":[{"name":"storage_module","status":"loaded"}]}'
        ;;
    call)
        printf '%s\n' call >> "$config_dir/commands"
        printf '%s\n' '{"module":"storage_module","method":"get","result":{"value":"ok"},"status":"ok"}'
        ;;
esac
"#,
        )?;
        let runtime = LogoscoreCliRuntime::managed(
            program.display().to_string(),
            directory.path().display().to_string(),
        );
        let barrier = Arc::new(Barrier::new(3));
        let mut workers = Vec::new();
        for _ in 0..2 {
            let worker_runtime = runtime.clone();
            let worker_barrier = Arc::clone(&barrier);
            workers.push(thread::spawn(move || -> Result<LogosCoreOutput> {
                worker_barrier.wait();
                worker_runtime.status_with_timeout(Duration::from_secs(3))
            }));
        }
        barrier.wait();
        for worker in workers {
            let status = worker
                .join()
                .map_err(|_| anyhow::anyhow!("snapshot worker panicked"))??;
            anyhow::ensure!(
                status
                    .value
                    .pointer("/daemon/status")
                    .and_then(Value::as_str)
                    == Some("running"),
                "cached status returned the wrong payload"
            );
        }

        runtime.call("storage_module", "get", &[])?;
        runtime.call("storage_module", "get", &[])?;
        let first_stopped = runtime
            .call("delivery_module", "get", &[])
            .err()
            .context("stopped module unexpectedly accepted a call")?;
        let second_stopped = runtime
            .call("delivery_module", "get", &[])
            .err()
            .context("cached stopped module unexpectedly accepted a call")?;
        anyhow::ensure!(
            first_stopped.to_string().contains("not loaded")
                && second_stopped.to_string().contains("not loaded"),
            "cached stopped module lost its diagnostic"
        );

        let commands = fs::read_to_string(directory.path().join("commands"))?;
        anyhow::ensure!(
            commands
                .lines()
                .filter(|command| *command == "status")
                .count()
                == 1,
            "concurrent status consumers launched duplicate commands: {commands}"
        );
        anyhow::ensure!(
            commands
                .lines()
                .filter(|command| *command == "list-modules")
                .count()
                == 0,
            "status inventory did not satisfy same-epoch module calls: {commands}"
        );
        anyhow::ensure!(
            commands
                .lines()
                .filter(|command| *command == "call")
                .count()
                == 2,
            "snapshot changed the requested module-call count: {commands}"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cli_status_uses_a_fresh_snapshot_without_waiting_for_the_command_gate() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-snapshot-fast-path");
        write_executable_script(
            &program,
            r#"#!/bin/sh
set -eu
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
if [ "$1" = "status" ]; then
    printf '%s\n' status >> "$config_dir/commands"
    printf '%s\n' '{"daemon":{"status":"running"}}'
fi
"#,
        )?;
        let runtime = LogoscoreCliRuntime::managed(
            program.display().to_string(),
            directory.path().display().to_string(),
        );

        runtime.status_with_timeout(Duration::from_secs(1))?;
        let gate = logoscore_cli_command_gate(&runtime.runner)?;
        let held = gate
            .lock
            .lock()
            .map_err(|_| anyhow::anyhow!("test command gate is poisoned"))?;
        let status = runtime.status_with_timeout(Duration::from_millis(20))?;
        drop(held);

        anyhow::ensure!(
            status
                .value
                .pointer("/daemon/status")
                .and_then(Value::as_str)
                == Some("running"),
            "fresh status snapshot returned an unexpected payload"
        );
        anyhow::ensure!(
            fs::read_to_string(directory.path().join("commands"))?
                .lines()
                .eq(["status"]),
            "fresh status snapshot executed another command while the gate was held"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cli_status_accepts_structured_not_running_output_after_nonzero_exit() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-not-running-status");
        write_executable_script(
            &program,
            r#"#!/bin/sh
set -eu
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
if [ "$1" = "status" ] && [ "$2" = "--json" ]; then
    printf '%s\n' status >> "$config_dir/commands"
    printf '%s\n' '{"daemon":{"status":"not_running"},"rpc_error":"core_service not reachable"}'
    exit 1
fi
exit 9
"#,
        )?;
        let runtime = LogoscoreCliRuntime::local(
            program.display().to_string(),
            directory.path().display().to_string(),
        );

        let status = runtime.status_with_timeout(Duration::from_secs(1))?;
        anyhow::ensure!(
            status
                .value
                .pointer("/daemon/status")
                .and_then(Value::as_str)
                == Some("not_running"),
            "known stopped status was not preserved: {}",
            status.value
        );
        runtime.status_with_timeout(Duration::from_secs(1))?;
        anyhow::ensure!(
            fs::read_to_string(directory.path().join("commands"))?
                .lines()
                .eq(["status"]),
            "recognized not-running status was not cached"
        );

        let direct = runtime.status_probe_with_timeout(Duration::from_secs(1))?;
        anyhow::ensure!(
            direct
                .value
                .pointer("/daemon/status")
                .and_then(Value::as_str)
                == Some("not_running"),
            "direct status probe discarded the known stopped state"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cli_status_rejects_an_unrecognized_nonzero_response() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-invalid-nonzero-status");
        write_executable_script(
            &program,
            r#"#!/bin/sh
if [ "$1" = "--config-dir" ]; then
    shift 2
fi
printf '%s\n' '{"daemon":{"status":"running"}}'
exit 1
"#,
        )?;
        let runtime = LogoscoreCliRuntime::local(
            program.display().to_string(),
            directory.path().display().to_string(),
        );
        let error = runtime
            .status_probe_with_timeout(Duration::from_secs(1))
            .err()
            .context("nonzero running status was unexpectedly accepted")?;
        anyhow::ensure!(
            error.to_string().contains("exited with")
                && error.to_string().contains("\"status\":\"running\""),
            "unexpected nonzero status error: {error:#}"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cli_module_inventory_waits_for_the_command_gate_even_when_cached() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-module-gate");
        write_executable_script(
            &program,
            r#"#!/bin/sh
set -eu
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
if [ "$1" = "list-modules" ]; then
    printf '%s\n' list-modules >> "$config_dir/commands"
    printf '%s\n' '{"modules":[]}'
fi
"#,
        )?;
        let runtime = LogoscoreCliRuntime::managed(
            program.display().to_string(),
            directory.path().display().to_string(),
        );

        runtime.list_modules()?;
        let gate = logoscore_cli_command_gate(&runtime.runner)?;
        let held = gate
            .lock
            .lock()
            .map_err(|_| anyhow::anyhow!("test command gate is poisoned"))?;
        let error = runtime
            .cached_json(
                LogoscoreCliSnapshotKind::Modules,
                ["list-modules", "--json"],
                Duration::from_millis(20),
            )
            .err()
            .context("cached module inventory bypassed the command gate")?;
        drop(held);

        anyhow::ensure!(
            format!("{error:#}").contains("timed out waiting for another request"),
            "module inventory returned the wrong gate error: {error:#}"
        );
        anyhow::ensure!(
            fs::read_to_string(directory.path().join("commands"))?
                .lines()
                .eq(["list-modules"]),
            "blocked module inventory executed another command"
        );
        Ok(())
    }

    #[test]
    fn cli_snapshot_uses_runtime_and_inventory_specific_freshness() -> Result<()> {
        let now = StdInstant::now();
        let status_within_poll_window = LogoscoreCliSnapshotEntry {
            observed_at: now
                .checked_sub(Duration::from_secs(19))
                .context("status test instant underflowed")?,
            result: LogoscoreCliSnapshotResult::Output(LogosCoreOutput {
                runner: "fixture".to_owned(),
                value: json!({"daemon": {"status": "running"}}),
                stderr: None,
            }),
        };
        let status_beyond_poll_window = LogoscoreCliSnapshotEntry {
            observed_at: now
                .checked_sub(Duration::from_secs(21))
                .context("expired status test instant underflowed")?,
            result: status_within_poll_window.result.clone(),
        };
        let inventory_beyond_status_window = LogoscoreCliSnapshotEntry {
            observed_at: now
                .checked_sub(Duration::from_secs(21))
                .context("inventory test instant underflowed")?,
            result: LogoscoreCliSnapshotResult::Output(LogosCoreOutput {
                runner: "fixture".to_owned(),
                value: json!({"modules": []}),
                stderr: None,
            }),
        };
        let expired_inventory = LogoscoreCliSnapshotEntry {
            observed_at: now
                .checked_sub(Duration::from_secs(31))
                .context("expired inventory test instant underflowed")?,
            result: inventory_beyond_status_window.result.clone(),
        };

        anyhow::ensure!(
            status_within_poll_window.is_fresh(LogoscoreCliSnapshotKind::Status, now),
            "status snapshot expired inside one polling window"
        );
        anyhow::ensure!(
            !status_beyond_poll_window.is_fresh(LogoscoreCliSnapshotKind::Status, now),
            "status snapshot remained fresh beyond its documented limit"
        );
        anyhow::ensure!(
            inventory_beyond_status_window.is_fresh(LogoscoreCliSnapshotKind::Modules, now),
            "module inventory did not survive one UI refresh epoch"
        );
        anyhow::ensure!(
            !expired_inventory.is_fresh(LogoscoreCliSnapshotKind::Modules, now),
            "module inventory remained fresh beyond its documented limit"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cli_snapshot_backs_off_failures_and_explicit_invalidation_retries() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-snapshot-failure");
        write_executable_script(
            &program,
            r#"#!/bin/sh
set -eu
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
if [ "$1" = "status" ]; then
    printf '%s\n' status >> "$config_dir/commands"
    printf '%s\n' '{"code":"RPC_FAILED","message":"daemon unavailable","status":"error"}'
    exit 4
fi
"#,
        )?;
        let runtime = LogoscoreCliRuntime::managed(
            program.display().to_string(),
            directory.path().display().to_string(),
        );

        let first = runtime
            .status()
            .err()
            .context("failing status fixture unexpectedly succeeded")?;
        let second = runtime
            .status()
            .err()
            .context("cached status failure unexpectedly succeeded")?;
        anyhow::ensure!(
            format!("{first:#}").contains("daemon unavailable")
                && format!("{second:#}").contains("daemon unavailable"),
            "cached failure lost its diagnostic"
        );
        anyhow::ensure!(
            fs::read_to_string(directory.path().join("commands"))?
                .lines()
                .count()
                == 1,
            "failure backoff relaunched LogosCore"
        );

        runtime.invalidate_cli_snapshot()?;
        let _retry = runtime
            .status()
            .err()
            .context("invalidated status failure unexpectedly succeeded")?;
        anyhow::ensure!(
            fs::read_to_string(directory.path().join("commands"))?
                .lines()
                .count()
                == 2,
            "explicit invalidation did not retry LogosCore status"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cli_module_lifecycle_invalidates_cached_inventory() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-snapshot-lifecycle");
        write_executable_script(
            &program,
            r#"#!/bin/sh
set -eu
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
case "$1" in
    list-modules)
        printf '%s\n' list-modules >> "$config_dir/commands"
        status="$(cat "$config_dir/module-status")"
        printf '{"modules":[{"name":"storage_module","status":"%s"}]}\n' "$status"
        ;;
    unload-module)
        printf '%s' not_loaded > "$config_dir/module-status"
        printf '%s\n' '{"status":"ok"}'
        ;;
esac
"#,
        )?;
        fs::write(directory.path().join("module-status"), "loaded")?;
        let runtime = LogoscoreCliRuntime::managed(
            program.display().to_string(),
            directory.path().display().to_string(),
        );

        let first = runtime.list_modules()?;
        let cached = runtime.list_modules()?;
        anyhow::ensure!(
            listed_module_status("storage_module", &first.value)? == "loaded"
                && listed_module_status("storage_module", &cached.value)? == "loaded",
            "initial module inventory was not reused"
        );
        runtime.unload_module("storage_module")?;
        let refreshed = runtime.list_modules()?;
        anyhow::ensure!(
            listed_module_status("storage_module", &refreshed.value)? == "not_loaded",
            "module lifecycle invalidation served stale inventory"
        );
        anyhow::ensure!(
            fs::read_to_string(directory.path().join("commands"))?
                .lines()
                .count()
                == 2,
            "module inventory did not refresh exactly once after lifecycle mutation"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn controlled_call_preserves_unready_metadata_error_without_invoking_mutation() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-unready-metadata");
        write_executable_script(
            &program,
            r#"#!/bin/sh
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
case "$1" in
    list-modules)
        printf '%s\n' '{"modules":[{"name":"storage_module","status":"loaded"}]}'
        ;;
    module-info)
        printf '%s\n' '{"code":"RPC_FAILED","message":"storage replica is unavailable","status":"error"}'
        exit 4
        ;;
    call)
        touch "$config_dir/mutation-invoked"
        ;;
esac
"#,
        )?;
        let runtime = LogoscoreCliRuntime::managed(
            program.display().to_string(),
            directory.path().display().to_string(),
        );
        let control = CommandControl::new(
            CancellationToken::new(),
            StdInstant::now() + Duration::from_secs(15),
        );

        let error = runtime
            .call_checked_controlled(
                "storage_module",
                "init",
                "init(QString)",
                &["@/tmp/storage.json".to_owned()],
                control,
            )
            .err()
            .context("unready metadata unexpectedly invoked Storage init")?;

        anyhow::ensure!(
            format!("{error:#}").contains("RPC_FAILED"),
            "unready metadata error lost its CLI cause: {error:#}"
        );
        anyhow::ensure!(
            !directory.path().join("mutation-invoked").exists(),
            "Storage init ran despite failed module metadata"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn pre_canceled_cli_call_reports_that_no_external_process_started() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let marker = directory.path().join("unexpected-start");
        let transport = LogoscoreCliTransport {
            runtime: LogoscoreRuntimeBinding::Fixed(LogoscoreCliRuntime {
                runner: LogosCoreRunner {
                    program: marker.to_string_lossy().into_owned(),
                    sudo_user: None,
                    home: None,
                    config_dir: None,
                    label: "test logoscore".to_owned(),
                },
            }),
            close_cancellation: CancellationToken::new(),
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
    async fn controlled_dispatch_preserves_json_arguments_and_transport_identity() -> Result<()> {
        let transport = RecordingTransport::new(
            ModuleTransportKind::LogoscoreCli,
            ModuleTransportKind::LogoscoreCli,
        );
        let args = vec![json!({ "range": [10, 20, 2] })];
        let call = ModuleCall::new(
            ModuleTransportKind::LogoscoreCli,
            "blockchain_module",
            "get_finalized_blocks_range",
            args.clone(),
        )?;
        let control = ModuleCallControl::new(
            CancellationToken::new(),
            Instant::now() + Duration::from_secs(5),
            Arc::new(AtomicU8::new(1)),
        )
        .with_json_output_limit(LOGOSCORE_MAX_JSON_OUTPUT_LIMIT)?;

        let reply = dispatch_module_call_controlled(&transport, call, control).await?;
        anyhow::ensure!(reply.transport() == ModuleTransportKind::LogoscoreCli);
        let recorded = transport
            .last_call
            .lock()
            .map_err(|error| anyhow::anyhow!("recording transport lock failed: {error}"))?
            .clone()
            .context("controlled dispatch did not invoke the transport")?;
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
    fn controlled_module_calls_validate_and_expose_their_json_output_limit() -> Result<()> {
        let cancellation = CancellationToken::new();
        let control = ModuleCallControl::new(
            cancellation,
            Instant::now() + Duration::from_secs(5),
            Arc::new(AtomicU8::new(1)),
        );
        anyhow::ensure!(
            control.json_output_limit() == LOGOSCORE_JSON_OUTPUT_LIMIT,
            "controlled module call default output limit changed"
        );
        let expanded = control.with_json_output_limit(LOGOSCORE_MAX_JSON_OUTPUT_LIMIT)?;
        anyhow::ensure!(
            expanded.json_output_limit() == LOGOSCORE_MAX_JSON_OUTPUT_LIMIT,
            "controlled module call did not retain the Catalog range output limit"
        );
        anyhow::ensure!(
            ModuleCallControl::new(
                CancellationToken::new(),
                Instant::now() + Duration::from_secs(5),
                Arc::new(AtomicU8::new(1)),
            )
            .with_json_output_limit(0)
            .is_err(),
            "zero JSON output limit was accepted"
        );
        anyhow::ensure!(
            parse_json_stdout_with_limit("test", b"{}", 2)? == json!({}),
            "custom JSON output limit rejected an in-bounds response"
        );
        anyhow::ensure!(
            parse_json_stdout_with_limit("test", b"{}", 1).is_err(),
            "custom JSON output limit accepted an oversized response"
        );
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
    fn module_call_value_unwraps_cli_result_without_error_field() -> Result<()> {
        let value = normalize_module_call_value(
            "delivery_module",
            "getNodeInfo",
            json!({
                "status": "ok",
                "result": {
                    "success": true,
                    "value": "peer-test"
                }
            }),
        )?;

        anyhow::ensure!(value.as_str() == Some("peer-test"));
        Ok(())
    }

    #[test]
    fn module_call_value_unwraps_nested_result_envelopes() -> Result<()> {
        let value = normalize_module_call_value(
            "blockchain_module",
            "nodeAction",
            json!({
                "result": r#"{"result":"{\"schema\":\"logos.managed_node_lifecycle.ack\",\"version\":1}"}"#
            }),
        )?;

        anyhow::ensure!(
            value.get("schema").and_then(Value::as_str) == Some("logos.managed_node_lifecycle.ack")
        );
        anyhow::ensure!(value.get("version").and_then(Value::as_u64) == Some(1));
        Ok(())
    }

    #[test]
    fn module_call_value_preserves_payload_with_result_field() -> Result<()> {
        let payload = json!({"result": "application-value", "kind": "block"});
        let value = normalize_module_call_value("module", "method", payload.clone())?;

        anyhow::ensure!(value == payload);
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

    #[cfg(unix)]
    #[tokio::test]
    async fn cli_transport_preserves_typed_module_arguments() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-typed-call");
        write_executable_script(
            &program,
            r#"#!/bin/sh
set -eu
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
case "$1" in
    list-modules)
        printf '%s\n' '{"modules":[{"name":"lez_indexer_module","status":"loaded"}]}'
        ;;
    call)
        : > "$config_dir/call-args"
        for argument in "$@"; do
            printf '%s\n' "$argument" >> "$config_dir/call-args"
        done
        printf '%s\n' '{"method":"getBlocks","module":"lez_indexer_module","result":"[]","status":"ok"}'
        ;;
esac
"#,
        )?;
        let transport = LogoscoreCliTransport::managed(
            program.display().to_string(),
            directory.path().display().to_string(),
        );
        let call = ModuleCall::new(
            ModuleTransportKind::LogoscoreCli,
            "lez_indexer_module",
            "getBlocks",
            vec![json!("25"), json!(3), json!(false), json!({"key": "value"})],
        )?;

        let reply = transport.call(call.clone()).await?.into_value();

        anyhow::ensure!(reply == json!([]), "unexpected module reply: {reply}");
        let expected = [
            "call",
            "lez_indexer_module",
            "getBlocks",
            "--args-json",
            r#"["25",3,false,{"key":"value"}]"#,
            "--json",
        ];
        let captured = fs::read_to_string(directory.path().join("call-args"))?;
        let arguments = captured.lines().collect::<Vec<_>>();
        anyhow::ensure!(
            arguments == expected,
            "typed module arguments were not preserved: {arguments:?}"
        );

        let control = ModuleCallControl::new(
            CancellationToken::new(),
            Instant::now() + Duration::from_secs(5),
            Arc::new(AtomicU8::new(1)),
        );
        let controlled_reply = transport.call_controlled(call, control).await?.into_value();

        anyhow::ensure!(
            controlled_reply == json!([]),
            "unexpected controlled module reply: {controlled_reply}"
        );
        let controlled_capture = fs::read_to_string(directory.path().join("call-args"))?;
        let controlled_arguments = controlled_capture.lines().collect::<Vec<_>>();
        anyhow::ensure!(
            controlled_arguments == expected,
            "controlled typed arguments were not preserved: {controlled_arguments:?}"
        );
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

    #[cfg(unix)]
    #[tokio::test]
    async fn cli_transport_refuses_unloaded_module_before_call() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("logoscore-load-before-call");
        write_executable_script(
            &program,
            r#"#!/bin/sh
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
printf '%s\n' "$1" >> "$config_dir/sequence"
case "$1" in
    list-modules)
        status="$(cat "$config_dir/status")"
        printf '{"modules":[{"name":"lez_indexer_module","status":"%s"}]}\n' "$status"
        ;;
    call)
        if [ "$(cat "$config_dir/status")" != "loaded" ]; then
            touch "$config_dir/unsafe-call"
            exit 91
        fi
        printf '%s\n' '{"method":"getStatus","module":"lez_indexer_module","result":"{\"state\":\"stopped\"}","status":"ok"}'
        ;;
    module-info)
        if [ "$(cat "$config_dir/status")" != "loaded" ]; then
            touch "$config_dir/unsafe-module-info"
            exit 92
        fi
        printf '%s\n' '{"name":"lez_indexer_module","methods":[]}'
        ;;
esac
"#,
        )?;
        fs::write(directory.path().join("status"), "not_loaded")?;
        let transport = LogoscoreCliTransport::managed(
            program.display().to_string(),
            directory.path().display().to_string(),
        );
        let call = ModuleCall::new(
            ModuleTransportKind::LogoscoreCli,
            "lez_indexer_module",
            "getStatus",
            vec![],
        )?;

        let error = transport
            .call(call.clone())
            .await
            .err()
            .context("unloaded module call unexpectedly succeeded")?;

        anyhow::ensure!(
            error.to_string().contains("not loaded"),
            "unloaded module call lost status: {error:#}"
        );
        anyhow::ensure!(
            fs::read_to_string(directory.path().join("sequence"))?
                .lines()
                .eq(["list-modules"]),
            "unloaded module call continued past its listing"
        );
        anyhow::ensure!(
            !directory.path().join("unsafe-call").exists(),
            "transport invoked an unloaded module"
        );

        fs::write(directory.path().join("sequence"), "")?;
        fs::write(directory.path().join("status"), "loading")?;
        transport.runtime()?.invalidate_observation_snapshot()?;
        let control = ModuleCallControl::new(
            CancellationToken::new(),
            Instant::now() + Duration::from_secs(5),
            Arc::new(AtomicU8::new(1)),
        );
        let controlled_error = transport
            .call_controlled(call.clone(), control)
            .await
            .err()
            .context("controlled unloaded module call unexpectedly succeeded")?;
        anyhow::ensure!(
            controlled_error.to_string().contains("not loaded"),
            "controlled unloaded call lost status: {controlled_error:#}"
        );
        anyhow::ensure!(
            fs::read_to_string(directory.path().join("sequence"))?
                .lines()
                .eq(["list-modules"]),
            "controlled unloaded module call continued past its listing"
        );
        anyhow::ensure!(
            !directory.path().join("unsafe-call").exists(),
            "controlled transport invoked an unloaded module"
        );

        fs::write(directory.path().join("sequence"), "")?;
        fs::write(directory.path().join("status"), "crashed")?;
        transport.runtime()?.invalidate_observation_snapshot()?;
        let metadata_error = transport
            .module_info("lez_indexer_module".to_owned())
            .await
            .err()
            .context("crashed module metadata unexpectedly succeeded")?;
        anyhow::ensure!(
            metadata_error.to_string().contains("not loaded"),
            "crashed module metadata lost status: {metadata_error:#}"
        );
        anyhow::ensure!(
            fs::read_to_string(directory.path().join("sequence"))?
                .lines()
                .eq(["list-modules"]),
            "crashed module metadata continued past its listing"
        );
        anyhow::ensure!(
            !directory.path().join("unsafe-module-info").exists(),
            "diagnostics queried metadata from a crashed module"
        );

        fs::write(directory.path().join("sequence"), "")?;
        fs::write(directory.path().join("status"), "loaded")?;
        transport.runtime()?.invalidate_observation_snapshot()?;
        let loaded = transport.call(call).await?.into_value();
        anyhow::ensure!(
            loaded.get("state").and_then(Value::as_str) == Some("stopped"),
            "loaded module call returned unexpected value: {loaded}"
        );
        anyhow::ensure!(
            fs::read_to_string(directory.path().join("sequence"))?
                .lines()
                .eq(["list-modules", "call"]),
            "loaded module call did not execute exactly once"
        );
        Ok(())
    }

    #[test]
    fn dynamic_runtime_binding_tracks_start_restart_stop_and_explicit_precedence() -> Result<()> {
        let current = Arc::new(Mutex::new(None::<LogoscoreCliRuntime>));
        let resolver_state = Arc::clone(&current);
        let binding = LogoscoreRuntimeBinding::ConfiguredWithFallback(Arc::new(move || {
            resolver_state
                .lock()
                .map(|runtime| runtime.clone())
                .map_err(|_| anyhow::anyhow!("runtime resolver lock poisoned"))
        }));

        *current
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime state lock poisoned"))? = Some(
            LogoscoreCliRuntime::managed("/bin/first".to_owned(), "/config/first".to_owned()),
        );
        let first = binding.resolve_with_explicit(None)?;
        *current
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime state lock poisoned"))? = Some(
            LogoscoreCliRuntime::managed("/bin/second".to_owned(), "/config/second".to_owned()),
        );
        let second = binding.resolve_with_explicit(None)?;
        *current
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime state lock poisoned"))? = None;
        let stopped = binding.resolve_with_explicit(None)?;
        let explicit =
            LogoscoreCliRuntime::managed("/bin/external".to_owned(), "/config/external".to_owned());
        let selected_explicit = binding.resolve_with_explicit(Some(explicit.clone()))?;

        anyhow::ensure!(
            first.runner.config_dir.as_deref() == Some("/config/first")
                && second.runner.config_dir.as_deref() == Some("/config/second")
                && stopped.runner.config_dir.is_none()
                && selected_explicit == explicit,
            "dynamic LogosCore runtime selection retained stale state"
        );
        Ok(())
    }

    #[test]
    fn pinned_cli_transport_keeps_one_runtime_identity() -> Result<()> {
        let current = Arc::new(Mutex::new(Some(LogoscoreCliRuntime::managed(
            "/bin/first".to_owned(),
            "/config/first".to_owned(),
        ))));
        let resolver_state = Arc::clone(&current);
        let transport = LogoscoreCliTransport {
            runtime: LogoscoreRuntimeBinding::ConfiguredWithFallback(Arc::new(move || {
                resolver_state
                    .lock()
                    .map(|runtime| runtime.clone())
                    .map_err(|_| anyhow::anyhow!("runtime resolver lock poisoned"))
            })),
            close_cancellation: CancellationToken::new(),
        };

        let pinned = pin_module_transport(Arc::new(transport.clone()))?;
        *current
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime state lock poisoned"))? = Some(
            LogoscoreCliRuntime::managed("/bin/second".to_owned(), "/config/second".to_owned()),
        );

        anyhow::ensure!(
            pinned
                .logoscore_cli_transport()
                .context("pinned transport lost LogosCore CLI identity")?
                .runtime()?
                .runner
                .config_dir
                .as_deref()
                == Some("/config/first"),
            "pinned transport migrated to a newer runtime"
        );
        anyhow::ensure!(
            transport.runtime()?.runner.config_dir.as_deref() == Some("/config/second"),
            "dynamic transport stopped tracking runtime changes"
        );
        Ok(())
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
    fn module_discovery_accepts_the_universal_storage_v2_signature() -> Result<()> {
        let modules = json!([{"name": "storage_module", "status": "loaded"}]);
        let info = json!({
            "name": "storage_module",
            "methods": [{
                "isInvokable": true,
                "name": "downloadToUrlV2",
                "signature": crate::support::storage_download_contract::STORAGE_DOWNLOAD_V2_UNIVERSAL_METHOD_SIGNATURE,
            }]
        });
        let discovery = module_discovery("storage_module", &modules, &info)?;

        discovery.require_method_with_signatures(
            "downloadToUrlV2",
            &crate::support::storage_download_contract::STORAGE_DOWNLOAD_V2_METHOD_SIGNATURES,
        )
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
