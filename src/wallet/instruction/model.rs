use lee::AccountId;
use lee_core::program::ProgramId;

use super::ResolvedInstructionArg;

#[derive(Debug, Clone)]
pub(super) struct PreparedInstruction {
    pub(super) instruction: String,
    pub(super) program_id: ProgramId,
    pub(super) program_id_hex: String,
    pub(super) program_binary: String,
    pub(super) program_binary_required: bool,
    pub(super) mode: InstructionMode,
    pub(super) accounts: Vec<PreparedAccount>,
    pub(super) args: Vec<ResolvedInstructionArg>,
    pub(super) instruction_words: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InstructionMode {
    Public,
    Private,
}

impl InstructionMode {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct PreparedAccount {
    pub(super) name: String,
    pub(super) account_id: AccountId,
    pub(super) privacy: AccountPrivacy,
    pub(super) signer: bool,
    pub(super) rest: bool,
    pub(super) pda: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AccountPrivacy {
    Public,
    Private,
}

impl AccountPrivacy {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}
