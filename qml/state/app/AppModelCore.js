.import "../../services/BridgeHelpers.js" as BridgeHelpers
.import "../runtime/RuntimeOperationLifecycle.js" as RuntimeOperationLifecycle
.import "PageRegistry.js" as PageRegistry

function handleNetworkConfigurationChanged(root) {
    resetDashboardConfiguration(root)
    root.metrics.invalidateConfiguration("blockchain", qsTr("Blockchain configuration changed."))
    root.wallet.invalidateBedrockBalanceSource()
    root.chainPages.resetSourceScopedState(
        qsTr("Blockchain configuration changed."))
    sanitizeBlockchainNavigationState(root)
    root.refreshCapabilityRegistryIfLoaded()
    with (root) {
        localNodesReport = null
        localNodesError = ""
        localNodesRevision += 1
        saveSettingsState()
    }
}

function handleBlockchainConfigurationChanged(root) {
    with (root) {
        blockchainConfigurationRevision += 1
        root.handleNetworkConfigurationChanged()
    }
}

function handleMessagingConfigurationChanged(root) {
    resetDashboardConfiguration(root)
    root.metrics.invalidateConfiguration("messaging", qsTr("Messaging configuration changed."))
    root.refreshCapabilityRegistryIfLoaded()
    with (root) {
        deliveryApp.invalidateSourceRequests()
        saveSettingsState()
    }
}

function handleStorageConfigurationChanged(root) {
    resetDashboardConfiguration(root)
    root.metrics.invalidateConfiguration("storage", qsTr("Storage configuration changed."))
    root.refreshCapabilityRegistryIfLoaded()
    with (root) {
        storageApp.invalidateSourceRequests()
        saveSettingsState()
    }
}

function resetDashboardConfiguration(root) {
    with (root) {
        networkConfigurationRevision += 1
        metrics.invalidateDashboard(qsTr("Dashboard configuration changed."))
    }
}

function navTreeItems(root) {
    return PageRegistry.navTreeItems(root)
}

function navRows(root) {
    with (root) {
        const revision = shell.navRevision
        const rows = []
        appendNavRows(root, rows, navTreeItems(), 0, "")
        return rows
    }
}

function appendNavRows(root, rows, items, depth, parentKey) {
    const values = Array.isArray(items) ? items : []
    for (let i = 0; i < values.length; ++i) {
        const item = values[i]
        const isGroup = String(item.type || "") === "group"
        const row = {
            type: isGroup ? "group" : "item",
            key: item.key,
            view: item.view,
            label: item.label,
            token: item.token,
            layer: item.layer,
            parentKey: parentKey,
            active: isGroup
                ? PageRegistry.navItemContainsView(item, root.shell.currentView)
                : root.shell.currentView === item.view,
            depth: depth
        }
        if (isGroup) {
            row.expanded = navGroupExpanded(root, item.key)
        }
        rows.push(row)
        if (isGroup && !row.expanded) {
            continue
        }
        appendNavRows(root, rows, item.children || [], depth + 1,
            String(item.key || ""))
    }
}

function navGroupExpanded(root, key) {
    with (root) {
        const revision = shell.navRevision
        return shell.navExpanded[String(key || "")] === true
    }
}

function toggleNavGroup(root, key) {
    with (root) {
        const groupKey = String(key || "")
        if (!groupKey.length) {
            return
        }
        const next = copyMap(shell.navExpanded)
        next[groupKey] = next[groupKey] !== true
        shell.navExpanded = next
        shell.navRevision += 1
    }
}

function expandNavGroupForView(root, view) {
    with (root) {
        const ancestorKeys = PageRegistry.ancestorNavKeysForView(root, view)
        if (ancestorKeys.length === 0) {
            return
        }
        const next = copyMap(shell.navExpanded)
        let changed = false
        for (let i = 0; i < ancestorKeys.length; ++i) {
            const key = String(ancestorKeys[i] || "")
            if (key.length > 0 && next[key] !== true) {
                next[key] = true
                changed = true
            }
        }
        if (!changed) {
            return
        }
        shell.navExpanded = next
        shell.navRevision += 1
    }
}

function parentNavKeyForView(root, view) {
    return PageRegistry.parentNavKeyForView(root, view)
}

function navItemForView(root, view) {
    return PageRegistry.navItemForView(root, view)
}

function layerForView(root, view) {
    return PageRegistry.layerForView(root, view)
}

function navLabelForView(root, view) {
    return PageRegistry.navLabelForView(root, view)
}

function navTokenForView(root, view) {
    return PageRegistry.navTokenForView(root, view)
}

function navItemForQuery(root, query) {
    return PageRegistry.navItemForQuery(root, query)
}

function navItemMatches(root, item, normalized) {
    return PageRegistry.navItemMatches(item, normalized)
}

function viewTitle(root) {
    return PageRegistry.viewTitle(root)
}

function normalizedNavigationView(root, requestedView) {
    return PageRegistry.normalizedNavigationView(requestedView)
}

function cloneNavigationValue(root, value) {
    if (value === undefined || value === null) {
        return null
    }
    if (typeof value !== "object") {
        return value
    }
    try {
        return JSON.parse(JSON.stringify(value))
    } catch (error) {
        return value
    }
}

function blockchainResultOwner(owner) {
    const value = String(owner || "")
    return value === "overview"
        || value === "blocks"
        || value === "transactions"
        || value === "blockDetail"
        || value === "transactionDetail"
        || value === "blockchain"
}

function sanitizedBlockchainNavigationSnapshot(root, snapshot) {
    const sanitized = cloneNavigationValue(root, snapshot)
    if (!sanitized || typeof sanitized !== "object") {
        return snapshot
    }
    const values = sanitized.values && typeof sanitized.values === "object"
        ? sanitized.values : ({})
    values.blockDetailValue = null
    values.blockDetailError = ""
    values.transactionDetailValue = null
    values.transactionDetailError = ""
    if (blockchainResultOwner(values.resultOwner)) {
        values.statusText = qsTr("Ready")
        values.resultTitle = qsTr("Output")
        values.resultText = ""
        values.resultValue = null
        values.resultIsError = false
        values.resultOwner = ""
    }
    sanitized.values = values
    sanitized.label = navigationSnapshotLabel(root, sanitized)
    sanitized.signature = navigationSnapshotSignature(root, sanitized)
    return sanitized
}

function sanitizeBlockchainNavigationStack(root, stack) {
    const source = Array.isArray(stack) ? stack : []
    const sanitized = []
    for (let i = 0; i < source.length; ++i) {
        sanitized.push(sanitizedBlockchainNavigationSnapshot(root, source[i]))
    }
    return sanitized
}

function sanitizeBlockchainNavigationState(root) {
    root.shell.navigationBackStack = sanitizeBlockchainNavigationStack(
        root, root.shell.navigationBackStack)
    root.shell.navigationForwardStack = sanitizeBlockchainNavigationStack(
        root, root.shell.navigationForwardStack)
    if (blockchainResultOwner(root.shell.resultOwner)) {
        clearDisplayedResult(root)
        if (!root.asyncPresentationBusy) {
            root.shell.statusText = qsTr("Ready")
        }
    }
    root.shell.navigationRevision += 1
}

function navigationSnapshot(root) {
    const values = {
        statusText: String(root.shell.statusText || ""),
        resultTitle: String(root.shell.resultTitle || ""),
        resultText: String(root.shell.resultText || ""),
        resultValue: cloneNavigationValue(root, root.shell.resultValue),
        resultIsError: root.shell.resultIsError === true,
        resultOwner: String(root.shell.resultOwner || ""),
        blockDetailValue: cloneNavigationValue(root, root.blockDetailValue),
        blockDetailError: String(root.blockDetailError || ""),
        transactionDetailValue: cloneNavigationValue(root, root.transactionDetailValue),
        transactionDetailError: String(root.transactionDetailError || ""),
        storageAppTab: String(root.storageAppTab || ""),
        storageDiagnosticsTab: String(root.storageDiagnosticsTab || ""),
        deliveryAppTab: String(root.deliveryAppTab || ""),
        programTab: String(root.programTab || ""),
        localWalletTab: String(root.localWalletTab || ""),
        localWalletLookupTarget: String(root.localWalletLookupTarget || ""),
        walletPublicKeyProbe: String(root.walletPublicKeyProbe || ""),
        storageCidProbe: String(root.storageCidProbe || ""),
        settingsSection: String(root.shell.settingsSection || ""),
        settingsNetworkSection: String(root.shell.settingsNetworkSection || ""),
        settingsUiSection: String(root.shell.settingsUiSection || "")
    }
    const currentView = String(root.shell.currentView || "")
    values.inspectionEntityRef = currentView === "zones"
            || currentView === "sequencerDashboard"
        ? cloneNavigationValue(root, root.currentInspectionEntityRef) : null
    const snapshot = {
        view: normalizedNavigationView(root, root.shell.currentView),
        values: values,
        label: ""
    }
    snapshot.label = navigationSnapshotLabel(root, snapshot)
    snapshot.signature = navigationSnapshotSignature(root, snapshot)
    return snapshot
}

function navigationSnapshotSignature(root, snapshot) {
    const value = snapshot || {}
    try {
        return JSON.stringify({
            view: String(value.view || ""),
            values: value.values || {}
        })
    } catch (error) {
        return String(value.view || "") + "|" + String(value.label || "")
    }
}

function navigationSnapshotsEqual(root, left, right) {
    if (!left || !right) {
        return false
    }
    const leftSignature = String(left.signature || navigationSnapshotSignature(root, left))
    const rightSignature = String(right.signature || navigationSnapshotSignature(root, right))
    return leftSignature.length > 0 && leftSignature === rightSignature
}

function pushNavigationHistory(root) {
    if (root.shell.navigationRestoring) {
        return
    }
    const snapshot = navigationSnapshot(root)
    if (!String(snapshot.view || "").length) {
        return
    }
    const back = Array.isArray(root.shell.navigationBackStack)
        ? root.shell.navigationBackStack.slice(0) : []
    const previous = back.length > 0 ? back[back.length - 1] : null
    if (previous && navigationSnapshotsEqual(root, previous, snapshot)) {
        return
    }
    back.push(snapshot)
    while (back.length > root.shell.navigationHistoryLimit) {
        back.shift()
    }
    root.shell.navigationBackStack = back
    root.shell.navigationForwardStack = []
    root.shell.navigationRevision += 1
}

function restoreNavigationSnapshot(root, snapshot) {
    if (!snapshot || typeof snapshot !== "object") {
        return
    }
    const values = snapshot.values && typeof snapshot.values === "object" ? snapshot.values : ({})
    const targetView = normalizedNavigationView(root, snapshot.view).length ? normalizedNavigationView(root, snapshot.view) : "overview"
    root.shell.navigationRestoring = true
    try {
        root.shell.statusText = String(values.statusText || qsTr("Ready"))
        root.shell.resultGeneration += 1
        root.shell.resultTitle = String(values.resultTitle || qsTr("Output"))
        root.shell.resultText = String(values.resultText || "")
        root.shell.resultValue = cloneNavigationValue(root, values.resultValue)
        root.shell.resultIsError = values.resultIsError === true
        root.shell.resultOwner = String(values.resultOwner || "")
        root.blockDetailValue = cloneNavigationValue(root, values.blockDetailValue)
        root.blockDetailError = String(values.blockDetailError || "")
        root.transactionDetailValue = cloneNavigationValue(root, values.transactionDetailValue)
        root.transactionDetailError = String(values.transactionDetailError || "")
        root.storageAppTab = String(values.storageAppTab || root.storageAppTab)
        root.storageDiagnosticsTab = String(values.storageDiagnosticsTab
            || root.storageDiagnosticsTab)
        root.deliveryAppTab = String(values.deliveryAppTab || root.deliveryAppTab)
        root.programTab = String(values.programTab || root.programTab)
        root.localWalletTab = String(values.localWalletTab || root.localWalletTab)
        root.localWalletLookupTarget = String(values.localWalletLookupTarget || "")
        root.walletPublicKeyProbe = String(values.walletPublicKeyProbe || "")
        root.storageCidProbe = String(values.storageCidProbe || "")
        root.shell.settingsSection = String(values.settingsSection || root.shell.settingsSection)
        root.shell.settingsNetworkSection = String(values.settingsNetworkSection
            || root.shell.settingsNetworkSection)
        root.shell.settingsUiSection = String(values.settingsUiSection
            || root.shell.settingsUiSection)
        expandNavGroupForView(root, targetView)
        root.shell.currentView = targetView
        if (values.inspectionEntityRef) {
            const entity = cloneNavigationValue(root, values.inspectionEntityRef)
            Qt.callLater(function () {
                root.openInspectionEntityRef(entity, false)
            })
        } else {
            root.currentInspectionEntityRef = null
        }
    } finally {
        root.shell.navigationRestoring = false
    }
}

function canNavigateBack(root) {
    const revision = root.shell.navigationRevision
    return Array.isArray(root.shell.navigationBackStack)
        && root.shell.navigationBackStack.length > 0
}

function canNavigateForward(root) {
    const revision = root.shell.navigationRevision
    return Array.isArray(root.shell.navigationForwardStack)
        && root.shell.navigationForwardStack.length > 0
}

function navigateBack(root) {
    const back = Array.isArray(root.shell.navigationBackStack)
        ? root.shell.navigationBackStack.slice(0) : []
    if (!back.length) {
        return
    }
    const target = back.pop()
    const current = navigationSnapshot(root)
    const forward = Array.isArray(root.shell.navigationForwardStack)
        ? root.shell.navigationForwardStack.slice(0) : []
    if (!navigationSnapshotsEqual(root, current, target)) {
        forward.push(current)
    }
    root.shell.navigationBackStack = back
    root.shell.navigationForwardStack = forward
    root.shell.navigationRevision += 1
    restoreNavigationSnapshot(root, target)
}

function navigateForward(root) {
    const forward = Array.isArray(root.shell.navigationForwardStack)
        ? root.shell.navigationForwardStack.slice(0) : []
    if (!forward.length) {
        return
    }
    const target = forward.pop()
    const current = navigationSnapshot(root)
    const back = Array.isArray(root.shell.navigationBackStack)
        ? root.shell.navigationBackStack.slice(0) : []
    if (!navigationSnapshotsEqual(root, current, target)) {
        back.push(current)
    }
    while (back.length > root.shell.navigationHistoryLimit) {
        back.shift()
    }
    root.shell.navigationBackStack = back
    root.shell.navigationForwardStack = forward
    root.shell.navigationRevision += 1
    restoreNavigationSnapshot(root, target)
}

function navigationBackLabel(root) {
    const revision = root.shell.navigationRevision
    const stack = Array.isArray(root.shell.navigationBackStack)
        ? root.shell.navigationBackStack : []
    return stack.length ? navigationSnapshotDisplayLabel(root, stack[stack.length - 1]) : ""
}

function navigationForwardLabel(root) {
    const revision = root.shell.navigationRevision
    const stack = Array.isArray(root.shell.navigationForwardStack)
        ? root.shell.navigationForwardStack : []
    return stack.length ? navigationSnapshotDisplayLabel(root, stack[stack.length - 1]) : ""
}

function navigationSnapshotDisplayLabel(root, snapshot) {
    const label = String(snapshot && snapshot.label ? snapshot.label : "")
    if (label.length) {
        return label
    }
    return navigationSnapshotLabel(root, snapshot || {})
}

function navigationSnapshotLabel(root, snapshot) {
    const targetView = normalizedNavigationView(root, snapshot.view)
    const values = snapshot.values && typeof snapshot.values === "object" ? snapshot.values : ({})
    const base = navLabelForView(root, targetView) || qsTr("Dashboard")
    if (targetView === "blockDetail") {
        return navigationLabelWithDetail(root, base, navigationObjectValue(values.blockDetailValue, ["hash", "block_id", "slot", "height"]))
    }
    if (targetView === "transactionDetail") {
        return navigationLabelWithDetail(root, base, navigationObjectValue(values.transactionDetailValue, ["hash", "transaction_hash", "tx_hash"]))
    }
    if (targetView === "storage" && String(values.storageCidProbe || "").length) {
        return navigationLabelWithDetail(root, base, values.storageCidProbe)
    }
    if (targetView === "localWallet" && String(values.localWalletLookupTarget || "").length) {
        return navigationLabelWithDetail(root, base, values.localWalletLookupTarget)
    }
    if (targetView === "settings" && String(values.settingsSection || "").length) {
        return navigationLabelWithDetail(root, base, values.settingsSection)
    }
    return base
}

function navigationObjectValue(value, keys) {
    if (!value || typeof value !== "object") {
        return ""
    }
    const names = Array.isArray(keys) ? keys : []
    for (let i = 0; i < names.length; ++i) {
        const current = value[names[i]]
        if (current !== undefined && current !== null && String(current).length) {
            return String(current)
        }
    }
    return ""
}

function navigationLabelWithDetail(root, label, detail) {
    const value = String(detail || "")
    if (!value.length) {
        return String(label || "")
    }
    return qsTr("%1 %2").arg(String(label || "")).arg(shortNavigationText(root, value))
}

function shortNavigationText(root, value) {
    const text = String(value || "")
    if (text.length <= 18) {
        return text
    }
    return text.slice(0, 10) + "..." + text.slice(text.length - 6)
}

function selectView(root, requestedView, recordHistory) {
    const targetView = normalizedNavigationView(root, requestedView)
    with (root) {
        if (!targetView.length) {
            return
        }
        if (recordHistory !== false && shell.currentView !== targetView) {
            pushNavigationHistory()
        }
        expandNavGroupForView(targetView)
        shell.currentView = targetView
        shell.statusText = qsTr("Ready")
    }
}

function openSettings(root, section, subsection, recordHistory) {
    with (root) {
        const targetSection = String(section || "")
        const targetSubsection = String(subsection || "")
        const sectionChanged = targetSection.length > 0
            && shell.settingsSection !== targetSection
        const networkChanged = targetSection === "network" && targetSubsection.length > 0
            && shell.settingsNetworkSection !== targetSubsection
        const uiChanged = targetSection === "ui" && targetSubsection.length > 0
            && shell.settingsUiSection !== targetSubsection
        if (recordHistory !== false && (shell.currentView !== "settings"
                || sectionChanged || networkChanged || uiChanged)) {
            pushNavigationHistory()
        }
        selectView("settings", false)
        if (targetSection.length) {
            shell.settingsSection = targetSection
        }
        if (targetSection === "network" && targetSubsection.length) {
            shell.settingsNetworkSection = targetSubsection
        }
        if (targetSection === "ui" && targetSubsection.length) {
            shell.settingsUiSection = targetSubsection
        }
        shell.statusText = qsTr("Ready")
    }
}

function clearResult(root) {
    root.shell.resultGeneration += 1
    clearDisplayedResult(root)
}

function clearDisplayedResult(root) {
    with (root) {
        shell.resultTitle = qsTr("Output")
        shell.resultText = ""
        shell.resultValue = null
        shell.resultIsError = false
        shell.resultOwner = ""
    }
}

function setResult(root, title, text, isError, value, owner) {
    with (root) {
        shell.resultGeneration += 1
        shell.resultTitle = title
        shell.resultText = text
        shell.resultValue = value === undefined ? null : value
        shell.resultIsError = isError
        shell.resultOwner = owner === undefined ? shell.currentView : String(owner || "")
        shell.statusText = isError ? qsTr("Error") : qsTr("Ready")
    }
}

function pageHasOutput(root, view) {
    with (root) {
        return shell.resultOwner === view
            && (shell.resultText.length > 0 || shell.resultValue !== null)
    }
}

function runtimeOperationStart(root, request, showResult, callback) {
    return RuntimeOperationLifecycle.runtimeOperationStart(root, request, showResult, callback)
}

function runtimeOperationStatus(root, operationId, showResult, callback) {
    return RuntimeOperationLifecycle.runtimeOperationStatus(root, operationId, showResult, callback)
}

function runtimeOperationEvents(root, operationId, afterSeq, showResult, callback) {
    return RuntimeOperationLifecycle.runtimeOperationEvents(root, operationId, afterSeq, showResult, callback)
}

function runtimeOperationCancel(root, operationId, showResult, callback) {
    return RuntimeOperationLifecycle.runtimeOperationCancel(root, operationId, showResult, callback)
}

function runtimeOperationModuleEvent(root, event, showResult, callback) {
    return RuntimeOperationLifecycle.runtimeOperationModuleEvent(root, event, showResult, callback)
}

function updateRuntimeOperation(root, operation) {
    return RuntimeOperationLifecycle.updateRuntimeOperation(root, operation)
}

function coreUpdateRuntimeOperation(root, operation) {
    return RuntimeOperationLifecycle.coreUpdateRuntimeOperation(root, operation)
}

function runtimeOperationTerminal(root, operation) {
    return RuntimeOperationLifecycle.runtimeOperationTerminal(root, operation)
}

function runtimeOperationResponse(root, operation) {
    return RuntimeOperationLifecycle.runtimeOperationResponse(root, operation)
}

function appendRuntimeOperationHistory(root, operation, detail) {
    return RuntimeOperationLifecycle.appendRuntimeOperationHistory(root, operation, detail)
}

function appendOperationHistory(root, operation, detail) {
    return RuntimeOperationLifecycle.appendOperationHistory(root, operation, detail)
}

function runtimeOperationHistoryRows(root, domain) {
    return RuntimeOperationLifecycle.runtimeOperationHistoryRows(root, domain)
}

function nodeOperationStart(root, request, showResult, callback) {
    return runtimeOperationStart(root, request, showResult, callback)
}

function nodeOperationStatus(root, operationId, showResult, callback) {
    return runtimeOperationStatus(root, operationId, showResult, callback)
}

function nodeOperationEvents(root, operationId, afterSeq, showResult, callback) {
    return runtimeOperationEvents(root, operationId, afterSeq, showResult, callback)
}

function nodeOperationCancel(root, operationId, showResult, callback) {
    return runtimeOperationCancel(root, operationId, showResult, callback)
}

function updateNodeOperation(root, operation) {
    return updateRuntimeOperation(root, operation)
}

function nodeOperationTerminal(root, operation) {
    return runtimeOperationTerminal(root, operation)
}

function nodeOperationResponse(root, operation) {
    return runtimeOperationResponse(root, operation)
}

function appendNodeOperationHistory(root, operation, detail) {
    return appendRuntimeOperationHistory(root, operation, detail)
}

function nodeOperationHistoryRows(root, domain) {
    return runtimeOperationHistoryRows(root, domain)
}

function operationHistoryRows(root, domain) {
    return RuntimeOperationLifecycle.operationHistoryRows(root, domain)
}

function runtimeOperationDetail(root, operation) {
    return RuntimeOperationLifecycle.runtimeOperationDetail(root, operation)
}

function decodeAccountData(root, dataHex, idlJson, accountType) {
    with (root) {
        if (shell.busy) {
            return {
                ok: false,
                text: "",
                value: null,
                error: qsTr("Another inspection is already running.")
            }
        }

        const args = [String(dataHex || ""), String(idlJson || ""), String(accountType || "")]
        return bridge.callModule(inspectorModule, "decodeAccount", args)
    }
}

function decodeAccountDataAsync(root, dataHex, idlJson, accountType, callback) {
    with (root) {
        const args = [String(dataHex || ""), String(idlJson || ""), String(accountType || "")]
        return requestModuleAsync(inspectorModule, "decodeAccount", args, qsTr("Account decode"), false, callback)
    }
}

function decodeTransactionSummaryAsync(root, summary, idlJson, callback) {
    with (root) {
        return requestModuleAsync(inspectorModule, "decodeTransactionSummary", [summary || {}, String(idlJson || "")], qsTr("Transaction decode"), false, callback)
    }
}

function resolveAccountDecodeSessionAsync(root, dataHex, accountId, candidates, callback) {
    return selectAccountDecodeSessionAsync(root, dataHex, accountId, "", candidates, callback)
}

function selectAccountDecodeSessionAsync(root, dataHex, accountId, ownerProgramId, candidates, callback) {
    with (root) {
        return requestModuleAsync(
            inspectorModule,
            "selectAccountDecodeSession",
            [String(dataHex || ""), String(accountId || ""), Array.isArray(candidates) ? candidates : [], String(ownerProgramId || "")],
            qsTr("Account decode"),
            false,
            callback
        )
    }
}

function resolveTransactionDecodeSessionAsync(root, summary, candidates, callback) {
    return selectTransactionDecodeSessionAsync(root, summary, candidates, callback)
}

function selectTransactionDecodeSessionAsync(root, summary, candidates, callback) {
    with (root) {
        return requestModuleAsync(
            inspectorModule,
            "selectTransactionDecodeSession",
            [summary || {}, Array.isArray(candidates) ? candidates : []],
            qsTr("Transaction decode"),
            false,
            callback
        )
    }
}
