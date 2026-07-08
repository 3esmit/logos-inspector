use anyhow::{Result, bail};
use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    account_lookup, account_lookup_with_idl, indexer_block_by_hash,
    inspection::l2::lez::{
        IndexerBlockReport, RegisteredIdlResolver, TransactionIdlInspectionReport,
        sequencer_account, sequencer_transaction_inspection,
        sequencer_transaction_inspection_with_idl,
    },
    source_routing::{self, AccountSources, CoreEndpointMode},
    state_store::registered_idl_entries,
};

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LezResolvedTarget {
    Block {
        payload: IndexerBlockReport,
        source_path: &'static str,
    },
    Transaction {
        payload: Value,
        source_path: &'static str,
        decode_source: &'static str,
    },
    Account {
        payload: Value,
        source_path: &'static str,
        decode_source: &'static str,
    },
}

pub(crate) struct LezInspectionSession<'a> {
    sources: AccountSources<'a>,
}

impl<'a> LezInspectionSession<'a> {
    #[must_use]
    pub(crate) fn new(sources: AccountSources<'a>) -> Self {
        Self { sources }
    }

    pub(crate) async fn resolve_target(&self, target: &str) -> Result<Option<LezResolvedTarget>> {
        if let Some(block) = self.indexer_block_by_hash(target).await? {
            return Ok(Some(LezResolvedTarget::Block {
                payload: block,
                source_path: self.indexer_source_path(),
            }));
        }
        if let Some(transaction) = self.inspect_transaction(target, None).await? {
            return Ok(Some(LezResolvedTarget::Transaction {
                payload: transaction,
                source_path: self.execution_source_path(),
                decode_source: "registered_or_raw",
            }));
        }
        let account = self.account(target, None, None).await?;
        if account.is_null() {
            return Ok(None);
        }
        Ok(Some(LezResolvedTarget::Account {
            payload: account,
            source_path: self.account_source_path(),
            decode_source: "registered_or_raw",
        }))
    }

    async fn indexer_block_by_hash(&self, hash: &str) -> Result<Option<IndexerBlockReport>> {
        match self.sources.indexer_mode {
            CoreEndpointMode::Module => source_routing::indexer_block_by_hash(hash),
            CoreEndpointMode::Rpc => {
                indexer_block_by_hash(self.sources.indexer_endpoint, hash).await
            }
        }
    }

    async fn inspect_transaction(
        &self,
        hash: &str,
        explicit_idl: Option<&str>,
    ) -> Result<Option<Value>> {
        if self.sources.execution_mode == CoreEndpointMode::Module {
            bail!(
                "{} does not expose Inspector transaction reads; use sequencer RPC for transaction inspection",
                source_routing::LEZ_CORE_MODULE
            );
        }
        if let Some(idl) = explicit_idl {
            return Ok(Some(json!(
                sequencer_transaction_inspection_with_idl(
                    self.sources.sequencer_endpoint,
                    hash,
                    idl
                )
                .await?
            )));
        }
        let Some(inspection) =
            sequencer_transaction_inspection(self.sources.sequencer_endpoint, hash).await?
        else {
            return Ok(None);
        };
        if let Some(report) = registered_decode(&inspection.raw_summary)? {
            return Ok(Some(json!(report)));
        }
        Ok(Some(json!(inspection)))
    }

    async fn account(
        &self,
        account: &str,
        explicit_idl: Option<&str>,
        account_type: Option<&str>,
    ) -> Result<Value> {
        if self.sources.execution_mode == CoreEndpointMode::Module {
            bail!(
                "{} does not expose Inspector account reads; use sequencer RPC for account inspection",
                source_routing::LEZ_CORE_MODULE
            );
        }
        let mut value = if self.sources.indexer_mode == CoreEndpointMode::Module {
            let mut account_report =
                sequencer_account(self.sources.sequencer_endpoint, account).await?;
            source_routing::attach_module_account_transactions(&mut account_report);
            if let Some(idl) = explicit_idl {
                json!(crate::lez::account_report_with_optional_idl_decode(
                    account_report,
                    idl,
                    account_type,
                ))
            } else {
                json!(account_report)
            }
        } else if let Some(idl) = explicit_idl {
            json!(
                account_lookup_with_idl(
                    self.sources.sequencer_endpoint,
                    self.sources.indexer_endpoint,
                    account,
                    idl,
                    account_type,
                )
                .await?
            )
        } else {
            json!(
                account_lookup(
                    self.sources.sequencer_endpoint,
                    self.sources.indexer_endpoint,
                    account,
                )
                .await?
            )
        };
        let idl_entries = registered_idl_entries()?;
        RegisteredIdlResolver::new(&idl_entries)
            .enrich_account_related_transaction_decodes(&mut value)?;
        Ok(value)
    }

    fn execution_source_path(&self) -> &'static str {
        match self.sources.execution_mode {
            CoreEndpointMode::Module => "execution_module",
            CoreEndpointMode::Rpc => "sequencer_rpc",
        }
    }

    fn indexer_source_path(&self) -> &'static str {
        match self.sources.indexer_mode {
            CoreEndpointMode::Module => "indexer_module",
            CoreEndpointMode::Rpc => "indexer_rpc",
        }
    }

    fn account_source_path(&self) -> &'static str {
        match (self.sources.execution_mode, self.sources.indexer_mode) {
            (CoreEndpointMode::Rpc, CoreEndpointMode::Module) => "sequencer_rpc+indexer_module",
            (CoreEndpointMode::Rpc, CoreEndpointMode::Rpc) => "sequencer_rpc+indexer_rpc",
            (CoreEndpointMode::Module, CoreEndpointMode::Module) => {
                "execution_module+indexer_module"
            }
            (CoreEndpointMode::Module, CoreEndpointMode::Rpc) => "execution_module+indexer_rpc",
        }
    }
}

fn registered_decode(
    summary: &crate::TransactionSummary,
) -> Result<Option<TransactionIdlInspectionReport>> {
    let idl_entries = registered_idl_entries()?;
    Ok(RegisteredIdlResolver::new(&idl_entries).transaction_inspection(summary))
}
