use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    AccountTransactionSummary, TransactionIdlInspectionReport, TransactionSummary,
    TransactionTraceReport, inspect_transaction_summary_with_idl,
    trace_transaction_summary_with_idl,
};
use crate::{AccountIdlDecodeReport, normalize_program_id_hex, state_store::RegisteredIdlEntry};

use super::program_decode_session::ProgramDecodeSession;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramDecodeCandidate {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub name: String,
    #[serde(alias = "program_id_hex")]
    pub program_id_hex: String,
    pub json: String,
    #[serde(default, alias = "account_type")]
    pub account_type: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedDecodeEvidence {
    pub key: String,
    pub name: String,
    pub program_id_hex: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDecodeSelection {
    pub evidence: SelectedDecodeEvidence,
    pub report: AccountIdlDecodeReport,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedAccountDecodeSession {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<AccountDecodeSelection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial: Option<AccountDecodeSelection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionDecodeSelection {
    pub evidence: SelectedDecodeEvidence,
    pub report: TransactionIdlInspectionReport,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedTransactionDecodeSession {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<TransactionDecodeSelection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial: Option<TransactionDecodeSelection>,
}

impl ProgramDecodeCandidate {
    fn from_registered_entry(entry: &RegisteredIdlEntry) -> Self {
        Self {
            key: entry.key.clone().unwrap_or_default(),
            name: entry.name.clone().unwrap_or_default(),
            program_id_hex: entry.program_id_hex.clone(),
            json: entry.json.clone(),
            account_type: None,
            source: entry.source.clone(),
        }
    }

    pub(super) fn evidence(&self, account_type: Option<String>) -> SelectedDecodeEvidence {
        SelectedDecodeEvidence {
            key: self.key.clone(),
            name: self.name.clone(),
            program_id_hex: self.program_id_hex.clone(),
            account_type,
            source: self.source.clone(),
        }
    }
}

pub fn resolve_account_decode_session(
    account_id: Option<&str>,
    data_hex: &str,
    candidates: &[ProgramDecodeCandidate],
) -> ResolvedAccountDecodeSession {
    ProgramDecodeSession::new(candidates).resolve_account(account_id, data_hex)
}

pub fn resolve_transaction_decode_session(
    summary: &TransactionSummary,
    candidates: &[ProgramDecodeCandidate],
) -> ResolvedTransactionDecodeSession {
    ProgramDecodeSession::new(candidates).resolve_transaction(summary)
}

pub(crate) struct RegisteredIdlResolver<'a> {
    entries: &'a [RegisteredIdlEntry],
}

impl<'a> RegisteredIdlResolver<'a> {
    pub(crate) fn new(entries: &'a [RegisteredIdlEntry]) -> Self {
        Self { entries }
    }

    pub(crate) fn transaction_inspection(
        &self,
        summary: &TransactionSummary,
    ) -> Option<TransactionIdlInspectionReport> {
        let candidates = self
            .matching_entries(summary)
            .map(ProgramDecodeCandidate::from_registered_entry)
            .collect::<Vec<_>>();
        let session = resolve_transaction_decode_session(summary, &candidates);
        session
            .selected
            .or(session.partial)
            .map(|selection| selection.report)
    }

    pub(crate) fn transaction_trace(
        &self,
        summary: &TransactionSummary,
    ) -> Option<TransactionTraceReport> {
        let (entry, _) = self.selected_transaction_decode(summary)?;
        trace_transaction_summary_with_idl(summary, &entry.json).ok()
    }

    pub(crate) fn enrich_account_related_transaction_decodes(
        &self,
        value: &mut Value,
    ) -> Result<()> {
        if self.entries.is_empty() {
            return Ok(());
        }
        if let Some(account) = value.get_mut("account") {
            self.enrich_account_report_related_transaction_decodes(account)
        } else {
            self.enrich_account_report_related_transaction_decodes(value)
        }
    }

    fn enrich_account_report_related_transaction_decodes(&self, account: &mut Value) -> Result<()> {
        let Some(transactions) = account
            .get_mut("related_transactions")
            .and_then(Value::as_array_mut)
        else {
            return Ok(());
        };

        for transaction in transactions {
            if transaction.get("decoded_instruction").is_some() {
                continue;
            }
            let Ok(summary) =
                serde_json::from_value::<AccountTransactionSummary>(transaction.clone())
            else {
                continue;
            };
            let summary = TransactionSummary::from(&summary);
            if summary.kind != "Public" || summary.instruction_data.is_empty() {
                continue;
            }
            let Some(report) = self.transaction_inspection(&summary) else {
                continue;
            };
            let Some(decoded) = report.decoded_instruction else {
                continue;
            };
            if let Some(object) = transaction.as_object_mut() {
                object.insert(
                    "decoded_instruction".to_owned(),
                    serde_json::to_value(decoded)
                        .context("failed to serialize transaction decode")?,
                );
            }
        }
        Ok(())
    }

    fn matching_entries(
        &self,
        summary: &TransactionSummary,
    ) -> impl Iterator<Item = &'a RegisteredIdlEntry> + '_ {
        let summary_program_id = summary
            .program_id_hex
            .as_deref()
            .and_then(|value| normalize_program_id_hex(value).ok())
            .filter(|value| !value.is_empty());
        self.entries.iter().filter(move |entry| {
            summary_program_id
                .as_deref()
                .is_some_and(|program_id| entry.program_id_hex == program_id)
        })
    }

    fn selected_transaction_decode(
        &self,
        summary: &TransactionSummary,
    ) -> Option<(&'a RegisteredIdlEntry, TransactionIdlInspectionReport)> {
        let mut partial = None;
        for entry in self.matching_entries(summary) {
            let Ok(report) = inspect_transaction_summary_with_idl(summary, &entry.json) else {
                continue;
            };
            if let Some(decoded) = &report.decoded_instruction {
                if decoded.decode_error.is_none() && decoded.remaining_words.is_empty() {
                    return Some((entry, report));
                }
                if partial.is_none() {
                    partial = Some((entry, report));
                }
            }
        }
        partial
    }
}
