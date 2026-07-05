.import "../../services/BridgeHelpers.js" as BridgeHelpers

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
    with (root) {
        return [
            { type: "item", key: "overview", view: "overview", label: qsTr("Dashboard"), token: "DAS", layer: "system" },
            {
                type: "group",
                key: "l1",
                label: qsTr("L1 Bedrock"),
                token: "L1",
                layer: "l1",
                children: [
                    { key: "blocks", view: "blocks", label: qsTr("Blocks"), token: "L1B", layer: "l1" },
                    { key: "transactions", view: "transactions", label: qsTr("Mantle Tx"), token: "L1T", layer: "l1" },
                    { key: "channels", view: "channels", label: qsTr("Channels"), token: "L1C", layer: "l1" },
                    { key: "blockchain", view: "blockchain", label: qsTr("Node / Module"), token: "L1N", layer: "l1" }
                ]
            },
            {
                type: "group",
                key: "l2",
                label: qsTr("L2 LEZ"),
                token: "L2",
                layer: "l2",
                children: [
                    { key: "l2Blocks", view: "l2Blocks", label: qsTr("Blocks"), token: "L2B", layer: "l2" },
                    { key: "l2Transactions", view: "l2Transactions", label: qsTr("Transactions"), token: "L2T", layer: "l2" },
                    { key: "accounts", view: "accounts", label: qsTr("Accounts"), token: "ACC", layer: "l2" },
                    { key: "transferActivity", view: "transferActivity", label: qsTr("Transfer Activity"), token: "XFR", layer: "l2" },
                    { key: "programs", view: "programs", label: qsTr("Programs"), token: "PRG", layer: "l2" }
                ]
            },
            {
                type: "group",
                key: "network",
                label: qsTr("Network"),
                token: "NET",
                layer: "module",
                children: [
                    { key: "storage", view: "storage", label: qsTr("Storage"), token: "STO", layer: "module" },
                    { key: "messaging", view: "messaging", label: qsTr("Delivery"), token: "DLV", layer: "module" }
                ]
            },
            {
                type: "group",
                key: "diagnostics",
                label: qsTr("Diagnostics"),
                token: "DIA",
                layer: "system",
                children: [
                    { key: "indexer", view: "indexer", label: qsTr("LEZ Indexer"), token: "IDX", layer: "system" },
                    { key: "storageDiagnostics", view: "diagnosticsStorage", label: qsTr("Storage"), token: "DST", layer: "system" },
                    { key: "deliveryDiagnostics", view: "diagnosticsDelivery", label: qsTr("Delivery"), token: "DDL", layer: "system" },
                    { key: "capabilities", view: "capabilities", label: qsTr("Capabilities"), token: "CAP", layer: "system" }
                ]
            },
            {
                type: "group",
                key: "local",
                label: qsTr("Local"),
                token: "LOC",
                layer: "local",
                children: [
                    { key: "localWallet", view: "localWallet", label: qsTr("Wallet"), token: "WAL", layer: "local" }
                ]
            },
            {
                type: "group",
                key: "system",
                label: qsTr("System"),
                token: "SYS",
                layer: "system",
                children: [
                    { key: "settings", view: "settings", label: qsTr("Settings"), token: "SET", layer: "system" }
                ]
            }
        ]
    }
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
    with (root) {
        const target = String(view || "")
        if (target === "blockDetail" || target === "transactionDetail") {
            return "l1"
        }
        if (target === "l2BlockDetail" || target === "l2TransactionDetail" || target === "sequencer") {
            return "l2"
        }
        const tree = navTreeItems()
        for (let i = 0; i < tree.length; ++i) {
            const item = tree[i]
            const children = item.children || []
            for (let j = 0; j < children.length; ++j) {
                if (String(children[j].view || "") === target) {
                    return item.key
                }
            }
        }
        return ""
    }
}

function navItemForView(root, view) {
    with (root) {
        const target = String(view || "")
        const tree = navTreeItems()
        for (let i = 0; i < tree.length; ++i) {
            const item = tree[i]
            if (String(item.view || "") === target) {
                return item
            }
            const children = item.children || []
            for (let j = 0; j < children.length; ++j) {
                if (String(children[j].view || "") === target) {
                    return children[j]
                }
            }
        }
        if (target === "blockDetail") {
            return { key: "blockDetail", view: "blockDetail", label: qsTr("Block"), token: "L1B", layer: "l1" }
        }
        if (target === "transactionDetail") {
            return { key: "transactionDetail", view: "transactionDetail", label: qsTr("Mantle Tx"), token: "L1T", layer: "l1" }
        }
        if (target === "l2BlockDetail") {
            return { key: "l2BlockDetail", view: "l2BlockDetail", label: qsTr("LEZ Block"), token: "L2B", layer: "l2" }
        }
        if (target === "l2TransactionDetail") {
            return { key: "l2TransactionDetail", view: "l2TransactionDetail", label: qsTr("LEZ Transaction"), token: "L2T", layer: "l2" }
        }
        return null
    }
}

function layerForView(root, view) {
    with (root) {
        const item = navItemForView(view)
        return item ? String(item.layer || "") : ""
    }
}

function navLabelForView(root, view) {
    with (root) {
        const item = navItemForView(view)
        return item ? String(item.label || "") : ""
    }
}

function navTokenForView(root, view) {
    with (root) {
        const item = navItemForView(view)
        return item ? String(item.token || "") : ""
    }
}

function navItemForQuery(root, query) {
    with (root) {
        const normalized = String(query || "").trim().toLowerCase()
        const tree = navTreeItems()
        for (let i = 0; i < tree.length; ++i) {
            const item = tree[i]
            if (navItemMatches(item, normalized)) {
                return item
            }
            const children = item.children || []
            for (let j = 0; j < children.length; ++j) {
                if (navItemMatches(children[j], normalized)) {
                    return children[j]
                }
            }
        }
        return null
    }
}

function navItemMatches(root, item, normalized) {
    with (root) {
        const key = String(item.key || "").toLowerCase()
        const view = String(item.view || "").toLowerCase()
        const label = String(item.label || "").toLowerCase()
        return normalized === key || normalized === view || normalized === label
    }
}

function viewTitle(root) {
    with (root) {
        const item = navItemForView(currentView)
        if (item) {
            return item.label
        }
        return qsTr("Dashboard")
    }
}

function selectView(root, view) {
    with (root) {
        const requested = String(view || "")
        const target = requested === "sequencer" ? "l2Blocks" : requested
        if (!target.length) {
            return
        }
        expandNavGroupForView(target)
        currentView = target
        statusText = qsTr("Ready")
    }
}

function openSettings(root, section, subsection) {
    with (root) {
        currentView = "settings"
        expandNavGroupForView(currentView)
        const targetSection = String(section || "")
        const targetSubsection = String(subsection || "")
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
