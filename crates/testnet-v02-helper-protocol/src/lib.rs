use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const HELPER_MODE: &str = "__testnet-v02-private-helper";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelperRequest {
    CheckProfile {
        wallet_home: PathBuf,
        sequencer_endpoint: Option<String>,
    },
    SubmitPrivateIdl(SubmitPrivateIdlRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitPrivateIdlRequest {
    pub wallet_home: PathBuf,
    pub sequencer_endpoint: Option<String>,
    pub expected_program_id: [u32; 8],
    pub program_binary: PathBuf,
    pub dependency_binaries: Vec<PathBuf>,
    pub accounts: Vec<HelperAccount>,
    pub instruction_words: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelperAccount {
    pub account_id: [u8; 32],
    pub privacy: HelperAccountPrivacy,
    pub signer: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelperAccountPrivacy {
    Public,
    Private,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum HelperResponse {
    Success { result: HelperSuccess },
    Failure { error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HelperSuccess {
    Profile {
        protocol: String,
    },
    Submitted {
        tx_hash: String,
        shared_secret_count: usize,
    },
}
