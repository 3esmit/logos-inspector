use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use anyhow::{Context as _, Result};

const SIGNAL_MONITOR_READY_TIMEOUT: Duration = Duration::from_secs(2);

type ShutdownNotifier = Arc<dyn Fn() + Send + Sync>;

/// Lets the Qt bridge receive a shutdown notification without polling.
#[derive(Clone)]
pub(crate) struct ShutdownSignalSubscription {
    requested: Arc<AtomicBool>,
    notified: Arc<AtomicBool>,
    notifier: Arc<Mutex<Option<ShutdownNotifier>>>,
}

impl ShutdownSignalSubscription {
    fn new() -> Self {
        Self {
            requested: Arc::new(AtomicBool::new(false)),
            notified: Arc::new(AtomicBool::new(false)),
            notifier: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn register(&self, notifier: ShutdownNotifier) -> Result<()> {
        let mut registered = self
            .notifier
            .lock()
            .map_err(|_| anyhow::anyhow!("standalone shutdown notifier is unavailable"))?;
        *registered = Some(notifier);
        drop(registered);
        self.notify_if_requested()
    }

    fn request(&self) -> Result<()> {
        self.requested.store(true, Ordering::Release);
        self.notify_if_requested()
    }

    fn notify_if_requested(&self) -> Result<()> {
        if !self.requested.load(Ordering::Acquire) || self.notified.load(Ordering::Acquire) {
            return Ok(());
        }
        let notifier = self
            .notifier
            .lock()
            .map_err(|_| anyhow::anyhow!("standalone shutdown notifier is unavailable"))?
            .clone();
        let Some(notifier) = notifier else {
            return Ok(());
        };
        if !self.notified.swap(true, Ordering::AcqRel) {
            notifier();
        }
        Ok(())
    }
}

/// Keeps process-wide Unix signal handling alive until process exit.
///
/// Tokio intentionally retains installed signal handlers after a listener is
/// dropped. The worker therefore stays alive for the remaining process
/// lifetime instead of leaving SIGINT or SIGTERM without a receiver while the
/// GUI is still running.
pub(crate) struct ShutdownSignalMonitor {
    subscription: ShutdownSignalSubscription,
    _worker: thread::JoinHandle<()>,
}

impl ShutdownSignalMonitor {
    pub(crate) fn start() -> Result<Self> {
        let subscription = ShutdownSignalSubscription::new();
        let worker_subscription = subscription.clone();
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("logos-inspector-shutdown-signal".to_owned())
            .spawn(move || run_signal_monitor(worker_subscription, ready_sender))
            .context("failed to start standalone shutdown signal monitor")?;
        match ready_receiver.recv_timeout(SIGNAL_MONITOR_READY_TIMEOUT) {
            Ok(Ok(())) => Ok(Self {
                subscription,
                _worker: worker,
            }),
            Ok(Err(error)) => anyhow::bail!(error),
            Err(error) => anyhow::bail!(
                "standalone shutdown signal monitor did not initialize within {} ms: {error}",
                SIGNAL_MONITOR_READY_TIMEOUT.as_millis()
            ),
        }
    }

    pub(crate) fn subscription(&self) -> ShutdownSignalSubscription {
        self.subscription.clone()
    }
}

#[cfg(unix)]
fn run_signal_monitor(
    subscription: ShutdownSignalSubscription,
    ready: mpsc::SyncSender<Result<(), String>>,
) {
    use tokio::signal::unix::{SignalKind, signal};

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _send_result = ready.send(Err(format!(
                "failed to create standalone shutdown signal runtime: {error}"
            )));
            return;
        }
    };
    runtime.block_on(async move {
        let mut interrupt = match signal(SignalKind::interrupt()) {
            Ok(signal) => signal,
            Err(error) => {
                let _send_result =
                    ready.send(Err(format!("failed to subscribe to SIGINT: {error}")));
                return;
            }
        };
        let mut terminate = match signal(SignalKind::terminate()) {
            Ok(signal) => signal,
            Err(error) => {
                let _send_result =
                    ready.send(Err(format!("failed to subscribe to SIGTERM: {error}")));
                return;
            }
        };
        let _send_result = ready.send(Ok(()));
        loop {
            tokio::select! {
                signal = interrupt.recv() => {
                    if signal.is_none() {
                        return;
                    }
                    let _request_result = subscription.request();
                }
                signal = terminate.recv() => {
                    if signal.is_none() {
                        return;
                    }
                    let _request_result = subscription.request();
                }
            }
        }
    });
}

#[cfg(not(unix))]
fn run_signal_monitor(
    subscription: ShutdownSignalSubscription,
    ready: mpsc::SyncSender<Result<(), String>>,
) {
    let _send_result = ready.send(Ok(()));
    while !subscription.requested.load(Ordering::Acquire) {
        thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use super::*;

    #[test]
    fn request_notifies_registered_handler_once() -> Result<()> {
        let subscription = ShutdownSignalSubscription::new();
        let notifications = Arc::new(AtomicUsize::new(0));
        let callback_notifications = Arc::clone(&notifications);
        let notifier: ShutdownNotifier = Arc::new(move || {
            callback_notifications.fetch_add(1, Ordering::SeqCst);
        });
        subscription.register(notifier)?;

        subscription.request()?;
        subscription.request()?;

        anyhow::ensure!(
            notifications.load(Ordering::SeqCst) == 1,
            "shutdown request did not notify exactly once"
        );
        Ok(())
    }

    #[test]
    fn registration_after_request_receives_pending_shutdown() -> Result<()> {
        let subscription = ShutdownSignalSubscription::new();
        let notifications = Arc::new(AtomicUsize::new(0));

        subscription.request()?;
        let callback_notifications = Arc::clone(&notifications);
        let notifier: ShutdownNotifier = Arc::new(move || {
            callback_notifications.fetch_add(1, Ordering::SeqCst);
        });
        subscription.register(notifier)?;

        anyhow::ensure!(
            notifications.load(Ordering::SeqCst) == 1,
            "pending shutdown did not notify a late handler"
        );
        Ok(())
    }
}
