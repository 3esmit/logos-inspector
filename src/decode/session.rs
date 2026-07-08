use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::{AccountIdlDecodeReport, decode_account_data_hex_with_idl};
use crate::lez::{
    TransactionIdlInspectionReport, TransactionSummary, inspect_transaction_summary_with_idl,
};

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
    fn evidence(&self, account_type: Option<String>) -> SelectedDecodeEvidence {
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

struct ProgramDecodeSession {
    candidates: Vec<ProgramDecodeCandidate>,
}

impl ProgramDecodeSession {
    fn new(candidates: &[ProgramDecodeCandidate]) -> Self {
        Self {
            candidates: unique_candidates(candidates),
        }
    }

    fn resolve_account(
        &self,
        account_id: Option<&str>,
        data_hex: &str,
    ) -> ResolvedAccountDecodeSession {
        let mut partial = None;
        let mut first_error = None;
        for candidate in &self.candidates {
            let account_type = candidate
                .account_type
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty());
            match decode_account_data_hex_with_idl(
                &candidate.json,
                account_type,
                data_hex,
                account_id,
            ) {
                Ok(report) => {
                    let selection = AccountDecodeSelection {
                        evidence: candidate.evidence(Some(report.account_type.clone())),
                        report,
                    };
                    if selection.report.consumed_bytes == selection.report.total_bytes
                        && selection.report.remaining_bytes == 0
                    {
                        return ResolvedAccountDecodeSession {
                            selected: Some(selection),
                            partial,
                            first_error,
                        };
                    }
                    if partial.is_none() {
                        partial = Some(selection);
                    }
                }
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error.to_string());
                    }
                }
            }
        }
        ResolvedAccountDecodeSession {
            selected: None,
            partial,
            first_error,
        }
    }

    fn resolve_transaction(
        &self,
        summary: &TransactionSummary,
    ) -> ResolvedTransactionDecodeSession {
        let mut partial = None;
        for candidate in &self.candidates {
            let Ok(report) = inspect_transaction_summary_with_idl(summary, &candidate.json) else {
                continue;
            };
            let is_full = match report.decoded_instruction.as_ref() {
                Some(decoded) => {
                    decoded.decode_error.is_none() && decoded.remaining_words.is_empty()
                }
                None => continue,
            };
            let selection = TransactionDecodeSelection {
                evidence: candidate.evidence(None),
                report,
            };
            if is_full {
                return ResolvedTransactionDecodeSession {
                    selected: Some(selection),
                    partial,
                };
            }
            if partial.is_none() {
                partial = Some(selection);
            }
        }
        ResolvedTransactionDecodeSession {
            selected: None,
            partial,
        }
    }

    #[cfg(test)]
    fn candidate_count(&self) -> usize {
        self.candidates.len()
    }
}

fn unique_candidates(candidates: &[ProgramDecodeCandidate]) -> Vec<ProgramDecodeCandidate> {
    let mut seen = HashSet::new();
    candidates
        .iter()
        .filter(|candidate| seen.insert(candidate_identity(candidate)))
        .cloned()
        .collect()
}

fn candidate_identity(candidate: &ProgramDecodeCandidate) -> String {
    let key = candidate.key.trim();
    if !key.is_empty() {
        return format!("key:{key}");
    }
    let program_id = candidate.program_id_hex.trim();
    if !program_id.is_empty() {
        let account_type = candidate.account_type.as_deref().unwrap_or("").trim();
        return format!(
            "program:{program_id}:{}:{account_type}",
            candidate.name.trim()
        );
    }
    format!("json:{}", candidate.json.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        key: &str,
        name: &str,
        program_id_hex: &str,
        json: &str,
    ) -> ProgramDecodeCandidate {
        ProgramDecodeCandidate {
            key: key.to_owned(),
            name: name.to_owned(),
            program_id_hex: program_id_hex.to_owned(),
            json: json.to_owned(),
            account_type: None,
            source: None,
        }
    }

    #[test]
    fn session_dedupes_candidates_before_attempting_decode() {
        let candidates = [
            candidate("a", "A", "01", "{}"),
            candidate("a", "A duplicate", "02", "{}"),
            candidate("", "B", "03", "{\"b\":true}"),
            candidate("", "B", "03", "{\"b\":true}"),
            candidate("", "", "", "{\"fallback\":true}"),
            candidate("", "", "", "{\"fallback\":true}"),
        ];

        let session = ProgramDecodeSession::new(&candidates);

        assert_eq!(session.candidate_count(), 3);
    }
}
