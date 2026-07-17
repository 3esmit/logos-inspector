mod inspection;
mod layer;
mod operations;
mod plan;
mod transport;

use anyhow::{Result, bail};

const STORAGE_CID_MAX_BYTES: usize = 256;
#[cfg(test)]
const BACKUP_CID_MAX_BYTES: usize = STORAGE_CID_MAX_BYTES;

fn parse_storage_cid(value: String) -> Result<String> {
    let value = value.trim();
    validate_storage_cid(value)?;
    Ok(value.to_owned())
}

fn parse_backup_cid(value: String) -> Result<String> {
    let value = value.trim();
    validate_backup_cid(value)?;
    Ok(value.to_owned())
}

fn validate_storage_cid(value: &str) -> Result<()> {
    validate_cid(value, "storage CID")
}

fn validate_backup_cid(value: &str) -> Result<()> {
    validate_cid(value, "backup CID")
}

fn validate_cid(value: &str, label: &str) -> Result<()> {
    if value.is_empty() {
        bail!("{label} is required");
    }
    if value.len() > STORAGE_CID_MAX_BYTES {
        bail!("{label} exceeds {STORAGE_CID_MAX_BYTES} byte limit");
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        bail!("{label} must contain only ASCII letters, digits, `-`, or `_`");
    }
    Ok(())
}

pub use inspection::storage_source_report;
pub(crate) use layer::{
    STORAGE_SOURCE_MODES, managed_config, managed_contract, module_report, report_inputs,
};
pub(crate) use operations::{
    StorageBackupDownloadRequest, StorageBackupUploadRequest, StorageClient,
    StorageDownloadRequest, StorageExistsRequest, StorageOperation, StorageOperationOutput,
    StorageOperationRequest, StoragePayloadUploadRequest, StorageRemoveSettlementUnconfirmed,
    StorageUploadSettlementUnconfirmed, download_response, execute_operation,
};
pub(crate) use plan::storage_module_probe_plan;
pub(crate) use transport::BackupDownloadCleanupUnconfirmed;
#[cfg(all(test, unix))]
pub(crate) use transport::serialize_cli_backup_test;
