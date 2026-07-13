use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Result, bail};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct RuntimeOperationId(String);

impl RuntimeOperationId {
    pub(super) fn parse(value: &str) -> Result<Self> {
        let value = value.trim();
        if value.is_empty() {
            bail!("runtime operation id is required");
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub(super) fn allocated(domain: &str, method: &str, sequence: u64) -> Self {
        Self(format!("{domain}-{method}-{sequence}"))
    }

    #[must_use]
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ClientRequestId(String);

impl ClientRequestId {
    pub(super) fn parse(value: &str) -> Result<Self> {
        let value = value.trim();
        if value.is_empty() {
            bail!("client request id cannot be empty");
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct EventCursor(u64);

impl EventCursor {
    #[must_use]
    pub(super) const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub(super) const fn value(self) -> u64 {
        self.0
    }

    pub(super) fn next(self) -> Result<Self> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or_else(|| anyhow::anyhow!("runtime operation event cursor is exhausted"))
    }
}

pub(super) fn allocate_sequence(counter: &AtomicU64) -> Result<u64> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map_err(|_| anyhow::anyhow!("runtime operation id space is exhausted"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_sequence_allocation_rejects_exhaustion_without_wrapping() -> Result<()> {
        let counter = AtomicU64::new(u64::MAX);

        let Err(error) = allocate_sequence(&counter) else {
            anyhow::bail!("exhausted allocator should fail");
        };

        anyhow::ensure!(
            error.to_string().contains("id space is exhausted")
                && counter.load(Ordering::Relaxed) == u64::MAX,
            "operation id allocator wrapped"
        );
        Ok(())
    }
}
