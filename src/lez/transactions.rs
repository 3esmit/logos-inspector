use anyhow::{Context as _, Result};
use common::transaction::LeeTransaction;
use k256::ecdsa::signature::hazmat::PrehashVerifier as _;
use lee::{PublicKey, program::Program, public_transaction::Message as PublicMessage};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::programs::{program_id_base58_from_hex, program_id_hex};
use crate::{
    InstructionDecodeReport,
    program_decode::{
        DecodeEnrichmentReport, TransactionDecodeInput, TransactionDecodeReport,
        decode_transaction_input_with_idl,
    },
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransactionSummary {
    pub hash: String,
    pub kind: String,
    pub program_id_hex: Option<String>,
    pub account_ids: Vec<String>,
    pub nonces: Vec<String>,
    pub instruction_data: Vec<u32>,
    pub bytecode_len: Option<usize>,
    pub raw_signature_valid: Option<bool>,
    pub message_prehash: Option<String>,
    pub prehash_signature_valid: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransactionInspectionReport {
    pub hash: String,
    pub kind: String,
    pub sections: Vec<TransactionInspectionSection>,
    pub raw_summary: TransactionSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransactionIdlInspectionReport {
    pub inspection: TransactionInspectionReport,
    pub decoded_instruction: Option<InstructionDecodeReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decode_enrichment: Option<DecodeEnrichmentReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransactionTraceReport {
    pub hash: String,
    pub kind: String,
    pub source: String,
    pub capabilities: Vec<String>,
    pub limitations: Vec<String>,
    pub steps: Vec<TransactionTraceStep>,
    pub inspection: TransactionInspectionReport,
    pub decoded_instruction: Option<InstructionDecodeReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransactionTraceStep {
    pub index: usize,
    pub phase: String,
    pub label: String,
    pub status: Option<String>,
    pub severity: Option<String>,
    pub details: Vec<String>,
    pub refs: Option<TransactionTraceRefs>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct TransactionTraceRefs {
    pub program_id_hex: Option<String>,
    pub program_id_base58: Option<String>,
    pub account_id: Option<String>,
    pub instruction_word_index: Option<usize>,
    pub decode_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransactionInspectionSection {
    pub title: String,
    pub rows: Vec<TransactionInspectionRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransactionInspectionRow {
    pub label: String,
    pub index: Option<usize>,
    pub value: String,
    pub decimal: Option<String>,
    pub hex: Option<String>,
    pub base58: Option<String>,
}

#[must_use]
pub fn summarize_transaction(tx: &LeeTransaction) -> TransactionSummary {
    match tx {
        LeeTransaction::ProgramDeployment(tx) => {
            let bytecode = tx.clone().into_message().into_bytecode();
            let program_id_hex = Program::new(bytecode.clone().into())
                .ok()
                .map(|program| program_id_hex(program.id()));
            TransactionSummary {
                hash: hex::encode(tx.hash()),
                kind: "ProgramDeployment".to_owned(),
                program_id_hex,
                account_ids: vec![],
                nonces: vec![],
                instruction_data: vec![],
                bytecode_len: Some(bytecode.len()),
                raw_signature_valid: None,
                message_prehash: None,
                prehash_signature_valid: None,
            }
        }
        LeeTransaction::Public(tx) => {
            let prehash = public_message_prehash(tx.message()).ok();
            TransactionSummary {
                hash: hex::encode(tx.hash()),
                kind: "Public".to_owned(),
                program_id_hex: Some(program_id_hex(tx.message().program_id)),
                account_ids: tx
                    .message()
                    .account_ids
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                nonces: tx
                    .message()
                    .nonces
                    .iter()
                    .map(|nonce| nonce.0.to_string())
                    .collect(),
                instruction_data: tx.message().instruction_data.clone(),
                bytecode_len: None,
                raw_signature_valid: Some(tx.witness_set().is_valid_for(tx.message())),
                message_prehash: prehash.map(hex::encode),
                prehash_signature_valid: public_message_prehash(tx.message())
                    .ok()
                    .map(|hash| prehash_witness_set_is_valid(tx.witness_set(), &hash)),
            }
        }
        LeeTransaction::PrivacyPreserving(tx) => TransactionSummary {
            hash: hex::encode(tx.hash()),
            kind: "PrivacyPreserving".to_owned(),
            program_id_hex: None,
            account_ids: vec![],
            nonces: vec![],
            instruction_data: vec![],
            bytecode_len: None,
            raw_signature_valid: None,
            message_prehash: None,
            prehash_signature_valid: None,
        },
    }
}

#[must_use]
pub fn inspect_transaction_summary(summary: &TransactionSummary) -> TransactionInspectionReport {
    let mut sections = Vec::with_capacity(6);
    sections.push(TransactionInspectionSection {
        title: "Summary".to_owned(),
        rows: vec![
            inspection_text_row("kind", summary.kind.clone()),
            inspection_text_row("hash", summary.hash.clone()),
        ],
    });

    let program_id_base58 = summary
        .program_id_hex
        .as_deref()
        .and_then(program_id_base58_from_hex);
    let mut program_rows = Vec::new();
    if summary.program_id_hex.is_some() || program_id_base58.is_some() {
        let value = program_id_base58
            .clone()
            .or_else(|| summary.program_id_hex.clone())
            .unwrap_or_default();
        program_rows.push(TransactionInspectionRow {
            label: "program_id".to_owned(),
            index: None,
            value,
            decimal: None,
            hex: summary.program_id_hex.clone(),
            base58: program_id_base58,
        });
    }
    if let Some(bytecode_len) = summary.bytecode_len {
        program_rows.push(TransactionInspectionRow {
            label: "deployment_bytecode_len".to_owned(),
            index: None,
            value: format!("{bytecode_len} bytes"),
            decimal: Some(bytecode_len.to_string()),
            hex: None,
            base58: None,
        });
    }
    if !program_rows.is_empty() {
        sections.push(TransactionInspectionSection {
            title: "Program".to_owned(),
            rows: program_rows,
        });
    }

    let account_rows = summary
        .account_ids
        .iter()
        .enumerate()
        .map(|(index, account_id)| inspection_indexed_text_row("account", index, account_id))
        .collect::<Vec<_>>();
    if !account_rows.is_empty() {
        sections.push(TransactionInspectionSection {
            title: "Accounts".to_owned(),
            rows: account_rows,
        });
    }

    let nonce_rows = summary
        .nonces
        .iter()
        .enumerate()
        .map(|(index, nonce)| TransactionInspectionRow {
            label: "nonce".to_owned(),
            index: Some(index),
            value: nonce.clone(),
            decimal: Some(nonce.clone()),
            hex: None,
            base58: None,
        })
        .collect::<Vec<_>>();
    if !nonce_rows.is_empty() {
        sections.push(TransactionInspectionSection {
            title: "Nonces".to_owned(),
            rows: nonce_rows,
        });
    }

    let instruction_rows = summary
        .instruction_data
        .iter()
        .enumerate()
        .map(|(index, word)| instruction_word_row(index, *word))
        .collect::<Vec<_>>();
    if !instruction_rows.is_empty() {
        sections.push(TransactionInspectionSection {
            title: "Instruction words".to_owned(),
            rows: instruction_rows,
        });
    }

    let mut validation_rows = Vec::new();
    if let Some(valid) = summary.raw_signature_valid {
        validation_rows.push(inspection_validity_row("raw_signature_valid", valid));
    }
    if let Some(prehash) = &summary.message_prehash {
        validation_rows.push(TransactionInspectionRow {
            label: "message_prehash".to_owned(),
            index: None,
            value: prehash.clone(),
            decimal: None,
            hex: Some(format!("0x{prehash}")),
            base58: None,
        });
    }
    if let Some(valid) = summary.prehash_signature_valid {
        validation_rows.push(inspection_validity_row("prehash_signature_valid", valid));
    }
    if !validation_rows.is_empty() {
        sections.push(TransactionInspectionSection {
            title: "Validation".to_owned(),
            rows: validation_rows,
        });
    }

    TransactionInspectionReport {
        hash: summary.hash.clone(),
        kind: summary.kind.clone(),
        sections,
        raw_summary: summary.clone(),
    }
}

pub fn inspect_transaction_summary_with_idl(
    summary: &TransactionSummary,
    idl_json: &str,
) -> Result<TransactionIdlInspectionReport> {
    LezTransactionDecodeAdapter::new(summary).inspect_with_idl(idl_json)
}

pub(crate) fn inspect_transaction_summary_with_optional_idl_decode(
    summary: &TransactionSummary,
    idl_json: &str,
    source: &str,
) -> TransactionIdlInspectionReport {
    LezTransactionDecodeAdapter::new(summary).inspect_with_optional_idl_decode(idl_json, source)
}

pub(crate) fn transaction_decode_input_from_summary(
    summary: &TransactionSummary,
) -> TransactionDecodeInput {
    LezTransactionDecodeAdapter::new(summary).decode_input()
}

pub(crate) struct LezTransactionDecodeAdapter<'a> {
    summary: &'a TransactionSummary,
}

impl<'a> LezTransactionDecodeAdapter<'a> {
    fn new(summary: &'a TransactionSummary) -> Self {
        Self { summary }
    }

    fn decode_input(&self) -> TransactionDecodeInput {
        TransactionDecodeInput {
            hash: self.summary.hash.clone(),
            kind: self.summary.kind.clone(),
            program_id_hex: self.summary.program_id_hex.clone(),
            account_ids: self.summary.account_ids.clone(),
            nonces: self.summary.nonces.clone(),
            instruction_data: self.summary.instruction_data.clone(),
            bytecode_len: self.summary.bytecode_len,
            raw_signature_valid: self.summary.raw_signature_valid,
            message_prehash: self.summary.message_prehash.clone(),
            prehash_signature_valid: self.summary.prehash_signature_valid,
        }
    }

    fn inspect_with_idl(&self, idl_json: &str) -> Result<TransactionIdlInspectionReport> {
        let decode_report = decode_transaction_input_with_idl(&self.decode_input(), idl_json)?;
        Ok(self.report_from_decode(decode_report))
    }

    fn inspect_with_optional_idl_decode(
        &self,
        idl_json: &str,
        source: &str,
    ) -> TransactionIdlInspectionReport {
        match decode_transaction_input_with_idl(&self.decode_input(), idl_json) {
            Ok(mut report) => {
                report.decode_enrichment.source = Some(source.to_owned());
                self.report_from_decode(report)
            }
            Err(error) => TransactionIdlInspectionReport {
                inspection: inspect_transaction_summary(self.summary),
                decoded_instruction: None,
                decode_enrichment: Some(DecodeEnrichmentReport {
                    status: "failed".to_owned(),
                    provenance: "program_decode_static".to_owned(),
                    source: Some(source.to_owned()),
                    error: Some(format!("{error:#}")),
                }),
            },
        }
    }

    fn report_from_decode(
        &self,
        report: TransactionDecodeReport,
    ) -> TransactionIdlInspectionReport {
        TransactionIdlInspectionReport {
            inspection: inspect_transaction_summary(self.summary),
            decoded_instruction: report.decoded_instruction,
            decode_enrichment: Some(report.decode_enrichment),
        }
    }
}

#[must_use]
pub fn trace_transaction_summary(summary: &TransactionSummary) -> TransactionTraceReport {
    let inspection = inspect_transaction_summary(summary);
    build_transaction_trace_report(summary, inspection, None, false, None)
}

pub fn trace_transaction_summary_with_idl(
    summary: &TransactionSummary,
    idl_json: &str,
) -> Result<TransactionTraceReport> {
    let inspection = inspect_transaction_summary(summary);
    let (decoded_instruction, decode_error) = if summary.kind == "Public"
        && !summary.instruction_data.is_empty()
        && summary.program_id_hex.is_some()
    {
        match inspect_transaction_summary_with_idl(summary, idl_json) {
            Ok(idl_report) => (idl_report.decoded_instruction, None),
            Err(err) => (None, Some(format!("{err:#}"))),
        }
    } else {
        (None, None)
    };

    Ok(build_transaction_trace_report(
        summary,
        inspection,
        decoded_instruction,
        true,
        decode_error,
    ))
}

pub(crate) fn inspect_transaction(tx: &LeeTransaction) -> TransactionInspectionReport {
    let summary = summarize_transaction(tx);
    inspect_transaction_summary(&summary)
}

fn build_transaction_trace_report(
    summary: &TransactionSummary,
    inspection: TransactionInspectionReport,
    decoded_instruction: Option<InstructionDecodeReport>,
    used_idl: bool,
    decode_error: Option<String>,
) -> TransactionTraceReport {
    let program_id_base58 = summary
        .program_id_hex
        .as_deref()
        .and_then(program_id_base58_from_hex);
    let mut capabilities = vec![
        "ordered best-effort timeline from sequencer transaction summary fields".to_owned(),
        "includes reproducible human inspection artifact for every trace".to_owned(),
    ];
    if summary.raw_signature_valid.is_some()
        || summary.message_prehash.is_some()
        || summary.prehash_signature_valid.is_some()
    {
        capabilities.push("surfaces signature and prehash validation when available".to_owned());
    }
    if used_idl {
        capabilities
            .push("can attach user-supplied IDL instruction decode when compatible".to_owned());
    }

    let mut limitations = vec![
        "no runtime execution trace, nested calls, logs, state diffs, or gas/resource metrics exposed by current APIs".to_owned(),
    ];
    if summary.kind != "Public" {
        limitations.push(format!(
            "{} transactions currently expose only summary-level fields",
            summary.kind
        ));
    } else if let Some(error) = &decode_error {
        limitations.push(format!(
            "IDL decode failed; raw instruction trace preserved: {error}"
        ));
    } else if !used_idl {
        limitations.push(
            "instruction accounts and args stay raw unless caller supplies compatible IDL JSON"
                .to_owned(),
        );
    }

    let mut steps = Vec::new();
    push_trace_step(
        &mut steps,
        "summary",
        "transaction summary loaded",
        Some("observed"),
        None,
        vec![
            format!("hash {}", summary.hash),
            format!("kind {}", summary.kind),
            "source sequencer transaction summary".to_owned(),
        ],
        None,
    );

    if let Some(valid) = summary.raw_signature_valid {
        push_trace_step(
            &mut steps,
            "validation",
            "raw witness validation",
            Some(if valid { "valid" } else { "invalid" }),
            (!valid).then_some("warning"),
            vec!["witness_set().is_valid_for(message())".to_owned()],
            None,
        );
    }
    if let Some(prehash) = &summary.message_prehash {
        push_trace_step(
            &mut steps,
            "validation",
            "message prehash derived",
            Some("derived"),
            None,
            vec![format!("sha256(prefixed Borsh public message) 0x{prehash}")],
            None,
        );
    }
    if let Some(valid) = summary.prehash_signature_valid {
        push_trace_step(
            &mut steps,
            "validation",
            "prehash witness validation",
            Some(if valid { "valid" } else { "invalid" }),
            (!valid).then_some("warning"),
            vec!["signature verified against message prehash".to_owned()],
            None,
        );
    }

    match summary.kind.as_str() {
        "ProgramDeployment" => {
            let mut details = Vec::new();
            if let Some(program_id_hex) = &summary.program_id_hex {
                details.push(format!("derived program id {program_id_hex}"));
            }
            if let Some(bytecode_len) = summary.bytecode_len {
                details.push(format!("bytecode length {bytecode_len} bytes"));
            }
            push_trace_step(
                &mut steps,
                "program",
                "program deployment payload",
                Some("observed"),
                None,
                details,
                trace_refs(
                    summary.program_id_hex.clone(),
                    program_id_base58.clone(),
                    None,
                    None,
                    None,
                ),
            );
        }
        "Public" => {
            let mut details = Vec::new();
            if let Some(program_id_hex) = &summary.program_id_hex {
                details.push(format!("program id {program_id_hex}"));
            }
            details.push(format!("accounts {}", summary.account_ids.len()));
            details.push(format!("nonces {}", summary.nonces.len()));
            details.push(format!(
                "instruction words {}",
                summary.instruction_data.len()
            ));
            push_trace_step(
                &mut steps,
                "program",
                "public program invocation",
                Some("observed"),
                None,
                details,
                trace_refs(
                    summary.program_id_hex.clone(),
                    program_id_base58.clone(),
                    None,
                    None,
                    None,
                ),
            );
        }
        other => {
            push_trace_step(
                &mut steps,
                "program",
                "transaction payload not expanded",
                Some("limited"),
                None,
                vec![format!(
                    "{other} summary has no program/account/instruction fields"
                )],
                None,
            );
        }
    }

    for (index, account_id) in summary.account_ids.iter().enumerate() {
        push_trace_step(
            &mut steps,
            "account",
            "account reference",
            Some("observed"),
            None,
            vec![format!("account[{index}] {account_id}")],
            trace_refs(None, None, Some(account_id.clone()), None, None),
        );
    }

    for (index, nonce) in summary.nonces.iter().enumerate() {
        push_trace_step(
            &mut steps,
            "nonce",
            "nonce reference",
            Some("observed"),
            None,
            vec![format!("nonce[{index}] {nonce}")],
            None,
        );
    }

    for (index, word) in summary.instruction_data.iter().enumerate() {
        push_trace_step(
            &mut steps,
            "instruction",
            "instruction word",
            Some("observed"),
            None,
            vec![
                format!("word[{index}] decimal {word}"),
                format!("word[{index}] hex 0x{word:08x}"),
            ],
            trace_refs(None, None, None, Some(index), None),
        );
    }

    if let Some(error) = &decode_error {
        push_trace_step(
            &mut steps,
            "decode",
            "IDL instruction decode unavailable",
            Some("error"),
            Some("warning"),
            vec![
                "raw instruction timeline preserved".to_owned(),
                error.clone(),
            ],
            trace_refs(
                summary.program_id_hex.clone(),
                program_id_base58.clone(),
                None,
                None,
                None,
            ),
        );
    }

    if let Some(decoded_instruction) = &decoded_instruction {
        let mut details = vec![
            format!("instruction {}", decoded_instruction.instruction),
            format!("variant {}", decoded_instruction.variant_index),
        ];
        if let Some(idl_name) = &decoded_instruction.idl_name {
            details.push(format!("idl {}", idl_name));
        }
        if !decoded_instruction.remaining_words.is_empty() {
            details.push(format!(
                "remaining words {}",
                decoded_instruction.remaining_words.len()
            ));
        }
        push_trace_step(
            &mut steps,
            "decode",
            "IDL instruction decode",
            Some("decoded"),
            None,
            details,
            trace_refs(
                Some(decoded_instruction.program_id.clone()),
                program_id_base58.clone(),
                None,
                None,
                None,
            ),
        );

        for field in &decoded_instruction.accounts {
            push_trace_step(
                &mut steps,
                "decode",
                "decoded instruction account",
                Some("decoded"),
                None,
                vec![format!("{} {}", field.path, field.value)],
                trace_refs(
                    None,
                    None,
                    (!is_placeholder_account_value(&field.value)).then(|| field.value.clone()),
                    None,
                    Some(field.path.clone()),
                ),
            );
        }

        for field in &decoded_instruction.args {
            push_trace_step(
                &mut steps,
                "decode",
                "decoded instruction arg",
                Some("decoded"),
                None,
                vec![format!("{} {}", field.path, field.value)],
                trace_refs(None, None, None, None, Some(field.path.clone())),
            );
        }

        if !decoded_instruction.remaining_words.is_empty() {
            push_trace_step(
                &mut steps,
                "decode",
                "remaining instruction words",
                Some("observed"),
                Some("warning"),
                vec![format!("{:?}", decoded_instruction.remaining_words)],
                None,
            );
        }
    }

    TransactionTraceReport {
        hash: summary.hash.clone(),
        kind: summary.kind.clone(),
        source: if used_idl {
            "sequencer transaction summary + user supplied IDL".to_owned()
        } else {
            "sequencer transaction summary".to_owned()
        },
        capabilities,
        limitations,
        steps,
        inspection,
        decoded_instruction,
    }
}

fn push_trace_step(
    steps: &mut Vec<TransactionTraceStep>,
    phase: &str,
    label: &str,
    status: Option<&str>,
    severity: Option<&str>,
    details: Vec<String>,
    refs: Option<TransactionTraceRefs>,
) {
    steps.push(TransactionTraceStep {
        index: steps.len(),
        phase: phase.to_owned(),
        label: label.to_owned(),
        status: status.map(ToOwned::to_owned),
        severity: severity.map(ToOwned::to_owned),
        details,
        refs,
    });
}

fn trace_refs(
    program_id_hex: Option<String>,
    program_id_base58: Option<String>,
    account_id: Option<String>,
    instruction_word_index: Option<usize>,
    decode_path: Option<String>,
) -> Option<TransactionTraceRefs> {
    let refs = TransactionTraceRefs {
        program_id_hex,
        program_id_base58,
        account_id,
        instruction_word_index,
        decode_path,
    };
    (refs.program_id_hex.is_some()
        || refs.program_id_base58.is_some()
        || refs.account_id.is_some()
        || refs.instruction_word_index.is_some()
        || refs.decode_path.is_some())
    .then_some(refs)
}

fn is_placeholder_account_value(value: &str) -> bool {
    value == "-"
}

fn inspection_text_row(label: &str, value: String) -> TransactionInspectionRow {
    TransactionInspectionRow {
        label: label.to_owned(),
        index: None,
        value,
        decimal: None,
        hex: None,
        base58: None,
    }
}

fn inspection_indexed_text_row(
    label: &str,
    index: usize,
    value: impl ToString,
) -> TransactionInspectionRow {
    TransactionInspectionRow {
        label: label.to_owned(),
        index: Some(index),
        value: value.to_string(),
        decimal: None,
        hex: None,
        base58: None,
    }
}

fn inspection_validity_row(label: &str, valid: bool) -> TransactionInspectionRow {
    TransactionInspectionRow {
        label: label.to_owned(),
        index: None,
        value: if valid { "valid" } else { "invalid" }.to_owned(),
        decimal: None,
        hex: None,
        base58: None,
    }
}

fn instruction_word_row(index: usize, word: u32) -> TransactionInspectionRow {
    TransactionInspectionRow {
        label: "instruction_word".to_owned(),
        index: Some(index),
        value: word.to_string(),
        decimal: Some(word.to_string()),
        hex: Some(format!("0x{word:08x}")),
        base58: None,
    }
}

fn public_message_prehash(message: &PublicMessage) -> Result<[u8; 32]> {
    const PREFIX: &[u8; 32] = b"/LEE/v0.3/Message/Public/\x00\x00\x00\x00\x00\x00\x00";

    let message_bytes = borsh::to_vec(message).context("failed to serialize public message")?;
    let mut bytes = Vec::with_capacity(PREFIX.len() + message_bytes.len());
    bytes.extend_from_slice(PREFIX);
    bytes.extend_from_slice(&message_bytes);

    Ok(Sha256::digest(bytes).into())
}

fn prehash_witness_set_is_valid(
    witness_set: &lee::public_transaction::WitnessSet,
    message_hash: &[u8; 32],
) -> bool {
    witness_set
        .signatures_and_public_keys()
        .iter()
        .all(|(signature, public_key)| {
            prehash_signature_is_valid(signature, public_key, message_hash)
        })
}

fn prehash_signature_is_valid(
    signature: &lee::Signature,
    public_key: &PublicKey,
    message_hash: &[u8; 32],
) -> bool {
    let Ok(verifying_key) = k256::schnorr::VerifyingKey::from_bytes(public_key.value()) else {
        return false;
    };
    let Ok(signature) = k256::schnorr::Signature::try_from(signature.value.as_slice()) else {
        return false;
    };

    verifying_key
        .verify_prehash(message_hash, &signature)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn public_summary() -> TransactionSummary {
        TransactionSummary {
            hash: "abcd1234".to_owned(),
            kind: "Public".to_owned(),
            program_id_hex: Some(
                "0100000002000000030000000400000005000000060000000700000008000000".to_owned(),
            ),
            account_ids: vec!["acct-a".to_owned(), "acct-b".to_owned()],
            nonces: vec!["9".to_owned()],
            instruction_data: vec![7, 255],
            bytecode_len: None,
            raw_signature_valid: Some(true),
            message_prehash: Some("feedbeef".to_owned()),
            prehash_signature_valid: Some(false),
        }
    }

    #[test]
    fn instruction_word_row_includes_index_decimal_and_hex() {
        let row = instruction_word_row(2, 255);

        assert_eq!(row.label, "instruction_word");
        assert_eq!(row.index, Some(2));
        assert_eq!(row.value, "255");
        assert_eq!(row.decimal.as_deref(), Some("255"));
        assert_eq!(row.hex.as_deref(), Some("0x000000ff"));
        assert_eq!(row.base58, None);
    }

    #[test]
    fn inspect_transaction_summary_builds_human_sections() {
        let summary = TransactionSummary {
            account_ids: vec!["acct-a".to_owned(), "acct-b".to_owned()],
            nonces: vec!["9".to_owned(), "10".to_owned()],
            bytecode_len: Some(42),
            ..public_summary()
        };

        let report = inspect_transaction_summary(&summary);
        assert_eq!(report.hash, summary.hash);
        assert_eq!(report.kind, summary.kind);
        assert_eq!(report.sections.len(), 6);

        let program_section = report
            .sections
            .iter()
            .find(|section| section.title == "Program");
        assert!(program_section.is_some(), "missing Program section");
        let Some(program_section) = program_section else {
            return;
        };
        let program_row = program_section
            .rows
            .iter()
            .find(|row| row.label == "program_id");
        assert!(program_row.is_some(), "missing program_id row");
        let Some(program_row) = program_row else {
            return;
        };
        assert_eq!(
            program_row.hex.as_deref(),
            summary.program_id_hex.as_deref()
        );
        assert!(program_row.base58.is_some());

        let instruction_section = report
            .sections
            .iter()
            .find(|section| section.title == "Instruction words");
        assert!(
            instruction_section.is_some(),
            "missing Instruction words section"
        );
        let Some(instruction_section) = instruction_section else {
            return;
        };
        let instruction_row = instruction_section.rows.get(1);
        assert!(instruction_row.is_some(), "missing instruction row 1");
        let Some(instruction_row) = instruction_row else {
            return;
        };
        assert_eq!(instruction_row.index, Some(1));
        assert_eq!(instruction_row.decimal.as_deref(), Some("255"));
        assert_eq!(instruction_row.hex.as_deref(), Some("0x000000ff"));

        let validation_section = report
            .sections
            .iter()
            .find(|section| section.title == "Validation");
        assert!(validation_section.is_some(), "missing Validation section");
        let Some(validation_section) = validation_section else {
            return;
        };
        let raw_signature_row = validation_section.rows.first();
        assert!(
            raw_signature_row.is_some(),
            "missing raw signature validation row"
        );
        let Some(raw_signature_row) = raw_signature_row else {
            return;
        };
        let prehash_signature_row = validation_section.rows.get(2);
        assert!(
            prehash_signature_row.is_some(),
            "missing prehash signature validation row"
        );
        let Some(prehash_signature_row) = prehash_signature_row else {
            return;
        };
        assert_eq!(raw_signature_row.value, "valid");
        assert_eq!(prehash_signature_row.value, "invalid");
    }

    #[test]
    fn optional_idl_decode_preserves_transaction_report_on_decode_failure() {
        let summary = public_summary();

        let report =
            inspect_transaction_summary_with_optional_idl_decode(&summary, "{", "registered_idl");

        assert_eq!(report.inspection.hash, summary.hash);
        assert!(report.decoded_instruction.is_none());
        let enrichment = report.decode_enrichment.as_ref();
        assert!(enrichment.is_some(), "missing decode enrichment state");
        let Some(enrichment) = enrichment else {
            return;
        };
        assert_eq!(enrichment.status, "failed");
        assert_eq!(enrichment.provenance, "program_decode_static");
        assert_eq!(enrichment.source.as_deref(), Some("registered_idl"));
        assert!(
            enrichment
                .error
                .as_deref()
                .is_some_and(|error| error.contains("failed to parse IDL JSON")),
            "{enrichment:?}"
        );
    }

    #[test]
    fn trace_transaction_summary_builds_public_validation_timeline() {
        let summary = public_summary();

        let report = trace_transaction_summary(&summary);

        assert_eq!(report.hash, summary.hash);
        assert_eq!(report.kind, summary.kind);
        assert_eq!(report.source, "sequencer transaction summary");
        assert!(
            report
                .limitations
                .iter()
                .any(|item| item.contains("no runtime execution trace")),
            "{report:?}"
        );

        let raw_validation = report
            .steps
            .iter()
            .find(|step| step.label == "raw witness validation");
        assert!(raw_validation.is_some(), "missing raw validation step");
        let Some(raw_validation) = raw_validation else {
            return;
        };
        assert_eq!(raw_validation.phase, "validation");
        assert_eq!(raw_validation.status.as_deref(), Some("valid"));

        let public_program = report
            .steps
            .iter()
            .find(|step| step.label == "public program invocation");
        assert!(public_program.is_some(), "missing public program step");
        let Some(public_program) = public_program else {
            return;
        };
        assert_eq!(
            public_program
                .refs
                .as_ref()
                .and_then(|refs| refs.program_id_hex.as_deref()),
            summary.program_id_hex.as_deref()
        );

        let account_step = report
            .steps
            .iter()
            .find(|step| step.label == "account reference");
        assert!(account_step.is_some(), "missing account reference step");
        let Some(account_step) = account_step else {
            return;
        };
        assert_eq!(
            account_step
                .refs
                .as_ref()
                .and_then(|refs| refs.account_id.as_deref()),
            Some("acct-a")
        );

        let invalid_prehash = report
            .steps
            .iter()
            .find(|step| step.label == "prehash witness validation");
        assert!(invalid_prehash.is_some(), "missing prehash validation step");
        let Some(invalid_prehash) = invalid_prehash else {
            return;
        };
        assert_eq!(invalid_prehash.status.as_deref(), Some("invalid"));
        assert_eq!(invalid_prehash.severity.as_deref(), Some("warning"));
    }

    #[test]
    fn trace_transaction_summary_builds_program_deployment_step() {
        let summary = TransactionSummary {
            hash: "deploy1234".to_owned(),
            kind: "ProgramDeployment".to_owned(),
            program_id_hex: Some(
                "0100000002000000030000000400000005000000060000000700000008000000".to_owned(),
            ),
            account_ids: vec![],
            nonces: vec![],
            instruction_data: vec![],
            bytecode_len: Some(42),
            raw_signature_valid: None,
            message_prehash: None,
            prehash_signature_valid: None,
        };

        let report = trace_transaction_summary(&summary);

        let deployment = report
            .steps
            .iter()
            .find(|step| step.label == "program deployment payload");
        assert!(deployment.is_some(), "missing deployment step");
        let Some(deployment) = deployment else {
            return;
        };
        assert_eq!(deployment.phase, "program");
        assert!(
            deployment
                .details
                .iter()
                .any(|detail| detail.contains("bytecode length 42 bytes")),
            "{deployment:?}"
        );
        assert_eq!(
            deployment
                .refs
                .as_ref()
                .and_then(|refs| refs.program_id_hex.as_deref()),
            summary.program_id_hex.as_deref()
        );
    }
}
