.import "../../services/BridgeHelpers.js" as BridgeHelpers
.import "PageRegistry.js" as PageRegistry

function handleNetworkConfigurationChanged(root) {
    with (root) {
        networkConfigurationRevision += 1
        networkConnectionStatus = ({})
        networkConnectionStatusRevision += 1
        networkConnectionPending = ({})
        networkConnectionPendingRevision += 1
        dashboardOverview = null
        dashboardNode = null
        dashboardL1Blocks = []
        dashboardBlocks = []
        dashboardSequencerBlocks = []
        dashboardError = ""
        dashboardRefreshing = false
        dashboardRefreshSerial += 1
        blockchainModuleReport = null
        storageModuleReport = null
        messagingModuleReport = null
        storageActiveOperation = null
        storageActiveOperationRevision += 1
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

function handleMessagingConfigurationChanged(root) {
    with (root) {
        root.clearDashboardMetricHistoryForPrefix("messaging.")
        handleNetworkConfigurationChanged()
    }
}

function handleStorageConfigurationChanged(root) {
    with (root) {
        root.clearDashboardMetricHistoryForPrefix("storage.")
        handleNetworkConfigurationChanged()
    }
}

function navTreeItems(root) {
    return PageRegistry.navTreeItems(root)
}

function navRows(root) {
    with (root) {
        const revision = navRevision
        const rows = []
        const parentKey = parentNavKeyForView(currentView)
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
                        active: currentView === child.view,
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
                active: currentView === item.view,
                depth: 0
            })
        }
        return rows
    }
}

function navGroupExpanded(root, key) {
    with (root) {
        const revision = navRevision
        return navExpanded[String(key || "")] === true
    }
}

function toggleNavGroup(root, key) {
    with (root) {
        const groupKey = String(key || "")
        if (!groupKey.length) {
            return
        }
        const next = copyMap(navExpanded)
        next[groupKey] = next[groupKey] !== true
        navExpanded = next
        navRevision += 1
    }
}

function expandNavGroupForView(root, view) {
    with (root) {
        const parentKey = parentNavKeyForView(view)
        if (!parentKey || navExpanded[parentKey] === true) {
            return
        }
        const next = copyMap(navExpanded)
        next[parentKey] = true
        navExpanded = next
        navRevision += 1
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
        statusText: String(root.statusText || ""),
        resultTitle: String(root.resultTitle || ""),
        resultText: String(root.resultText || ""),
        resultValue: cloneNavigationValue(root, root.resultValue),
        resultIsError: root.resultIsError === true,
        resultOwner: String(root.resultOwner || ""),
        blockDetailValue: cloneNavigationValue(root, root.blockDetailValue),
        blockDetailError: String(root.blockDetailError || ""),
        transactionDetailValue: cloneNavigationValue(root, root.transactionDetailValue),
        transactionDetailError: String(root.transactionDetailError || ""),
        accountDetailValue: cloneNavigationValue(root, root.accountDetailValue),
        transferRecipientDetailValue: cloneNavigationValue(root, root.transferRecipientDetailValue),
        channelDetailValue: cloneNavigationValue(root, root.channelDetailValue),
        channelDetailError: String(root.channelDetailError || ""),
        sequencerTab: String(root.sequencerTab || ""),
        storageAppTab: String(root.storageAppTab || ""),
        deliveryAppTab: String(root.deliveryAppTab || ""),
        accountTab: String(root.accountTab || ""),
        programTab: String(root.programTab || ""),
        indexerTab: String(root.indexerTab || ""),
        localWalletTab: String(root.localWalletTab || ""),
        localWalletLookupTarget: String(root.localWalletLookupTarget || ""),
        walletPublicKeyProbe: String(root.walletPublicKeyProbe || ""),
        storageCidProbe: String(root.storageCidProbe || ""),
        settingsSection: String(root.settingsSection || ""),
        settingsNetworkSection: String(root.settingsNetworkSection || ""),
        settingsUiSection: String(root.settingsUiSection || "")
    }
    const snapshot = {
        view: normalizedNavigationView(root, root.currentView),
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
    if (root.navigationRestoring) {
        return
    }
    const snapshot = navigationSnapshot(root)
    if (!String(snapshot.view || "").length) {
        return
    }
    const back = Array.isArray(root.navigationBackStack) ? root.navigationBackStack.slice(0) : []
    const previous = back.length > 0 ? back[back.length - 1] : null
    if (previous && navigationSnapshotsEqual(root, previous, snapshot)) {
        return
    }
    back.push(snapshot)
    while (back.length > root.navigationHistoryLimit) {
        back.shift()
    }
    root.navigationBackStack = back
    root.navigationForwardStack = []
    root.navigationRevision += 1
}

function restoreNavigationSnapshot(root, snapshot) {
    if (!snapshot || typeof snapshot !== "object") {
        return
    }
    const values = snapshot.values && typeof snapshot.values === "object" ? snapshot.values : ({})
    const targetView = normalizedNavigationView(root, snapshot.view).length ? normalizedNavigationView(root, snapshot.view) : "overview"
    root.navigationRestoring = true
    try {
        root.statusText = String(values.statusText || qsTr("Ready"))
        root.resultTitle = String(values.resultTitle || qsTr("Output"))
        root.resultText = String(values.resultText || "")
        root.resultValue = cloneNavigationValue(root, values.resultValue)
        root.resultIsError = values.resultIsError === true
        root.resultOwner = String(values.resultOwner || "")
        root.blockDetailValue = cloneNavigationValue(root, values.blockDetailValue)
        root.blockDetailError = String(values.blockDetailError || "")
        root.transactionDetailValue = cloneNavigationValue(root, values.transactionDetailValue)
        root.transactionDetailError = String(values.transactionDetailError || "")
        root.accountDetailValue = cloneNavigationValue(root, values.accountDetailValue)
        root.transferRecipientDetailValue = cloneNavigationValue(root, values.transferRecipientDetailValue)
        root.channelDetailValue = cloneNavigationValue(root, values.channelDetailValue)
        root.channelDetailError = String(values.channelDetailError || "")
        root.sequencerTab = String(values.sequencerTab || root.sequencerTab)
        root.storageAppTab = String(values.storageAppTab || root.storageAppTab)
        root.deliveryAppTab = String(values.deliveryAppTab || root.deliveryAppTab)
        root.accountTab = String(values.accountTab || root.accountTab)
        root.programTab = String(values.programTab || root.programTab)
        root.indexerTab = String(values.indexerTab || root.indexerTab)
        root.localWalletTab = String(values.localWalletTab || root.localWalletTab)
        root.localWalletLookupTarget = String(values.localWalletLookupTarget || "")
        root.walletPublicKeyProbe = String(values.walletPublicKeyProbe || "")
        root.storageCidProbe = String(values.storageCidProbe || "")
        root.settingsSection = String(values.settingsSection || root.settingsSection)
        root.settingsNetworkSection = String(values.settingsNetworkSection || root.settingsNetworkSection)
        root.settingsUiSection = String(values.settingsUiSection || root.settingsUiSection)
        root.searchResolveSerial += 1
        root.transactionAutoDecodeSerial += 1
        root.programOpenSerial += 1
        expandNavGroupForView(root, targetView)
        root.currentView = targetView
    } finally {
        root.navigationRestoring = false
    }
}

function canNavigateBack(root) {
    const revision = root.navigationRevision
    return Array.isArray(root.navigationBackStack) && root.navigationBackStack.length > 0
}

function canNavigateForward(root) {
    const revision = root.navigationRevision
    return Array.isArray(root.navigationForwardStack) && root.navigationForwardStack.length > 0
}

function navigateBack(root) {
    const back = Array.isArray(root.navigationBackStack) ? root.navigationBackStack.slice(0) : []
    if (!back.length) {
        return
    }
    const target = back.pop()
    const current = navigationSnapshot(root)
    const forward = Array.isArray(root.navigationForwardStack) ? root.navigationForwardStack.slice(0) : []
    if (!navigationSnapshotsEqual(root, current, target)) {
        forward.push(current)
    }
    root.navigationBackStack = back
    root.navigationForwardStack = forward
    root.navigationRevision += 1
    restoreNavigationSnapshot(root, target)
}

function navigateForward(root) {
    const forward = Array.isArray(root.navigationForwardStack) ? root.navigationForwardStack.slice(0) : []
    if (!forward.length) {
        return
    }
    const target = forward.pop()
    const current = navigationSnapshot(root)
    const back = Array.isArray(root.navigationBackStack) ? root.navigationBackStack.slice(0) : []
    if (!navigationSnapshotsEqual(root, current, target)) {
        back.push(current)
    }
    while (back.length > root.navigationHistoryLimit) {
        back.shift()
    }
    root.navigationBackStack = back
    root.navigationForwardStack = forward
    root.navigationRevision += 1
    restoreNavigationSnapshot(root, target)
}

function navigationBackLabel(root) {
    const revision = root.navigationRevision
    const stack = Array.isArray(root.navigationBackStack) ? root.navigationBackStack : []
    return stack.length ? navigationSnapshotDisplayLabel(root, stack[stack.length - 1]) : ""
}

function navigationForwardLabel(root) {
    const revision = root.navigationRevision
    const stack = Array.isArray(root.navigationForwardStack) ? root.navigationForwardStack : []
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
    if (targetView === "blockDetail" || targetView === "l2BlockDetail") {
        return navigationLabelWithDetail(root, base, navigationObjectValue(values.blockDetailValue, ["hash", "block_id", "slot", "height"]))
    }
    if (targetView === "transactionDetail" || targetView === "l2TransactionDetail") {
        return navigationLabelWithDetail(root, base, navigationObjectValue(values.transactionDetailValue, ["hash", "transaction_hash", "tx_hash"]))
    }
    if (targetView === "accounts") {
        return navigationLabelWithDetail(root, base, navigationObjectValue(values.accountDetailValue, ["account_id_base58", "account_id", "account_id_hex"]))
    }
    if (targetView === "transferActivity") {
        return navigationLabelWithDetail(root, base, navigationObjectValue(values.transferRecipientDetailValue, ["address", "recipient", "account_ref"]))
    }
    if (targetView === "channels") {
        return navigationLabelWithDetail(root, base, navigationObjectValue(values.channelDetailValue, ["channel_id", "channel"]))
    }
    if (targetView === "programs") {
        return navigationLabelWithDetail(root, base, navigationObjectValue(values.resultValue, ["program_id_base58", "program_id", "program_id_hex", "input"]))
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
        if (recordHistory !== false && currentView !== targetView) {
            pushNavigationHistory()
        }
        expandNavGroupForView(targetView)
        currentView = targetView
        statusText = qsTr("Ready")
    }
}

function openSettings(root, section, subsection, recordHistory) {
    with (root) {
        const targetSection = String(section || "")
        const targetSubsection = String(subsection || "")
        const sectionChanged = targetSection.length > 0 && settingsSection !== targetSection
        const networkChanged = targetSection === "network" && targetSubsection.length > 0 && settingsNetworkSection !== targetSubsection
        const uiChanged = targetSection === "ui" && targetSubsection.length > 0 && settingsUiSection !== targetSubsection
        if (recordHistory !== false && (currentView !== "settings" || sectionChanged || networkChanged || uiChanged)) {
            pushNavigationHistory()
        }
        selectView("settings", false)
        if (targetSection.length) {
            settingsSection = targetSection
        }
        if (targetSection === "network" && targetSubsection.length) {
            settingsNetworkSection = targetSubsection
        }
        if (targetSection === "ui" && targetSubsection.length) {
            settingsUiSection = targetSubsection
        }
        statusText = qsTr("Ready")
    }
}

function clearResult(root) {
    with (root) {
        resultTitle = qsTr("Output")
        resultText = ""
        resultValue = null
        resultIsError = false
        resultOwner = ""
    }
}

function setResult(root, title, text, isError, value, owner) {
    with (root) {
        resultTitle = title
        resultText = text
        resultValue = value === undefined ? null : value
        resultIsError = isError
        resultOwner = owner === undefined ? currentView : String(owner || "")
        statusText = isError ? qsTr("Error") : qsTr("Ready")
    }
}

function pageHasOutput(root, view) {
    with (root) {
        return resultOwner === view && (resultText.length > 0 || resultValue !== null)
    }
}

function callInspector(root, method, args, label) {
    with (root) {
        return callModule(inspectorModule, method, args, label)
    }
}

function callModule(root, moduleName, method, args, label) {
    with (root) {
        return requestModule(moduleName, method, args, label, true)
    }
}

function requestModule(root, moduleName, method, args, label, showResult, cacheResult) {
    with (root) {
        if (busy) {
            return {
                ok: false,
                text: "",
                error: qsTr("Another inspection is already running.")
            }
        }

        const targetModule = moduleName === inspectorModule ? moduleName : inspectorModule
        const targetMethod = moduleName === inspectorModule ? method : "callModule"
        const targetArgs = moduleName === inspectorModule ? args : [moduleName, method, args || []]

        busy = true
        statusText = label
        const response = bridge.callModule(targetModule, targetMethod, targetArgs)
        busy = false

        if (response.ok) {
            if (cacheResult !== false) {
                updateDashboardCache(method, response.value)
            }
            if (method === "programs") {
                root.updateKnownProgramIds(response.value)
            }
            if (showResult) {
                setResult(label, response.text, false, response.value)
            }
        } else if (showResult) {
            if (method === "account") {
                accountDetailValue = null
            }
            setResult(label, response.error, true, null)
        }
        updateNetworkConnectionStatusForMethod(method, response)
        return response
    }
}

function requestModuleAsync(root, moduleName, method, args, label, showResult, callback, acceptResponse) {
    with (root) {
        const targetModule = moduleName === inspectorModule ? moduleName : inspectorModule
        const targetMethod = moduleName === inspectorModule ? method : "callModule"
        const targetArgs = moduleName === inspectorModule ? args : [moduleName, method, args || []]

        if (showResult) {
            statusText = label
        }

        return bridge.callModuleAsync(targetModule, targetMethod, targetArgs, function (response) {
            if (acceptResponse && !acceptResponse(response)) {
                return
            }
            if (response.ok) {
                root.updateDashboardCache(method, response.value)
                if (method === "programs") {
                    root.updateKnownProgramIds(response.value)
                }
                if (showResult) {
                    root.setResult(label, response.text, false, response.value)
                }
            } else if (showResult) {
                if (method === "account") {
                    accountDetailValue = null
                }
                root.setResult(label, response.error, true, null)
            }
            if (callback) {
                callback(response)
            }
        })
    }
}

function nodeOperationStart(root, request, showResult, callback) {
    with (root) {
        const operationRequest = request && typeof request === "object" ? request : ({})
        const label = String(operationRequest.label || operationRequest.method || qsTr("Node operation"))
        return requestModuleAsync(inspectorModule, "nodeOperationStart", [operationRequest], label, showResult === true, function (response) {
            if (response && response.ok) {
                coreUpdateNodeOperation(root, response.value)
            }
            if (callback) {
                callback(response)
            }
        })
    }
}

function nodeOperationStatus(root, operationId, showResult, callback) {
    with (root) {
        const id = String(operationId || "")
        if (!id.length) {
            return null
        }
        return requestModuleAsync(inspectorModule, "nodeOperationStatus", [id], qsTr("Node operation"), showResult === true, function (response) {
            if (response && response.ok) {
                coreUpdateNodeOperation(root, response.value)
            }
            if (callback) {
                callback(response)
            }
        })
    }
}

function nodeOperationEvents(root, operationId, afterSeq, showResult, callback) {
    with (root) {
        const id = String(operationId || "")
        if (!id.length) {
            return null
        }
        return requestModuleAsync(inspectorModule, "nodeOperationEvents", [id, Number(afterSeq || 0)], qsTr("Node operation events"), showResult === true, function (response) {
            if (response && response.ok && response.value) {
                coreUpdateNodeOperation(root, response.value.operation)
                const next = copyObject(nodeOperationEventSeq)
                next[id] = response.value.nextSeq || 0
                nodeOperationEventSeq = next
            }
            if (callback) {
                callback(response)
            }
        })
    }
}

function nodeOperationCancel(root, operationId, showResult, callback) {
    with (root) {
        const id = String(operationId || "")
        if (!id.length) {
            return null
        }
        return requestModuleAsync(inspectorModule, "nodeOperationCancel", [id], qsTr("Cancel operation"), showResult === true, function (response) {
            if (response && response.ok) {
                coreUpdateNodeOperation(root, response.value)
            }
            if (callback) {
                callback(response)
            }
        })
    }
}

function updateNodeOperation(root, operation) {
    coreUpdateNodeOperation(root, operation)
}

function coreUpdateNodeOperation(root, operation) {
    with (root) {
        const value = operation || null
        const operationId = String(value && value.operationId ? value.operationId : "")
        if (!operationId.length) {
            return
        }
        const next = copyObject(nodeOperations)
        next[operationId] = value
        nodeOperations = next
        nodeOperationsRevision += 1
    }
}

function nodeOperationTerminal(root, operation) {
    const status = String(operation && operation.status ? operation.status : "")
    return status === "completed" || status === "failed" || status === "canceled"
}

function nodeOperationResponse(root, operation) {
    const status = String(operation && operation.status ? operation.status : "")
    const ok = status === "completed"
    return {
        ok: ok,
        value: operation && operation.result !== undefined && operation.result !== null ? operation.result : operation,
        text: "",
        error: ok ? "" : String(operation && operation.error ? operation.error : "")
    }
}

function appendNodeOperationHistory(root, operation, detail) {
    with (root) {
        const value = operation || {}
        const rows = Array.isArray(nodeOperationHistory) ? nodeOperationHistory.slice(-99) : []
        rows.push({
            time: new Date().toLocaleTimeString(Qt.locale(), "hh:mm:ss"),
            label: String(value.label || value.method || qsTr("Node operation")),
            status: String(value.status || ""),
            detail: String(detail || nodeOperationDetail(root, value)),
            domain: String(value.domain || ""),
            method: String(value.method || ""),
            operationId: String(value.operationId || "")
        })
        nodeOperationHistory = rows
        nodeOperationsRevision += 1
    }
}

function nodeOperationHistoryRows(root, domain) {
    with (root) {
        const revision = nodeOperationsRevision
        const wanted = String(domain || "")
        const rows = Array.isArray(nodeOperationHistory) ? nodeOperationHistory.slice(0) : []
        const filtered = wanted.length ? rows.filter(row => String(row.domain || "") === wanted) : rows
        return filtered.reverse()
    }
}

function nodeOperationDetail(root, operation) {
    const value = operation || {}
    const result = value.result
    if (result && typeof result === "object") {
        if (result.cid) {
            return String(result.cid)
        }
        if (result.contentTopic) {
            return String(result.contentTopic)
        }
        if (result.status) {
            return String(result.status)
        }
    }
    if (value.error) {
        return String(value.error)
    }
    if (value.progress !== undefined && value.progress !== null) {
        return String(Math.floor(Number(value.progress || 0) * 100)) + "%"
    }
    return String(value.method || "")
}

function copyObject(value) {
    const next = ({})
    const source = value && typeof value === "object" && !Array.isArray(value) ? value : ({})
    const keys = Object.keys(source)
    for (let i = 0; i < keys.length; ++i) {
        next[keys[i]] = source[keys[i]]
    }
    return next
}

function decodeAccountData(root, dataHex, idlJson, accountType) {
    with (root) {
        if (busy) {
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
    with (root) {
        return requestModuleAsync(
            inspectorModule,
            "resolveAccountDecodeSession",
            [String(dataHex || ""), String(accountId || ""), Array.isArray(candidates) ? candidates : []],
            qsTr("Account decode"),
            false,
            callback
        )
    }
}

function resolveTransactionDecodeSessionAsync(root, summary, candidates, callback) {
    with (root) {
        return requestModuleAsync(
            inspectorModule,
            "resolveTransactionDecodeSession",
            [summary || {}, Array.isArray(candidates) ? candidates : []],
            qsTr("Transaction decode"),
            false,
            callback
        )
    }
}
