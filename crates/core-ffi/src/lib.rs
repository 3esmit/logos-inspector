use std::{
    collections::HashMap,
    ffi::{CStr, CString, c_char, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
    sync::{
        Arc, Condvar, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
};

use logos_inspector::{
    bridge::{InspectorBridge, InspectorBridgeCloseHandle},
    module_transport::{
        BoxedModuleEventSubscription, BridgeCallbackId, ModuleCall, ModuleCallFuture,
        ModuleCallReply, ModuleDiagnosticFuture, ModuleEventSubscription, ModuleTransport,
        ModuleTransportClosed, ModuleTransportEvent, ModuleTransportKind, ModuleTransportResult,
        SharedModuleTransport,
    },
};
use serde_json::Value;
use tokio::sync::oneshot;

const HOST_TRANSPORT_ABI_VERSION: u32 = 1;
const ASYNC_WORKER_QUEUE_CAPACITY: usize = 128;
const HOST_EVENT_SUBSCRIPTION_CAPACITY: usize = 64;
const HOST_EVENT_SUBSCRIPTION_LIMIT: usize = 64;
const EVENT_REJECTED: i32 = 0;
const EVENT_ACCEPTED: i32 = 1;
const EVENT_BACKPRESSURE: i32 = -1;
const HOST_CLOSED_ERROR: &str = "Basecamp host transport closed: host_closed";
const REQUEST_CANCELLED_ERROR: &str = "Basecamp bridge request cancelled";
const ASYNC_REQUIRED_ERROR: &str =
    "host transport handles require logos_inspector_core_call_module_async";
const LOCAL_CALL_BACKPRESSURE_ERROR: &str = "Basecamp bridge worker queue is full";
const LOCAL_CALL_REENTRANT_ERROR: &str =
    "host-local synchronous calls cannot reenter the Basecamp bridge worker";
const WORKER_UNAVAILABLE_ERROR: &str = "Basecamp bridge worker is unavailable";
const HOST_EVENT_SUBSCRIPTION_OVERFLOW_ERROR: &str =
    "Basecamp module event subscription overflowed its bounded queue";

pub type LogosInspectorCoreReplyFn = unsafe extern "C" fn(*mut c_void, u64, *const c_char);
pub type LogosInspectorHostReplyFn = unsafe extern "C" fn(*mut c_void, u64, i32, *const c_char);
pub type LogosInspectorHostDispatchFn = unsafe extern "C" fn(
    *mut c_void,
    u64,
    *const c_char,
    *const c_char,
    *const c_char,
    LogosInspectorHostReplyFn,
    *mut c_void,
) -> i32;
pub type LogosInspectorHostCancelFn = unsafe extern "C" fn(*mut c_void, u64);
pub type LogosInspectorHostCloseFn = unsafe extern "C" fn(*mut c_void);

#[derive(Clone, Copy)]
#[repr(C)]
pub struct LogosInspectorHostTransportV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub context: *mut c_void,
    pub dispatch: Option<LogosInspectorHostDispatchFn>,
    pub cancel: Option<LogosInspectorHostCancelFn>,
    pub close: Option<LogosInspectorHostCloseFn>,
}

pub struct LogosInspectorCore {
    mode: CoreMode,
}

enum CoreMode {
    Synchronous(Box<SynchronousCore>),
    Asynchronous(AsynchronousCore),
}

struct SynchronousCore {
    bridge: InspectorBridge,
    closed: AtomicBool,
}

struct AsynchronousCore {
    state: Arc<AsyncState>,
    host: Arc<HostState>,
    sender: mpsc::SyncSender<WorkerCommand>,
    bridge_close: InspectorBridgeCloseHandle,
    worker_thread_id: thread::ThreadId,
    worker: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Clone, Copy)]
struct HostVtable {
    context: usize,
    dispatch: LogosInspectorHostDispatchFn,
    cancel: Option<LogosInspectorHostCancelFn>,
    close: LogosInspectorHostCloseFn,
}

struct BasecampHostTransport {
    state: Arc<HostState>,
}

struct HostState {
    vtable: HostVtable,
    registry: Mutex<HostRegistry>,
    quiesced: Condvar,
    native_runtime_module_events_ready: AtomicBool,
}

struct HostRegistry {
    phase: LifecyclePhase,
    next_request_id: u64,
    next_subscription_id: u64,
    active_host_calls: usize,
    pending: HashMap<ModuleRequestId, PendingHostRequest>,
    subscriptions: HashMap<HostSubscriptionId, HostEventSubscriptionEntry>,
}

struct PendingHostRequest {
    sender: Option<oneshot::Sender<Result<Value, String>>>,
    origin_thread: thread::ThreadId,
}

struct PendingHostCall {
    state: Arc<HostState>,
    module_request_id: ModuleRequestId,
}

struct HostEventSubscriptionEntry {
    module: String,
    event: String,
    sender: mpsc::SyncSender<ModuleTransportEvent>,
    status: Arc<HostEventSubscriptionStatus>,
}

struct HostEventSubscriptionStatus {
    overflowed: AtomicBool,
    closed: AtomicBool,
}

struct BasecampModuleEventSubscription {
    state: Arc<HostState>,
    subscription_id: HostSubscriptionId,
    receiver: mpsc::Receiver<ModuleTransportEvent>,
    status: Arc<HostEventSubscriptionStatus>,
}

struct AsyncState {
    registry: Mutex<AsyncRegistry>,
    closed: Condvar,
    #[cfg(test)]
    queued_local_calls: std::sync::atomic::AtomicUsize,
    #[cfg(test)]
    shutdown_queued: AtomicBool,
}

struct AsyncRegistry {
    phase: LifecyclePhase,
    next_ingress_token: u64,
    pending: HashMap<BridgeRequestId, PendingCoreRequest>,
    completing: HashMap<BridgeRequestId, IngressRequestToken>,
}

struct PendingCoreRequest {
    ingress_token: IngressRequestToken,
    reply: LogosInspectorCoreReplyFn,
    reply_context: usize,
}

enum WorkerCommand {
    LocalCall {
        method: String,
        args_json: String,
        reply: mpsc::SyncSender<String>,
    },
    Call {
        bridge_request_id: BridgeRequestId,
        ingress_token: IngressRequestToken,
        module: String,
        method: String,
        args_json: String,
    },
    ModuleEvent {
        module: String,
        event: String,
        args: Vec<Value>,
    },
    Shutdown,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LifecyclePhase {
    Open,
    Closing,
    Closed,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct BridgeRequestId(u64);

#[derive(Clone, Copy, PartialEq, Eq)]
struct IngressRequestToken(u64);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ModuleRequestId(u64);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct HostSubscriptionId(u64);

impl HostVtable {
    unsafe fn copy_from(transport: *const LogosInspectorHostTransportV1) -> Result<Self, String> {
        if transport.is_null() {
            return Err("host transport is required".to_owned());
        }

        // SAFETY: the constructor contract requires a readable v1 prefix for
        // the duration of this call; the value is copied before returning.
        let transport = unsafe { *transport };
        if transport.abi_version != HOST_TRANSPORT_ABI_VERSION {
            return Err(format!(
                "unsupported host transport ABI version {}",
                transport.abi_version
            ));
        }
        if (transport.struct_size as usize) < size_of::<LogosInspectorHostTransportV1>() {
            return Err("host transport vtable is smaller than version 1".to_owned());
        }
        let Some(dispatch) = transport.dispatch else {
            return Err("host transport dispatch callback is required".to_owned());
        };
        let Some(close) = transport.close else {
            return Err("host transport close callback is required".to_owned());
        };
        Ok(Self {
            context: transport.context.expose_provenance(),
            dispatch,
            cancel: transport.cancel,
            close,
        })
    }

    fn context(self) -> *mut c_void {
        ptr::with_exposed_provenance_mut(self.context)
    }
}

impl HostState {
    fn new(vtable: HostVtable) -> Arc<Self> {
        Arc::new(Self {
            vtable,
            registry: Mutex::new(HostRegistry {
                phase: LifecyclePhase::Open,
                next_request_id: 1,
                next_subscription_id: 1,
                active_host_calls: 0,
                pending: HashMap::new(),
                subscriptions: HashMap::new(),
            }),
            quiesced: Condvar::new(),
            native_runtime_module_events_ready: AtomicBool::new(false),
        })
    }

    fn set_native_runtime_module_events_ready(&self, ready: bool) -> bool {
        let registry = lock(&self.registry);
        if registry.phase != LifecyclePhase::Open {
            return false;
        }
        self.native_runtime_module_events_ready
            .store(ready, Ordering::Release);
        true
    }

    fn dispatch(self: &Arc<Self>, call: ModuleCall) -> ModuleCallFuture<'static> {
        let state = Arc::clone(self);
        Box::pin(async move {
            let module = call.module().to_owned();
            let method = call.method().to_owned();
            let args_json = serde_json::to_string(call.args()).map_err(|error| {
                std::io::Error::other(format!("failed to encode module args: {error}"))
            })?;
            let module_c = CString::new(module.clone())
                .map_err(|_| std::io::Error::other("module name contains a NUL byte"))?;
            let method_c = CString::new(method.clone())
                .map_err(|_| std::io::Error::other("method name contains a NUL byte"))?;
            let args_c = CString::new(args_json)
                .map_err(|_| std::io::Error::other("module args contain a NUL byte"))?;
            let (sender, receiver) = oneshot::channel();

            let module_request_id = {
                let mut registry = lock(&state.registry);
                if registry.phase != LifecyclePhase::Open {
                    return Err(ModuleTransportClosed::new(HOST_CLOSED_ERROR).into());
                }
                let module_request_id = ModuleRequestId(registry.next_request_id);
                registry.next_request_id = registry
                    .next_request_id
                    .checked_add(1)
                    .ok_or_else(|| std::io::Error::other("module request id space exhausted"))?;
                registry.pending.insert(
                    module_request_id,
                    PendingHostRequest {
                        sender: Some(sender),
                        origin_thread: thread::current().id(),
                    },
                );
                registry.active_host_calls += 1;
                module_request_id
            };

            // SAFETY: all strings remain live through the dispatch call. The
            // callback context points to this stable Arc allocation, retained
            // by the core until host close has quiesced every reply callback.
            let accepted = unsafe {
                (state.vtable.dispatch)(
                    state.vtable.context(),
                    module_request_id.0,
                    module_c.as_ptr(),
                    method_c.as_ptr(),
                    args_c.as_ptr(),
                    host_transport_reply,
                    Arc::as_ptr(&state).cast_mut().cast(),
                )
            };
            if accepted != 1 {
                lock(&state.registry).pending.remove(&module_request_id);
            }
            state.finish_host_call();
            if accepted != 1 {
                return Err(std::io::Error::other(format!(
                    "host rejected module request {}",
                    module_request_id.0
                ))
                .into());
            }

            let _pending_call = PendingHostCall {
                state: Arc::clone(&state),
                module_request_id,
            };
            let result = receiver.await.map_err(|_| {
                std::io::Error::other(format!(
                    "host dropped module request {} without a reply",
                    module_request_id.0
                ))
            })?;
            let value = match result {
                Ok(value) => value,
                Err(error) if error == HOST_CLOSED_ERROR => {
                    return Err(ModuleTransportClosed::new(error).into());
                }
                Err(error) => return Err(std::io::Error::other(error).into()),
            };
            Ok(ModuleCallReply::new(ModuleTransportKind::Module, value)
                .with_bridge_callback(BridgeCallbackId::new(module_request_id.0)))
        })
    }

    fn complete(&self, module_request_id: ModuleRequestId, result: Result<Value, String>) {
        let pending = lock(&self.registry).pending.remove(&module_request_id);
        if let Some(pending) = pending
            && let Some(sender) = pending.sender
        {
            let _result = sender.send(result);
        }
    }

    fn subscribe_module_event(
        self: &Arc<Self>,
        module: &str,
        event: &str,
    ) -> ModuleTransportResult<BoxedModuleEventSubscription> {
        if module.trim().is_empty() {
            return Err(std::io::Error::other("module event module name is required").into());
        }
        if event.trim().is_empty() {
            return Err(std::io::Error::other("module event name is required").into());
        }
        let (sender, receiver) = mpsc::sync_channel(HOST_EVENT_SUBSCRIPTION_CAPACITY);
        let status = Arc::new(HostEventSubscriptionStatus {
            overflowed: AtomicBool::new(false),
            closed: AtomicBool::new(false),
        });
        let subscription_id = {
            let mut registry = lock(&self.registry);
            if registry.phase != LifecyclePhase::Open {
                return Err(ModuleTransportClosed::new(HOST_CLOSED_ERROR).into());
            }
            if registry.subscriptions.len() >= HOST_EVENT_SUBSCRIPTION_LIMIT {
                return Err(std::io::Error::other(format!(
                    "Basecamp module event subscription limit of {HOST_EVENT_SUBSCRIPTION_LIMIT} reached"
                ))
                .into());
            }
            let subscription_id = HostSubscriptionId(registry.next_subscription_id);
            registry.next_subscription_id = registry
                .next_subscription_id
                .checked_add(1)
                .ok_or_else(|| std::io::Error::other("module subscription id space exhausted"))?;
            registry.subscriptions.insert(
                subscription_id,
                HostEventSubscriptionEntry {
                    module: module.to_owned(),
                    event: event.to_owned(),
                    sender,
                    status: Arc::clone(&status),
                },
            );
            subscription_id
        };
        Ok(Box::new(BasecampModuleEventSubscription {
            state: Arc::clone(self),
            subscription_id,
            receiver,
            status,
        }))
    }

    fn publish_module_event(
        &self,
        module: &str,
        event: &str,
        args: &[Value],
    ) -> ModuleTransportResult<()> {
        let transport_event = ModuleTransportEvent::new(module, event, args.to_vec())?;
        let mut registry = lock(&self.registry);
        if registry.phase != LifecyclePhase::Open {
            return Err(ModuleTransportClosed::new(HOST_CLOSED_ERROR).into());
        }
        let mut remove = Vec::new();
        let mut overflowed = false;
        for (subscription_id, subscription) in &registry.subscriptions {
            if subscription.module != module || subscription.event != event {
                continue;
            }
            match subscription.sender.try_send(transport_event.clone()) {
                Ok(()) => {}
                Err(mpsc::TrySendError::Full(_)) => {
                    subscription
                        .status
                        .overflowed
                        .store(true, Ordering::Release);
                    overflowed = true;
                    remove.push(*subscription_id);
                }
                Err(mpsc::TrySendError::Disconnected(_)) => remove.push(*subscription_id),
            }
        }
        for subscription_id in remove {
            registry.subscriptions.remove(&subscription_id);
        }
        drop(registry);
        if overflowed {
            return Err(std::io::Error::other(HOST_EVENT_SUBSCRIPTION_OVERFLOW_ERROR).into());
        }
        Ok(())
    }

    fn unsubscribe_module_event(&self, subscription_id: HostSubscriptionId) {
        lock(&self.registry).subscriptions.remove(&subscription_id);
    }

    fn interrupt_requests_from_thread(&self, origin_thread: thread::ThreadId) {
        let interrupted = {
            let mut registry = lock(&self.registry);
            let request_ids = registry
                .pending
                .iter()
                .filter_map(|(request_id, pending)| {
                    (pending.origin_thread == origin_thread).then_some(*request_id)
                })
                .collect::<Vec<_>>();
            request_ids
                .into_iter()
                .filter_map(|request_id| registry.pending.remove(&request_id))
                .filter_map(|mut pending| pending.sender.take())
                .collect::<Vec<_>>()
        };
        for sender in interrupted {
            let _result = sender.send(Err(HOST_CLOSED_ERROR.to_owned()));
        }
    }

    fn finish_host_call(&self) {
        let mut registry = lock(&self.registry);
        if registry.active_host_calls > 0 {
            registry.active_host_calls -= 1;
        }
        self.quiesced.notify_all();
    }

    fn abandon(&self, module_request_id: ModuleRequestId) {
        let should_cancel = {
            let mut registry = lock(&self.registry);
            if registry.pending.remove(&module_request_id).is_none() {
                return;
            }
            let should_cancel =
                registry.phase == LifecyclePhase::Open && self.vtable.cancel.is_some();
            if should_cancel {
                registry.active_host_calls += 1;
            }
            should_cancel
        };
        if should_cancel && let Some(cancel) = self.vtable.cancel {
            // SAFETY: the copied host context remains valid until close.
            unsafe {
                cancel(self.vtable.context(), module_request_id.0);
            }
            self.finish_host_call();
        }
    }

    fn close(&self) {
        {
            let mut registry = lock(&self.registry);
            match registry.phase {
                LifecyclePhase::Open => {
                    registry.phase = LifecyclePhase::Closing;
                    self.native_runtime_module_events_ready
                        .store(false, Ordering::Release);
                }
                LifecyclePhase::Closing => {
                    while registry.phase != LifecyclePhase::Closed {
                        registry = wait(&self.quiesced, registry);
                    }
                    return;
                }
                LifecyclePhase::Closed => return,
            }
            while registry.active_host_calls > 0 {
                registry = wait(&self.quiesced, registry);
            }
        }

        // SAFETY: close is called once by the lifecycle owner and the copied
        // context remains valid until this callback returns.
        unsafe {
            (self.vtable.close)(self.vtable.context());
        }

        let (pending, subscriptions) = {
            let mut registry = lock(&self.registry);
            let pending = registry
                .pending
                .drain()
                .map(|(_, pending)| pending)
                .collect::<Vec<_>>();
            let subscriptions = registry
                .subscriptions
                .drain()
                .map(|(_, subscription)| subscription)
                .collect::<Vec<_>>();
            for subscription in &subscriptions {
                subscription.status.closed.store(true, Ordering::Release);
            }
            registry.phase = LifecyclePhase::Closed;
            self.quiesced.notify_all();
            (pending, subscriptions)
        };
        drop(subscriptions);
        for pending in pending {
            if let Some(sender) = pending.sender {
                let _result = sender.send(Err(HOST_CLOSED_ERROR.to_owned()));
            }
        }
    }
}

impl Drop for PendingHostCall {
    fn drop(&mut self) {
        self.state.abandon(self.module_request_id);
    }
}

impl ModuleEventSubscription for BasecampModuleEventSubscription {
    fn next_within(
        &mut self,
        timeout: std::time::Duration,
    ) -> ModuleTransportResult<Option<ModuleTransportEvent>> {
        if self.status.overflowed.load(Ordering::Acquire) {
            return Err(std::io::Error::other(HOST_EVENT_SUBSCRIPTION_OVERFLOW_ERROR).into());
        }
        if self.status.closed.load(Ordering::Acquire) {
            return Err(ModuleTransportClosed::new(HOST_CLOSED_ERROR).into());
        }
        match self.receiver.recv_timeout(timeout) {
            Ok(event) => {
                if self.status.overflowed.load(Ordering::Acquire) {
                    return Err(
                        std::io::Error::other(HOST_EVENT_SUBSCRIPTION_OVERFLOW_ERROR).into(),
                    );
                }
                if self.status.closed.load(Ordering::Acquire) {
                    return Err(ModuleTransportClosed::new(HOST_CLOSED_ERROR).into());
                }
                Ok(Some(event))
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if self.status.overflowed.load(Ordering::Acquire) {
                    return Err(
                        std::io::Error::other(HOST_EVENT_SUBSCRIPTION_OVERFLOW_ERROR).into(),
                    );
                }
                if self.status.closed.load(Ordering::Acquire) {
                    return Err(ModuleTransportClosed::new(HOST_CLOSED_ERROR).into());
                }
                Ok(None)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if self.status.overflowed.load(Ordering::Acquire) {
                    return Err(
                        std::io::Error::other(HOST_EVENT_SUBSCRIPTION_OVERFLOW_ERROR).into(),
                    );
                }
                Err(ModuleTransportClosed::new(HOST_CLOSED_ERROR).into())
            }
        }
    }
}

impl Drop for BasecampModuleEventSubscription {
    fn drop(&mut self) {
        self.state.unsubscribe_module_event(self.subscription_id);
    }
}

impl ModuleTransport for BasecampHostTransport {
    fn kind(&self) -> ModuleTransportKind {
        ModuleTransportKind::Module
    }

    fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
        self.state.dispatch(call)
    }

    fn subscribe_module_event(
        &self,
        module: &str,
        event: &str,
    ) -> ModuleTransportResult<BoxedModuleEventSubscription> {
        self.state.subscribe_module_event(module, event)
    }

    fn ingest_module_event(
        &self,
        module: &str,
        event: &str,
        args: &[Value],
    ) -> ModuleTransportResult<()> {
        self.state.publish_module_event(module, event, args)
    }

    fn supports_shared_file_staging(&self) -> bool {
        true
    }

    fn native_runtime_module_events_ready(&self) -> bool {
        self.state
            .native_runtime_module_events_ready
            .load(Ordering::Acquire)
    }

    fn module_info(&self, module: String) -> ModuleDiagnosticFuture<'_> {
        let state = Arc::clone(&self.state);
        Box::pin(async move {
            let methods = state
                .dispatch(ModuleCall::new(
                    ModuleTransportKind::Module,
                    module.clone(),
                    "getPluginMethods",
                    Vec::new(),
                )?)
                .await?
                .into_value();
            if !methods.is_array() {
                return Err(std::io::Error::other(format!(
                    "host module `{module}` method metadata is not an array"
                ))
                .into());
            }
            let events = state
                .dispatch(ModuleCall::new(
                    ModuleTransportKind::Module,
                    module.clone(),
                    "getPluginEvents",
                    Vec::new(),
                )?)
                .await?
                .into_value();
            if !events.is_array() {
                return Err(std::io::Error::other(format!(
                    "host module `{module}` event metadata is not an array"
                ))
                .into());
            }
            Ok(serde_json::json!({
                "name": module,
                "methods": methods,
                "events": events,
            }))
        })
    }
}

impl AsyncState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            registry: Mutex::new(AsyncRegistry {
                phase: LifecyclePhase::Open,
                next_ingress_token: 1,
                pending: HashMap::new(),
                completing: HashMap::new(),
            }),
            closed: Condvar::new(),
            #[cfg(test)]
            queued_local_calls: std::sync::atomic::AtomicUsize::new(0),
            #[cfg(test)]
            shutdown_queued: AtomicBool::new(false),
        })
    }

    fn start(
        &self,
        bridge_request_id: BridgeRequestId,
        ingress_token: IngressRequestToken,
    ) -> bool {
        let registry = lock(&self.registry);
        if registry.phase != LifecyclePhase::Open
            || registry
                .pending
                .get(&bridge_request_id)
                .is_none_or(|pending| pending.ingress_token != ingress_token)
        {
            return false;
        }
        true
    }

    fn claim_completion(
        &self,
        bridge_request_id: BridgeRequestId,
        ingress_token: IngressRequestToken,
    ) -> Option<PendingCoreRequest> {
        let mut registry = lock(&self.registry);
        if registry
            .pending
            .get(&bridge_request_id)
            .is_none_or(|pending| pending.ingress_token != ingress_token)
        {
            return None;
        }
        let pending = registry.pending.remove(&bridge_request_id);
        if pending.is_some() {
            registry.completing.insert(bridge_request_id, ingress_token);
        }
        pending
    }

    fn cancel(&self, bridge_request_id: BridgeRequestId) -> bool {
        let pending = {
            let mut registry = lock(&self.registry);
            if registry.phase != LifecyclePhase::Open {
                return false;
            }
            let pending = registry.pending.remove(&bridge_request_id);
            if let Some(pending) = pending.as_ref() {
                registry
                    .completing
                    .insert(bridge_request_id, pending.ingress_token);
            }
            pending
        };
        let Some(pending) = pending else {
            return false;
        };
        let ingress_token = pending.ingress_token;
        invoke_core_reply(
            pending,
            bridge_request_id,
            &InspectorBridge::error_json(REQUEST_CANCELLED_ERROR),
        );
        self.finish_callback(bridge_request_id, ingress_token);
        true
    }

    fn finish_callback(
        &self,
        bridge_request_id: BridgeRequestId,
        ingress_token: IngressRequestToken,
    ) {
        let mut registry = lock(&self.registry);
        if registry.completing.get(&bridge_request_id) == Some(&ingress_token) {
            registry.completing.remove(&bridge_request_id);
        }
    }

    fn begin_close(&self) -> Option<Vec<(BridgeRequestId, PendingCoreRequest)>> {
        let mut registry = lock(&self.registry);
        match registry.phase {
            LifecyclePhase::Open => registry.phase = LifecyclePhase::Closing,
            LifecyclePhase::Closing => {
                while registry.phase != LifecyclePhase::Closed {
                    registry = wait(&self.closed, registry);
                }
                return None;
            }
            LifecyclePhase::Closed => return None,
        }
        let pending = registry.pending.drain().collect::<Vec<_>>();
        for (bridge_request_id, request) in &pending {
            registry
                .completing
                .insert(*bridge_request_id, request.ingress_token);
        }
        Some(pending)
    }

    fn finish_close(&self) {
        let mut registry = lock(&self.registry);
        registry.completing.clear();
        registry.phase = LifecyclePhase::Closed;
        self.closed.notify_all();
    }
}

impl AsynchronousCore {
    fn new(vtable: HostVtable) -> Result<Self, String> {
        let host = HostState::new(vtable);
        let transport: SharedModuleTransport = Arc::new(BasecampHostTransport {
            state: Arc::clone(&host),
        });
        let bridge = InspectorBridge::with_shared_module_transport(transport)
            .map_err(|error| format!("failed to initialize asynchronous bridge: {error}"))?;
        let bridge_close = bridge.close_handle();
        let state = AsyncState::new();
        let (sender, receiver) = mpsc::sync_channel(ASYNC_WORKER_QUEUE_CAPACITY);
        let worker_state = Arc::clone(&state);
        let worker = thread::Builder::new()
            .name("logos-inspector-core".to_owned())
            .spawn(move || run_worker(bridge, receiver, &worker_state))
            .map_err(|error| format!("failed to start asynchronous bridge worker: {error}"))?;
        let worker_thread_id = worker.thread().id();
        Ok(Self {
            state,
            host,
            sender,
            bridge_close,
            worker_thread_id,
            worker: Mutex::new(Some(worker)),
        })
    }

    fn call_local_inspector(&self, method: String, args_json: String) -> String {
        if !InspectorBridge::allows_host_synchronous_call(&method) {
            return InspectorBridge::error_json(ASYNC_REQUIRED_ERROR);
        }
        if thread::current().id() == self.worker_thread_id {
            return InspectorBridge::error_json(LOCAL_CALL_REENTRANT_ERROR);
        }

        let (reply, response) = mpsc::sync_channel(1);
        let registry = lock(&self.state.registry);
        if registry.phase != LifecyclePhase::Open {
            return InspectorBridge::error_json(HOST_CLOSED_ERROR);
        }
        #[cfg(test)]
        self.state.queued_local_calls.fetch_add(1, Ordering::AcqRel);
        let enqueue_result = self.sender.try_send(WorkerCommand::LocalCall {
            method,
            args_json,
            reply,
        });
        #[cfg(test)]
        if enqueue_result.is_err() {
            self.state.queued_local_calls.fetch_sub(1, Ordering::AcqRel);
        }
        drop(registry);
        match enqueue_result {
            Ok(()) => {}
            Err(mpsc::TrySendError::Full(_)) => {
                return InspectorBridge::error_json(LOCAL_CALL_BACKPRESSURE_ERROR);
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                return InspectorBridge::error_json(WORKER_UNAVAILABLE_ERROR);
            }
        }
        response
            .recv()
            .unwrap_or_else(|_| InspectorBridge::error_json(WORKER_UNAVAILABLE_ERROR))
    }

    fn set_native_runtime_module_events_ready(&self, ready: bool) -> bool {
        let registry = lock(&self.state.registry);
        if registry.phase != LifecyclePhase::Open {
            return false;
        }
        self.host.set_native_runtime_module_events_ready(ready)
    }

    fn enqueue(
        &self,
        bridge_request_id: BridgeRequestId,
        module: String,
        method: String,
        args_json: String,
        reply: LogosInspectorCoreReplyFn,
        reply_context: *mut c_void,
    ) -> bool {
        if bridge_request_id.0 == 0 {
            return false;
        }
        let mut registry = lock(&self.state.registry);
        if registry.phase != LifecyclePhase::Open
            || registry.pending.contains_key(&bridge_request_id)
            || registry.completing.contains_key(&bridge_request_id)
        {
            return false;
        }
        let ingress_token = IngressRequestToken(registry.next_ingress_token);
        let Some(next_ingress_token) = registry.next_ingress_token.checked_add(1) else {
            return false;
        };
        registry.next_ingress_token = next_ingress_token;
        registry.pending.insert(
            bridge_request_id,
            PendingCoreRequest {
                ingress_token,
                reply,
                reply_context: reply_context.expose_provenance(),
            },
        );
        let sent = self
            .sender
            .try_send(WorkerCommand::Call {
                bridge_request_id,
                ingress_token,
                module,
                method,
                args_json,
            })
            .is_ok();
        if !sent
            && registry
                .pending
                .get(&bridge_request_id)
                .is_some_and(|pending| pending.ingress_token == ingress_token)
        {
            registry.pending.remove(&bridge_request_id);
        }
        sent
    }

    fn cancel(&self, bridge_request_id: BridgeRequestId) -> bool {
        self.state.cancel(bridge_request_id)
    }

    fn enqueue_module_event(&self, module: String, event: String, args: Vec<Value>) -> i32 {
        let registry = lock(&self.state.registry);
        if registry.phase != LifecyclePhase::Open {
            return EVENT_REJECTED;
        }
        match self.sender.try_send(WorkerCommand::ModuleEvent {
            module,
            event,
            args,
        }) {
            Ok(()) => EVENT_ACCEPTED,
            Err(mpsc::TrySendError::Full(_)) => EVENT_BACKPRESSURE,
            Err(mpsc::TrySendError::Disconnected(_)) => EVENT_REJECTED,
        }
    }

    fn close(&self) {
        let Some(pending) = self.state.begin_close() else {
            return;
        };
        self.host
            .native_runtime_module_events_ready
            .store(false, Ordering::Release);
        let _closing = self.bridge_close.begin_close().is_ok();
        self.host
            .interrupt_requests_from_thread(self.worker_thread_id);
        let _sent = self.sender.send(WorkerCommand::Shutdown).is_ok();
        #[cfg(test)]
        self.state.shutdown_queued.store(_sent, Ordering::Release);
        if let Some(worker) = lock(&self.worker).take() {
            let _joined = worker.join().is_ok();
        }
        self.host.close();
        for (bridge_request_id, pending) in pending {
            let ingress_token = pending.ingress_token;
            invoke_core_reply(
                pending,
                bridge_request_id,
                &InspectorBridge::error_json(HOST_CLOSED_ERROR),
            );
            self.state.finish_callback(bridge_request_id, ingress_token);
        }
        self.state.finish_close();
    }
}

impl Drop for AsynchronousCore {
    fn drop(&mut self) {
        self.close();
    }
}

fn run_worker(
    bridge: InspectorBridge,
    receiver: mpsc::Receiver<WorkerCommand>,
    state: &AsyncState,
) {
    while let Ok(command) = receiver.recv() {
        match command {
            WorkerCommand::LocalCall {
                method,
                args_json,
                reply,
            } => {
                #[cfg(test)]
                state.queued_local_calls.fetch_sub(1, Ordering::AcqRel);
                let response = match catch_unwind(AssertUnwindSafe(|| {
                    bridge.call_inspector_json(&method, &args_json)
                })) {
                    Ok(response) => response,
                    Err(_) => InspectorBridge::error_json("asynchronous bridge worker panicked"),
                };
                let _sent = reply.send(response).is_ok();
            }
            WorkerCommand::Call {
                bridge_request_id,
                ingress_token,
                module,
                method,
                args_json,
            } => {
                if !state.start(bridge_request_id, ingress_token) {
                    continue;
                }
                let response = match catch_unwind(AssertUnwindSafe(|| {
                    bridge.call_module_json(&module, &method, &args_json)
                })) {
                    Ok(response) => response,
                    Err(_) => InspectorBridge::error_json("asynchronous bridge worker panicked"),
                };
                let pending = state.claim_completion(bridge_request_id, ingress_token);
                if let Some(pending) = pending {
                    let completion_token = pending.ingress_token;
                    invoke_core_reply(pending, bridge_request_id, &response);
                    state.finish_callback(bridge_request_id, completion_token);
                }
            }
            WorkerCommand::ModuleEvent {
                module,
                event,
                args,
            } => {
                let _result = catch_unwind(AssertUnwindSafe(|| {
                    bridge.ingest_module_event(&module, &event, args)
                }));
            }
            WorkerCommand::Shutdown => {
                let _shutdown = bridge.shutdown().is_ok();
                break;
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn logos_inspector_core_new() -> *mut LogosInspectorCore {
    match catch_unwind(AssertUnwindSafe(|| {
        InspectorBridge::basecamp_unavailable().map(|bridge| LogosInspectorCore {
            mode: CoreMode::Synchronous(Box::new(SynchronousCore {
                bridge,
                closed: AtomicBool::new(false),
            })),
        })
    })) {
        Ok(Ok(core)) => Box::into_raw(Box::new(core)),
        Ok(Err(_)) | Err(_) => ptr::null_mut(),
    }
}

/// Creates an asynchronous bridge around a copied host transport vtable.
///
/// # Safety
///
/// `transport` must point to a readable `LogosInspectorHostTransportV1`. Its
/// context must remain valid until `logos_inspector_core_close` returns and
/// satisfy the concurrency and callback-quiescence contract in the C header.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn logos_inspector_core_new_with_host_transport(
    transport: *const LogosInspectorHostTransportV1,
) -> *mut LogosInspectorCore {
    match catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: forwarded from this function's constructor contract.
        let vtable = unsafe { HostVtable::copy_from(transport) }?;
        AsynchronousCore::new(vtable).map(|core| LogosInspectorCore {
            mode: CoreMode::Asynchronous(core),
        })
    })) {
        Ok(Ok(core)) => Box::into_raw(Box::new(core)),
        Ok(Err(_)) | Err(_) => ptr::null_mut(),
    }
}

/// Closes a bridge handle without releasing its allocation.
///
/// # Safety
///
/// `handle` must be null or a live pointer returned by a constructor in this
/// library. Close may race async call/cancel and
/// `logos_inspector_core_call` on a host-transport handle while the allocation
/// remains live. Join every racing call and close before free. Close must not
/// be called reentrantly from a core reply or host transport callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn logos_inspector_core_close(handle: *mut LogosInspectorCore) {
    if handle.is_null() {
        return;
    }
    let _result = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: guaranteed by this function's handle contract.
        let core = unsafe { &*handle };
        match &core.mode {
            CoreMode::Synchronous(core) => {
                if !core.closed.swap(true, Ordering::AcqRel) {
                    let _closing = core.bridge.begin_close().is_ok();
                    let _shutdown = core.bridge.shutdown().is_ok();
                }
            }
            CoreMode::Asynchronous(core) => core.close(),
        }
    }));
}

/// Releases a bridge handle created by `logos_inspector_core_new`.
///
/// # Safety
///
/// `handle` must be null or a pointer returned by a constructor in this
/// library that has not already been released. Free must not race another ABI
/// call or callback. Join every racing host-transport call and close before
/// invoking free. Free must not be called reentrantly from a core reply or host
/// transport callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn logos_inspector_core_free(handle: *mut LogosInspectorCore) {
    if handle.is_null() {
        return;
    }

    // SAFETY: guaranteed by this function's handle contract.
    unsafe { logos_inspector_core_close(handle) };
    let _result = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: `handle` was allocated by a constructor in this library;
        // this function is the matching owner-releasing boundary.
        unsafe {
            drop(Box::from_raw(handle));
        }
    }));
}

/// Calls a method on the embedded `logos_inspector` bridge.
///
/// Host-transport handles accept only explicitly catalogued synchronous
/// methods. Accepted calls enter the same bounded worker and bridge instance
/// as asynchronous calls; Tokio- or module-transport-backed methods and all
/// other async-required commands return an error without dispatch.
///
/// # Safety
///
/// `handle` must be null or a live pointer returned by
/// `logos_inspector_core_new`. `method` and `args_json` must be valid
/// NUL-terminated UTF-8 strings for the duration of the call. This call may
/// race `logos_inspector_core_close` on a host-transport handle while the
/// allocation remains live; join both calls before free. The returned pointer
/// must be released with `logos_inspector_core_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn logos_inspector_core_call(
    handle: *mut LogosInspectorCore,
    method: *const c_char,
    args_json: *const c_char,
) -> *mut c_char {
    let response = match catch_unwind(AssertUnwindSafe(|| {
        match call_inputs(handle, method, args_json) {
            Ok((core, method, args_json)) => core.call_inspector(&method, &args_json),
            Err(error) => InspectorBridge::error_json(error),
        }
    })) {
        Ok(response) => response,
        Err(_) => InspectorBridge::error_json("core call panicked"),
    };
    into_c_string(response)
}

/// Calls any module through the embedded inspector bridge.
///
/// Host-transport handles reject this synchronous entry point without host
/// dispatch. Use `logos_inspector_core_call_module_async` instead.
///
/// # Safety
///
/// `handle` must be null or a live pointer returned by
/// `logos_inspector_core_new`. `module`, `method`, and `args_json` must be valid
/// NUL-terminated UTF-8 strings for the duration of the call. The returned
/// pointer must be released with `logos_inspector_core_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn logos_inspector_core_call_module(
    handle: *mut LogosInspectorCore,
    module: *const c_char,
    method: *const c_char,
    args_json: *const c_char,
) -> *mut c_char {
    let response = match catch_unwind(AssertUnwindSafe(|| {
        match call_module_inputs(handle, module, method, args_json) {
            Ok((core, module, method, args_json)) => core.call_module(&module, &method, &args_json),
            Err(error) => InspectorBridge::error_json(error),
        }
    })) {
        Ok(response) => response,
        Err(_) => InspectorBridge::error_json("core module call panicked"),
    };
    into_c_string(response)
}

/// Enqueues one copied asynchronous bridge call.
///
/// # Safety
///
/// `handle` must be null or live. All string pointers must remain readable
/// NUL-terminated UTF-8 for this call. A non-null reply is required. Return 1
/// transfers `reply_context` until exactly one callback; return 0 does not.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn logos_inspector_core_call_module_async(
    handle: *mut LogosInspectorCore,
    bridge_request_id: u64,
    module: *const c_char,
    method: *const c_char,
    args_json: *const c_char,
    reply: Option<LogosInspectorCoreReplyFn>,
    reply_context: *mut c_void,
) -> i32 {
    let accepted = catch_unwind(AssertUnwindSafe(|| {
        let Some(reply) = reply else {
            return false;
        };
        let Ok((core, module, method, args_json)) =
            call_module_inputs(handle, module, method, args_json)
        else {
            return false;
        };
        match &core.mode {
            CoreMode::Synchronous(_) => false,
            CoreMode::Asynchronous(core) => core.enqueue(
                BridgeRequestId(bridge_request_id),
                module,
                method,
                args_json,
                reply,
                reply_context,
            ),
        }
    }))
    .unwrap_or(false);
    i32::from(accepted)
}

/// Publishes whether the native Basecamp adapter owns a complete, healthy
/// runtime module-event ingress path.
///
/// This additive function deliberately does not extend the version-1 host
/// transport vtable. Health starts false and can be changed only while an
/// asynchronous host-backed core remains open.
///
/// # Safety
///
/// `handle` must be null or live. The allocation must remain live for this
/// call and the caller must join any race with close before freeing it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn logos_inspector_core_set_runtime_module_event_health(
    handle: *mut LogosInspectorCore,
    ready: i32,
) -> i32 {
    let accepted = catch_unwind(AssertUnwindSafe(|| {
        if !matches!(ready, 0 | 1) {
            return false;
        }
        let Ok(core) = core_ref(handle) else {
            return false;
        };
        match &core.mode {
            CoreMode::Synchronous(_) => false,
            CoreMode::Asynchronous(core) => core.set_native_runtime_module_events_ready(ready == 1),
        }
    }))
    .unwrap_or(false);
    i32::from(accepted)
}

/// Copies and queues one asynchronous host module event.
///
/// # Safety
///
/// `handle` must be null or live. All string pointers must remain readable
/// NUL-terminated UTF-8 for this call. The host must quiesce event ingress
/// before its close callback returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn logos_inspector_core_ingest_module_event(
    handle: *mut LogosInspectorCore,
    module: *const c_char,
    event: *const c_char,
    args_json: *const c_char,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Ok((core, module, event, args)) = module_event_inputs(handle, module, event, args_json)
        else {
            return EVENT_REJECTED;
        };
        match &core.mode {
            CoreMode::Synchronous(_) => EVENT_REJECTED,
            CoreMode::Asynchronous(core) => core.enqueue_module_event(module, event, args),
        }
    }))
    .unwrap_or(EVENT_REJECTED)
}

/// Cancels one accepted asynchronous bridge call.
///
/// # Safety
///
/// `handle` must be null or a live handle returned by this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn logos_inspector_core_cancel(
    handle: *mut LogosInspectorCore,
    bridge_request_id: u64,
) -> i32 {
    let cancelled = catch_unwind(AssertUnwindSafe(|| {
        let Ok(core) = core_ref(handle) else {
            return false;
        };
        match &core.mode {
            CoreMode::Synchronous(_) => false,
            CoreMode::Asynchronous(core) => core.cancel(BridgeRequestId(bridge_request_id)),
        }
    }))
    .unwrap_or(false);
    i32::from(cancelled)
}

/// Releases a string returned by this library.
///
/// # Safety
///
/// `value` must be null or a pointer returned by `logos_inspector_core_call` or
/// `logos_inspector_core_call_module` that has not already been released.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn logos_inspector_core_string_free(value: *mut c_char) {
    if value.is_null() {
        return;
    }

    let _result = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: `value` must come from `CString::into_raw` in this library.
        unsafe {
            drop(CString::from_raw(value));
        }
    }));
}

impl LogosInspectorCore {
    fn call_inspector(&self, method: &str, args_json: &str) -> String {
        match &self.mode {
            CoreMode::Synchronous(core) if !core.closed.load(Ordering::Acquire) => {
                core.bridge.call_inspector_json(method, args_json)
            }
            CoreMode::Synchronous(_) => InspectorBridge::error_json(HOST_CLOSED_ERROR),
            CoreMode::Asynchronous(core) => {
                core.call_local_inspector(method.to_owned(), args_json.to_owned())
            }
        }
    }

    fn call_module(&self, module: &str, method: &str, args_json: &str) -> String {
        match &self.mode {
            CoreMode::Synchronous(core) if !core.closed.load(Ordering::Acquire) => {
                core.bridge.call_module_json(module, method, args_json)
            }
            CoreMode::Synchronous(_) => InspectorBridge::error_json(HOST_CLOSED_ERROR),
            CoreMode::Asynchronous(_) => InspectorBridge::error_json(ASYNC_REQUIRED_ERROR),
        }
    }
}

fn call_inputs(
    handle: *mut LogosInspectorCore,
    method: *const c_char,
    args_json: *const c_char,
) -> Result<(&'static LogosInspectorCore, String, String), String> {
    Ok((
        core_ref(handle)?,
        c_string(method, "method")?,
        c_string(args_json, "args JSON")?,
    ))
}

fn call_module_inputs(
    handle: *mut LogosInspectorCore,
    module: *const c_char,
    method: *const c_char,
    args_json: *const c_char,
) -> Result<(&'static LogosInspectorCore, String, String, String), String> {
    Ok((
        core_ref(handle)?,
        c_string(module, "module")?,
        c_string(method, "method")?,
        c_string(args_json, "args JSON")?,
    ))
}

fn module_event_inputs(
    handle: *mut LogosInspectorCore,
    module: *const c_char,
    event: *const c_char,
    args_json: *const c_char,
) -> Result<(&'static LogosInspectorCore, String, String, Vec<Value>), String> {
    let module = c_string(module, "module")?;
    let event = c_string(event, "event")?;
    let args_json = c_string(args_json, "module event args JSON")?;
    let module = module.trim();
    let event = event.trim();
    if module.is_empty() {
        return Err("module is required".to_owned());
    }
    if event.is_empty() {
        return Err("event is required".to_owned());
    }
    let args = serde_json::from_str::<Value>(&args_json)
        .map_err(|error| format!("module event args are not valid JSON: {error}"))?;
    let Value::Array(args) = args else {
        return Err("module event args must be a JSON array".to_owned());
    };
    Ok((core_ref(handle)?, module.to_owned(), event.to_owned(), args))
}

fn core_ref(handle: *mut LogosInspectorCore) -> Result<&'static LogosInspectorCore, String> {
    if handle.is_null() {
        return Err("logos inspector core is not initialized".to_owned());
    }

    // SAFETY: caller passes an opaque handle returned by
    // `logos_inspector_core_new`; lifetime is bounded by the host module.
    Ok(unsafe { &*handle })
}

fn c_string(value: *const c_char, label: &str) -> Result<String, String> {
    if value.is_null() {
        return Err(format!("{label} is required"));
    }

    // SAFETY: caller provides a valid NUL-terminated C string for the duration
    // of this call.
    unsafe { CStr::from_ptr(value) }
        .to_str()
        .map(ToOwned::to_owned)
        .map_err(|error| format!("{label} is not valid UTF-8: {error}"))
}

fn into_c_string(value: String) -> *mut c_char {
    let sanitized = value.replace('\0', "\\u0000");
    match CString::new(sanitized) {
        Ok(value) => value.into_raw(),
        Err(_) => match CString::new(InspectorBridge::error_json(
            "failed to encode bridge response",
        )) {
            Ok(value) => value.into_raw(),
            Err(_) => ptr::null_mut(),
        },
    }
}

unsafe extern "C" fn host_transport_reply(
    reply_context: *mut c_void,
    module_request_id: u64,
    ok: i32,
    payload_json: *const c_char,
) {
    if reply_context.is_null() {
        return;
    }
    let _result = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: every dispatch receives the stable HostState allocation.
        // Host close quiesces reply callbacks before the owning Arc can drop.
        let state = unsafe { &*reply_context.cast::<HostState>() };
        let result = host_result(ok, payload_json);
        state.complete(ModuleRequestId(module_request_id), result);
    }));
}

fn host_result(ok: i32, payload_json: *const c_char) -> Result<Value, String> {
    let payload = c_string(payload_json, "host response JSON")?;
    if ok == 1 {
        return serde_json::from_str(&payload)
            .map_err(|error| format!("host returned invalid success JSON: {error}"));
    }
    let error = serde_json::from_str::<Value>(&payload)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .unwrap_or(payload);
    Err(error)
}

fn invoke_core_reply(
    pending: PendingCoreRequest,
    bridge_request_id: BridgeRequestId,
    response: &str,
) {
    let response = CString::new(response.replace('\0', "\\u0000")).unwrap_or_default();
    // SAFETY: accepted ingress transfers a live callback/context pair until
    // this one terminal invocation. The string is borrowed for this call.
    unsafe {
        (pending.reply)(
            ptr::with_exposed_provenance_mut(pending.reply_context),
            bridge_request_id.0,
            response.as_ptr(),
        );
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn wait<'a, T>(condition: &Condvar, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
    match condition.wait(guard) {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicI32, AtomicUsize},
        time::{Duration, Instant},
    };

    use super::*;
    use serde_json::Value;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    struct TestHost {
        registry: Mutex<TestHostRegistry>,
        changed: Condvar,
    }

    struct TestHostRegistry {
        reject_dispatch: bool,
        inline_reply: Option<(i32, String)>,
        block_dispatch: bool,
        dispatch_entered: bool,
        release_dispatch: bool,
        requests: Vec<TestHostRequest>,
        cancelled: Vec<u64>,
        close_count: usize,
    }

    #[derive(Clone)]
    struct TestHostRequest {
        id: u64,
        module: String,
        method: String,
        args_json: String,
        reply: LogosInspectorHostReplyFn,
        reply_context: usize,
    }

    #[derive(Default)]
    struct ReplyCollector {
        replies: Mutex<Vec<(u64, String)>>,
        changed: Condvar,
    }

    #[derive(Default)]
    struct ReplyGate {
        state: Mutex<ReplyGateState>,
        changed: Condvar,
    }

    #[derive(Default)]
    struct ReplyGateState {
        entered: bool,
        released: bool,
    }

    struct ReplyContext {
        collector: Arc<ReplyCollector>,
        drops: Arc<AtomicUsize>,
        gate: Option<Arc<ReplyGate>>,
        reenter_handle: Option<usize>,
        reentry_result: Option<Arc<AtomicI32>>,
    }

    struct TestCoreHandle(*mut LogosInspectorCore);

    impl TestHost {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                registry: Mutex::new(TestHostRegistry {
                    reject_dispatch: false,
                    inline_reply: None,
                    block_dispatch: false,
                    dispatch_entered: false,
                    release_dispatch: false,
                    requests: Vec::new(),
                    cancelled: Vec::new(),
                    close_count: 0,
                }),
                changed: Condvar::new(),
            })
        }

        fn vtable(self: &Arc<Self>) -> LogosInspectorHostTransportV1 {
            LogosInspectorHostTransportV1 {
                abi_version: HOST_TRANSPORT_ABI_VERSION,
                struct_size: size_of::<LogosInspectorHostTransportV1>() as u32,
                context: Arc::as_ptr(self).cast_mut().cast(),
                dispatch: Some(test_host_dispatch),
                cancel: Some(test_host_cancel),
                close: Some(test_host_close),
            }
        }

        fn reject_dispatch(&self) {
            lock(&self.registry).reject_dispatch = true;
        }

        fn reply_inline(&self, ok: i32, payload_json: &str) {
            lock(&self.registry).inline_reply = Some((ok, payload_json.to_owned()));
        }

        fn block_dispatch(&self) {
            let mut registry = lock(&self.registry);
            registry.block_dispatch = true;
            registry.release_dispatch = false;
        }

        fn wait_for_dispatch_entry(&self) -> TestResult {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut registry = lock(&self.registry);
            while !registry.dispatch_entered {
                let now = Instant::now();
                if now >= deadline {
                    return err("timed out waiting for host dispatch entry");
                }
                let remaining = deadline.saturating_duration_since(now);
                let (next, timeout) = match self.changed.wait_timeout(registry, remaining) {
                    Ok(result) => result,
                    Err(poisoned) => poisoned.into_inner(),
                };
                registry = next;
                if timeout.timed_out() && !registry.dispatch_entered {
                    return err("timed out waiting for host dispatch entry");
                }
            }
            Ok(())
        }

        fn release_dispatch(&self) {
            let mut registry = lock(&self.registry);
            registry.release_dispatch = true;
            self.changed.notify_all();
        }

        fn wait_for_request(&self) -> Result<TestHostRequest, Box<dyn std::error::Error>> {
            let requests = self.wait_for_requests(1)?;
            requests
                .first()
                .cloned()
                .ok_or_else(|| std::io::Error::other("host request disappeared").into())
        }

        fn wait_for_requests(
            &self,
            count: usize,
        ) -> Result<Vec<TestHostRequest>, Box<dyn std::error::Error>> {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut registry = lock(&self.registry);
            loop {
                if registry.requests.len() >= count {
                    return Ok(registry.requests.clone());
                }
                let now = Instant::now();
                if now >= deadline {
                    return err("timed out waiting for host request");
                }
                let remaining = deadline.saturating_duration_since(now);
                let (next, timeout) = match self.changed.wait_timeout(registry, remaining) {
                    Ok(result) => result,
                    Err(poisoned) => poisoned.into_inner(),
                };
                registry = next;
                if timeout.timed_out() && registry.requests.len() < count {
                    return err("timed out waiting for host request");
                }
            }
        }

        fn complete(
            &self,
            request_id: u64,
            ok: i32,
            payload_json: &str,
            foreign_thread: bool,
        ) -> TestResult {
            let request = {
                let mut registry = lock(&self.registry);
                let Some(index) = registry
                    .requests
                    .iter()
                    .position(|request| request.id == request_id)
                else {
                    return err("host request was not pending");
                };
                registry.requests.remove(index)
            };
            let payload_json = payload_json.to_owned();
            let invoke = move || -> Result<(), String> {
                let payload = CString::new(payload_json).map_err(|error| error.to_string())?;
                // SAFETY: the accepted host request owns this reply context
                // until this one completion.
                unsafe {
                    (request.reply)(
                        ptr::with_exposed_provenance_mut(request.reply_context),
                        request.id,
                        ok,
                        payload.as_ptr(),
                    );
                }
                Ok(())
            };
            if foreign_thread {
                thread::spawn(invoke)
                    .join()
                    .map_err(|_| std::io::Error::other("host completion thread panicked"))?
                    .map_err(std::io::Error::other)?;
                return Ok(());
            }
            invoke().map_err(|error| std::io::Error::other(error).into())
        }

        fn cancelled(&self) -> Vec<u64> {
            lock(&self.registry).cancelled.clone()
        }

        fn wait_for_cancellations(
            &self,
            count: usize,
        ) -> Result<Vec<u64>, Box<dyn std::error::Error>> {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut registry = lock(&self.registry);
            while registry.cancelled.len() < count {
                let now = Instant::now();
                if now >= deadline {
                    return err("timed out waiting for host cancellation");
                }
                let remaining = deadline.saturating_duration_since(now);
                let (next, timeout) = match self.changed.wait_timeout(registry, remaining) {
                    Ok(result) => result,
                    Err(poisoned) => poisoned.into_inner(),
                };
                registry = next;
                if timeout.timed_out() && registry.cancelled.len() < count {
                    return err("timed out waiting for host cancellation");
                }
            }
            Ok(registry.cancelled.clone())
        }

        fn close_count(&self) -> usize {
            lock(&self.registry).close_count
        }
    }

    impl Drop for ReplyContext {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::AcqRel);
        }
    }

    impl ReplyCollector {
        fn wait_for_replies(
            &self,
            count: usize,
        ) -> Result<Vec<(u64, String)>, Box<dyn std::error::Error>> {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut replies = lock(&self.replies);
            loop {
                if replies.len() >= count {
                    return Ok(replies.clone());
                }
                let now = Instant::now();
                if now >= deadline {
                    return err("timed out waiting for core reply");
                }
                let remaining = deadline.saturating_duration_since(now);
                let (next, timeout) = match self.changed.wait_timeout(replies, remaining) {
                    Ok(result) => result,
                    Err(poisoned) => poisoned.into_inner(),
                };
                replies = next;
                if timeout.timed_out() && replies.len() < count {
                    return err("timed out waiting for core reply");
                }
            }
        }

        fn count(&self) -> usize {
            lock(&self.replies).len()
        }
    }

    impl ReplyGate {
        fn enter_and_wait(&self) {
            let mut state = lock(&self.state);
            state.entered = true;
            self.changed.notify_all();
            while !state.released {
                state = wait(&self.changed, state);
            }
        }

        fn wait_for_entry(&self) -> TestResult {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut state = lock(&self.state);
            while !state.entered {
                let now = Instant::now();
                if now >= deadline {
                    return err("timed out waiting for core reply callback entry");
                }
                let remaining = deadline.saturating_duration_since(now);
                let (next, timeout) = match self.changed.wait_timeout(state, remaining) {
                    Ok(result) => result,
                    Err(poisoned) => poisoned.into_inner(),
                };
                state = next;
                if timeout.timed_out() && !state.entered {
                    return err("timed out waiting for core reply callback entry");
                }
            }
            Ok(())
        }

        fn release(&self) {
            let mut state = lock(&self.state);
            state.released = true;
            self.changed.notify_all();
        }
    }

    impl TestCoreHandle {
        fn new(host: &Arc<TestHost>) -> Result<Self, Box<dyn std::error::Error>> {
            let vtable = host.vtable();
            // SAFETY: vtable and host context satisfy the constructor contract.
            let handle = unsafe { logos_inspector_core_new_with_host_transport(&vtable) };
            if handle.is_null() {
                return err("failed to create asynchronous core handle");
            }
            // SAFETY: this helper owns a live asynchronous handle and models a
            // native adapter with its complete event catalog armed.
            if unsafe { logos_inspector_core_set_runtime_module_event_health(handle, 1) } != 1 {
                // SAFETY: constructor ownership has not escaped this helper.
                unsafe {
                    logos_inspector_core_free(handle);
                }
                return err("failed to publish healthy native event ownership");
            }
            Ok(Self(handle))
        }

        fn as_ptr(&self) -> *mut LogosInspectorCore {
            self.0
        }

        fn close(&self) {
            // SAFETY: this guard owns a live handle.
            unsafe {
                logos_inspector_core_close(self.0);
            }
        }

        fn begin_bridge_close(&self) -> TestResult {
            // SAFETY: this guard keeps the core allocation live. The close
            // handle is thread-safe and does not move the bridge worker.
            let core = unsafe { &*self.0 };
            let CoreMode::Asynchronous(core) = &core.mode else {
                return err("expected asynchronous test core");
            };
            core.bridge_close
                .begin_close()
                .map_err(|error| std::io::Error::other(error.to_string()).into())
        }

        fn assert_shutdown_drained(&self) -> TestResult {
            // SAFETY: this guard keeps the explicitly closed allocation live
            // while synchronized lifecycle state is inspected.
            let core = unsafe { &*self.0 };
            let CoreMode::Asynchronous(core) = &core.mode else {
                return err("expected asynchronous test core");
            };
            let state = lock(&core.state.registry);
            if state.phase != LifecyclePhase::Closed
                || !state.pending.is_empty()
                || !state.completing.is_empty()
            {
                return err("core request lifecycle did not drain on close");
            }
            drop(state);
            let host = lock(&core.host.registry);
            if host.phase != LifecyclePhase::Closed
                || host.active_host_calls != 0
                || !host.pending.is_empty()
            {
                return err("host transport lifecycle did not drain on close");
            }
            drop(host);
            if lock(&core.worker).is_some() {
                return err("bridge worker remained live after close");
            }
            Ok(())
        }

        fn wait_for_core_closing(&self) -> TestResult {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                // SAFETY: this guard keeps the core allocation live. Close may
                // mutate only through synchronized interior state.
                let core = unsafe { &*self.0 };
                let CoreMode::Asynchronous(core) = &core.mode else {
                    return err("expected asynchronous test core");
                };
                if lock(&core.state.registry).phase != LifecyclePhase::Open {
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return err("timed out waiting for core closing phase");
                }
                thread::yield_now();
            }
        }

        fn wait_for_shutdown_queued(&self) -> TestResult {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                // SAFETY: this guard keeps the allocation live while only
                // atomic test instrumentation is inspected.
                let core = unsafe { &*self.0 };
                let CoreMode::Asynchronous(core) = &core.mode else {
                    return err("expected asynchronous test core");
                };
                if core.state.shutdown_queued.load(Ordering::Acquire) {
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return err("timed out waiting for bridge shutdown command");
                }
                thread::yield_now();
            }
        }

        fn wait_for_queued_local_call(&self) -> TestResult {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                // SAFETY: this guard keeps the core allocation live and only
                // synchronized test instrumentation is inspected.
                let core = unsafe { &*self.0 };
                let CoreMode::Asynchronous(core) = &core.mode else {
                    return err("expected asynchronous test core");
                };
                if core.state.queued_local_calls.load(Ordering::Acquire) > 0 {
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return err("timed out waiting for queued local call");
                }
                thread::yield_now();
            }
        }

        fn wait_for_bridge_id_available(&self, bridge_request_id: u64) -> TestResult {
            let deadline = Instant::now() + Duration::from_secs(5);
            let bridge_request_id = BridgeRequestId(bridge_request_id);
            loop {
                // SAFETY: this guard keeps the core allocation live and only
                // synchronized interior state is inspected.
                let core = unsafe { &*self.0 };
                let CoreMode::Asynchronous(core) = &core.mode else {
                    return err("expected asynchronous test core");
                };
                let registry = lock(&core.state.registry);
                if !registry.pending.contains_key(&bridge_request_id)
                    && !registry.completing.contains_key(&bridge_request_id)
                {
                    return Ok(());
                }
                drop(registry);
                if Instant::now() >= deadline {
                    return err("timed out waiting for bridge id release");
                }
                thread::yield_now();
            }
        }
    }

    impl Drop for TestCoreHandle {
        fn drop(&mut self) {
            // SAFETY: this guard is the unique allocation owner. Tests do not
            // race drop with another ABI call.
            unsafe {
                logos_inspector_core_free(self.0);
            }
        }
    }

    unsafe extern "C" fn test_host_dispatch(
        host_context: *mut c_void,
        module_request_id: u64,
        module: *const c_char,
        method: *const c_char,
        args_json: *const c_char,
        reply: LogosInspectorHostReplyFn,
        reply_context: *mut c_void,
    ) -> i32 {
        if host_context.is_null() {
            return 0;
        }
        // SAFETY: each test keeps its Arc host alive through core close.
        let host = unsafe { &*host_context.cast::<TestHost>() };
        let Ok(module) = c_string(module, "module") else {
            return 0;
        };
        let Ok(method) = c_string(method, "method") else {
            return 0;
        };
        let Ok(args_json) = c_string(args_json, "args JSON") else {
            return 0;
        };
        let request = TestHostRequest {
            id: module_request_id,
            module,
            method,
            args_json,
            reply,
            reply_context: reply_context.expose_provenance(),
        };
        let inline_reply = {
            let mut registry = lock(&host.registry);
            if registry.reject_dispatch {
                return 0;
            }
            let inline_reply = registry.inline_reply.take();
            if inline_reply.is_none() {
                registry.requests.push(request.clone());
            }
            registry.dispatch_entered = true;
            host.changed.notify_all();
            while registry.block_dispatch && !registry.release_dispatch {
                registry = wait(&host.changed, registry);
            }
            inline_reply
        };
        if let Some((ok, payload_json)) = inline_reply {
            let Ok(payload_json) = CString::new(payload_json) else {
                return 0;
            };
            // SAFETY: inline completion borrows the core callback context for
            // this accepted dispatch.
            unsafe {
                (request.reply)(
                    ptr::with_exposed_provenance_mut(request.reply_context),
                    request.id,
                    ok,
                    payload_json.as_ptr(),
                );
            }
        }
        1
    }

    unsafe extern "C" fn test_host_cancel(host_context: *mut c_void, module_request_id: u64) {
        if host_context.is_null() {
            return;
        }
        // SAFETY: each test keeps its Arc host alive through core close.
        let host = unsafe { &*host_context.cast::<TestHost>() };
        lock(&host.registry).cancelled.push(module_request_id);
        host.changed.notify_all();
    }

    unsafe extern "C" fn test_host_close(host_context: *mut c_void) {
        if host_context.is_null() {
            return;
        }
        // SAFETY: each test keeps its Arc host alive through close.
        let host = unsafe { &*host_context.cast::<TestHost>() };
        let mut registry = lock(&host.registry);
        registry.close_count += 1;
        registry.requests.clear();
        host.changed.notify_all();
    }

    unsafe extern "C" fn collect_core_reply(
        context: *mut c_void,
        bridge_request_id: u64,
        response_json: *const c_char,
    ) {
        if context.is_null() {
            return;
        }
        // SAFETY: accepted ingress transfers this unique context until its one
        // terminal callback.
        let context = unsafe { Box::from_raw(context.cast::<ReplyContext>()) };
        let response = if response_json.is_null() {
            String::new()
        } else {
            // SAFETY: the core lends a NUL-terminated response for this call.
            unsafe { CStr::from_ptr(response_json) }
                .to_string_lossy()
                .into_owned()
        };
        if let Some(gate) = context.gate.as_ref() {
            gate.enter_and_wait();
        }
        if let (Some(handle), Some(result)) =
            (context.reenter_handle, context.reentry_result.as_ref())
        {
            // SAFETY: the test keeps the handle open through this callback;
            // cancel is an allowed non-closing reentrant call.
            let value = unsafe {
                logos_inspector_core_cancel(ptr::with_exposed_provenance_mut(handle), u64::MAX)
            };
            result.store(value, Ordering::Release);
        }
        let collector = Arc::clone(&context.collector);
        lock(&collector.replies).push((bridge_request_id, response));
        drop(context);
        collector.changed.notify_all();
    }

    fn reply_context(collector: &Arc<ReplyCollector>, drops: &Arc<AtomicUsize>) -> *mut c_void {
        Box::into_raw(Box::new(ReplyContext {
            collector: Arc::clone(collector),
            drops: Arc::clone(drops),
            gate: None,
            reenter_handle: None,
            reentry_result: None,
        }))
        .cast()
    }

    fn gated_reply_context(
        collector: &Arc<ReplyCollector>,
        drops: &Arc<AtomicUsize>,
        gate: &Arc<ReplyGate>,
    ) -> *mut c_void {
        Box::into_raw(Box::new(ReplyContext {
            collector: Arc::clone(collector),
            drops: Arc::clone(drops),
            gate: Some(Arc::clone(gate)),
            reenter_handle: None,
            reentry_result: None,
        }))
        .cast()
    }

    fn reentrant_reply_context(
        collector: &Arc<ReplyCollector>,
        drops: &Arc<AtomicUsize>,
        handle: *mut LogosInspectorCore,
        result: &Arc<AtomicI32>,
    ) -> *mut c_void {
        Box::into_raw(Box::new(ReplyContext {
            collector: Arc::clone(collector),
            drops: Arc::clone(drops),
            gate: None,
            reenter_handle: Some(handle.expose_provenance()),
            reentry_result: Some(Arc::clone(result)),
        }))
        .cast()
    }

    fn enqueue_test_call(
        handle: *mut LogosInspectorCore,
        bridge_request_id: u64,
        module: &str,
        method: &str,
        args_json: &str,
        context: *mut c_void,
    ) -> Result<i32, Box<dyn std::error::Error>> {
        let module = CString::new(module)?;
        let method = CString::new(method)?;
        let args_json = CString::new(args_json)?;
        // SAFETY: all pointers are valid for this call; callback context follows
        // the returned ownership bit.
        Ok(unsafe {
            logos_inspector_core_call_module_async(
                handle,
                bridge_request_id,
                module.as_ptr(),
                method.as_ptr(),
                args_json.as_ptr(),
                Some(collect_core_reply),
                context,
            )
        })
    }

    fn call_test_inspector(
        handle: *mut LogosInspectorCore,
        method: &str,
        args_json: &str,
    ) -> Result<Value, String> {
        let method = CString::new(method).map_err(|error| error.to_string())?;
        let args_json = CString::new(args_json).map_err(|error| error.to_string())?;
        // SAFETY: the owning test keeps the handle allocation live and both
        // strings remain readable for this call.
        let response =
            unsafe { logos_inspector_core_call(handle, method.as_ptr(), args_json.as_ptr()) };
        if response.is_null() {
            return Err("FFI returned null string".to_owned());
        }
        // SAFETY: this library returns a live NUL-terminated string.
        let text = unsafe { CStr::from_ptr(response) }
            .to_string_lossy()
            .into_owned();
        // SAFETY: the response pointer is released exactly once after copying.
        unsafe {
            logos_inspector_core_string_free(response);
        }
        serde_json::from_str(&text).map_err(|error| error.to_string())
    }

    fn ingest_test_module_event(
        handle: *mut LogosInspectorCore,
        module: &str,
        event: &str,
        args_json: &str,
    ) -> Result<i32, Box<dyn std::error::Error>> {
        let module = CString::new(module)?;
        let event = CString::new(event)?;
        let args_json = CString::new(args_json)?;
        // SAFETY: every string remains readable for this call. The ingress
        // function copies accepted inputs before returning.
        Ok(unsafe {
            logos_inspector_core_ingest_module_event(
                handle,
                module.as_ptr(),
                event.as_ptr(),
                args_json.as_ptr(),
            )
        })
    }

    fn operation_status(
        handle: *mut LogosInspectorCore,
        bridge_request_id: u64,
        operation_id: &str,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));
        let context = reply_context(&collector, &drops);
        let args = serde_json::json!([operation_id]).to_string();
        if enqueue_test_call(
            handle,
            bridge_request_id,
            "logos_inspector",
            "runtimeOperationStatus",
            &args,
            context,
        )? != 1
        {
            // SAFETY: rejected ingress leaves its callback context caller-owned.
            unsafe {
                drop(Box::from_raw(context.cast::<ReplyContext>()));
            }
            return err("runtime operation status ingress was rejected");
        }
        let replies = collector.wait_for_replies(1)?;
        if drops.load(Ordering::Acquire) != 1 {
            return err("runtime operation status callback context was not released once");
        }
        Ok(serde_json::from_str(&replies[0].1)?)
    }

    fn wait_for_operation_status(
        handle: *mut LogosInspectorCore,
        operation_id: &str,
        expected: &str,
        mut bridge_request_id: u64,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let status = operation_status(handle, bridge_request_id, operation_id)?;
            if status.pointer("/value/status").and_then(Value::as_str) == Some(expected) {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                return Err(std::io::Error::other(format!(
                    "runtime operation did not reach {expected}: {status}"
                ))
                .into());
            }
            bridge_request_id = bridge_request_id
                .checked_add(1)
                .ok_or_else(|| std::io::Error::other("status request id space exhausted"))?;
            thread::yield_now();
        }
    }

    #[test]
    fn call_returns_error_for_null_handle() -> TestResult {
        let method = CString::new("moduleVersion")?;
        let args = CString::new("[]")?;

        // SAFETY: null handle is an accepted error path for this FFI call.
        let ptr =
            unsafe { logos_inspector_core_call(ptr::null_mut(), method.as_ptr(), args.as_ptr()) };
        let value = response_value(ptr)?;

        if value.get("ok").and_then(Value::as_bool) != Some(false) {
            return err("expected error response");
        }
        expect_error_envelope_shape(&value)?;
        if value
            .get("error")
            .and_then(Value::as_str)
            .is_none_or(|error| !error.contains("not initialized"))
        {
            return err("expected initialization error");
        }
        Ok(())
    }

    #[test]
    fn call_rejects_null_method() -> TestResult {
        let handle = logos_inspector_core_new();
        if handle.is_null() {
            return err("failed to create core handle");
        }
        let args = CString::new("[]")?;

        // SAFETY: handle was created by this library; null method is an
        // accepted error path for this FFI call.
        let ptr = unsafe { logos_inspector_core_call(handle, ptr::null(), args.as_ptr()) };
        let value = response_value(ptr)?;

        // SAFETY: handle was created by this library and not yet released.
        unsafe {
            logos_inspector_core_free(handle);
        }

        if value.get("ok").and_then(Value::as_bool) != Some(false) {
            return err("expected error response");
        }
        expect_error_envelope_shape(&value)?;
        if value
            .get("error")
            .and_then(Value::as_str)
            .is_none_or(|error| !error.contains("method is required"))
        {
            return err("expected method error");
        }
        Ok(())
    }

    #[test]
    fn returned_strings_escape_interior_nul() -> TestResult {
        let ptr = into_c_string("a\0b".to_owned());
        let text = c_string_from_owned_ptr(ptr)?;

        if text != "a\\u0000b" {
            return err("expected escaped interior nul");
        }
        Ok(())
    }

    #[test]
    fn handles_keep_independent_command_surfaces() -> TestResult {
        let module = CString::new("logos_inspector")?;
        let method = CString::new("sourcePolicy")?;
        let args = CString::new("[]")?;
        let first = logos_inspector_core_new();
        let second = logos_inspector_core_new();
        if first.is_null() || second.is_null() || first == second {
            // SAFETY: null is accepted; non-null handles were created above.
            unsafe {
                logos_inspector_core_free(first);
                logos_inspector_core_free(second);
            }
            return err("failed to create independent core handles");
        }

        // SAFETY: both handles and C strings remain live for these calls.
        let first_response = unsafe {
            logos_inspector_core_call_module(first, module.as_ptr(), method.as_ptr(), args.as_ptr())
        };
        // SAFETY: both handles and C strings remain live for these calls.
        let second_response = unsafe {
            logos_inspector_core_call_module(
                second,
                module.as_ptr(),
                method.as_ptr(),
                args.as_ptr(),
            )
        };
        let first_value = response_value(first_response)?;
        let second_value = response_value(second_response)?;

        // SAFETY: first was created above and has not been released.
        unsafe {
            logos_inspector_core_free(first);
        }
        // SAFETY: second remains live after first is released.
        let surviving_response = unsafe {
            logos_inspector_core_call_module(
                second,
                module.as_ptr(),
                method.as_ptr(),
                args.as_ptr(),
            )
        };
        let surviving_value = response_value(surviving_response)?;
        // SAFETY: second was created above and has not been released.
        unsafe {
            logos_inspector_core_free(second);
        }

        if first_value.get("ok").and_then(Value::as_bool) != Some(true)
            || second_value.get("ok").and_then(Value::as_bool) != Some(true)
            || surviving_value.get("ok").and_then(Value::as_bool) != Some(true)
        {
            return Err(std::io::Error::other(format!(
                "independent core handle call failed: first={first_value}, second={second_value}, surviving={surviving_value}"
            ))
            .into());
        }
        if second_value.get("value") != surviving_value.get("value") {
            return err("surviving core handle changed after sibling release");
        }
        Ok(())
    }

    #[test]
    fn new_handle_fails_external_module_calls_closed_without_cli_fallback() -> TestResult {
        let module = CString::new("logos_blockchain")?;
        let method = CString::new("getCryptarchiaInfo")?;
        let args = CString::new("[]")?;
        let handle = logos_inspector_core_new();
        if handle.is_null() {
            return err("failed to create core handle");
        }

        // SAFETY: handle and C strings remain live for this call.
        let response = unsafe {
            logos_inspector_core_call_module(
                handle,
                module.as_ptr(),
                method.as_ptr(),
                args.as_ptr(),
            )
        };
        let value = response_value(response);
        // SAFETY: handle was created above and has not been released.
        unsafe {
            logos_inspector_core_free(handle);
        }
        let value = value?;

        if value.get("ok").and_then(Value::as_bool) != Some(false) {
            return err("expected fail-closed error response");
        }
        expect_error_envelope_shape(&value)?;
        if value.get("error").and_then(Value::as_str)
            != Some(
                "Basecamp host module transport is unavailable: no host transport was configured for this core handle",
            )
        {
            return Err(std::io::Error::other(format!(
                "unexpected Basecamp transport error: {value}"
            ))
            .into());
        }
        if value.get("error_details").is_some() {
            return err("unexpected structured details in transport error");
        }
        Ok(())
    }

    #[test]
    fn host_transport_constructor_rejects_unknown_abi_without_taking_context() -> TestResult {
        let host = TestHost::new();
        let mut vtable = host.vtable();
        vtable.abi_version = HOST_TRANSPORT_ABI_VERSION + 1;

        // SAFETY: the vtable remains readable for this constructor call.
        let handle = unsafe { logos_inspector_core_new_with_host_transport(&vtable) };

        if !handle.is_null() {
            // SAFETY: unexpected non-null result still came from this library.
            unsafe {
                logos_inspector_core_free(handle);
            }
            return err("unknown host transport ABI was accepted");
        }
        if host.close_count() != 0 {
            return err("rejected constructor took ownership of host context");
        }
        Ok(())
    }

    #[test]
    fn host_transport_constructor_rejects_incomplete_vtables() -> TestResult {
        let host = TestHost::new();
        let mut undersized = host.vtable();
        undersized.struct_size = 0;
        let mut missing_dispatch = host.vtable();
        missing_dispatch.dispatch = None;
        let mut missing_close = host.vtable();
        missing_close.close = None;

        // SAFETY: each vtable remains readable for its constructor call.
        let handles = unsafe {
            [
                logos_inspector_core_new_with_host_transport(&undersized),
                logos_inspector_core_new_with_host_transport(&missing_dispatch),
                logos_inspector_core_new_with_host_transport(&missing_close),
            ]
        };
        if handles.iter().any(|handle| !handle.is_null()) {
            for handle in handles {
                // SAFETY: null is accepted; any non-null value came from this
                // library and must be released before failing the test.
                unsafe {
                    logos_inspector_core_free(handle);
                }
            }
            return err("incomplete host transport vtable was accepted");
        }
        if host.close_count() != 0 {
            return err("rejected vtable transferred host ownership");
        }
        Ok(())
    }

    #[test]
    fn native_runtime_event_health_is_explicit_and_closes_fail_closed() -> TestResult {
        let host = TestHost::new();
        let vtable = host.vtable();
        // SAFETY: vtable and host context satisfy the constructor contract.
        let handle = unsafe { logos_inspector_core_new_with_host_transport(&vtable) };
        if handle.is_null() {
            return err("failed to create asynchronous health-test handle");
        }

        // SAFETY: this test owns the live handle through all calls below.
        let initial_ready = unsafe {
            let core = &*handle;
            let CoreMode::Asynchronous(core) = &core.mode else {
                logos_inspector_core_free(handle);
                return err("health-test handle was not asynchronous");
            };
            core.host
                .native_runtime_module_events_ready
                .load(Ordering::Acquire)
        };
        if initial_ready
            // SAFETY: live asynchronous handle; invalid values must fail closed.
            || unsafe { logos_inspector_core_set_runtime_module_event_health(handle, 2) } != 0
            // SAFETY: live asynchronous handle; native activation publishes health.
            || unsafe { logos_inspector_core_set_runtime_module_event_health(handle, 1) } != 1
        {
            // SAFETY: this test still owns the allocation.
            unsafe {
                logos_inspector_core_free(handle);
            }
            return err("native event health admission did not start explicit and fail closed");
        }

        // SAFETY: the allocation is live and only synchronized state is read.
        let ready = unsafe {
            let core = &*handle;
            let CoreMode::Asynchronous(core) = &core.mode else {
                logos_inspector_core_free(handle);
                return err("health-test handle changed mode");
            };
            core.host
                .native_runtime_module_events_ready
                .load(Ordering::Acquire)
        };
        if !ready {
            // SAFETY: this test still owns the allocation.
            unsafe {
                logos_inspector_core_free(handle);
            }
            return err("accepted native event health was not visible to Rust transport");
        }

        // SAFETY: this test owns the live handle.
        unsafe {
            logos_inspector_core_close(handle);
        }
        // SAFETY: closed allocation remains live until free.
        if unsafe { logos_inspector_core_set_runtime_module_event_health(handle, 1) } != 0 {
            // SAFETY: this test still owns the closed allocation.
            unsafe {
                logos_inspector_core_free(handle);
            }
            return err("closing core accepted healthy native event ownership");
        }
        // SAFETY: closed allocation remains live and synchronized state is read.
        let closed_ready = unsafe {
            let core = &*handle;
            let CoreMode::Asynchronous(core) = &core.mode else {
                logos_inspector_core_free(handle);
                return err("closed health-test handle changed mode");
            };
            core.host
                .native_runtime_module_events_ready
                .load(Ordering::Acquire)
        };
        // SAFETY: final matching release; no calls race this free.
        unsafe {
            logos_inspector_core_free(handle);
        }
        if closed_ready || host.close_count() != 1 {
            return err("close retained native event health or repeated host close");
        }
        Ok(())
    }

    #[test]
    fn dropping_module_future_owns_host_cancellation() -> TestResult {
        let host = TestHost::new();
        let vtable = host.vtable();
        // SAFETY: the local vtable provides a complete readable v1 prefix.
        let copied = unsafe { HostVtable::copy_from(&vtable) }.map_err(std::io::Error::other)?;
        let state = HostState::new(copied);
        let call = ModuleCall::new(
            ModuleTransportKind::Module,
            "storage_module",
            "space",
            Vec::new(),
        )?;
        let mut future = state.dispatch(call);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        if std::future::Future::poll(future.as_mut(), &mut context).is_ready() {
            return err("host module future completed before its reply");
        }
        let request = host.wait_for_request()?;
        drop(future);
        if host.wait_for_cancellations(1)? != vec![request.id] {
            return err("dropped module future did not cancel its host request");
        }
        host.complete(request.id, 1, "12", false)?;
        state.close();
        Ok(())
    }

    #[test]
    fn completed_host_dispatch_preserves_callback_identity() -> TestResult {
        let host = TestHost::new();
        let vtable = host.vtable();
        // SAFETY: the local vtable provides a complete readable v1 prefix.
        let copied = unsafe { HostVtable::copy_from(&vtable) }.map_err(std::io::Error::other)?;
        let state = HostState::new(copied);
        let call = ModuleCall::new(
            ModuleTransportKind::Module,
            "delivery_module",
            "send",
            Vec::new(),
        )?;
        let mut future = state.dispatch(call);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        if std::future::Future::poll(future.as_mut(), &mut context).is_ready() {
            return err("host module future completed before its reply");
        }
        let request = host.wait_for_request()?;
        host.complete(request.id, 1, "\"request-7\"", false)?;
        let reply = match std::future::Future::poll(future.as_mut(), &mut context) {
            std::task::Poll::Ready(Ok(reply)) => reply,
            std::task::Poll::Ready(Err(error)) => {
                return err(&format!("host module future failed: {error:#}"));
            }
            std::task::Poll::Pending => {
                return err("host module future remained pending after its reply");
            }
        };
        if reply.bridge_callback_id().map(|id| id.value()) != Some(request.id) {
            return err("host module reply lost its callback identity");
        }
        if reply.into_value() != Value::String("request-7".to_owned()) {
            return err("host module reply value drifted");
        }
        drop(future);
        state.close();
        Ok(())
    }

    #[test]
    fn host_event_subscription_filters_and_preserves_typed_args() -> TestResult {
        let host = TestHost::new();
        let vtable = host.vtable();
        // SAFETY: the local vtable provides a complete readable v1 prefix.
        let copied = unsafe { HostVtable::copy_from(&vtable) }.map_err(std::io::Error::other)?;
        let state = HostState::new(copied);
        let transport = BasecampHostTransport {
            state: Arc::clone(&state),
        };
        let mut subscription =
            transport.subscribe_module_event("storage_module", "storageDownloadDoneV2")?;

        if !transport.supports_shared_file_staging() {
            return err("Basecamp host transport did not advertise shared file staging");
        }
        transport.ingest_module_event(
            "delivery_module",
            "storageDownloadDoneV2",
            &[Value::String("foreign-module".to_owned())],
        )?;
        transport.ingest_module_event(
            "storage_module",
            "storageUploadDone",
            &[Value::String("foreign-event".to_owned())],
        )?;
        if subscription.next_within(Duration::ZERO)?.is_some() {
            return err("filtered host subscription received an unrelated event");
        }

        let args = vec![serde_json::json!({
            "operationId": "download-7",
            "status": "completed",
        })];
        transport.ingest_module_event("storage_module", "storageDownloadDoneV2", &args)?;
        let event = subscription
            .next_within(Duration::from_secs(1))?
            .ok_or_else(|| std::io::Error::other("matching host event was not delivered"))?;
        if event.module() != "storage_module"
            || event.event() != "storageDownloadDoneV2"
            || event.args() != args
        {
            return err("host subscription changed typed event identity or arguments");
        }

        drop(subscription);
        state.close();
        Ok(())
    }

    #[test]
    fn host_event_subscription_reports_overflow_and_close() -> TestResult {
        let host = TestHost::new();
        let vtable = host.vtable();
        // SAFETY: the local vtable provides a complete readable v1 prefix.
        let copied = unsafe { HostVtable::copy_from(&vtable) }.map_err(std::io::Error::other)?;
        let state = HostState::new(copied);
        let transport = BasecampHostTransport {
            state: Arc::clone(&state),
        };
        let mut overflowed =
            transport.subscribe_module_event("storage_module", "storageDownloadDoneV2")?;
        for index in 0..HOST_EVENT_SUBSCRIPTION_CAPACITY {
            transport.ingest_module_event(
                "storage_module",
                "storageDownloadDoneV2",
                &[serde_json::json!({ "sequence": index })],
            )?;
        }
        let publish_error = transport
            .ingest_module_event(
                "storage_module",
                "storageDownloadDoneV2",
                &[serde_json::json!({ "sequence": HOST_EVENT_SUBSCRIPTION_CAPACITY })],
            )
            .err()
            .ok_or_else(|| std::io::Error::other("full host event queue accepted another event"))?;
        if publish_error.to_string() != HOST_EVENT_SUBSCRIPTION_OVERFLOW_ERROR {
            return err("host event publisher lost explicit overflow evidence");
        }
        let receive_error = overflowed
            .next_within(Duration::ZERO)
            .err()
            .ok_or_else(|| std::io::Error::other("overflowed subscription returned queued data"))?;
        if receive_error.to_string() != HOST_EVENT_SUBSCRIPTION_OVERFLOW_ERROR {
            return err("host event subscriber lost explicit overflow evidence");
        }

        let mut disconnected =
            transport.subscribe_module_event("delivery_module", "messageSent")?;
        state.close();
        let close_error = disconnected
            .next_within(Duration::ZERO)
            .err()
            .ok_or_else(|| std::io::Error::other("closed subscription remained connected"))?;
        if close_error.to_string() != HOST_CLOSED_ERROR
            || close_error
                .downcast_ref::<ModuleTransportClosed>()
                .is_none()
        {
            return err("host subscription close lost typed transport closure");
        }
        Ok(())
    }

    #[test]
    fn host_module_info_merges_methods_and_events() -> TestResult {
        let host = TestHost::new();
        let vtable = host.vtable();
        // SAFETY: the local vtable provides a complete readable v1 prefix.
        let copied = unsafe { HostVtable::copy_from(&vtable) }.map_err(std::io::Error::other)?;
        let state = HostState::new(copied);
        let transport = BasecampHostTransport {
            state: Arc::clone(&state),
        };
        let mut future = transport.module_info("storage_module".to_owned());
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        if std::future::Future::poll(future.as_mut(), &mut context).is_ready() {
            return err("host module info completed before method metadata reply");
        }
        let methods_request = host.wait_for_request()?;
        if methods_request.module != "storage_module"
            || methods_request.method != "getPluginMethods"
            || methods_request.args_json != "[]"
        {
            return err("host module info issued the wrong method metadata call");
        }
        host.complete(
            methods_request.id,
            1,
            r#"[{"name":"downloadProtocol","signature":"downloadProtocol()"}]"#,
            false,
        )?;
        if std::future::Future::poll(future.as_mut(), &mut context).is_ready() {
            return err("host module info completed before event metadata reply");
        }
        let events_request = host.wait_for_request()?;
        if events_request.module != "storage_module"
            || events_request.method != "getPluginEvents"
            || events_request.args_json != "[]"
        {
            return err("host module info issued the wrong event metadata call");
        }
        host.complete(
            events_request.id,
            1,
            r#"[{"name":"storageDownloadDoneV2","signature":"storageDownloadDoneV2(QString)"}]"#,
            false,
        )?;
        let info = match std::future::Future::poll(future.as_mut(), &mut context) {
            std::task::Poll::Ready(Ok(info)) => info,
            std::task::Poll::Ready(Err(error)) => {
                return err(&format!("host module info failed: {error:#}"));
            }
            std::task::Poll::Pending => {
                return err("host module info remained pending after both replies");
            }
        };
        if info.get("name").and_then(Value::as_str) != Some("storage_module")
            || info
                .get("methods")
                .and_then(Value::as_array)
                .is_none_or(|methods| methods.len() != 1)
            || info
                .get("events")
                .and_then(Value::as_array)
                .is_none_or(|events| events.len() != 1)
        {
            return err("host module info did not merge named method and event metadata");
        }
        drop(future);
        state.close();
        Ok(())
    }

    #[test]
    fn dropping_module_future_without_cancel_callback_is_safe() -> TestResult {
        let host = TestHost::new();
        let mut vtable = host.vtable();
        vtable.cancel = None;
        // SAFETY: the local vtable provides a complete readable v1 prefix.
        let copied = unsafe { HostVtable::copy_from(&vtable) }.map_err(std::io::Error::other)?;
        let state = HostState::new(copied);
        let call = ModuleCall::new(
            ModuleTransportKind::Module,
            "storage_module",
            "space",
            Vec::new(),
        )?;
        let mut future = state.dispatch(call);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        if std::future::Future::poll(future.as_mut(), &mut context).is_ready() {
            return err("host module future completed before its reply");
        }
        let request = host.wait_for_request()?;
        drop(future);
        if !host.cancelled().is_empty() {
            return err("missing cancel callback produced host cancellation");
        }
        host.complete(request.id, 1, "12", false)?;
        state.close();
        Ok(())
    }

    #[test]
    fn host_enabled_handle_runs_local_inspector_call_without_host_dispatch() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        let method = CString::new("sourcePolicy")?;
        let args = CString::new("[]")?;

        // SAFETY: handle and C strings remain live for this call.
        let response =
            unsafe { logos_inspector_core_call(handle.as_ptr(), method.as_ptr(), args.as_ptr()) };
        let value = response_value(response)?;
        if value.get("ok").and_then(Value::as_bool) != Some(true)
            || !value.get("value").is_some_and(Value::is_object)
        {
            return Err(std::io::Error::other(format!(
                "host-local inspector call failed: {value}"
            ))
            .into());
        }
        if !lock(&host.registry).requests.is_empty() {
            return err("host-local inspector call reached host dispatch");
        }
        Ok(())
    }

    #[test]
    fn host_enabled_handle_rejects_async_required_inspector_calls_before_dispatch() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;

        for (method, args) in [
            ("rawRpc", "[]"),
            ("runtimeOperationStatus", "[]"),
            ("blockchainNode", "[\"module\"]"),
            (
                "storageDownloadBackupCatalogEntry",
                r#"[{"adapter":{"source_mode":"module","inputs":{}},"mutating_enabled":false,"payload":{"cid":"cid-backup","local_only":false}}]"#,
            ),
            (
                "storageRestoreSettings",
                r#"[{"adapter":{"source_mode":"module","inputs":{}},"mutating_enabled":false,"payload":{"cid":"cid-legacy","local_only":false}}]"#,
            ),
            ("callModule", "[]"),
            ("modules", "[]"),
        ] {
            let method = CString::new(method)?;
            let args = CString::new(args)?;
            // SAFETY: handle and C strings remain live for this call.
            let response = unsafe {
                logos_inspector_core_call(handle.as_ptr(), method.as_ptr(), args.as_ptr())
            };
            let value = response_value(response)?;
            if value.get("ok").and_then(Value::as_bool) != Some(false)
                || value.get("error").and_then(Value::as_str) != Some(ASYNC_REQUIRED_ERROR)
            {
                return Err(std::io::Error::other(format!(
                    "async-required inspector method returned wrong response: {value}"
                ))
                .into());
            }
        }
        if !lock(&host.registry).requests.is_empty() {
            return err("async-required inspector call reached host dispatch");
        }
        Ok(())
    }

    #[test]
    fn host_enabled_handle_rejects_synchronous_module_calls_and_closes_once() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        let module = CString::new("storage_module")?;
        let method = CString::new("space")?;
        let args = CString::new("[]")?;

        // SAFETY: handle and C strings remain live for this call.
        let response = unsafe {
            logos_inspector_core_call_module(
                handle.as_ptr(),
                module.as_ptr(),
                method.as_ptr(),
                args.as_ptr(),
            )
        };
        let value = response_value(response)?;
        if value.get("ok").and_then(Value::as_bool) != Some(false) {
            return err("host-enabled synchronous call did not fail");
        }
        if value.get("error").and_then(Value::as_str) != Some(ASYNC_REQUIRED_ERROR) {
            return err("host-enabled synchronous call returned wrong error");
        }
        if !lock(&host.registry).requests.is_empty() {
            return err("synchronous module call reached host dispatch");
        }

        handle.close();
        handle.close();
        drop(handle);
        if host.close_count() != 1 {
            return err("host close callback did not run exactly once");
        }
        Ok(())
    }

    #[test]
    fn async_host_rejects_direct_backup_download_aliases_before_dispatch() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));
        let args = r#"[{"adapter":{"source_mode":"module","inputs":{}},"mutating_enabled":false,"payload":{"cid":"cid-backup","local_only":false}}]"#;

        for (bridge_request_id, method) in [
            (31_u64, "storageDownloadBackupCatalogEntry"),
            (32_u64, "storageRestoreSettings"),
        ] {
            let context = reply_context(&collector, &drops);
            if enqueue_test_call(
                handle.as_ptr(),
                bridge_request_id,
                "logos_inspector",
                method,
                args,
                context,
            )? != 1
            {
                // SAFETY: rejected ingress leaves its callback context caller-owned.
                unsafe {
                    drop(Box::from_raw(context.cast::<ReplyContext>()));
                }
                return err("backup download direct-call ingress was rejected");
            }
        }

        let replies = collector.wait_for_replies(2)?;
        let expected_error = "host-backed operation `storageDownloadBackupCatalogEntry` requires `runtimeOperationStart`";
        for bridge_request_id in [31_u64, 32_u64] {
            let response = replies
                .iter()
                .find(|(reply_id, _)| *reply_id == bridge_request_id)
                .ok_or_else(|| std::io::Error::other("backup download reply was missing"))?;
            let value: Value = serde_json::from_str(&response.1)?;
            if value.get("ok").and_then(Value::as_bool) != Some(false)
                || value.get("error").and_then(Value::as_str) != Some(expected_error)
            {
                return Err(std::io::Error::other(format!(
                    "backup download direct call returned wrong response: {value}"
                ))
                .into());
            }
            handle.wait_for_bridge_id_available(bridge_request_id)?;
        }
        if collector.count() != 2 || drops.load(Ordering::Acquire) != 2 {
            return err("backup download direct calls did not complete exactly once");
        }
        if !lock(&host.registry).requests.is_empty() || !host.cancelled().is_empty() {
            return err("backup download direct call reached host transport");
        }

        handle.close();
        handle.close();
        drop(handle);
        if host.close_count() != 1 {
            return err("backup download rejection did not close host exactly once");
        }
        Ok(())
    }

    #[test]
    fn async_host_copies_inputs_and_distinguishes_null_from_error() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));

        if enqueue_test_call(
            handle.as_ptr(),
            41,
            "storage_module",
            "readValue",
            "[\"copied\"]",
            reply_context(&collector, &drops),
        )? != 1
        {
            return err("asynchronous ingress rejected valid call");
        }
        let first = host.wait_for_request()?;
        if first.module != "storage_module"
            || first.method != "readValue"
            || first.args_json != "[\"copied\"]"
        {
            return err("host did not receive copied call inputs");
        }
        host.complete(first.id, 1, "null", true)?;
        let replies = collector.wait_for_replies(1)?;
        let null_response: Value = serde_json::from_str(&replies[0].1)?;
        if replies[0].0 != 41
            || null_response.get("ok").and_then(Value::as_bool) != Some(true)
            || !null_response.get("value").is_some_and(Value::is_null)
        {
            return err("valid JSON null was not preserved as success");
        }

        if enqueue_test_call(
            handle.as_ptr(),
            42,
            "storage_module",
            "readValue",
            "[]",
            reply_context(&collector, &drops),
        )? != 1
        {
            return err("second asynchronous ingress was rejected");
        }
        let second = host.wait_for_request()?;
        host.complete(second.id, 0, "{\"error\":\"timeout\"}", false)?;
        let replies = collector.wait_for_replies(2)?;
        let error_response: Value = serde_json::from_str(&replies[1].1)?;
        if replies[1].0 != 42
            || error_response.get("ok").and_then(Value::as_bool) != Some(false)
            || error_response.get("error").and_then(Value::as_str) != Some("timeout")
        {
            return err("host error was not distinct from valid JSON null");
        }
        if drops.load(Ordering::Acquire) != 2 {
            return err("core reply contexts were not released exactly once");
        }
        Ok(())
    }

    #[test]
    fn asynchronous_handles_isolate_same_bridge_id_and_host_close() -> TestResult {
        let first_host = TestHost::new();
        let second_host = TestHost::new();
        let first_handle = TestCoreHandle::new(&first_host)?;
        let second_handle = TestCoreHandle::new(&second_host)?;
        let first_collector = Arc::new(ReplyCollector::default());
        let second_collector = Arc::new(ReplyCollector::default());
        let first_drops = Arc::new(AtomicUsize::new(0));
        let second_drops = Arc::new(AtomicUsize::new(0));

        if enqueue_test_call(
            first_handle.as_ptr(),
            50,
            "storage_module",
            "space",
            "[]",
            reply_context(&first_collector, &first_drops),
        )? != 1
            || enqueue_test_call(
                second_handle.as_ptr(),
                50,
                "storage_module",
                "space",
                "[]",
                reply_context(&second_collector, &second_drops),
            )? != 1
        {
            return err("same bridge id was not isolated across handles");
        }
        let _first_request = first_host.wait_for_request()?;
        let second_request = second_host.wait_for_request()?;
        first_handle.close();
        second_host.complete(second_request.id, 1, "8", false)?;

        let first_replies = first_collector.wait_for_replies(1)?;
        let second_replies = second_collector.wait_for_replies(1)?;
        let first_response: Value = serde_json::from_str(&first_replies[0].1)?;
        let second_response: Value = serde_json::from_str(&second_replies[0].1)?;
        if first_response.get("error").and_then(Value::as_str) != Some(HOST_CLOSED_ERROR)
            || second_response.pointer("/value").and_then(Value::as_u64) != Some(8)
            || second_host.close_count() != 0
        {
            return err("closing one asynchronous handle affected its sibling");
        }
        if first_drops.load(Ordering::Acquire) != 1 || second_drops.load(Ordering::Acquire) != 1 {
            return err("independent handle callback contexts were not released once");
        }
        Ok(())
    }

    #[test]
    fn malformed_or_null_host_success_payloads_fail_explicitly() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));

        if enqueue_test_call(
            handle.as_ptr(),
            51,
            "storage_module",
            "readValue",
            "[]",
            reply_context(&collector, &drops),
        )? != 1
        {
            return err("malformed-success call was rejected");
        }
        let malformed_request = host.wait_for_request()?;
        host.complete(malformed_request.id, 1, "{bad", false)?;
        collector.wait_for_replies(1)?;

        if enqueue_test_call(
            handle.as_ptr(),
            52,
            "storage_module",
            "readValue",
            "[]",
            reply_context(&collector, &drops),
        )? != 1
        {
            return err("null-success call was rejected");
        }
        let null_request = host.wait_for_request()?;
        // SAFETY: this deliberately invalid host reply borrows the stable core
        // context and exercises null payload validation.
        unsafe {
            (null_request.reply)(
                ptr::with_exposed_provenance_mut(null_request.reply_context),
                null_request.id,
                1,
                ptr::null(),
            );
        }
        let replies = collector.wait_for_replies(2)?;
        let malformed: Value = serde_json::from_str(&replies[0].1)?;
        let null: Value = serde_json::from_str(&replies[1].1)?;
        if malformed
            .get("error")
            .and_then(Value::as_str)
            .is_none_or(|error| !error.contains("invalid success JSON"))
            || null
                .get("error")
                .and_then(Value::as_str)
                .is_none_or(|error| !error.contains("host response JSON is required"))
            || drops.load(Ordering::Acquire) != 2
        {
            return err("invalid host success payload was not explicit");
        }
        Ok(())
    }

    #[test]
    fn inline_host_reply_completes_once() -> TestResult {
        let host = TestHost::new();
        host.reply_inline(1, "{\"inline\":true}");
        let handle = TestCoreHandle::new(&host)?;
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));

        if enqueue_test_call(
            handle.as_ptr(),
            43,
            "storage_module",
            "readValue",
            "[]",
            reply_context(&collector, &drops),
        )? != 1
        {
            return err("inline host call was rejected");
        }
        let replies = collector.wait_for_replies(1)?;
        let response: Value = serde_json::from_str(&replies[0].1)?;
        if response.get("value") != Some(&serde_json::json!({ "inline": true })) {
            return err("inline host reply was not preserved");
        }
        if collector.count() != 1 || drops.load(Ordering::Acquire) != 1 {
            return err("inline host reply was not terminal exactly once");
        }
        Ok(())
    }

    #[test]
    fn detached_runtime_operation_dispatches_without_ingress_ownership() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));
        let args = serde_json::json!([{
            "domain": "delivery",
            "method": "deliverySend",
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "topic": "/topic", "payload": "hello" }
        }])
        .to_string();

        if enqueue_test_call(
            handle.as_ptr(),
            45,
            "logos_inspector",
            "runtimeOperationStart",
            &args,
            reply_context(&collector, &drops),
        )? != 1
        {
            return err("runtime operation start ingress was rejected");
        }
        let replies = collector.wait_for_replies(1)?;
        let start_response: Value = serde_json::from_str(&replies[0].1)?;
        let Some(operation_id) = start_response
            .pointer("/value/operationId")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
        else {
            return err("runtime operation start did not return operation identity");
        };
        if start_response.get("ok").and_then(Value::as_bool) != Some(true) {
            return err("runtime operation start returned an error");
        }

        let request = host.wait_for_request()?;
        if request.module != "delivery_module"
            || request.method != "send"
            || request.args_json != "[\"/topic\",\"hello\"]"
        {
            return err("detached runtime operation crossed wrong host boundary");
        }
        host.complete(request.id, 1, "\"request-45\"", true)?;

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut status_request_id = 4_500_u64;
        loop {
            let status_collector = Arc::new(ReplyCollector::default());
            let status_drops = Arc::new(AtomicUsize::new(0));
            let status_context = reply_context(&status_collector, &status_drops);
            let status_args = serde_json::json!([operation_id.as_str()]).to_string();
            if enqueue_test_call(
                handle.as_ptr(),
                status_request_id,
                "logos_inspector",
                "runtimeOperationStatus",
                &status_args,
                status_context,
            )? != 1
            {
                // SAFETY: return 0 leaves this callback context caller-owned.
                unsafe {
                    drop(Box::from_raw(status_context.cast::<ReplyContext>()));
                }
                return err("runtime operation status ingress was rejected");
            }
            let status_replies = status_collector.wait_for_replies(1)?;
            let status: Value = serde_json::from_str(&status_replies[0].1)?;
            if status.pointer("/value/status").and_then(Value::as_str) == Some("awaiting_external")
            {
                if status_drops.load(Ordering::Acquire) != 1 {
                    return err("runtime status callback context was not released once");
                }
                break;
            }
            if Instant::now() >= deadline {
                return Err(std::io::Error::other(format!(
                    "detached runtime operation did not settle after host reply: {status}"
                ))
                .into());
            }
            status_request_id = status_request_id
                .checked_add(1)
                .ok_or_else(|| std::io::Error::other("status request id space exhausted"))?;
            thread::yield_now();
        }
        handle.close();
        if drops.load(Ordering::Acquire) != 1 {
            return err("runtime operation start callback context was not released once");
        }
        Ok(())
    }

    #[test]
    fn module_event_ingress_validates_inputs_and_lifecycle() -> TestResult {
        let module = CString::new("delivery_module")?;
        let event = CString::new("messageSent")?;
        let args = CString::new("[\"request-1\"]")?;

        // SAFETY: null is an accepted rejected-handle path.
        if unsafe {
            logos_inspector_core_ingest_module_event(
                ptr::null_mut(),
                module.as_ptr(),
                event.as_ptr(),
                args.as_ptr(),
            )
        } != EVENT_REJECTED
        {
            return err("null handle accepted a module event");
        }

        let synchronous = logos_inspector_core_new();
        if synchronous.is_null() {
            return err("failed to create synchronous core handle");
        }
        if ingest_test_module_event(
            synchronous,
            "delivery_module",
            "messageSent",
            "[\"request-1\"]",
        )? != EVENT_REJECTED
        {
            // SAFETY: this test owns the synchronous handle.
            unsafe {
                logos_inspector_core_free(synchronous);
            }
            return err("synchronous handle accepted asynchronous module event ingress");
        }
        // SAFETY: this test owns the synchronous handle.
        unsafe {
            logos_inspector_core_free(synchronous);
        }

        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        if ingest_test_module_event(handle.as_ptr(), "", "messageSent", "[]")? != EVENT_REJECTED
            || ingest_test_module_event(handle.as_ptr(), "delivery_module", "", "[]")?
                != EVENT_REJECTED
            || ingest_test_module_event(handle.as_ptr(), "delivery_module", "messageSent", "{bad")?
                != EVENT_REJECTED
            || ingest_test_module_event(handle.as_ptr(), "delivery_module", "messageSent", "null")?
                != EVENT_REJECTED
        {
            return err("invalid module event input was accepted");
        }
        if ingest_test_module_event(
            handle.as_ptr(),
            "delivery_module",
            "messageSent",
            "[\"unknown\"]",
        )? != EVENT_ACCEPTED
        {
            return err("valid module event input was rejected");
        }
        handle.close();
        if ingest_test_module_event(
            handle.as_ptr(),
            "delivery_module",
            "messageSent",
            "[\"late\"]",
        )? != EVENT_REJECTED
        {
            return err("closed handle accepted a module event");
        }
        Ok(())
    }

    #[test]
    fn module_event_ingress_rejects_when_bounded_worker_queue_is_full() -> TestResult {
        let host = TestHost::new();
        host.block_dispatch();
        let handle = TestCoreHandle::new(&host)?;
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));
        if enqueue_test_call(
            handle.as_ptr(),
            47,
            "storage_module",
            "space",
            "[]",
            reply_context(&collector, &drops),
        )? != 1
        {
            return err("gated host call was rejected");
        }
        host.wait_for_dispatch_entry()?;
        let request = host.wait_for_request()?;

        for index in 0..ASYNC_WORKER_QUEUE_CAPACITY {
            let args = serde_json::json!([format!("request-{index}")]).to_string();
            if ingest_test_module_event(handle.as_ptr(), "delivery_module", "messageSent", &args)?
                != EVENT_ACCEPTED
            {
                host.release_dispatch();
                return err("bounded module event queue rejected before capacity");
            }
        }
        if ingest_test_module_event(
            handle.as_ptr(),
            "delivery_module",
            "messageSent",
            "[\"overflow\"]",
        )? != EVENT_BACKPRESSURE
        {
            host.release_dispatch();
            return err("bounded module event queue accepted overflow");
        }

        host.release_dispatch();
        host.complete(request.id, 1, "1", false)?;
        collector.wait_for_replies(1)?;
        if drops.load(Ordering::Acquire) != 1 {
            return err("gated host call callback context was not released once");
        }
        Ok(())
    }

    #[test]
    fn early_foreign_thread_module_event_completes_detached_operation_once() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));
        let args = serde_json::json!([{
            "domain": "delivery",
            "method": "deliverySend",
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "topic": "/topic", "payload": "hello" }
        }])
        .to_string();
        if enqueue_test_call(
            handle.as_ptr(),
            46,
            "logos_inspector",
            "runtimeOperationStart",
            &args,
            reply_context(&collector, &drops),
        )? != 1
        {
            return err("runtime operation start ingress was rejected");
        }
        let replies = collector.wait_for_replies(1)?;
        let start_response: Value = serde_json::from_str(&replies[0].1)?;
        let Some(operation_id) = start_response
            .pointer("/value/operationId")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
        else {
            return err("runtime operation start did not return operation identity");
        };
        let request = host.wait_for_request()?;
        if request.module != "delivery_module" || request.method != "send" {
            return err("runtime operation did not reach delivery host adapter");
        }

        let handle_address = handle.as_ptr().expose_provenance();
        let event_result = thread::spawn(move || {
            ingest_test_module_event(
                ptr::with_exposed_provenance_mut(handle_address),
                "delivery_module",
                "messageSent",
                "[\"request-46\",\"hash-original\"]",
            )
            .map_err(|error| error.to_string())
        })
        .join()
        .map_err(|_| std::io::Error::other("module event ingress thread panicked"))??;
        if event_result != EVENT_ACCEPTED {
            return err("foreign-thread module event was rejected");
        }

        let pending = operation_status(handle.as_ptr(), 4_600, &operation_id)?;
        if pending.pointer("/value/status").and_then(Value::as_str) != Some("running") {
            return Err(std::io::Error::other(format!(
                "early module event did not remain pending before host reply: {pending}"
            ))
            .into());
        }
        host.complete(request.id, 1, "\"request-46\"", true)?;

        let completed =
            wait_for_operation_status(handle.as_ptr(), &operation_id, "completed", 4_601)?;
        if completed.pointer("/value/result/0").and_then(Value::as_str) != Some("request-46")
            || completed.pointer("/value/result/1").and_then(Value::as_str) != Some("hash-original")
        {
            return Err(std::io::Error::other(format!(
                "early module event result was not replayed: {completed}"
            ))
            .into());
        }

        if ingest_test_module_event(
            handle.as_ptr(),
            "delivery_module",
            "messageSent",
            "[\"request-46\",\"hash-replacement\"]",
        )? != EVENT_ACCEPTED
        {
            return err("duplicate module event ingress was rejected at queue boundary");
        }
        let after_duplicate = operation_status(handle.as_ptr(), 4_700, &operation_id)?;
        if after_duplicate
            .pointer("/value/result/1")
            .and_then(Value::as_str)
            != Some("hash-original")
        {
            return err("duplicate module event replaced terminal operation result");
        }
        if drops.load(Ordering::Acquire) != 1 {
            return err("runtime operation start callback context was not released once");
        }
        Ok(())
    }

    #[test]
    fn duplicate_and_post_close_host_replies_are_safe_noops() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));

        if enqueue_test_call(
            handle.as_ptr(),
            44,
            "storage_module",
            "readValue",
            "[]",
            reply_context(&collector, &drops),
        )? != 1
        {
            return err("duplicate-reply test call was rejected");
        }
        let request = host.wait_for_request()?;
        let payload = CString::new("7")?;
        // SAFETY: the core callback context is borrowed and remains live until
        // this handle is freed. The second call deliberately violates the
        // host's exactly-once contract to verify registry hardening.
        unsafe {
            (request.reply)(
                ptr::with_exposed_provenance_mut(request.reply_context),
                request.id,
                1,
                payload.as_ptr(),
            );
            (request.reply)(
                ptr::with_exposed_provenance_mut(request.reply_context),
                request.id,
                1,
                payload.as_ptr(),
            );
        }
        collector.wait_for_replies(1)?;
        handle.close();
        // SAFETY: explicit close retains the core allocation until Drop. This
        // deliberately late reply must be ignored by the closed registry.
        unsafe {
            (request.reply)(
                ptr::with_exposed_provenance_mut(request.reply_context),
                request.id,
                1,
                payload.as_ptr(),
            );
        }
        if collector.count() != 1 || drops.load(Ordering::Acquire) != 1 {
            return err("duplicate or post-close host reply escaped take-once state");
        }
        Ok(())
    }

    #[test]
    fn rejected_host_dispatch_still_completes_accepted_ingress_once() -> TestResult {
        let host = TestHost::new();
        host.reject_dispatch();
        let handle = TestCoreHandle::new(&host)?;
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));

        if enqueue_test_call(
            handle.as_ptr(),
            7,
            "storage_module",
            "space",
            "[]",
            reply_context(&collector, &drops),
        )? != 1
        {
            return err("core did not accept call before host dispatch");
        }
        let replies = collector.wait_for_replies(1)?;
        let response: Value = serde_json::from_str(&replies[0].1)?;
        if response.get("ok").and_then(Value::as_bool) != Some(false) {
            return err("host dispatch rejection did not become bridge error");
        }
        if response
            .get("error")
            .and_then(Value::as_str)
            .is_none_or(|error| !error.contains("host rejected module request"))
        {
            return err("host rejection error lost request evidence");
        }
        if drops.load(Ordering::Acquire) != 1 || collector.count() != 1 {
            return err("host rejection did not complete callback exactly once");
        }
        Ok(())
    }

    #[test]
    fn duplicate_bridge_id_is_rejected_without_stealing_context() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        let first_collector = Arc::new(ReplyCollector::default());
        let first_drops = Arc::new(AtomicUsize::new(0));
        let second_collector = Arc::new(ReplyCollector::default());
        let second_drops = Arc::new(AtomicUsize::new(0));
        let reused_collector = Arc::new(ReplyCollector::default());
        let reused_drops = Arc::new(AtomicUsize::new(0));

        let first_context = reply_context(&first_collector, &first_drops);
        if enqueue_test_call(
            handle.as_ptr(),
            9,
            "storage_module",
            "space",
            "[]",
            first_context,
        )? != 1
        {
            return err("first bridge id was rejected");
        }
        let second_context = reply_context(&second_collector, &second_drops);
        if enqueue_test_call(
            handle.as_ptr(),
            9,
            "storage_module",
            "space",
            "[]",
            second_context,
        )? != 0
        {
            return err("duplicate bridge id was accepted");
        }
        // SAFETY: return 0 leaves this callback context caller-owned.
        unsafe {
            drop(Box::from_raw(second_context.cast::<ReplyContext>()));
        }

        let request = host.wait_for_request()?;
        host.complete(request.id, 1, "12", false)?;
        first_collector.wait_for_replies(1)?;
        handle.wait_for_bridge_id_available(9)?;
        if first_collector.count() != 1
            || second_collector.count() != 0
            || first_drops.load(Ordering::Acquire) != 1
            || second_drops.load(Ordering::Acquire) != 1
        {
            return err("duplicate bridge id violated callback ownership");
        }

        if enqueue_test_call(
            handle.as_ptr(),
            9,
            "storage_module",
            "space",
            "[]",
            reply_context(&reused_collector, &reused_drops),
        )? != 1
        {
            return err("terminal bridge id could not be reused");
        }
        let reused_request = host.wait_for_request()?;
        host.complete(reused_request.id, 1, "13", false)?;
        reused_collector.wait_for_replies(1)?;
        if reused_collector.count() != 1 || reused_drops.load(Ordering::Acquire) != 1 {
            return err("reused bridge id lost its independent callback ownership");
        }
        Ok(())
    }

    #[test]
    fn bridge_id_remains_reserved_until_terminal_callback_returns() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        let first_collector = Arc::new(ReplyCollector::default());
        let first_drops = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new(ReplyGate::default());

        if enqueue_test_call(
            handle.as_ptr(),
            10,
            "storage_module",
            "space",
            "[]",
            gated_reply_context(&first_collector, &first_drops, &gate),
        )? != 1
        {
            return err("gated bridge request was rejected");
        }
        let request = host.wait_for_request()?;
        host.complete(request.id, 1, "12", false)?;
        if let Err(error) = gate.wait_for_entry() {
            gate.release();
            return Err(error);
        }

        let early_collector = Arc::new(ReplyCollector::default());
        let early_drops = Arc::new(AtomicUsize::new(0));
        let early_context = reply_context(&early_collector, &early_drops);
        if enqueue_test_call(
            handle.as_ptr(),
            10,
            "storage_module",
            "space",
            "[]",
            early_context,
        )? != 0
        {
            gate.release();
            return err("bridge id was reused before its callback returned");
        }
        // SAFETY: return 0 leaves this callback context caller-owned.
        unsafe {
            drop(Box::from_raw(early_context.cast::<ReplyContext>()));
        }
        gate.release();
        first_collector.wait_for_replies(1)?;
        handle.wait_for_bridge_id_available(10)?;

        let reused_collector = Arc::new(ReplyCollector::default());
        let reused_drops = Arc::new(AtomicUsize::new(0));
        if enqueue_test_call(
            handle.as_ptr(),
            10,
            "storage_module",
            "space",
            "[]",
            reply_context(&reused_collector, &reused_drops),
        )? != 1
        {
            return err("bridge id remained reserved after callback return");
        }
        let reused_request = host.wait_for_request()?;
        host.complete(reused_request.id, 1, "13", false)?;
        reused_collector.wait_for_replies(1)?;
        if first_drops.load(Ordering::Acquire) != 1
            || early_drops.load(Ordering::Acquire) != 1
            || reused_drops.load(Ordering::Acquire) != 1
        {
            return err("bridge id reservation violated callback context ownership");
        }
        Ok(())
    }

    #[test]
    fn ingress_cancellation_is_local_and_late_host_completion_is_ignored() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));

        if enqueue_test_call(
            handle.as_ptr(),
            13,
            "storage_module",
            "fetch",
            "[\"cid\"]",
            reply_context(&collector, &drops),
        )? != 1
        {
            return err("cancellable call was rejected");
        }
        let request = host.wait_for_request()?;
        // SAFETY: handle remains live and owns this accepted bridge request.
        if unsafe { logos_inspector_core_cancel(handle.as_ptr(), 13) } != 1 {
            return err("accepted bridge request was not cancelled");
        }
        let replies = collector.wait_for_replies(1)?;
        let cancelled: Value = serde_json::from_str(&replies[0].1)?;
        if cancelled.get("error").and_then(Value::as_str) != Some(REQUEST_CANCELLED_ERROR) {
            return err("cancellation did not produce canonical terminal error");
        }
        if !host.cancelled().is_empty() {
            return err("ingress cancellation escaped into uncorrelated host work");
        }

        if enqueue_test_call(
            handle.as_ptr(),
            14,
            "storage_module",
            "space",
            "[]",
            reply_context(&collector, &drops),
        )? != 1
        {
            return err("request after cancellation was rejected");
        }
        host.complete(request.id, 1, "{\"late\":true}", true)?;
        if collector.count() != 1 {
            return err("late host completion defeated ingress cancellation");
        }
        let next = host.wait_for_request()?;
        host.complete(next.id, 1, "4", false)?;
        collector.wait_for_replies(2)?;
        handle.close();
        if collector.count() != 2 || drops.load(Ordering::Acquire) != 2 {
            return err("late host completion defeated cancellation");
        }
        Ok(())
    }

    #[test]
    fn cancellation_during_dispatch_stays_local_and_nonblocking() -> TestResult {
        let host = TestHost::new();
        host.block_dispatch();
        let handle = TestCoreHandle::new(&host)?;
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));

        if enqueue_test_call(
            handle.as_ptr(),
            15,
            "storage_module",
            "fetch",
            "[\"cid\"]",
            reply_context(&collector, &drops),
        )? != 1
        {
            return err("gated host call was rejected");
        }
        if let Err(error) = host.wait_for_dispatch_entry() {
            host.release_dispatch();
            return Err(error);
        }
        let request = match host.wait_for_request() {
            Ok(request) => request,
            Err(error) => {
                host.release_dispatch();
                return Err(error);
            }
        };

        // SAFETY: handle remains live and owns this accepted bridge request.
        if unsafe { logos_inspector_core_cancel(handle.as_ptr(), 15) } != 1 {
            host.release_dispatch();
            return err("dispatching request was not cancelled");
        }
        let replies = match collector.wait_for_replies(1) {
            Ok(replies) => replies,
            Err(error) => {
                host.release_dispatch();
                return Err(error);
            }
        };
        if !host.cancelled().is_empty() {
            host.release_dispatch();
            return err("host cancel reentered an active dispatch callback");
        }
        let response: Value = match serde_json::from_str(&replies[0].1) {
            Ok(response) => response,
            Err(error) => {
                host.release_dispatch();
                return Err(error.into());
            }
        };
        if response.get("error").and_then(Value::as_str) != Some(REQUEST_CANCELLED_ERROR) {
            host.release_dispatch();
            return err("gated dispatch did not terminalize locally");
        }

        host.release_dispatch();
        host.complete(request.id, 1, "{\"late\":true}", false)?;
        // SAFETY: the request already reached its cancellation terminal state.
        if unsafe { logos_inspector_core_cancel(handle.as_ptr(), 15) } != 0 {
            return err("second cancellation reclaimed terminal request");
        }
        handle.close();
        if !host.cancelled().is_empty()
            || collector.count() != 1
            || drops.load(Ordering::Acquire) != 1
        {
            return err("dispatch cancellation violated exactly-once callback ownership");
        }
        Ok(())
    }

    #[test]
    fn out_of_band_close_cancels_pending_runtime_operation_and_drains_bridge() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));
        let args = serde_json::json!([{
            "domain": "delivery",
            "method": "deliverySend",
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "topic": "/close", "payload": "pending" }
        }])
        .to_string();

        if enqueue_test_call(
            handle.as_ptr(),
            20,
            "logos_inspector",
            "runtimeOperationStart",
            &args,
            reply_context(&collector, &drops),
        )? != 1
        {
            return err("runtime operation start ingress was rejected");
        }
        let replies = collector.wait_for_replies(1)?;
        let start: Value = serde_json::from_str(&replies[0].1)?;
        if start.get("ok").and_then(Value::as_bool) != Some(true)
            || start
                .pointer("/value/operationId")
                .and_then(Value::as_str)
                .is_none()
        {
            return Err(std::io::Error::other(format!(
                "runtime operation did not start before close: {start}"
            ))
            .into());
        }
        let request = host.wait_for_request()?;
        if request.module != "delivery_module" || request.method != "send" {
            return err("pending runtime operation reached wrong host method");
        }

        handle.begin_bridge_close()?;
        if host.wait_for_cancellations(1)? != vec![request.id] {
            return err("out-of-band close did not cancel pending host call exactly once");
        }
        handle.close();
        handle.assert_shutdown_drained()?;

        if host.cancelled() != vec![request.id]
            || host.close_count() != 1
            || collector.count() != 1
            || drops.load(Ordering::Acquire) != 1
        {
            return err("runtime operation close violated host or callback ownership");
        }

        let payload = CString::new("\"late-request\"")?;
        // SAFETY: explicit close retains the HostState allocation. The saved
        // host callback must observe its already-drained request as a no-op.
        unsafe {
            (request.reply)(
                ptr::with_exposed_provenance_mut(request.reply_context),
                request.id,
                1,
                payload.as_ptr(),
            );
        }
        if collector.count() != 1 || drops.load(Ordering::Acquire) != 1 {
            return err("late host completion produced a callback after close");
        }

        let rejected_collector = Arc::new(ReplyCollector::default());
        let rejected_drops = Arc::new(AtomicUsize::new(0));
        let rejected_context = reply_context(&rejected_collector, &rejected_drops);
        if enqueue_test_call(
            handle.as_ptr(),
            21,
            "logos_inspector",
            "runtimeOperationStatus",
            "[\"closed\"]",
            rejected_context,
        )? != 0
        {
            return err("closed core accepted a new asynchronous call");
        }
        // SAFETY: rejected ingress leaves its callback context caller-owned.
        unsafe {
            drop(Box::from_raw(rejected_context.cast::<ReplyContext>()));
        }
        if rejected_collector.count() != 0 || rejected_drops.load(Ordering::Acquire) != 1 {
            return err("closed core invoked a rejected callback");
        }
        Ok(())
    }

    #[test]
    fn close_keeps_host_open_until_accepted_backup_download_is_canceled() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));
        let args = serde_json::json!([{
            "domain": "storage",
            "method": "storageDownloadBackupCatalogEntry",
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": false,
            "payload": { "cid": "cid-basecamp-close", "local_only": false }
        }])
        .to_string();

        if enqueue_test_call(
            handle.as_ptr(),
            26,
            "logos_inspector",
            "runtimeOperationStart",
            &args,
            reply_context(&collector, &drops),
        )? != 1
        {
            return err("backup runtime operation start ingress was rejected");
        }
        let replies = collector.wait_for_replies(1)?;
        let start: Value = serde_json::from_str(&replies[0].1)?;
        if start.get("ok").and_then(Value::as_bool) != Some(true)
            || start
                .pointer("/value/operationId")
                .and_then(Value::as_str)
                .is_none()
        {
            return Err(std::io::Error::other(format!(
                "backup runtime operation did not start: {start}"
            ))
            .into());
        }

        let methods = host.wait_for_request()?;
        if methods.module != "storage_module"
            || methods.method != "getPluginMethods"
            || methods.args_json != "[]"
        {
            return err("backup close regression requested wrong method metadata");
        }
        host.complete(
            methods.id,
            1,
            &serde_json::json!([
                {
                    "type": "method",
                    "isInvokable": true,
                    "name": "downloadProtocol",
                    "signature": "downloadProtocol()"
                },
                {
                    "type": "method",
                    "isInvokable": true,
                    "name": "downloadToUrlV2",
                    "signature": "downloadToUrlV2(QString,QString,bool,int,QString,int)"
                },
                {
                    "type": "method",
                    "isInvokable": true,
                    "name": "downloadCancelV2",
                    "signature": "downloadCancelV2(QString)"
                }
            ])
            .to_string(),
            false,
        )?;

        let events = host.wait_for_request()?;
        if events.module != "storage_module"
            || events.method != "getPluginEvents"
            || events.args_json != "[]"
        {
            return err("backup close regression requested wrong event metadata");
        }
        host.complete(
            events.id,
            1,
            &serde_json::json!([{
                "type": "event",
                "name": "storageDownloadDoneV2",
                "signature": "storageDownloadDoneV2(QString)"
            }])
            .to_string(),
            false,
        )?;

        let protocol = host.wait_for_request()?;
        if protocol.module != "storage_module"
            || protocol.method != "downloadProtocol"
            || protocol.args_json != "[]"
        {
            return err("backup close regression requested wrong protocol");
        }
        host.complete(
            protocol.id,
            1,
            &serde_json::json!({
                "protocol": "logos.storage.download",
                "version": 2,
                "moduleOperationIdOwner": "caller",
                "cancelTimeoutMs": 15_000,
                "maxDownloadBytes": 1_073_741_824_u64
            })
            .to_string(),
            false,
        )?;

        let download = host.wait_for_request()?;
        if download.module != "storage_module" || download.method != "downloadToUrlV2" {
            return err("backup close regression did not dispatch downloadToUrlV2");
        }
        let download_args: Vec<Value> = serde_json::from_str(&download.args_json)?;
        let cid = download_args
            .first()
            .and_then(Value::as_str)
            .ok_or_else(|| std::io::Error::other("backup download CID was missing"))?
            .to_owned();
        let staged_path = download_args
            .get(1)
            .and_then(Value::as_str)
            .map(std::path::PathBuf::from)
            .ok_or_else(|| std::io::Error::other("backup staging path was missing"))?;
        let operation_id = download_args
            .get(4)
            .and_then(Value::as_str)
            .ok_or_else(|| std::io::Error::other("backup module operation ID was missing"))?
            .to_owned();
        if cid != "cid-basecamp-close" || operation_id == cid || !staged_path.exists() {
            return err("backup download dispatch identity or staging drifted");
        }
        host.complete(
            download.id,
            1,
            &serde_json::json!({
                "protocol": "logos.storage.download",
                "version": 2,
                "accepted": true,
                "moduleOperationId": operation_id,
                "cid": cid
            })
            .to_string(),
            false,
        )?;

        let handle_address = handle.as_ptr().expose_provenance();
        let (closed_sender, closed_receiver) = mpsc::channel();
        let closer = thread::spawn(move || {
            // SAFETY: the owning test keeps the allocation live until close joins.
            unsafe {
                logos_inspector_core_close(ptr::with_exposed_provenance_mut(handle_address));
            }
            let _sent = closed_sender.send(()).is_ok();
        });
        let cancel = match host.wait_for_request() {
            Ok(cancel) => cancel,
            Err(error) => {
                let _closed = closed_receiver.recv_timeout(Duration::from_secs(20));
                let _joined = closer.join().is_ok();
                return Err(error);
            }
        };
        let expected_cancel_args = serde_json::to_string(&[operation_id.as_str()])?;
        let cancel_identity_matches = cancel.module == "storage_module"
            && cancel.method == "downloadCancelV2"
            && cancel.args_json == expected_cancel_args;
        if host.close_count() != 0 {
            host.complete(
                cancel.id,
                1,
                &serde_json::json!({
                    "protocol": "logos.storage.download",
                    "version": 2,
                    "moduleOperationId": operation_id,
                    "cid": cid,
                    "cancelStatus": "canceled"
                })
                .to_string(),
                false,
            )?;
            let _closed = closed_receiver.recv_timeout(Duration::from_secs(5));
            let _joined = closer.join().is_ok();
            return err("host closed before accepted backup cancellation settled");
        }
        host.complete(
            cancel.id,
            1,
            &serde_json::json!({
                "protocol": "logos.storage.download",
                "version": 2,
                "moduleOperationId": operation_id,
                "cid": cid,
                "cancelStatus": "canceled"
            })
            .to_string(),
            false,
        )?;

        closed_receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        if closer.join().is_err() {
            return err("backup close thread panicked");
        }
        if !cancel_identity_matches {
            return err("shutdown did not cancel the exact accepted backup operation");
        }
        handle.assert_shutdown_drained()?;
        if host.close_count() != 1
            || !host.cancelled().is_empty()
            || staged_path.exists()
            || collector.count() != 1
            || drops.load(Ordering::Acquire) != 1
        {
            return err("backup shutdown violated cancellation, staging, or ownership invariants");
        }
        Ok(())
    }

    #[test]
    fn terminal_event_queued_before_close_completes_runtime_operation() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        let start_collector = Arc::new(ReplyCollector::default());
        let start_drops = Arc::new(AtomicUsize::new(0));
        let args = serde_json::json!([{
            "domain": "delivery",
            "method": "deliverySend",
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "topic": "/close", "payload": "terminal-event" }
        }])
        .to_string();

        if enqueue_test_call(
            handle.as_ptr(),
            25,
            "logos_inspector",
            "runtimeOperationStart",
            &args,
            reply_context(&start_collector, &start_drops),
        )? != 1
        {
            return err("runtime operation start ingress was rejected");
        }
        let start_replies = start_collector.wait_for_replies(1)?;
        let start: Value = serde_json::from_str(&start_replies[0].1)?;
        let Some(operation_id) = start
            .pointer("/value/operationId")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
        else {
            return err("runtime operation start did not return operation identity");
        };
        let request = host.wait_for_request()?;
        host.complete(request.id, 1, "\"request-before-close\"", true)?;
        let awaiting =
            wait_for_operation_status(handle.as_ptr(), &operation_id, "awaiting_external", 2_500)?;
        if awaiting.pointer("/value/status").and_then(Value::as_str) != Some("awaiting_external") {
            return err("runtime operation did not await its terminal module event");
        }

        if ingest_test_module_event(
            handle.as_ptr(),
            "delivery_module",
            "messageSent",
            "[\"request-before-close\",\"hash-before-close\"]",
        )? != EVENT_ACCEPTED
        {
            return err("terminal module event was rejected before close");
        }
        let status_collector = Arc::new(ReplyCollector::default());
        let status_drops = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new(ReplyGate::default());
        let status_context = gated_reply_context(&status_collector, &status_drops, &gate);
        let status_args = serde_json::json!([operation_id]).to_string();
        if enqueue_test_call(
            handle.as_ptr(),
            2_501,
            "logos_inspector",
            "runtimeOperationStatus",
            &status_args,
            status_context,
        )? != 1
        {
            // SAFETY: rejected ingress leaves its callback context caller-owned.
            unsafe {
                drop(Box::from_raw(status_context.cast::<ReplyContext>()));
            }
            return err("terminal status observation was rejected");
        }
        if let Err(error) = gate.wait_for_entry() {
            gate.release();
            return Err(error);
        }

        let handle_address = handle.as_ptr().expose_provenance();
        let (closed_sender, closed_receiver) = mpsc::channel();
        let closer = thread::spawn(move || {
            // SAFETY: the owning test keeps the allocation live until this
            // close call and the selected reply callback have both joined.
            unsafe {
                logos_inspector_core_close(ptr::with_exposed_provenance_mut(handle_address));
            }
            let _sent = closed_sender.send(()).is_ok();
        });
        if let Err(error) = handle.wait_for_core_closing() {
            gate.release();
            let _joined = closer.join().is_ok();
            return Err(error);
        }
        gate.release();

        closed_receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        if closer.join().is_err() {
            return err("close thread panicked while draining terminal event");
        }
        let status_replies = status_collector.wait_for_replies(1)?;
        let completed: Value = serde_json::from_str(&status_replies[0].1)?;
        if completed.pointer("/value/status").and_then(Value::as_str) != Some("completed")
            || completed.pointer("/value/result/0").and_then(Value::as_str)
                != Some("request-before-close")
            || completed.pointer("/value/result/1").and_then(Value::as_str)
                != Some("hash-before-close")
        {
            return Err(std::io::Error::other(format!(
                "queued terminal event lost completion during close: {completed}"
            ))
            .into());
        }
        handle.assert_shutdown_drained()?;
        if host.close_count() != 1
            || !host.cancelled().is_empty()
            || start_collector.count() != 1
            || start_drops.load(Ordering::Acquire) != 1
            || status_collector.count() != 1
            || status_drops.load(Ordering::Acquire) != 1
        {
            return err("terminal-event close violated lifecycle ownership");
        }
        Ok(())
    }

    #[test]
    fn closing_event_ingress_rejects_after_shutdown_is_queued() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new(ReplyGate::default());

        if enqueue_test_call(
            handle.as_ptr(),
            2_502,
            "storage_module",
            "space",
            "[]",
            gated_reply_context(&collector, &drops, &gate),
        )? != 1
        {
            return err("close-race fixture call was rejected");
        }
        let request = host.wait_for_request()?;
        host.complete(request.id, 1, "12", false)?;
        if let Err(error) = gate.wait_for_entry() {
            gate.release();
            return Err(error);
        }

        let handle_address = handle.as_ptr().expose_provenance();
        let (closed_sender, closed_receiver) = mpsc::channel();
        let closer = thread::spawn(move || {
            // SAFETY: the owning test keeps the allocation live until close joins.
            unsafe {
                logos_inspector_core_close(ptr::with_exposed_provenance_mut(handle_address));
            }
            let _sent = closed_sender.send(()).is_ok();
        });

        let closing = handle
            .wait_for_core_closing()
            .and_then(|()| handle.wait_for_shutdown_queued());
        if let Err(error) = closing {
            gate.release();
            let _closed = closed_receiver.recv_timeout(Duration::from_secs(5));
            let _joined = closer.join().is_ok();
            return Err(error);
        }
        let ingress = ingest_test_module_event(
            handle.as_ptr(),
            "delivery_module",
            "messageSent",
            "[\"after-shutdown\"]",
        )?;
        gate.release();

        closed_receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        if closer.join().is_err() {
            return err("close-race closer thread panicked");
        }
        collector.wait_for_replies(1)?;
        handle.assert_shutdown_drained()?;
        if ingress != EVENT_REJECTED
            || collector.count() != 1
            || drops.load(Ordering::Acquire) != 1
            || host.close_count() != 1
        {
            return err(
                "closing ingress was accepted behind shutdown or lifecycle ownership drifted",
            );
        }
        Ok(())
    }

    #[test]
    fn close_drains_inflight_request_and_is_idempotent() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));

        if enqueue_test_call(
            handle.as_ptr(),
            21,
            "storage_module",
            "fetch",
            "[\"cid\"]",
            reply_context(&collector, &drops),
        )? != 1
        {
            return err("inflight close call was rejected");
        }
        let _request = host.wait_for_request()?;
        handle.close();
        handle.close();
        let replies = collector.wait_for_replies(1)?;
        let closed: Value = serde_json::from_str(&replies[0].1)?;
        if closed.get("error").and_then(Value::as_str) != Some(HOST_CLOSED_ERROR) {
            return err("close did not terminalize inflight request");
        }
        if drops.load(Ordering::Acquire) != 1 || host.close_count() != 1 {
            return err("close did not release request and host exactly once");
        }
        drop(handle);
        if host.close_count() != 1 {
            return err("free repeated host close");
        }
        Ok(())
    }

    #[test]
    fn close_racing_queued_local_call_terminalizes_without_ownership_loss() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));

        if enqueue_test_call(
            handle.as_ptr(),
            24,
            "storage_module",
            "fetch",
            "[\"cid\"]",
            reply_context(&collector, &drops),
        )? != 1
        {
            return err("inflight host call was rejected");
        }
        let _request = host.wait_for_request()?;

        let handle_address = handle.as_ptr().expose_provenance();
        let (local_sender, local_receiver) = mpsc::channel();
        let local_call = thread::spawn(move || {
            let result = call_test_inspector(
                ptr::with_exposed_provenance_mut(handle_address),
                "sourcePolicy",
                "[]",
            );
            let _sent = local_sender.send(result).is_ok();
        });
        if let Err(error) = handle.wait_for_queued_local_call() {
            handle.close();
            let _joined = local_call.join().is_ok();
            return Err(error);
        }

        let handle_address = handle.as_ptr().expose_provenance();
        let (close_sender, close_receiver) = mpsc::channel();
        let closer = thread::spawn(move || {
            // SAFETY: the owning test keeps the allocation live. The ABI
            // permits close to race a host-transport synchronous call.
            unsafe {
                logos_inspector_core_close(ptr::with_exposed_provenance_mut(handle_address));
            }
            let _sent = close_sender.send(()).is_ok();
        });

        let local_result = local_receiver.recv_timeout(Duration::from_secs(5));
        let close_result = close_receiver.recv_timeout(Duration::from_secs(5));
        let local_join = local_call.join();
        let close_join = closer.join();
        let local_value = local_result
            .map_err(|error| std::io::Error::other(error.to_string()))?
            .map_err(std::io::Error::other)?;
        close_result.map_err(|error| std::io::Error::other(error.to_string()))?;
        if local_join.is_err() || close_join.is_err() {
            return err("local call or close thread panicked");
        }
        if local_value.get("ok").and_then(Value::as_bool) != Some(false)
            || local_value.get("error").and_then(Value::as_str)
                != Some("inspector command surface is shutting down")
        {
            return Err(std::io::Error::other(format!(
                "queued local call did not terminalize during close: {local_value}"
            ))
            .into());
        }

        let replies = collector.wait_for_replies(1)?;
        let closed: Value = serde_json::from_str(&replies[0].1)?;
        if closed.get("error").and_then(Value::as_str) != Some(HOST_CLOSED_ERROR) {
            return err("close did not terminalize pending host-backed ingress");
        }
        if host.close_count() != 1
            || !host.cancelled().is_empty()
            || collector.count() != 1
            || drops.load(Ordering::Acquire) != 1
        {
            return err("close race violated host or callback ownership");
        }

        drop(handle);
        if host.close_count() != 1 {
            return err("free repeated host close after racing calls joined");
        }
        Ok(())
    }

    #[test]
    fn concurrent_close_callers_join_one_host_shutdown() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));

        if enqueue_test_call(
            handle.as_ptr(),
            23,
            "storage_module",
            "fetch",
            "[\"cid\"]",
            reply_context(&collector, &drops),
        )? != 1
        {
            return err("concurrent-close call was rejected");
        }
        let _request = host.wait_for_request()?;
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let handle_address = handle.as_ptr().expose_provenance();
        let closers = (0..2)
            .map(|_| {
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    let _leader = barrier.wait().is_leader();
                    // SAFETY: the owning test guard keeps the allocation live;
                    // concurrent close callers are part of the ABI contract.
                    unsafe {
                        logos_inspector_core_close(ptr::with_exposed_provenance_mut(
                            handle_address,
                        ));
                    }
                })
            })
            .collect::<Vec<_>>();
        let _leader = barrier.wait().is_leader();
        for closer in closers {
            if closer.join().is_err() {
                return err("concurrent close thread panicked");
            }
        }
        collector.wait_for_replies(1)?;
        if host.close_count() != 1 || collector.count() != 1 || drops.load(Ordering::Acquire) != 1 {
            return err("concurrent close callers repeated shutdown ownership");
        }
        Ok(())
    }

    #[test]
    fn close_waits_for_dispatch_callback_before_host_close() -> TestResult {
        let host = TestHost::new();
        host.block_dispatch();
        let handle = TestCoreHandle::new(&host)?;
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));

        if enqueue_test_call(
            handle.as_ptr(),
            22,
            "storage_module",
            "fetch",
            "[\"cid\"]",
            reply_context(&collector, &drops),
        )? != 1
        {
            return err("gated close call was rejected");
        }
        if let Err(error) = host.wait_for_dispatch_entry() {
            host.release_dispatch();
            return Err(error);
        }

        let handle_address = handle.as_ptr().expose_provenance();
        let (closed_sender, closed_receiver) = mpsc::channel();
        let closer = thread::spawn(move || {
            // SAFETY: the owning test guard keeps this handle allocation live
            // and close is allowed to race other non-free ABI activity.
            unsafe {
                logos_inspector_core_close(ptr::with_exposed_provenance_mut(handle_address));
            }
            let _sent = closed_sender.send(()).is_ok();
        });
        if let Err(error) = handle.wait_for_core_closing() {
            host.release_dispatch();
            let _joined = closer.join().is_ok();
            return Err(error);
        }
        if host.close_count() != 0 {
            host.release_dispatch();
            let _joined = closer.join().is_ok();
            return err("host close overlapped active dispatch callback");
        }

        host.release_dispatch();
        let close_result = closed_receiver.recv_timeout(Duration::from_secs(5));
        let join_result = closer.join();
        if let Err(error) = close_result {
            return Err(std::io::Error::other(error.to_string()).into());
        }
        if join_result.is_err() {
            return err("close thread panicked");
        }
        let replies = collector.wait_for_replies(1)?;
        let response: Value = serde_json::from_str(&replies[0].1)?;
        if response.get("error").and_then(Value::as_str) != Some(HOST_CLOSED_ERROR) {
            return err("gated close did not terminalize accepted ingress");
        }
        if host.close_count() != 1 || collector.count() != 1 || drops.load(Ordering::Acquire) != 1 {
            return err("gated close violated callback or host ownership");
        }
        Ok(())
    }

    #[test]
    fn core_reply_can_reenter_nonclosing_abi_without_deadlock() -> TestResult {
        let host = TestHost::new();
        let handle = TestCoreHandle::new(&host)?;
        let collector = Arc::new(ReplyCollector::default());
        let drops = Arc::new(AtomicUsize::new(0));
        let reentry_result = Arc::new(AtomicI32::new(-1));
        let context = reentrant_reply_context(&collector, &drops, handle.as_ptr(), &reentry_result);

        if enqueue_test_call(
            handle.as_ptr(),
            31,
            "storage_module",
            "space",
            "[]",
            context,
        )? != 1
        {
            return err("reentrant callback call was rejected");
        }
        let request = host.wait_for_request()?;
        host.complete(request.id, 1, "1", true)?;
        collector.wait_for_replies(1)?;
        if reentry_result.load(Ordering::Acquire) != 0 {
            return err("reply callback reentry did not return normally");
        }
        if drops.load(Ordering::Acquire) != 1 {
            return err("reentrant reply context was not released once");
        }
        Ok(())
    }

    fn expect_error_envelope_shape(value: &Value) -> TestResult {
        if !value.get("value").is_some_and(Value::is_null) {
            return err("expected null envelope value");
        }
        if value.get("text").and_then(Value::as_str) != Some("") {
            return err("expected empty envelope text");
        }
        Ok(())
    }

    fn response_value(ptr: *mut c_char) -> Result<Value, Box<dyn std::error::Error>> {
        let text = c_string_from_owned_ptr(ptr)?;
        Ok(serde_json::from_str(&text)?)
    }

    fn c_string_from_owned_ptr(ptr: *mut c_char) -> Result<String, Box<dyn std::error::Error>> {
        if ptr.is_null() {
            return err("FFI returned null string");
        }
        // SAFETY: pointer is returned by this library and remains valid until
        // the matching free below.
        let text = unsafe { CStr::from_ptr(ptr) }.to_str()?.to_owned();
        // SAFETY: pointer was returned by this library and is released once.
        unsafe {
            logos_inspector_core_string_free(ptr);
        }
        Ok(text)
    }

    fn err<T>(message: &str) -> Result<T, Box<dyn std::error::Error>> {
        Err(std::io::Error::other(message).into())
    }
}
