use anyhow::{Context as _, Result};
use serde_json::Value;

use super::{
    AccountTransactionSummary, TransactionIdlInspectionReport, TransactionSummary,
    TransactionTraceReport, trace_transaction_summary_with_idl,
};
use crate::{
    normalize_program_id_hex,
    program_decode::{
        ProgramDecodeCandidate, SelectedDecodeEvidence, resolve_transaction_decode_session,
    },
    support::state_store::RegisteredIdlEntry,
};

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
        self.selected_transaction_decode(summary)
            .map(|(_, report)| report)
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
        let candidate_entries = self
            .matching_entries(summary)
            .map(|entry| (entry, program_decode_candidate_from_registered_entry(entry)))
            .collect::<Vec<_>>();
        let candidates = candidate_entries
            .iter()
            .map(|(_, candidate)| candidate.clone())
            .collect::<Vec<_>>();
        let session = resolve_transaction_decode_session(summary, &candidates);
        let selection = session.selected.or(session.partial)?;
        let entry = candidate_entries
            .iter()
            .find(|(_, candidate)| candidate_matches_evidence(candidate, &selection.evidence))
            .map(|(entry, _)| *entry)?;
        Some((entry, selection.report))
    }
}

fn candidate_matches_evidence(
    candidate: &ProgramDecodeCandidate,
    evidence: &SelectedDecodeEvidence,
) -> bool {
    let evidence_key = evidence.key.trim();
    if !evidence_key.is_empty() {
        return candidate.key == evidence_key;
    }
    candidate.name == evidence.name
        && candidate.program_id_hex == evidence.program_id_hex
        && candidate.source == evidence.source
}

fn program_decode_candidate_from_registered_entry(
    entry: &RegisteredIdlEntry,
) -> ProgramDecodeCandidate {
    ProgramDecodeCandidate {
        key: entry.key.clone().unwrap_or_default(),
        name: entry.name.clone().unwrap_or_default(),
        program_id_hex: entry.program_id_hex.clone(),
        json: entry.json.clone(),
        account_type: None,
        source: entry.source.clone(),
        cached: false,
        shared: entry.source.as_deref() == Some("shared"),
        owner_matched: false,
    }
}
