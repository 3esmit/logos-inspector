use super::{LocalWalletInstructionReport, ResolvedInstructionAccount, model::PreparedInstruction};
use crate::wallet::unix_time_text;

pub(super) fn report_from_prepared(
    prepared: PreparedInstruction,
    status: &str,
    tx_hash: Option<String>,
    shared_secret_count: Option<usize>,
) -> LocalWalletInstructionReport {
    let submitted_at = tx_hash.as_ref().map(|_| unix_time_text());
    let operation_detail = instruction_operation_detail(
        prepared.mode.as_str(),
        &prepared.instruction,
        tx_hash.as_deref(),
        prepared.instruction_words.len(),
    );
    LocalWalletInstructionReport {
        source: "local_wallet_direct".to_owned(),
        status: status.to_owned(),
        mode: prepared.mode.as_str().to_owned(),
        instruction: prepared.instruction,
        program_id_hex: prepared.program_id_hex,
        command: "wallet direct IDL instruction".to_owned(),
        operation_detail,
        program_binary_required: prepared.program_binary_required,
        program_binary: prepared.program_binary,
        accounts: prepared
            .accounts
            .into_iter()
            .map(|account| ResolvedInstructionAccount {
                name: account.name,
                account_id: account.account_id.to_string(),
                privacy: account.privacy.as_str().to_owned(),
                signer: account.signer,
                rest: account.rest,
                pda: account.pda,
            })
            .collect(),
        args: prepared.args,
        instruction_words_hex: prepared
            .instruction_words
            .iter()
            .map(|word| format!("{word:08x}"))
            .collect(),
        instruction_words: prepared.instruction_words,
        tx_hash,
        shared_secret_count,
        submitted_at,
    }
}

fn instruction_operation_detail(
    mode: &str,
    instruction: &str,
    tx_hash: Option<&str>,
    word_count: usize,
) -> String {
    if let Some(tx_hash) = tx_hash.filter(|value| !value.is_empty()) {
        return format!("{mode} {instruction}, tx {}", short_hash(tx_hash));
    }
    format!("{mode} {instruction}, {word_count} word(s)")
}

fn short_hash(value: &str) -> String {
    let value = value.trim();
    if value.len() <= 18 {
        return value.to_owned();
    }
    format!("{}...{}", &value[..10], &value[value.len() - 6..])
}
