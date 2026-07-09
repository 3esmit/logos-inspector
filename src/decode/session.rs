use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::{
    AccountIdlDecodeReport, InstructionDecodeReport, decode_account_data_hex_with_idl,
    decode_instruction_words_with_idl,
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
    #[serde(default)]
    pub cached: bool,
    #[serde(default)]
    pub shared: bool,
    #[serde(default, alias = "owner_matched")]
    pub owner_matched: bool,
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct TransactionDecodeInput {
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub program_id_hex: Option<String>,
    #[serde(default)]
    pub account_ids: Vec<String>,
    #[serde(default)]
    pub nonces: Vec<String>,
    #[serde(default)]
    pub instruction_data: Vec<u32>,
    #[serde(default)]
    pub bytecode_len: Option<usize>,
    #[serde(default)]
    pub raw_signature_valid: Option<bool>,
    #[serde(default)]
    pub message_prehash: Option<String>,
    #[serde(default)]
    pub prehash_signature_valid: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransactionDecodeInspectionReport {
    pub hash: String,
    pub kind: String,
    pub sections: Vec<TransactionDecodeInspectionSection>,
    pub raw_summary: TransactionDecodeInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransactionDecodeInspectionSection {
    pub title: String,
    pub rows: Vec<TransactionDecodeInspectionRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransactionDecodeInspectionRow {
    pub label: String,
    pub index: Option<usize>,
    pub value: String,
    pub decimal: Option<String>,
    pub hex: Option<String>,
    pub base58: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecodeEnrichmentReport {
    pub status: String,
    pub provenance: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransactionDecodeReport {
    pub inspection: TransactionDecodeInspectionReport,
    pub decoded_instruction: Option<InstructionDecodeReport>,
    pub decode_enrichment: DecodeEnrichmentReport,
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
    pub report: TransactionDecodeReport,
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
    input: &TransactionDecodeInput,
    candidates: &[ProgramDecodeCandidate],
) -> ResolvedTransactionDecodeSession {
    ProgramDecodeSession::new(candidates).resolve_transaction(input)
}

pub fn decode_transaction_input_with_idl(
    input: &TransactionDecodeInput,
    idl_json: &str,
) -> anyhow::Result<TransactionDecodeReport> {
    let inspection = inspect_transaction_decode_input(input);
    let decoded_instruction = if input.kind == "Public" && !input.instruction_data.is_empty() {
        input
            .program_id_hex
            .as_deref()
            .map(|program_id| {
                decode_instruction_words_with_idl(
                    idl_json,
                    program_id,
                    &input.instruction_data,
                    &input.account_ids,
                )
            })
            .transpose()?
    } else {
        None
    };
    let status = if decoded_instruction.is_some() {
        "applied"
    } else {
        "skipped"
    };
    Ok(TransactionDecodeReport {
        inspection,
        decoded_instruction,
        decode_enrichment: DecodeEnrichmentReport {
            status: status.to_owned(),
            provenance: "program_decode_static".to_owned(),
            source: Some("explicit_idl".to_owned()),
            error: None,
        },
    })
}

#[must_use]
pub fn inspect_transaction_decode_input(
    input: &TransactionDecodeInput,
) -> TransactionDecodeInspectionReport {
    let mut sections = Vec::new();
    if let Some(program_id) = input
        .program_id_hex
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        sections.push(TransactionDecodeInspectionSection {
            title: "Program".to_owned(),
            rows: vec![TransactionDecodeInspectionRow {
                label: "program_id".to_owned(),
                index: None,
                value: program_id.to_owned(),
                decimal: None,
                hex: Some(program_id.to_owned()),
                base58: None,
            }],
        });
    }
    if !input.account_ids.is_empty() {
        sections.push(TransactionDecodeInspectionSection {
            title: "Accounts".to_owned(),
            rows: input
                .account_ids
                .iter()
                .enumerate()
                .map(|(index, account)| TransactionDecodeInspectionRow {
                    label: "account".to_owned(),
                    index: Some(index),
                    value: account.clone(),
                    decimal: None,
                    hex: None,
                    base58: None,
                })
                .collect(),
        });
    }
    if !input.instruction_data.is_empty() {
        sections.push(TransactionDecodeInspectionSection {
            title: "Instruction words".to_owned(),
            rows: input
                .instruction_data
                .iter()
                .enumerate()
                .map(|(index, word)| TransactionDecodeInspectionRow {
                    label: "instruction_word".to_owned(),
                    index: Some(index),
                    value: word.to_string(),
                    decimal: Some(word.to_string()),
                    hex: Some(format!("0x{word:08x}")),
                    base58: None,
                })
                .collect(),
        });
    }
    if let Some(bytecode_len) = input.bytecode_len {
        sections.push(TransactionDecodeInspectionSection {
            title: "Program deployment".to_owned(),
            rows: vec![TransactionDecodeInspectionRow {
                label: "bytecode_len".to_owned(),
                index: None,
                value: bytecode_len.to_string(),
                decimal: Some(bytecode_len.to_string()),
                hex: None,
                base58: None,
            }],
        });
    }
    TransactionDecodeInspectionReport {
        hash: input.hash.clone(),
        kind: input.kind.clone(),
        sections,
        raw_summary: input.clone(),
    }
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
                    if account_decode_is_full(&selection.report) {
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
        input: &TransactionDecodeInput,
    ) -> ResolvedTransactionDecodeSession {
        let mut partial = None;
        for candidate in &self.candidates {
            let Ok(report) = decode_transaction_input_with_idl(input, &candidate.json) else {
                continue;
            };
            if report.decoded_instruction.is_none() {
                continue;
            }
            let is_full = transaction_decode_is_full(&report);
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

fn account_decode_is_full(report: &AccountIdlDecodeReport) -> bool {
    report.consumed_bytes == report.total_bytes && report.remaining_bytes == 0
}

fn transaction_decode_is_full(report: &TransactionDecodeReport) -> bool {
    report
        .decoded_instruction
        .as_ref()
        .is_some_and(|decoded| decoded.decode_error.is_none() && decoded.remaining_words.is_empty())
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
    use serde_json::json;

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
            cached: false,
            shared: false,
            owner_matched: false,
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

    #[test]
    fn account_full_verdict_requires_all_bytes_consumed() {
        assert!(account_decode_is_full(&account_report(4, 4, 0)));
        assert!(!account_decode_is_full(&account_report(3, 4, 1)));
    }

    #[test]
    fn transaction_full_verdict_requires_decoded_instruction_without_remainder() {
        assert!(transaction_decode_is_full(&transaction_report(Some(
            decoded_instruction(None, Vec::new())
        ))));
        assert!(!transaction_decode_is_full(&transaction_report(Some(
            decoded_instruction(Some("unknown variant"), Vec::new())
        ))));
        assert!(!transaction_decode_is_full(&transaction_report(Some(
            decoded_instruction(None, vec![7])
        ))));
        assert!(!transaction_decode_is_full(&transaction_report(None)));
    }

    #[test]
    fn transaction_decode_input_with_idl_decodes_without_runtime_context() {
        let input = TransactionDecodeInput {
            hash: "tx".to_owned(),
            kind: "Public".to_owned(),
            program_id_hex: Some("program".to_owned()),
            account_ids: vec!["acct-a".to_owned()],
            nonces: Vec::new(),
            instruction_data: vec![0, 9],
            bytecode_len: None,
            raw_signature_valid: None,
            message_prehash: None,
            prehash_signature_valid: None,
        };
        let idl = r#"{
            "name": "test_program",
            "instructions": [
                {
                    "name": "set_value",
                    "accounts": [{ "name": "target" }],
                    "args": [{ "name": "value", "type": "u32" }]
                }
            ]
        }"#;

        let report = decode_transaction_input_with_idl(&input, idl);

        assert!(report.is_ok(), "{report:?}");
        let Ok(report) = report else {
            return;
        };
        assert_eq!(report.inspection.raw_summary, input);
        assert_eq!(report.decode_enrichment.status, "applied");
        let decoded = report.decoded_instruction.as_ref();
        assert!(decoded.is_some(), "missing instruction decode");
        let Some(decoded) = decoded else {
            return;
        };
        assert_eq!(decoded.instruction, "set_value");
        assert_eq!(
            decoded.accounts.first().map(|row| row.path.as_str()),
            Some("target")
        );
    }

    fn account_report(
        consumed_bytes: usize,
        total_bytes: usize,
        remaining_bytes: usize,
    ) -> AccountIdlDecodeReport {
        AccountIdlDecodeReport {
            account_id: None,
            account_type: "Account".to_owned(),
            consumed_bytes,
            total_bytes,
            remaining_bytes,
            remaining_data_hex: None,
            decoded: json!({}),
            rows: Vec::new(),
        }
    }

    fn transaction_report(
        decoded_instruction: Option<InstructionDecodeReport>,
    ) -> TransactionDecodeReport {
        TransactionDecodeReport {
            inspection: TransactionDecodeInspectionReport {
                hash: "tx".to_owned(),
                kind: "Public".to_owned(),
                sections: Vec::<TransactionDecodeInspectionSection>::new(),
                raw_summary: TransactionDecodeInput {
                    hash: "tx".to_owned(),
                    kind: "Public".to_owned(),
                    program_id_hex: None,
                    account_ids: Vec::new(),
                    nonces: Vec::new(),
                    instruction_data: Vec::new(),
                    bytecode_len: None,
                    raw_signature_valid: None,
                    message_prehash: None,
                    prehash_signature_valid: None,
                },
            },
            decoded_instruction,
            decode_enrichment: DecodeEnrichmentReport {
                status: "applied".to_owned(),
                provenance: "program_decode_static".to_owned(),
                source: Some("test".to_owned()),
                error: None,
            },
        }
    }

    fn decoded_instruction(
        decode_error: Option<&str>,
        remaining_words: Vec<u32>,
    ) -> InstructionDecodeReport {
        InstructionDecodeReport {
            program_id: "program".to_owned(),
            idl_name: Some("IDL".to_owned()),
            instruction: "transfer".to_owned(),
            variant_index: 0,
            accounts: Vec::new(),
            args: Vec::new(),
            decode_error: decode_error.map(str::to_owned),
            remaining_words,
        }
    }
}
