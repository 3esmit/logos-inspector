use std::{
    fmt,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, bail};
use serde_json::{Map, Value, json};
use tokio::runtime::Runtime;

use super::{
    RuntimeOperationRequest, RuntimeOperations,
    record::{RuntimeOperationRecord, RuntimeOperationStatus, runtime_operation_value},
    spec::OperationMethod,
};
use crate::support::{
    backup_catalog::{
        BackupCatalogId, LocalBackupImportReceipt, preview_local_settings_restore_with_options,
        restore_local_settings_backup_with_options,
    },
    local_state::{
        DirectoryDurability, LocalStateCommitCancellation, LocalStateFailureStatus,
        LocalStateTransactionError,
    },
    settings_backup::{
        BackupImportArea, BackupImportOptions, BackupImportSelection, SettingsBackupCommitResult,
    },
};

#[cfg(test)]
use crate::support::local_state::LOCAL_STATE_TRANSACTION_ID_HEX_LENGTH;

#[cfg(not(test))]
const STOP_TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(test)]
const STOP_TIMEOUT: Duration = Duration::from_millis(25);
#[cfg(not(test))]
const CANCELLATION_RECONCILIATION_TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(test)]
const CANCELLATION_RECONCILIATION_TIMEOUT: Duration = Duration::from_millis(100);
const STOP_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(super) trait BackupImportStore: Send + Sync {
    fn preview(
        &self,
        backup_catalog_id: &str,
        wallet_profile: Option<&Value>,
        options: &BackupImportOptions,
    ) -> Result<Value>;

    fn restore(
        &self,
        backup_catalog_id: &str,
        wallet_profile: Option<&Value>,
        options: &BackupImportOptions,
        cancellation: &LocalStateCommitCancellation,
    ) -> Result<BackupImportCommitReceipt>;
}

#[derive(Debug)]
pub(super) struct BackupImportCommitReceipt {
    backup_catalog_id: BackupCatalogId,
    selection: BackupImportSelection,
    applied_areas: Vec<BackupImportArea>,
    commit: BackupImportCommitResult,
    summary: Map<String, Value>,
}

#[derive(Debug)]
enum BackupImportCommitResult {
    NoOp,
    Applied {
        transaction_id: String,
        directory_durability: DirectoryDurability,
    },
}

impl BackupImportCommitReceipt {
    fn from_local(receipt: LocalBackupImportReceipt) -> Self {
        let (backup_catalog_id, selection, restore) = receipt.into_parts();
        let (summary, applied_areas, commit) = restore.into_parts();
        let commit = match commit {
            SettingsBackupCommitResult::NoOp => BackupImportCommitResult::NoOp,
            SettingsBackupCommitResult::Applied(report) => BackupImportCommitResult::Applied {
                transaction_id: report.transaction_id().to_owned(),
                directory_durability: report.directory_durability(),
            },
        };
        Self {
            backup_catalog_id,
            selection,
            applied_areas,
            commit,
            summary,
        }
    }

    fn validate_for(
        &self,
        backup_catalog_id: &BackupCatalogId,
        selection: &BackupImportSelection,
    ) -> Result<()> {
        if &self.backup_catalog_id != backup_catalog_id {
            bail!("backup import receipt has a different backup catalog identity");
        }
        if &self.selection != selection {
            bail!("backup import receipt has a different area selection");
        }
        Ok(())
    }

    fn into_summary(mut self) -> Value {
        let selected = area_value(self.selection.selected_areas());
        let applied = area_value(self.applied_areas);
        self.summary.insert(
            "backup_catalog_id".to_owned(),
            Value::String(self.backup_catalog_id.as_str().to_owned()),
        );
        self.summary
            .insert("selected_areas".to_owned(), selected.clone());
        self.summary.insert("affected_areas".to_owned(), selected);
        self.summary.insert("applied_areas".to_owned(), applied);
        match self.commit {
            BackupImportCommitResult::NoOp => {
                self.summary
                    .insert("restored".to_owned(), Value::Bool(false));
                self.summary.remove("transaction");
            }
            BackupImportCommitResult::Applied {
                transaction_id,
                directory_durability,
            } => {
                self.summary
                    .insert("restored".to_owned(), Value::Bool(true));
                self.summary.insert(
                    "transaction".to_owned(),
                    json!({
                        "transaction_id": transaction_id,
                        "status": "applied",
                        "directory_durability": directory_durability.as_str(),
                    }),
                );
            }
        }
        Value::Object(self.summary)
    }

    #[cfg(test)]
    fn try_from_adapter_parts(
        backup_catalog_id: &str,
        selection: BackupImportSelection,
        applied_areas: Vec<BackupImportArea>,
        summary: Value,
        transaction_id: Option<&str>,
        directory_durability: Option<DirectoryDurability>,
    ) -> Result<Self> {
        let parsed_catalog_id = BackupCatalogId::parse(backup_catalog_id)?;
        if parsed_catalog_id.as_str() != backup_catalog_id {
            bail!("backup import receipt catalog identity is not canonical");
        }
        let summary = summary
            .as_object()
            .cloned()
            .context("backup import receipt summary must be a JSON object")?;
        let mut unique = Vec::with_capacity(applied_areas.len());
        for area in &applied_areas {
            if !selection.mode(*area).is_selected() {
                bail!("backup import receipt applied an unselected area");
            }
            if unique.contains(area) {
                bail!("backup import receipt repeats an applied area");
            }
            unique.push(*area);
        }
        let commit = match (transaction_id, directory_durability) {
            (None, None) if applied_areas.is_empty() => BackupImportCommitResult::NoOp,
            (Some(transaction_id), Some(directory_durability)) if !applied_areas.is_empty() => {
                if transaction_id.len() != LOCAL_STATE_TRANSACTION_ID_HEX_LENGTH
                    || !transaction_id.bytes().all(|byte| byte.is_ascii_hexdigit())
                {
                    bail!("backup import receipt transaction identity is invalid");
                }
                BackupImportCommitResult::Applied {
                    transaction_id: transaction_id.to_owned(),
                    directory_durability,
                }
            }
            (Some(_), None) => bail!("backup import receipt directory durability is missing"),
            (None, Some(_)) => bail!("backup import receipt transaction identity is missing"),
            _ => bail!("backup import receipt transaction evidence conflicts with applied areas"),
        };
        Ok(Self {
            backup_catalog_id: parsed_catalog_id,
            selection,
            applied_areas,
            commit,
            summary,
        })
    }
}

fn area_value(areas: Vec<BackupImportArea>) -> Value {
    Value::Array(
        areas
            .into_iter()
            .map(|area| Value::String(area.as_str().to_owned()))
            .collect(),
    )
}

#[derive(Debug)]
pub(super) struct LocalBackupImportStore;

impl BackupImportStore for LocalBackupImportStore {
    fn preview(
        &self,
        backup_catalog_id: &str,
        wallet_profile: Option<&Value>,
        options: &BackupImportOptions,
    ) -> Result<Value> {
        preview_local_settings_restore_with_options(backup_catalog_id, wallet_profile, options)
    }

    fn restore(
        &self,
        backup_catalog_id: &str,
        wallet_profile: Option<&Value>,
        options: &BackupImportOptions,
        cancellation: &LocalStateCommitCancellation,
    ) -> Result<BackupImportCommitReceipt> {
        restore_local_settings_backup_with_options(
            backup_catalog_id,
            wallet_profile,
            options,
            cancellation,
        )
        .map(BackupImportCommitReceipt::from_local)
    }
}

pub(super) struct BackupImportCoordinator {
    store: Arc<dyn BackupImportStore>,
    gate: Mutex<ImportGate>,
    next_import_sequence: AtomicU64,
}

impl fmt::Debug for BackupImportCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackupImportCoordinator")
            .finish_non_exhaustive()
    }
}

impl BackupImportCoordinator {
    pub(super) fn new(store: Arc<dyn BackupImportStore>) -> Self {
        Self {
            store,
            gate: Mutex::new(ImportGate::Idle),
            next_import_sequence: AtomicU64::new(1),
        }
    }

    pub(super) fn operation_permit(
        &self,
        request: &RuntimeOperationRequest,
    ) -> Result<BackupOperationPermit<'_>> {
        let active = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("backup import state is unavailable"))?;
        if matches!(&*active, ImportGate::Closing) {
            bail!("backup import coordinator is closing");
        }
        if active
            .selection()
            .is_some_and(|selection| selection_affects(selection, request.method()))
        {
            bail!(
                "operation `{}` is blocked while affected settings are being imported",
                request.method_name()
            );
        }
        Ok(BackupOperationPermit { _active: active })
    }

    pub(super) fn preview(
        &self,
        operations: &RuntimeOperations,
        backup_catalog_id: &str,
        wallet_profile: Option<&Value>,
        options: Option<&Value>,
    ) -> Result<Value> {
        let backup_catalog_id = BackupCatalogId::parse(backup_catalog_id)?;
        let options = BackupImportOptions::parse(options)?;
        let selection = options.selection().clone();
        let summary = self
            .store
            .preview(backup_catalog_id.as_str(), wallet_profile, &options)?;
        build_plan(
            operations,
            backup_catalog_id.as_str(),
            selection,
            summary,
            Vec::new(),
        )
    }

    pub(super) fn apply(
        &self,
        runtime: &Runtime,
        operations: &RuntimeOperations,
        backup_catalog_id: &str,
        wallet_profile: Option<&Value>,
        options: Option<&Value>,
    ) -> Result<Value> {
        let backup_catalog_id = BackupCatalogId::parse(backup_catalog_id)?;
        let options = BackupImportOptions::parse(options)?;
        let selection = options.selection().clone();
        if options.is_empty() {
            bail!("select at least one backup section to import");
        }

        let mut lease = ImportLease::acquire(&self.gate, selection.clone())?;
        let cancellation = lease.cancellation().clone();
        let import_id = self.allocate_import_id(backup_catalog_id.as_str())?;
        let preview = match self
            .store
            .preview(backup_catalog_id.as_str(), wallet_profile, &options)
        {
            Ok(preview) => preview,
            Err(error) => {
                let mut plan = BackupImportPlan::preparing(
                    backup_catalog_id.as_str(),
                    import_id,
                    selection,
                    preparation_summary(backup_catalog_id.as_str()),
                );
                plan.transition(BackupImportPhase::RolledBack)?;
                return plan.into_value(
                    BackupImportOutcome::RolledBack,
                    None,
                    Some(error.to_string()),
                );
            }
        };
        let mut plan = match BackupImportPlan::new(
            operations,
            backup_catalog_id.as_str(),
            import_id.clone(),
            selection.clone(),
            preview.clone(),
        ) {
            Ok(plan) => plan,
            Err(error) => {
                let failure_summary = if preview.is_object() {
                    preview
                } else {
                    preparation_summary(backup_catalog_id.as_str())
                };
                let mut plan = BackupImportPlan::preparing(
                    backup_catalog_id.as_str(),
                    import_id,
                    selection,
                    failure_summary,
                );
                plan.transition(BackupImportPhase::RolledBack)?;
                return plan.into_value(
                    BackupImportOutcome::RolledBack,
                    None,
                    Some(error.to_string()),
                );
            }
        };
        plan.transition(BackupImportPhase::Quiescing)?;
        if plan.blocked() {
            plan.record_block_events();
            plan.transition(BackupImportPhase::RolledBack)?;
            return plan.into_value(BackupImportOutcome::Blocked, None, None);
        }

        let mut stopped = StoppedOperationGuard::new(
            runtime,
            operations,
            &plan.import_id,
            &plan.backup_catalog_id,
        );
        let quiesce_deadline = Instant::now() + STOP_TIMEOUT;
        let mut requested_stops = Vec::new();
        for decision in plan.stop_decisions() {
            let operation_id = decision.operation_id.clone();
            let requested_by_coordinator = match operations.cancel_for_backup_import(&operation_id)
            {
                Ok(requested) => requested,
                Err(error) => {
                    plan.record_stop_error(decision, &error);
                    stopped.retain_for_recovery();
                    plan.transition(BackupImportPhase::RecoveryRequired)?;
                    lease.retain_recovery_required(Some(format!(
                        "coordinator:{}:quiesce",
                        plan.import_id
                    )))?;
                    return plan.into_value(
                        BackupImportOutcome::RecoveryRequired,
                        None,
                        Some(error.to_string()),
                    );
                }
            };
            requested_stops.push((decision, requested_by_coordinator));
        }

        let mut awaiting_reconciliation = Vec::new();
        for (decision, requested_by_coordinator) in requested_stops {
            let operation_id = decision.operation_id.clone();
            let remaining = quiesce_deadline.saturating_duration_since(Instant::now());
            match wait_until_terminal(operations, &operation_id, remaining) {
                Ok(Some(operation)) => {
                    let canceled_by_coordinator =
                        coordinator_owns_cancellation(requested_by_coordinator, &operation);
                    plan.record_terminal(decision.clone(), operation, canceled_by_coordinator);
                    if canceled_by_coordinator {
                        stopped.record(decision);
                    }
                }
                Err(error) => {
                    plan.record_stop_error(decision, &error);
                    stopped.retain_for_recovery();
                    plan.transition(BackupImportPhase::RecoveryRequired)?;
                    lease.retain_recovery_required(Some(format!(
                        "coordinator:{}:quiesce",
                        plan.import_id
                    )))?;
                    return plan.into_value(
                        BackupImportOutcome::RecoveryRequired,
                        None,
                        Some(error.to_string()),
                    );
                }
                Ok(None) => {
                    plan.record_stop_timeout(decision.clone());
                    awaiting_reconciliation.push((decision, requested_by_coordinator));
                }
            }
        }

        if !awaiting_reconciliation.is_empty() {
            let reconciliation_deadline = Instant::now() + CANCELLATION_RECONCILIATION_TIMEOUT;
            for (decision, requested_by_coordinator) in awaiting_reconciliation {
                let operation_id = decision.operation_id.clone();
                let remaining = reconciliation_deadline.saturating_duration_since(Instant::now());
                let operation = match wait_until_terminal(operations, &operation_id, remaining) {
                    Ok(Some(operation)) => operation,
                    Err(error) => {
                        plan.record_stop_error(decision, &error);
                        stopped.retain_for_recovery();
                        plan.transition(BackupImportPhase::RecoveryRequired)?;
                        lease.retain_recovery_required(Some(format!(
                            "coordinator:{}:quiesce",
                            plan.import_id
                        )))?;
                        return plan.into_value(
                            BackupImportOutcome::RecoveryRequired,
                            None,
                            Some(error.to_string()),
                        );
                    }
                    Ok(None) => {
                        stopped.retain_for_recovery();
                        plan.transition(BackupImportPhase::RecoveryRequired)?;
                        lease.retain_recovery_required(Some(format!(
                            "coordinator:{}:quiesce",
                            plan.import_id
                        )))?;
                        return plan.into_value(
                            BackupImportOutcome::RecoveryRequired,
                            None,
                            Some(
                                "affected operation cancellation requires reconciliation"
                                    .to_owned(),
                            ),
                        );
                    }
                };
                let canceled_by_coordinator =
                    coordinator_owns_cancellation(requested_by_coordinator, &operation);
                plan.record_terminal(decision.clone(), operation, canceled_by_coordinator);
                if canceled_by_coordinator {
                    stopped.record(decision);
                }
            }
            plan.events.extend(stopped.compensate());
            plan.transition(BackupImportPhase::RolledBack)?;
            return plan.into_value(
                BackupImportOutcome::Blocked,
                None,
                Some("timed out while quiescing affected operations".to_owned()),
            );
        }

        plan.transition(BackupImportPhase::Committing)?;
        lease.begin_committing(&plan.import_id);
        stopped.enter_committing();
        match self.store.restore(
            backup_catalog_id.as_str(),
            wallet_profile,
            &options,
            &cancellation,
        ) {
            Ok(receipt) => {
                if let Err(error) = receipt.validate_for(&backup_catalog_id, &selection) {
                    plan.transition(BackupImportPhase::RecoveryRequired)?;
                    stopped.retain_for_recovery();
                    lease.retain_recovery_required(Some(format!(
                        "coordinator:{}",
                        plan.import_id
                    )))?;
                    return plan.into_value(
                        BackupImportOutcome::RecoveryRequired,
                        None,
                        Some(error.to_string()),
                    );
                }
                let summary = receipt.into_summary();
                plan.transition(BackupImportPhase::Applied)?;
                plan.events.extend(stopped.compensate());
                let result = plan.into_value(BackupImportOutcome::Applied, Some(summary), None);
                if result.is_ok() {
                    lease.release_after_terminal();
                }
                drop(lease);
                result
            }
            Err(error) => match local_state_failure(&error) {
                Some(LocalStateFailureStatus::RolledBack) => {
                    plan.transition(BackupImportPhase::RolledBack)?;
                    plan.events.extend(stopped.compensate());
                    let result = plan.into_value(
                        BackupImportOutcome::RolledBack,
                        None,
                        Some(error.to_string()),
                    );
                    if result.is_ok() {
                        lease.release_after_terminal();
                    }
                    drop(lease);
                    result
                }
                Some(LocalStateFailureStatus::RecoveryRequired) => {
                    plan.transition(BackupImportPhase::RecoveryRequired)?;
                    stopped.retain_for_recovery();
                    lease.retain_recovery_required(local_state_transaction_id(&error))?;
                    plan.into_value(
                        BackupImportOutcome::RecoveryRequired,
                        None,
                        Some(error.to_string()),
                    )
                }
                None => {
                    plan.transition(BackupImportPhase::RolledBack)?;
                    plan.events.extend(stopped.compensate());
                    let failure = error
                        .context("backup import failed before a durable apply")
                        .to_string();
                    let result =
                        plan.into_value(BackupImportOutcome::RolledBack, None, Some(failure));
                    if result.is_ok() {
                        lease.release_after_terminal();
                    }
                    drop(lease);
                    result
                }
            },
        }
    }

    fn allocate_import_id(&self, backup_catalog_id: &str) -> Result<String> {
        let sequence = self
            .next_import_sequence
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| anyhow::anyhow!("backup import identity sequence is exhausted"))?;
        Ok(format!("backup_import:{sequence}:{backup_catalog_id}"))
    }

    pub(super) fn begin_close(&self) -> Result<()> {
        let mut gate = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("backup import state is unavailable"))?;
        match &mut *gate {
            ImportGate::Idle => *gate = ImportGate::Closing,
            ImportGate::Active {
                cancellation,
                closing,
                ..
            } => {
                cancellation.request();
                *closing = true;
            }
            ImportGate::RecoveryRequired { closing, .. } => *closing = true,
            ImportGate::Closing => {}
        }
        Ok(())
    }
}

pub(super) struct BackupOperationPermit<'a> {
    _active: MutexGuard<'a, ImportGate>,
}

struct ImportLease<'a> {
    gate: &'a Mutex<ImportGate>,
    cancellation: LocalStateCommitCancellation,
    release_on_drop: bool,
    fail_closed_transaction_id: Option<String>,
}

impl<'a> ImportLease<'a> {
    fn acquire(gate: &'a Mutex<ImportGate>, selection: BackupImportSelection) -> Result<Self> {
        let mut active = gate
            .lock()
            .map_err(|_| anyhow::anyhow!("backup import state is unavailable"))?;
        match &*active {
            ImportGate::Idle => {}
            ImportGate::Active { .. } => bail!("another backup import is already active"),
            ImportGate::Closing => bail!("backup import coordinator is closing"),
            ImportGate::RecoveryRequired { transaction_id, .. } => bail!(
                "backup import mutation is blocked while local transaction `{}` requires recovery",
                transaction_id.as_deref().unwrap_or("unknown")
            ),
        }
        let cancellation = LocalStateCommitCancellation::default();
        *active = ImportGate::Active {
            selection,
            cancellation: cancellation.clone(),
            closing: false,
        };
        Ok(Self {
            gate,
            cancellation,
            release_on_drop: true,
            fail_closed_transaction_id: None,
        })
    }

    fn cancellation(&self) -> &LocalStateCommitCancellation {
        &self.cancellation
    }

    fn begin_committing(&mut self, import_id: &str) {
        self.fail_closed_transaction_id = Some(format!("coordinator:{import_id}"));
    }

    fn release_after_terminal(&mut self) {
        self.fail_closed_transaction_id = None;
    }

    fn retain_recovery_required(&mut self, transaction_id: Option<String>) -> Result<()> {
        self.release_on_drop = false;
        let mut gate = self
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("backup import state is unavailable"))?;
        let ImportGate::Active {
            selection, closing, ..
        } = &*gate
        else {
            bail!("backup import recovery gate lost its active selection");
        };
        *gate = ImportGate::RecoveryRequired {
            selection: selection.clone(),
            transaction_id,
            closing: *closing,
        };
        Ok(())
    }
}

impl Drop for ImportLease<'_> {
    fn drop(&mut self) {
        if self.release_on_drop
            && let Ok(mut active) = self.gate.lock()
            && let ImportGate::Active {
                selection, closing, ..
            } = &*active
        {
            if let Some(transaction_id) = &self.fail_closed_transaction_id {
                *active = ImportGate::RecoveryRequired {
                    selection: selection.clone(),
                    transaction_id: Some(transaction_id.clone()),
                    closing: *closing,
                };
            } else if *closing {
                *active = ImportGate::Closing;
            } else {
                *active = ImportGate::Idle;
            }
        }
    }
}

#[derive(Debug)]
enum ImportGate {
    Idle,
    Closing,
    Active {
        selection: BackupImportSelection,
        cancellation: LocalStateCommitCancellation,
        closing: bool,
    },
    RecoveryRequired {
        selection: BackupImportSelection,
        transaction_id: Option<String>,
        closing: bool,
    },
}

impl ImportGate {
    fn selection(&self) -> Option<&BackupImportSelection> {
        match self {
            Self::Idle | Self::Closing => None,
            Self::Active { selection, .. } | Self::RecoveryRequired { selection, .. } => {
                Some(selection)
            }
        }
    }
}

fn selection_affects(selection: &BackupImportSelection, method: OperationMethod) -> bool {
    selection
        .affected_areas()
        .into_iter()
        .any(|area| match area {
            BackupImportArea::Settings => true,
            BackupImportArea::Favorites => false,
            BackupImportArea::IdlRegistry => {
                matches!(method, OperationMethod::LocalWalletInstructionSubmit)
            }
            BackupImportArea::WalletProfile => matches!(
                method,
                OperationMethod::LocalWalletCreateAccount
                    | OperationMethod::LocalWalletSendTransaction
                    | OperationMethod::LocalWalletInstructionSubmit
                    | OperationMethod::LocalWalletCommand
                    | OperationMethod::LocalWalletDeployProgram
                    | OperationMethod::LocalWalletSyncPrivate
                    | OperationMethod::LocalWalletAccounts
            ),
        })
}

fn selection_value(selection: &BackupImportSelection) -> Value {
    Value::Array(
        selection
            .selected_areas()
            .into_iter()
            .map(|area| Value::String(area.as_str().to_owned()))
            .collect(),
    )
}

fn local_state_failure(error: &anyhow::Error) -> Option<LocalStateFailureStatus> {
    error
        .downcast_ref::<LocalStateTransactionError>()
        .map(LocalStateTransactionError::status)
}

fn local_state_transaction_id(error: &anyhow::Error) -> Option<String> {
    error
        .downcast_ref::<LocalStateTransactionError>()
        .and_then(LocalStateTransactionError::transaction_id)
        .map(str::to_owned)
}

#[derive(Debug, Clone)]
struct OperationDecision {
    operation_id: String,
    label: String,
    action: DecisionAction,
    operation: Value,
    request: Option<RuntimeOperationRequest>,
    operation_class: &'static str,
    affected_inputs: Value,
    restart_policy: &'static str,
    restart_eligible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecisionAction {
    Stop,
    Block,
}

struct StoppedOperationGuard<'a> {
    runtime: &'a Runtime,
    operations: &'a RuntimeOperations,
    import_id: String,
    backup_catalog_id: String,
    stopped: Vec<OperationDecision>,
    restart_on_drop: bool,
}

impl<'a> StoppedOperationGuard<'a> {
    fn new(
        runtime: &'a Runtime,
        operations: &'a RuntimeOperations,
        import_id: &str,
        backup_catalog_id: &str,
    ) -> Self {
        Self {
            runtime,
            operations,
            import_id: import_id.to_owned(),
            backup_catalog_id: backup_catalog_id.to_owned(),
            stopped: Vec::new(),
            restart_on_drop: true,
        }
    }

    fn record(&mut self, decision: OperationDecision) {
        if decision.restart_eligible {
            self.stopped.push(decision);
        }
    }

    fn enter_committing(&mut self) {
        self.restart_on_drop = false;
    }

    fn compensate(&mut self) -> Vec<Value> {
        let events = restart_stopped_operations(
            self.runtime,
            self.operations,
            &self.import_id,
            &self.backup_catalog_id,
            std::mem::take(&mut self.stopped),
        );
        self.restart_on_drop = false;
        events
    }

    fn retain_for_recovery(&mut self) {
        self.stopped.clear();
        self.restart_on_drop = false;
    }
}

impl Drop for StoppedOperationGuard<'_> {
    fn drop(&mut self) {
        if self.restart_on_drop {
            let _events = restart_stopped_operations(
                self.runtime,
                self.operations,
                &self.import_id,
                &self.backup_catalog_id,
                std::mem::take(&mut self.stopped),
            );
        }
    }
}

fn restart_stopped_operations(
    runtime: &Runtime,
    operations: &RuntimeOperations,
    import_id: &str,
    backup_catalog_id: &str,
    stopped: Vec<OperationDecision>,
) -> Vec<Value> {
    let mut events = Vec::new();
    for decision in stopped {
        let Some(request) = decision.request.clone() else {
            continue;
        };
        match operations.start_after_backup_import(runtime, request) {
            Ok(operation) => events.push(decision_event(
                import_id,
                backup_catalog_id,
                &decision,
                "restart",
                "Restarted safe read operation after backup import.",
                Some(operation),
            )),
            Err(error) => events.push(decision_event(
                import_id,
                backup_catalog_id,
                &decision,
                "restart_failed",
                &error.to_string(),
                None,
            )),
        }
    }
    events
}

impl DecisionAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Stop => "stop",
            Self::Block => "block",
        }
    }
}

struct BackupImportPlan {
    backup_catalog_id: String,
    import_id: String,
    selection: BackupImportSelection,
    preview: Value,
    decisions: Vec<OperationDecision>,
    events: Vec<Value>,
    phase_history: Vec<BackupImportPhase>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackupImportPhase {
    Preparing,
    Quiescing,
    Committing,
    Applied,
    RolledBack,
    RecoveryRequired,
}

impl BackupImportPhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::Preparing => "Preparing",
            Self::Quiescing => "Quiescing",
            Self::Committing => "Committing",
            Self::Applied => "Applied",
            Self::RolledBack => "RolledBack",
            Self::RecoveryRequired => "RecoveryRequired",
        }
    }

    fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Preparing, Self::Quiescing | Self::RolledBack)
                | (
                    Self::Quiescing,
                    Self::Committing | Self::RolledBack | Self::RecoveryRequired
                )
                | (
                    Self::Committing,
                    Self::Applied | Self::RolledBack | Self::RecoveryRequired
                )
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackupImportOutcome {
    Preview,
    Applied,
    Blocked,
    RolledBack,
    RecoveryRequired,
}

impl BackupImportOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Applied => "applied",
            Self::Blocked => "blocked",
            Self::RolledBack => "rolled_back",
            Self::RecoveryRequired => "recovery_required",
        }
    }

    fn terminal(self) -> bool {
        self != Self::Preview
    }
}

impl BackupImportPlan {
    fn preparing(
        backup_catalog_id: &str,
        import_id: String,
        selection: BackupImportSelection,
        preview: Value,
    ) -> Self {
        Self {
            backup_catalog_id: backup_catalog_id.to_owned(),
            import_id,
            selection,
            preview,
            decisions: Vec::new(),
            events: Vec::new(),
            phase_history: vec![BackupImportPhase::Preparing],
        }
    }

    fn new(
        operations: &RuntimeOperations,
        backup_catalog_id: &str,
        import_id: String,
        selection: BackupImportSelection,
        preview: Value,
    ) -> Result<Self> {
        if !preview.is_object() {
            bail!("backup import preview must be a JSON object");
        }
        let decisions = operation_decisions(operations, &selection)?;
        let mut plan = Self::preparing(backup_catalog_id, import_id, selection, preview);
        plan.decisions = decisions;
        Ok(plan)
    }

    fn transition(&mut self, next: BackupImportPhase) -> Result<()> {
        let current = self
            .phase_history
            .last()
            .copied()
            .context("backup import phase history is empty")?;
        if !current.can_transition_to(next) {
            bail!(
                "invalid backup import phase transition {} -> {}",
                current.as_str(),
                next.as_str()
            );
        }
        self.phase_history.push(next);
        Ok(())
    }

    fn blocked(&self) -> bool {
        self.decisions
            .iter()
            .any(|decision| decision.action == DecisionAction::Block)
    }

    fn stop_decisions(&self) -> Vec<OperationDecision> {
        self.decisions
            .iter()
            .filter(|decision| decision.action == DecisionAction::Stop)
            .cloned()
            .collect()
    }

    fn record_block_events(&mut self) {
        for decision in self.decisions.clone() {
            if decision.action == DecisionAction::Block {
                self.events.push(decision_event(
                    &self.import_id,
                    &self.backup_catalog_id,
                    &decision,
                    "block",
                    "Blocked backup import while affected operation is running.",
                    None,
                ));
            }
        }
    }

    fn record_terminal(&mut self, decision: OperationDecision, operation: Value, canceled: bool) {
        self.events.push(decision_event(
            &self.import_id,
            &self.backup_catalog_id,
            &decision,
            if canceled { "stop" } else { "settled" },
            if canceled {
                "Stopped affected operation before backup import."
            } else {
                "Affected operation settled before backup import."
            },
            Some(operation),
        ));
    }

    fn record_stop_timeout(&mut self, decision: OperationDecision) {
        if let Some(planned) = self
            .decisions
            .iter_mut()
            .find(|planned| planned.operation_id == decision.operation_id)
        {
            planned.action = DecisionAction::Block;
        }
        self.events.push(decision_event(
            &self.import_id,
            &self.backup_catalog_id,
            &decision,
            "block",
            "Timed out waiting for affected operation to stop.",
            None,
        ));
    }

    fn record_stop_error(&mut self, decision: OperationDecision, error: &anyhow::Error) {
        if let Some(planned) = self
            .decisions
            .iter_mut()
            .find(|planned| planned.operation_id == decision.operation_id)
        {
            planned.action = DecisionAction::Block;
        }
        self.events.push(decision_event(
            &self.import_id,
            &self.backup_catalog_id,
            &decision,
            "block",
            &error.to_string(),
            None,
        ));
    }

    fn into_value(
        mut self,
        outcome: BackupImportOutcome,
        restore_summary: Option<Value>,
        error: Option<String>,
    ) -> Result<Value> {
        let mut summary = restore_summary.unwrap_or_else(|| self.preview.clone());
        if outcome.terminal() && outcome != BackupImportOutcome::Applied {
            sanitize_unapplied_summary(&mut summary)?;
        }
        let mut value = summary
            .as_object()
            .cloned()
            .context("backup import summary must be a JSON object")?;
        let applied = outcome == BackupImportOutcome::Applied;
        let blocked = self.blocked()
            || outcome == BackupImportOutcome::Blocked
            || outcome == BackupImportOutcome::RecoveryRequired;
        value.insert("applied".to_owned(), Value::Bool(applied));
        value.insert("blocked".to_owned(), Value::Bool(blocked));
        value.insert(
            "blockedOperationLabel".to_owned(),
            Value::String(
                self.decisions
                    .iter()
                    .find(|decision| decision.action == DecisionAction::Block)
                    .map(|decision| decision.label.clone())
                    .unwrap_or_default(),
            ),
        );
        value.insert("selectedAreas".to_owned(), selection_value(&self.selection));
        value.insert(
            "appliedAreas".to_owned(),
            if applied {
                summary
                    .get("applied_areas")
                    .or_else(|| summary.get("appliedAreas"))
                    .filter(|areas| areas.is_array())
                    .cloned()
                    .unwrap_or_else(|| selection_value(&self.selection))
            } else {
                Value::Array(Vec::new())
            },
        );
        value.insert(
            "operation_decisions".to_owned(),
            Value::Array(
                self.decisions
                    .iter()
                    .map(|decision| {
                        decision_value(&self.import_id, &self.backup_catalog_id, decision)
                    })
                    .collect(),
            ),
        );
        if outcome.terminal() {
            self.events.push(import_terminal_event(
                &self.import_id,
                &self.backup_catalog_id,
                outcome,
                self.phase_history
                    .last()
                    .copied()
                    .unwrap_or(BackupImportPhase::Preparing),
                error.as_deref(),
            ));
        }
        value.insert(
            "operationEvents".to_owned(),
            Value::Array(std::mem::take(&mut self.events)),
        );
        value.insert("importId".to_owned(), Value::String(self.import_id));
        value.insert(
            "phase".to_owned(),
            Value::String(
                self.phase_history
                    .last()
                    .copied()
                    .unwrap_or(BackupImportPhase::Preparing)
                    .as_str()
                    .to_owned(),
            ),
        );
        value.insert(
            "phaseHistory".to_owned(),
            Value::Array(
                self.phase_history
                    .iter()
                    .map(|phase| Value::String(phase.as_str().to_owned()))
                    .collect(),
            ),
        );
        value.insert(
            "outcome".to_owned(),
            Value::String(outcome.as_str().to_owned()),
        );
        value.insert("terminal".to_owned(), Value::Bool(outcome.terminal()));
        value.insert(
            "recoveryRequired".to_owned(),
            Value::Bool(outcome == BackupImportOutcome::RecoveryRequired),
        );
        value.insert(
            "rolledBack".to_owned(),
            Value::Bool(outcome == BackupImportOutcome::RolledBack),
        );
        if let Some(error) = error {
            value.insert("error".to_owned(), Value::String(error));
        }
        value.insert(
            "backupCatalogId".to_owned(),
            Value::String(self.backup_catalog_id),
        );
        value.insert(
            "import_plan".to_owned(),
            Value::Bool(outcome == BackupImportOutcome::Preview),
        );
        value.insert("summary".to_owned(), summary);
        Ok(Value::Object(value))
    }
}

fn preparation_summary(backup_catalog_id: &str) -> Value {
    json!({
        "backup_catalog_id": backup_catalog_id,
        "restored": false,
        "applied_areas": []
    })
}

fn sanitize_unapplied_summary(summary: &mut Value) -> Result<()> {
    let value = summary
        .as_object_mut()
        .context("backup import summary must be a JSON object")?;
    let planned_areas = value
        .remove("applied_areas")
        .or_else(|| value.remove("appliedAreas"))
        .filter(Value::is_array)
        .unwrap_or_else(|| Value::Array(Vec::new()));
    value.insert("planned_areas".to_owned(), planned_areas);
    value.insert("applied_areas".to_owned(), Value::Array(Vec::new()));
    value.insert("restored".to_owned(), Value::Bool(false));
    Ok(())
}

fn build_plan(
    operations: &RuntimeOperations,
    backup_catalog_id: &str,
    selection: BackupImportSelection,
    preview: Value,
    events: Vec<Value>,
) -> Result<Value> {
    let mut plan = BackupImportPlan::new(
        operations,
        backup_catalog_id,
        format!("backup_import_preview:{backup_catalog_id}"),
        selection,
        preview,
    )?;
    plan.events = events;
    plan.into_value(BackupImportOutcome::Preview, None, None)
}

fn operation_decisions(
    operations: &RuntimeOperations,
    selection: &BackupImportSelection,
) -> Result<Vec<OperationDecision>> {
    operations.registry.inspect(|registry| {
        let mut decisions = registry
            .values()
            .filter(|record| !record.operation.status.is_terminal())
            .filter_map(|record| operation_decision(record, selection))
            .collect::<Vec<_>>();
        decisions.sort_by(|left, right| left.operation_id.cmp(&right.operation_id));
        decisions
    })
}

fn operation_decision(
    record: &RuntimeOperationRecord,
    selection: &BackupImportSelection,
) -> Option<OperationDecision> {
    let method = record.restart_request.as_ref()?.method();
    if !selection_affects(selection, method) {
        return None;
    }
    let can_stop = record.operation.status == RuntimeOperationStatus::Running
        && record.operation.cancellable
        && record.operation.policy.safe_to_restart();
    Some(OperationDecision {
        operation_id: record.operation.operation_id.as_str().to_owned(),
        label: record.operation.label.clone(),
        action: if can_stop {
            DecisionAction::Stop
        } else {
            DecisionAction::Block
        },
        operation: runtime_operation_value(&record.operation),
        request: record.restart_request.clone(),
        operation_class: record.operation.policy.class_name(),
        affected_inputs: record.operation.policy.affected_inputs_value(),
        restart_policy: record.operation.policy.restart_policy_name(),
        restart_eligible: can_stop && record.operation.policy.safe_to_restart(),
    })
}

fn wait_until_terminal(
    operations: &RuntimeOperations,
    operation_id: &str,
    timeout: Duration,
) -> Result<Option<Value>> {
    let deadline = Instant::now() + timeout;
    loop {
        let operation = operations.value(operation_id)?;
        let status = operation
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if matches!(
            status,
            "completed" | "dispatched" | "failed" | "canceled" | "timed_out"
        ) {
            return Ok(Some(operation));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(STOP_POLL_INTERVAL);
    }
}

fn coordinator_owns_cancellation(requested_by_coordinator: bool, operation: &Value) -> bool {
    requested_by_coordinator
        && operation
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|status| status == "canceled")
}

fn decision_value(import_id: &str, backup_catalog_id: &str, decision: &OperationDecision) -> Value {
    json!({
        "operation": decision.operation,
        "operationId": decision.operation_id,
        "label": decision.label,
        "operationClass": decision.operation_class,
        "affectedInputs": decision.affected_inputs,
        "restartPolicy": decision.restart_policy,
        "action": decision.action.as_str(),
        "affected": true,
        "restart": decision.restart_eligible,
        "restartEligible": decision.restart_eligible,
        "safeToLetFinish": false,
        "importId": import_id,
        "backupCatalogId": backup_catalog_id,
    })
}

fn decision_event(
    import_id: &str,
    backup_catalog_id: &str,
    decision: &OperationDecision,
    action: &str,
    detail: &str,
    operation: Option<Value>,
) -> Value {
    let (status, reason) = action_facts(action);
    let restart_operation_id = if action == "restart" {
        operation
            .as_ref()
            .and_then(|value| value.get("operationId"))
            .and_then(Value::as_str)
    } else {
        None
    };
    json!({
        "domain": "backup",
        "method": "settingsBackupImportPolicy",
        "status": status,
        "label": "Backup import policy",
        "operationId": restart_operation_id.unwrap_or(&decision.operation_id),
        "previousOperationId": (action == "restart").then_some(&decision.operation_id),
        "restartOperationId": restart_operation_id,
        "operationClass": decision.operation_class,
        "affectedInputs": decision.affected_inputs,
        "restartPolicy": decision.restart_policy,
        "confirmationRequired": false,
        "importId": import_id,
        "backupCatalogId": backup_catalog_id,
        "reason": reason,
        "detail": detail,
        "action": action,
        "operation": operation,
        "provenance": ["backup_import_coordinator", "runtime_operation_registry", "local_backup_catalog"],
        "result": {
            "action": action,
            "status": status,
            "reason": reason,
            "import_id": import_id,
            "backup_catalog_id": backup_catalog_id,
            "operation_id": decision.operation_id,
            "previous_operation_id": (action == "restart").then_some(&decision.operation_id),
            "restart_operation_id": restart_operation_id,
            "operation_class": decision.operation_class,
            "restart": action == "restart",
            "restart_eligible": decision.restart_eligible,
            "safe_to_let_finish": false,
            "provenance": ["backup_import_coordinator", "runtime_operation_registry", "local_backup_catalog"]
        }
    })
}

fn import_terminal_event(
    import_id: &str,
    backup_catalog_id: &str,
    outcome: BackupImportOutcome,
    phase: BackupImportPhase,
    error: Option<&str>,
) -> Value {
    let status = match outcome {
        BackupImportOutcome::Applied => "applied_for_import",
        BackupImportOutcome::Blocked => "blocked_for_import",
        BackupImportOutcome::RolledBack => "rolled_back_for_import",
        BackupImportOutcome::RecoveryRequired => "recovery_required_for_import",
        BackupImportOutcome::Preview => "prepared_for_import",
    };
    json!({
        "domain": "backup",
        "method": "settingsBackupImportApply",
        "status": status,
        "label": "Settings backup import",
        "operationId": import_id,
        "operationClass": "backup",
        "restartPolicy": "manual_required",
        "confirmationRequired": true,
        "importId": import_id,
        "backupCatalogId": backup_catalog_id,
        "phase": phase.as_str(),
        "outcome": outcome.as_str(),
        "reason": format!("backup_import_{}", outcome.as_str()),
        "detail": error.unwrap_or_default(),
        "terminal": outcome.terminal(),
        "provenance": [
            "backup_import_coordinator",
            "runtime_operation_registry",
            "local_backup_catalog"
        ],
        "result": {
            "importId": import_id,
            "backupCatalogId": backup_catalog_id,
            "phase": phase.as_str(),
            "outcome": outcome.as_str(),
            "terminal": outcome.terminal()
        }
    })
}

fn action_facts(action: &str) -> (&'static str, &'static str) {
    match action {
        "stop" => (
            "stopped_for_import",
            "affected_operation_stopped_for_import",
        ),
        "block" => (
            "blocked_for_import",
            "affected_operation_blocked_for_import",
        ),
        "restart" => (
            "restarted_after_import",
            "safe_operation_restarted_after_import",
        ),
        "restart_failed" => (
            "restart_failed_after_import",
            "safe_operation_restart_failed_after_import",
        ),
        "settled" => (
            "settled_before_import",
            "affected_operation_settled_before_import",
        ),
        _ => ("ignored", "not_applicable"),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
            mpsc,
        },
        thread,
        time::{Duration, Instant},
    };

    use anyhow::{Context as _, Result, bail};
    use serde_json::json;

    use super::super::{
        identity::RuntimeOperationId, record::running_runtime_operation_record,
        transition::RuntimeOperationTransition,
    };
    use super::*;
    use crate::support::{
        backup_catalog::{
            preview_local_settings_restore_with_options_in_dir_for_test,
            record_remote_settings_backup_payload_in_dir,
            restore_local_settings_backup_at_boundary_in_dir_for_test,
            restore_local_settings_backup_with_options_in_dir_for_test,
        },
        local_state::{
            LocalStateTestBoundary, LocalStateTestFault, local_state_hot_journal_exists_in,
        },
    };

    const OLD_SETTINGS: &[u8] =
        br#"{"version":2,"theme":"old","channel_source_configs":[],"favorites":[{"value":"old-favorite"}]}"#;
    const OLD_IDL: &[u8] = br#"{"version":1,"idls":[],"account_idl_selections":{}}"#;
    const OLD_WALLET: &[u8] = br#"{"profile":{"label":"Old wallet"}}"#;

    #[derive(Debug)]
    struct DirectoryBackupImportStore {
        base_dir: PathBuf,
        fault: Option<LocalStateTestFault>,
    }

    impl BackupImportStore for DirectoryBackupImportStore {
        fn preview(
            &self,
            backup_catalog_id: &str,
            wallet_profile: Option<&Value>,
            options: &BackupImportOptions,
        ) -> Result<Value> {
            preview_local_settings_restore_with_options_in_dir_for_test(
                &self.base_dir,
                backup_catalog_id,
                wallet_profile,
                options,
            )
        }

        fn restore(
            &self,
            backup_catalog_id: &str,
            wallet_profile: Option<&Value>,
            options: &BackupImportOptions,
            cancellation: &LocalStateCommitCancellation,
        ) -> Result<BackupImportCommitReceipt> {
            restore_local_settings_backup_with_options_in_dir_for_test(
                &self.base_dir,
                backup_catalog_id,
                wallet_profile,
                options,
                cancellation,
                self.fault,
            )
            .map(BackupImportCommitReceipt::from_local)
        }
    }

    struct BoundaryPausedBackupImportStore {
        base_dir: PathBuf,
        boundary: LocalStateTestBoundary,
        pause: Mutex<Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>>,
    }

    impl BackupImportStore for BoundaryPausedBackupImportStore {
        fn preview(
            &self,
            backup_catalog_id: &str,
            wallet_profile: Option<&Value>,
            options: &BackupImportOptions,
        ) -> Result<Value> {
            preview_local_settings_restore_with_options_in_dir_for_test(
                &self.base_dir,
                backup_catalog_id,
                wallet_profile,
                options,
            )
        }

        fn restore(
            &self,
            backup_catalog_id: &str,
            wallet_profile: Option<&Value>,
            options: &BackupImportOptions,
            cancellation: &LocalStateCommitCancellation,
        ) -> Result<BackupImportCommitReceipt> {
            let (entered, release) = self
                .pause
                .lock()
                .map_err(|_| anyhow::anyhow!("backup import boundary pause is unavailable"))?
                .take()
                .context("backup import boundary pause was already consumed")?;
            restore_local_settings_backup_at_boundary_in_dir_for_test(
                &self.base_dir,
                backup_catalog_id,
                wallet_profile,
                options,
                cancellation,
                self.boundary,
                move || {
                    entered
                        .send(())
                        .context("failed to signal backup import test boundary")?;
                    release
                        .recv()
                        .context("failed to release backup import test boundary")
                },
            )
            .map(BackupImportCommitReceipt::from_local)
        }
    }

    fn real_import_options() -> Value {
        json!({
            "app_settings": "replace",
            "favorites": "replace",
            "idl": "replace",
            "wallet": "replace"
        })
    }

    fn seeded_real_import(
        fault: Option<LocalStateTestFault>,
    ) -> Result<(tempfile::TempDir, Arc<DirectoryBackupImportStore>, String)> {
        let directory = tempfile::tempdir().context("failed to create backup import test dir")?;
        fs::write(directory.path().join("settings.json"), OLD_SETTINGS)?;
        fs::write(directory.path().join("idls.json"), OLD_IDL)?;
        fs::write(directory.path().join("wallet.json"), OLD_WALLET)?;
        let payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": {
                "settings": {
                    "version": 2,
                    "theme": "new",
                    "channel_source_configs": [],
                    "favorites": [{ "value": "new-favorite" }]
                },
                "idls": {
                    "version": 1,
                    "idls": [{ "key": "idl-new", "json": "{}" }],
                    "account_idl_selections": {}
                },
                "wallet": { "profile": { "label": "New wallet" } }
            }
        });
        let entry = record_remote_settings_backup_payload_in_dir(
            directory.path(),
            Some("Ticket 03 vertical fixture"),
            &payload,
            "z-ticket-03-vertical",
            Some("logos_storage"),
        )?;
        let store = Arc::new(DirectoryBackupImportStore {
            base_dir: directory.path().to_path_buf(),
            fault,
        });
        Ok((directory, store, entry.backup_catalog_id))
    }

    fn assert_exact_original_state(base_dir: &Path) -> Result<()> {
        for (file_name, expected) in [
            ("settings.json", OLD_SETTINGS),
            ("idls.json", OLD_IDL),
            ("wallet.json", OLD_WALLET),
        ] {
            let actual = fs::read(base_dir.join(file_name))?;
            if actual != expected {
                bail!("backup import did not restore exact original bytes for {file_name}");
            }
        }
        Ok(())
    }

    fn assert_exact_imported_state(base_dir: &Path) -> Result<()> {
        let settings: Value = serde_json::from_slice(&fs::read(base_dir.join("settings.json"))?)?;
        let idl: Value = serde_json::from_slice(&fs::read(base_dir.join("idls.json"))?)?;
        let wallet: Value = serde_json::from_slice(&fs::read(base_dir.join("wallet.json"))?)?;
        if settings.get("theme").and_then(Value::as_str) != Some("new")
            || settings
                .pointer("/favorites/0/value")
                .and_then(Value::as_str)
                != Some("new-favorite")
            || idl.pointer("/idls/0/key").and_then(Value::as_str) != Some("idl-new")
            || wallet.pointer("/profile/label").and_then(Value::as_str) != Some("New wallet")
        {
            bail!("backup import did not persist one coherent selected state");
        }
        Ok(())
    }

    fn apply_with_close_at_local_state_boundary(
        boundary: LocalStateTestBoundary,
    ) -> Result<(tempfile::TempDir, Value)> {
        let (directory, _regular_store, backup_catalog_id) = seeded_real_import(None)?;
        let (entered_sender, entered_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        let store = Arc::new(BoundaryPausedBackupImportStore {
            base_dir: directory.path().to_path_buf(),
            boundary,
            pause: Mutex::new(Some((entered_sender, release_receiver))),
        });
        let operations = Arc::new(RuntimeOperations::with_backup_import_store(store));
        let close_handle = operations.close_handle();
        let apply_operations = Arc::clone(&operations);
        let apply = thread::spawn(move || -> Result<Value> {
            let runtime = Runtime::new()?;
            let result = apply_operations.backup_import.apply(
                &runtime,
                &apply_operations,
                &backup_catalog_id,
                None,
                Some(&real_import_options()),
            )?;
            apply_operations.shutdown(&runtime)?;
            Ok(result)
        });
        entered_receiver
            .recv_timeout(Duration::from_secs(3))
            .context("timed out waiting for local state commit boundary")?;
        let close_result = close_handle.begin_close();
        release_sender
            .send(())
            .context("failed to release local state commit boundary")?;
        close_result?;
        let result = apply
            .join()
            .map_err(|_| anyhow::anyhow!("boundary backup import thread panicked"))??;
        Ok((directory, result))
    }

    fn insert_restartable_running_operation(
        operations: &RuntimeOperations,
        operation_id: &str,
    ) -> Result<()> {
        let request = storage_module_request()?;
        let mut record = running_runtime_operation_record(
            RuntimeOperationId::parse(operation_id)?,
            &request,
            1,
        )?;
        record.operation.cancellable = true;
        operations.registry.insert(record)
    }

    fn confirm_cancel_when_requested(
        operations: Arc<RuntimeOperations>,
        operation_id: String,
    ) -> thread::JoinHandle<Result<()>> {
        confirm_cancel_when_requested_after(operations, operation_id, Duration::ZERO)
    }

    fn confirm_cancel_when_requested_after(
        operations: Arc<RuntimeOperations>,
        operation_id: String,
        confirmation_delay: Duration,
    ) -> thread::JoinHandle<Result<()>> {
        thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                let operation = operations.value(&operation_id)?;
                if operation.get("status").and_then(Value::as_str) == Some("canceling") {
                    thread::sleep(confirmation_delay);
                    operations.registry.transition(
                        &RuntimeOperationId::parse(&operation_id)?,
                        RuntimeOperationTransition::CancellationConfirmed {
                            error: Some("stopped by backup import test".to_owned()),
                        },
                    )?;
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    bail!("timed out waiting for backup import cancellation request");
                }
                thread::sleep(Duration::from_millis(5));
            }
        })
    }

    fn confirm_cancels_after_all_requested(
        operations: Arc<RuntimeOperations>,
        operation_ids: Vec<String>,
        confirmation_delay: Duration,
    ) -> thread::JoinHandle<Result<()>> {
        thread::spawn(move || {
            wait_for_all_cancel_requests(&operations, &operation_ids)?;
            thread::sleep(confirmation_delay);
            for operation_id in operation_ids {
                operations.registry.transition(
                    &RuntimeOperationId::parse(&operation_id)?,
                    RuntimeOperationTransition::CancellationConfirmed {
                        error: Some("stopped by collective backup import test".to_owned()),
                    },
                )?;
            }
            Ok(())
        })
    }

    fn confirm_cancels_in_two_waves_after_all_requested(
        operations: Arc<RuntimeOperations>,
        operation_ids: Vec<String>,
        first_delay: Duration,
        second_delay: Duration,
    ) -> thread::JoinHandle<Result<()>> {
        thread::spawn(move || {
            wait_for_all_cancel_requests(&operations, &operation_ids)?;
            let delayed = operation_ids
                .get(1)
                .context("two-wave cancellation fixture requires two operations")?;
            thread::sleep(first_delay);
            for (index, operation_id) in operation_ids.iter().enumerate() {
                if index == 1 {
                    continue;
                }
                operations.registry.transition(
                    &RuntimeOperationId::parse(operation_id)?,
                    RuntimeOperationTransition::CancellationConfirmed {
                        error: Some("stopped in first cancellation wave".to_owned()),
                    },
                )?;
            }
            thread::sleep(second_delay);
            operations.registry.transition(
                &RuntimeOperationId::parse(delayed)?,
                RuntimeOperationTransition::CancellationConfirmed {
                    error: Some("stopped in delayed cancellation wave".to_owned()),
                },
            )?;
            Ok(())
        })
    }

    fn wait_for_all_cancel_requests(
        operations: &RuntimeOperations,
        operation_ids: &[String],
    ) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let mut all_requested = true;
            for operation_id in operation_ids {
                let operation = operations.value(operation_id)?;
                if operation.get("status").and_then(Value::as_str) != Some("canceling") {
                    all_requested = false;
                    break;
                }
            }
            if all_requested {
                return Ok(());
            }
            if Instant::now() >= deadline {
                bail!("backup import did not request every affected cancellation first");
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    struct FakeBackupImportStore {
        restore_calls: AtomicUsize,
        restore_failure: Option<LocalStateFailureStatus>,
    }

    impl FakeBackupImportStore {
        fn new() -> Self {
            Self {
                restore_calls: AtomicUsize::new(0),
                restore_failure: None,
            }
        }

        fn failing(status: LocalStateFailureStatus) -> Self {
            Self {
                restore_calls: AtomicUsize::new(0),
                restore_failure: Some(status),
            }
        }
    }

    impl BackupImportStore for FakeBackupImportStore {
        fn preview(
            &self,
            _backup_catalog_id: &str,
            _wallet_profile: Option<&Value>,
            _options: &BackupImportOptions,
        ) -> Result<Value> {
            Ok(json!({
                "restored": true,
                "settings": true,
                "favorites": 0,
                "idls": false,
                "wallet": false
            }))
        }

        fn restore(
            &self,
            backup_catalog_id: &str,
            _wallet_profile: Option<&Value>,
            options: &BackupImportOptions,
            _cancellation: &LocalStateCommitCancellation,
        ) -> Result<BackupImportCommitReceipt> {
            self.restore_calls.fetch_add(1, Ordering::Relaxed);
            if let Some(status) = self.restore_failure {
                let error = match status {
                    LocalStateFailureStatus::RolledBack => {
                        LocalStateTransactionError::test_rolled_back("tx-test")
                    }
                    LocalStateFailureStatus::RecoveryRequired => {
                        LocalStateTransactionError::test_recovery_required("tx-test")
                    }
                };
                return Err(error.into());
            }
            valid_test_receipt(backup_catalog_id, options.selection())
        }
    }

    #[derive(Debug, Clone, Copy)]
    enum BoundaryStoreBehavior {
        PreviewFailure,
        RestoreFailure,
        ForeignReceipt,
    }

    struct BoundaryFailureStore {
        behavior: BoundaryStoreBehavior,
    }

    impl BackupImportStore for BoundaryFailureStore {
        fn preview(
            &self,
            _backup_catalog_id: &str,
            _wallet_profile: Option<&Value>,
            _options: &BackupImportOptions,
        ) -> Result<Value> {
            if matches!(self.behavior, BoundaryStoreBehavior::PreviewFailure) {
                bail!("injected preview failure");
            }
            Ok(json!({
                "restored": false,
                "applied_areas": ["settings"]
            }))
        }

        fn restore(
            &self,
            _backup_catalog_id: &str,
            _wallet_profile: Option<&Value>,
            options: &BackupImportOptions,
            _cancellation: &LocalStateCommitCancellation,
        ) -> Result<BackupImportCommitReceipt> {
            match self.behavior {
                BoundaryStoreBehavior::RestoreFailure => bail!("injected restore failure"),
                BoundaryStoreBehavior::ForeignReceipt => {
                    valid_test_receipt("backup-foreign", options.selection())
                }
                BoundaryStoreBehavior::PreviewFailure => {
                    bail!("restore must not run after preview failure")
                }
            }
        }
    }

    fn valid_test_receipt(
        backup_catalog_id: &str,
        selection: &BackupImportSelection,
    ) -> Result<BackupImportCommitReceipt> {
        let applied_areas = selection.selected_areas();
        BackupImportCommitReceipt::try_from_adapter_parts(
            backup_catalog_id,
            selection.clone(),
            applied_areas,
            json!({
                "restored": true,
                "settings": true,
                "favorites": 0,
                "idl_count": 0,
                "encrypted": false,
            }),
            Some("00000000000000000000000000000000"),
            Some(DirectoryDurability::Verified),
        )
    }

    #[test]
    fn selection_uses_typed_method_impact() -> Result<()> {
        let favorites = BackupImportOptions::parse(Some(&json!({
            "settings": "skip",
            "favorites": "merge",
            "idl_registry": "skip",
            "wallet_profile": "skip"
        })))?;
        if selection_affects(favorites.selection(), OperationMethod::StorageManifests) {
            bail!("favorites import should not affect storage reads");
        }

        let wallet = BackupImportOptions::parse(Some(&json!({
            "wallet_profile": "replace"
        })))?;
        if !selection_affects(wallet.selection(), OperationMethod::LocalWalletAccounts)
            || selection_affects(wallet.selection(), OperationMethod::StorageManifests)
        {
            bail!("wallet import impact is incorrect");
        }
        Ok(())
    }

    fn storage_module_request() -> Result<RuntimeOperationRequest> {
        RuntimeOperationRequest::from_call(
            OperationMethod::StorageManifests,
            json!([{
                "adapter": { "source_mode": "module", "inputs": {} },
                "payload": {}
            }]),
            "Storage manifests",
        )
    }

    fn restartable_test_decision(operation_id: &str) -> Result<OperationDecision> {
        Ok(OperationDecision {
            operation_id: operation_id.to_owned(),
            label: "Storage manifests".to_owned(),
            action: DecisionAction::Stop,
            operation: json!({ "operationId": operation_id, "status": "canceled" }),
            request: Some(storage_module_request()?),
            operation_class: "read_poll",
            affected_inputs: json!([]),
            restart_policy: "safe_read_polling",
            restart_eligible: true,
        })
    }

    #[test]
    fn phase_reducer_accepts_only_declared_paths() {
        assert!(BackupImportPhase::Preparing.can_transition_to(BackupImportPhase::Quiescing));
        assert!(BackupImportPhase::Preparing.can_transition_to(BackupImportPhase::RolledBack));
        assert!(BackupImportPhase::Quiescing.can_transition_to(BackupImportPhase::Committing));
        assert!(BackupImportPhase::Quiescing.can_transition_to(BackupImportPhase::RolledBack));
        assert!(
            BackupImportPhase::Quiescing.can_transition_to(BackupImportPhase::RecoveryRequired)
        );
        assert!(BackupImportPhase::Committing.can_transition_to(BackupImportPhase::Applied));
        assert!(BackupImportPhase::Committing.can_transition_to(BackupImportPhase::RolledBack));
        assert!(
            BackupImportPhase::Committing.can_transition_to(BackupImportPhase::RecoveryRequired)
        );
        assert!(!BackupImportPhase::Preparing.can_transition_to(BackupImportPhase::Applied));
        assert!(!BackupImportPhase::Applied.can_transition_to(BackupImportPhase::Preparing));
    }

    #[test]
    fn admitted_boundary_failures_return_one_correlated_terminal_result() -> Result<()> {
        for behavior in [
            BoundaryStoreBehavior::PreviewFailure,
            BoundaryStoreBehavior::RestoreFailure,
        ] {
            let store = Arc::new(BoundaryFailureStore { behavior });
            let operations = RuntimeOperations::with_backup_import_store(store);
            let runtime = Runtime::new()?;
            let result = operations.backup_import.apply(
                &runtime,
                &operations,
                "backup-boundary",
                None,
                Some(&json!({ "settings": "replace" })),
            )?;
            let events = result
                .get("operationEvents")
                .and_then(Value::as_array)
                .context("boundary failure omitted operation events")?;
            let terminal_events = events
                .iter()
                .filter(|event| event.get("terminal").and_then(Value::as_bool) == Some(true))
                .collect::<Vec<_>>();
            if result.get("phase").and_then(Value::as_str) != Some("RolledBack")
                || result.get("outcome").and_then(Value::as_str) != Some("rolled_back")
                || result.get("terminal").and_then(Value::as_bool) != Some(true)
                || result.get("appliedAreas") != Some(&json!([]))
                || terminal_events.len() != 1
                || terminal_events
                    .first()
                    .and_then(|event| event.get("importId"))
                    != result.get("importId")
            {
                bail!("admitted boundary failure escaped terminal reducer: {result}");
            }
            let request = storage_module_request()?;
            let permit = operations.backup_import.operation_permit(&request)?;
            drop(permit);
            operations.shutdown(&runtime)?;
        }
        Ok(())
    }

    #[test]
    fn unexpected_committing_exit_and_foreign_typed_receipt_fail_closed() -> Result<()> {
        let selection = BackupImportOptions::parse(Some(&json!({ "settings": "replace" })))?
            .selection()
            .clone();
        let gate = Mutex::new(ImportGate::Idle);
        {
            let mut lease = ImportLease::acquire(&gate, selection)?;
            lease.begin_committing("backup_import:unexpected");
        }
        let active = gate
            .lock()
            .map_err(|_| anyhow::anyhow!("test import gate is unavailable"))?;
        if !matches!(
            &*active,
            ImportGate::RecoveryRequired {
                transaction_id: Some(transaction_id),
                ..
            } if transaction_id == "coordinator:backup_import:unexpected"
        ) {
            bail!("unexpected committing exit released mutation gate");
        }
        drop(active);

        let store = Arc::new(BoundaryFailureStore {
            behavior: BoundaryStoreBehavior::ForeignReceipt,
        });
        let operations = RuntimeOperations::with_backup_import_store(store);
        let runtime = Runtime::new()?;
        let result = operations.backup_import.apply(
            &runtime,
            &operations,
            "backup-expected",
            None,
            Some(&json!({ "settings": "replace" })),
        )?;
        if result.get("phase").and_then(Value::as_str) != Some("RecoveryRequired")
            || result.get("outcome").and_then(Value::as_str) != Some("recovery_required")
        {
            bail!("foreign typed receipt did not fail closed: {result}");
        }
        if operations
            .backup_import
            .operation_permit(&storage_module_request()?)
            .is_ok()
        {
            bail!("foreign typed receipt released affected mutation gate");
        }
        operations.shutdown(&runtime)?;
        Ok(())
    }

    #[test]
    fn typed_commit_receipt_rejects_invalid_adapter_evidence() -> Result<()> {
        let selection = BackupImportOptions::parse(Some(&json!({ "settings": "replace" })))?
            .selection()
            .clone();
        let summary = json!({ "restored": true, "settings": true });
        let valid_transaction_id = "00000000000000000000000000000000";
        let cases = [
            BackupImportCommitReceipt::try_from_adapter_parts(
                "../backup",
                selection.clone(),
                vec![BackupImportArea::Settings],
                summary.clone(),
                Some(valid_transaction_id),
                Some(DirectoryDurability::Verified),
            ),
            BackupImportCommitReceipt::try_from_adapter_parts(
                "backup-valid",
                selection.clone(),
                vec![BackupImportArea::Settings],
                summary.clone(),
                Some("invalid-transaction"),
                Some(DirectoryDurability::Verified),
            ),
            BackupImportCommitReceipt::try_from_adapter_parts(
                "backup-valid",
                selection.clone(),
                vec![BackupImportArea::Settings],
                summary.clone(),
                Some(valid_transaction_id),
                None,
            ),
            BackupImportCommitReceipt::try_from_adapter_parts(
                "backup-valid",
                selection.clone(),
                vec![BackupImportArea::Settings],
                summary.clone(),
                None,
                Some(DirectoryDurability::Verified),
            ),
            BackupImportCommitReceipt::try_from_adapter_parts(
                "backup-valid",
                selection,
                vec![BackupImportArea::Settings, BackupImportArea::Settings],
                summary,
                Some(valid_transaction_id),
                Some(DirectoryDurability::Verified),
            ),
        ];
        if cases.into_iter().any(|result| result.is_ok()) {
            bail!("typed backup import receipt admitted invalid adapter evidence");
        }
        Ok(())
    }

    #[test]
    fn unapplied_summary_and_cancel_causality_remain_unambiguous() -> Result<()> {
        let selection = BackupImportOptions::parse(Some(&json!({ "settings": "replace" })))?
            .selection()
            .clone();
        let mut plan = BackupImportPlan::preparing(
            "backup-1",
            "backup_import:1:backup-1".to_owned(),
            selection,
            json!({ "restored": true, "applied_areas": ["settings"] }),
        );
        plan.transition(BackupImportPhase::RolledBack)?;
        let value = plan.into_value(BackupImportOutcome::RolledBack, None, None)?;
        let terminal = value
            .get("operationEvents")
            .and_then(Value::as_array)
            .and_then(|events| events.last())
            .context("terminal backup import event is missing")?;
        if value.pointer("/summary/applied_areas") != Some(&json!([]))
            || value.pointer("/summary/planned_areas") != Some(&json!(["settings"]))
            || terminal.get("restartPolicy").and_then(Value::as_str) != Some("manual_required")
        {
            bail!("unapplied result contradicts its nested summary or terminal policy: {value}");
        }
        if coordinator_owns_cancellation(false, &json!({ "status": "canceled" }))
            || coordinator_owns_cancellation(true, &json!({ "status": "completed" }))
            || !coordinator_owns_cancellation(true, &json!({ "status": "canceled" }))
        {
            bail!("backup import cancellation ownership is ambiguous");
        }
        Ok(())
    }

    #[test]
    fn compensation_guard_restarts_only_recorded_safe_stops() -> Result<()> {
        let store = Arc::new(FakeBackupImportStore::new());
        let operations = RuntimeOperations::with_backup_import_store(store);
        let runtime = Runtime::new()?;
        let selection = BackupImportOptions::parse(Some(&json!({ "settings": "replace" })))?
            .selection()
            .clone();
        let lease = ImportLease::acquire(&operations.backup_import.gate, selection)?;
        if operations
            .start(&runtime, storage_module_request()?)
            .is_ok()
        {
            bail!("active import gate admitted an affected ordinary start");
        }
        let mut guard =
            StoppedOperationGuard::new(&runtime, &operations, "backup_import:backup-1", "backup-1");
        guard.record(restartable_test_decision("stopped-1")?);
        let events = guard.compensate();
        let event = events.first().context("compensation event is missing")?;

        if events.len() != 1
            || event.get("action").and_then(Value::as_str) != Some("restart")
            || event.get("previousOperationId").and_then(Value::as_str) != Some("stopped-1")
            || operations.len()? != 1
        {
            bail!("compensation did not restart the exact recorded set: {events:?}");
        }
        drop(lease);
        operations.shutdown(&runtime)?;
        Ok(())
    }

    #[test]
    fn quiesce_requests_all_stops_before_waiting_on_shared_deadline() -> Result<()> {
        let store = Arc::new(FakeBackupImportStore::new());
        let operations = Arc::new(RuntimeOperations::with_backup_import_store(store.clone()));
        let runtime = Runtime::new()?;
        let operation_ids = [
            "collective-safe-read-1".to_owned(),
            "collective-safe-read-2".to_owned(),
            "collective-safe-read-3".to_owned(),
            "collective-safe-read-4".to_owned(),
        ];
        for operation_id in &operation_ids {
            insert_restartable_running_operation(&operations, operation_id)?;
        }
        let confirmation = confirm_cancels_after_all_requested(
            Arc::clone(&operations),
            operation_ids.to_vec(),
            Duration::ZERO,
        );

        let result = operations.backup_import.apply(
            &runtime,
            &operations,
            "backup-collective-quiesce",
            None,
            Some(&json!({ "settings": "replace" })),
        )?;
        confirmation
            .join()
            .map_err(|_| anyhow::anyhow!("collective cancellation confirmer panicked"))??;
        let events = result
            .get("operationEvents")
            .and_then(Value::as_array)
            .context("collective quiesce result omitted operation events")?;
        let stop_count = events
            .iter()
            .filter(|event| event.get("action").and_then(Value::as_str) == Some("stop"))
            .count();
        let restart_count = events
            .iter()
            .filter(|event| event.get("action").and_then(Value::as_str) == Some("restart"))
            .count();
        if result.get("outcome").and_then(Value::as_str) != Some("applied")
            || store.restore_calls.load(Ordering::Relaxed) != 1
            || stop_count != operation_ids.len()
            || restart_count != operation_ids.len()
        {
            bail!("collective quiesce did not stop and compensate one exact set: {result}");
        }
        operations.shutdown(&runtime)?;
        Ok(())
    }

    #[test]
    fn quiesce_uses_one_deadline_for_staggered_operation_set() -> Result<()> {
        let store = Arc::new(FakeBackupImportStore::new());
        let operations = Arc::new(RuntimeOperations::with_backup_import_store(store.clone()));
        let runtime = Runtime::new()?;
        let operation_ids = [
            "staggered-safe-read-1".to_owned(),
            "staggered-safe-read-2".to_owned(),
            "staggered-safe-read-3".to_owned(),
            "staggered-safe-read-4".to_owned(),
        ];
        for operation_id in &operation_ids {
            insert_restartable_running_operation(&operations, operation_id)?;
        }
        let confirmation = confirm_cancels_in_two_waves_after_all_requested(
            Arc::clone(&operations),
            operation_ids.to_vec(),
            Duration::from_millis(5),
            STOP_TIMEOUT + Duration::from_millis(10),
        );

        let result = operations.backup_import.apply(
            &runtime,
            &operations,
            "backup-staggered-quiesce",
            None,
            Some(&json!({ "settings": "replace" })),
        )?;
        confirmation
            .join()
            .map_err(|_| anyhow::anyhow!("staggered cancellation confirmer panicked"))??;
        let events = result
            .get("operationEvents")
            .and_then(Value::as_array)
            .context("staggered quiesce result omitted operation events")?;
        let stop_count = events
            .iter()
            .filter(|event| event.get("action").and_then(Value::as_str) == Some("stop"))
            .count();
        let restart_count = events
            .iter()
            .filter(|event| event.get("action").and_then(Value::as_str) == Some("restart"))
            .count();
        let timed_out_second = events.iter().any(|event| {
            event.get("action").and_then(Value::as_str) == Some("block")
                && event.get("operationId").and_then(Value::as_str) == Some("staggered-safe-read-2")
        });
        if result.get("outcome").and_then(Value::as_str) != Some("blocked")
            || store.restore_calls.load(Ordering::Relaxed) != 0
            || stop_count != operation_ids.len()
            || restart_count != operation_ids.len()
            || !timed_out_second
        {
            bail!("staggered quiesce escaped the shared deadline: {result}");
        }
        operations.shutdown(&runtime)?;
        Ok(())
    }

    #[test]
    fn quiesce_timeout_waits_for_owned_cancel_then_compensates_before_release() -> Result<()> {
        let store = Arc::new(FakeBackupImportStore::new());
        let operations = Arc::new(RuntimeOperations::with_backup_import_store(store.clone()));
        let runtime = Runtime::new()?;
        let operation_id = "timeout-safe-read";
        insert_restartable_running_operation(&operations, operation_id)?;
        let cancellation = confirm_cancel_when_requested_after(
            Arc::clone(&operations),
            operation_id.to_owned(),
            STOP_TIMEOUT + Duration::from_millis(20),
        );

        let result = operations.backup_import.apply(
            &runtime,
            &operations,
            "backup-timeout",
            None,
            Some(&json!({ "settings": "replace" })),
        )?;
        cancellation
            .join()
            .map_err(|_| anyhow::anyhow!("timeout cancellation confirmer panicked"))??;
        let events = result
            .get("operationEvents")
            .and_then(Value::as_array)
            .context("timeout result omitted compensation events")?;
        if result.get("outcome").and_then(Value::as_str) != Some("blocked")
            || store.restore_calls.load(Ordering::Relaxed) != 0
            || !events.iter().any(|event| {
                event.get("action").and_then(Value::as_str) == Some("restart")
                    && event.get("previousOperationId").and_then(Value::as_str)
                        == Some(operation_id)
            })
        {
            bail!("quiesce timeout escaped without exact compensation: {result}");
        }
        let permit = operations
            .backup_import
            .operation_permit(&storage_module_request()?)?;
        drop(permit);
        operations.shutdown(&runtime)?;
        Ok(())
    }

    #[test]
    fn unreconciled_owned_cancel_returns_recovery_required_and_retains_gate() -> Result<()> {
        let store = Arc::new(FakeBackupImportStore::new());
        let operations = RuntimeOperations::with_backup_import_store(store.clone());
        let runtime = Runtime::new()?;
        insert_restartable_running_operation(&operations, "unreconciled-safe-read")?;

        let result = operations.backup_import.apply(
            &runtime,
            &operations,
            "backup-unreconciled",
            None,
            Some(&json!({ "settings": "replace" })),
        )?;
        if result.get("phase").and_then(Value::as_str) != Some("RecoveryRequired")
            || result.get("outcome").and_then(Value::as_str) != Some("recovery_required")
            || store.restore_calls.load(Ordering::Relaxed) != 0
        {
            bail!("unreconciled cancellation escaped fail-closed outcome: {result}");
        }
        if operations
            .backup_import
            .operation_permit(&storage_module_request()?)
            .is_ok()
        {
            bail!("unreconciled cancellation released affected mutation gate");
        }
        operations.shutdown(&runtime)?;
        Ok(())
    }

    #[test]
    fn recovery_disarms_compensation_without_restart() -> Result<()> {
        let store = Arc::new(FakeBackupImportStore::new());
        let operations = RuntimeOperations::with_backup_import_store(store);
        let runtime = Runtime::new()?;
        {
            let mut guard = StoppedOperationGuard::new(
                &runtime,
                &operations,
                "backup_import:backup-1",
                "backup-1",
            );
            guard.record(restartable_test_decision("stopped-1")?);
            guard.retain_for_recovery();
        }
        if operations.len()? != 0 {
            bail!("recovery-required compensation restarted an operation");
        }
        Ok(())
    }

    #[test]
    fn preview_blocks_affected_non_cancellable_operation() -> Result<()> {
        let store = Arc::new(FakeBackupImportStore::new());
        let operations = RuntimeOperations::with_backup_import_store(store);
        operations.insert_test_running_operation(
            "wallet-1",
            RuntimeOperationRequest::from_call(
                OperationMethod::LocalWalletAccounts,
                json!(["default"]),
                "Wallet accounts",
            )?,
        )?;

        let plan = operations.backup_import.preview(
            &operations,
            "backup-1",
            None,
            Some(&json!({ "wallet_profile": "replace" })),
        )?;

        if plan.get("blocked").and_then(Value::as_bool) != Some(true) {
            bail!("affected operation should block import: {plan}");
        }
        let decision = plan
            .get("operation_decisions")
            .and_then(Value::as_array)
            .and_then(|decisions| decisions.first())
            .context("backup import decision is missing")?;
        let policy = decision
            .get("operation")
            .and_then(|operation| operation.get("policyFacts"))
            .context("runtime operation policy facts are missing")?;
        if decision.get("operationClass") != policy.get("operationClass")
            || decision.get("affectedInputs") != policy.get("affectedInputs")
            || decision.get("restartPolicy") != policy.get("restartPolicy")
            || decision.get("action").and_then(Value::as_str) != Some("block")
            || decision.get("restartEligible").and_then(Value::as_bool) != Some(false)
        {
            bail!("backup decision did not reuse runtime policy facts: {decision}");
        }
        Ok(())
    }

    #[test]
    fn legacy_alias_and_canonical_selection_drive_identical_runtime_policy() -> Result<()> {
        let store = Arc::new(FakeBackupImportStore::new());
        let operations = RuntimeOperations::with_backup_import_store(store);
        operations.insert_test_running_operation(
            "wallet-1",
            RuntimeOperationRequest::from_call(
                OperationMethod::LocalWalletAccounts,
                json!(["default"]),
                "Wallet accounts",
            )?,
        )?;

        let canonical = operations.backup_import.preview(
            &operations,
            "backup-1",
            None,
            Some(&json!({ "wallet_profile": "replace" })),
        )?;
        let legacy = operations.backup_import.preview(
            &operations,
            " backup-1 ",
            None,
            Some(&json!({ "wallet": "replace" })),
        )?;

        if canonical.get("selectedAreas") != legacy.get("selectedAreas")
            || canonical.get("operation_decisions") != legacy.get("operation_decisions")
            || legacy.get("backupCatalogId").and_then(Value::as_str) != Some("backup-1")
        {
            bail!("legacy alias changed runtime import meaning");
        }
        Ok(())
    }

    #[test]
    fn apply_restores_once_per_unique_import_identity() -> Result<()> {
        let store = Arc::new(FakeBackupImportStore::new());
        let operations = RuntimeOperations::with_backup_import_store(store.clone());
        let runtime = Runtime::new()?;

        let result = operations.backup_import.apply(
            &runtime,
            &operations,
            "backup-1",
            None,
            Some(&json!({ "settings": "replace" })),
        )?;
        let second = operations.backup_import.apply(
            &runtime,
            &operations,
            "backup-1",
            None,
            Some(&json!({ "settings": "replace" })),
        )?;

        if result.get("applied").and_then(Value::as_bool) != Some(true)
            || result.get("phase").and_then(Value::as_str) != Some("Applied")
            || result.get("outcome").and_then(Value::as_str) != Some("applied")
            || result.get("importId") == second.get("importId")
            || store.restore_calls.load(Ordering::Relaxed) != 2
        {
            bail!("backup import application identity drifted: {result}");
        }
        Ok(())
    }

    #[test]
    fn rolled_back_import_releases_affected_mutation_gate() -> Result<()> {
        let store = Arc::new(FakeBackupImportStore::failing(
            LocalStateFailureStatus::RolledBack,
        ));
        let operations = RuntimeOperations::with_backup_import_store(store);
        let runtime = Runtime::new()?;
        let result = operations.backup_import.apply(
            &runtime,
            &operations,
            "backup-1",
            None,
            Some(&json!({ "wallet": "replace" })),
        )?;

        if result.get("phase").and_then(Value::as_str) != Some("RolledBack")
            || result.get("outcome").and_then(Value::as_str) != Some("rolled_back")
            || result.get("terminal").and_then(Value::as_bool) != Some(true)
        {
            bail!("typed rollback outcome was lost: {result}");
        }
        let request = RuntimeOperationRequest::from_call(
            OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        let _permit = operations.backup_import.operation_permit(&request)?;
        Ok(())
    }

    #[test]
    fn recovery_required_retains_affected_gate_and_rejects_second_import() -> Result<()> {
        let store = Arc::new(FakeBackupImportStore::failing(
            LocalStateFailureStatus::RecoveryRequired,
        ));
        let operations = RuntimeOperations::with_backup_import_store(store);
        let runtime = Runtime::new()?;
        let result = operations.backup_import.apply(
            &runtime,
            &operations,
            "backup-1",
            None,
            Some(&json!({ "wallet": "replace" })),
        )?;

        if result.get("phase").and_then(Value::as_str) != Some("RecoveryRequired")
            || result.get("outcome").and_then(Value::as_str) != Some("recovery_required")
            || result.get("recoveryRequired").and_then(Value::as_bool) != Some(true)
        {
            bail!("typed recovery-required outcome was lost: {result}");
        }
        let affected = RuntimeOperationRequest::from_call(
            OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        if operations.backup_import.operation_permit(&affected).is_ok() {
            bail!("recovery-required gate admitted affected wallet mutation");
        }
        {
            let unrelated = storage_module_request()?;
            let _permit = operations.backup_import.operation_permit(&unrelated)?;
        }
        let second = operations.backup_import.apply(
            &runtime,
            &operations,
            "backup-2",
            None,
            Some(&json!({ "wallet_profile": "replace" })),
        );
        let Err(error) = second else {
            bail!("second import should remain blocked");
        };
        if !error.to_string().contains("requires recovery") {
            bail!("second import returned wrong recovery gate error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn close_race_preserves_terminal_gate_and_rejects_post_close_import_admission() -> Result<()> {
        let store = Arc::new(FakeBackupImportStore::new());
        let operations = RuntimeOperations::with_backup_import_store(store.clone());
        let selection = BackupImportOptions::parse(Some(&json!({ "settings": "replace" })))?
            .selection()
            .clone();
        let lease = ImportLease::acquire(&operations.backup_import.gate, selection)?;
        let cancellation = lease.cancellation().clone();

        operations.close_handle().begin_close()?;
        if !cancellation.is_requested() {
            bail!("close did not signal the admitted backup import");
        }
        drop(lease);
        if !matches!(
            &*operations
                .backup_import
                .gate
                .lock()
                .map_err(|_| anyhow::anyhow!("test import gate is unavailable"))?,
            ImportGate::Closing
        ) {
            bail!("active import lease drop reopened a closing coordinator");
        }

        let runtime = Runtime::new()?;
        let admission = operations.backup_import.apply(
            &runtime,
            &operations,
            "backup-after-close",
            None,
            Some(&json!({ "settings": "replace" })),
        );
        let Err(error) = admission else {
            bail!("post-close import admission succeeded");
        };
        if !error.to_string().contains("coordinator is closing")
            || store.restore_calls.load(Ordering::Relaxed) != 0
        {
            bail!("post-close import reached durable store boundary: {error:#}");
        }
        operations.shutdown(&runtime)?;
        Ok(())
    }

    #[test]
    fn close_cancellation_before_journal_rolls_back_whole_import() -> Result<()> {
        let (directory, result) =
            apply_with_close_at_local_state_boundary(LocalStateTestBoundary::WalletStaged)?;
        if result.get("phase").and_then(Value::as_str) != Some("RolledBack")
            || result.get("outcome").and_then(Value::as_str) != Some("rolled_back")
            || result.get("appliedAreas") != Some(&json!([]))
        {
            bail!("pre-journal close did not produce one rolled-back terminal: {result}");
        }
        assert_exact_original_state(directory.path())?;
        if local_state_hot_journal_exists_in(directory.path())? {
            bail!("pre-journal cancellation retained a hot journal");
        }
        Ok(())
    }

    #[test]
    fn close_between_final_check_and_journal_persist_rolls_back_whole_import() -> Result<()> {
        let (directory, result) =
            apply_with_close_at_local_state_boundary(LocalStateTestBoundary::JournalPersistReady)?;
        if result.get("phase").and_then(Value::as_str) != Some("RolledBack")
            || result.get("outcome").and_then(Value::as_str) != Some("rolled_back")
            || result.get("appliedAreas") != Some(&json!([]))
        {
            bail!("pre-durability close did not produce one rolled-back terminal: {result}");
        }
        assert_exact_original_state(directory.path())?;
        if local_state_hot_journal_exists_in(directory.path())? {
            bail!("pre-durability cancellation retained a hot journal");
        }
        Ok(())
    }

    #[test]
    fn close_cancellation_after_journal_durability_defers_until_applied() -> Result<()> {
        let (directory, result) =
            apply_with_close_at_local_state_boundary(LocalStateTestBoundary::JournalDurable)?;
        if result.get("phase").and_then(Value::as_str) != Some("Applied")
            || result.get("outcome").and_then(Value::as_str) != Some("applied")
            || result
                .pointer("/summary/transaction/status")
                .and_then(Value::as_str)
                != Some("applied")
            || result
                .pointer("/summary/transaction/directory_durability")
                .and_then(Value::as_str)
                .is_none()
        {
            bail!("post-journal close interrupted durable completion: {result}");
        }
        assert_exact_imported_state(directory.path())?;
        if local_state_hot_journal_exists_in(directory.path())? {
            bail!("post-journal completed import retained a hot journal");
        }
        Ok(())
    }

    #[test]
    fn real_catalog_aliases_drive_runtime_policy_persistence_and_terminal_result() -> Result<()> {
        let (directory, store, backup_catalog_id) = seeded_real_import(None)?;
        let operations = RuntimeOperations::with_backup_import_store(store);
        let runtime = Runtime::new()?;
        let options = real_import_options();
        let blocking_operation_id = "ticket-03-wallet-read";
        operations.insert_test_running_operation(
            blocking_operation_id,
            RuntimeOperationRequest::from_call(
                OperationMethod::LocalWalletAccounts,
                json!(["default"]),
                "Wallet accounts",
            )?,
        )?;

        let requested_catalog_id = format!("  {backup_catalog_id}  ");
        let preview = operations.backup_import.preview(
            &operations,
            &requested_catalog_id,
            None,
            Some(&options),
        )?;
        let decision = preview
            .get("operation_decisions")
            .and_then(Value::as_array)
            .and_then(|decisions| decisions.first())
            .context("vertical import preview omitted runtime policy decision")?;
        if preview.get("backupCatalogId").and_then(Value::as_str)
            != Some(backup_catalog_id.as_str())
            || preview.get("selectedAreas")
                != Some(&json!([
                    "settings",
                    "favorites",
                    "idl_registry",
                    "wallet_profile"
                ]))
            || decision.get("operationId").and_then(Value::as_str) != Some(blocking_operation_id)
            || decision.get("action").and_then(Value::as_str) != Some("block")
        {
            bail!(
                "vertical import preview lost canonical catalog, selection, or policy: {preview}"
            );
        }
        operations
            .registry
            .remove(&RuntimeOperationId::parse(blocking_operation_id)?)?;

        let result = operations.backup_import.apply(
            &runtime,
            &operations,
            &requested_catalog_id,
            None,
            Some(&options),
        )?;
        let events = result
            .get("operationEvents")
            .and_then(Value::as_array)
            .context("vertical import result omitted operation events")?;
        let terminal_events = events
            .iter()
            .filter(|event| event.get("terminal").and_then(Value::as_bool) == Some(true))
            .collect::<Vec<_>>();
        if result.get("terminal").and_then(Value::as_bool) != Some(true)
            || result.get("phase").and_then(Value::as_str) != Some("Applied")
            || result.get("outcome").and_then(Value::as_str) != Some("applied")
            || result.get("backupCatalogId").and_then(Value::as_str)
                != Some(backup_catalog_id.as_str())
            || result.get("appliedAreas")
                != Some(&json!([
                    "settings",
                    "favorites",
                    "idl_registry",
                    "wallet_profile"
                ]))
            || result
                .pointer("/summary/transaction/status")
                .and_then(Value::as_str)
                != Some("applied")
            || terminal_events.len() != 1
            || terminal_events
                .first()
                .and_then(|event| event.get("importId"))
                != result.get("importId")
        {
            bail!("vertical import result was not one authoritative applied terminal: {result}");
        }

        let settings: Value =
            serde_json::from_slice(&fs::read(directory.path().join("settings.json"))?)?;
        let idl: Value = serde_json::from_slice(&fs::read(directory.path().join("idls.json"))?)?;
        let wallet: Value =
            serde_json::from_slice(&fs::read(directory.path().join("wallet.json"))?)?;
        if settings.get("theme").and_then(Value::as_str) != Some("new")
            || settings
                .pointer("/favorites/0/value")
                .and_then(Value::as_str)
                != Some("new-favorite")
            || idl.pointer("/idls/0/key").and_then(Value::as_str) != Some("idl-new")
            || wallet.pointer("/profile/label").and_then(Value::as_str) != Some("New wallet")
        {
            bail!("vertical import did not persist selected catalog payload");
        }
        operations.shutdown(&runtime)?;
        Ok(())
    }

    #[test]
    fn real_rollback_restores_exact_state_and_compensates_safe_stop() -> Result<()> {
        let (directory, store, backup_catalog_id) =
            seeded_real_import(Some(LocalStateTestFault::Rollback))?;
        let operations = Arc::new(RuntimeOperations::with_backup_import_store(store));
        let runtime = Runtime::new()?;
        let stopped_operation_id = "ticket-03-safe-read";
        insert_restartable_running_operation(&operations, stopped_operation_id)?;
        let cancellation =
            confirm_cancel_when_requested(Arc::clone(&operations), stopped_operation_id.to_owned());

        let apply_result = operations.backup_import.apply(
            &runtime,
            &operations,
            &backup_catalog_id,
            None,
            Some(&real_import_options()),
        );
        cancellation
            .join()
            .map_err(|_| anyhow::anyhow!("backup import cancellation confirmer panicked"))??;
        let result = apply_result?;
        let events = result
            .get("operationEvents")
            .and_then(Value::as_array)
            .context("rolled-back import omitted compensation events")?;
        if result.get("phase").and_then(Value::as_str) != Some("RolledBack")
            || result.get("outcome").and_then(Value::as_str) != Some("rolled_back")
            || !events.iter().any(|event| {
                event.get("action").and_then(Value::as_str) == Some("stop")
                    && event.get("operationId").and_then(Value::as_str)
                        == Some(stopped_operation_id)
            })
            || !events.iter().any(|event| {
                event.get("action").and_then(Value::as_str) == Some("restart")
                    && event.get("previousOperationId").and_then(Value::as_str)
                        == Some(stopped_operation_id)
            })
        {
            bail!("real rollback lost typed outcome or safe-stop compensation: {result}");
        }
        assert_exact_original_state(directory.path())?;
        if local_state_hot_journal_exists_in(directory.path())? {
            bail!("successful backup import rollback retained a hot journal");
        }
        let affected = RuntimeOperationRequest::from_call(
            OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        let permit = operations.backup_import.operation_permit(&affected)?;
        drop(permit);
        operations.shutdown(&runtime)?;
        Ok(())
    }

    #[test]
    fn real_recovery_failure_retains_hot_journal_and_mutation_gate() -> Result<()> {
        let (directory, store, backup_catalog_id) =
            seeded_real_import(Some(LocalStateTestFault::RecoveryRequired))?;
        let operations = RuntimeOperations::with_backup_import_store(store);
        let runtime = Runtime::new()?;
        let options = real_import_options();

        let result = operations.backup_import.apply(
            &runtime,
            &operations,
            &backup_catalog_id,
            None,
            Some(&options),
        )?;
        if result.get("phase").and_then(Value::as_str) != Some("RecoveryRequired")
            || result.get("outcome").and_then(Value::as_str) != Some("recovery_required")
            || result.get("recoveryRequired").and_then(Value::as_bool) != Some(true)
            || !local_state_hot_journal_exists_in(directory.path())?
        {
            bail!("real recovery failure lost hot-journal or terminal evidence: {result}");
        }
        let affected = RuntimeOperationRequest::from_call(
            OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        if operations.backup_import.operation_permit(&affected).is_ok() {
            bail!("real recovery failure released affected mutation gate");
        }
        let second = operations.backup_import.apply(
            &runtime,
            &operations,
            &backup_catalog_id,
            None,
            Some(&options),
        );
        let Err(error) = second else {
            bail!("real recovery failure admitted a second import");
        };
        if !error.to_string().contains("requires recovery") {
            bail!("real recovery gate returned wrong error: {error:#}");
        }
        operations.shutdown(&runtime)?;
        Ok(())
    }
}
