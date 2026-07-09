use anyhow::{Result, bail};
use serde::Serialize;
use serde_json::{Value, json};

use super::target_decode::LezTargetDecodeCoordinator;
use crate::{
    account_lookup, account_lookup_with_idl, indexer_block_by_hash,
    lez::{
        IndexerBlockReport, inspect_transaction_summary_with_optional_idl_decode,
        sequencer_account, sequencer_transaction_inspection, sequencer_transaction_trace,
        sequencer_transaction_trace_with_idl,
    },
    source_routing::{self, AccountSources, CoreEndpointMode, SourceEndpoint},
};

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LezResolvedTarget {
    Block {
        payload: IndexerBlockReport,
        source_path: &'static str,
        source_provenance: &'static str,
    },
    Transaction {
        payload: Value,
        source_path: &'static str,
        source_provenance: &'static str,
        decode_source: &'static str,
    },
    Account {
        payload: Value,
        source_path: &'static str,
        source_provenance: &'static str,
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
        let block_resolver = self.block_resolver();
        if let Some(block) = block_resolver.resolve(target).await? {
            return Ok(Some(LezResolvedTarget::Block {
                payload: block,
                source_path: block_resolver.source_path(),
                source_provenance: block_resolver.source_path(),
            }));
        }

        let transaction_resolver = self.transaction_resolver();
        if let Some(transaction) = transaction_resolver.inspect(target, None).await? {
            return Ok(Some(LezResolvedTarget::Transaction {
                decode_source: LezTargetDecodeCoordinator::decode_source_for_payload(
                    &transaction,
                    "registered_or_raw",
                ),
                payload: transaction,
                source_path: transaction_resolver.source_path(),
                source_provenance: transaction_resolver.source_path(),
            }));
        }

        let account_resolver = self.account_resolver();
        let account = account_resolver.inspect(target, None, None).await?;
        if account.is_null() {
            return Ok(None);
        }
        Ok(Some(LezResolvedTarget::Account {
            payload: account,
            source_path: account_resolver.source_path(),
            source_provenance: account_resolver.source_path(),
            decode_source: "registered_or_raw",
        }))
    }

    pub(crate) async fn inspect_transaction(
        &self,
        hash: &str,
        explicit_idl: Option<&str>,
    ) -> Result<Option<Value>> {
        self.transaction_resolver()
            .inspect(hash, explicit_idl)
            .await
    }

    pub(crate) async fn trace_transaction(
        &self,
        hash: &str,
        explicit_idl: Option<&str>,
    ) -> Result<Option<Value>> {
        self.transaction_resolver().trace(hash, explicit_idl).await
    }

    pub(crate) async fn inspect_account(
        &self,
        account: &str,
        explicit_idl: Option<&str>,
        account_type: Option<&str>,
    ) -> Result<Value> {
        self.account_resolver()
            .inspect(account, explicit_idl, account_type)
            .await
    }

    fn block_resolver(&self) -> LezBlockTargetResolver<'a> {
        LezBlockTargetResolver {
            mode: self.indexer_mode,
            endpoint: self.indexer_endpoint,
        }
    }

    fn transaction_resolver(&self) -> LezTransactionTargetResolver<'a> {
        LezTransactionTargetResolver {
            mode: self.execution_mode,
            endpoint: self.sequencer_endpoint,
            module: self.execution_module,
        }
    }

    fn account_resolver(&self) -> LezAccountTargetResolver<'a> {
        LezAccountTargetResolver {
            execution_mode: self.execution_mode,
            sequencer_endpoint: self.sequencer_endpoint,
            indexer_mode: self.indexer_mode,
            indexer_endpoint: self.indexer_endpoint,
        }
    }
}

struct LezBlockTargetResolver<'a> {
    mode: CoreEndpointMode,
    endpoint: &'a str,
}

impl LezBlockTargetResolver<'_> {
    async fn resolve(&self, hash: &str) -> Result<Option<IndexerBlockReport>> {
        match self.mode {
            CoreEndpointMode::Module => source_routing::indexer_block_by_hash(hash),
            CoreEndpointMode::Rpc => indexer_block_by_hash(self.endpoint, hash).await,
        }
    }

    fn source_path(&self) -> &'static str {
        match self.mode {
            CoreEndpointMode::Module => "indexer_module",
            CoreEndpointMode::Rpc => "indexer_rpc",
        }
    }
}

struct LezTransactionTargetResolver<'a> {
    mode: CoreEndpointMode,
    endpoint: &'a str,
    module: &'static str,
}

impl LezTransactionTargetResolver<'_> {
    async fn inspect(&self, hash: &str, explicit_idl: Option<&str>) -> Result<Option<Value>> {
        self.require_rpc_execution("inspectTransaction")?;
        let Some(inspection) = sequencer_transaction_inspection(self.endpoint, hash).await? else {
            return Ok(None);
        };
        if let Some(idl) = explicit_idl {
            return Ok(Some(json!(
                inspect_transaction_summary_with_optional_idl_decode(
                    &inspection.raw_summary,
                    idl,
                    "explicit_idl",
                )
            )));
        }
        if let Some(report) =
            LezTargetDecodeCoordinator::registered_transaction_inspection(&inspection.raw_summary)?
        {
            return Ok(Some(json!(report)));
        }
        Ok(Some(json!(inspection)))
    }

    async fn trace(&self, hash: &str, explicit_idl: Option<&str>) -> Result<Option<Value>> {
        self.require_rpc_execution("traceTransaction")?;
        if let Some(idl) = explicit_idl {
            return Ok(Some(json!(
                sequencer_transaction_trace_with_idl(self.endpoint, hash, idl).await?
            )));
        }
        let Some(trace) = sequencer_transaction_trace(self.endpoint, hash).await? else {
            return Ok(None);
        };
        if let Some(report) =
            LezTargetDecodeCoordinator::registered_transaction_trace(&trace.inspection.raw_summary)?
        {
            return Ok(Some(json!(report)));
        }
        Ok(Some(json!(trace)))
    }

    fn source_path(&self) -> &'static str {
        match self.mode {
            CoreEndpointMode::Module => "execution_module",
            CoreEndpointMode::Rpc => "sequencer_rpc",
        }
    }

    fn require_rpc_execution(&self, method: &str) -> Result<()> {
        if self.mode == CoreEndpointMode::Rpc {
            return Ok(());
        }
        bail!(
            "`{method}` is not exposed by the selected Basecamp module source `{}`; use RPC source for this call",
            self.module
        )
    }
}

struct LezAccountTargetResolver<'a> {
    execution_mode: CoreEndpointMode,
    sequencer_endpoint: &'a str,
    indexer_mode: CoreEndpointMode,
    indexer_endpoint: &'a str,
}

impl LezAccountTargetResolver<'_> {
    async fn inspect(
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
        LezTargetDecodeCoordinator::enrich_account_related_transaction_decodes(&mut value)?;
        Ok(value)
    }

    fn source_path(&self) -> &'static str {
        match (self.execution_mode, self.indexer_mode) {
            (CoreEndpointMode::Rpc, CoreEndpointMode::Module) => "sequencer_rpc+indexer_module",
            (CoreEndpointMode::Rpc, CoreEndpointMode::Rpc) => "sequencer_rpc+indexer_rpc",
            (CoreEndpointMode::Module, CoreEndpointMode::Module) => {
                "execution_module+indexer_module"
            }
            (CoreEndpointMode::Module, CoreEndpointMode::Rpc) => "execution_module+indexer_rpc",
        }
    }
}
