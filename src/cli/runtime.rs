use std::{sync::mpsc, thread, time::Duration};

use anyhow::{Context as _, Result, bail};
use serde_json::Value;
use tokio::sync::oneshot;

use crate::inspector::command_surface::{
    InspectorCommandSurface, InspectorCommandSurfaceCloseHandle,
};

const SIGNAL_MONITOR_START_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) struct CliCommandRuntime {
    surface: InspectorCommandSurface,
}

impl CliCommandRuntime {
    pub(super) fn new() -> Result<Self> {
        Ok(Self {
            surface: InspectorCommandSurface::new()
                .context("failed to create CLI command surface")?,
        })
    }

    pub(super) fn call(&self, method: &str, args: Value) -> Result<Value> {
        self.surface.call_inspector(method, args)
    }

    pub(super) fn call_signal_aware(&self, method: &str, args: Value) -> Result<CliCallReceipt> {
        let signal_monitor = CliSignalMonitor::start(self.surface.close_handle())?;
        let call = self.surface.call_inspector(method, args);
        match signal_monitor.finish() {
            Ok(CliSignalOutcome::Completed) => call.map(CliCallReceipt::completed),
            Ok(CliSignalOutcome::Interrupted(signal)) => {
                let shutdown = self.surface.shutdown();
                interrupted_call_result(signal, call, shutdown)
            }
            Err(monitor) => {
                let shutdown = self.surface.shutdown();
                failed_signal_monitor_result(call, monitor, shutdown)
            }
        }
    }
}

pub(super) struct CliCallReceipt {
    value: Value,
    post_result_error: Option<anyhow::Error>,
}

impl CliCallReceipt {
    fn completed(value: Value) -> Self {
        Self {
            value,
            post_result_error: None,
        }
    }

    fn completed_with_cleanup_error(value: Value, error: anyhow::Error) -> Self {
        Self {
            value,
            post_result_error: Some(error),
        }
    }

    pub(super) fn into_parts(self) -> (Value, Option<anyhow::Error>) {
        (self.value, self.post_result_error)
    }
}

fn interrupted_call_result(
    signal: &'static str,
    call: Result<Value>,
    shutdown: Result<()>,
) -> Result<CliCallReceipt> {
    let context = || format!("CLI backup download interrupted by {signal}");
    match (call, shutdown) {
        (Ok(value), Ok(())) => Ok(CliCallReceipt::completed(value)),
        (Err(operation), Ok(())) => Err(operation).with_context(context),
        (Ok(value), Err(cleanup)) => Ok(CliCallReceipt::completed_with_cleanup_error(
            value,
            cleanup.context(format!("{}; shutdown failed", context())),
        )),
        (Err(operation), Err(cleanup)) => Err(anyhow::anyhow!(
            "operation failed: {operation:#}; shutdown also failed: {cleanup:#}"
        ))
        .with_context(context),
    }
}

fn failed_signal_monitor_result(
    call: Result<Value>,
    monitor: anyhow::Error,
    shutdown: Result<()>,
) -> Result<CliCallReceipt> {
    match (call, shutdown) {
        (Ok(value), Ok(())) => Ok(CliCallReceipt::completed_with_cleanup_error(
            value,
            monitor.context("CLI signal monitor failed"),
        )),
        (Err(operation), Ok(())) => Err(anyhow::anyhow!(
            "signal monitor failed: {monitor:#}; operation also failed: {operation:#}"
        )),
        (Ok(value), Err(cleanup)) => Ok(CliCallReceipt::completed_with_cleanup_error(
            value,
            anyhow::anyhow!(
                "signal monitor failed: {monitor:#}; shutdown also failed: {cleanup:#}"
            ),
        )),
        (Err(operation), Err(cleanup)) => Err(anyhow::anyhow!(
            "signal monitor failed: {monitor:#}; operation also failed: {operation:#}; shutdown also failed: {cleanup:#}"
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CliSignalOutcome {
    Completed,
    Interrupted(&'static str),
}

struct CliSignalMonitor {
    completion: Option<oneshot::Sender<()>>,
    worker: Option<thread::JoinHandle<Result<CliSignalOutcome>>>,
}

impl CliSignalMonitor {
    fn start(close_handle: InspectorCommandSurfaceCloseHandle) -> Result<Self> {
        let (completion, completed) = oneshot::channel();
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("logos-inspector-cli-signal".to_owned())
            .spawn(move || run_signal_monitor(close_handle, completed, ready_sender))
            .context("failed to start CLI signal monitor")?;
        let mut monitor = Self {
            completion: Some(completion),
            worker: Some(worker),
        };
        match ready_receiver.recv_timeout(SIGNAL_MONITOR_START_TIMEOUT) {
            Ok(Ok(())) => Ok(monitor),
            Ok(Err(error)) => {
                monitor.stop();
                let _worker_result = monitor.join();
                bail!("failed to initialize CLI signal monitor: {error}")
            }
            Err(error) => {
                monitor.stop();
                let worker = monitor.join();
                match worker {
                    Ok(_) => Err(anyhow::anyhow!(error))
                        .context("CLI signal monitor did not report readiness"),
                    Err(worker_error) => Err(worker_error)
                        .context("CLI signal monitor failed before reporting readiness"),
                }
            }
        }
    }

    fn finish(mut self) -> Result<CliSignalOutcome> {
        self.stop();
        self.join()
    }

    fn stop(&mut self) {
        if let Some(completion) = self.completion.take() {
            let _send_result = completion.send(());
        }
    }

    fn join(&mut self) -> Result<CliSignalOutcome> {
        match self.worker.take() {
            Some(worker) => worker
                .join()
                .map_err(|_| anyhow::anyhow!("CLI signal monitor panicked"))?,
            None => Ok(CliSignalOutcome::Completed),
        }
    }
}

impl Drop for CliSignalMonitor {
    fn drop(&mut self) {
        self.stop();
        let _join_result = self.join();
    }
}

fn run_signal_monitor(
    close_handle: InspectorCommandSurfaceCloseHandle,
    mut completed: oneshot::Receiver<()>,
    ready: mpsc::SyncSender<std::result::Result<(), String>>,
) -> Result<CliSignalOutcome> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to create CLI signal runtime");
    let runtime = match runtime {
        Ok(runtime) => runtime,
        Err(error) => {
            let _send_result = ready.send(Err(error.to_string()));
            return Err(error);
        }
    };
    runtime.block_on(async move {
        let mut listener = tokio::spawn(wait_for_termination_signal());
        tokio::task::yield_now().await;
        ready
            .send(Ok(()))
            .context("CLI signal monitor readiness receiver closed")?;
        tokio::select! {
            biased;
            _completion = &mut completed => {
                listener.abort();
                let _listener_result = listener.await;
                Ok(CliSignalOutcome::Completed)
            }
            signal = &mut listener => {
                let signal = signal
                    .context("CLI signal listener task failed")?
                    .context("CLI signal listener failed")?;
                close_handle
                    .begin_close()
                    .with_context(|| {
                        format!("failed to begin CLI shutdown after termination signal {signal}")
                    })?;
                Ok(CliSignalOutcome::Interrupted(signal))
            }
        }
    })
}

async fn wait_for_termination_signal() -> std::io::Result<&'static str> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result.map(|()| "SIGINT"),
            _signal = terminate.recv() => Ok("SIGTERM"),
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await.map(|()| "Ctrl-C")
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{Context as _, Result};
    use serde_json::json;

    use super::*;

    #[test]
    fn interrupted_call_retains_operation_and_shutdown_failures() -> Result<()> {
        let error = interrupted_call_result(
            "SIGINT",
            Err(anyhow::anyhow!("remote cleanup remains unconfirmed")),
            Err(anyhow::anyhow!("surface drain failed")),
        )
        .err()
        .context("interrupted call unexpectedly succeeded")?;
        let message = format!("{error:#}");
        anyhow::ensure!(
            message.contains("interrupted by SIGINT")
                && message.contains("remote cleanup remains unconfirmed")
                && message.contains("surface drain failed"),
            "interrupted call lost failure evidence: {message}"
        );
        Ok(())
    }

    #[test]
    fn completed_call_wins_racing_signal() -> Result<()> {
        let expected = json!({ "downloaded": true, "backup_catalog_id": "backup-1" });
        let receipt = interrupted_call_result("SIGINT", Ok(expected.clone()), Ok(()))?;
        let (value, post_result_error) = receipt.into_parts();
        anyhow::ensure!(value == expected);
        anyhow::ensure!(post_result_error.is_none());
        Ok(())
    }

    #[test]
    fn completed_call_preserves_result_when_signal_shutdown_fails() -> Result<()> {
        let expected = json!({ "downloaded": true, "backup_catalog_id": "backup-1" });
        let receipt = interrupted_call_result(
            "SIGTERM",
            Ok(expected.clone()),
            Err(anyhow::anyhow!("surface drain failed")),
        )?;
        let (value, post_result_error) = receipt.into_parts();
        anyhow::ensure!(value == expected);
        let error = post_result_error.context("shutdown failure was not retained")?;
        let message = format!("{error:#}");
        anyhow::ensure!(message.contains("interrupted by SIGTERM"));
        anyhow::ensure!(message.contains("shutdown failed"));
        anyhow::ensure!(message.contains("surface drain failed"));
        Ok(())
    }

    #[test]
    fn signal_monitor_failure_retains_operation_and_shutdown_failures() -> Result<()> {
        let error = failed_signal_monitor_result(
            Err(anyhow::anyhow!("operation cleanup failed")),
            anyhow::anyhow!("signal listener failed"),
            Err(anyhow::anyhow!("surface drain failed")),
        )
        .err()
        .context("failed signal monitor unexpectedly returned success")?;
        let message = format!("{error:#}");
        anyhow::ensure!(
            message.contains("signal listener failed")
                && message.contains("operation cleanup failed")
                && message.contains("surface drain failed"),
            "signal monitor failure lost evidence: {message}"
        );
        Ok(())
    }
}
