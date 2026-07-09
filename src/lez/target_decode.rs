use anyhow::Result;
use serde_json::Value;

use crate::support::state_store::registered_idl_entries;

use super::{
    RegisteredIdlResolver, TransactionIdlInspectionReport, TransactionSummary,
    TransactionTraceReport,
};

pub(crate) struct LezTargetDecodeCoordinator;

impl LezTargetDecodeCoordinator {
    pub(crate) fn registered_transaction_inspection(
        summary: &TransactionSummary,
    ) -> Result<Option<TransactionIdlInspectionReport>> {
        let idl_entries = registered_idl_entries()?;
        Ok(RegisteredIdlResolver::new(&idl_entries).transaction_inspection(summary))
    }

    pub(crate) fn registered_transaction_trace(
        summary: &TransactionSummary,
    ) -> Result<Option<TransactionTraceReport>> {
        let idl_entries = registered_idl_entries()?;
        Ok(RegisteredIdlResolver::new(&idl_entries).transaction_trace(summary))
    }

    pub(crate) fn enrich_account_related_transaction_decodes(value: &mut Value) -> Result<()> {
        let idl_entries = registered_idl_entries()?;
        RegisteredIdlResolver::new(&idl_entries).enrich_account_related_transaction_decodes(value)
    }

    #[must_use]
    pub(crate) fn decode_source_for_payload(
        payload: &Value,
        fallback: &'static str,
    ) -> &'static str {
        if payload.get("decode_enrichment").is_some()
            || payload.get("decoded_instruction").is_some()
        {
            return "registered_idl";
        }
        fallback
    }
}
