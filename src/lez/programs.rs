use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context as _, Result};
use lee::{AccountId, ProgramDeploymentTransaction, program::Program};
use lee_core::program::ProgramId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProgramIdEntry {
    pub label: String,
    pub base58: String,
    pub hex: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgramFileInfo {
    pub path: String,
    pub bytecode_len: usize,
    pub program_id_hex: String,
    pub program_id_base58: String,
    pub deployment_tx_hash: String,
}

pub fn program_file_info(path: impl AsRef<Path>) -> Result<ProgramFileInfo> {
    let path = path.as_ref();
    let bytecode = fs::read(path)
        .with_context(|| format!("failed to read program bytecode at {}", path.display()))?;
    let tx = ProgramDeploymentTransaction::new(lee::program_deployment_transaction::Message::new(
        bytecode.clone(),
    ));
    let program = Program::new(bytecode.clone().into())
        .map_err(|err| anyhow::anyhow!("failed to parse program bytecode: {err:?}"))?;
    let program_id = program.id();
    Ok(ProgramFileInfo {
        path: path.display().to_string(),
        bytecode_len: bytecode.len(),
        program_id_hex: program_id_hex(program_id),
        program_id_base58: program_id_base58(program_id),
        deployment_tx_hash: hex::encode(tx.hash()),
    })
}

#[must_use]
pub fn program_id_hex(program_id: ProgramId) -> String {
    program_id
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[must_use]
pub fn program_id_base58(program_id: ProgramId) -> String {
    AccountId::new(program_id_bytes(program_id)).to_string()
}

pub(crate) fn program_id_base58_from_hex(program_id_hex: &str) -> Option<String> {
    let bytes = hex::decode(program_id_hex).ok()?;
    let fixed: [u8; 32] = bytes.try_into().ok()?;
    Some(AccountId::new(fixed).to_string())
}

pub(crate) fn program_entries(programs: BTreeMap<String, ProgramId>) -> Vec<ProgramIdEntry> {
    programs
        .into_iter()
        .map(|(label, program_id)| ProgramIdEntry {
            label,
            base58: program_id_base58(program_id),
            hex: program_id_hex(program_id),
        })
        .collect()
}

fn program_id_bytes(program_id: ProgramId) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    for (chunk, word) in bytes.chunks_exact_mut(4).zip(program_id.iter()) {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    bytes
}
