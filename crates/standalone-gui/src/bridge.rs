use std::{
    pin::Pin,
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use anyhow::Context as _;
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use logos_inspector::{
    bridge::InspectorBridge,
    local_nodes::{LocalNodeModuleSubscription, LocalNodeModuleWatcher},
    module_transport::ModuleTransportEvent,
};

static BRIDGE: OnceLock<InspectorBridge> = OnceLock::new();
static CALL_LIFECYCLE: OnceLock<Arc<CallLifecycle>> = OnceLock::new();
static MODULE_WATCHER: OnceLock<Mutex<Option<ModuleWatcherRuntime>>> = OnceLock::new();

const MODULE_WATCHER_DELIVERY_POLL_INTERVAL: Duration = Duration::from_millis(100);

struct ModuleWatcherRuntime {
    watcher: LocalNodeModuleWatcher,
    delivery_stop: Arc<AtomicBool>,
    delivery_worker: thread::JoinHandle<()>,
}

#[derive(Default)]
struct CallLifecycleState {
    closing: bool,
    active: usize,
}

struct CallLifecycle {
    state: Mutex<CallLifecycleState>,
    quiesced: Condvar,
}

impl Default for CallLifecycle {
    fn default() -> Self {
        Self {
            state: Mutex::new(CallLifecycleState::default()),
            quiesced: Condvar::new(),
        }
    }
}

impl CallLifecycle {
    fn is_closing(&self) -> anyhow::Result<bool> {
        self.state
            .lock()
            .map(|state| state.closing)
            .map_err(|_| anyhow::anyhow!("standalone bridge lifecycle is unavailable"))
    }

    fn begin_close(&self) -> anyhow::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("standalone bridge lifecycle is unavailable"))?;
        state.closing = true;
        Ok(())
    }

    fn wait_for_quiescence(&self) -> anyhow::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("standalone bridge lifecycle is unavailable"))?;
        while state.active > 0 {
            state = self
                .quiesced
                .wait(state)
                .map_err(|_| anyhow::anyhow!("standalone bridge lifecycle is unavailable"))?;
        }
        Ok(())
    }

    #[cfg(test)]
    fn active_calls(&self) -> anyhow::Result<usize> {
        self.state
            .lock()
            .map(|state| state.active)
            .map_err(|_| anyhow::anyhow!("standalone bridge lifecycle is unavailable"))
    }
}

struct ActiveCall {
    lifecycle: Arc<CallLifecycle>,
}

impl ActiveCall {
    fn delivery_allowed(&self) -> bool {
        self.lifecycle
            .state
            .lock()
            .is_ok_and(|state| !state.closing)
    }
}

impl Drop for ActiveCall {
    fn drop(&mut self) {
        if let Ok(mut state) = self.lifecycle.state.lock() {
            state.active = state.active.saturating_sub(1);
            self.lifecycle.quiesced.notify_all();
        }
    }
}

struct QueuedCallDelivery {
    active_call: ActiveCall,
}

impl QueuedCallDelivery {
    fn new(active_call: ActiveCall) -> Self {
        Self { active_call }
    }

    fn deliver(self, deliver: impl FnOnce()) {
        if self.active_call.delivery_allowed() {
            deliver();
        }
    }
}

#[derive(Default)]
pub struct LogosBridgeRust;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[namespace = "logos_bridge"]
        type LogosBridge = super::LogosBridgeRust;
    }

    extern "RustQt" {
        #[qinvokable]
        #[cxx_name = "callModuleJson"]
        fn call_module_json(
            self: &LogosBridge,
            module: &QString,
            method: &QString,
            args_json: &QString,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "callModuleJsonAsync"]
        fn call_module_json_async(
            self: Pin<&mut LogosBridge>,
            request_id: i32,
            module: &QString,
            method: &QString,
            args_json: &QString,
        );

        #[qinvokable]
        #[cxx_name = "startModuleWatcher"]
        fn start_module_watcher(self: Pin<&mut LogosBridge>) -> bool;

        #[qinvokable]
        #[cxx_name = "backendOwnsRuntimeModuleEvents"]
        fn backend_owns_runtime_module_events(self: &LogosBridge) -> bool;

        #[qsignal]
        #[cxx_name = "moduleCallFinished"]
        fn module_call_finished(
            self: Pin<&mut LogosBridge>,
            request_id: i32,
            response_json: &QString,
        );

        #[qsignal]
        #[cxx_name = "moduleEventJson"]
        fn module_event_json(
            self: Pin<&mut LogosBridge>,
            module: &QString,
            event: &QString,
            args_json: &QString,
        );
    }

    impl cxx_qt::Threading for LogosBridge {}
}

impl qobject::LogosBridge {
    pub fn call_module_json(
        &self,
        module: &QString,
        method: &QString,
        args_json: &QString,
    ) -> QString {
        QString::from(call_module_response_json(
            &module.to_string(),
            &method.to_string(),
            &args_json.to_string(),
        ))
    }

    pub fn call_module_json_async(
        self: Pin<&mut Self>,
        request_id: i32,
        module: &QString,
        method: &QString,
        args_json: &QString,
    ) {
        let qt_thread = self.qt_thread();
        let module = module.to_string();
        let method = method.to_string();
        let args_json = args_json.to_string();
        let active_call = match begin_async_call() {
            Ok(active_call) => active_call,
            Err(error) => {
                let response = QString::from(InspectorBridge::error_json(error.to_string()));
                self.module_call_finished(request_id, &response);
                return;
            }
        };
        let spawn = std::thread::Builder::new()
            .name("logos-inspector-standalone-call".to_owned())
            .spawn(move || {
                let response_json = call_module_response_json(&module, &method, &args_json);
                let delivery = QueuedCallDelivery::new(active_call);
                let _queue_result = qt_thread.queue(move |mut qobject| {
                    delivery.deliver(|| {
                        let response = QString::from(response_json);
                        qobject.as_mut().module_call_finished(request_id, &response);
                    });
                });
            });
        if let Err(error) = spawn {
            let response = QString::from(InspectorBridge::error_json(format!(
                "failed to start standalone bridge call: {error}"
            )));
            self.module_call_finished(request_id, &response);
        }
    }

    pub fn start_module_watcher(self: Pin<&mut Self>) -> bool {
        start_module_watcher_runtime(self.qt_thread()).is_ok()
    }

    pub fn backend_owns_runtime_module_events(&self) -> bool {
        module_watcher_is_running()
    }
}

fn start_module_watcher_runtime(qt_thread: qobject::LogosBridgeCxxQtThread) -> anyhow::Result<()> {
    if call_lifecycle().is_closing()? {
        anyhow::bail!("standalone bridge is shutting down");
    }
    bridge()?;

    let mut runtime = module_watcher_runtime()
        .lock()
        .map_err(|_| anyhow::anyhow!("standalone module watcher is unavailable"))?;
    if runtime.is_some() {
        return Ok(());
    }

    let watcher = LocalNodeModuleWatcher::start()?;
    let subscription = watcher.subscribe()?;
    let delivery_stop = Arc::new(AtomicBool::new(false));
    let worker_delivery_stop = Arc::clone(&delivery_stop);
    let delivery_worker = thread::Builder::new()
        .name("logos-inspector-module-event-delivery".to_owned())
        .spawn(move || deliver_module_watcher_events(subscription, qt_thread, worker_delivery_stop))
        .context("failed to start standalone module event delivery worker")?;
    *runtime = Some(ModuleWatcherRuntime {
        watcher,
        delivery_stop,
        delivery_worker,
    });
    Ok(())
}

fn module_watcher_runtime() -> &'static Mutex<Option<ModuleWatcherRuntime>> {
    MODULE_WATCHER.get_or_init(|| Mutex::new(None))
}

fn module_watcher_is_running() -> bool {
    module_watcher_runtime()
        .lock()
        .is_ok_and(|runtime| runtime.is_some())
}

fn deliver_module_watcher_events(
    mut subscription: LocalNodeModuleSubscription,
    qt_thread: qobject::LogosBridgeCxxQtThread,
    delivery_stop: Arc<AtomicBool>,
) {
    while !delivery_stop.load(Ordering::Acquire) {
        match subscription.next_within(MODULE_WATCHER_DELIVERY_POLL_INTERVAL) {
            Ok(Some(event)) => {
                if !module_watcher_delivery_allowed() {
                    break;
                }
                deliver_module_watcher_event(event, &qt_thread);
            }
            Ok(None) => {}
            Err(_) => break,
        }
    }
}

fn module_watcher_delivery_allowed() -> bool {
    call_lifecycle().is_closing().is_ok_and(|closing| !closing)
}

fn deliver_module_watcher_event(
    event: ModuleTransportEvent,
    qt_thread: &qobject::LogosBridgeCxxQtThread,
) {
    let module = event.module().to_owned();
    let event_name = event.event().to_owned();
    let args_json = serde_json::to_string(event.args()).ok();
    let args = event.args().to_vec();

    if let Ok(bridge) = bridge() {
        let _ingest_result = bridge.ingest_module_event(&module, &event_name, args);
    }

    let Some(args_json) = args_json else {
        return;
    };
    if !module_watcher_delivery_allowed() {
        return;
    }
    let _queue_result = qt_thread.queue(move |mut qobject| {
        if !module_watcher_delivery_allowed() {
            return;
        }
        let module = QString::from(module);
        let event = QString::from(event_name);
        let args_json = QString::from(args_json);
        qobject
            .as_mut()
            .module_event_json(&module, &event, &args_json);
    });
}

fn stop_module_watcher() -> anyhow::Result<()> {
    let runtime = module_watcher_runtime()
        .lock()
        .map_err(|_| anyhow::anyhow!("standalone module watcher is unavailable"))?
        .take();
    let Some(mut runtime) = runtime else {
        return Ok(());
    };

    runtime.delivery_stop.store(true, Ordering::Release);
    let watcher_result = runtime.watcher.stop();
    let delivery_result = runtime
        .delivery_worker
        .join()
        .map_err(|_| anyhow::anyhow!("standalone module event delivery worker panicked"));
    match (watcher_result, delivery_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(watcher_error), Err(delivery_error)) => Err(watcher_error.context(format!(
            "standalone module event delivery worker also failed: {delivery_error:#}"
        ))),
    }
}

fn call_module_response_json(module: &str, method: &str, args_json: &str) -> String {
    match bridge() {
        Ok(bridge) => bridge.call_module_json(module, method, args_json),
        Err(error) => InspectorBridge::error_json(format!("{error:#}")),
    }
}

fn bridge() -> anyhow::Result<&'static InspectorBridge> {
    if call_lifecycle().is_closing()? {
        anyhow::bail!("standalone bridge is shutting down");
    }
    if let Some(bridge) = BRIDGE.get() {
        return Ok(bridge);
    }

    let bridge = InspectorBridge::standalone()?;
    let _ = BRIDGE.set(bridge);
    BRIDGE
        .get()
        .ok_or_else(|| anyhow::anyhow!("failed to initialize logos_inspector bridge"))
}

fn call_lifecycle() -> &'static Arc<CallLifecycle> {
    CALL_LIFECYCLE.get_or_init(|| Arc::new(CallLifecycle::default()))
}

fn begin_async_call() -> anyhow::Result<ActiveCall> {
    begin_async_call_for(Arc::clone(call_lifecycle()))
}

fn begin_async_call_for(lifecycle: Arc<CallLifecycle>) -> anyhow::Result<ActiveCall> {
    let mut state = lifecycle
        .state
        .lock()
        .map_err(|_| anyhow::anyhow!("standalone bridge lifecycle is unavailable"))?;
    if state.closing {
        anyhow::bail!("standalone bridge is shutting down");
    }
    state.active = state
        .active
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("standalone bridge active-call count is exhausted"))?;
    drop(state);
    Ok(ActiveCall { lifecycle })
}

pub(crate) fn begin_close() -> anyhow::Result<()> {
    call_lifecycle().begin_close()?;
    let watcher_result = stop_module_watcher();
    let bridge_result = if let Some(bridge) = BRIDGE.get() {
        bridge.begin_close()
    } else {
        Ok(())
    };
    watcher_result?;
    bridge_result?;
    Ok(())
}

pub(crate) fn shutdown() -> anyhow::Result<()> {
    begin_close()?;
    call_lifecycle().wait_for_quiescence()?;
    if let Some(bridge) = BRIDGE.get() {
        bridge.shutdown()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
            mpsc,
        },
        thread,
        time::Duration,
    };

    use anyhow::{Result, bail};

    use super::*;

    #[test]
    fn queued_delivery_retains_active_call_until_execution() -> Result<()> {
        let lifecycle = Arc::new(CallLifecycle::default());
        let active_call = begin_async_call_for(Arc::clone(&lifecycle))?;
        let delivered = Arc::new(AtomicUsize::new(0));
        let delivered_by_closure = Arc::clone(&delivered);
        let delivery = QueuedCallDelivery::new(active_call);

        anyhow::ensure!(lifecycle.active_calls()? == 1);
        delivery.deliver(move || {
            delivered_by_closure.fetch_add(1, Ordering::SeqCst);
        });

        anyhow::ensure!(lifecycle.active_calls()? == 0);
        anyhow::ensure!(delivered.load(Ordering::SeqCst) == 1);
        Ok(())
    }

    #[test]
    fn dropped_queued_delivery_acknowledges_without_emitting() -> Result<()> {
        let lifecycle = Arc::new(CallLifecycle::default());
        let active_call = begin_async_call_for(Arc::clone(&lifecycle))?;
        let delivered = Arc::new(AtomicUsize::new(0));
        let delivery = QueuedCallDelivery::new(active_call);

        anyhow::ensure!(lifecycle.active_calls()? == 1);
        drop(delivery);

        anyhow::ensure!(lifecycle.active_calls()? == 0);
        anyhow::ensure!(delivered.load(Ordering::SeqCst) == 0);
        Ok(())
    }

    #[test]
    fn queued_delivery_after_close_is_acknowledged_without_emitting() -> Result<()> {
        let lifecycle = Arc::new(CallLifecycle::default());
        let active_call = begin_async_call_for(Arc::clone(&lifecycle))?;
        let delivered = Arc::new(AtomicUsize::new(0));
        let delivered_by_closure = Arc::clone(&delivered);
        let delivery = QueuedCallDelivery::new(active_call);

        lifecycle.begin_close()?;
        delivery.deliver(move || {
            delivered_by_closure.fetch_add(1, Ordering::SeqCst);
        });

        anyhow::ensure!(lifecycle.active_calls()? == 0);
        anyhow::ensure!(delivered.load(Ordering::SeqCst) == 0);
        Ok(())
    }

    #[test]
    fn closing_rejects_new_calls_and_waits_for_active_call() -> Result<()> {
        let lifecycle = Arc::new(CallLifecycle::default());
        let active_call = begin_async_call_for(Arc::clone(&lifecycle))?;
        let (finished, completion) = mpsc::sync_channel(1);
        let closing_lifecycle = Arc::clone(&lifecycle);
        let closer = thread::spawn(move || {
            let result = closing_lifecycle
                .begin_close()
                .and_then(|()| closing_lifecycle.wait_for_quiescence());
            let _sent = finished.send(result).is_ok();
        });

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            let closing = lifecycle
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("standalone lifecycle lock poisoned"))?
                .closing;
            if closing {
                break;
            }
            if std::time::Instant::now() >= deadline {
                bail!("standalone shutdown did not close admission");
            }
            thread::yield_now();
        }
        if begin_async_call_for(Arc::clone(&lifecycle)).is_ok() {
            bail!("standalone shutdown accepted a new call");
        }
        if completion.recv_timeout(Duration::from_millis(25)).is_ok() {
            bail!("standalone shutdown completed before active call exited");
        }

        drop(active_call);
        completion
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| anyhow::anyhow!("standalone shutdown did not observe quiescence"))??;
        closer
            .join()
            .map_err(|_| anyhow::anyhow!("standalone shutdown thread panicked"))?;
        Ok(())
    }
}
