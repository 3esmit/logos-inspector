use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use anyhow::{Context as _, Result, bail};

#[derive(Clone)]
pub(crate) struct BlockingWorkTracker {
    inner: Arc<BlockingWorkTrackerInner>,
}

struct BlockingWorkTrackerInner {
    state: Mutex<BlockingWorkState>,
    idle: Condvar,
}

struct BlockingWorkState {
    accepting: bool,
    active: usize,
}

#[must_use = "the guard must remain alive for the full blocking-work lifetime"]
pub(crate) struct BlockingWorkGuard {
    tracker: BlockingWorkTracker,
}

impl BlockingWorkTracker {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(BlockingWorkTrackerInner {
                state: Mutex::new(BlockingWorkState {
                    accepting: true,
                    active: 0,
                }),
                idle: Condvar::new(),
            }),
        }
    }

    pub(crate) fn worker_guard(&self) -> Result<BlockingWorkGuard> {
        let mut state = self.lock_state();
        if !state.accepting {
            bail!("blocking work tracker is closed");
        }
        state.active = state
            .active
            .checked_add(1)
            .context("blocking work tracker capacity overflow")?;
        Ok(BlockingWorkGuard {
            tracker: self.clone(),
        })
    }

    pub(crate) fn stop_accepting(&self) {
        self.lock_state().accepting = false;
    }

    pub(crate) fn wait_idle(&self) {
        let mut state = self.lock_state();
        while state.active != 0 {
            state = match self.inner.idle.wait(state) {
                Ok(state) => state,
                Err(poisoned) => poisoned.into_inner(),
            };
        }
    }

    fn lock_state(&self) -> MutexGuard<'_, BlockingWorkState> {
        match self.inner.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl Drop for BlockingWorkGuard {
    fn drop(&mut self) {
        let mut state = self.tracker.lock_state();
        if state.active != 0 {
            state.active -= 1;
        }
        if state.active == 0 {
            self.tracker.inner.idle.notify_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::mpsc, thread, time::Duration};

    use anyhow::{Result, bail};

    use super::BlockingWorkTracker;

    #[test]
    fn close_rejects_new_work_and_waits_for_existing_guard() -> Result<()> {
        let tracker = BlockingWorkTracker::new();
        let guard = tracker.worker_guard()?;
        tracker.stop_accepting();
        if tracker.worker_guard().is_ok() {
            bail!("closed work tracker accepted a new worker");
        }

        let waiter = tracker.clone();
        let (waiting, observed_wait) = mpsc::channel();
        let (idle, observed_idle) = mpsc::channel();
        let thread = thread::spawn(move || {
            let _waiting_result = waiting.send(());
            waiter.wait_idle();
            let _idle_result = idle.send(());
        });
        observed_wait.recv()?;
        if observed_idle
            .recv_timeout(Duration::from_millis(20))
            .is_ok()
        {
            bail!("work tracker reported idle while a worker guard remained live");
        }

        drop(guard);
        observed_idle.recv_timeout(Duration::from_secs(1))?;
        thread
            .join()
            .map_err(|_| anyhow::anyhow!("work tracker waiter panicked"))?;
        Ok(())
    }
}
