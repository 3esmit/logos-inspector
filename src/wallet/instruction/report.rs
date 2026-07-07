use super::{LocalWalletInstructionReport, ResolvedInstructionAccount, model::PreparedInstruction};
use crate::wallet::unix_time_text;

pub(super) fn report_from_prepared(
    prepared: PreparedInstruction,
    status: &str,
    tx_hash: Option<String>,
    shared_secret_count: Option<usize>,
) -> LocalWalletInstructionReport {
    let submitted_at = tx_hash.as_ref().map(|_| unix_time_text());
    LocalWalletInstructionReport {
        source: "local_wallet_direct".to_owned(),
        status: status.to_owned(),
        mode: prepared.mode.as_str().to_owned(),
        instruction: prepared.instruction,
        program_id_hex: prepared.program_id_hex,
        command: "wallet direct IDL instruction".to_owned(),
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
