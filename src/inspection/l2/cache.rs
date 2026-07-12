use std::collections::VecDeque;

use serde::Serialize;

use super::{
    L2AccountSnapshotData, L2AccountValue, L2BlockAnchor, NetworkScope, NormalizedL2Block,
};
use crate::lez::TransactionSummary;

const MAX_CACHE_ENTRIES: usize = 128;
const MAX_CACHE_BYTES: usize = 32 * 1024 * 1024;
const MAX_CACHE_ENTRY_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct L2CacheScope {
    pub schema_version: u32,
    pub network_scope: NetworkScope,
    pub channel_id: String,
    pub source_id: String,
    pub source_config_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum L2CacheEntityKind {
    Block,
    Transaction,
    HistoricalAccount,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct L2CacheKey {
    scope: L2CacheScope,
    entity_kind: L2CacheEntityKind,
    content_key: String,
}

#[derive(Debug, Clone, Serialize)]
enum L2CachedPayload {
    Block(NormalizedL2Block),
    Transaction(TransactionSummary),
    HistoricalAccount {
        account: L2AccountValue,
        anchor: L2BlockAnchor,
    },
}

#[derive(Debug, Clone)]
struct L2CacheEntry {
    key: L2CacheKey,
    payload: L2CachedPayload,
    serialized_bytes: usize,
}

#[derive(Debug, Default)]
pub(crate) struct L2EvidenceCache {
    entries: VecDeque<L2CacheEntry>,
    serialized_bytes: usize,
}

impl L2EvidenceCache {
    pub(crate) fn block(
        &mut self,
        scope: &L2CacheScope,
        canonical_key: &str,
    ) -> Option<NormalizedL2Block> {
        let key = L2CacheKey {
            scope: scope.clone(),
            entity_kind: L2CacheEntityKind::Block,
            content_key: canonical_key.to_owned(),
        };
        match self.get(&key) {
            Some(L2CachedPayload::Block(block)) => Some(block),
            _ => None,
        }
    }

    pub(crate) fn block_by_id(
        &mut self,
        scope: &L2CacheScope,
        block_id: u64,
    ) -> Option<NormalizedL2Block> {
        let position = self.entries.iter().position(|entry| {
            entry.key.scope == *scope
                && entry.key.entity_kind == L2CacheEntityKind::Block
                && matches!(
                    &entry.payload,
                    L2CachedPayload::Block(block) if block.summary.block_id == block_id
                )
        })?;
        match self.promote(position) {
            Some(L2CachedPayload::Block(block)) => Some(block),
            _ => None,
        }
    }

    pub(crate) fn block_by_hash(
        &mut self,
        scope: &L2CacheScope,
        block_hash: &str,
    ) -> Option<NormalizedL2Block> {
        let position = self.entries.iter().position(|entry| {
            entry.key.scope == *scope
                && entry.key.entity_kind == L2CacheEntityKind::Block
                && matches!(
                    &entry.payload,
                    L2CachedPayload::Block(block) if block.summary.block_hash == block_hash
                )
        })?;
        match self.promote(position) {
            Some(L2CachedPayload::Block(block)) => Some(block),
            _ => None,
        }
    }

    pub(crate) fn insert_block(&mut self, scope: L2CacheScope, block: NormalizedL2Block) {
        if block.summary.block_hash.is_empty() {
            return;
        }
        self.insert(
            L2CacheKey {
                scope,
                entity_kind: L2CacheEntityKind::Block,
                content_key: block.summary.canonical_key(),
            },
            L2CachedPayload::Block(block),
        );
    }

    pub(crate) fn transaction(
        &mut self,
        scope: &L2CacheScope,
        transaction_id: &str,
    ) -> Option<TransactionSummary> {
        let key = L2CacheKey {
            scope: scope.clone(),
            entity_kind: L2CacheEntityKind::Transaction,
            content_key: transaction_id.to_owned(),
        };
        match self.get(&key) {
            Some(L2CachedPayload::Transaction(transaction)) => Some(transaction),
            _ => None,
        }
    }

    pub(crate) fn insert_transaction(
        &mut self,
        scope: L2CacheScope,
        transaction: TransactionSummary,
    ) {
        let transaction_id = transaction.hash.clone();
        if transaction_id.is_empty() {
            return;
        }
        self.insert(
            L2CacheKey {
                scope,
                entity_kind: L2CacheEntityKind::Transaction,
                content_key: transaction_id,
            },
            L2CachedPayload::Transaction(transaction),
        );
    }

    pub(crate) fn historical_account(
        &mut self,
        scope: &L2CacheScope,
        account_id: &str,
        anchor: &L2BlockAnchor,
    ) -> Option<L2AccountSnapshotData> {
        let key = L2CacheKey {
            scope: scope.clone(),
            entity_kind: L2CacheEntityKind::HistoricalAccount,
            content_key: historical_account_key(account_id, anchor),
        };
        match self.get(&key) {
            Some(L2CachedPayload::HistoricalAccount { account, anchor }) => {
                Some(L2AccountSnapshotData {
                    account,
                    anchor: Some(anchor),
                    after_anchor: None,
                    anchor_state: super::L2AccountAnchorState::Exact,
                    source: super::L2SourceObservation {
                        source_id: scope.source_id.clone(),
                        source_role: super::ZoneSourceRole::Indexer,
                        source_config_revision: scope.source_config_revision,
                        finality: super::L2RouteFinality::Finalized,
                        retrieval: super::L2Retrieval::MemoryCache,
                    },
                })
            }
            _ => None,
        }
    }

    pub(crate) fn insert_historical_account(
        &mut self,
        scope: L2CacheScope,
        account: L2AccountValue,
        anchor: L2BlockAnchor,
    ) {
        let key = L2CacheKey {
            content_key: historical_account_key(&account.account_id, &anchor),
            scope,
            entity_kind: L2CacheEntityKind::HistoricalAccount,
        };
        self.insert(key, L2CachedPayload::HistoricalAccount { account, anchor });
    }

    pub(crate) fn retain_scopes(&mut self, valid_scopes: &[L2CacheScope]) {
        self.entries
            .retain(|entry| valid_scopes.contains(&entry.key.scope));
        self.serialized_bytes = self
            .entries
            .iter()
            .map(|entry| entry.serialized_bytes)
            .sum();
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.serialized_bytes = 0;
    }

    fn get(&mut self, key: &L2CacheKey) -> Option<L2CachedPayload> {
        let position = self.entries.iter().position(|entry| entry.key == *key)?;
        self.promote(position)
    }

    fn promote(&mut self, position: usize) -> Option<L2CachedPayload> {
        let entry = self.entries.remove(position)?;
        let payload = entry.payload.clone();
        self.entries.push_back(entry);
        Some(payload)
    }

    fn insert(&mut self, key: L2CacheKey, payload: L2CachedPayload) {
        let Ok(serialized) = serde_json::to_vec(&payload) else {
            return;
        };
        let serialized_bytes = serialized.len();
        if serialized_bytes > MAX_CACHE_ENTRY_BYTES {
            return;
        }
        if let Some(position) = self.entries.iter().position(|entry| entry.key == key)
            && let Some(previous) = self.entries.remove(position)
        {
            self.serialized_bytes = self
                .serialized_bytes
                .saturating_sub(previous.serialized_bytes);
        }
        self.serialized_bytes = self.serialized_bytes.saturating_add(serialized_bytes);
        self.entries.push_back(L2CacheEntry {
            key,
            payload,
            serialized_bytes,
        });
        while self.entries.len() > MAX_CACHE_ENTRIES || self.serialized_bytes > MAX_CACHE_BYTES {
            let Some(evicted) = self.entries.pop_front() else {
                self.serialized_bytes = 0;
                break;
            };
            self.serialized_bytes = self
                .serialized_bytes
                .saturating_sub(evicted.serialized_bytes);
        }
    }

    #[cfg(test)]
    pub(crate) fn stats(&self) -> (usize, usize) {
        (self.entries.len(), self.serialized_bytes)
    }
}

fn historical_account_key(account_id: &str, anchor: &L2BlockAnchor) -> String {
    format!(
        "account:{account_id}:block:{}:{}",
        anchor.block_id, anchor.block_hash
    )
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};

    use super::*;

    #[test]
    fn cache_isolates_scopes_and_promotes_hits() -> Result<()> {
        let mut cache = L2EvidenceCache::default();
        let first = scope('a', 1);
        let second = scope('b', 1);
        let transaction = transaction('1');
        cache.insert_transaction(first.clone(), transaction.clone());

        if cache.transaction(&second, &transaction.hash).is_some() {
            bail!("transaction crossed cache scope");
        }
        if cache.transaction(&first, &transaction.hash) != Some(transaction) {
            bail!("transaction cache hit was lost");
        }
        Ok(())
    }

    #[test]
    fn cache_enforces_entry_and_single_payload_bounds() -> Result<()> {
        let mut cache = L2EvidenceCache::default();
        let scope = scope('a', 1);
        for index in 0..=MAX_CACHE_ENTRIES {
            let digit = char::from_digit(u32::try_from(index % 16)?, 16)
                .ok_or_else(|| anyhow::anyhow!("test hex digit is invalid"))?;
            let mut item = transaction(digit);
            item.hash = format!("{index:064x}");
            cache.insert_transaction(scope.clone(), item);
        }
        let (entries, bytes) = cache.stats();
        if entries != MAX_CACHE_ENTRIES || bytes > MAX_CACHE_BYTES {
            bail!("cache bounds were not enforced: {entries} entries, {bytes} bytes");
        }

        let mut oversized = transaction('f');
        oversized.kind = "x".repeat(MAX_CACHE_ENTRY_BYTES.saturating_add(1));
        cache.insert_transaction(scope.clone(), oversized.clone());
        if cache.transaction(&scope, &oversized.hash).is_some() {
            bail!("oversized cache payload was inserted");
        }
        Ok(())
    }

    fn scope(source: char, revision: u64) -> L2CacheScope {
        L2CacheScope {
            schema_version: 1,
            network_scope: NetworkScope::GenesisId {
                genesis_id: "11".repeat(32),
            },
            channel_id: "22".repeat(32),
            source_id: format!("src_{source}"),
            source_config_revision: revision,
        }
    }

    fn transaction(hash: char) -> TransactionSummary {
        TransactionSummary {
            hash: hash.to_string().repeat(64),
            kind: "Public".to_owned(),
            program_id_hex: None,
            account_ids: Vec::new(),
            nonces: Vec::new(),
            instruction_data: Vec::new(),
            bytecode_len: None,
            raw_signature_valid: None,
            message_prehash: None,
            prehash_signature_valid: None,
        }
    }
}
