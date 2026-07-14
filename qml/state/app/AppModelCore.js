.import "../../services/BridgeHelpers.js" as BridgeHelpers
.import "../runtime/RuntimeOperationLifecycle.js" as RuntimeOperationLifecycle
.import "PageRegistry.js" as PageRegistry

function handleNetworkConfigurationChanged(root) {
    resetDashboardConfiguration(root)
    clearNetworkConnectionFamily(root, "blockchain")
    with (root) {
        blockchainModuleReport = null
        localNodesReport = null
        localNodesError = ""
        localNodesRevision += 1
        blocksLiveEnabled = false
        blocksLiveError = ""
        blocksLiveSource = ""
        blocksLiveUnknownEvents = 0
        blocksLiveCheckedAt = ""
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
    clearNetworkConnectionFamily(root, "messaging")
    with (root) {
        root.clearDashboardMetricHistoryForPrefix("messaging.")
        messagingModuleReport = null
        messagingSourceReport = null
        deliveryApp.invalidateSourceRequests()
        saveSettingsState()
    }
}

function handleStorageConfigurationChanged(root) {
    resetDashboardConfiguration(root)
    clearNetworkConnectionFamily(root, "storage")
    with (root) {
        root.clearDashboardMetricHistoryForPrefix("storage.")
        storageModuleReport = null
        storageSourceReport = null
        storageApp.invalidateSourceRequests()
        saveSettingsState()
    }
}

function resetDashboardConfiguration(root) {
    with (root) {
        networkConfigurationRevision += 1
        dashboardOverview = null
        dashboardNode = null
        dashboardL1Blocks = []
        dashboardBlocks = []
        dashboardProvisionalBlocks = []
        dashboardLezBlockRows = []
        dashboardError = ""
        dashboardRefreshing = false
        dashboardRefreshSerial += 1
        if (chainPages) {
            chainPages.invalidateOperationCaller("dashboard.node",
                qsTr("Dashboard configuration changed."))
            chainPages.invalidateOperationCaller("dashboard.live",
                qsTr("Dashboard configuration changed."))
        }
    }
}

function clearNetworkConnectionFamily(root, family) {
    with (root) {
        networkConnectionStatus = mapWithoutKey(networkConnectionStatus, family)
        networkConnectionStatusRevision += 1
        networkConnectionPending = mapWithoutKey(networkConnectionPending, family)
        networkConnectionPendingRevision += 1
    }
}

function mapWithoutKey(value, key) {
    const next = ({})
    const source = value && typeof value === "object" ? value : ({})
    for (const currentKey in source) {
        if (currentKey !== key) {
            next[currentKey] = source[currentKey]
        }
    }
    return next
}

function navTreeItems(root) {
    return PageRegistry.navTreeItems(root)
}

function navRows(root) {
    with (root) {
        const revision = shell.navRevision
        const rows = []
        const parentKey = parentNavKeyForView(shell.currentView)
        const tree = navTreeItems()
        for (let i = 0; i < tree.length; ++i) {
            const item = tree[i]
            if (item.type === "group") {
                rows.push({
                    type: "group",
                    key: item.key,
                    label: item.label,
                    token: item.token,
                    layer: item.layer,
                    expanded: navGroupExpanded(item.key),
                    active: parentKey === item.key,
                    depth: 0
                })
                if (!navGroupExpanded(item.key)) {
                    continue
                }
                const children = item.children || []
                for (let j = 0; j < children.length; ++j) {
                    const child = children[j]
                    rows.push({
                        type: "item",
                        key: child.key,
                        view: child.view,
                        label: child.label,
                        token: child.token,
                        layer: child.layer,
                        parentKey: item.key,
                        active: shell.currentView === child.view,
                        depth: 1
                    })
                }
                continue
            }
            rows.push({
                type: "item",
                key: item.key,
                view: item.view,
                label: item.label,
                token: item.token,
                layer: item.layer,
                active: shell.currentView === item.view,
                depth: 0
            })
        }
        return rows
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
        const parentKey = parentNavKeyForView(view)
        if (!parentKey || shell.navExpanded[parentKey] === true) {
            return
        }
        const next = copyMap(shell.navExpanded)
        next[parentKey] = true
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
    values.inspectionEntityRef = String(root.shell.currentView || "") === "zones"
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
    with (root) {
        shell.resultGeneration += 1
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
