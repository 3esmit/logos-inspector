use std::{
    collections::BTreeSet,
    error::Error as StdError,
    fmt,
    fs::{self, File, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use anyhow::{Context as _, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

const LOCK_FILE_NAME: &str = ".settings.lock";
const JOURNAL_FILE_NAME: &str = ".local-state.rollback.json";
const JOURNAL_SCHEMA_VERSION: u64 = 1;
const TRANSACTION_ID_BYTES: usize = 16;
pub(crate) const LOCAL_STATE_TRANSACTION_ID_HEX_LENGTH: usize = TRANSACTION_ID_BYTES * 2;
const SHA256_HEX_LENGTH: usize = 64;

struct LocalStateProcessState {
    recovery_required: BTreeSet<PathBuf>,
}

static LOCAL_STATE_PROCESS_LOCK: Mutex<LocalStateProcessState> =
    Mutex::new(LocalStateProcessState {
        recovery_required: BTreeSet::new(),
    });

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StateFile {
    Settings,
    Idl,
    Wallet,
}

impl StateFile {
    const ALL: [Self; 3] = [Self::Settings, Self::Idl, Self::Wallet];

    const fn file_name(self) -> &'static str {
        match self {
            Self::Settings => "settings.json",
            Self::Idl => "idls.json",
            Self::Wallet => "wallet.json",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Settings => "settings",
            Self::Idl => "IDL",
            Self::Wallet => "wallet",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum StoredBytes {
    Missing,
    Present(Vec<u8>),
}

impl StoredBytes {
    fn checksum(&self) -> Option<String> {
        match self {
            Self::Missing => None,
            Self::Present(bytes) => Some(sha256_hex(bytes)),
        }
    }
}

impl fmt::Debug for StoredBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => formatter.write_str("Missing"),
            Self::Present(bytes) => formatter
                .debug_struct("Present")
                .field("byte_len", &bytes.len())
                .finish(),
        }
    }
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalStateSnapshot {
    pub(crate) settings: StoredBytes,
    pub(crate) idl: StoredBytes,
    pub(crate) wallet: StoredBytes,
}

#[derive(Default)]
pub(crate) struct LocalStateWriteSet {
    settings: Option<Vec<u8>>,
    idl: Option<Vec<u8>>,
    wallet: Option<Vec<u8>>,
}

impl fmt::Debug for LocalStateWriteSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalStateWriteSet")
            .field("settings_bytes", &self.settings.as_ref().map(Vec::len))
            .field("idl_bytes", &self.idl.as_ref().map(Vec::len))
            .field("wallet_bytes", &self.wallet.as_ref().map(Vec::len))
            .finish()
    }
}

impl LocalStateWriteSet {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn settings(mut self, bytes: Vec<u8>) -> Self {
        self.settings = Some(bytes);
        self
    }

    pub(crate) fn idl(mut self, bytes: Vec<u8>) -> Self {
        self.idl = Some(bytes);
        self
    }

    pub(crate) fn wallet(mut self, bytes: Vec<u8>) -> Self {
        self.wallet = Some(bytes);
        self
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.settings.is_none() && self.idl.is_none() && self.wallet.is_none()
    }

    fn into_entries(self) -> Vec<(StateFile, Vec<u8>)> {
        let mut entries = Vec::with_capacity(StateFile::ALL.len());
        if let Some(bytes) = self.settings {
            entries.push((StateFile::Settings, bytes));
        }
        if let Some(bytes) = self.idl {
            entries.push((StateFile::Idl, bytes));
        }
        if let Some(bytes) = self.wallet {
            entries.push((StateFile::Wallet, bytes));
        }
        entries
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DirectoryDurability {
    Verified,
    PlatformUnverified,
}

impl DirectoryDurability {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::PlatformUnverified => "platform_unverified",
        }
    }

    fn combine(self, other: Self) -> Self {
        if self == Self::Verified && other == Self::Verified {
            Self::Verified
        } else {
            Self::PlatformUnverified
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalStateCommitReport {
    pub(crate) transaction_id: String,
    pub(crate) directory_durability: DirectoryDurability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalStateFailureStatus {
    RolledBack,
    RecoveryRequired,
}

impl LocalStateFailureStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::RolledBack => "rolled_back",
            Self::RecoveryRequired => "recovery_required",
        }
    }
}

#[derive(Debug)]
pub(crate) struct LocalStateTransactionError {
    transaction_id: Option<String>,
    status: LocalStateFailureStatus,
    phase: &'static str,
}

impl LocalStateTransactionError {
    fn rolled_back(transaction_id: &str, phase: &'static str) -> Self {
        Self {
            transaction_id: Some(transaction_id.to_owned()),
            status: LocalStateFailureStatus::RolledBack,
            phase,
        }
    }

    fn recovery_required(transaction_id: Option<&str>, phase: &'static str) -> Self {
        Self {
            transaction_id: transaction_id.map(str::to_owned),
            status: LocalStateFailureStatus::RecoveryRequired,
            phase,
        }
    }

    pub(crate) const fn status(&self) -> LocalStateFailureStatus {
        self.status
    }

    pub(crate) fn transaction_id(&self) -> Option<&str> {
        self.transaction_id.as_deref()
    }
}

impl fmt::Display for LocalStateTransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let transaction_id = self.transaction_id.as_deref().unwrap_or("unknown");
        write!(
            formatter,
            "local state transaction `{transaction_id}` {} during {}",
            self.status.as_str(),
            self.phase
        )
    }
}

impl StdError for LocalStateTransactionError {}

pub(crate) struct LocalStateSession {
    base_dir: PathBuf,
    process_guard: MutexGuard<'static, LocalStateProcessState>,
    _file_guard: File,
}

impl fmt::Debug for LocalStateSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalStateSession")
            .field("base_dir", &self.base_dir)
            .finish_non_exhaustive()
    }
}

pub(crate) fn with_local_state<R>(
    operation: impl FnOnce(&mut LocalStateSession) -> Result<R>,
) -> Result<R> {
    with_local_state_in(&super::config_path::config_dir()?, operation)
}

pub(crate) fn with_local_state_in<R>(
    base_dir: &Path,
    operation: impl FnOnce(&mut LocalStateSession) -> Result<R>,
) -> Result<R> {
    let mut session = lock_local_state_in(base_dir)?;
    operation(&mut session)
}

pub(crate) fn lock_local_state_in(base_dir: &Path) -> Result<LocalStateSession> {
    LocalStateSession::acquire(base_dir, &mut NoopHook)
}

pub(crate) fn recover_local_state() -> Result<()> {
    with_local_state(|_| Ok(()))
}

impl LocalStateSession {
    fn acquire(base_dir: &Path, hook: &mut impl IoHook) -> Result<Self> {
        let process_guard = LOCAL_STATE_PROCESS_LOCK
            .lock()
            .map_err(|_| anyhow::anyhow!("local state process lock is poisoned"))?;
        fs::create_dir_all(base_dir).with_context(|| {
            format!(
                "failed to create local state directory {}",
                base_dir.display()
            )
        })?;
        let base_dir = fs::canonicalize(base_dir).with_context(|| {
            format!(
                "failed to resolve local state directory {}",
                base_dir.display()
            )
        })?;
        if process_guard.recovery_required.contains(&base_dir) {
            return Err(LocalStateTransactionError::recovery_required(
                None,
                "process recovery gate",
            )
            .into());
        }
        let lock_path = base_dir.join(LOCK_FILE_NAME);
        reject_symlink(&lock_path, "local state lock")?;
        let lock_file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .with_context(|| format!("failed to open local state lock {}", lock_path.display()))?;
        lock_file
            .lock()
            .with_context(|| format!("failed to lock local state at {}", lock_path.display()))?;
        let mut session = Self {
            base_dir,
            process_guard,
            _file_guard: lock_file,
        };
        session.recover_hot_journal(hook)?;
        Ok(session)
    }

    pub(crate) fn read(&self, file: StateFile) -> Result<StoredBytes> {
        read_stored_bytes(&self.path(file), file.label())
    }

    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> Result<LocalStateSnapshot> {
        Ok(LocalStateSnapshot {
            settings: self.read(StateFile::Settings)?,
            idl: self.read(StateFile::Idl)?,
            wallet: self.read(StateFile::Wallet)?,
        })
    }

    pub(crate) fn path_text(&self, file: StateFile) -> String {
        self.path(file).display().to_string()
    }

    pub(crate) fn atomic_replace(
        &mut self,
        file: StateFile,
        bytes: &[u8],
    ) -> Result<DirectoryDurability> {
        let mut hook = NoopHook;
        let mut staged = self.stage_target(file, bytes, &mut hook)?;
        self.persist_target(&mut staged, &mut hook)
    }

    pub(crate) fn commit(
        &mut self,
        writes: LocalStateWriteSet,
        cancellation_probe: impl FnMut() -> Result<()>,
    ) -> Result<LocalStateCommitReport> {
        self.commit_with_hook(writes, cancellation_probe, &mut NoopHook)
    }

    fn commit_with_hook(
        &mut self,
        writes: LocalStateWriteSet,
        mut cancellation_probe: impl FnMut() -> Result<()>,
        hook: &mut impl IoHook,
    ) -> Result<LocalStateCommitReport> {
        let entries = writes.into_entries();
        if entries.is_empty() {
            bail!("local state transaction write set is empty");
        }
        let transaction_id = transaction_id()?;
        let mut journal_entries = Vec::with_capacity(entries.len());
        let mut staged = Vec::with_capacity(entries.len());
        for (file, bytes) in entries {
            let original = self.read(file)?;
            let new_sha256 = sha256_hex(&bytes);
            let old_sha256 = original.checksum();
            let temporary = self.stage_target(file, &bytes, hook)?;
            journal_entries.push(JournalEntry {
                file,
                original: JournalMemento::from_stored(&original),
                old_sha256,
                new_sha256,
            });
            staged.push(temporary);
        }

        cancellation_probe().context("local state transaction canceled before commit")?;

        let journal = RollbackJournal {
            schema_version: JOURNAL_SCHEMA_VERSION,
            transaction_id: transaction_id.clone(),
            entries: journal_entries,
        };
        validate_journal(&journal)?;
        let journal_bytes = serde_json::to_vec_pretty(&journal)
            .context("failed to serialize local state rollback journal")?;
        let mut journal_installed = false;
        let mut durability = DirectoryDurability::Verified;

        let result = (|| -> Result<()> {
            let journal_durability =
                self.install_journal(&journal_bytes, hook, &mut cancellation_probe)?;
            journal_installed = true;
            durability = durability.combine(journal_durability);
            for target in &mut staged {
                durability = durability.combine(self.persist_target(target, hook)?);
            }
            self.verify_new_targets(&journal, hook)?;
            self.remove_journal(hook, IoPoint::JournalRemove, IoPoint::CommitDirectorySync)
                .map(|status| durability = durability.combine(status))?;
            journal_installed = false;
            Ok(())
        })();

        match result {
            Ok(()) => Ok(LocalStateCommitReport {
                transaction_id,
                directory_durability: durability,
            }),
            Err(error) if is_simulated_crash(&error) => Err(error),
            Err(error) => {
                let journal_present = self.inspect_journal_or_gate(
                    Some(&transaction_id),
                    "journal inspection",
                    hook,
                )?;
                if !journal_installed && !journal_present {
                    return Err(error);
                }
                if !journal_present {
                    self.restore_hot_journal_after_removal(&journal_bytes, hook)?;
                }
                match self.rollback_journal(&journal, hook) {
                    Ok(()) => Err(LocalStateTransactionError::rolled_back(
                        &transaction_id,
                        "commit",
                    )
                    .into()),
                    Err(rollback_error) if is_simulated_crash(&rollback_error) => {
                        Err(rollback_error)
                    }
                    Err(_rollback_error) => Err(LocalStateTransactionError::recovery_required(
                        Some(&transaction_id),
                        "rollback",
                    )
                    .into()),
                }
            }
        }
    }

    fn path(&self, file: StateFile) -> PathBuf {
        self.base_dir.join(file.file_name())
    }

    fn journal_path(&self) -> PathBuf {
        self.base_dir.join(JOURNAL_FILE_NAME)
    }

    fn stage_target(
        &self,
        file: StateFile,
        bytes: &[u8],
        hook: &mut impl IoHook,
    ) -> Result<StagedTarget> {
        let path = self.path(file);
        reject_symlink(&path, file.label())?;
        let create = IoPoint::StageCreate(file);
        hook.before(create)?;
        let mut temporary = tempfile::Builder::new()
            .prefix(&format!(".{}.", file.file_name()))
            .suffix(".stage")
            .tempfile_in(&self.base_dir)
            .with_context(|| format!("failed to stage {} state", file.label()))?;
        hook.after(create)?;
        let write = IoPoint::StageWrite(file);
        hook.before(write)?;
        temporary
            .write_all(bytes)
            .with_context(|| format!("failed to write staged {} state", file.label()))?;
        hook.after(write)?;
        let sync = IoPoint::StageFileSync(file);
        hook.before(sync)?;
        temporary
            .as_file()
            .sync_all()
            .with_context(|| format!("failed to sync staged {} state", file.label()))?;
        hook.after(sync)?;
        Ok(StagedTarget {
            file,
            checksum: sha256_hex(bytes),
            temporary: Some(temporary),
        })
    }

    fn persist_target(
        &self,
        target: &mut StagedTarget,
        hook: &mut impl IoHook,
    ) -> Result<DirectoryDurability> {
        let point = IoPoint::TargetPersist(target.file);
        hook.before(point)?;
        let temporary = target
            .temporary
            .take()
            .context("staged local state target was already consumed")?;
        let path = self.path(target.file);
        reject_symlink(&path, target.file.label())?;
        temporary
            .persist(&path)
            .map_err(|error| error.error)
            .with_context(|| format!("failed to replace {} state", target.file.label()))?;
        hook.after(point)?;
        let durability = self.sync_directory(IoPoint::TargetDirectorySync(target.file), hook)?;
        self.verify_checksum(
            target.file,
            &target.checksum,
            IoPoint::TargetVerify(target.file),
            hook,
        )?;
        Ok(durability)
    }

    fn install_journal(
        &self,
        journal_bytes: &[u8],
        hook: &mut impl IoHook,
        cancellation_probe: &mut impl FnMut() -> Result<()>,
    ) -> Result<DirectoryDurability> {
        let journal_path = self.journal_path();
        if path_present(&journal_path)? {
            bail!("local state rollback journal already exists");
        }
        reject_symlink(&journal_path, "local state rollback journal")?;
        hook.before(IoPoint::JournalCreate)?;
        let mut temporary = tempfile::Builder::new()
            .prefix(".local-state.rollback.")
            .suffix(".tmp")
            .tempfile_in(&self.base_dir)
            .context("failed to create local state rollback journal")?;
        hook.after(IoPoint::JournalCreate)?;
        hook.before(IoPoint::JournalWrite)?;
        temporary
            .write_all(journal_bytes)
            .context("failed to write local state rollback journal")?;
        hook.after(IoPoint::JournalWrite)?;
        hook.before(IoPoint::JournalFileSync)?;
        temporary
            .as_file()
            .sync_all()
            .context("failed to sync local state rollback journal")?;
        hook.after(IoPoint::JournalFileSync)?;
        cancellation_probe().context("local state transaction canceled before commit")?;
        hook.before(IoPoint::JournalPersist)?;
        temporary
            .persist(&journal_path)
            .map_err(|error| error.error)
            .context("failed to persist local state rollback journal")?;
        hook.after(IoPoint::JournalPersist)?;
        self.sync_directory(IoPoint::JournalDirectorySync, hook)
    }

    fn reinstall_journal(&self, journal_bytes: &[u8]) -> Result<()> {
        if path_present(&self.journal_path())? {
            return Ok(());
        }
        let mut hook = NoopHook;
        let mut cancellation_probe = || Ok(());
        self.install_journal(journal_bytes, &mut hook, &mut cancellation_probe)
            .map(|_| ())
    }

    fn restore_hot_journal_after_removal(
        &mut self,
        journal_bytes: &[u8],
        hook: &mut impl IoHook,
    ) -> Result<()> {
        let result = (|| -> Result<()> {
            hook.before(IoPoint::JournalReinstall)?;
            self.reinstall_journal(journal_bytes)?;
            hook.after(IoPoint::JournalReinstall)
        })();
        if result.is_err() {
            self.process_guard
                .recovery_required
                .insert(self.base_dir.clone());
            return Err(LocalStateTransactionError::recovery_required(
                None,
                "journal reinstatement",
            )
            .into());
        }
        Ok(())
    }

    fn verify_new_targets(&self, journal: &RollbackJournal, hook: &mut impl IoHook) -> Result<()> {
        for entry in &journal.entries {
            self.verify_checksum(
                entry.file,
                &entry.new_sha256,
                IoPoint::TargetVerify(entry.file),
                hook,
            )?;
        }
        Ok(())
    }

    fn verify_checksum(
        &self,
        file: StateFile,
        expected: &str,
        point: IoPoint,
        hook: &mut impl IoHook,
    ) -> Result<()> {
        hook.before(point)?;
        let current = self.read(file)?;
        if current.checksum().as_deref() != Some(expected) {
            bail!("{} state verification failed", file.label());
        }
        hook.after(point)
    }

    fn recover_hot_journal(&mut self, hook: &mut impl IoHook) -> Result<()> {
        let journal_path = self.journal_path();
        if !path_present(&journal_path).map_err(|_| {
            LocalStateTransactionError::recovery_required(None, "journal inspection")
        })? {
            return Ok(());
        }
        reject_symlink(&journal_path, "local state rollback journal").map_err(|_| {
            LocalStateTransactionError::recovery_required(None, "journal validation")
        })?;
        let bytes = fs::read(&journal_path)
            .map_err(|_| LocalStateTransactionError::recovery_required(None, "journal read"))?;
        let journal = serde_json::from_slice::<RollbackJournal>(&bytes)
            .ok()
            .and_then(|journal| validate_journal(&journal).ok().map(|()| journal))
            .ok_or_else(|| {
                LocalStateTransactionError::recovery_required(None, "journal validation")
            })?;
        self.rollback_journal(&journal, hook).map_err(|error| {
            if error.downcast_ref::<LocalStateTransactionError>().is_some() {
                error
            } else {
                LocalStateTransactionError::recovery_required(
                    Some(&journal.transaction_id),
                    "hot journal recovery",
                )
                .into()
            }
        })
    }

    fn rollback_journal(
        &mut self,
        journal: &RollbackJournal,
        hook: &mut impl IoHook,
    ) -> Result<()> {
        self.verify_recoverable_targets(journal)?;
        for entry in journal.entries.iter().rev() {
            match entry.original.to_stored()? {
                StoredBytes::Missing => self.remove_for_rollback(entry.file, hook)?,
                StoredBytes::Present(bytes) => {
                    self.restore_for_rollback(entry.file, &bytes, hook)?;
                }
            }
        }
        let removal = self.remove_journal(
            hook,
            IoPoint::RollbackJournalRemove,
            IoPoint::RollbackJournalDirectorySync,
        );
        if let Err(error) = removal {
            if !self.inspect_journal_or_gate(
                Some(&journal.transaction_id),
                "rollback journal inspection",
                hook,
            )? {
                let journal_bytes = serde_json::to_vec_pretty(journal)
                    .context("failed to serialize local state recovery journal")?;
                self.restore_hot_journal_after_removal(&journal_bytes, hook)?;
            }
            return Err(error);
        }
        Ok(())
    }

    fn verify_recoverable_targets(&self, journal: &RollbackJournal) -> Result<()> {
        for entry in &journal.entries {
            let current = self.read(entry.file)?;
            let checksum = current.checksum();
            let matches_old = checksum == entry.old_sha256;
            let matches_new = checksum.as_deref() == Some(entry.new_sha256.as_str());
            if !matches_old && !matches_new {
                return Err(LocalStateTransactionError::recovery_required(
                    Some(&journal.transaction_id),
                    "target verification",
                )
                .into());
            }
        }
        Ok(())
    }

    fn restore_for_rollback(
        &self,
        file: StateFile,
        bytes: &[u8],
        hook: &mut impl IoHook,
    ) -> Result<()> {
        let path = self.path(file);
        reject_symlink(&path, file.label())?;
        hook.before(IoPoint::RollbackCreate(file))?;
        let mut temporary = tempfile::Builder::new()
            .prefix(&format!(".{}.rollback.", file.file_name()))
            .suffix(".tmp")
            .tempfile_in(&self.base_dir)
            .with_context(|| format!("failed to stage {} rollback", file.label()))?;
        hook.after(IoPoint::RollbackCreate(file))?;
        hook.before(IoPoint::RollbackWrite(file))?;
        temporary
            .write_all(bytes)
            .with_context(|| format!("failed to write {} rollback", file.label()))?;
        hook.after(IoPoint::RollbackWrite(file))?;
        hook.before(IoPoint::RollbackFileSync(file))?;
        temporary
            .as_file()
            .sync_all()
            .with_context(|| format!("failed to sync {} rollback", file.label()))?;
        hook.after(IoPoint::RollbackFileSync(file))?;
        hook.before(IoPoint::RollbackPersist(file))?;
        temporary
            .persist(&path)
            .map_err(|error| error.error)
            .with_context(|| format!("failed to persist {} rollback", file.label()))?;
        hook.after(IoPoint::RollbackPersist(file))?;
        self.sync_directory(IoPoint::RollbackDirectorySync(file), hook)?;
        hook.before(IoPoint::RollbackVerify(file))?;
        if self.read(file)? != StoredBytes::Present(bytes.to_vec()) {
            bail!("{} rollback verification failed", file.label());
        }
        hook.after(IoPoint::RollbackVerify(file))
    }

    fn remove_for_rollback(&self, file: StateFile, hook: &mut impl IoHook) -> Result<()> {
        let path = self.path(file);
        reject_symlink(&path, file.label())?;
        hook.before(IoPoint::RollbackRemove(file))?;
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to remove {} rollback target", file.label()));
            }
        }
        hook.after(IoPoint::RollbackRemove(file))?;
        self.sync_directory(IoPoint::RollbackDirectorySync(file), hook)?;
        hook.before(IoPoint::RollbackVerify(file))?;
        if self.read(file)? != StoredBytes::Missing {
            bail!("{} absence rollback verification failed", file.label());
        }
        hook.after(IoPoint::RollbackVerify(file))
    }

    fn remove_journal(
        &self,
        hook: &mut impl IoHook,
        remove_point: IoPoint,
        sync_point: IoPoint,
    ) -> Result<DirectoryDurability> {
        hook.before(remove_point)?;
        fs::remove_file(self.journal_path())
            .context("failed to remove local state rollback journal")?;
        hook.after(remove_point)?;
        self.sync_directory(sync_point, hook)
    }

    fn sync_directory(
        &self,
        point: IoPoint,
        hook: &mut impl IoHook,
    ) -> Result<DirectoryDurability> {
        hook.before(point)?;
        let status = sync_parent_directory(&self.base_dir)?;
        hook.after(point)?;
        Ok(status)
    }

    fn journal_present(&self) -> Result<bool> {
        path_present(&self.journal_path())
    }

    fn inspect_journal_or_gate(
        &mut self,
        transaction_id: Option<&str>,
        phase: &'static str,
        hook: &mut impl IoHook,
    ) -> Result<bool> {
        let inspected = (|| -> Result<bool> {
            hook.before(IoPoint::JournalInspect)?;
            let present = self.journal_present()?;
            hook.after(IoPoint::JournalInspect)?;
            Ok(present)
        })();
        match inspected {
            Ok(present) => Ok(present),
            Err(_) => {
                self.process_guard
                    .recovery_required
                    .insert(self.base_dir.clone());
                Err(LocalStateTransactionError::recovery_required(transaction_id, phase).into())
            }
        }
    }
}

struct StagedTarget {
    file: StateFile,
    checksum: String,
    temporary: Option<tempfile::NamedTempFile>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RollbackJournal {
    schema_version: u64,
    transaction_id: String,
    entries: Vec<JournalEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalEntry {
    file: StateFile,
    original: JournalMemento,
    old_sha256: Option<String>,
    new_sha256: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum JournalMemento {
    Missing,
    Present { bytes_base64: String },
}

impl fmt::Debug for JournalMemento {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => formatter.write_str("Missing"),
            Self::Present { bytes_base64 } => formatter
                .debug_struct("Present")
                .field("encoded_byte_len", &bytes_base64.len())
                .finish(),
        }
    }
}

impl JournalMemento {
    fn from_stored(stored: &StoredBytes) -> Self {
        match stored {
            StoredBytes::Missing => Self::Missing,
            StoredBytes::Present(bytes) => Self::Present {
                bytes_base64: BASE64_STANDARD.encode(bytes),
            },
        }
    }

    fn to_stored(&self) -> Result<StoredBytes> {
        match self {
            Self::Missing => Ok(StoredBytes::Missing),
            Self::Present { bytes_base64 } => BASE64_STANDARD
                .decode(bytes_base64)
                .map(StoredBytes::Present)
                .context("local state rollback journal contains invalid base64"),
        }
    }
}

fn validate_journal(journal: &RollbackJournal) -> Result<()> {
    if journal.schema_version != JOURNAL_SCHEMA_VERSION {
        bail!("local state rollback journal version is not supported");
    }
    if journal.transaction_id.len() != LOCAL_STATE_TRANSACTION_ID_HEX_LENGTH
        || !journal
            .transaction_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        bail!("local state rollback journal transaction id is invalid");
    }
    if journal.entries.is_empty() || journal.entries.len() > StateFile::ALL.len() {
        bail!("local state rollback journal write set is invalid");
    }
    let mut seen = BTreeSet::new();
    let mut previous = None;
    for entry in &journal.entries {
        if !seen.insert(entry.file) || previous.is_some_and(|file| file >= entry.file) {
            bail!("local state rollback journal target order is invalid");
        }
        previous = Some(entry.file);
        validate_checksum(&entry.new_sha256)?;
        if let Some(checksum) = entry.old_sha256.as_deref() {
            validate_checksum(checksum)?;
        }
        let original = entry.original.to_stored()?;
        if original.checksum() != entry.old_sha256 {
            bail!("local state rollback journal original checksum is invalid");
        }
    }
    Ok(())
}

fn validate_checksum(value: &str) -> Result<()> {
    if value.len() != SHA256_HEX_LENGTH || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("local state rollback journal checksum is invalid");
    }
    Ok(())
}

fn transaction_id() -> Result<String> {
    let mut bytes = [0_u8; TRANSACTION_ID_BYTES];
    getrandom::fill(&mut bytes).context("failed to generate local state transaction id")?;
    Ok(hex::encode(bytes))
}

fn read_stored_bytes(path: &Path, label: &str) -> Result<StoredBytes> {
    reject_symlink(path, label)?;
    match fs::read(path) {
        Ok(bytes) => Ok(StoredBytes::Present(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(StoredBytes::Missing),
        Err(error) => Err(error).with_context(|| format!("failed to read {label} state")),
    }
}

fn reject_symlink(path: &Path, label: &str) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("{label} path must not be a symbolic link")
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to inspect {label} path")),
    }
}

fn path_present(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).context("failed to inspect local state recovery path"),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) -> Result<DirectoryDurability> {
    File::open(parent)
        .with_context(|| format!("failed to open local state directory {}", parent.display()))?
        .sync_all()
        .with_context(|| format!("failed to sync local state directory {}", parent.display()))?;
    Ok(DirectoryDurability::Verified)
}

#[cfg(not(unix))]
fn sync_parent_directory(_parent: &Path) -> Result<DirectoryDurability> {
    Ok(DirectoryDurability::PlatformUnverified)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IoPoint {
    StageCreate(StateFile),
    StageWrite(StateFile),
    StageFileSync(StateFile),
    JournalCreate,
    JournalWrite,
    JournalFileSync,
    JournalPersist,
    JournalDirectorySync,
    TargetPersist(StateFile),
    TargetDirectorySync(StateFile),
    TargetVerify(StateFile),
    JournalRemove,
    CommitDirectorySync,
    RollbackCreate(StateFile),
    RollbackWrite(StateFile),
    RollbackFileSync(StateFile),
    RollbackPersist(StateFile),
    RollbackRemove(StateFile),
    RollbackDirectorySync(StateFile),
    RollbackVerify(StateFile),
    RollbackJournalRemove,
    RollbackJournalDirectorySync,
    JournalInspect,
    JournalReinstall,
}

trait IoHook {
    fn before(&mut self, _point: IoPoint) -> Result<()> {
        Ok(())
    }

    fn after(&mut self, _point: IoPoint) -> Result<()> {
        Ok(())
    }
}

struct NoopHook;

impl IoHook for NoopHook {}

#[derive(Debug)]
struct SimulatedCrash;

impl fmt::Display for SimulatedCrash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("simulated process crash")
    }
}

impl StdError for SimulatedCrash {}

fn is_simulated_crash(error: &anyhow::Error) -> bool {
    error.downcast_ref::<SimulatedCrash>().is_some()
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        process::{Command, exit},
        sync::mpsc,
        thread,
        time::Duration,
    };

    use super::*;

    const OLD_SETTINGS: &[u8] = b"old-settings";
    const OLD_IDL: &[u8] = b"old-idl";
    const OLD_WALLET: &[u8] = b"old-wallet-secret";
    const NEW_SETTINGS: &[u8] = b"new-settings";
    const NEW_IDL: &[u8] = b"new-idl";
    const NEW_WALLET: &[u8] = b"new-wallet";
    const CRASH_WORKER_ENV: &str = "LOGOS_INSPECTOR_LOCAL_STATE_CRASH_WORKER";
    const CRASH_POINT_ENV: &str = "LOGOS_INSPECTOR_LOCAL_STATE_CRASH_POINT";
    const CRASH_DIR_ENV: &str = "LOGOS_INSPECTOR_LOCAL_STATE_CRASH_DIR";
    const FILE_LOCK_READER_ENV: &str = "LOGOS_INSPECTOR_LOCAL_STATE_FILE_LOCK_READER";

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum HookMoment {
        Before,
        After,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum InjectionKind {
        Failure,
        Crash,
        ProcessExit,
    }

    #[derive(Debug, Clone, Copy)]
    struct Injection {
        moment: HookMoment,
        point: IoPoint,
        kind: InjectionKind,
        fired: bool,
    }

    struct InjectHook {
        injections: Vec<Injection>,
    }

    impl InjectHook {
        fn failure(point: IoPoint) -> Self {
            Self::new(vec![(HookMoment::Before, point, InjectionKind::Failure)])
        }

        fn crash_after(point: IoPoint) -> Self {
            Self::new(vec![(HookMoment::After, point, InjectionKind::Crash)])
        }

        fn process_exit_after(point: IoPoint) -> Self {
            Self::new(vec![(HookMoment::After, point, InjectionKind::ProcessExit)])
        }

        fn failure_then_rollback(primary: IoPoint, rollback: IoPoint) -> Self {
            Self::new(vec![
                (HookMoment::Before, primary, InjectionKind::Failure),
                (HookMoment::Before, rollback, InjectionKind::Failure),
            ])
        }

        fn new(injections: Vec<(HookMoment, IoPoint, InjectionKind)>) -> Self {
            Self {
                injections: injections
                    .into_iter()
                    .map(|(moment, point, kind)| Injection {
                        moment,
                        point,
                        kind,
                        fired: false,
                    })
                    .collect(),
            }
        }

        fn trigger(&mut self, moment: HookMoment, point: IoPoint) -> Result<()> {
            let Some(injection) = self.injections.iter_mut().find(|injection| {
                !injection.fired && injection.moment == moment && injection.point == point
            }) else {
                return Ok(());
            };
            injection.fired = true;
            match injection.kind {
                InjectionKind::Failure => {
                    bail!("injected local state I/O failure at {point:?}")
                }
                InjectionKind::Crash => Err(SimulatedCrash.into()),
                InjectionKind::ProcessExit => exit(90),
            }
        }
    }

    impl IoHook for InjectHook {
        fn before(&mut self, point: IoPoint) -> Result<()> {
            self.trigger(HookMoment::Before, point)
        }

        fn after(&mut self, point: IoPoint) -> Result<()> {
            self.trigger(HookMoment::After, point)
        }
    }

    struct PauseHook {
        point: IoPoint,
        entered: Option<mpsc::Sender<()>>,
        release: mpsc::Receiver<()>,
    }

    impl IoHook for PauseHook {
        fn after(&mut self, point: IoPoint) -> Result<()> {
            if point != self.point {
                return Ok(());
            }
            if let Some(entered) = self.entered.take() {
                entered
                    .send(())
                    .map_err(|_| anyhow::anyhow!("failed to report paused transaction"))?;
                self.release
                    .recv()
                    .map_err(|_| anyhow::anyhow!("failed to release paused transaction"))?;
            }
            Ok(())
        }
    }

    #[test]
    fn commit_applies_typed_write_set_and_cancellation_stays_before_journal() -> Result<()> {
        let directory = seeded_directory()?;
        let report = with_local_state_in(directory.path(), |session| {
            session.commit(new_write_set(), || Ok(()))
        })?;
        if report.transaction_id.len() != LOCAL_STATE_TRANSACTION_ID_HEX_LENGTH {
            bail!("transaction id is invalid");
        }
        assert_triple(directory.path(), NEW_SETTINGS, NEW_IDL, NEW_WALLET)?;
        if directory.path().join(JOURNAL_FILE_NAME).exists() {
            bail!("committed transaction left a hot journal");
        }

        seed_triple(directory.path())?;
        let error = with_local_state_in(directory.path(), |session| {
            session.commit(new_write_set(), || bail!("canceled by caller"))
        })
        .err()
        .context("canceled transaction should fail")?;
        if !error.to_string().contains("canceled before commit") {
            bail!("unexpected cancellation error: {error:#}");
        }
        assert_triple(directory.path(), OLD_SETTINGS, OLD_IDL, OLD_WALLET)?;
        if directory.path().join(JOURNAL_FILE_NAME).exists() {
            bail!("canceled transaction created a hot journal");
        }

        let mut probes = 0_u8;
        let error = with_local_state_in(directory.path(), |session| {
            session.commit(new_write_set(), || {
                probes = probes.saturating_add(1);
                if probes == 2 {
                    bail!("canceled at final pre-persist boundary");
                }
                Ok(())
            })
        })
        .err()
        .context("final-boundary cancellation should fail")?;
        if probes != 2 || !error.to_string().contains("canceled before commit") {
            bail!("final cancellation probe was not honored: probes={probes}, error={error:#}");
        }
        assert_triple(directory.path(), OLD_SETTINGS, OLD_IDL, OLD_WALLET)?;
        if path_present(&directory.path().join(JOURNAL_FILE_NAME))? {
            bail!("final-boundary cancellation persisted a hot journal");
        }
        Ok(())
    }

    #[test]
    fn secret_bytes_are_redacted_from_local_state_debug_views() -> Result<()> {
        let secret = b"wallet-private-sentinel-never-log".to_vec();
        let stored = StoredBytes::Present(secret.clone());
        let write_set = LocalStateWriteSet::new().wallet(secret.clone());
        let memento = JournalMemento::from_stored(&stored);
        let rendered = format!("{stored:?} {write_set:?} {memento:?}");
        if rendered.contains("wallet-private-sentinel-never-log")
            || rendered.contains(&BASE64_STANDARD.encode(secret))
        {
            bail!("local state debug output exposed secret bytes");
        }
        Ok(())
    }

    #[test]
    fn every_forward_io_failure_restores_the_exact_old_triple() -> Result<()> {
        let points = [
            IoPoint::StageCreate(StateFile::Settings),
            IoPoint::StageWrite(StateFile::Settings),
            IoPoint::StageFileSync(StateFile::Settings),
            IoPoint::StageCreate(StateFile::Idl),
            IoPoint::StageWrite(StateFile::Idl),
            IoPoint::StageFileSync(StateFile::Idl),
            IoPoint::StageCreate(StateFile::Wallet),
            IoPoint::StageWrite(StateFile::Wallet),
            IoPoint::StageFileSync(StateFile::Wallet),
            IoPoint::JournalCreate,
            IoPoint::JournalWrite,
            IoPoint::JournalFileSync,
            IoPoint::JournalPersist,
            IoPoint::JournalDirectorySync,
            IoPoint::TargetPersist(StateFile::Settings),
            IoPoint::TargetDirectorySync(StateFile::Settings),
            IoPoint::TargetVerify(StateFile::Settings),
            IoPoint::TargetPersist(StateFile::Idl),
            IoPoint::TargetDirectorySync(StateFile::Idl),
            IoPoint::TargetVerify(StateFile::Idl),
            IoPoint::TargetPersist(StateFile::Wallet),
            IoPoint::TargetDirectorySync(StateFile::Wallet),
            IoPoint::TargetVerify(StateFile::Wallet),
            IoPoint::JournalRemove,
            IoPoint::CommitDirectorySync,
        ];
        for point in points {
            let directory = seeded_directory()?;
            let mut hook = InjectHook::failure(point);
            let result = {
                let mut session = LocalStateSession::acquire(directory.path(), &mut NoopHook)?;
                session.commit_with_hook(new_write_set(), || Ok(()), &mut hook)
            };
            if result.is_ok() {
                bail!("injected forward failure succeeded at {point:?}");
            }
            with_local_state_in(directory.path(), |_| Ok(()))?;
            assert_triple(directory.path(), OLD_SETTINGS, OLD_IDL, OLD_WALLET)
                .with_context(|| format!("old triple not restored after {point:?}"))?;
            if directory.path().join(JOURNAL_FILE_NAME).exists() {
                bail!("forward failure left a hot journal after recovery at {point:?}");
            }
        }
        Ok(())
    }

    #[test]
    fn rollback_failure_keeps_hot_journal_and_next_access_recovers() -> Result<()> {
        let rollback_points = [
            IoPoint::RollbackCreate(StateFile::Wallet),
            IoPoint::RollbackWrite(StateFile::Wallet),
            IoPoint::RollbackFileSync(StateFile::Wallet),
            IoPoint::RollbackPersist(StateFile::Wallet),
            IoPoint::RollbackDirectorySync(StateFile::Wallet),
            IoPoint::RollbackVerify(StateFile::Wallet),
            IoPoint::RollbackCreate(StateFile::Idl),
            IoPoint::RollbackWrite(StateFile::Idl),
            IoPoint::RollbackFileSync(StateFile::Idl),
            IoPoint::RollbackPersist(StateFile::Idl),
            IoPoint::RollbackDirectorySync(StateFile::Idl),
            IoPoint::RollbackVerify(StateFile::Idl),
            IoPoint::RollbackCreate(StateFile::Settings),
            IoPoint::RollbackWrite(StateFile::Settings),
            IoPoint::RollbackFileSync(StateFile::Settings),
            IoPoint::RollbackPersist(StateFile::Settings),
            IoPoint::RollbackDirectorySync(StateFile::Settings),
            IoPoint::RollbackVerify(StateFile::Settings),
            IoPoint::RollbackJournalRemove,
            IoPoint::RollbackJournalDirectorySync,
        ];
        for rollback_point in rollback_points {
            let directory = seeded_directory()?;
            let mut hook =
                InjectHook::failure_then_rollback(IoPoint::CommitDirectorySync, rollback_point);
            let error = {
                let mut session = LocalStateSession::acquire(directory.path(), &mut NoopHook)?;
                session
                    .commit_with_hook(new_write_set(), || Ok(()), &mut hook)
                    .err()
                    .context("rollback fault should fail transaction")?
            };
            if !error.to_string().contains("recovery_required") {
                bail!("rollback fault was not gated at {rollback_point:?}: {error:#}");
            }
            if !directory.path().join(JOURNAL_FILE_NAME).is_file() {
                bail!("rollback fault did not retain journal at {rollback_point:?}");
            }
            with_local_state_in(directory.path(), |_| Ok(()))?;
            assert_triple(directory.path(), OLD_SETTINGS, OLD_IDL, OLD_WALLET)?;
        }
        Ok(())
    }

    #[test]
    fn rollback_restores_original_absence_and_errors_hide_wallet_bytes() -> Result<()> {
        let directory = tempfile::tempdir().context("failed to create local state test dir")?;
        fs::write(directory.path().join("idls.json"), OLD_IDL)
            .context("failed to seed IDL state")?;
        let secret = b"wallet-private-sentinel-never-log";
        fs::write(directory.path().join("wallet.json"), secret)
            .context("failed to seed wallet state")?;
        let mut hook = InjectHook::failure_then_rollback(
            IoPoint::CommitDirectorySync,
            IoPoint::RollbackPersist(StateFile::Wallet),
        );
        let error = {
            let mut session = LocalStateSession::acquire(directory.path(), &mut NoopHook)?;
            session
                .commit_with_hook(new_write_set(), || Ok(()), &mut hook)
                .err()
                .context("rollback fault should fail")?
        };
        let rendered = format!("{error:#?} {error:#}");
        if rendered.contains("wallet-private-sentinel-never-log") {
            bail!("wallet bytes leaked through transaction error");
        }
        with_local_state_in(directory.path(), |_| Ok(()))?;
        if directory.path().join("settings.json").exists() {
            bail!("rollback materialized originally missing settings");
        }
        if fs::read(directory.path().join("wallet.json"))? != secret {
            bail!("rollback did not restore exact wallet bytes");
        }
        Ok(())
    }

    #[test]
    fn every_missing_target_remove_failure_recovers_exact_absence() -> Result<()> {
        for missing in StateFile::ALL {
            let directory = tempfile::tempdir().context("failed to create local state test dir")?;
            for file in StateFile::ALL {
                if file != missing {
                    fs::write(directory.path().join(file.file_name()), old_bytes(file))?;
                }
            }
            let mut hook = InjectHook::failure_then_rollback(
                IoPoint::CommitDirectorySync,
                IoPoint::RollbackRemove(missing),
            );
            let error = {
                let mut session = LocalStateSession::acquire(directory.path(), &mut NoopHook)?;
                session
                    .commit_with_hook(new_write_set(), || Ok(()), &mut hook)
                    .err()
                    .context("missing-target rollback fault should fail")?
            };
            if !error.to_string().contains("recovery_required")
                || !directory.path().join(JOURNAL_FILE_NAME).is_file()
            {
                bail!("missing {missing:?} rollback did not retain recovery gate: {error:#}");
            }
            with_local_state_in(directory.path(), |_| Ok(()))?;
            for file in StateFile::ALL {
                let path = directory.path().join(file.file_name());
                if file == missing {
                    if path_present(&path)? {
                        bail!("hot recovery materialized originally missing {file:?}");
                    }
                } else if fs::read(&path)? != old_bytes(file) {
                    bail!("hot recovery changed original {file:?} bytes");
                }
            }
        }
        Ok(())
    }

    #[test]
    fn journal_reinstall_double_fault_poison_gates_followup_access() -> Result<()> {
        for (primary, cleanup) in [
            (IoPoint::CommitDirectorySync, None),
            (
                IoPoint::TargetPersist(StateFile::Idl),
                Some(IoPoint::RollbackJournalDirectorySync),
            ),
        ] {
            let directory = seeded_directory()?;
            let mut injections = vec![(HookMoment::Before, primary, InjectionKind::Failure)];
            if let Some(cleanup) = cleanup {
                injections.push((HookMoment::Before, cleanup, InjectionKind::Failure));
            }
            injections.push((
                HookMoment::Before,
                IoPoint::JournalReinstall,
                InjectionKind::Failure,
            ));
            let mut hook = InjectHook::new(injections);
            let error = {
                let mut session = LocalStateSession::acquire(directory.path(), &mut NoopHook)?;
                session
                    .commit_with_hook(new_write_set(), || Ok(()), &mut hook)
                    .err()
                    .context("journal reinstall double fault should fail")?
            };
            let transaction = error
                .downcast_ref::<LocalStateTransactionError>()
                .context("double fault should return a typed transaction error")?;
            if transaction.status() != LocalStateFailureStatus::RecoveryRequired
                || path_present(&directory.path().join(JOURNAL_FILE_NAME))?
            {
                bail!("double fault did not enter recovery gate: {error:#}");
            }
            let followup = with_local_state_in(directory.path(), |_| Ok(()))
                .err()
                .context("process recovery poison should gate followup access")?;
            if !followup.to_string().contains("recovery_required") {
                bail!("followup access escaped process recovery gate: {followup:#}");
            }
        }
        Ok(())
    }

    #[test]
    fn failed_journal_inspection_after_partial_replace_poison_gates_access() -> Result<()> {
        let directory = seeded_directory()?;
        let mut hook = InjectHook::new(vec![
            (
                HookMoment::Before,
                IoPoint::TargetPersist(StateFile::Idl),
                InjectionKind::Failure,
            ),
            (
                HookMoment::Before,
                IoPoint::JournalInspect,
                InjectionKind::Failure,
            ),
        ]);
        let error = {
            let mut session = LocalStateSession::acquire(directory.path(), &mut NoopHook)?;
            session
                .commit_with_hook(new_write_set(), || Ok(()), &mut hook)
                .err()
                .context("journal inspection fault should fail transaction")?
        };
        let transaction = error
            .downcast_ref::<LocalStateTransactionError>()
            .context("journal inspection fault should return typed transaction error")?;
        if transaction.status() != LocalStateFailureStatus::RecoveryRequired
            || !path_present(&directory.path().join(JOURNAL_FILE_NAME))?
            || fs::read(directory.path().join("settings.json"))? != NEW_SETTINGS
            || fs::read(directory.path().join("idls.json"))? != OLD_IDL
        {
            bail!("journal inspection fault did not preserve gated mixed state: {error:#}");
        }
        let followup = with_local_state_in(directory.path(), |_| Ok(()))
            .err()
            .context("journal inspection poison should gate followup access")?;
        if !followup.to_string().contains("recovery_required") {
            bail!("journal inspection poison did not gate followup access: {followup:#}");
        }
        Ok(())
    }

    #[test]
    fn foreign_target_state_preserves_hot_journal_and_gates_access() -> Result<()> {
        let directory = seeded_directory()?;
        let mut hook = InjectHook::crash_after(IoPoint::TargetPersist(StateFile::Settings));
        {
            let mut session = LocalStateSession::acquire(directory.path(), &mut NoopHook)?;
            let error = session
                .commit_with_hook(new_write_set(), || Ok(()), &mut hook)
                .err()
                .context("simulated crash should interrupt transaction")?;
            if !is_simulated_crash(&error) {
                bail!("unexpected simulated crash error: {error:#}");
            }
        }
        fs::write(
            directory.path().join("settings.json"),
            b"foreign-third-state",
        )
        .context("failed to inject foreign state")?;
        let error = with_local_state_in(directory.path(), |_| Ok(()))
            .err()
            .context("foreign state should gate recovery")?;
        if !error.to_string().contains("recovery_required") {
            bail!("foreign state did not report RecoveryRequired: {error:#}");
        }
        if !directory.path().join(JOURNAL_FILE_NAME).is_file() {
            bail!("foreign state removed hot journal");
        }
        Ok(())
    }

    #[test]
    fn malformed_hot_journals_remain_byte_exact_and_gate_access() -> Result<()> {
        let valid = valid_journal_value();
        let mut wrong_version = valid.clone();
        *wrong_version
            .get_mut("schema_version")
            .context("valid journal schema version is missing")? = serde_json::json!(2);

        let mut duplicate_target = valid.clone();
        let duplicate = duplicate_target
            .pointer("/entries/0")
            .cloned()
            .context("valid journal entry is missing")?;
        duplicate_target
            .get_mut("entries")
            .and_then(serde_json::Value::as_array_mut)
            .context("valid journal entries are missing")?
            .push(duplicate);

        let mut mismatched_original = valid;
        *mismatched_original
            .pointer_mut("/entries/0/old_sha256")
            .context("valid journal original checksum is missing")? =
            serde_json::json!(sha256_hex(b"other"));

        let cases = [
            ("invalid_json", b"{".to_vec()),
            ("wrong_version", serde_json::to_vec(&wrong_version)?),
            ("duplicate_target", serde_json::to_vec(&duplicate_target)?),
            (
                "original_checksum_mismatch",
                serde_json::to_vec(&mismatched_original)?,
            ),
        ];
        for (name, bytes) in cases {
            let directory = seeded_directory()?;
            let journal_path = directory.path().join(JOURNAL_FILE_NAME);
            fs::write(&journal_path, &bytes)?;
            let error = with_local_state_in(directory.path(), |_| Ok(()))
                .err()
                .with_context(|| format!("malformed journal `{name}` should gate access"))?;
            let transaction = error
                .downcast_ref::<LocalStateTransactionError>()
                .with_context(|| format!("malformed journal `{name}` returned an untyped error"))?;
            if transaction.status() != LocalStateFailureStatus::RecoveryRequired {
                bail!("malformed journal `{name}` did not require recovery: {error:#}");
            }
            if fs::read(&journal_path)? != bytes {
                bail!("malformed journal `{name}` was modified during validation");
            }
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn dangling_hot_journal_symlink_is_retained_and_gates_access() -> Result<()> {
        use std::os::unix::fs::symlink;

        let directory = seeded_directory()?;
        let journal_path = directory.path().join(JOURNAL_FILE_NAME);
        symlink(
            directory.path().join("missing-journal-target"),
            &journal_path,
        )?;
        let error = with_local_state_in(directory.path(), |_| Ok(()))
            .err()
            .context("dangling hot journal symlink should gate access")?;
        let transaction = error
            .downcast_ref::<LocalStateTransactionError>()
            .context("dangling journal should return typed transaction error")?;
        if transaction.status() != LocalStateFailureStatus::RecoveryRequired
            || !fs::symlink_metadata(&journal_path)?
                .file_type()
                .is_symlink()
        {
            bail!("dangling journal symlink did not retain recovery gate: {error:#}");
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn durable_hot_journal_is_private() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = seeded_directory()?;
        let mut hook = InjectHook::crash_after(IoPoint::JournalDirectorySync);
        {
            let mut session = LocalStateSession::acquire(directory.path(), &mut NoopHook)?;
            let error = session
                .commit_with_hook(new_write_set(), || Ok(()), &mut hook)
                .err()
                .context("journal crash boundary should interrupt commit")?;
            if !is_simulated_crash(&error) {
                bail!("journal crash boundary returned another error: {error:#}");
            }
        }
        let journal_path = directory.path().join(JOURNAL_FILE_NAME);
        let mode = fs::metadata(&journal_path)?.permissions().mode();
        if mode & 0o077 != 0 {
            bail!("hot journal permissions expose private state: {mode:o}");
        }
        with_local_state_in(directory.path(), |_| Ok(()))?;
        assert_triple(directory.path(), OLD_SETTINGS, OLD_IDL, OLD_WALLET)
    }

    #[test]
    fn concurrent_snapshot_waits_for_whole_transaction() -> Result<()> {
        let directory = seeded_directory()?;
        let base_dir = directory.path().to_path_buf();
        let (entered_sender, entered_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        let writer_dir = base_dir.clone();
        let writer = thread::spawn(move || -> Result<()> {
            let mut hook = PauseHook {
                point: IoPoint::TargetPersist(StateFile::Settings),
                entered: Some(entered_sender),
                release: release_receiver,
            };
            let mut session = LocalStateSession::acquire(&writer_dir, &mut NoopHook)?;
            session
                .commit_with_hook(new_write_set(), || Ok(()), &mut hook)
                .map(|_| ())
        });
        entered_receiver
            .recv()
            .context("writer did not reach mixed-state boundary")?;

        let reader_dir = base_dir.clone();
        let (reader_sender, reader_receiver) = mpsc::channel();
        let reader = thread::spawn(move || -> Result<()> {
            let snapshot = with_local_state_in(&reader_dir, |session| session.snapshot())?;
            reader_sender
                .send(snapshot)
                .map_err(|_| anyhow::anyhow!("failed to return snapshot"))
        });
        if reader_receiver
            .recv_timeout(Duration::from_millis(100))
            .is_ok()
        {
            bail!("reader observed transaction while writer held shared lock");
        }
        release_sender
            .send(())
            .map_err(|_| anyhow::anyhow!("failed to release writer"))?;
        writer
            .join()
            .map_err(|_| anyhow::anyhow!("writer thread panicked"))??;
        let snapshot = reader_receiver
            .recv_timeout(Duration::from_secs(2))
            .context("reader did not resume after transaction")?;
        reader
            .join()
            .map_err(|_| anyhow::anyhow!("reader thread panicked"))??;
        if snapshot.settings != StoredBytes::Present(NEW_SETTINGS.to_vec())
            || snapshot.idl != StoredBytes::Present(NEW_IDL.to_vec())
            || snapshot.wallet != StoredBytes::Present(NEW_WALLET.to_vec())
        {
            bail!("reader did not observe one whole committed triple: {snapshot:?}");
        }
        Ok(())
    }

    #[test]
    fn cross_process_reader_waits_for_file_locked_transaction() -> Result<()> {
        let current_exe = env::current_exe().context("failed to locate test executable")?;
        let directory = seeded_directory()?;
        let base_dir = directory.path().to_path_buf();
        let ready_path = base_dir.join("reader-ready");
        let (entered_sender, entered_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        let writer_dir = base_dir.clone();
        let writer = thread::spawn(move || -> Result<()> {
            let mut hook = PauseHook {
                point: IoPoint::TargetPersist(StateFile::Settings),
                entered: Some(entered_sender),
                release: release_receiver,
            };
            let mut session = LocalStateSession::acquire(&writer_dir, &mut NoopHook)?;
            session
                .commit_with_hook(new_write_set(), || Ok(()), &mut hook)
                .map(|_| ())
        });
        entered_receiver
            .recv()
            .context("writer did not reach mixed-state file-lock boundary")?;

        let mut child = Command::new(&current_exe)
            .arg("--exact")
            .arg("support::local_state::tests::file_lock_reader_worker")
            .arg("--nocapture")
            .env(FILE_LOCK_READER_ENV, "1")
            .env(CRASH_DIR_ENV, &base_dir)
            .spawn()
            .context("failed to launch file-lock reader worker")?;
        let ready_deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !ready_path.is_file() {
            if let Some(status) = child.try_wait()? {
                release_sender
                    .send(())
                    .map_err(|_| anyhow::anyhow!("failed to release writer"))?;
                writer
                    .join()
                    .map_err(|_| anyhow::anyhow!("writer thread panicked"))??;
                bail!("file-lock reader exited before acquisition attempt: {status}");
            }
            if std::time::Instant::now() >= ready_deadline {
                child
                    .kill()
                    .context("failed to stop stalled reader worker")?;
                let _status = child.wait();
                release_sender
                    .send(())
                    .map_err(|_| anyhow::anyhow!("failed to release writer"))?;
                writer
                    .join()
                    .map_err(|_| anyhow::anyhow!("writer thread panicked"))??;
                bail!("file-lock reader did not report acquisition attempt");
            }
            thread::sleep(Duration::from_millis(10));
        }
        thread::sleep(Duration::from_millis(100));
        let early_status = child.try_wait()?;
        release_sender
            .send(())
            .map_err(|_| anyhow::anyhow!("failed to release writer"))?;
        writer
            .join()
            .map_err(|_| anyhow::anyhow!("writer thread panicked"))??;
        if let Some(status) = early_status {
            bail!("cross-process reader escaped file lock during mixed state: {status}");
        }
        let status = child.wait().context("failed to join file-lock reader")?;
        if !status.success() {
            bail!("file-lock reader failed after writer release: {status}");
        }
        fs::remove_file(&ready_path).context("failed to remove reader-ready marker")?;
        Ok(())
    }

    #[test]
    fn file_lock_reader_worker() -> Result<()> {
        if env::var_os(FILE_LOCK_READER_ENV).is_none() {
            return Ok(());
        }
        let directory = env::var_os(CRASH_DIR_ENV)
            .map(PathBuf::from)
            .context("file-lock reader directory is missing")?;
        fs::write(directory.join("reader-ready"), b"ready")?;
        let snapshot = with_local_state_in(&directory, |session| session.snapshot())?;
        if snapshot.settings != StoredBytes::Present(NEW_SETTINGS.to_vec())
            || snapshot.idl != StoredBytes::Present(NEW_IDL.to_vec())
            || snapshot.wallet != StoredBytes::Present(NEW_WALLET.to_vec())
        {
            bail!("cross-process reader observed a mixed local-state snapshot");
        }
        Ok(())
    }

    #[test]
    fn durable_crash_boundaries_recover_to_all_old_or_all_new_after_process_restart() -> Result<()>
    {
        let current_exe = env::current_exe().context("failed to locate test executable")?;
        for (point_name, expect_new) in [
            ("settings_stage_create", false),
            ("settings_stage_write", false),
            ("settings_stage_file_sync", false),
            ("idl_stage_create", false),
            ("idl_stage_write", false),
            ("idl_stage_file_sync", false),
            ("wallet_stage_create", false),
            ("wallet_stage_write", false),
            ("wallet_stage_file_sync", false),
            ("journal_create", false),
            ("journal_write", false),
            ("journal_file_sync", false),
            ("journal_persist", false),
            ("journal_directory_sync", false),
            ("settings_persist", false),
            ("settings_directory_sync", false),
            ("settings_verify", false),
            ("idl_persist", false),
            ("idl_directory_sync", false),
            ("idl_verify", false),
            ("wallet_persist", false),
            ("wallet_directory_sync", false),
            ("wallet_verify", false),
            ("journal_remove", true),
            ("commit_directory_sync", true),
        ] {
            let directory = seeded_directory()?;
            run_crash_worker(&current_exe, point_name, directory.path())?;
            with_local_state_in(directory.path(), |_| Ok(()))?;
            let expected = if expect_new {
                (NEW_SETTINGS, NEW_IDL, NEW_WALLET)
            } else {
                (OLD_SETTINGS, OLD_IDL, OLD_WALLET)
            };
            assert_triple(directory.path(), expected.0, expected.1, expected.2)
                .with_context(|| format!("unexpected restart state at {point_name}"))?;
        }
        Ok(())
    }

    #[test]
    fn rollback_crash_boundaries_recover_after_process_restart() -> Result<()> {
        let current_exe = env::current_exe().context("failed to locate test executable")?;
        for point_name in [
            "rollback_wallet_file_sync",
            "rollback_wallet_persist",
            "rollback_wallet_directory_sync",
            "rollback_idl_file_sync",
            "rollback_idl_persist",
            "rollback_idl_directory_sync",
            "rollback_settings_file_sync",
            "rollback_settings_persist",
            "rollback_settings_directory_sync",
            "rollback_journal_remove",
            "rollback_journal_directory_sync",
        ] {
            let directory = seeded_directory()?;
            run_crash_worker(&current_exe, point_name, directory.path())?;
            with_local_state_in(directory.path(), |_| Ok(()))?;
            assert_triple(directory.path(), OLD_SETTINGS, OLD_IDL, OLD_WALLET)
                .with_context(|| format!("rollback restart did not converge at {point_name}"))?;
        }

        for missing in StateFile::ALL {
            for boundary in ["remove", "directory_sync"] {
                let directory = tempfile::tempdir()
                    .context("failed to create missing-state crash directory")?;
                for file in StateFile::ALL {
                    if file != missing {
                        fs::write(directory.path().join(file.file_name()), old_bytes(file))?;
                    }
                }
                let point_name = format!(
                    "rollback_missing_{}_{}",
                    missing.file_name().trim_end_matches(".json"),
                    boundary
                );
                run_crash_worker(&current_exe, &point_name, directory.path())?;
                with_local_state_in(directory.path(), |_| Ok(()))?;
                for file in StateFile::ALL {
                    let path = directory.path().join(file.file_name());
                    if file == missing {
                        if path_present(&path)? {
                            bail!("rollback restart materialized missing {file:?} at {point_name}");
                        }
                    } else if fs::read(&path)? != old_bytes(file) {
                        bail!("rollback restart changed {file:?} at {point_name}");
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn crash_worker() -> Result<()> {
        if env::var_os(CRASH_WORKER_ENV).is_none() {
            return Ok(());
        }
        let point_name = env::var(CRASH_POINT_ENV).context("crash point is missing")?;
        let directory = env::var_os(CRASH_DIR_ENV)
            .map(PathBuf::from)
            .context("crash directory is missing")?;
        let mut hook = crash_hook(&point_name)?;
        let mut session = LocalStateSession::acquire(&directory, &mut NoopHook)?;
        let result = session.commit_with_hook(new_write_set(), || Ok(()), &mut hook);
        bail!("crash point was not reached: {result:?}")
    }

    fn run_crash_worker(current_exe: &Path, point_name: &str, directory: &Path) -> Result<()> {
        let output = Command::new(current_exe)
            .arg("--exact")
            .arg("support::local_state::tests::crash_worker")
            .arg("--nocapture")
            .env(CRASH_WORKER_ENV, "1")
            .env(CRASH_POINT_ENV, point_name)
            .env(CRASH_DIR_ENV, directory)
            .output()
            .context("failed to launch local state crash worker")?;
        if output.status.code() != Some(90) {
            bail!(
                "crash worker failed at {point_name}: status={:?}, stdout={}, stderr={}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }

    fn crash_hook(name: &str) -> Result<InjectHook> {
        let rollback_point = match name {
            "rollback_wallet_file_sync" => Some(IoPoint::RollbackFileSync(StateFile::Wallet)),
            "rollback_wallet_persist" => Some(IoPoint::RollbackPersist(StateFile::Wallet)),
            "rollback_wallet_directory_sync" => {
                Some(IoPoint::RollbackDirectorySync(StateFile::Wallet))
            }
            "rollback_idl_file_sync" => Some(IoPoint::RollbackFileSync(StateFile::Idl)),
            "rollback_idl_persist" => Some(IoPoint::RollbackPersist(StateFile::Idl)),
            "rollback_idl_directory_sync" => Some(IoPoint::RollbackDirectorySync(StateFile::Idl)),
            "rollback_settings_file_sync" => Some(IoPoint::RollbackFileSync(StateFile::Settings)),
            "rollback_settings_persist" => Some(IoPoint::RollbackPersist(StateFile::Settings)),
            "rollback_settings_directory_sync" => {
                Some(IoPoint::RollbackDirectorySync(StateFile::Settings))
            }
            "rollback_missing_settings_remove" => {
                Some(IoPoint::RollbackRemove(StateFile::Settings))
            }
            "rollback_missing_settings_directory_sync" => {
                Some(IoPoint::RollbackDirectorySync(StateFile::Settings))
            }
            "rollback_missing_idls_remove" => Some(IoPoint::RollbackRemove(StateFile::Idl)),
            "rollback_missing_idls_directory_sync" => {
                Some(IoPoint::RollbackDirectorySync(StateFile::Idl))
            }
            "rollback_missing_wallet_remove" => Some(IoPoint::RollbackRemove(StateFile::Wallet)),
            "rollback_missing_wallet_directory_sync" => {
                Some(IoPoint::RollbackDirectorySync(StateFile::Wallet))
            }
            "rollback_journal_remove" => Some(IoPoint::RollbackJournalRemove),
            "rollback_journal_directory_sync" => Some(IoPoint::RollbackJournalDirectorySync),
            _ => None,
        };
        if let Some(point) = rollback_point {
            return Ok(InjectHook::new(vec![
                (
                    HookMoment::Before,
                    IoPoint::TargetPersist(StateFile::Wallet),
                    InjectionKind::Failure,
                ),
                (HookMoment::After, point, InjectionKind::ProcessExit),
            ]));
        }
        crash_point(name).map(InjectHook::process_exit_after)
    }

    fn crash_point(name: &str) -> Result<IoPoint> {
        match name {
            "settings_stage_create" => Ok(IoPoint::StageCreate(StateFile::Settings)),
            "settings_stage_write" => Ok(IoPoint::StageWrite(StateFile::Settings)),
            "settings_stage_file_sync" => Ok(IoPoint::StageFileSync(StateFile::Settings)),
            "idl_stage_create" => Ok(IoPoint::StageCreate(StateFile::Idl)),
            "idl_stage_write" => Ok(IoPoint::StageWrite(StateFile::Idl)),
            "idl_stage_file_sync" => Ok(IoPoint::StageFileSync(StateFile::Idl)),
            "wallet_stage_create" => Ok(IoPoint::StageCreate(StateFile::Wallet)),
            "wallet_stage_write" => Ok(IoPoint::StageWrite(StateFile::Wallet)),
            "wallet_stage_file_sync" => Ok(IoPoint::StageFileSync(StateFile::Wallet)),
            "journal_create" => Ok(IoPoint::JournalCreate),
            "journal_write" => Ok(IoPoint::JournalWrite),
            "journal_file_sync" => Ok(IoPoint::JournalFileSync),
            "journal_persist" => Ok(IoPoint::JournalPersist),
            "journal_directory_sync" => Ok(IoPoint::JournalDirectorySync),
            "settings_persist" => Ok(IoPoint::TargetPersist(StateFile::Settings)),
            "settings_directory_sync" => Ok(IoPoint::TargetDirectorySync(StateFile::Settings)),
            "settings_verify" => Ok(IoPoint::TargetVerify(StateFile::Settings)),
            "idl_persist" => Ok(IoPoint::TargetPersist(StateFile::Idl)),
            "idl_directory_sync" => Ok(IoPoint::TargetDirectorySync(StateFile::Idl)),
            "idl_verify" => Ok(IoPoint::TargetVerify(StateFile::Idl)),
            "wallet_persist" => Ok(IoPoint::TargetPersist(StateFile::Wallet)),
            "wallet_directory_sync" => Ok(IoPoint::TargetDirectorySync(StateFile::Wallet)),
            "wallet_verify" => Ok(IoPoint::TargetVerify(StateFile::Wallet)),
            "journal_remove" => Ok(IoPoint::JournalRemove),
            "commit_directory_sync" => Ok(IoPoint::CommitDirectorySync),
            _ => bail!("unsupported crash point"),
        }
    }

    fn seeded_directory() -> Result<tempfile::TempDir> {
        let directory = tempfile::tempdir().context("failed to create local state test dir")?;
        seed_triple(directory.path())?;
        Ok(directory)
    }

    fn seed_triple(directory: &Path) -> Result<()> {
        fs::write(directory.join("settings.json"), OLD_SETTINGS)
            .context("failed to seed settings state")?;
        fs::write(directory.join("idls.json"), OLD_IDL).context("failed to seed IDL state")?;
        fs::write(directory.join("wallet.json"), OLD_WALLET)
            .context("failed to seed wallet state")?;
        let journal_path = directory.join(JOURNAL_FILE_NAME);
        if journal_path.exists() {
            fs::remove_file(journal_path).context("failed to remove stale test journal")?;
        }
        Ok(())
    }

    fn new_write_set() -> LocalStateWriteSet {
        LocalStateWriteSet::new()
            .settings(NEW_SETTINGS.to_vec())
            .idl(NEW_IDL.to_vec())
            .wallet(NEW_WALLET.to_vec())
    }

    fn old_bytes(file: StateFile) -> &'static [u8] {
        match file {
            StateFile::Settings => OLD_SETTINGS,
            StateFile::Idl => OLD_IDL,
            StateFile::Wallet => OLD_WALLET,
        }
    }

    fn valid_journal_value() -> serde_json::Value {
        serde_json::json!({
            "schema_version": JOURNAL_SCHEMA_VERSION,
            "transaction_id": "00000000000000000000000000000000",
            "entries": [{
                "file": "settings",
                "original": {
                    "kind": "present",
                    "bytes_base64": BASE64_STANDARD.encode(OLD_SETTINGS),
                },
                "old_sha256": sha256_hex(OLD_SETTINGS),
                "new_sha256": sha256_hex(NEW_SETTINGS),
            }],
        })
    }

    fn assert_triple(directory: &Path, settings: &[u8], idl: &[u8], wallet: &[u8]) -> Result<()> {
        if fs::read(directory.join("settings.json"))? != settings
            || fs::read(directory.join("idls.json"))? != idl
            || fs::read(directory.join("wallet.json"))? != wallet
        {
            bail!("local state triple differs from expected values");
        }
        Ok(())
    }
}
