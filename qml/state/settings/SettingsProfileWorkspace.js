.import "../status/StatusFieldCatalog.js" as StatusFieldCatalog

function connectionStatus(root, kind) {
    return root.model.metrics.networkConnectionState(kind)
}

function connectionStatusText(root, kind) {
    const status = connectionStatus(root, kind)
    if (!status.known) {
        return qsTr("Unknown")
    }
    return status.ok ? qsTr("OK") : qsTr("Error")
}

function connectionStatusDetail(root, kind) {
    const status = connectionStatus(root, kind)
    if (!status.known) {
        const rate = root.model.metrics.networkConnectionRate(kind)
        return rate > 0
            ? qsTr("Not queried. Auto refresh runs every %1 seconds.").arg(rate)
            : qsTr("Not queried. Auto refresh is off.")
    }
    const checked = status.checkedAt && status.checkedAt.length ? qsTr(" at %1").arg(status.checkedAt) : ""
    return qsTr("%1%2").arg(status.detail || "").arg(checked)
}

function connectionStatusColor(root, kind) {
    const status = connectionStatus(root, kind)
    if (!status.known) {
        return root.theme.textMuted
    }
    return status.ok ? root.theme.success : root.theme.warning
}

function walletSourceStatusText(root) {
    const status = root.model.localWalletStatus || null
    if (!status) {
        return root.model.localWalletStatusError.length ? qsTr("Down") : qsTr("Unknown")
    }
    const value = String(status.status || "unknown")
    return value.length ? value[0].toUpperCase() + value.slice(1) : qsTr("Unknown")
}

function walletSourceStatusDetail(root) {
    const status = root.model.localWalletStatus || null
    if (root.model.localWalletStatusError.length) {
        return root.model.localWalletStatusError
    }
    if (status && status.detail) {
        return String(status.detail)
    }
    return qsTr("Not checked")
}

function walletSourceStatusColor(root) {
    const status = root.model.localWalletStatus || null
    const value = status && status.status ? String(status.status) : ""
    if (root.model.localWalletStatusError.length || value === "down") {
        return root.theme.error
    }
    if (!value.length || value === "degraded" || value === "unknown") {
        return root.theme.warning
    }
    if (value === "ok") {
        return root.theme.success
    }
    return root.theme.textMuted
}

function walletBackupHint(root) {
    if (!root.model.settingsBackupEncrypted) {
        return qsTr("Plain backup. Use wallet encryption for private or portable profiles.")
    }
    if (!root.model.walletHomeConfigured()) {
        return qsTr("Configure Wallet home before encrypted backup or restore.")
    }
    return qsTr("Encrypted restore requires the same wallet config.")
}

function updateEndpoint(root, key, value) {
    root.model[key] = String(value || "").trim()
    syncProfileFromEndpoints(root)
}

function syncProfileFromEndpoints(root) {
    root.model.networkProfile = root.inferProfile(root.model.nodeUrl)
}

function applyProfileIndex(root, index) {
    root.model.applyProfileIndex(index)
}

function sourceIndexFor(root, family, value, optionsModel) {
    return root.model.sourceRouting.sourceModeIndexFor(family, value, optionsModel)
}

function sourceModeAt(root, index, optionsModel) {
    return root.model.sourceRouting.sourceModeAt(index, optionsModel)
}

function refreshSourceOptions(root, coreOptions, deliveryOptions, storageOptions) {
    populateSourceOptions(root, coreOptions, "core")
    populateSourceOptions(root, deliveryOptions, "delivery")
    populateSourceOptions(root, storageOptions, "storage")
}

function refreshProfileOptions(root, profileOptions) {
    profileOptions.clear()
    const options = root.model.networkProfileOptions()
    for (let i = 0; i < options.length; ++i) {
        profileOptions.append(options[i])
    }
}

function populateSourceOptions(root, targetModel, family) {
    targetModel.clear()
    const options = root.model.sourceRouting.sourceModeOptions(family)
    for (let i = 0; i < options.length; ++i) {
        targetModel.append(options[i])
    }
}

function profileIndexFor(root, value) {
    return root.model.profileIndexFor(value)
}

function inferProfile(root, node) {
    return root.model.inferNetworkProfileFromEndpoint(node)
}

function profileLabel(root, value) {
    return root.model.networkProfileLabel(value)
}

function profileSummary(root, value) {
    return root.model.networkProfileSummary(value)
}

function profileDetail(root) {
    return root.model.networkProfileDetail()
}

function normalizeEndpoint(root, value) {
    return root.model.normalizeEndpoint(value)
}

function shortEndpoint(value) {
    const text = String(value || "")
    if (!text.length) {
        return qsTr("Not configured")
    }
    return text.replace(/^https?:\/\//, "").replace(/\/$/, "")
}

function footerFieldGroups() {
    return StatusFieldCatalog.footerSelectorGroups()
}

function dashboardGraphGroups() {
    return StatusFieldCatalog.dashboardGraphGroups()
}

function resetPendingSettingsRestoreOptions(dialog) { dialog.reset() }
function copyPendingSettingsRestoreOptions(dialog) { return dialog.copyOptions() }
function copyNestedOptionMap(dialog, source) { return dialog.copyNestedOptionMap(source) }
function copyFlatOptionMap(dialog, source) { return dialog.copyFlatOptionMap(source) }
function setPendingImportMode(dialog, area, mode) { dialog.setMode(area, mode) }
function pendingImportItemRows(dialog, area) { return dialog.itemRows(area) }
function pendingImportItemSelected(dialog, area, key) { return dialog.itemSelected(area, key) }
function setPendingImportItemSelected(dialog, area, key, selected) { dialog.setItemSelected(area, key, selected) }
function pendingImportConflictRows(dialog) { return dialog.conflictRows() }
function pendingImportConflictDecision(dialog, area, key) { return dialog.conflictDecision(area, key) }
function conflictDecisionIndexFor(dialog, area, key, optionsModel) { return dialog.conflictDecisionIndexFor(area, key, optionsModel) }
function setPendingImportConflictDecision(dialog, area, key, decision) { dialog.setConflictDecision(area, key, decision) }
function pendingImportHasRequiredConflicts(dialog) { return dialog.hasRequiredConflicts() }
function importModeIndexFor(dialog, area, optionsModel) { return dialog.modeIndexFor(area, optionsModel) }
function importModeAt(dialog, index, optionsModel) { return dialog.modeAt(index, optionsModel) }
function previewPendingLocalRestore(dialog) { return dialog.preview() }
function pendingImportPlanText(dialog) { return dialog.planText() }
function pendingImportConfirmEnabled(dialog) { return dialog.confirmEnabled() }
function pendingImportModeText(dialog) { return dialog.modeText() }
function appendPendingImportMode(dialog, rows, label, mode) { dialog.appendMode(rows, label, mode) }
function importModeLabel(dialog, mode) { return dialog.modeLabel(mode) }
function pendingImportOperationText(dialog, plan) { return dialog.operationText(plan) }
function pendingImportWarningText(dialog, plan) { return dialog.warningText(plan) }
function pendingImportSelectedAreas(dialog) { return dialog.selectedAreas() }
