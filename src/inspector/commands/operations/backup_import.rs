use std::{
    fmt,
    sync::{Arc, Mutex, MutexGuard},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};
use tokio::runtime::Runtime;

use super::{
    RuntimeOperationRequest, RuntimeOperations,
    record::{RuntimeOperationRecord, RuntimeOperationStatus, runtime_operation_value},
    spec::OperationMethod,
};
use crate::support::backup_catalog::{
    preview_local_settings_restore, restore_local_settings_backup,
};

const STOP_TIMEOUT: Duration = Duration::from_secs(2);
const STOP_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(super) trait BackupImportStore: Send + Sync {
    fn preview(
        &self,
        backup_catalog_id: &str,
        wallet_profile: Option<&Value>,
        options: Option<&Value>,
    ) -> Result<Value>;

    fn restore(
        &self,
        backup_catalog_id: &str,
        wallet_profile: Option<&Value>,
        options: Option<&Value>,
    ) -> Result<Value>;
}

#[derive(Debug)]
pub(super) struct LocalBackupImportStore;

impl BackupImportStore for LocalBackupImportStore {
    fn preview(
        &self,
        backup_catalog_id: &str,
        wallet_profile: Option<&Value>,
        options: Option<&Value>,
    ) -> Result<Value> {
        preview_local_settings_restore(backup_catalog_id, wallet_profile, options)
    }

    fn restore(
        &self,
        backup_catalog_id: &str,
        wallet_profile: Option<&Value>,
        options: Option<&Value>,
    ) -> Result<Value> {
        restore_local_settings_backup(backup_catalog_id, wallet_profile, options)
    }
}

pub(super) struct BackupImportCoordinator {
    store: Arc<dyn BackupImportStore>,
    active_selection: Mutex<Option<BackupSelection>>,
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
            active_selection: Mutex::new(None),
        }
    }

    pub(super) fn operation_permit(
        &self,
        method: OperationMethod,
    ) -> Result<BackupOperationPermit<'_>> {
        let active = self
            .active_selection
            .lock()
            .map_err(|_| anyhow::anyhow!("backup import state is unavailable"))?;
        if active
            .as_ref()
            .is_some_and(|selection| selection.affects(method))
        {
            bail!(
                "operation `{}` is blocked while affected settings are being imported",
                method.as_str()
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
        let selection = BackupSelection::from_options(options)?;
        let summary = self
            .store
            .preview(backup_catalog_id, wallet_profile, options)?;
        build_plan(
            operations,
            backup_catalog_id,
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
        let selection = BackupSelection::from_options(options)?;
        if selection.is_empty() {
            bail!("select at least one backup section to import");
        }

        let lease = ImportLease::acquire(&self.active_selection, selection.clone())?;
        let preview = self
            .store
            .preview(backup_catalog_id, wallet_profile, options)?;
        let mut plan =
            BackupImportPlan::new(operations, backup_catalog_id, selection.clone(), preview)?;
        if plan.blocked() {
            plan.record_block_events();
            return plan.into_value(false, None);
        }

        for decision in plan.stop_decisions() {
            let operation_id = decision.operation_id.clone();
            operations.cancel(&operation_id)?;
            match wait_until_terminal(operations, &operation_id, STOP_TIMEOUT)? {
                Some(operation) => plan.record_stopped(decision, operation),
                None => {
                    plan.record_stop_timeout(decision);
                    return plan.into_value(false, None);
                }
            }
        }

        let summary = self
            .store
            .restore(backup_catalog_id, wallet_profile, options)?;
        drop(lease);
        plan.restart_safe_operations(runtime, operations);
        plan.into_value(true, Some(summary))
    }
}

pub(super) struct BackupOperationPermit<'a> {
    _active: MutexGuard<'a, Option<BackupSelection>>,
}

struct ImportLease<'a> {
    active_selection: &'a Mutex<Option<BackupSelection>>,
}

impl<'a> ImportLease<'a> {
    fn acquire(
        active_selection: &'a Mutex<Option<BackupSelection>>,
        selection: BackupSelection,
    ) -> Result<Self> {
        let mut active = active_selection
            .lock()
            .map_err(|_| anyhow::anyhow!("backup import state is unavailable"))?;
        if active.is_some() {
            bail!("another backup import is already active");
        }
        *active = Some(selection);
        Ok(Self { active_selection })
    }
}

impl Drop for ImportLease<'_> {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active_selection.lock() {
            *active = None;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackupArea {
    Settings,
    Favorites,
    IdlRegistry,
    WalletProfile,
}

impl BackupArea {
    fn as_str(self) -> &'static str {
        match self {
            Self::Settings => "settings",
            Self::Favorites => "favorites",
            Self::IdlRegistry => "idl_registry",
            Self::WalletProfile => "wallet_profile",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BackupSelection(Vec<BackupArea>);

impl BackupSelection {
    fn from_options(options: Option<&Value>) -> Result<Self> {
        let options = match options {
            Some(Value::Object(options)) => options,
            Some(_) => bail!("backup import options must be a JSON object"),
            None => return Ok(Self(Vec::new())),
        };
        let mut areas = Vec::new();
        for (key, area) in [
            ("settings", BackupArea::Settings),
            ("favorites", BackupArea::Favorites),
            ("idl_registry", BackupArea::IdlRegistry),
            ("wallet_profile", BackupArea::WalletProfile),
        ] {
            if import_mode_selected(options.get(key)) {
                areas.push(area);
            }
        }
        Ok(Self(areas))
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn affects(&self, method: OperationMethod) -> bool {
        self.0.iter().any(|area| match area {
            BackupArea::Settings => true,
            BackupArea::Favorites => false,
            BackupArea::IdlRegistry => {
                matches!(method, OperationMethod::LocalWalletInstructionSubmit)
            }
            BackupArea::WalletProfile => matches!(
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

    fn as_value(&self) -> Value {
        Value::Array(
            self.0
                .iter()
                .map(|area| Value::String(area.as_str().to_owned()))
                .collect(),
        )
    }
}

fn import_mode_selected(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .is_some_and(|mode| {
            !mode.is_empty()
                && !matches!(mode.as_str(), "skip" | "none" | "not_import" | "not import")
        })
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
    selection: BackupSelection,
    preview: Value,
    decisions: Vec<OperationDecision>,
    events: Vec<Value>,
}

impl BackupImportPlan {
    fn new(
        operations: &RuntimeOperations,
        backup_catalog_id: &str,
        selection: BackupSelection,
        preview: Value,
    ) -> Result<Self> {
        Ok(Self {
            backup_catalog_id: backup_catalog_id.to_owned(),
            import_id: format!("backup_import:{backup_catalog_id}"),
            decisions: operation_decisions(operations, &selection)?,
            selection,
            preview,
            events: Vec::new(),
        })
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

    fn record_stopped(&mut self, decision: OperationDecision, operation: Value) {
        self.events.push(decision_event(
            &self.import_id,
            &self.backup_catalog_id,
            &decision,
            "stop",
            "Stopped affected operation before backup import.",
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

    fn restart_safe_operations(&mut self, runtime: &Runtime, operations: &RuntimeOperations) {
        for decision in self.stop_decisions() {
            if !decision.restart_eligible {
                continue;
            }
            let Some(request) = decision.request.clone() else {
                continue;
            };
            match operations.start(runtime, request) {
                Ok(operation) => self.events.push(decision_event(
                    &self.import_id,
                    &self.backup_catalog_id,
                    &decision,
                    "restart",
                    "Restarted safe read operation after backup import.",
                    Some(operation),
                )),
                Err(error) => self.events.push(decision_event(
                    &self.import_id,
                    &self.backup_catalog_id,
                    &decision,
                    "restart_failed",
                    &error.to_string(),
                    None,
                )),
            }
        }
    }

    fn into_value(mut self, applied: bool, restore_summary: Option<Value>) -> Result<Value> {
        let summary = restore_summary.unwrap_or_else(|| self.preview.clone());
        let mut value = summary
            .as_object()
            .cloned()
            .context("backup import summary must be a JSON object")?;
        value.insert("applied".to_owned(), Value::Bool(applied));
        value.insert("blocked".to_owned(), Value::Bool(self.blocked()));
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
        value.insert("selectedAreas".to_owned(), self.selection.as_value());
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
        value.insert(
            "operation_events".to_owned(),
            Value::Array(std::mem::take(&mut self.events)),
        );
        value.insert("importId".to_owned(), Value::String(self.import_id));
        value.insert(
            "backupCatalogId".to_owned(),
            Value::String(self.backup_catalog_id.clone()),
        );
        value.insert(
            "backup_catalog_id".to_owned(),
            Value::String(self.backup_catalog_id),
        );
        value.insert("import_plan".to_owned(), Value::Bool(!applied));
        value.insert("summary".to_owned(), summary);
        Ok(Value::Object(value))
    }
}

fn build_plan(
    operations: &RuntimeOperations,
    backup_catalog_id: &str,
    selection: BackupSelection,
    preview: Value,
    events: Vec<Value>,
) -> Result<Value> {
    let mut plan = BackupImportPlan::new(operations, backup_catalog_id, selection, preview)?;
    plan.events = events;
    plan.into_value(false, None)
}

fn operation_decisions(
    operations: &RuntimeOperations,
    selection: &BackupSelection,
) -> Result<Vec<OperationDecision>> {
    let registry = operations
        .registry
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime operation registry is unavailable"))?;
    Ok(registry
        .values()
        .filter(|record| !record.operation.status.is_terminal())
        .filter_map(|record| operation_decision(record, selection))
        .collect())
}

fn operation_decision(
    record: &RuntimeOperationRecord,
    selection: &BackupSelection,
) -> Option<OperationDecision> {
    let method = record.restart_request.as_ref()?.method();
    if !selection.affects(method) {
        return None;
    }
    let can_stop =
        record.operation.status == RuntimeOperationStatus::Running && record.operation.cancellable;
    Some(OperationDecision {
        operation_id: record.operation.operation_id.clone(),
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
        if matches!(status, "completed" | "failed" | "canceled") {
            return Ok(Some(operation));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(STOP_POLL_INTERVAL);
    }
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
        _ => ("ignored", "not_applicable"),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use anyhow::{Result, bail};
    use serde_json::json;

    use super::*;

    struct FakeBackupImportStore {
        restore_calls: AtomicUsize,
    }

    impl FakeBackupImportStore {
        fn new() -> Self {
            Self {
                restore_calls: AtomicUsize::new(0),
            }
        }
    }

    impl BackupImportStore for FakeBackupImportStore {
        fn preview(
            &self,
            _backup_catalog_id: &str,
            _wallet_profile: Option<&Value>,
            _options: Option<&Value>,
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
            _backup_catalog_id: &str,
            _wallet_profile: Option<&Value>,
            _options: Option<&Value>,
        ) -> Result<Value> {
            self.restore_calls.fetch_add(1, Ordering::Relaxed);
            Ok(json!({
                "restored": true,
                "settings": true,
                "favorites": 0,
                "idl_count": 0,
                "encrypted": false
            }))
        }
    }

    #[test]
    fn selection_uses_typed_method_impact() -> Result<()> {
        let favorites = BackupSelection::from_options(Some(&json!({
            "settings": "skip",
            "favorites": "merge",
            "idl_registry": "skip",
            "wallet_profile": "skip"
        })))?;
        if favorites.affects(OperationMethod::StorageManifests) {
            bail!("favorites import should not affect storage reads");
        }

        let wallet = BackupSelection::from_options(Some(&json!({
            "wallet_profile": "replace"
        })))?;
        if !wallet.affects(OperationMethod::LocalWalletAccounts)
            || wallet.affects(OperationMethod::StorageManifests)
        {
            bail!("wallet import impact is incorrect");
        }
        Ok(())
    }

    #[test]
    fn preview_blocks_affected_non_cancellable_operation() -> Result<()> {
        let store = Arc::new(FakeBackupImportStore::new());
        let operations = RuntimeOperations::with_backup_import_store(store);
        operations.insert_test_running_operation(
            "wallet-1",
            "wallet",
            "localWalletAccounts",
            false,
        );

        let plan = operations.backup_import.preview(
            &operations,
            "backup-1",
            None,
            Some(&json!({ "wallet_profile": "replace" })),
        )?;

        if plan.get("blocked").and_then(Value::as_bool) != Some(true) {
            bail!("affected operation should block import: {plan}");
        }
        Ok(())
    }

    #[test]
    fn apply_restores_once_without_active_conflicts() -> Result<()> {
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

        if result.get("applied").and_then(Value::as_bool) != Some(true)
            || store.restore_calls.load(Ordering::Relaxed) != 1
        {
            bail!("backup import was not applied once: {result}");
        }
        Ok(())
    }
}
