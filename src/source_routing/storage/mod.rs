mod inspection;
mod layer;
mod operations;
mod plan;
mod transport;

use anyhow::{Result, bail};

const BACKUP_CID_MAX_BYTES: usize = 256;

fn parse_backup_cid(value: String) -> Result<String> {
    let value = value.trim();
    validate_backup_cid(value)?;
    Ok(value.to_owned())
}

fn validate_backup_cid(value: &str) -> Result<()> {
    if value.is_empty() {
        bail!("backup CID is required");
    }
    if value.len() > BACKUP_CID_MAX_BYTES {
        bail!("backup CID exceeds {BACKUP_CID_MAX_BYTES} byte limit");
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        bail!("backup CID must contain only ASCII letters, digits, `-`, or `_`");
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
    StorageOperationRequest, StoragePayloadUploadRequest, download_response, execute_operation,
};
pub(crate) use plan::storage_module_probe_plan;
pub(crate) use transport::BackupDownloadCleanupUnconfirmed;
