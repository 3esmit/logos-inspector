import QtQuick
import QtQml.Models
import "../services/BridgeHelpers.js" as BridgeHelpers
import "../services"

QtObject {
    id: root

    required property BridgeClient bridge

    readonly property string inspectorModule: "logos_inspector"
    readonly property string blockchainModule: "blockchain_module"
    readonly property string storageModule: "storage_module"
    readonly property string deliveryModule: "delivery_module"
    readonly property string capabilityModule: "capability_module"

    property string currentView: "overview"
    property string statusText: qsTr("Ready")
    property bool busy: false
    property string resultTitle: qsTr("Output")
    property string resultText: ""
    property var resultValue: null
    property bool resultIsError: false
    property string resultOwner: ""
    property var dashboardOverview: null
    property var dashboardNode: null
    property var dashboardBlocks: []
    property string dashboardError: ""
    property var blockDetailValue: null
    property var transactionDetailValue: null
    property var accountDetailValue: null
    property var transferRecipientDetailValue: null
    property var channelDetailValue: null
    property var blocksPageRows: []
    property int blocksPageSlotFrom: 0
    property int blocksPageSlotTo: 0
    property int blocksPageWindow: 2000
    property int blocksPageLimit: 20
    property string blocksPageError: ""
    property var transactionsPageRows: []
    property int transactionsPageBeforeBlock: 0
    property int transactionsPageNextBeforeBlock: 0
    property int transactionsPageBlockBatch: 1000
    property int transactionsPageLimit: 20
    property string transactionsPageError: ""
    property var transferActivityRows: []
    property int transferActivityBeforeBlock: 0
    property int transferActivityNextBeforeBlock: 0
    property int transferActivityBlockBatch: 50
    property int transferActivityLimit: 20
    property var transferActivityHistory: []
    property string transferActivityError: ""
    property var channelsPageRows: []
    property int channelsPageSlotFrom: 0
    property int channelsPageSlotTo: 0
    property int channelsPageWindow: 4000
    property int channelsPageLimit: 20
    property string channelsPageError: ""

    property string networkProfile: "default"
    property string sequencerUrl: "https://testnet.lez.logos.co/"
    property string indexerUrl: "http://127.0.0.1:8779/"
    property string nodeUrl: "http://127.0.0.1:8080/"
    property string messagingNodeInfoId: ""
    property string messagingSourceMode: "module"
    property string messagingRestUrl: "http://127.0.0.1:8645"
    property string messagingMetricsUrl: "http://127.0.0.1:8008/metrics"
    property string messagingNetworkPreset: "logos.test"
    property int messagingRollingWindow: 120
    property bool messagingAdminRestEnabled: false
    property bool messagingMutatingDiagnosticsEnabled: false
    property string storageSourceMode: "module"
    property string storageRestUrl: "http://127.0.0.1:8080/api/storage/v1"
    property string storageMetricsUrl: "http://127.0.0.1:8008/metrics"
    property string storageNetworkPreset: "logos.test"
    property string storageDataDir: ""
    property int storageRollingWindow: 120
    property bool storageLocalDiagnosticsEnabled: false
    property bool storagePrivilegedDebugEnabled: false
    property bool storageMutatingDiagnosticsEnabled: false
    property string storageCidProbe: ""

    property string sequencerTab: "blocks"
    property string accountTab: "lookup"
    property string programTab: "programIds"
    property string indexerTab: "status"
    property string localWalletTab: "profiles"
    property string localWalletLookupTarget: ""
    property string settingsSection: "general"
    property string settingsNetworkSection: "blockchain"
    property string settingsUiSection: "footer"
    property int blockchainRefreshRate: 30
    property int indexerRefreshRate: 30
    property int executionRefreshRate: 30
    property int messagingRefreshRate: 30
    property int storageRefreshRate: 30
    property var networkConnectionStatus: ({})
    property int networkConnectionStatusRevision: 0
    property var footerFieldSelections: defaultFooterFieldSelections()
    property int footerFieldRevision: 0
    property var dashboardGraphSelections: defaultDashboardGraphSelections()
    property int dashboardGraphRevision: 0
    property var dashboardMetricHistory: ({})
    property int dashboardMetricHistoryRevision: 0
    property var networkConnectionPending: ({})
    property int networkConnectionPendingRevision: 0
    property bool dashboardRefreshing: false
    property int dashboardRefreshSerial: 0
    property var storageModuleReport: null
    property var messagingModuleReport: null

    property ListModel registeredIdls: ListModel {}
    property bool idlStateLoaded: false
    property bool walletStateLoaded: false
    property bool settingsStateLoaded: false
    property string walletProfileLabel: "Local wallet"
    property string walletBinary: ""
    property string walletHome: ""
    property string walletSequencerUrl: ""
    property string walletIndexerUrl: ""
    property string walletBedrockNodeUrl: ""
    property string walletPublicKeyProbe: ""
    property string bedrockWalletBalanceTip: ""
    property var localWalletStatus: null
    property string localWalletStatusError: ""
    property var localWalletOperations: []
    property var bedrockWalletBalanceValue: null
    property string bedrockWalletBalanceError: ""
    property var accountIdlSelections: ({})
    property int accountIdlSelectionRevision: 0
    property int accountAutoDecodeSerial: 0
    property int transactionAutoDecodeSerial: 0
    property int searchResolveSerial: 0
    property var navExpanded: ({ l1: true, l2: true, network: false, local: true, system: true })
    property int navRevision: 0

    onCurrentViewChanged: expandNavGroupForView(currentView)
    onNetworkProfileChanged: saveSettingsState()
    onSequencerUrlChanged: saveSettingsState()
    onIndexerUrlChanged: saveSettingsState()
    onNodeUrlChanged: saveSettingsState()
    onMessagingNodeInfoIdChanged: saveSettingsState()
    onMessagingSourceModeChanged: saveSettingsState()
    onMessagingRestUrlChanged: saveSettingsState()
    onMessagingMetricsUrlChanged: saveSettingsState()
    onMessagingNetworkPresetChanged: saveSettingsState()
    onMessagingRollingWindowChanged: saveSettingsState()
    onMessagingAdminRestEnabledChanged: saveSettingsState()
    onMessagingMutatingDiagnosticsEnabledChanged: saveSettingsState()
    onStorageSourceModeChanged: saveSettingsState()
    onStorageRestUrlChanged: saveSettingsState()
    onStorageMetricsUrlChanged: saveSettingsState()
    onStorageNetworkPresetChanged: saveSettingsState()
    onStorageDataDirChanged: saveSettingsState()
    onStorageRollingWindowChanged: saveSettingsState()
    onStorageLocalDiagnosticsEnabledChanged: saveSettingsState()
    onStoragePrivilegedDebugEnabledChanged: saveSettingsState()
    onStorageMutatingDiagnosticsEnabledChanged: saveSettingsState()
    onStorageCidProbeChanged: saveSettingsState()
    onBlockchainRefreshRateChanged: saveSettingsState()
    onIndexerRefreshRateChanged: saveSettingsState()
    onExecutionRefreshRateChanged: saveSettingsState()
    onMessagingRefreshRateChanged: saveSettingsState()
    onStorageRefreshRateChanged: saveSettingsState()
    onFooterFieldRevisionChanged: saveSettingsState()
    onDashboardGraphRevisionChanged: saveSettingsState()

    function navTreeItems() {
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
                    { key: "blockchain", view: "blockchain", label: qsTr("Node"), token: "L1N", layer: "l1" }
                ]
            },
            {
                type: "group",
                key: "l2",
                label: qsTr("L2 LEZ"),
                token: "L2",
                layer: "l2",
                children: [
                    { key: "sequencer", view: "sequencer", label: qsTr("Blocks / Tx"), token: "L2X", layer: "l2" },
                    { key: "accounts", view: "accounts", label: qsTr("Accounts"), token: "ACC", layer: "l2" },
                    { key: "transferActivity", view: "transferActivity", label: qsTr("Transfer Activity"), token: "XFR", layer: "l2" },
                    { key: "programs", view: "programs", label: qsTr("Programs"), token: "PRG", layer: "l2" },
                    { key: "indexer", view: "indexer", label: qsTr("Indexer"), token: "IDX", layer: "l2" }
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
                    { key: "messaging", view: "messaging", label: qsTr("Delivery"), token: "DLV", layer: "module" },
                    { key: "capabilities", view: "capabilities", label: qsTr("Capabilities"), token: "CAP", layer: "module" }
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

    function navRows() {
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

    function navGroupExpanded(key) {
        const revision = navRevision
        return navExpanded[String(key || "")] === true
    }

    function toggleNavGroup(key) {
        const groupKey = String(key || "")
        if (!groupKey.length) {
            return
        }
        const next = copyMap(navExpanded)
        next[groupKey] = next[groupKey] !== true
        navExpanded = next
        navRevision += 1
    }

    function expandNavGroupForView(view) {
        const parentKey = parentNavKeyForView(view)
        if (!parentKey || navExpanded[parentKey] === true) {
            return
        }
        const next = copyMap(navExpanded)
        next[parentKey] = true
        navExpanded = next
        navRevision += 1
    }

    function parentNavKeyForView(view) {
        const target = String(view || "")
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

    function navItemForView(view) {
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
        return null
    }

    function layerForView(view) {
        const item = navItemForView(view)
        return item ? String(item.layer || "") : ""
    }

    function navLabelForView(view) {
        const item = navItemForView(view)
        return item ? String(item.label || "") : ""
    }

    function navTokenForView(view) {
        const item = navItemForView(view)
        return item ? String(item.token || "") : ""
    }

    function navItemForQuery(query) {
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

    function navItemMatches(item, normalized) {
        const key = String(item.key || "").toLowerCase()
        const view = String(item.view || "").toLowerCase()
        const label = String(item.label || "").toLowerCase()
        return normalized === key || normalized === view || normalized === label
    }

    function viewTitle() {
        const item = navItemForView(currentView)
        if (item) {
            return item.label
        }
        return qsTr("Dashboard")
    }

    function selectView(view) {
        const target = String(view || "")
        if (!target.length) {
            return
        }
        expandNavGroupForView(target)
        currentView = target
        statusText = qsTr("Ready")
    }

    function openSettings(section, subsection) {
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

    function clearResult() {
        resultTitle = qsTr("Output")
        resultText = ""
        resultValue = null
        resultIsError = false
        resultOwner = ""
    }

    function setResult(title, text, isError, value) {
        resultTitle = title
        resultText = text
        resultValue = value === undefined ? null : value
        resultIsError = isError
        resultOwner = currentView
        statusText = isError ? qsTr("Error") : qsTr("Ready")
    }

    function pageHasOutput(view) {
        return resultOwner === view && (resultText.length > 0 || resultValue !== null)
    }

    function callInspector(method, args, label) {
        return callModule(inspectorModule, method, args, label)
    }

    function callModule(moduleName, method, args, label) {
        return requestModule(moduleName, method, args, label, true)
    }

    function requestModule(moduleName, method, args, label, showResult) {
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
            updateDashboardCache(method, response.value)
            if (showResult) {
                setResult(label, response.text, false, response.value)
            }
        } else if (showResult) {
            if (method === "account" || method === "decodeAccount") {
                accountDetailValue = null
            }
            setResult(label, response.error, true, null)
        }
        updateNetworkConnectionStatusForMethod(method, response)
        return response
    }

    function requestModuleAsync(moduleName, method, args, label, showResult, callback) {
        const targetModule = moduleName === inspectorModule ? moduleName : inspectorModule
        const targetMethod = moduleName === inspectorModule ? method : "callModule"
        const targetArgs = moduleName === inspectorModule ? args : [moduleName, method, args || []]

        if (showResult) {
            statusText = label
        }

        return bridge.callModuleAsync(targetModule, targetMethod, targetArgs, function (response) {
            if (response.ok) {
                root.updateDashboardCache(method, response.value)
                if (showResult) {
                    root.setResult(label, response.text, false, response.value)
                }
            } else if (showResult) {
                if (method === "account" || method === "decodeAccount") {
                    accountDetailValue = null
                }
                root.setResult(label, response.error, true, null)
            }
            if (callback) {
                callback(response)
            }
        })
    }

    function decodeAccountData(dataHex, idlJson, accountType) {
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

    function decodeAccountDataAsync(dataHex, idlJson, accountType, callback) {
        const args = [String(dataHex || ""), String(idlJson || ""), String(accountType || "")]
        return requestModuleAsync(inspectorModule, "decodeAccount", args, qsTr("Account decode"), false, callback)
    }

    function decodeTransactionSummaryAsync(summary, idlJson, callback) {
        return requestModuleAsync(inspectorModule, "decodeTransactionSummary", [summary || {}, String(idlJson || "")], qsTr("Transaction decode"), false, callback)
    }

    function loadIdlState() {
        const response = bridge.callModule(inspectorModule, "loadIdlState", [])
        idlStateLoaded = true
        if (!response.ok || !response.value || typeof response.value !== "object") {
            return
        }

        registeredIdls.clear()
        const idls = Array.isArray(response.value.idls) ? response.value.idls : []
        for (let i = 0; i < idls.length; ++i) {
            const entry = root.normalizedIdlEntry(idls[i], registeredIdls.count)
            if (entry.json.length) {
                registeredIdls.append(entry)
            }
        }

        accountIdlSelections = response.value.account_idl_selections && typeof response.value.account_idl_selections === "object"
            ? response.value.account_idl_selections
            : ({})
        accountIdlSelectionRevision += 1
    }

    function saveIdlState() {
        if (!idlStateLoaded) {
            return
        }
        bridge.callModule(inspectorModule, "saveIdlState", [idlStatePayload()])
    }

    function idlStatePayload() {
        return {
            version: 1,
            idls: registeredIdlEntries(),
            account_idl_selections: accountIdlSelections || {}
        }
    }

    function loadSettingsState() {
        const response = bridge.callModule(inspectorModule, "loadSettingsState", [])
        if (response.ok && response.value && typeof response.value === "object") {
            const value = response.value
            const storedNetworkProfile = root.normalizedNetworkProfile(root.stringSetting(value, "network_profile", networkProfile))
            sequencerUrl = root.stringSetting(value, "sequencer_url", sequencerUrl)
            indexerUrl = root.stringSetting(value, "indexer_url", indexerUrl)
            nodeUrl = root.stringSetting(value, "node_url", nodeUrl)
            networkProfile = root.resolvedNetworkProfile(storedNetworkProfile, sequencerUrl, indexerUrl, nodeUrl)
            messagingSourceMode = root.stringSetting(value, "messaging_source_mode", messagingSourceMode)
            messagingRestUrl = root.stringSetting(value, "messaging_rest_url", messagingRestUrl)
            messagingMetricsUrl = root.stringSetting(value, "messaging_metrics_url", messagingMetricsUrl)
            messagingNetworkPreset = root.normalizedMessagingNetworkPreset(root.stringSetting(value, "messaging_network_preset", messagingNetworkPreset))
            messagingNodeInfoId = root.stringSetting(value, "messaging_node_info_id", messagingNodeInfoId)
            messagingRollingWindow = root.numberSetting(value, "messaging_rolling_window", messagingRollingWindow)
            messagingAdminRestEnabled = root.boolSetting(value, "messaging_admin_rest_enabled", messagingAdminRestEnabled)
            messagingMutatingDiagnosticsEnabled = root.boolSetting(value, "messaging_mutating_diagnostics_enabled", messagingMutatingDiagnosticsEnabled)
            storageSourceMode = root.stringSetting(value, "storage_source_mode", storageSourceMode)
            storageRestUrl = root.stringSetting(value, "storage_rest_url", storageRestUrl)
            storageMetricsUrl = root.stringSetting(value, "storage_metrics_url", storageMetricsUrl)
            storageNetworkPreset = root.stringSetting(value, "storage_network_preset", storageNetworkPreset)
            storageDataDir = root.stringSetting(value, "storage_data_dir", storageDataDir)
            storageCidProbe = root.stringSetting(value, "storage_cid_probe", storageCidProbe)
            storageRollingWindow = root.numberSetting(value, "storage_rolling_window", storageRollingWindow)
            storageLocalDiagnosticsEnabled = root.boolSetting(value, "storage_local_diagnostics_enabled", storageLocalDiagnosticsEnabled)
            storagePrivilegedDebugEnabled = root.boolSetting(value, "storage_privileged_debug_enabled", storagePrivilegedDebugEnabled)
            storageMutatingDiagnosticsEnabled = root.boolSetting(value, "storage_mutating_diagnostics_enabled", storageMutatingDiagnosticsEnabled)
            blockchainRefreshRate = root.numberSetting(value, "blockchain_refresh_rate", blockchainRefreshRate)
            indexerRefreshRate = root.numberSetting(value, "indexer_refresh_rate", indexerRefreshRate)
            executionRefreshRate = root.numberSetting(value, "execution_refresh_rate", executionRefreshRate)
            messagingRefreshRate = root.numberSetting(value, "messaging_refresh_rate", messagingRefreshRate)
            storageRefreshRate = root.numberSetting(value, "storage_refresh_rate", storageRefreshRate)
            if (value.footer_fields && typeof value.footer_fields === "object" && !Array.isArray(value.footer_fields)) {
                footerFieldSelections = root.mergeMap(root.defaultFooterFieldSelections(), value.footer_fields)
                footerFieldRevision += 1
            }
            if (value.dashboard_graphs && typeof value.dashboard_graphs === "object" && !Array.isArray(value.dashboard_graphs)) {
                dashboardGraphSelections = root.mergeMap(root.defaultDashboardGraphSelections(), value.dashboard_graphs)
                dashboardGraphRevision += 1
            }
        }
        settingsStateLoaded = true
    }

    function saveSettingsState() {
        if (!settingsStateLoaded) {
            return
        }
        bridge.callModule(inspectorModule, "saveSettingsState", [settingsStatePayload()])
    }

    function settingsStatePayload() {
        const resolvedProfile = root.inferNetworkProfileFromEndpoints(sequencerUrl, indexerUrl, nodeUrl)
        return {
            version: 1,
            network_profile: resolvedProfile,
            sequencer_url: String(sequencerUrl || ""),
            indexer_url: String(indexerUrl || ""),
            node_url: String(nodeUrl || ""),
            messaging_source_mode: String(messagingSourceMode || ""),
            messaging_rest_url: String(messagingRestUrl || ""),
            messaging_metrics_url: String(messagingMetricsUrl || ""),
            messaging_network_preset: root.normalizedMessagingNetworkPreset(messagingNetworkPreset),
            messaging_node_info_id: String(messagingNodeInfoId || ""),
            messaging_rolling_window: Number(messagingRollingWindow || 0),
            messaging_admin_rest_enabled: messagingAdminRestEnabled === true,
            messaging_mutating_diagnostics_enabled: messagingMutatingDiagnosticsEnabled === true,
            storage_source_mode: String(storageSourceMode || ""),
            storage_rest_url: String(storageRestUrl || ""),
            storage_metrics_url: String(storageMetricsUrl || ""),
            storage_network_preset: String(storageNetworkPreset || ""),
            storage_data_dir: String(storageDataDir || ""),
            storage_cid_probe: String(storageCidProbe || ""),
            storage_rolling_window: Number(storageRollingWindow || 0),
            storage_local_diagnostics_enabled: storageLocalDiagnosticsEnabled === true,
            storage_privileged_debug_enabled: storagePrivilegedDebugEnabled === true,
            storage_mutating_diagnostics_enabled: storageMutatingDiagnosticsEnabled === true,
            blockchain_refresh_rate: Number(blockchainRefreshRate || 0),
            indexer_refresh_rate: Number(indexerRefreshRate || 0),
            execution_refresh_rate: Number(executionRefreshRate || 0),
            messaging_refresh_rate: Number(messagingRefreshRate || 0),
            storage_refresh_rate: Number(storageRefreshRate || 0),
            footer_fields: footerFieldSelections || {},
            dashboard_graphs: dashboardGraphSelections || {}
        }
    }

    function loadWalletState() {
        const response = bridge.callModule(inspectorModule, "loadWalletState", [])
        walletStateLoaded = true
        if (!response.ok || !response.value || typeof response.value !== "object") {
            return
        }

        const profile = response.value.profile && typeof response.value.profile === "object" ? response.value.profile : response.value
        walletProfileLabel = String(profile.label || profile.name || qsTr("Local wallet"))
        walletBinary = String(profile.wallet_binary || profile.walletBinary || "")
        walletHome = String(profile.wallet_home || profile.walletHome || "")
        walletSequencerUrl = String(profile.sequencer_url || profile.sequencerUrl || "")
        walletIndexerUrl = String(profile.indexer_url || profile.indexerUrl || "")
        walletBedrockNodeUrl = String(profile.bedrock_node_url || profile.bedrockNodeUrl || "")
        walletPublicKeyProbe = String(profile.public_key_probe || profile.publicKeyProbe || "")
        localWalletOperations = Array.isArray(response.value.operations) ? response.value.operations : []
    }

    function saveWalletState() {
        if (!walletStateLoaded) {
            return
        }
        bridge.callModule(inspectorModule, "saveWalletState", [walletStatePayload()])
    }

    function walletStatePayload() {
        return {
            version: 1,
            profile: walletProfile(),
            operations: Array.isArray(localWalletOperations) ? localWalletOperations.slice(-50) : []
        }
    }

    function walletProfile() {
        return {
            label: String(walletProfileLabel || qsTr("Local wallet")),
            wallet_binary: String(walletBinary || ""),
            wallet_home: String(walletHome || ""),
            network_profile: String(networkProfile || ""),
            sequencer_url: String(walletSequencerUrl || sequencerUrl || ""),
            indexer_url: String(walletIndexerUrl || indexerUrl || ""),
            bedrock_node_url: String(walletBedrockNodeUrl || nodeUrl || ""),
            public_key_probe: String(walletPublicKeyProbe || "")
        }
    }

    function walletProfileConfigured() {
        return String(walletBinary || "").trim().length > 0
    }

    function walletProfileUsable() {
        return walletProfileConfigured()
            && localWalletStatus
            && String(localWalletStatus.status || "") === "ok"
    }

    function clearLocalWalletStatus() {
        localWalletStatus = null
        localWalletStatusError = ""
    }

    function walletHomeFallbackLabel() {
        if (String(walletHome || "").trim().length > 0) {
            return String(walletHome || "")
        }
        const source = String(localWalletStatus && localWalletStatus.home_source ? localWalletStatus.home_source : "")
        if (source === "LEE_WALLET_HOME_DIR") {
            return "$LEE_WALLET_HOME_DIR"
        }
        if (source === "NSSA_WALLET_HOME_DIR") {
            return "$NSSA_WALLET_HOME_DIR"
        }
        if (!source.length) {
            return "$LEE_WALLET_HOME_DIR, $NSSA_WALLET_HOME_DIR, or $HOME/.lee/wallet"
        }
        return "$HOME/.lee/wallet"
    }

    function checkLocalWalletProfile(showResult) {
        localWalletStatusError = ""
        statusText = qsTr("Local wallet")
        return requestModuleAsync(inspectorModule, "localWalletProfileStatus", [walletProfile()], qsTr("Local wallet"), showResult === true, function (response) {
            if (response.ok) {
                localWalletStatus = response.value || null
                localWalletStatusError = ""
                appendLocalWalletOperation(qsTr("Profile status"), String(response.value && response.value.status ? response.value.status : "ok"), String(response.value && response.value.detail ? response.value.detail : ""))
            } else {
                localWalletStatus = null
                localWalletStatusError = response.error || qsTr("Profile status failed.")
                appendLocalWalletOperation(qsTr("Profile status"), "down", localWalletStatusError)
            }
        })
    }

    function checkedLocalWalletProfile() {
        const response = requestModule(inspectorModule, "localWalletProfileStatus", [walletProfile()], qsTr("Local wallet"), false)
        if (response.ok) {
            localWalletStatus = response.value || null
            localWalletStatusError = ""
            const status = String(response.value && response.value.status ? response.value.status : "")
            return {
                ok: status === "ok",
                detail: String(response.value && response.value.detail ? response.value.detail : "")
            }
        }
        localWalletStatus = null
        localWalletStatusError = response.error || qsTr("Profile status failed.")
        return {
            ok: false,
            detail: localWalletStatusError
        }
    }

    function queryBedrockWalletBalance() {
        const publicKey = String(walletPublicKeyProbe || "").trim()
        if (!publicKey.length) {
            bedrockWalletBalanceError = qsTr("Wallet public key is required.")
            return
        }
        bedrockWalletBalanceError = ""
        statusText = qsTr("Bedrock wallet")
        return requestModuleAsync(inspectorModule, "bedrockWalletBalance", [String(walletBedrockNodeUrl || nodeUrl || ""), publicKey, String(bedrockWalletBalanceTip || "")], qsTr("Bedrock wallet"), false, function (response) {
            if (response.ok) {
                bedrockWalletBalanceValue = response.value
                bedrockWalletBalanceError = ""
                appendLocalWalletOperation(qsTr("Bedrock balance"), "ok", publicKey)
            } else {
                bedrockWalletBalanceValue = null
                bedrockWalletBalanceError = response.error || qsTr("Balance query failed.")
                appendLocalWalletOperation(qsTr("Bedrock balance"), "down", bedrockWalletBalanceError)
            }
        })
    }

    function appendLocalWalletOperation(label, status, detail) {
        const rows = Array.isArray(localWalletOperations) ? localWalletOperations.slice(-49) : []
        rows.push({
            label: String(label || ""),
            status: String(status || ""),
            detail: String(detail || ""),
            time: new Date().toLocaleTimeString(Qt.locale(), "hh:mm:ss")
        })
        localWalletOperations = rows
        saveWalletState()
    }

    function registeredIdlEntries() {
        const rows = []
        for (let i = 0; i < registeredIdls.count; ++i) {
            rows.push(root.idlEntryAt(i))
        }
        return rows
    }

    function normalizedIdlEntry(entry, fallbackIndex) {
        const row = entry || {}
        const json = String(row.json || "")
        const name = String(row.name || root.idlNameFromJson(json) || qsTr("IDL %1").arg(Number(fallbackIndex || 0) + 1))
        const programId = String(row.programId || row.program_id || "")
        const programIdHex = String(row.programIdHex || row.program_id_hex || root.canonicalProgramIdHex(programId))
        return {
            key: String(row.key || root.idlKey(name, programIdHex, json)),
            name: name,
            programId: programId,
            programIdHex: programIdHex,
            json: json
        }
    }

    function idlEntryAt(index) {
        if (index < 0 || index >= registeredIdls.count) {
            return { key: "", name: "", programId: "", json: "" }
        }
        const row = registeredIdls.get(index)
        return root.normalizedIdlEntry(row, index)
    }

    function idlNameFromJson(json) {
        const parsed = BridgeHelpers.parseJson(String(json || ""))
        return parsed.ok && parsed.value && parsed.value.name ? String(parsed.value.name) : ""
    }

    function idlKey(name, programId, json) {
        const text = String(name || "") + "\n" + String(programId || "") + "\n" + String(json || "")
        let hash = 2166136261
        for (let i = 0; i < text.length; ++i) {
            hash ^= text.charCodeAt(i)
            hash = Math.imul(hash, 16777619)
        }
        return (hash >>> 0).toString(16)
    }

    function idlEntryForKey(key) {
        const text = String(key || "")
        if (!text.length) {
            return null
        }
        for (let i = 0; i < registeredIdls.count; ++i) {
            const entry = root.idlEntryAt(i)
            if (entry.key === text) {
                return entry
            }
        }
        return null
    }

    function idlEntriesForProgram(programId) {
        const normalizedProgram = root.canonicalProgramIdHex(programId) || root.normalizedHexText(programId)
        if (!normalizedProgram.length) {
            return []
        }
        const entries = []
        for (let i = 0; i < registeredIdls.count; ++i) {
            const entry = root.idlEntryAt(i)
            const entryProgram = String(entry.programIdHex || "") || root.canonicalProgramIdHex(entry.programId) || root.normalizedHexText(entry.programId)
            if (entryProgram === normalizedProgram) {
                entries.push(entry)
            }
        }
        return entries
    }

    function cacheAccountIdlSelection(accountId, idlEntry, accountType) {
        const key = root.accountCacheKey(accountId)
        const entry = idlEntry || {}
        const entryKey = String(entry.key || entry.idlKey || "")
        if (!key.length || !entryKey.length) {
            return
        }
        const next = copyMap(accountIdlSelections)
        next[key] = {
            idlKey: entryKey,
            accountType: String(accountType || "")
        }
        accountIdlSelections = next
        accountIdlSelectionRevision += 1
        saveIdlState()
    }

    function accountIdlSelection(accountId) {
        const revision = accountIdlSelectionRevision
        const key = root.accountCacheKey(accountId)
        return key.length ? (accountIdlSelections || {})[key] || null : null
    }

    function cachedIdlEntryForAccount(accountId) {
        const selection = accountIdlSelection(accountId)
        return selection ? root.idlEntryForKey(selection.idlKey) : null
    }

    function cachedAccountType(accountId) {
        const selection = accountIdlSelection(accountId)
        return selection ? String(selection.accountType || "") : ""
    }

    function accountCacheKey(accountId) {
        return String(accountId || "").trim()
    }

    function accountDecodeFullyConsumed(value) {
        if (!value) {
            return false
        }
        const consumed = Number(value.consumed_bytes)
        const total = Number(value.total_bytes)
        const remaining = Number(value.remaining_bytes || 0)
        return Number.isFinite(consumed) && Number.isFinite(total) && consumed === total && remaining === 0
    }

    function transactionDecodeFullyConsumed(value) {
        const decoded = root.transactionDecodedInstruction(value)
        return decoded !== null && Array.isArray(decoded.remaining_words) && decoded.remaining_words.length === 0
    }

    function transactionDecodedInstruction(value) {
        if (!value || typeof value !== "object") {
            return null
        }
        if (value.decoded_instruction) {
            return value.decoded_instruction
        }
        if (value.decoded) {
            return value.decoded
        }
        return null
    }

    function transactionSummaryFromDetail(value) {
        if (!value || typeof value !== "object") {
            return null
        }
        if (value.raw_summary) {
            return value.raw_summary
        }
        if (value.inspection && value.inspection.raw_summary) {
            return value.inspection.raw_summary
        }
        if (value.summary) {
            return value.summary
        }
        return null
    }

    function normalizedHexText(value) {
        return String(value || "").trim().replace(/^0x/i, "").toLowerCase()
    }

    function canonicalProgramIdHex(value) {
        const text = String(value || "").trim()
        if (!text.length) {
            return ""
        }
        if (/^(0x)?[0-9a-fA-F]{64}$/.test(text)) {
            return root.normalizedHexText(text)
        }
        const response = bridge.callModule(inspectorModule, "normalizeProgramId", [text])
        return response.ok && response.value !== undefined && response.value !== null ? String(response.value) : ""
    }

    function autoDecodeAccountData(dataHex, accountId, callback) {
        const serial = accountAutoDecodeSerial + 1
        accountAutoDecodeSerial = serial
        const candidates = root.accountDecodeCandidates(accountId)
        if (!String(dataHex || "").length || candidates.length === 0) {
            callback({ ok: false, error: "", value: null, entry: null })
            return serial
        }

        root.tryAccountDecodeCandidate(serial, String(dataHex || ""), candidates, 0, "", callback)
        return serial
    }

    function accountDecodeCandidates(accountId) {
        const candidates = []
        const cached = root.cachedIdlEntryForAccount(accountId)
        if (cached) {
            candidates.push({
                entry: cached,
                accountType: root.cachedAccountType(accountId),
                cached: true
            })
        }
        for (let i = 0; i < registeredIdls.count; ++i) {
            const entry = root.idlEntryAt(i)
            if (!root.candidateListHasEntry(candidates, entry.key)) {
                candidates.push({
                    entry: entry,
                    accountType: "",
                    cached: false
                })
            }
        }
        return candidates
    }

    function tryAccountDecodeCandidate(serial, dataHex, candidates, index, firstError, callback) {
        if (serial !== accountAutoDecodeSerial) {
            return
        }
        if (index >= candidates.length) {
            callback({ ok: false, error: firstError, value: null, entry: null })
            return
        }

        const candidate = candidates[index]
        decodeAccountDataAsync(dataHex, candidate.entry.json, candidate.accountType, function (response) {
            if (serial !== accountAutoDecodeSerial) {
                return
            }
            const error = firstError.length ? firstError : String(response.error || "")
            if (response.ok && response.value && root.accountDecodeFullyConsumed(response.value)) {
                callback({
                    ok: true,
                    error: "",
                    value: response.value,
                    entry: candidate.entry,
                    accountType: response.value.account_type || candidate.accountType
                })
                return
            }
            root.tryAccountDecodeCandidate(serial, dataHex, candidates, index + 1, error, callback)
        })
    }

    function autoDecodeTransactionDetail(detail) {
        const summary = root.transactionSummaryFromDetail(detail)
        if (!summary || String(summary.kind || "") !== "Public" || !Array.isArray(summary.instruction_data) || summary.instruction_data.length === 0) {
            return
        }

        const serial = transactionAutoDecodeSerial + 1
        transactionAutoDecodeSerial = serial
        const candidates = root.transactionDecodeCandidates(summary)
        if (candidates.length === 0) {
            return
        }

        root.tryTransactionDecodeCandidate(serial, summary, candidates, 0)
    }

    function transactionDecodeCandidates(summary) {
        const candidates = []
        const accountIds = Array.isArray(summary.account_ids) ? summary.account_ids : []
        for (let i = 0; i < accountIds.length; ++i) {
            const cached = root.cachedIdlEntryForAccount(accountIds[i])
            if (cached && !root.candidateListHasEntry(candidates, cached.key)) {
                candidates.push({
                    entry: cached,
                    cached: true
                })
            }
        }

        const programEntries = root.idlEntriesForProgram(summary.program_id_hex)
        for (let j = 0; j < programEntries.length; ++j) {
            if (!root.candidateListHasEntry(candidates, programEntries[j].key)) {
                candidates.push({
                    entry: programEntries[j],
                    cached: false
                })
            }
        }

        for (let k = 0; k < registeredIdls.count; ++k) {
            const entry = root.idlEntryAt(k)
            if (!root.candidateListHasEntry(candidates, entry.key)) {
                candidates.push({
                    entry: entry,
                    cached: false
                })
            }
        }
        return candidates
    }

    function candidateListHasEntry(candidates, key) {
        const text = String(key || "")
        for (let i = 0; i < candidates.length; ++i) {
            if (String(candidates[i].entry.key || "") === text) {
                return true
            }
        }
        return false
    }

    function tryTransactionDecodeCandidate(serial, summary, candidates, index) {
        if (serial !== transactionAutoDecodeSerial) {
            return
        }
        if (index >= candidates.length) {
            return
        }

        const candidate = candidates[index]
        decodeTransactionSummaryAsync(summary, candidate.entry.json, function (response) {
            if (serial !== transactionAutoDecodeSerial) {
                return
            }
            if (response.ok && response.value && root.transactionDecodeFullyConsumed(response.value)) {
                transactionDetailValue = response.value
                transactionsPageError = ""
                setResult(qsTr("Transaction"), response.text, false, response.value)
                return
            }
            root.tryTransactionDecodeCandidate(serial, summary, candidates, index + 1)
        })
    }

    function refreshInterval(seconds) {
        return Math.max(5, Number(seconds || 0)) * 1000
    }

    function networkConnectionRate(kind) {
        switch (kind) {
        case "blockchain":
            return blockchainRefreshRate
        case "indexer":
            return indexerRefreshRate
        case "execution":
            return executionRefreshRate
        case "messaging":
            return messagingRefreshRate
        case "storage":
            return storageRefreshRate
        default:
            return 30
        }
    }

    function setNetworkConnectionRate(kind, seconds) {
        const value = Math.max(0, Number(seconds || 0))
        switch (kind) {
        case "blockchain":
            blockchainRefreshRate = value
            return
        case "indexer":
            indexerRefreshRate = value
            return
        case "execution":
            executionRefreshRate = value
            return
        case "messaging":
            messagingRefreshRate = value
            return
        case "storage":
            storageRefreshRate = value
        }
    }

    function queryNetworkConnection(kind, showResult) {
        const target = String(kind || "")
        const request = root.networkConnectionRequest(target)
        if (!request) {
            return {
                ok: false,
                text: "",
                error: qsTr("Unknown connection.")
            }
        }

        if (root.networkConnectionPending[target] === true) {
            return {
                ok: false,
                text: "",
                error: qsTr("Connection query already running.")
            }
        }

        root.setNetworkConnectionPending(target, true)
        return requestModuleAsync(request.module, request.method, request.args, request.label, showResult, function (response) {
            root.setNetworkConnectionPending(target, false)
            root.updateNetworkConnectionStatus(target, response)
            root.recordDashboardSnapshot()
        })
    }

    function networkConnectionRequest(kind) {
        switch (kind) {
        case "blockchain":
            return { module: inspectorModule, method: "blockchainNode", args: [nodeUrl], label: qsTr("Blockchain node") }
        case "indexer":
            return { module: inspectorModule, method: "indexerFinalizedHead", args: [indexerUrl], label: qsTr("Indexer head") }
        case "execution":
            return { module: inspectorModule, method: "head", args: [sequencerUrl], label: qsTr("Sequencer head") }
        case "messaging":
            return { module: inspectorModule, method: "deliverySourceReport", args: root.deliverySourceReportArgs(), label: qsTr("Delivery source") }
        case "storage":
            return { module: inspectorModule, method: "storageSourceReport", args: root.storageSourceReportArgs(), label: qsTr("Storage source") }
        default:
            return null
        }
    }

    function updateNetworkConnectionStatusForMethod(method, response) {
        const kind = root.networkConnectionKindForMethod(method)
        if (kind.length > 0) {
            root.updateNetworkConnectionStatus(kind, response)
        }
    }

    function networkConnectionKindForMethod(method) {
        switch (String(method || "")) {
        case "blockchainNode":
            return "blockchain"
        case "indexerFinalizedHead":
            return "indexer"
        case "head":
            return "execution"
        case "deliveryReport":
        case "deliverySourceReport":
            return "messaging"
        case "storageReport":
        case "storageSourceReport":
            return "storage"
        default:
            return ""
        }
    }

    function setNetworkConnectionPending(kind, pending) {
        const next = copyMap(networkConnectionPending)
        next[String(kind || "")] = pending === true
        networkConnectionPending = next
        networkConnectionPendingRevision += 1
    }

    function networkConnectionIsPending(kind) {
        const revision = networkConnectionPendingRevision
        return networkConnectionPending[String(kind || "")] === true
    }

    function updateNetworkConnectionStatus(kind, response) {
        const next = copyMap(networkConnectionStatus)
        const value = response && response.value !== undefined ? response.value : null
        const ok = response && response.ok === true && root.connectionValueOk(kind, value)
        next[kind] = {
            known: true,
            ok: ok,
            text: ok ? qsTr("OK") : qsTr("Error"),
            detail: response && response.ok ? networkConnectionSummary(kind, value) : (response && response.error ? response.error : qsTr("No response")),
            value: value,
            checkedAt: new Date().toLocaleTimeString(Qt.locale(), "hh:mm:ss")
        }
        networkConnectionStatus = next
        networkConnectionStatusRevision += 1
    }

    function networkConnectionSummary(kind, value) {
        if (kind === "blockchain") {
            const info = value && value.cryptarchia_info ? value.cryptarchia_info : null
            return info && info.slot !== undefined ? qsTr("slot %1").arg(info.slot) : qsTr("node reachable")
        }
        if (kind === "indexer" || kind === "execution") {
            const scalar = root.scalarValue(value)
            return scalar !== null ? qsTr("head %1").arg(root.valueText(scalar)) : qsTr("reachable")
        }
        if (kind === "messaging") {
            if (!root.moduleReportReachable(value)) {
                return root.moduleReportError(value) || qsTr("source unavailable")
            }
            const version = root.moduleProbeValue("messaging", "version")
            return version !== null ? qsTr("version %1").arg(root.valueText(version)) : qsTr("%1 reachable").arg(root.deliverySourceLabel())
        }
        if (kind === "storage") {
            if (!root.moduleReportReachable(value)) {
                return root.moduleReportError(value) || qsTr("source unavailable")
            }
            const version = root.moduleProbeValue("storage", "version") || root.moduleProbeValue("storage", "moduleVersion")
            return version !== null ? qsTr("version %1").arg(root.valueText(version)) : qsTr("%1 reachable").arg(root.storageSourceLabel())
        }
        return qsTr("reachable")
    }

    function connectionValueOk(kind, value) {
        if (kind === "messaging" || kind === "storage") {
            return root.moduleReportReachable(value)
        }
        return true
    }

    function moduleReportReachable(report) {
        if (!report || typeof report !== "object") {
            return false
        }
        if (report.module_info && report.module_info.ok === true) {
            return true
        }
        const probes = Array.isArray(report.probes) ? report.probes : []
        for (let i = 0; i < probes.length; ++i) {
            if (probes[i] && probes[i].ok === true) {
                return true
            }
        }
        return false
    }

    function moduleReportError(report) {
        if (!report || typeof report !== "object") {
            return ""
        }
        if (report.module_info && report.module_info.ok === false && report.module_info.error) {
            return String(report.module_info.error)
        }
        const probes = Array.isArray(report.probes) ? report.probes : []
        for (let i = 0; i < probes.length; ++i) {
            if (probes[i] && probes[i].ok === false && probes[i].error) {
                return String(probes[i].error)
            }
        }
        return ""
    }

    function deliverySourceReportArgs() {
        return [
            String(messagingSourceMode || "module"),
            String(messagingRestUrl || ""),
            String(messagingMetricsUrl || ""),
            String(messagingNodeInfoId || "")
        ]
    }

    function deliverySourceLabel() {
        switch (String(messagingSourceMode || "module")) {
        case "rest":
            return qsTr("Direct Waku REST")
        case "metrics":
            return qsTr("Metrics only")
        case "network-monitor":
            return qsTr("Network monitor")
        case "discovery-crawler":
            return qsTr("Discovery crawler")
        default:
            return qsTr("Basecamp module")
        }
    }

    function deliverySourceTarget() {
        switch (String(messagingSourceMode || "module")) {
        case "rest":
            return String(messagingRestUrl || "")
        case "metrics":
            return String(messagingMetricsUrl || "")
        case "network-monitor":
            return String(messagingRestUrl || "")
        case "discovery-crawler":
            return root.normalizedMessagingNetworkPreset(messagingNetworkPreset)
        default:
            return String(deliveryModule || "")
        }
    }

    function storageSourceReportArgs() {
        return [
            String(storageSourceMode || "module"),
            String(storageRestUrl || ""),
            String(storageMetricsUrl || ""),
            String(storageCidProbe || "")
        ]
    }

    function storageSourceLabel() {
        switch (String(storageSourceMode || "module")) {
        case "rest":
            return qsTr("Standalone REST")
        case "metrics":
            return qsTr("Metrics only")
        case "c-library":
            return qsTr("C library")
        case "local-os":
            return qsTr("Local OS diagnostics")
        default:
            return qsTr("Basecamp module")
        }
    }

    function storageSourceTarget() {
        switch (String(storageSourceMode || "module")) {
        case "rest":
            return String(storageRestUrl || "")
        case "metrics":
            return String(storageMetricsUrl || "")
        case "c-library":
        case "local-os":
            return String(storageDataDir || storageNetworkPreset || "")
        default:
            return String(storageModule || "")
        }
    }

    function networkConnectionState(kind) {
        const revision = networkConnectionStatusRevision
        const status = networkConnectionStatus[String(kind || "")]
        if (!status) {
            return {
                known: false,
                ok: false,
                text: qsTr("Unknown"),
                detail: qsTr("Not queried"),
                checkedAt: ""
            }
        }
        return status
    }

    function setFooterFieldEnabled(key, enabled) {
        const next = copyMap(footerFieldSelections)
        next[String(key || "")] = enabled === true
        footerFieldSelections = next
        footerFieldRevision += 1
    }

    function footerFieldEnabled(key) {
        const revision = footerFieldRevision
        const value = footerFieldSelections[String(key || "")]
        return value === true
    }

    function setDashboardGraphEnabled(key, enabled) {
        const next = copyMap(dashboardGraphSelections)
        next[String(key || "")] = enabled === true
        dashboardGraphSelections = next
        dashboardGraphRevision += 1
    }

    function dashboardGraphEnabled(key) {
        const revision = dashboardGraphRevision
        const value = dashboardGraphSelections[String(key || "")]
        return value === true
    }

    function copyMap(source) {
        const next = {}
        const current = source || {}
        for (const key in current) {
            next[key] = current[key]
        }
        return next
    }

    function mergeMap(base, overrides) {
        const next = root.copyMap(base)
        const current = overrides || {}
        for (const key in current) {
            next[key] = current[key]
        }
        return next
    }

    function stringSetting(value, key, fallback) {
        const raw = value ? value[key] : undefined
        return raw === undefined || raw === null ? String(fallback || "") : String(raw)
    }

    function numberSetting(value, key, fallback) {
        const number = Number(value ? value[key] : undefined)
        return Number.isFinite(number) ? number : Number(fallback || 0)
    }

    function boolSetting(value, key, fallback) {
        const raw = value ? value[key] : undefined
        if (raw === true || raw === false) {
            return raw
        }
        return fallback === true
    }

    function normalizedNetworkProfile(value) {
        const profile = String(value || "default")
        if (profile === "local" || profile === "custom") {
            return profile
        }
        return "default"
    }

    function resolvedNetworkProfile(storedProfile, sequencer, indexer, node) {
        const inferred = root.inferNetworkProfileFromEndpoints(sequencer, indexer, node)
        if (inferred !== "custom") {
            return inferred
        }
        return root.normalizedNetworkProfile(storedProfile) === "custom" ? "custom" : inferred
    }

    function inferNetworkProfileFromEndpoints(sequencer, indexer, node) {
        const seq = root.normalizeEndpoint(sequencer)
        const idx = root.normalizeEndpoint(indexer)
        const nod = root.normalizeEndpoint(node)
        const testnetSeq = root.normalizeEndpoint("https://testnet.lez.logos.co/")
        const localSeq = root.normalizeEndpoint("http://127.0.0.1:3040/")
        const localIndexer = root.normalizeEndpoint("http://127.0.0.1:8779/")
        const localNode = root.normalizeEndpoint("http://127.0.0.1:8080/")

        if (seq === localSeq && idx === localIndexer && nod === localNode) {
            return "local"
        }
        if (seq === testnetSeq && idx === localIndexer && nod === localNode) {
            return "default"
        }
        return "custom"
    }

    function normalizeEndpoint(value) {
        return String(value || "").trim().replace(/\/+$/, "")
    }

    function normalizedMessagingNetworkPreset(value) {
        const preset = String(value || "").trim()
        if (!preset.length || preset === "testnet") {
            return "logos.test"
        }
        return preset
    }

    function scalarValue(value) {
        if (value === undefined || value === null || value === "") {
            return null
        }
        if (typeof value === "number" || typeof value === "string" || typeof value === "boolean") {
            return value
        }
        if (Array.isArray(value)) {
            return value.length
        }
        if (typeof value === "object") {
            if (value.result !== undefined && value.result !== null) {
                return root.scalarValue(value.result)
            }
            if (value.value !== undefined && value.value !== null) {
                return root.scalarValue(value.value)
            }
            if (value.count !== undefined && value.count !== null) {
                return root.scalarValue(value.count)
            }
            if (value.total !== undefined && value.total !== null) {
                return root.scalarValue(value.total)
            }
        }
        return null
    }

    function valueText(value) {
        const scalar = root.scalarValue(value)
        if (scalar === null) {
            return "-"
        }
        if (typeof scalar === "number") {
            return scalar.toLocaleString(Qt.locale(), "f", Number.isInteger(scalar) ? 0 : 2)
        }
        return String(scalar)
    }

    function moduleReport(kind) {
        if (kind === "storage") {
            return storageModuleReport || null
        }
        if (kind === "messaging") {
            return messagingModuleReport || null
        }
        return null
    }

    function moduleProbe(kind, method) {
        const report = root.moduleReport(kind)
        const probes = report && Array.isArray(report.probes) ? report.probes : []
        const wanted = String(method || "")
        for (let i = 0; i < probes.length; ++i) {
            const probe = probes[i] || {}
            const label = String(probe.label || "")
            const source = String(probe.source || "")
            if (label.indexOf("." + wanted) >= 0 || source.indexOf(" " + wanted) >= 0) {
                return probe
            }
        }
        return null
    }

    function moduleProbeValue(kind, method) {
        const probe = root.moduleProbe(kind, method)
        if (!probe || probe.ok !== true || probe.value === undefined || probe.value === null) {
            return null
        }
        return probe.value
    }

    function moduleProbeError(kind, method) {
        const probe = root.moduleProbe(kind, method)
        return probe && probe.error ? String(probe.error) : ""
    }

    function moduleLastError(kind) {
        const report = root.moduleReport(kind)
        if (!report) {
            return ""
        }
        if (report.module_info && report.module_info.ok === false && report.module_info.error) {
            return String(report.module_info.error)
        }
        const probes = Array.isArray(report.probes) ? report.probes : []
        for (let i = 0; i < probes.length; ++i) {
            const probe = probes[i] || {}
            if (probe.ok === false && probe.error) {
                return String(probe.error)
            }
        }
        return ""
    }

    function openMetricsText(kind) {
        const value = root.moduleProbeValue(kind, kind === "storage" ? "collectMetrics" : "collectOpenMetricsText")
        return root.openMetricsTextFromValue(value)
    }

    function openMetricsTextFromValue(value) {
        if (typeof value === "string") {
            return value
        }
        const scalar = root.scalarValue(value)
        return scalar === null ? "" : String(scalar)
    }

    function openMetricValue(kind, names) {
        const wanted = Array.isArray(names) ? names : [names]
        const value = root.moduleProbeValue(kind, kind === "storage" ? "collectMetrics" : "collectOpenMetricsText")
        const jsonMetric = root.metricJsonValue(value, wanted)
        if (jsonMetric !== null) {
            return jsonMetric
        }
        const text = root.openMetricsTextFromValue(value)
        if (!text.length) {
            return null
        }
        const lines = text.split(/\r?\n/)
        for (let i = 0; i < lines.length; ++i) {
            const line = lines[i].trim()
            if (!line.length || line[0] === "#") {
                continue
            }
            const name = line.split(/[{\s]/)[0]
            for (let j = 0; j < wanted.length; ++j) {
                if (name === wanted[j]) {
                    const match = line.match(/^[^\s]+\s+(-?[0-9]+(?:\.[0-9]+)?(?:e[+-]?[0-9]+)?)/i)
                    if (match) {
                        const number = Number(match[1])
                        return Number.isFinite(number) ? number : null
                    }
                }
            }
        }
        return null
    }

    function metricJsonValue(value, names) {
        if (value === undefined || value === null) {
            return null
        }
        const wanted = Array.isArray(names) ? names : [names]
        if (Array.isArray(value)) {
            for (let i = 0; i < value.length; ++i) {
                const match = root.metricJsonValue(value[i], wanted)
                if (match !== null) {
                    return match
                }
            }
            return null
        }
        if (typeof value !== "object") {
            return null
        }
        if (Array.isArray(value.metrics)) {
            return root.metricJsonValue(value.metrics, wanted)
        }
        const metricName = String(value.name || value.metric || value.key || "")
        for (let i = 0; i < wanted.length; ++i) {
            const wantedName = String(wanted[i] || "")
            if (metricName === wantedName) {
                return root.metricNumber(value.value !== undefined ? value.value : (value.count !== undefined ? value.count : value.total))
            }
            if (value[wantedName] !== undefined) {
                return root.metricNumber(value[wantedName])
            }
        }
        return null
    }

    function metricNumber(value) {
        const scalar = root.scalarValue(value)
        const number = Number(scalar)
        return Number.isFinite(number) ? number : null
    }

    function overviewProbeValue(section, field) {
        const sectionValue = dashboardOverview ? dashboardOverview[section] : null
        const probe = sectionValue ? sectionValue[field] : null
        return probe && probe.value !== undefined && probe.value !== null ? root.scalarValue(probe.value) : null
    }

    function indexerHeadValue() {
        const overviewValue = root.overviewProbeValue("indexer", "head")
        if (overviewValue !== null) {
            return overviewValue
        }
        const status = networkConnectionStatus.indexer
        const statusValue = status ? root.scalarValue(status.value) : null
        if (statusValue !== null) {
            return statusValue
        }
        const blocks = dashboardBlocks || []
        if (blocks.length > 0) {
            return root.scalarValue((blocks[0] || {}).block_id)
        }
        return null
    }

    function sequencerHeadValue() {
        const overviewValue = root.overviewProbeValue("sequencer", "head")
        if (overviewValue !== null) {
            return overviewValue
        }
        const status = networkConnectionStatus.execution
        return status ? root.scalarValue(status.value) : null
    }

    function nodeProbeValue(name) {
        const report = dashboardNode || {}
        const probe = report[name]
        return probe && probe.value !== undefined && probe.value !== null ? probe.value : null
    }

    function cryptarchiaInfo() {
        const fromOverview = dashboardOverview && dashboardOverview.node && dashboardOverview.node.consensus
            ? dashboardOverview.node.consensus.value
            : null
        if (fromOverview && typeof fromOverview === "object") {
            return fromOverview.cryptarchia_info || fromOverview
        }
        const fromNode = root.nodeProbeValue("cryptarchia_info")
        if (fromNode && typeof fromNode === "object") {
            return fromNode.cryptarchia_info || fromNode
        }
        return {}
    }

    function cryptarchiaValue(key) {
        const value = root.cryptarchiaInfo()[key]
        return value === undefined || value === null ? null : root.scalarValue(value)
    }

    function networkInfo() {
        const value = root.nodeProbeValue("network_info")
        return value && typeof value === "object" ? value : {}
    }

    function networkValue(key) {
        const value = root.networkInfo()[key]
        return value === undefined || value === null ? null : root.scalarValue(value)
    }

    function mantleMetrics() {
        const value = root.nodeProbeValue("mantle_metrics")
        return value && typeof value === "object" ? value : {}
    }

    function mantleValue(keys) {
        const list = Array.isArray(keys) ? keys : [keys]
        const metrics = root.mantleMetrics()
        for (let i = 0; i < list.length; ++i) {
            const value = metrics[list[i]]
            if (value !== undefined && value !== null) {
                return root.scalarValue(value)
            }
        }
        return null
    }

    function tipMinusLib() {
        const tip = Number(root.cryptarchiaValue("slot"))
        const lib = Number(root.cryptarchiaValue("lib_slot"))
        return Number.isFinite(tip) && Number.isFinite(lib) ? Math.max(0, tip - lib) : null
    }

    function finalityLagSeconds() {
        const gap = root.tipMinusLib()
        return gap === null ? null : gap * 2
    }

    function indexerLag() {
        const sequencerValue = root.sequencerHeadValue()
        const indexerValue = root.indexerHeadValue()
        if (sequencerValue === null || indexerValue === null) {
            return null
        }
        const sequencerHead = Number(sequencerValue)
        const indexerHead = Number(indexerValue)
        return Number.isFinite(sequencerHead) && Number.isFinite(indexerHead) ? Math.max(0, sequencerHead - indexerHead) : null
    }

    function moduleMetricValue(kind, names) {
        const metric = root.openMetricValue(kind, names)
        if (metric !== null) {
            return metric
        }
        return null
    }

    function dashboardMetricValue(key) {
        switch (key) {
        case "bedrock.peer_count":
            return root.networkValue("n_peers")
        case "bedrock.tip_minus_lib":
            return root.tipMinusLib()
        case "bedrock.finality_lag_seconds":
            return root.finalityLagSeconds()
        case "lez.pending_tx_count":
            return root.mantleValue(["pending_tx_count", "pending_txs", "pending_transactions"])
        case "lez.mempool_tx_count":
            return root.mantleValue(["mempool_tx_count", "mempool_txs", "mempool_size"])
        case "lez.rejected_tx_count_recent":
            return root.mantleValue(["rejected_tx_count_recent", "rejected_txs_recent"])
        case "lez.blocks_produced_recent":
            return Array.isArray(dashboardBlocks) ? dashboardBlocks.length : null
        case "lez.pending_blocks_count":
            return root.mantleValue(["pending_blocks_count", "pending_blocks"])
        case "indexer.indexer_lag_vs_sequencer_head":
            return root.indexerLag()
        case "storage.peer_count":
            return root.moduleMetricValue("storage", ["storage_peer_count", "storage_libp2p_peers", "libp2p_peers", "peers"])
        case "storage.shared_files_count":
            return root.moduleMetricValue("storage", ["storage_shared_files_count", "shared_files_count"])
        case "storage.manifest_count":
            return root.moduleMetricValue("storage", ["storage_manifest_count", "manifest_count"])
        case "storage.local_storage_used":
            return root.moduleMetricValue("storage", ["storage_local_storage_used_bytes", "local_storage_used_bytes", "storage_used_bytes", "storage_repostore_bytes_used"])
        case "storage.active_uploads":
            return root.moduleMetricValue("storage", ["storage_active_uploads", "active_uploads", "storage_api_uploads"])
        case "storage.active_downloads":
            return root.moduleMetricValue("storage", ["storage_active_downloads", "active_downloads", "storage_api_downloads"])
        case "storage.failed_transfers_recent":
            return root.moduleMetricValue("storage", ["storage_failed_transfers_recent", "failed_transfers_recent", "storage_block_exchange_requests_failed_total", "storage_block_exchange_peer_timeouts_total"])
        case "messaging.peer_count":
            return root.moduleMetricValue("messaging", ["libp2p_peers", "waku_peers", "messaging_peer_count", "peer_count"])
        case "messaging.active_subscriptions":
            return root.moduleMetricValue("messaging", ["active_subscriptions"])
        case "messaging.pubsub_peers":
            return root.moduleMetricValue("messaging", ["libp2p_pubsub_peers"])
        case "messaging.store_peers":
            return root.moduleMetricValue("messaging", ["waku_store_peers"])
        case "messaging.filter_peers":
            return root.moduleMetricValue("messaging", ["waku_filter_peers"])
        case "messaging.lightpush_peers":
            return root.moduleMetricValue("messaging", ["waku_lightpush_peers"])
        case "messaging.content_topics":
            return root.moduleMetricValue("messaging", ["content_topics"])
        case "messaging.outbound_queue":
            return root.moduleMetricValue("messaging", ["outbound_queue"])
        case "messaging.message_sent_events_recent":
            return null
        case "messaging.message_propagated_events_recent":
            return null
        case "messaging.message_received_events_recent":
            return root.moduleMetricValue("messaging", ["waku_node_messages_total", "waku_node_messages", "message_received_events_recent"])
        case "messaging.message_error_events_recent":
            return root.moduleMetricValue("messaging", ["waku_node_errors_total", "waku_node_errors", "message_error_events_recent"])
        case "messaging.publish_latency_ms":
            return null
        case "messaging.receive_latency_ms":
            return null
        default:
            return null
        }
    }

    function dashboardMetricText(key) {
        return root.valueText(root.dashboardMetricValue(key))
    }

    function recordDashboardSnapshot() {
        const keys = [
            "bedrock.peer_count",
            "bedrock.tip_minus_lib",
            "bedrock.finality_lag_seconds",
            "lez.pending_tx_count",
            "lez.mempool_tx_count",
            "lez.rejected_tx_count_recent",
            "lez.blocks_produced_recent",
            "lez.pending_blocks_count",
            "indexer.indexer_lag_vs_sequencer_head",
            "storage.peer_count",
            "storage.shared_files_count",
            "storage.manifest_count",
            "storage.local_storage_used",
            "storage.active_uploads",
            "storage.active_downloads",
            "storage.failed_transfers_recent",
            "messaging.peer_count",
            "messaging.active_subscriptions",
            "messaging.content_topics",
            "messaging.outbound_queue",
            "messaging.message_sent_events_recent",
            "messaging.message_propagated_events_recent",
            "messaging.message_received_events_recent",
            "messaging.message_error_events_recent",
            "messaging.publish_latency_ms",
            "messaging.receive_latency_ms"
        ]
        const next = copyMap(dashboardMetricHistory)
        let changed = false
        for (let i = 0; i < keys.length; ++i) {
            const value = Number(root.dashboardMetricValue(keys[i]))
            if (!Number.isFinite(value)) {
                continue
            }
            const samples = Array.isArray(next[keys[i]]) ? next[keys[i]].slice(-23) : []
            samples.push(value)
            next[keys[i]] = samples
            changed = true
        }
        if (changed) {
            dashboardMetricHistory = next
            dashboardMetricHistoryRevision += 1
        }
    }

    function dashboardMetricSamples(key) {
        const revision = dashboardMetricHistoryRevision
        const samples = dashboardMetricHistory[String(key || "")]
        if (Array.isArray(samples) && samples.length > 0) {
            return samples
        }
        const value = Number(root.dashboardMetricValue(key))
        return Number.isFinite(value) ? [value] : []
    }

    function defaultFooterFieldSelections() {
        return {
            "network.network": true,
            "bedrock.node_health": true,
            "bedrock.sync_state": true,
            "bedrock.tip_height": true,
            "bedrock.tip_minus_lib": true,
            "lez.rpc_health": true,
            "lez.last_lez_block_id": true,
            "indexer.rpc_health": true,
            "indexer.indexed_finalized_height": true,
            "messaging.connection_state": true,
            "messaging.peer_count": true,
            "messaging.message_error_events_recent": true,
            "storage.module": true,
            "storage.node_reachable": true,
            "storage.peer_count": true,
            "storage.failed_transfers_recent": true,
            "overall.status": true,
            "overall.main_risk": true,
            "overall.operator_action": true
        }
    }

    function defaultDashboardGraphSelections() {
        return {
            "bedrock.peer_count": true,
            "bedrock.tip_minus_lib": true,
            "bedrock.finality_lag_seconds": true,
            "lez.blocks_produced_recent": true,
            "indexer.indexer_lag_vs_sequencer_head": true
        }
    }

    function refreshBlocksPage(anchorSlot) {
        const node = requestModule(inspectorModule, "blockchainNode", [nodeUrl], qsTr("Blocks node state"), false)
        if (!node.ok) {
            blocksPageError = node.error
            setResult(qsTr("Blocks"), blocksPageError, true)
            return
        }

        const infoProbe = node.value ? node.value.cryptarchia_info : null
        const info = infoProbe && infoProbe.value ? infoProbe.value.cryptarchia_info : null
        const fallbackSlot = info ? (info.lib_slot || info.slot || 0) : 0
        const requestedSlot = Math.max(0, Number(anchorSlot === undefined || anchorSlot === null ? fallbackSlot : anchorSlot))
        const slotTo = fallbackSlot > 0 ? Math.min(requestedSlot, Number(fallbackSlot)) : requestedSlot
        const slotFrom = Math.max(0, slotTo - blocksPageWindow)
        const blocks = requestModule(inspectorModule, "blockchainBlocks", [nodeUrl, slotFrom, slotTo], qsTr("Blocks"), false)
        if (!blocks.ok) {
            blocksPageError = blocks.error
            setResult(qsTr("Blocks"), blocksPageError, true)
            return
        }

        blocksPageSlotFrom = slotFrom
        blocksPageSlotTo = slotTo
        blocksPageRows = sortedBlocks(blocks.value || []).slice(0, blocksPageLimit)
        blocksPageError = ""
        setResult(qsTr("Blocks"), BridgeHelpers.formatValue(blocksPageRows), false, blocksPageRows)
    }

    function olderBlocksPage() {
        refreshBlocksPage(Math.max(0, blocksPageSlotFrom - 1))
    }

    function newerBlocksPage() {
        refreshBlocksPage(blocksPageSlotTo + blocksPageWindow + 1)
    }

    function setBlocksPageLimit(limit) {
        const value = Math.max(1, Number(limit || blocksPageLimit))
        if (blocksPageLimit === value) {
            return
        }
        blocksPageLimit = value
        refreshBlocksPage(blocksPageSlotTo > 0 ? blocksPageSlotTo : null)
    }

    function sortedBlocks(blocks) {
        const copy = Array.isArray(blocks) ? blocks.slice(0) : []
        copy.sort(function (left, right) {
            return blockSlot(right) - blockSlot(left)
        })
        return copy
    }

    function blockSlot(block) {
        return Number(block && block.header ? (block.header.slot || 0) : 0)
    }

    function blockHash(block) {
        const raw = block || {}
        const header = raw.header || {}
        return String(header.id || header.hash || raw.header_hash || raw.hash || "")
    }

    function blockParent(block) {
        const raw = block || {}
        const header = raw.header || {}
        return String(header.parent_block || header.parent_hash || header.parent || raw.parent_hash || raw.parent || "")
    }

    function blockProof(block) {
        const raw = block || {}
        const header = raw.header || {}
        return header.proof_of_leadership || raw.proof_of_leadership || {}
    }

    function blockRoot(block) {
        const raw = block || {}
        const header = raw.header || {}
        return String(header.block_root || raw.block_root || "")
    }

    function blockHeight(block) {
        const raw = block || {}
        const header = raw.header || {}
        return raw.height !== undefined ? raw.height : header.height
    }

    function blockVersion(block) {
        const raw = block || {}
        const header = raw.header || {}
        return raw.version !== undefined ? raw.version : header.version
    }

    function blockSignature(block) {
        const raw = block || {}
        const header = raw.header || {}
        return String(raw.signature_hex || raw.signature || header.signature_hex || header.signature || "")
    }

    function blockStatus(block) {
        const raw = block || {}
        const explicitStatus = String(raw.bedrock_status || raw.status || "")
        if (explicitStatus.length) {
            return explicitStatus
        }

        const slot = blockSlot(block)
        const info = blockchainInfo()
        if (!slot || !info) {
            return "-"
        }
        if (info.lib_slot !== undefined && slot <= Number(info.lib_slot)) {
            return qsTr("finalized")
        }
        if (info.slot !== undefined && slot <= Number(info.slot)) {
            return qsTr("pending")
        }
        return "-"
    }

    function blockchainInfo() {
        const report = dashboardNode
        const probe = report ? report.cryptarchia_info : null
        return probe && probe.value ? probe.value.cryptarchia_info : null
    }

    function blockTransactions(block) {
        const raw = block || {}
        const transactions = Array.isArray(raw.transactions) ? raw.transactions : []
        const rows = []
        for (let i = 0; i < transactions.length; ++i) {
            const tx = transactions[i]
            const ops = transactionOps(tx)
            rows.push({
                index: i,
                hash: transactionHash(tx),
                ops: ops.length,
                operations: ops.map(function (op, index) {
                    return operationSummary(op, tx, index)
                }),
                raw: tx
            })
        }
        return rows
    }

    function blockchainBlockDetail(block) {
        const proof = blockProof(block)
        return {
            type: "blockchain_block",
            hash: blockHash(block),
            parent: blockParent(block),
            slot: blockSlot(block),
            height: blockHeight(block),
            status: blockStatus(block),
            version: blockVersion(block),
            block_root: blockRoot(block),
            voucher_cm: String(proof.voucher_cm || ""),
            entropy: String(proof.entropy_contribution || proof.entropy || ""),
            signature: blockSignature(block),
            leader_key: String(proof.leader_key || ""),
            transactions: blockTransactions(block),
            raw: block
        }
    }

    function blockchainBlockDetailById(value) {
        const wanted = normalizedHashOrValue(value)
        if (!wanted.length) {
            return null
        }
        const rows = blocksPageRows || []
        for (let i = 0; i < rows.length; ++i) {
            const block = rows[i]
            const hash = blockHash(block)
            const slot = String(blockSlot(block))
            if (normalizedHashOrValue(hash) === wanted || slot === wanted) {
                return blockchainBlockDetail(block)
            }
        }
        return null
    }

    function normalizedHashOrValue(value) {
        let text = String(value || "").trim().toLowerCase()
        if (text.startsWith("0x") && text.length === 66) {
            text = text.slice(2)
        }
        return text
    }

    function refreshTransactionsPage(beforeBlock) {
        const node = requestModule(inspectorModule, "blockchainNode", [nodeUrl], qsTr("Transactions node state"), false)
        if (!node.ok) {
            transactionsPageError = node.error
            setResult(qsTr("Transactions"), transactionsPageError, true)
            return
        }

        const infoProbe = node.value ? node.value.cryptarchia_info : null
        const info = infoProbe && infoProbe.value ? infoProbe.value.cryptarchia_info : null
        const fallbackSlot = info ? (info.lib_slot || info.slot || 0) : 0
        const requestedSlot = Math.max(0, Number(beforeBlock === undefined || beforeBlock === null ? fallbackSlot : beforeBlock))
        const slotTo = fallbackSlot > 0 ? Math.min(requestedSlot, Number(fallbackSlot)) : requestedSlot
        const slotFrom = Math.max(0, slotTo - transactionsPageBlockBatch)
        const blocks = requestModule(inspectorModule, "blockchainBlocks", [nodeUrl, slotFrom, slotTo], qsTr("Transactions"), false)
        if (!blocks.ok) {
            transactionsPageError = blocks.error
            setResult(qsTr("Transactions"), transactionsPageError, true)
            return
        }

        transactionsPageBeforeBlock = slotTo
        transactionsPageRows = transactionRowsFromBlocks(blocks.value || []).slice(0, transactionsPageLimit)
        transactionsPageNextBeforeBlock = slotFrom > 0 ? slotFrom - 1 : 0
        transactionsPageError = ""
        setResult(qsTr("Transactions"), BridgeHelpers.formatValue(transactionsPageRows), false, transactionsPageRows)
    }

    function olderTransactionsPage() {
        refreshTransactionsPage(transactionsPageNextBeforeBlock)
    }

    function newerTransactionsPage() {
        refreshTransactionsPage(transactionsPageBeforeBlock + transactionsPageBlockBatch + 1)
    }

    function setTransactionsPageLimit(limit) {
        const value = Math.max(1, Number(limit || transactionsPageLimit))
        if (transactionsPageLimit === value) {
            return
        }
        transactionsPageLimit = value
        refreshTransactionsPage(transactionsPageBeforeBlock > 0 ? transactionsPageBeforeBlock : null)
    }

    function transactionRowsFromBlocks(blocks) {
        const rows = []
        const sorted = sortedBlockchainBlocks(blocks)
        for (let i = 0; i < sorted.length; ++i) {
            const block = sorted[i]
            const header = block.header || {}
            const transactions = Array.isArray(block.transactions) ? block.transactions : []
            for (let j = 0; j < transactions.length; ++j) {
                const tx = transactions[j]
                const ops = transactionOps(tx)
                rows.push({
                    slot: header.slot || 0,
                    hash: transactionHash(tx),
                    block: header.id || header.hash || "",
                    index: j,
                    ops: ops.length,
                    operations: ops.map(function (op, index) {
                        return operationSummary(op, tx, index)
                    }),
                    raw: tx
                })
            }
        }
        return rows
    }

    function sortedBlockchainBlocks(blocks) {
        const copy = Array.isArray(blocks) ? blocks.slice(0) : []
        copy.sort(function (left, right) {
            return Number(right.header ? (right.header.slot || 0) : 0) - Number(left.header ? (left.header.slot || 0) : 0)
        })
        return copy
    }

    function transactionHash(tx) {
        const mantle = tx && tx.mantle_tx ? tx.mantle_tx : tx
        return String((mantle && mantle.hash) || (tx && tx.hash) || "")
    }

    function transactionOps(tx) {
        const mantle = tx && tx.mantle_tx ? tx.mantle_tx : tx
        return mantle && Array.isArray(mantle.ops) ? mantle.ops : []
    }

    function operationSummary(op, tx, index) {
        const opcode = Number(op && op.opcode !== undefined ? op.opcode : -1)
        const payload = op && op.payload ? op.payload : {}
        const proofs = tx && tx.ops_proofs ? tx.ops_proofs : []
        return {
            index: index,
            opcode: opcode,
            opcode_hex: byteHex(opcode),
            opcode_name: operationName(opcode),
            channel: String(payload.channel_id || payload.channelId || payload.channel || ""),
            signer: String(payload.signer || ""),
            parent: String(payload.parent || payload.parent_id || payload.parentId || ""),
            payload: payload,
            proof: Array.isArray(proofs) && proofs.length > index ? proofs[index] : null
        }
    }

    function byteHex(value) {
        const number = Number(value)
        if (number < 0 || !Number.isFinite(number)) {
            return "-"
        }
        const hex = number.toString(16)
        return "0x" + (hex.length < 2 ? "0" + hex : hex)
    }

    function operationName(opcode) {
        if (opcode === 0) {
            return "Transfer"
        }
        if (opcode === 16) {
            return "ChannelConfig"
        }
        if (opcode === 17) {
            return "ChannelInscribe"
        }
        if (opcode === 18) {
            return "ChannelDeposit"
        }
        if (opcode === 19) {
            return "ChannelWithdraw"
        }
        if (opcode === 32) {
            return "SDPDeclare"
        }
        if (opcode === 33) {
            return "SDPWithdraw"
        }
        if (opcode === 34) {
            return "SDPActive"
        }
        if (opcode === 48) {
            return "LeaderClaim"
        }
        return qsTr("Unknown")
    }

    function refreshTransferActivityPage(beforeBlock, preserveHistory) {
        const before = beforeBlock === undefined || beforeBlock === null ? null : beforeBlock
        if (!preserveHistory) {
            transferActivityHistory = []
        }
        const recipients = requestModule(inspectorModule, "indexerTransferRecipients", [indexerUrl, before, transferActivityBlockBatch], qsTr("Transfer activity"), false)
        if (!recipients.ok) {
            transferActivityError = recipients.error
            setResult(qsTr("Transfer activity"), transferActivityError, true)
            return
        }

        transferActivityBeforeBlock = before || 0
        transferActivityRows = (recipients.value || []).slice(0, transferActivityLimit)
        transferActivityNextBeforeBlock = nextTransferActivityBlock(transferActivityRows)
        transferActivityError = ""
        setResult(qsTr("Transfer activity"), BridgeHelpers.formatValue(transferActivityRows), false, transferActivityRows)
    }

    function nextTransferActivityPage() {
        const history = Array.isArray(transferActivityHistory) ? transferActivityHistory.slice(0) : []
        history.push(transferActivityBeforeBlock)
        transferActivityHistory = history
        refreshTransferActivityPage(transferActivityNextBeforeBlock, true)
    }

    function previousTransferActivityPage() {
        const history = Array.isArray(transferActivityHistory) ? transferActivityHistory.slice(0) : []
        if (!history.length) {
            return
        }
        const before = history.pop()
        transferActivityHistory = history
        refreshTransferActivityPage(before || null, true)
    }

    function setTransferActivityPageLimit(limit) {
        const value = Math.max(1, Number(limit || transferActivityLimit))
        if (transferActivityLimit === value) {
            return
        }
        transferActivityLimit = value
        refreshTransferActivityPage(transferActivityBeforeBlock || null, true)
    }

    function nextTransferActivityBlock(recipients) {
        const rows = Array.isArray(recipients) ? recipients : []
        let next = 0
        for (let i = 0; i < rows.length; ++i) {
            const slot = Number(rows[i].last_slot || 0)
            if (slot > 0 && (next === 0 || slot < next)) {
                next = slot
            }
        }
        return next
    }

    function transferRecipientDetail(row) {
        const recipient = row || {}
        return {
            type: "transfer_recipient",
            address: String(recipient.recipient || recipient.address || ""),
            total_received: recipient.received,
            txs: recipient.txs || 0,
            outputs: recipient.outputs || 0,
            last_slot: recipient.last_slot,
            source: String(recipient.source || ""),
            transfers: Array.isArray(recipient.transfers) ? recipient.transfers : [],
            raw: recipient
        }
    }

    function transferRecipientDetailById(value) {
        const wanted = normalizedHashOrValue(value)
        if (!wanted.length) {
            return null
        }
        const rows = transferActivityRows || []
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i]
            if (normalizedHashOrValue(row.recipient || row.address) === wanted) {
                return transferRecipientDetail(row)
            }
        }
        return null
    }

    function refreshChannelsPage(anchorSlot) {
        const node = requestModule(inspectorModule, "blockchainNode", [nodeUrl], qsTr("Channels node state"), false)
        if (!node.ok) {
            channelsPageError = node.error
            setResult(qsTr("Channels"), channelsPageError, true)
            return
        }

        const infoProbe = node.value ? node.value.cryptarchia_info : null
        const info = infoProbe && infoProbe.value ? infoProbe.value.cryptarchia_info : null
        const fallbackSlot = info ? (info.slot || info.lib_slot || 0) : 0
        const requestedSlot = Math.max(0, Number(anchorSlot === undefined || anchorSlot === null ? fallbackSlot : anchorSlot))
        const slotTo = fallbackSlot > 0 ? Math.min(requestedSlot, Number(fallbackSlot)) : requestedSlot
        const slotFrom = Math.max(0, slotTo - channelsPageWindow)
        const report = requestModule(inspectorModule, "channelScan", [nodeUrl, slotFrom, slotTo], qsTr("Channels"), false)
        if (!report.ok) {
            channelsPageError = report.error
            setResult(qsTr("Channels"), channelsPageError, true)
            return
        }

        channelsPageSlotFrom = slotFrom
        channelsPageSlotTo = slotTo
        channelsPageRows = ((report.value || {}).summaries || []).slice(0, channelsPageLimit)
        channelsPageError = ""
        setResult(qsTr("Channels"), BridgeHelpers.formatValue(report.value || {}), false, report.value || {})
    }

    function olderChannelsPage() {
        refreshChannelsPage(Math.max(0, channelsPageSlotFrom - 1))
    }

    function newerChannelsPage() {
        refreshChannelsPage(channelsPageSlotTo + channelsPageWindow + 1)
    }

    function setChannelsPageLimit(limit) {
        const value = Math.max(1, Number(limit || channelsPageLimit))
        if (channelsPageLimit === value) {
            return
        }
        channelsPageLimit = value
        refreshChannelsPage(channelsPageSlotTo > 0 ? channelsPageSlotTo : null)
    }

    function channelDetail(row) {
        const channel = row || {}
        const channelId = String(channel.channel || channel.channel_id || "")
        const lastTxHash = String(channel.last_tx_hash || channel.tx_hash || "")
        const lastBlockHash = String(channel.last_block_hash || channel.header || channel.block_hash || "")
        return {
            type: "channel",
            channel: channelId,
            channel_id: channelId,
            operation_type: String(channel.operation_type || channel.last_operation_type || ""),
            l1_slot: channel.last_slot || channel.l1_slot,
            header: lastBlockHash,
            l1_header_hash: lastBlockHash,
            tx_hash: lastTxHash,
            transaction_hash: lastTxHash,
            parent: String(channel.parent || channel.parent_hash || ""),
            signer: String(channel.signer || channel.author || ""),
            source_confidence: String(channel.source_confidence || channel.source || "scan"),
            label: channel.label,
            first_slot: channel.first_slot,
            first_tx_hash: channel.first_tx_hash,
            first_block_hash: channel.first_block_hash,
            last_slot: channel.last_slot,
            last_tx_hash: lastTxHash,
            last_block_hash: lastBlockHash,
            tip: channel.tip,
            balance: channel.balance,
            withdraw_threshold: channel.withdraw_threshold,
            keys: channel.keys,
            key_values: Array.isArray(channel.key_values) ? channel.key_values : [],
            operations: channel.operations || 0,
            raw_json: channel.raw || channel,
            raw: channel
        }
    }

    function channelDetailById(value) {
        const wanted = normalizedHashOrValue(value)
        if (!wanted.length) {
            return null
        }
        const rows = channelsPageRows || []
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i]
            if (normalizedHashOrValue(row.channel || row.channel_id) === wanted) {
                return channelDetail(row)
            }
        }
        return null
    }

    function refreshDashboard() {
        const refreshId = dashboardRefreshSerial + 1
        dashboardRefreshSerial = refreshId
        dashboardRefreshing = true
        dashboardError = ""
        const requests = [
            { module: inspectorModule, method: "overview", args: [sequencerUrl, indexerUrl, nodeUrl], label: qsTr("Dashboard overview") },
            { module: inspectorModule, method: "blockchainNode", args: [nodeUrl], label: qsTr("Blockchain node") },
            { module: inspectorModule, method: "indexerBlocks", args: [indexerUrl, null, 10], label: qsTr("Latest blocks") },
            { module: inspectorModule, method: "storageSourceReport", args: root.storageSourceReportArgs(), label: qsTr("Storage source") },
            { module: inspectorModule, method: "deliverySourceReport", args: root.deliverySourceReportArgs(), label: qsTr("Delivery source") }
        ]
        const errors = []
        let remaining = requests.length
        let okCount = 0

        for (let i = 0; i < requests.length; ++i) {
            const request = requests[i]
            requestModuleAsync(request.module, request.method, request.args, request.label, false, function (response) {
                if (refreshId !== dashboardRefreshSerial) {
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
                            blocks: dashboardBlocks || [],
                            storage: storageModuleReport || null,
                            messaging: messagingModuleReport || null
                        }), false)
                    } else {
                        setResult(qsTr("Dashboard"), dashboardError, true)
                    }
                }
            })
        }
    }

    function updateDashboardCache(method, value) {
        if (method === "overview") {
            dashboardOverview = value
        } else if (method === "blockchainNode") {
            dashboardNode = value
        } else if (method === "indexerBlocks") {
            dashboardBlocks = value || []
        } else if (method === "account" || method === "decodeAccount") {
            accountDetailValue = value || null
        } else if (method === "storageReport" || method === "storageSourceReport") {
            storageModuleReport = value || null
        } else if (method === "deliveryReport" || method === "deliverySourceReport") {
            messagingModuleReport = value || null
        }
    }

    function routeSearch(query) {
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
            const recipient = transferRecipientDetailById(value)
            if (recipient) {
                openRecipient(value)
                return
            }
            const channel = channelDetailById(value)
            if (channel) {
                openChannel(value)
                return
            }
            resolveSearchHash(value)
            return
        }

        const recipient = transferRecipientDetailById(value)
        if (recipient) {
            openRecipient(value)
            return
        }

        const channel = channelDetailById(value)
        if (channel) {
            openChannel(value)
            return
        }

        openAccount(value)
    }

    function numericSearchUsesLezBlock() {
        const view = String(currentView || "")
        if (root.layerForView(view) === "l2") {
            return true
        }
        return view === "sequencer" || view === "accounts" || view === "programs"
            || view === "transferActivity" || view === "indexer"
    }

    function routePrefixedSearch(query) {
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
                sequencerTab = "blocks"
                selectView("sequencer")
            }
            return true
        }
        if (prefix === "tx" || prefix === "transaction") {
            if (target.length > 0) {
                openLezTransaction(target)
            } else {
                sequencerTab = "transactions"
                selectView("sequencer")
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
            openLocalWallet(target, "profiles")
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

    function searchPrefix(query) {
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

    function isSearchPrefix(prefix) {
        const value = String(prefix || "").toLowerCase()
        return value === "l1" || value === "slot" || value === "bedrock" || value === "cryptarchia"
            || value === "mantle" || value === "channel" || value === "l2" || value === "lez"
            || value === "block" || value === "tx" || value === "transaction" || value === "account"
            || value === "public" || value === "private" || value === "program" || value === "wallet"
            || value === "l1-wallet" || value === "note" || value === "recipient" || value === "module"
    }

    function routeModuleSearchTarget(target) {
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

    function resolveSearchHash(hash) {
        const value = String(hash || "").trim()
        if (!value.length) {
            return
        }

        const serial = searchResolveSerial + 1
        searchResolveSerial = serial
        statusText = qsTr("Search")
        requestModuleAsync(inspectorModule, "indexerBlockByHash", [indexerUrl, value], qsTr("Block lookup"), false, function (response) {
            if (serial !== searchResolveSerial) {
                return
            }
            if (response.ok && response.value !== null && response.value !== undefined) {
                sequencerTab = "blocks"
                currentView = "sequencer"
                setResult(qsTr("LEZ block"), response.text, false, response.value)
                return
            }
            root.resolveSearchTransaction(serial, value)
        })
    }

    function resolveSearchTransaction(serial, hash) {
        requestModuleAsync(inspectorModule, "inspectTransaction", [sequencerUrl, hash], qsTr("Transaction inspection"), false, function (response) {
            if (serial !== searchResolveSerial) {
                return
            }
            if (response.ok && response.value !== null && response.value !== undefined) {
                sequencerTab = "transactions"
                currentView = "sequencer"
                transactionDetailValue = response.value
                transactionsPageError = ""
                setResult(qsTr("LEZ transaction"), response.text, false, response.value)
                root.autoDecodeTransactionDetail(response.value)
                return
            }
            root.resolveSearchAccount(serial, hash)
        })
    }

    function resolveSearchAccount(serial, account) {
        requestModuleAsync(inspectorModule, "account", [sequencerUrl, indexerUrl, account], qsTr("Account lookup"), false, function (response) {
            if (serial !== searchResolveSerial) {
                return
            }
            currentView = "accounts"
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

    function viewKeyForQuery(query) {
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
        if (normalized === "l2" || normalized === "lez" || normalized === "sequencer") {
            return "sequencer"
        }
        if (normalized === "indexer") {
            return "indexer"
        }
        if (normalized === "chain" || normalized === "base chain" || normalized === "node" || normalized === "consensus") {
            return "blockchain"
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

    function settingsTargetForQuery(query) {
        const normalized = String(query || "").trim().toLowerCase()
        if (!normalized.length) {
            return { section: "", subsection: "" }
        }
        if (normalized === "network") {
            return { section: "network", subsection: settingsNetworkSection }
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

    function openReference(kind, value, payload) {
        const target = String(value || "").trim()
        if (!target.length && payload === undefined) {
            return
        }

        switch (kind) {
        case "block":
        case "blockHash":
        case "blockNumber":
        case "slot":
            openBlockchainBlock(payload === undefined ? target : payload)
            return
        case "indexerBlock":
            openIndexerBlock(target)
            return
        case "lezBlock":
            openLezBlock(target)
            return
        case "transaction":
        case "transactionHash":
        case "tx":
            openTransaction(target)
            return
        case "mantleTransaction":
            openMantleTransaction(target)
            return
        case "wallet":
            openLocalWallet(target, "profiles")
            return
        case "private":
        case "privateAccount":
            openPrivateAccountReference(target)
            return
        case "bedrockWallet":
        case "note":
            openLocalWallet(target, "bedrockNotes")
            return
        case "recipient":
        case "transferRecipient":
            openRecipient(target)
            return
        case "channel":
            openChannel(payload === undefined ? target : payload)
            return
        case "account":
        case "signer":
            openAccount(target)
            return
        case "program":
            openProgram(target)
            return
        default:
            routeSearch(target)
        }
    }

    function openMantleTransaction(hash) {
        const value = String(hash || "").trim()
        if (!value.length) {
            return
        }

        const detail = transactionDetail(value)
        currentView = "transactions"
        if (detail) {
            transactionDetailValue = detail
            transactionsPageError = ""
            setResult(qsTr("Mantle transaction"), BridgeHelpers.formatValue(detail), false, detail)
            return
        }

        transactionDetailValue = null
        transactionsPageError = qsTr("Mantle transaction %1 is not in the loaded L1 slot window.").arg(value)
        setResult(qsTr("Mantle transaction"), transactionsPageError, true)
    }

    function openAccount(account) {
        const value = String(account || "").trim()
        if (!value.length) {
            return
        }
        const serial = searchResolveSerial + 1
        searchResolveSerial = serial
        currentView = "accounts"
        accountTab = "lookup"
        statusText = qsTr("Account lookup")
        requestModuleAsync(inspectorModule, "account", [sequencerUrl, indexerUrl, value], qsTr("Account lookup"), false, function (response) {
            if (serial !== searchResolveSerial) {
                return
            }
            if (response.ok) {
                accountDetailValue = response.value || null
                setResult(qsTr("Account lookup"), response.text, false, response.value)
            } else {
                accountDetailValue = null
                setResult(qsTr("Account lookup"), response.error, true, null)
            }
        })
    }

    function openPrivateAccountReference(account) {
        const value = String(account || "").trim()
        currentView = "accounts"
        accountTab = "lookup"
        accountDetailValue = {
            type: "private_account_reference",
            account_id: value.length && value.indexOf("Private/") !== 0 ? "Private/" + value : value,
            source: "local_wallet_required"
        }
        setResult(qsTr("Private account reference"), qsTr("Private account state is local wallet state. Public RPC can only expose public effects, commitments, nullifiers, or proofs when available."), false, accountDetailValue)
    }

    function openTransaction(hash) {
        openLezTransaction(hash)
    }

    function openLezSearchTarget(target) {
        const value = String(target || "").trim()
        if (!value.length) {
            return
        }
        if (/^[0-9]+$/.test(value)) {
            openLezBlock(value)
            return
        }
        resolveLezHash(value)
    }

    function openLezBlock(blockId) {
        const value = String(blockId || "").trim()
        if (!value.length) {
            return
        }

        const serial = searchResolveSerial + 1
        searchResolveSerial = serial
        sequencerTab = "blocks"
        currentView = "sequencer"
        statusText = qsTr("LEZ block lookup")
        requestModuleAsync(inspectorModule, "block", [sequencerUrl, value], qsTr("LEZ block"), false, function (response) {
            if (serial !== searchResolveSerial) {
                return
            }
            if (response.ok) {
                setResult(qsTr("LEZ block"), response.text, false, response.value)
            } else {
                setResult(qsTr("LEZ block"), response.error, true)
            }
        })
    }

    function resolveLezHash(hash) {
        const value = String(hash || "").trim()
        if (!value.length) {
            return
        }

        const serial = searchResolveSerial + 1
        searchResolveSerial = serial
        sequencerTab = "blocks"
        currentView = "sequencer"
        statusText = qsTr("L2 lookup")
        requestModuleAsync(inspectorModule, "indexerBlockByHash", [indexerUrl, value], qsTr("LEZ block lookup"), false, function (response) {
            if (serial !== searchResolveSerial) {
                return
            }
            if (response.ok && response.value !== null && response.value !== undefined) {
                const detail = root.indexerBlockDetail(response.value)
                setResult(qsTr("LEZ block"), BridgeHelpers.formatValue(detail), false, detail)
                return
            }
            root.openLezTransaction(value)
        })
    }

    function openLezTransaction(hash) {
        const value = String(hash || "").trim()
        if (!value.length) {
            return
        }

        searchResolveSerial += 1
        sequencerTab = "transactions"
        currentView = "sequencer"
        inspectTransaction(value, "")
    }

    function inspectTransaction(hash, idl) {
        const value = String(hash || "").trim()
        if (!value.length) {
            return
        }

        sequencerTab = "transactions"
        currentView = "sequencer"
        const trimmedIdl = String(idl || "").trim()
        const args = trimmedIdl.length ? [sequencerUrl, value, trimmedIdl] : [sequencerUrl, value]
        const serial = transactionAutoDecodeSerial + 1
        transactionAutoDecodeSerial = serial
        requestModuleAsync(inspectorModule, "inspectTransaction", args, qsTr("Transaction inspection"), true, function (response) {
            if (serial !== transactionAutoDecodeSerial) {
                return
            }
            if (response.ok) {
                transactionDetailValue = response.value
                transactionsPageError = ""
                setResult(qsTr("Transaction"), response.text, false, response.value)
                if (!trimmedIdl.length) {
                    root.autoDecodeTransactionDetail(response.value)
                }
            } else {
                transactionDetailValue = null
                transactionsPageError = response.error
                setResult(qsTr("Transaction"), response.error, true)
            }
        })
    }

    function openBlockchainBlock(blockOrId) {
        let detail = null
        if (blockOrId && typeof blockOrId === "object") {
            detail = blockchainBlockDetail(blockOrId)
        } else {
            detail = blockchainBlockDetailById(blockOrId)
        }
        if (!detail) {
            const fallback = blockOrId && typeof blockOrId === "object" ? blockHash(blockOrId) : blockOrId
            if (/^[0-9]+$/.test(String(fallback || "").trim())) {
                loadBlockchainBlockBySlot(Number(fallback))
                return
            }
            loadBlockchainBlockById(String(fallback || ""))
            return
        }

        currentView = "blocks"
        blockDetailValue = detail
        setResult(qsTr("Block"), BridgeHelpers.formatValue(detail), false, detail)
    }

    function loadBlockchainBlockById(blockId) {
        const value = String(blockId || "").trim()
        if (!value.length) {
            return
        }
        currentView = "blocks"
        const response = requestModule(inspectorModule, "blockchainBlock", [nodeUrl, value], qsTr("Block lookup"), false)
        if (response.ok) {
            blockDetailValue = blockchainBlockDetail(response.value)
            blocksPageError = ""
            setResult(qsTr("Block"), BridgeHelpers.formatValue(blockDetailValue), false, blockDetailValue)
            return
        }
        const normalized = normalizedHashOrValue(value)
        const retryValue = normalized !== value ? normalized : ""
        if (retryValue.length) {
            const retry = requestModule(inspectorModule, "blockchainBlock", [nodeUrl, retryValue], qsTr("Block lookup"), false)
            if (retry.ok) {
                blockDetailValue = blockchainBlockDetail(retry.value)
                blocksPageError = ""
                setResult(qsTr("Block"), BridgeHelpers.formatValue(blockDetailValue), false, blockDetailValue)
                return
            }
        }
        currentView = "blocks"
        blocksPageError = qsTr("L1 block %1 was not found.").arg(value)
        setResult(qsTr("Block"), blocksPageError, true)
    }

    function loadBlockchainBlockBySlot(slot) {
        const value = Math.max(0, Number(slot || 0))
        currentView = "blocks"
        const response = requestModule(inspectorModule, "blockchainBlocks", [nodeUrl, value, value], qsTr("Block lookup"), false)
        if (response.ok) {
            const blocks = Array.isArray(response.value) ? response.value : []
            if (blocks.length > 0) {
                blockDetailValue = blockchainBlockDetail(blocks[0])
                setResult(qsTr("Block"), BridgeHelpers.formatValue(blockDetailValue), false, blockDetailValue)
                return
            }
            blocksPageError = qsTr("No block found at slot %1.").arg(value)
            setResult(qsTr("Block"), blocksPageError, true)
        } else {
            blocksPageError = response.error
            setResult(qsTr("Block"), response.error, true)
        }
    }

    function openBlockchainTransaction(transaction, block) {
        const tx = transaction || {}
        const parentBlock = block || {}
        const detail = {
            type: "blockchain_transaction",
            hash: String(tx.hash || ""),
            block: String(parentBlock.hash || ""),
            slot: parentBlock.slot,
            index: tx.index,
            ops: Array.isArray(tx.operations) ? tx.operations : [],
            raw: tx.raw || null
        }
        currentView = "transactions"
        transactionDetailValue = detail
        setResult(qsTr("Transaction"), BridgeHelpers.formatValue(detail), false, detail)
    }

    function transactionDetail(hash) {
        const normalized = normalizedHashOrValue(hash)
        const rows = transactionsPageRows || []
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i]
            if (normalizedHashOrValue(row.hash) === normalized) {
                return {
                    type: "blockchain_transaction",
                    hash: row.hash,
                    block: row.block,
                    slot: row.slot,
                    index: row.index,
                    ops: row.operations || [],
                    raw: row.raw
                }
            }
        }
        return null
    }

    function openIndexerBlock(headerHash) {
        const value = String(headerHash || "").trim()
        if (!value.length) {
            return
        }

        sequencerTab = "blocks"
        currentView = "sequencer"

        const response = requestModule(inspectorModule, "indexerBlockByHash", [indexerUrl, value], qsTr("Block lookup"), false)
        if (response.ok) {
            if (response.value === null || response.value === undefined) {
                blocksPageError = qsTr("No block found for %1.").arg(value)
                setResult(qsTr("LEZ block"), blocksPageError, true)
                return
            }
            blocksPageError = ""
            const detail = root.indexerBlockDetail(response.value)
            setResult(qsTr("LEZ block"), BridgeHelpers.formatValue(detail), false, detail)
        } else {
            blocksPageError = response.error
            setResult(qsTr("LEZ block"), response.error, true)
        }
    }

    function indexerBlockDetail(value) {
        const block = value || {}
        const transactions = Array.isArray(block.transactions) ? block.transactions : []
        return {
            type: "indexer_block",
            hash: String(block.header_hash || ""),
            parent: String(block.parent_hash || ""),
            block_id: block.block_id,
            slot: block.block_id,
            height: block.block_id,
            status: String(block.bedrock_status || ""),
            version: "",
            block_root: "",
            voucher_cm: "",
            entropy: "",
            signature: "",
            leader_key: "",
            transactions: transactions.map(function (tx, index) {
                return {
                    index: tx.index !== undefined ? tx.index : index,
                    hash: String(tx.hash || ""),
                    ops: Array.isArray(tx.instruction_data) ? tx.instruction_data.length : 0,
                    operations: [],
                    raw: tx.raw || tx
                }
            }),
            raw: block.raw || block
        }
    }

    function openLocalWallet(wallet, tab) {
        const target = String(wallet || "").trim()
        const targetTab = String(tab || "").length ? String(tab || "") : "profiles"
        if (!walletProfileConfigured()) {
            setResult(
                qsTr("Local wallet"),
                qsTr("Configure an explicit local wallet profile. Transfer recipients use recipient:<id>; wallet:<id> is reserved for local wallet state."),
                true,
                null
            )
            return
        }
        const profileStatus = checkedLocalWalletProfile()
        if (!profileStatus.ok) {
            setResult(
                qsTr("Local wallet"),
                profileStatus.detail.length ? profileStatus.detail : qsTr("Local wallet profile is not usable."),
                true,
                localWalletStatus
            )
            return
        }
        currentView = "localWallet"
        localWalletTab = targetTab
        localWalletLookupTarget = target
        transferRecipientDetailValue = null
        if (localWalletTab === "bedrockNotes" && target.length > 0 && walletPublicKeyProbe.length === 0) {
            walletPublicKeyProbe = target
        }
        setResult(
            qsTr("Local wallet"),
            target.length ? qsTr("Local wallet context: %1").arg(target) : qsTr("Local wallet profile configured."),
            false,
            walletProfile()
        )
    }

    function showLocalWalletRequired(wallet) {
        openLocalWallet(wallet, "profiles")
    }

    function openProgram(programId) {
        const value = String(programId || "").trim()
        if (!value.length) {
            selectView("programs")
            return
        }
        currentView = "programs"
        programTab = "programIds"
        const detail = {
            type: "program",
            program_id: value,
            source: "search"
        }
        setResult(qsTr("Program"), BridgeHelpers.formatValue(detail), false, detail)
    }

    function openRecipient(recipient) {
        const value = String(recipient || "").trim()
        if (!value.length) {
            return
        }

        const detail = transferRecipientDetailById(value)
        if (detail) {
            currentView = "transferActivity"
            transferRecipientDetailValue = detail
            setResult(qsTr("Transfer recipient"), BridgeHelpers.formatValue(detail), false, detail)
            return
        }
        currentView = "transferActivity"
        transferRecipientDetailValue = null
        setResult(qsTr("Transfer recipient"), qsTr("No transfer recipient found for %1 in the loaded finalized L2 block window.").arg(value), true, null)
    }

    function openChannel(channel) {
        const detail = typeof channel === "object" ? channelDetail(channel) : channelDetailById(channel)
        if (detail) {
            currentView = "channels"
            channelDetailValue = detail
            setResult(qsTr("Channel"), BridgeHelpers.formatValue(detail), false, detail)
            return
        }

        const value = { type: "channel", channel: String(channel || "") }
        currentView = "channels"
        channelDetailValue = value
        setResult(qsTr("Channel"), BridgeHelpers.formatValue(value), false, value)
    }

    function registerIdl(name, programId, json) {
        if (!json.trim().length) {
            setResult(qsTr("IDL registry"), qsTr("IDL JSON is required."), true)
            return
        }

        const parsed = BridgeHelpers.parseJson(json)
        if (!parsed.ok) {
            setResult(qsTr("IDL registry"), qsTr("Invalid IDL JSON: %1").arg(parsed.error), true)
            return
        }

        const idl = parsed.value
        const resolvedName = name.trim().length ? name.trim() : (idl.name || qsTr("IDL %1").arg(registeredIdls.count + 1))
        const resolvedProgramId = programId.trim()
        const resolvedProgramIdHex = resolvedProgramId.length ? root.canonicalProgramIdHex(resolvedProgramId) : ""
        if (resolvedProgramId.length && !resolvedProgramIdHex.length) {
            setResult(qsTr("IDL registry"), qsTr("Program ID must be hex or base58."), true)
            return
        }
        registeredIdls.append({
            key: idlKey(resolvedName, resolvedProgramIdHex, json),
            name: resolvedName,
            programId: resolvedProgramId,
            programIdHex: resolvedProgramIdHex,
            json: json
        })
        saveIdlState()
        if (transactionDetailValue !== null) {
            autoDecodeTransactionDetail(transactionDetailValue)
        }
        setResult(qsTr("IDL registry"), qsTr("Saved %1.").arg(resolvedName), false)
    }

    function removeIdl(index) {
        if (index < 0 || index >= registeredIdls.count) {
            return
        }
        const entry = idlEntryAt(index)
        registeredIdls.remove(index)
        if (entry.key.length) {
            const next = {}
            const current = accountIdlSelections || {}
            for (const accountId in current) {
                if (String(current[accountId].idlKey || "") !== entry.key) {
                    next[accountId] = current[accountId]
                }
            }
            accountIdlSelections = next
            accountIdlSelectionRevision += 1
        }
        saveIdlState()
    }

    function profileIndex() {
        if (networkProfile === "local") {
            return 1
        }
        if (networkProfile === "custom") {
            return 2
        }
        return 0
    }

    function applyProfile(index) {
        if (index === 1) {
            networkProfile = "local"
            sequencerUrl = "http://127.0.0.1:3040/"
            indexerUrl = "http://127.0.0.1:8779/"
            nodeUrl = "http://127.0.0.1:8080/"
            return
        }

        networkProfile = "default"
        sequencerUrl = "https://testnet.lez.logos.co/"
        indexerUrl = "http://127.0.0.1:8779/"
        nodeUrl = "http://127.0.0.1:8080/"
        messagingNetworkPreset = "logos.test"
    }
}
