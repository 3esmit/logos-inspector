import QtQuick
import QtTest
import "../../qml/state/domains" as Domains

TestCase {
    id: testRoot

    name: "BackupImportState"

    QtObject {
        id: model

        property string settingsBackupStatus: ""
        property bool settingsBackupEncrypted: false
        property var settingsBackupContents: ({})
        property int settingsLoads: 0
        property int idlLoads: 0
        property int walletLoads: 0
        property int walletChecks: 0
        property int capabilityLoads: 0
        property int runtimeUpdates: 0

        function walletProfile() {
            return { wallet_home: "/tmp/wallet" }
        }

        function loadSettingsState() {
            settingsLoads += 1
        }

        function loadIdlState() {
            idlLoads += 1
        }

        function loadWalletState() {
            walletLoads += 1
        }

        function checkLocalWalletProfile(showResult) {
            walletChecks += 1
        }

        function loadCapabilityRegistry() {
            capabilityLoads += 1
        }

        function updateRuntimeOperation(operation) {
            runtimeUpdates += 1
        }
    }

    QtObject {
        id: catalog

        property string error: ""
        property bool importRunning: false
        property bool admit: true
        property var completion: null
        property string lastCatalogId: ""
        property var lastWalletProfile: null
        property var lastOptions: null

        function applyImport(backupCatalogId, walletProfile, options, callback) {
            lastCatalogId = String(backupCatalogId || "")
            lastWalletProfile = walletProfile
            lastOptions = options
            if (!admit) {
                error = "not admitted"
                return false
            }
            completion = callback
            importRunning = true
            return true
        }

        function complete(response) {
            const callback = completion
            completion = null
            importRunning = false
            if (typeof callback === "function") {
                callback(response)
            }
        }
    }

    QtObject {
        id: operationHistory

        property var rows: []

        function append(operation, detail) {
            rows = rows.concat([{
                operation: operation,
                detail: String(detail || "")
            }])
        }
    }

    Domains.BackupImportState {
        id: state

        model: model
        catalog: catalog
        operationHistory: operationHistory
    }

    function init() {
        model.settingsBackupStatus = ""
        model.settingsBackupEncrypted = false
        model.settingsLoads = 0
        model.idlLoads = 0
        model.walletLoads = 0
        model.walletChecks = 0
        model.capabilityLoads = 0
        model.runtimeUpdates = 0
        catalog.error = ""
        catalog.importRunning = false
        catalog.admit = true
        catalog.completion = null
        catalog.lastCatalogId = ""
        catalog.lastWalletProfile = null
        catalog.lastOptions = null
        operationHistory.rows = []
    }

    function terminalResult(catalogId, phase, outcome, selectedAreas, appliedAreas, events) {
        const importId = "backup_import:" + catalogId
        const operationEvents = Array.isArray(events) ? events.slice() : []
        operationEvents.push({
            domain: "backup",
            method: "settingsBackupImportApply",
            status: terminalStatus(outcome),
            operationId: importId,
            importId: importId,
            backupCatalogId: catalogId,
            phase: phase,
            outcome: outcome,
            restartPolicy: "manual_required",
            terminal: true,
            detail: ""
        })
        return {
            terminal: true,
            phase: phase,
            outcome: outcome,
            importId: importId,
            backupCatalogId: catalogId,
            selectedAreas: selectedAreas,
            appliedAreas: appliedAreas,
            operationEvents: operationEvents,
            encrypted: false,
            favorites: 1,
            idl_count: 2
        }
    }

    function terminalStatus(outcome) {
        switch (String(outcome || "")) {
        case "applied":
            return "applied_for_import"
        case "blocked":
            return "blocked_for_import"
        case "rolled_back":
            return "rolled_back_for_import"
        case "recovery_required":
            return "recovery_required_for_import"
        default:
            return ""
        }
    }

    function completeWith(value) {
        catalog.complete({
            ok: true,
            value: value,
            text: "OK",
            error: ""
        })
    }

    function verifyNoReloads() {
        compare(model.settingsLoads, 0)
        compare(model.idlLoads, 0)
        compare(model.walletLoads, 0)
        compare(model.walletChecks, 0)
        compare(model.capabilityLoads, 0)
    }

    function test_restore_is_admission_only_and_projects_backend_applied_areas() {
        verify(state.restoreLocalSettingsBackup("backup-wallet", {
            settings: "replace",
            wallet_profile: "skip"
        }))

        verify(state.running)
        compare(model.settingsBackupStatus, "Backup import started.")
        compare(catalog.lastCatalogId, "backup-wallet")
        compare(catalog.lastWalletProfile.wallet_home, "/tmp/wallet")
        verifyNoReloads()

        const backendEvent = {
            importId: "backup_import:backup-wallet",
            backupCatalogId: "backup-wallet",
            operationId: "op-wallet",
            action: "restart",
            detail: "Restarted backend operation.",
            operation: { operationId: "op-wallet-restarted" }
        }
        completeWith(terminalResult(
            "backup-wallet",
            "Applied",
            "applied",
            ["wallet_profile"],
            ["wallet_profile"],
            [backendEvent]
        ))

        verify(!state.running)
        compare(model.settingsLoads, 0)
        compare(model.idlLoads, 0)
        compare(model.walletLoads, 1)
        compare(model.walletChecks, 1)
        compare(model.capabilityLoads, 1)
        compare(model.runtimeUpdates, 1)
        compare(operationHistory.rows.length, 2)
        compare(operationHistory.rows[0].operation, backendEvent)
        compare(operationHistory.rows[1].operation.status, "applied_for_import")
        verify(model.settingsBackupStatus.indexOf("Imported") >= 0)
    }

    function test_applied_terminal_reloads_each_backend_area_once() {
        verify(state.restoreLocalSettingsBackup("backup-all", {}))
        completeWith(terminalResult(
            "backup-all",
            "Applied",
            "applied",
            ["settings", "favorites", "idl_registry", "wallet_profile"],
            ["settings", "favorites", "idl_registry", "wallet_profile"],
            []
        ))

        compare(model.settingsLoads, 1)
        compare(model.idlLoads, 1)
        compare(model.walletLoads, 1)
        compare(model.walletChecks, 1)
        compare(model.capabilityLoads, 1)
        compare(operationHistory.rows.length, 1)
        compare(operationHistory.rows[0].operation.status, "applied_for_import")
    }

    function test_favorites_only_preserves_encryption_setting_and_skips_capability_reload() {
        model.settingsBackupEncrypted = true
        verify(state.restoreLocalSettingsBackup("backup-favorites", {}))
        completeWith(terminalResult(
            "backup-favorites",
            "Applied",
            "applied",
            ["favorites"],
            ["favorites"],
            []
        ))

        compare(model.settingsLoads, 1)
        compare(model.idlLoads, 0)
        compare(model.walletLoads, 0)
        compare(model.capabilityLoads, 0)
        verify(model.settingsBackupEncrypted)
        compare(operationHistory.rows.length, 1)
    }

    function test_rolled_back_and_recovery_required_do_not_reload() {
        verify(state.restoreLocalSettingsBackup("backup-rollback", {}))
        completeWith(terminalResult(
            "backup-rollback",
            "RolledBack",
            "rolled_back",
            ["settings"],
            [],
            [{
                importId: "backup_import:backup-rollback",
                backupCatalogId: "backup-rollback",
                operationId: "op-read",
                action: "restart"
            }]
        ))
        verifyNoReloads()
        verify(model.settingsBackupStatus.indexOf("prior local state") >= 0)
        compare(operationHistory.rows.length, 2)

        operationHistory.rows = []
        verify(state.restoreLocalSettingsBackup("backup-recovery", {}))
        completeWith(terminalResult(
            "backup-recovery",
            "RecoveryRequired",
            "recovery_required",
            ["settings"],
            [],
            []
        ))
        verifyNoReloads()
        verify(model.settingsBackupStatus.indexOf("requires local state recovery") >= 0)
        compare(operationHistory.rows.length, 1)
    }

    function test_failed_request_does_not_project_or_reload() {
        verify(state.restoreLocalSettingsBackup("backup-failed", {}))
        catalog.complete({
            ok: false,
            value: null,
            text: "",
            error: "backend failed"
        })

        verifyNoReloads()
        compare(operationHistory.rows.length, 0)
        compare(model.settingsBackupStatus, "backend failed")
    }
}
