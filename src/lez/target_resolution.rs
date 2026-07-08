use anyhow::{Result, bail};
use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    account_lookup, account_lookup_with_idl, indexer_block_by_hash,
    lez::{
        IndexerBlockReport, RegisteredIdlResolver, TransactionIdlInspectionReport,
        TransactionTraceReport, sequencer_account, sequencer_transaction_inspection,
        sequencer_transaction_inspection_with_idl, sequencer_transaction_trace,
        sequencer_transaction_trace_with_idl,
    },
    source_routing::{self, AccountSources, CoreEndpointMode, SourceEndpoint},
    support::state_store::registered_idl_entries,
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

pub(crate) struct LezTargetResolver<'a> {
    execution_mode: CoreEndpointMode,
    sequencer_endpoint: &'a str,
    execution_module: &'static str,
    indexer_mode: CoreEndpointMode,
    indexer_endpoint: &'a str,
}

impl<'a> LezTargetResolver<'a> {
    #[must_use]
    pub(crate) fn from_account_sources(sources: AccountSources<'a>) -> Self {
        Self {
            execution_mode: sources.execution_mode,
            sequencer_endpoint: sources.sequencer_endpoint,
            execution_module: source_routing::LEZ_CORE_MODULE,
            indexer_mode: sources.indexer_mode,
            indexer_endpoint: sources.indexer_endpoint,
        }
    }

    #[must_use]
    pub(crate) fn from_execution_source(source: SourceEndpoint<'a>) -> Self {
        Self {
            execution_mode: source.mode,
            sequencer_endpoint: source.endpoint,
            execution_module: source.module,
            indexer_mode: CoreEndpointMode::Module,
            indexer_endpoint: "",
        }
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
        let account = self.inspect_account(target, None, None).await?;
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
        match self.indexer_mode {
            CoreEndpointMode::Module => source_routing::indexer_block_by_hash(hash),
            CoreEndpointMode::Rpc => indexer_block_by_hash(self.indexer_endpoint, hash).await,
        }
    }

    pub(crate) async fn inspect_transaction(
        &self,
        hash: &str,
        explicit_idl: Option<&str>,
    ) -> Result<Option<Value>> {
        self.require_rpc_execution("inspectTransaction")?;
        if let Some(idl) = explicit_idl {
            return Ok(Some(json!(
                sequencer_transaction_inspection_with_idl(self.sequencer_endpoint, hash, idl)
                    .await?
            )));
        }
        let Some(inspection) =
            sequencer_transaction_inspection(self.sequencer_endpoint, hash).await?
        else {
            return Ok(None);
        };
        if let Some(report) = registered_decode(&inspection.raw_summary)? {
            return Ok(Some(json!(report)));
        }
        Ok(Some(json!(inspection)))
    }

    pub(crate) async fn trace_transaction(
        &self,
        hash: &str,
        explicit_idl: Option<&str>,
    ) -> Result<Option<Value>> {
        self.require_rpc_execution("traceTransaction")?;
        if let Some(idl) = explicit_idl {
            return Ok(Some(json!(
                sequencer_transaction_trace_with_idl(self.sequencer_endpoint, hash, idl).await?
            )));
        }
        let Some(trace) = sequencer_transaction_trace(self.sequencer_endpoint, hash).await? else {
            return Ok(None);
        };
        if let Some(report) = registered_trace(&trace.inspection.raw_summary)? {
            return Ok(Some(json!(report)));
        }
        Ok(Some(json!(trace)))
    }

    pub(crate) async fn inspect_account(
        &self,
        account: &str,
        explicit_idl: Option<&str>,
        account_type: Option<&str>,
    ) -> Result<Value> {
        if self.execution_mode == CoreEndpointMode::Module {
            bail!(
                "{} does not expose Inspector account reads; use sequencer RPC for account inspection",
                source_routing::LEZ_CORE_MODULE
            );
        }
        let mut value = if self.indexer_mode == CoreEndpointMode::Module {
            let mut account_report = sequencer_account(self.sequencer_endpoint, account).await?;
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
                    self.sequencer_endpoint,
                    self.indexer_endpoint,
                    account,
                    idl,
                    account_type,
                )
                .await?
            )
        } else {
            json!(account_lookup(self.sequencer_endpoint, self.indexer_endpoint, account,).await?)
        };
        let idl_entries = registered_idl_entries()?;
        RegisteredIdlResolver::new(&idl_entries)
            .enrich_account_related_transaction_decodes(&mut value)?;
        Ok(value)
    }

    fn execution_source_path(&self) -> &'static str {
        match self.execution_mode {
            CoreEndpointMode::Module => "execution_module",
            CoreEndpointMode::Rpc => "sequencer_rpc",
        }
    }

    fn indexer_source_path(&self) -> &'static str {
        match self.indexer_mode {
            CoreEndpointMode::Module => "indexer_module",
            CoreEndpointMode::Rpc => "indexer_rpc",
        }
    }

    fn account_source_path(&self) -> &'static str {
        match (self.execution_mode, self.indexer_mode) {
            (CoreEndpointMode::Rpc, CoreEndpointMode::Module) => "sequencer_rpc+indexer_module",
            (CoreEndpointMode::Rpc, CoreEndpointMode::Rpc) => "sequencer_rpc+indexer_rpc",
            (CoreEndpointMode::Module, CoreEndpointMode::Module) => {
                "execution_module+indexer_module"
            }
            (CoreEndpointMode::Module, CoreEndpointMode::Rpc) => "execution_module+indexer_rpc",
        }
    }

    fn require_rpc_execution(&self, method: &str) -> Result<()> {
        if self.execution_mode == CoreEndpointMode::Rpc {
            return Ok(());
        }
        bail!(
            "`{method}` is not exposed by the selected Basecamp module source `{}`; use RPC source for this call",
            self.execution_module
        )
    }
}

fn registered_decode(
    summary: &crate::TransactionSummary,
) -> Result<Option<TransactionIdlInspectionReport>> {
    let idl_entries = registered_idl_entries()?;
    Ok(RegisteredIdlResolver::new(&idl_entries).transaction_inspection(summary))
}

fn registered_trace(summary: &crate::TransactionSummary) -> Result<Option<TransactionTraceReport>> {
    let idl_entries = registered_idl_entries()?;
    Ok(RegisteredIdlResolver::new(&idl_entries).transaction_trace(summary))
}
