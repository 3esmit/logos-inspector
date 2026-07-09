import QtQml

QtObject {
    id: root

    required property var model
    required property var catalog
    required property var operationHistory

    function defaultSettingsBackupContents() {
        return {
            settings: true,
            favorites: true,
            idl_registry: true,
            wallet_profile: true
        }
    }

    function normalizedBackupContents(contents) {
        const value = contents && typeof contents === "object" ? contents : defaultSettingsBackupContents()
        return {
            settings: value.settings === true,
            favorites: value.favorites === true,
            idl_registry: value.idl_registry === true || value.idls === true || value.idl === true,
            wallet_profile: value.wallet_profile === true || value.wallet === true
        }
    }

    function backupContentsSelected(contents) {
        const value = normalizedBackupContents(contents || model.settingsBackupContents)
        return value.settings || value.favorites || value.idl_registry || value.wallet_profile
    }

    function setSettingsBackupContent(area, enabled) {
        const next = normalizedBackupContents(model.settingsBackupContents)
        const key = String(area || "")
        if (key === "settings" || key === "favorites" || key === "idl_registry" || key === "wallet_profile") {
            next[key] = enabled === true
        }
        model.settingsBackupContents = next
    }

    function previewLocalSettingsImportPlan(backupCatalogId, options) {
        const importOptions = options && typeof options === "object" ? options : ({})
        const preview = model.previewLocalSettingsRestore(backupCatalogId, importOptions)
        if (!preview) {
            return null
        }
        return backupImportPlan(importOptions, preview, backupCatalogId)
    }

    function restoreLocalSettingsBackup(backupCatalogId, options) {
        const importOptions = options && typeof options === "object" ? options : ({})
        const preview = catalog.previewLocalRestore(backupCatalogId, model.walletProfile(), importOptions)
        if (!preview) {
            model.settingsBackupStatus = model.backupCatalogError
            return null
        }
        const plan = backupImportPlan(importOptions, preview, backupCatalogId)
        if (plan.selectedAreas.length === 0) {
            model.settingsBackupStatus = qsTr("Select at least one backup section to import.")
            return null
        }
        if (plan.blocked) {
            for (let i = 0; i < plan.decisions.length; ++i) {
                if (plan.decisions[i].action === "block") {
                    recordBackupImportDecision(plan.decisions[i], qsTr("Blocked backup import while affected operation is running."))
                }
            }
            model.settingsBackupStatus = qsTr("Backup import blocked by running operation %1.")
                .arg(plan.blockedOperationLabel)
            return null
        }
        if (!stopBackupImportOperations(plan)) {
            return null
        }
        const summary = catalog.restoreLocal(backupCatalogId, model.walletProfile(), importOptions)
        if (!summary) {
            model.settingsBackupStatus = model.backupCatalogError.length ? model.backupCatalogError : qsTr("Local backup restore failed.")
            return null
        }
        const touchesLocalSettings = backupImportTouchesLocalSettings(plan.selectedAreas)
        if (touchesLocalSettings) {
            model.loadSettingsState()
            model.settingsBackupEncrypted = summary.encrypted === true
        }
        if (plan.selectedAreas.indexOf("idl_registry") >= 0) {
            model.loadIdlState()
        }
        if (plan.selectedAreas.indexOf("wallet_profile") >= 0) {
            model.loadWalletState()
        }
        if (touchesLocalSettings || plan.selectedAreas.indexOf("wallet_profile") >= 0) {
            model.loadCapabilityRegistry()
        }
        model.settingsBackupStatus = summary.encrypted === true
            ? qsTr("Imported encrypted backup: %1 IDLs and %2 favorites.")
                .arg(Number(summary.idl_count || 0))
                .arg(Number(summary.favorites || 0))
            : qsTr("Imported %1 IDLs and %2 favorites from local backup.")
                .arg(Number(summary.idl_count || 0))
                .arg(Number(summary.favorites || 0))
        model.appendOperationHistory({
            domain: "backup",
            method: "restoreLocalSettingsBackup",
            status: "applied_for_import",
            label: qsTr("Settings backup import"),
            operationClass: "backup",
            affectedInputs: backupImportAffectedInputs(plan.selectedAreas),
            restartPolicy: "safe_read_poll_only",
            confirmationRequired: true,
            importId: plan.importId,
            backupCatalogId: plan.backupCatalogId,
            reason: "backup_import_applied_for_import",
            provenance: ["backup_import_policy", "local_backup_catalog"],
            result: summary
        }, qsTr("Local backup import applied."))
        restartBackupImportOperations(plan)
        if (touchesLocalSettings) {
            model.saveSettingsState()
        }
        return summary
    }

    function backupImportPlan(options, summary, backupCatalogId) {
        const selectedAreas = selectedBackupImportAreas(options, summary)
        const decisions = []
        const operations = runningBackupImportOperations()
        let blocked = false
        let blockedLabel = ""
        const catalogId = String((summary && summary.backup_catalog_id) || backupCatalogId || "")
        const importId = backupImportId(catalogId)
        for (let i = 0; i < operations.length; ++i) {
            const decision = backupImportOperationDecision(operations[i], selectedAreas)
            if (!decision.affected) {
                continue
            }
            decision.importId = importId
            decision.backupCatalogId = catalogId
            decisions.push(decision)
            if (decision.action === "block") {
                blocked = true
                if (!blockedLabel.length) {
                    blockedLabel = decision.label
                }
            }
        }
        const result = backupImportPlanBase(summary)
        result.selectedAreas = selectedAreas
        result.decisions = decisions
        result.operation_decisions = decisions
        result.blocked = blocked
        result.blockedOperationLabel = blockedLabel
        result.importId = importId
        result.backupCatalogId = catalogId
        result.summary = summary || {}
        result.import_plan = true
        return result
    }

    function backupImportId(backupCatalogId) {
        const catalogId = String(backupCatalogId || "unknown")
        return "backup_import:" + catalogId
    }

    function backupImportPlanBase(summary) {
        const result = ({})
        const source = summary && typeof summary === "object" ? summary : ({})
        const keys = Object.keys(source)
        for (let i = 0; i < keys.length; ++i) {
            result[keys[i]] = source[keys[i]]
        }
        return result
    }

    function backupImportEnabledGate(provenance) {
        return {
            enabled: true,
            status: "enabled",
            missing: [],
            warnings: [],
            provenance: [String(provenance || "backup_import_policy")]
        }
    }

    function backupImportDisabledGate(status, dependency, label, provenance) {
        return {
            enabled: false,
            status: String(status || "disabled"),
            missing: [{
                dependency: String(dependency || ""),
                label: String(label || dependency || ""),
                status: String(status || "disabled"),
                provenance: String(provenance || "backup_import_policy")
            }],
            warnings: [],
            provenance: [String(provenance || "backup_import_policy")]
        }
    }

    function backupImportGateSummary(gate) {
        const value = gate && typeof gate === "object" ? gate : backupImportEnabledGate("backup_import_policy")
        const missing = Array.isArray(value.missing) ? value.missing : []
        return {
            enabled: value.enabled === true,
            status: String(value.status || (value.enabled === true ? "enabled" : "disabled")),
            missing: missing,
            warnings: Array.isArray(value.warnings) ? value.warnings : [],
            provenance: Array.isArray(value.provenance) ? value.provenance : []
        }
    }

    function backupImportSafeReadOperation(metadata) {
        const operationClass = String(metadata && metadata.operationClass ? metadata.operationClass : "")
        const restartPolicy = String(metadata && metadata.restartPolicy ? metadata.restartPolicy : "")
        return operationClass === "read_poll" || restartPolicy === "safe_read_polling"
    }

    function backupImportRestartRequest(operation) {
        const request = operation && operation.restartRequest
        return request && typeof request === "object" ? request : null
    }

    function backupImportOperationGate(operation, metadata) {
        const value = operation || {}
        const domain = String(value.domain || "").toLowerCase()
        const method = String(value.method || value.label || "").toLowerCase()
        if (domain === "storage" || method.indexOf("storage") >= 0) {
            if (method.indexOf("manifest") >= 0 || method.indexOf("list") >= 0) {
                return model.storageGate("manifests")
            }
            if (method.indexOf("exists") >= 0 || method.indexOf("probe") >= 0) {
                return model.storageGate("exists")
            }
            if (method.indexOf("read") >= 0 || method.indexOf("cid") >= 0) {
                return model.storageGate("read_by_cid")
            }
            return model.storageGate("")
        }
        if (domain === "delivery" || method.indexOf("delivery") >= 0) {
            if (method.indexOf("store") >= 0 || method.indexOf("query") >= 0 || method.indexOf("read") >= 0) {
                return model.deliveryGate("store_query")
            }
            if (method.indexOf("subscribe") >= 0) {
                return model.deliveryGate("subscribe")
            }
            return model.deliveryGate("")
        }
        if (domain === "wallet" || method.indexOf("wallet") >= 0) {
            return model.walletGate("")
        }
        if (domain === "program" || method.indexOf("decode") >= 0 || method.indexOf("idl") >= 0) {
            return model.programDecodeGate()
        }
        if (domain === "backup") {
            return backupImportDisabledGate("manual_required", "backup", qsTr("Backup operation"), "operation_history")
        }
        return backupImportEnabledGate("operation_history")
    }

    function backupImportCanRestartOperation(operation, metadata) {
        const gate = backupImportOperationGate(operation, metadata)
        return backupImportRestartRequest(operation) !== null
            && backupImportSafeReadOperation(metadata)
            && gate.enabled === true
    }

    function backupImportDecisionWithAction(decision, action, restart) {
        const source = decision || {}
        return {
            operation: source.operation || {},
            operationId: String(source.operationId || ""),
            label: String(source.label || ""),
            operationClass: String(source.operationClass || ""),
            affectedInputs: source.affectedInputs || [],
            restartPolicy: String(source.restartPolicy || ""),
            action: String(action || source.action || ""),
            affected: source.affected === true,
            restart: restart === undefined ? source.restart === true : restart === true,
            restartEligible: source.restartEligible === true,
            restartGate: source.restartGate || null,
            safeToLetFinish: source.safeToLetFinish === true,
            previousOperationId: String(source.previousOperationId || source.previous_operation_id || ""),
            restartOperationId: String(source.restartOperationId || source.restart_operation_id || ""),
            importId: String(source.importId || ""),
            backupCatalogId: String(source.backupCatalogId || "")
        }
    }

    function backupImportDecisionActionLabel(decision) {
        const value = decision || {}
        switch (String(value.action || "")) {
        case "stop":
            return value.restart === true
                ? qsTr("will stop and restart if gates still pass")
                : qsTr("will stop; manual rerun required")
        case "let_finish":
            return qsTr("safe to let finish")
        case "restart":
            return qsTr("restarted")
        case "block":
            return qsTr("blocks import")
        case "skip_restart":
            return qsTr("manual rerun required")
        case "restart_failed":
            return qsTr("restart failed")
        default:
            return qsTr("not affected")
        }
    }

    function backupImportDecisionGateText(decision) {
        const gate = decision && decision.restartGate ? decision.restartGate : null
        if (!gate || gate.enabled === true) {
            return ""
        }
        const missing = Array.isArray(gate.missing) ? gate.missing : []
        if (missing.length > 0) {
            return String(missing[0].label || missing[0].dependency || gate.status || "")
        }
        return String(gate.status || "")
    }

    function backupImportDecisionSummaryText(decision) {
        const value = decision || {}
        const gateText = backupImportDecisionGateText(value)
        const base = qsTr("%1: %2").arg(String(value.label || value.operationId || qsTr("operation"))).arg(backupImportDecisionActionLabel(value))
        return gateText.length ? qsTr("%1 (%2)").arg(base).arg(gateText) : base
    }

    function backupImportOperationDecision(operation, selectedAreas) {
        const metadata = operationHistory.operationMetadata(operation || {})
        const operationClass = String(metadata.operationClass || "unknown")
        const restartPolicy = String(metadata.restartPolicy || "")
        const affected = backupImportOperationAffected(operation, selectedAreas)
        const operationId = String(operation && operation.operationId ? operation.operationId : "")
        const status = String(operation && operation.status ? operation.status : "")
        const canCancel = operation && operation.cancellable === true && status === "running"
        const safeToLetFinish = backupImportSafeReadOperation(metadata)
        const restartEligible = canCancel && backupImportRestartRequest(operation) !== null && safeToLetFinish
        const restartGate = restartEligible ? backupImportGateSummary(backupImportOperationGate(operation, metadata)) : null
        let action = "ignore"
        if (affected) {
            action = backupImportOperationConflictsWithImport(operation, metadata)
                ? "block"
                : (canCancel ? "stop" : (safeToLetFinish ? "let_finish" : "block"))
        }
        return {
            selectedAreas: selectedAreas,
            operation: operation || {},
            operationId: operationId,
            label: String(operation && (operation.label || operation.method) ? (operation.label || operation.method) : operationId),
            operationClass: operationClass,
            affectedInputs: metadata.affectedInputs || [],
            restartPolicy: restartPolicy,
            action: action,
            affected: affected,
            restart: restartEligible,
            restartEligible: restartEligible,
            restartGate: restartGate,
            safeToLetFinish: safeToLetFinish
        }
    }

    function selectedBackupImportAreas(options, summary) {
        const selected = []
        const value = options && typeof options === "object" ? options : ({})
        const areas = ["settings", "favorites", "idl_registry", "wallet_profile"]
        for (let i = 0; i < areas.length; ++i) {
            const area = areas[i]
            const mode = String(value[area] || "").trim().toLowerCase()
            if (mode.length && mode !== "skip" && mode !== "none" && mode !== "not_import" && mode !== "not import") {
                selected.push(area)
            }
        }
        if (selected.length > 0 || !summary || typeof summary !== "object") {
            return selected
        }
        const applied = Array.isArray(summary.applied_areas) ? summary.applied_areas : []
        for (let i = 0; i < applied.length; ++i) {
            selected.push(String(applied[i] || ""))
        }
        return selected
    }

    function backupImportTouchesLocalSettings(selectedAreas) {
        const areas = Array.isArray(selectedAreas) ? selectedAreas : []
        return areas.indexOf("settings") >= 0 || areas.indexOf("favorites") >= 0
    }

    function runningBackupImportOperations() {
        const revision = model.runtimeOperationsRevision
        const values = model.runtimeOperations && typeof model.runtimeOperations === "object" ? model.runtimeOperations : ({})
        const keys = Object.keys(values)
        const rows = []
        for (let i = 0; i < keys.length; ++i) {
            const operation = values[keys[i]] || {}
            const status = String(operation.status || "")
            if (status === "running" || status === "canceling") {
                rows.push(operation)
            }
        }
        return rows
    }

    function backupImportOperationAffected(operation, selectedAreas) {
        const areas = Array.isArray(selectedAreas) ? selectedAreas : []
        const metadata = operationHistory.operationMetadata(operation || {})
        if (areas.length > 0 && backupImportOperationConflictsWithImport(operation, metadata)) {
            return true
        }
        for (let i = 0; i < areas.length; ++i) {
            if (backupImportOperationAffectsArea(operation, areas[i], metadata)) {
                return true
            }
        }
        return false
    }

    function backupImportOperationConflictsWithImport(operation, metadata) {
        const value = operation || {}
        const domain = String(value.domain || "").toLowerCase()
        const method = String(value.method || value.label || "").toLowerCase()
        const info = metadata || operationHistory.operationMetadata(value)
        const operationClass = String(info.operationClass || "").toLowerCase()
        return domain === "backup"
            || operationClass === "backup"
            || method.indexOf("backup") >= 0
            || method.indexOf("restore") >= 0
            || method.indexOf("import") >= 0
            || method.indexOf("export") >= 0
            || method.indexOf("decrypt") >= 0
    }

    function backupImportOperationAffectsArea(operation, area, metadata) {
        if (backupImportMetadataAffectsArea(metadata, area)) {
            return true
        }
        const domain = String(operation && operation.domain ? operation.domain : "").toLowerCase()
        const method = String(operation && operation.method ? operation.method : "").toLowerCase()
        switch (String(area || "")) {
        case "settings":
            return domain !== "backup"
        case "favorites":
            return domain === "favorites" || method.indexOf("favorite") >= 0
        case "idl_registry":
            return method.indexOf("idl") >= 0
                || method.indexOf("decode") >= 0
                || method.indexOf("instruction") >= 0
                || method.indexOf("account") >= 0
                || domain === "program"
        case "wallet_profile":
            return domain === "wallet"
                || method.indexOf("wallet") >= 0
                || method.indexOf("sign") >= 0
                || method.indexOf("submit") >= 0
                || method.indexOf("deploy") >= 0
        default:
            return false
        }
    }

    function backupImportMetadataAffectsArea(metadata, area) {
        const wanted = backupImportCanonicalArea(area)
        if (!wanted.length) {
            return false
        }
        const inputs = metadata && Array.isArray(metadata.affectedInputs) ? metadata.affectedInputs : []
        for (let i = 0; i < inputs.length; ++i) {
            const input = inputs[i] || {}
            const key = backupImportCanonicalArea(input.key)
            const value = backupImportCanonicalArea(input.value)
            if (key === wanted || value === wanted) {
                return true
            }
        }
        return false
    }

    function backupImportCanonicalArea(value) {
        const text = String(value || "").trim().toLowerCase().replace(/[- ]/g, "_")
        switch (text) {
        case "favorite":
            return "favorites"
        case "idl":
        case "idls":
            return "idl_registry"
        case "wallet":
        case "wallet_profile_state":
            return "wallet_profile"
        case "app_settings":
        case "local_settings":
        case "settings_profile":
            return "settings"
        default:
            return text
        }
    }

    function backupImportStoppedStatus(status) {
        const value = String(status || "").toLowerCase()
        return value === "canceled" || value === "cancelled" || value === "stopped"
    }

    function backupImportTerminalStatus(status) {
        const value = String(status || "").toLowerCase()
        return backupImportStoppedStatus(value) || value === "completed" || value === "failed"
    }

    function backupImportOperationWithRestart(decision, operation) {
        const value = operation || (decision ? decision.operation : null)
        const restartRequest = decision && decision.operation ? decision.operation.restartRequest : undefined
        if (!value || typeof value !== "object" || restartRequest === undefined || value.restartRequest !== undefined || value.restart_request !== undefined) {
            return value
        }
        const next = {}
        for (const key in value) {
            next[key] = value[key]
        }
        next.restartRequest = restartRequest
        return next
    }

    function backupImportMarkLetFinish(decision) {
        if (decision && typeof decision === "object") {
            decision.action = "let_finish"
            decision.restart = false
            decision.restartEligible = false
            decision.restartGate = null
        }
        return backupImportDecisionWithAction(decision, "let_finish", false)
    }

    function backupImportStopState(decision, operation) {
        const value = backupImportOperationWithRestart(decision, operation)
        if (value && typeof value === "object") {
            model.updateRuntimeOperation(value)
        }
        const status = String(value && value.status ? value.status : "").toLowerCase()
        if (backupImportStoppedStatus(status)) {
            return { ok: true, operation: value }
        }
        if (backupImportTerminalStatus(status)) {
            return {
                ok: false,
                operation: value,
                terminal: true,
                error: qsTr("Affected operation finished instead of stopping before backup import.")
            }
        }
        return null
    }

    function awaitBackupImportStoppedOperation(decision, initialOperation) {
        const operationId = String(decision && decision.operationId ? decision.operationId : "")
        let latest = backupImportOperationWithRestart(decision, initialOperation)
        let state = backupImportStopState(decision, latest)
        if (state !== null) {
            return state
        }
        if (!operationId.length) {
            return {
                ok: false,
                operation: latest,
                error: qsTr("Backup import could not identify an affected operation to stop.")
            }
        }
        for (let attempt = 0; attempt < 6; ++attempt) {
            const response = model.requestModule(model.inspectorModule, "runtimeOperationStatus", [operationId], qsTr("Runtime operation"), false, false)
            if (!response || !response.ok) {
                return {
                    ok: false,
                    operation: latest,
                    error: response && response.error ? response.error : qsTr("Backup import could not check whether an affected operation stopped.")
                }
            }
            latest = backupImportOperationWithRestart(decision, response.value)
            state = backupImportStopState(decision, latest)
            if (state !== null) {
                return state
            }
        }
        return {
            ok: false,
            operation: latest,
            timeout: true,
            error: qsTr("Backup import timed out waiting for an affected operation to stop.")
        }
    }

    function stopBackupImportOperations(plan) {
        const decisions = plan && Array.isArray(plan.decisions) ? plan.decisions : []
        for (let i = 0; i < decisions.length; ++i) {
            const decision = decisions[i]
            if (decision.action === "let_finish") {
                recordBackupImportDecision(decision, qsTr("Left safe affected operation running during backup import."))
                continue
            }
            if (decision.action !== "stop") {
                continue
            }
            const response = model.callInspector("runtimeOperationCancel", [decision.operationId], qsTr("Cancel operation"))
            if (!response.ok) {
                if (decision.safeToLetFinish === true) {
                    recordBackupImportDecision(backupImportMarkLetFinish(decision), qsTr("Stop failed; safe operation was left to finish."))
                    continue
                }
                model.settingsBackupStatus = response.error || qsTr("Backup import could not stop a running operation.")
                recordBackupImportDecision(backupImportDecisionWithAction(decision, "block", false), qsTr("Failed to stop affected operation before backup import."))
                return false
            }
            const stopped = awaitBackupImportStoppedOperation(decision, response.value || decision.operation)
            if (!stopped.ok) {
                if (stopped.operation && typeof stopped.operation === "object") {
                    model.updateRuntimeOperation(stopped.operation)
                }
                if (decision.safeToLetFinish === true) {
                    recordBackupImportDecision(backupImportMarkLetFinish(decision), stopped.error || qsTr("Safe operation was left to finish before backup import."))
                    continue
                }
                model.settingsBackupStatus = stopped.error || qsTr("Backup import could not stop a running operation.")
                recordBackupImportDecision(backupImportDecisionWithAction(decision, "block", false), stopped.error || qsTr("Affected operation did not stop before backup import."))
                return false
            }
            recordBackupImportDecision(decision, qsTr("Stopped affected operation before backup import."))
        }
        return true
    }

    function restartBackupImportOperations(plan) {
        const decisions = plan && Array.isArray(plan.decisions) ? plan.decisions : []
        for (let i = 0; i < decisions.length; ++i) {
            const decision = decisions[i]
            if (decision.action !== "stop") {
                continue
            }
            const request = decision.operation && decision.operation.restartRequest
            if (!request || typeof request !== "object" || decision.restartEligible !== true) {
                recordBackupImportDecision(backupImportDecisionWithAction(decision, "skip_restart", false), qsTr("Manual rerun required after backup import."))
                continue
            }
            const metadata = operationHistory.operationMetadata(decision.operation || {})
            if (!backupImportCanRestartOperation(decision.operation, metadata)) {
                const skipped = backupImportDecisionWithAction(decision, "skip_restart", false)
                skipped.restartGate = backupImportGateSummary(backupImportOperationGate(decision.operation, metadata))
                recordBackupImportDecision(skipped, qsTr("Skipped automatic restart because gates do not pass after import."))
                continue
            }
            model.runtimeOperationStart(request, false, function (response) {
                if (!response || !response.ok) {
                    const failed = backupImportDecisionWithAction(decision, "restart_failed", false)
                    failed.previousOperationId = decision.operationId
                    recordBackupImportDecision(failed, response && response.error ? response.error : qsTr("Safe read operation restart failed."))
                    return
                }
                const restarted = backupImportDecisionWithAction(decision, "restart", true)
                restarted.previousOperationId = decision.operationId
                restarted.restartOperationId = String(response.value && response.value.operationId ? response.value.operationId : "")
                if (restarted.restartOperationId.length) {
                    restarted.operationId = restarted.restartOperationId
                }
                recordBackupImportDecision(restarted, qsTr("Restarted safe read operation after backup import."))
            })
        }
    }

    function recordBackupImportDecision(decision, detail) {
        const value = decision || {}
        const action = String(value.action || "")
        const status = backupImportActionStatus(action)
        const reason = backupImportActionReason(action)
        model.appendOperationHistory({
            domain: "backup",
            method: "settingsBackupImportPolicy",
            status: status,
            label: qsTr("Backup import policy"),
            operationId: value.operationId,
            previousOperationId: value.previousOperationId,
            restartOperationId: value.restartOperationId,
            operationClass: value.operationClass,
            affectedInputs: value.affectedInputs || [],
            restartPolicy: value.restartPolicy,
            confirmationRequired: false,
            importId: value.importId,
            backupCatalogId: value.backupCatalogId,
            reason: reason,
            provenance: ["backup_import_policy", "operation_history"],
            result: {
                action: action,
                status: status,
                reason: reason,
                import_id: value.importId,
                backup_catalog_id: value.backupCatalogId,
                operation_id: value.operationId,
                previous_operation_id: value.previousOperationId || null,
                restart_operation_id: value.restartOperationId || null,
                operation_class: value.operationClass,
                restart: value.restart === true,
                restart_eligible: value.restartEligible === true,
                restart_gate: value.restartGate || null,
                safe_to_let_finish: value.safeToLetFinish === true,
                provenance: ["backup_import_policy", "operation_history"]
            }
        }, detail)
    }

    function backupImportActionStatus(action) {
        switch (String(action || "")) {
        case "stop":
            return "stopped_for_import"
        case "let_finish":
            return "let_finish_for_import"
        case "block":
            return "blocked_for_import"
        case "skip_restart":
            return "restart_skipped_for_import"
        case "restart":
            return "restarted_after_import"
        case "restart_failed":
            return "restart_failed_after_import"
        default:
            return "ignored"
        }
    }

    function backupImportActionReason(action) {
        switch (String(action || "")) {
        case "stop":
            return "affected_operation_stopped_for_import"
        case "let_finish":
            return "safe_operation_left_running_for_import"
        case "block":
            return "affected_operation_blocked_for_import"
        case "skip_restart":
            return "restart_not_safe_for_import"
        case "restart":
            return "safe_operation_restarted_after_import"
        case "restart_failed":
            return "safe_operation_restart_failed_after_import"
        default:
            return "not_applicable"
        }
    }

    function backupImportAffectedInputs(selectedAreas) {
        const rows = []
        const areas = Array.isArray(selectedAreas) ? selectedAreas : []
        for (let i = 0; i < areas.length; ++i) {
            rows.push({ key: "backup_area", value: String(areas[i] || "") })
        }
        return rows
    }

    function uploadBackupCatalogEntry(backupCatalogId) {
        if (!model.settingsBackupAvailable()) {
            model.settingsBackupStatus = qsTr("Storage upload capability is required.")
            return null
        }
        return catalog.uploadLocal(backupCatalogId, [
            model.effectiveStorageSourceMode(model.storageSourceMode),
            model.configuredStorageRestUrl(),
            model.storageMutatingDiagnosticsEnabled === true
        ])
    }

    function backupCatalogRows() {
        return catalog.rows()
    }

    function recordSettingsBackupCatalogEntry(encrypted, cid) {
        const entry = model.createLocalSettingsBackup(model.settingsBackupEncrypted ? qsTr("Encrypted settings backup") : qsTr("Settings backup"), encrypted === true, model.settingsBackupContents)
        if (!entry || !String(entry.backup_catalog_id || "").length || !String(cid || "").length) {
            return entry
        }
        return model.attachBackupRemote(entry.backup_catalog_id, cid, "logos_storage") || entry
    }
}
