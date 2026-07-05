.import "../../services/BridgeHelpers.js" as BridgeHelpers

function refreshDashboard(root) {
    with (root) {
        if (dashboardRefreshing) {
            return
        }
        const refreshId = dashboardRefreshSerial + 1
        const configRevision = networkConfigurationRevision
        dashboardRefreshSerial = refreshId
        dashboardRefreshing = true
        dashboardError = ""
        const requests = [
            { module: inspectorModule, method: "overview", args: [sequencerUrl, indexerUrl, nodeUrl], label: qsTr("Dashboard overview") },
            { module: inspectorModule, method: "blockchainNode", args: root.blockchainArgs([]), label: qsTr("Blockchain node") },
            { module: inspectorModule, method: "blockchainLiveBlocks", args: root.blockchainArgs([0, 9007199254740991, 5]), label: qsTr("Latest L1 blocks") },
            { module: inspectorModule, method: "sequencerBlocks", args: root.executionArgs([null, 5]), label: qsTr("Latest L2 blocks") },
            { module: inspectorModule, method: "indexerBlocks", args: root.indexerArgs([null, 10]), label: qsTr("Latest blocks") },
            { module: inspectorModule, method: "storageSourceReport", args: root.storageSourceReportArgs(false), label: qsTr("Storage source") },
            { module: inspectorModule, method: "deliverySourceReport", args: root.deliverySourceReportArgs(), label: qsTr("Delivery source") }
        ]
        const errors = []
        let remaining = requests.length
        let okCount = 0

        for (let i = 0; i < requests.length; ++i) {
            const request = requests[i]
            requestModuleAsync(request.module, request.method, request.args, request.label, false, function (response) {
                if (refreshId !== dashboardRefreshSerial || configRevision !== networkConfigurationRevision) {
                    return
                }
                if (response.ok) {
                    okCount += 1
                } else {
                    errors.push(response.error)
                }
                if (request.method === "blockchainNode") {
                    root.updateNetworkConnectionStatus("blockchain", response)
                } else if (request.method === "storageReport" || request.method === "storageSourceReport") {
                    root.updateNetworkConnectionStatus("storage", response)
                } else if (request.method === "deliverySourceReport") {
                    root.updateNetworkConnectionStatus("messaging", response)
                }
                remaining -= 1
                if (remaining === 0) {
                    dashboardRefreshing = false
                    dashboardError = errors.join("\n")
                    root.recordDashboardSnapshot()
                    if (okCount > 0) {
                        setResult(qsTr("Dashboard"), BridgeHelpers.formatValue({
                            overview: dashboardOverview || null,
                            node: dashboardNode || null,
                            l1Blocks: dashboardL1Blocks || [],
                            blocks: dashboardBlocks || [],
                            storage: storageModuleReport || null,
                            messaging: messagingModuleReport || null
                        }), false)
                    } else {
                        setResult(qsTr("Dashboard"), dashboardError, true)
                    }
                }
            }, function () {
                return refreshId === dashboardRefreshSerial && configRevision === networkConfigurationRevision
            })
        }
    }
}

function updateDashboardCache(root, method, value) {
    with (root) {
        if (method === "overview") {
            dashboardOverview = value
        } else if (method === "blockchainNode") {
            dashboardNode = value
        } else if (method === "blockchainLiveBlocks") {
            dashboardL1Blocks = value && Array.isArray(value.blocks) ? value.blocks : []
        } else if (method === "indexerBlocks") {
            dashboardBlocks = value || []
        } else if (method === "sequencerBlocks") {
            dashboardSequencerBlocks = value || []
        } else if (method === "blockchainModuleReport") {
            blockchainModuleReport = value || null
        } else if (method === "account") {
            accountDetailValue = value || null
        } else if (method === "storageReport" || method === "storageSourceReport") {
            storageModuleReport = value || null
        } else if (method === "deliveryReport" || method === "deliverySourceReport") {
            messagingModuleReport = value || null
        }
    }
}

function routeSearch(root, query) {
    with (root) {
        const value = query.trim()
        if (!value.length) {
            return
        }

        if (routePrefixedSearch(value)) {
            return
        }

        const settingsTarget = settingsTargetForQuery(value)
        if (settingsTarget.section.length > 0) {
            openSettings(settingsTarget.section, settingsTarget.subsection)
            return
        }

        const view = viewKeyForQuery(value)
        if (view.length > 0) {
            selectView(view)
            return
        }

        if (/^[0-9]+$/.test(value)) {
            if (root.numericSearchUsesLezBlock()) {
                openLezBlock(value)
                return
            }
            const detail = blockchainBlockDetailById(value)
            if (detail) {
                openBlockchainBlock(value)
                return
            }
            openBlockchainBlock(value)
            return
        }

        if (/^(0x)?[0-9a-fA-F]{64}$/.test(value)) {
            const detail = blockchainBlockDetailById(value)
            if (detail) {
                openBlockchainBlock(value)
                return
            }
            if (root.programIdKnown(value)) {
                openProgram(value)
                return
            }
            resolveSearchHash(value)
            return
        }

        if (root.programIdKnown(value)) {
            openProgram(value)
            return
        }

        if (root.isStorageCid(value)) {
            openStorageCid(value)
            return
        }

        openAccount(value)
    }
}

function numericSearchUsesLezBlock(root) {
    with (root) {
        const view = String(currentView || "")
        if (root.layerForView(view) === "l2") {
            return true
        }
        return view === "l2Blocks" || view === "l2Transactions" || view === "l2BlockDetail"
            || view === "l2TransactionDetail" || view === "sequencer" || view === "accounts"
            || view === "programs" || view === "transferActivity" || view === "indexer"
    }
}

function routePrefixedSearch(root, query) {
    with (root) {
        const parsed = searchPrefix(query)
        if (!parsed.prefix.length) {
            return false
        }

        const prefix = parsed.prefix
        const target = parsed.target
        if ((prefix === "l1" || prefix === "slot" || prefix === "bedrock" || prefix === "cryptarchia") && target.length > 0) {
            openBlockchainBlock(target)
            return true
        }
        if (prefix === "mantle") {
            if (target.length > 0) {
                openMantleTransaction(target)
            } else {
                selectView("transactions")
            }
            return true
        }
        if (prefix === "channel") {
            if (target.length > 0) {
                openChannel(target)
            } else {
                selectView("channels")
            }
            return true
        }
        if (prefix === "l2" || prefix === "lez" || prefix === "block") {
            if (target.length > 0) {
                openLezSearchTarget(target)
            } else {
                selectView("l2Blocks")
            }
            return true
        }
        if (prefix === "tx" || prefix === "transaction") {
            if (target.length > 0) {
                openLezTransaction(target)
            } else {
                selectView("l2Transactions")
            }
            return true
        }
        if (prefix === "account") {
            if (target.length > 0) {
                openAccount(target)
            } else {
                selectView("accounts")
            }
            return true
        }
        if (prefix === "public") {
            if (target.length > 0) {
                openAccount(target.indexOf("Public/") === 0 ? target : "Public/" + target)
            } else {
                selectView("accounts")
            }
            return true
        }
        if (prefix === "private") {
            openPrivateAccountReference(target.length > 0 && target.indexOf("Private/") !== 0 ? "Private/" + target : target)
            return true
        }
        if (prefix === "recipient") {
            if (target.length > 0) {
                openRecipient(target)
            } else {
                selectView("transferActivity")
            }
            return true
        }
        if (prefix === "wallet") {
            openLocalWallet(target, "lezAccounts")
            return true
        }
        if (prefix === "cid" || prefix === "storage") {
            if (target.length > 0) {
                openStorageCid(target)
            } else {
                selectView("storage")
            }
            return true
        }
        if (prefix === "l1-wallet" || prefix === "note") {
            openLocalWallet(target, "bedrockNotes")
            return true
        }
        if (prefix === "program") {
            if (target.length > 0) {
                openProgram(target)
            } else {
                selectView("programs")
            }
            return true
        }
        if (prefix === "module") {
            routeModuleSearchTarget(target)
            return true
        }
        return false
    }
}

function searchPrefix(root, query) {
    with (root) {
        const text = String(query || "").trim()
        let match = text.match(/^([A-Za-z][A-Za-z0-9_-]*)\s*:\s*(.*)$/)
        if (match && isSearchPrefix(match[1])) {
            return { prefix: String(match[1]).toLowerCase(), target: String(match[2] || "").trim() }
        }
        match = text.match(/^([A-Za-z][A-Za-z0-9_-]*)\s+(.+)$/)
        if (match && isSearchPrefix(match[1])) {
            return { prefix: String(match[1]).toLowerCase(), target: String(match[2] || "").trim() }
        }
        return { prefix: "", target: "" }
    }
}

function isSearchPrefix(root, prefix) {
    with (root) {
        const value = String(prefix || "").toLowerCase()
        return value === "l1" || value === "slot" || value === "bedrock" || value === "cryptarchia"
            || value === "mantle" || value === "channel" || value === "l2" || value === "lez"
            || value === "block" || value === "tx" || value === "transaction" || value === "account"
            || value === "public" || value === "private" || value === "program" || value === "wallet"
            || value === "l1-wallet" || value === "note" || value === "recipient" || value === "module"
            || value === "cid" || value === "storage"
    }
}

function openStorageCid(root, cid) {
    with (root) {
        const value = String(cid || "").trim()
        if (!value.length) {
            selectView("storage")
            return
        }
        pushNavigationHistory()
        storageCidProbe = value
        storageAppTab = "cid"
        selectView("storage", false)
        setResult(qsTr("Storage CID"), qsTr("Storage CID context: %1").arg(value), false, {
            cid: value,
            source: root.storageSourceTarget()
        })
        if (root.storageSourceTarget().length > 0) {
            root.queryNetworkConnection("storage", false, true)
        }
    }
}

function isStorageCid(root, value) {
    with (root) {
        const text = String(value || "").trim()
        if (text.length < 20 || /\s/.test(text)) {
            return false
        }
        if (/^Qm[1-9A-HJ-NP-Za-km-z]{44}$/.test(text)) {
            return true
        }
        if (/^b[a-z2-7]{20,}$/i.test(text)) {
            return true
        }
        return /^z[1-9A-HJ-NP-Za-km-z]{20,}$/.test(text)
    }
}

function routeModuleSearchTarget(root, target) {
    with (root) {
        const value = String(target || "").trim().toLowerCase()
        if (value === "storage") {
            selectView("storage")
        } else if (value === "messaging" || value === "delivery") {
            selectView("messaging")
        } else if (value === "capability" || value === "capabilities") {
            selectView("capabilities")
        } else if (value === "blockchain" || value === "bedrock" || value === "node") {
            selectView("blockchain")
        } else {
            selectView("storage")
        }
    }
}

function resolveSearchHash(root, hash) {
    with (root) {
        const value = String(hash || "").trim()
        if (!value.length) {
            return
        }

        pushNavigationHistory()
        const serial = searchResolveSerial + 1
        searchResolveSerial = serial
        statusText = qsTr("Search")
        requestModuleAsync(inspectorModule, "indexerBlockByHash", root.indexerArgs([value]), qsTr("Block lookup"), false, function (response) {
            if (serial !== searchResolveSerial) {
                return
            }
            if (response.ok && response.value !== null && response.value !== undefined) {
                selectView("l2BlockDetail", false)
                blockDetailValue = root.indexerBlockDetail(response.value)
                setResult(qsTr("LEZ block"), BridgeHelpers.formatValue(blockDetailValue), false, blockDetailValue)
                return
            }
            root.resolveSearchTransaction(serial, value, false)
        })
    }
}

function resolveSearchTransaction(root, serial, hash, recordHistory) {
    with (root) {
        if (recordHistory !== false) {
            pushNavigationHistory()
        }
        requestModuleAsync(inspectorModule, "inspectTransaction", root.executionArgs([hash]), qsTr("Transaction inspection"), false, function (response) {
            if (serial !== searchResolveSerial) {
                return
            }
            if (response.ok && response.value !== null && response.value !== undefined) {
                selectView("l2TransactionDetail", false)
                transactionDetailValue = response.value
                lezTransactionsPageError = ""
                setResult(qsTr("LEZ transaction"), response.text, false, response.value)
                root.autoDecodeTransactionDetail(response.value)
                return
            }
            root.resolveSearchAccount(serial, hash, false)
        })
    }
}

function resolveSearchAccount(root, serial, account, recordHistory) {
    with (root) {
        if (recordHistory !== false) {
            pushNavigationHistory()
        }
        requestModuleAsync(inspectorModule, "account", root.accountLookupArgs(account), qsTr("Account lookup"), false, function (response) {
            if (serial !== searchResolveSerial) {
                return
            }
            selectView("accounts", false)
            accountTab = "lookup"
            if (response.ok) {
                accountDetailValue = response.value || null
                setResult(qsTr("Account lookup"), response.text, false, response.value)
            } else {
                accountDetailValue = null
                setResult(qsTr("Search"), response.error || qsTr("No block, transaction, or account found."), true, null)
            }
        })
    }
}

function viewKeyForQuery(root, query) {
    with (root) {
        const normalized = String(query || "").trim().toLowerCase()
        if (!normalized.length) {
            return ""
        }
        const item = navItemForQuery(normalized)
        if (item && String(item.view || "").length > 0) {
            return item.view
        }
        if (normalized === "home" || normalized === "dashboard" || normalized === "overview") {
            return "overview"
        }
        if (normalized === "l1" || normalized === "l1 bedrock" || normalized === "bedrock" || normalized === "cryptarchia" || normalized === "block" || normalized === "latest blocks") {
            return "blocks"
        }
        if (normalized === "transaction" || normalized === "tx" || normalized === "txs" || normalized === "latest transactions") {
            return "transactions"
        }
        if (normalized === "l2 transaction" || normalized === "l2 transactions" || normalized === "lez transaction" || normalized === "lez transactions") {
            return "l2Transactions"
        }
        if (normalized === "wallet" || normalized === "local wallet" || normalized === "wallets") {
            return "localWallet"
        }
        if (normalized === "recipient" || normalized === "recipients" || normalized === "transfer" || normalized === "transfers" || normalized === "transfer activity") {
            return "transferActivity"
        }
        if (normalized === "channel") {
            return "channels"
        }
        if (normalized === "account" || normalized === "public account") {
            return "accounts"
        }
        if (normalized === "spel" || normalized === "program" || normalized === "programs") {
            return "programs"
        }
        if (normalized === "l2" || normalized === "lez" || normalized === "sequencer" || normalized === "l2 blocks" || normalized === "lez blocks") {
            return "l2Blocks"
        }
        if (normalized === "indexer" || normalized === "lez indexer" || normalized === "indexer diagnostics") {
            return "indexer"
        }
        if (normalized === "chain" || normalized === "base chain" || normalized === "node" || normalized === "consensus" || normalized === "bedrock node" || normalized === "node diagnostics") {
            return "blockchain"
        }
        if (normalized === "storage diagnostics") {
            return "diagnosticsStorage"
        }
        if (normalized === "delivery diagnostics" || normalized === "messaging diagnostics") {
            return "diagnosticsDelivery"
        }
        if (normalized === "messages" || normalized === "messaging" || normalized === "delivery") {
            return "messaging"
        }
        if (normalized === "capability") {
            return "capabilities"
        }
        if (normalized === "config" || normalized === "profile") {
            return "settings"
        }
        return ""
    }
}

function settingsTargetForQuery(root, query) {
    with (root) {
        const normalized = String(query || "").trim().toLowerCase()
        if (!normalized.length) {
            return { section: "", subsection: "" }
        }
        if (normalized === "network") {
            return { section: "network", subsection: settingsNetworkSection }
        }
        if (normalized === "wallet settings" || normalized === "local wallet settings" || normalized === "wallet profile") {
            return { section: "wallet", subsection: "" }
        }
        if (normalized === "blockchain rpc" || normalized === "node rpc" || normalized === "chain rpc" || normalized === "base chain rpc") {
            return { section: "network", subsection: "blockchain" }
        }
        if (normalized === "indexer rpc") {
            return { section: "network", subsection: "indexer" }
        }
        if (normalized === "execution" || normalized === "execution zone" || normalized === "lez rpc" || normalized === "sequencer node" || normalized === "sequencer rpc") {
            return { section: "network", subsection: "execution" }
        }
        if (normalized === "messaging rpc" || normalized === "delivery rpc" || normalized === "delivery settings") {
            return { section: "network", subsection: "messaging" }
        }
        if (normalized === "storage rpc" || normalized === "storage network") {
            return { section: "network", subsection: "storage" }
        }
        if (normalized === "footer") {
            return { section: "ui", subsection: "footer" }
        }
        if (normalized === "dashboard settings") {
            return { section: "ui", subsection: "dashboard" }
        }
        if (normalized === "config" || normalized === "profile" || normalized === "settings") {
            return { section: "general", subsection: "" }
        }
        return { section: "", subsection: "" }
    }
}
