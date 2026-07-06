import QtQuick
import QtQml.Models
import "../services"
import "appmodel/AppModelCore.js" as AppModelCore
import "appmodel/AppModelIdentity.js" as AppModelIdentity
import "appmodel/AppModelNetwork.js" as AppModelNetwork
import "appmodel/AppModelMetrics.js" as AppModelMetrics
import "appmodel/AppModelPages.js" as AppModelPages
import "appmodel/AppModelSearch.js" as AppModelSearch
import "appmodel/AppModelOpeners.js" as AppModelOpeners
import "appmodel/AppModelRegistry.js" as AppModelRegistry
import "appmodel/AppModelSocial.js" as AppModelSocial

QtObject {
    id: root

    required property BridgeClient bridge

    readonly property string inspectorModule: "logos_inspector"
    readonly property string blockchainModule: "blockchain_module"
    readonly property string indexerModule: "lez_indexer_module"
    readonly property string storageModule: "storage_module"
    readonly property string deliveryModule: "delivery_module"
    readonly property string capabilityModule: "capability_module"
    property var sourcePolicy: ({})
    property bool sourcePolicyLoaded: false

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
    property var dashboardL1Blocks: []
    property var dashboardBlocks: []
    property var dashboardSequencerBlocks: []
    property string dashboardError: ""
    property var blockDetailValue: null
    property string blockDetailError: ""
    property var transactionDetailValue: null
    property string transactionDetailError: ""
    property var accountDetailValue: null
    property var transferRecipientDetailValue: null
    property var channelDetailValue: null
    property string channelDetailError: ""
    property var blocksPageRows: []
    property int blocksPageSlotFrom: 0
    property int blocksPageSlotTo: 0
    property int blocksPageWindow: 2000
    property int blocksPageLimit: 20
    property string blocksPageError: ""
    property bool blocksLiveEnabled: false
    property string blocksLiveError: ""
    property string blocksLiveSource: ""
    property int blocksLiveUnknownEvents: 0
    property string blocksLiveCheckedAt: ""
    property var transactionsPageRows: []
    property int transactionsPageBeforeBlock: 0
    property int transactionsPageNextBeforeBlock: 0
    property int transactionsPageBlockBatch: 1000
    property int transactionsPageLimit: 20
    property string transactionsPageError: ""
    property var lezBlocksPageRows: []
    property int lezBlocksPageBeforeBlock: 0
    property int lezBlocksPageNextBeforeBlock: 0
    property int lezBlocksPageLimit: 20
    property string lezBlocksPageError: ""
    property bool lezBlocksPageLoading: false
    property int lezBlocksPageRequestSerial: 0
    property var lezTransactionsPageRows: []
    property int lezTransactionsPageBeforeBlock: 0
    property int lezTransactionsPageNextBeforeBlock: 0
    property var lezTransactionsPageOverflowRows: []
    property int lezTransactionsPageOverflowNextBeforeBlock: 0
    property int lezTransactionsBlockBatch: 50
    property int lezTransactionsPageLimit: 20
    property string lezTransactionsPageError: ""
    property var transferActivityRows: []
    property int transferActivityBeforeBlock: 0
    property int transferActivityNextBeforeBlock: 0
    property var transferActivityOverflowRows: []
    property int transferActivityOverflowNextBeforeBlock: 0
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
    property string blockchainSourceMode: "auto"
    property string indexerSourceMode: "auto"
    property string executionSourceMode: "rpc"
    property string messagingSourceMode: "auto"
    property string messagingRestUrl: "http://127.0.0.1:8645"
    property string messagingMetricsUrl: "http://127.0.0.1:8008/metrics"
    property string messagingNetworkPreset: "logos.test"
    property int messagingRollingWindow: 120
    property bool messagingAdminRestEnabled: false
    property bool messagingMutatingDiagnosticsEnabled: false
    property int socialCommentPageSize: 20
    property string socialIdentityDefaultMode: "perConversation"
    property string selectedSocialIdentityKey: ""
    property var socialConversationIdentityKeys: ({})
    property int socialIdentityRevision: 0
    property var socialCommentState: ({})
    property int socialCommentRevision: 0
    property var socialSharedIdls: ({})
    property string sharedIdlPolicy: "suggestion"
    property bool sharedIdlAutoShare: false
    property var socialAutoSharedIdls: ({})
    property int sharedIdlRevision: 0
    property string storageSourceMode: "auto"
    property string storageRestUrl: "http://127.0.0.1:8080/api/storage/v1"
    property string storageMetricsUrl: "http://127.0.0.1:8008/metrics"
    property string storageNetworkPreset: "logos.test"
    property string storageDataDir: ""
    property int storageRollingWindow: 120
    property bool storageLocalDiagnosticsEnabled: false
    property bool storagePrivilegedDebugEnabled: false
    property bool storageMutatingDiagnosticsEnabled: false
    property var nodeOperations: ({})
    property var nodeOperationEventSeq: ({})
    property var nodeOperationHistory: []
    property int nodeOperationsRevision: 0
    property string settingsBackupCid: ""
    property string settingsRestoreCid: ""
    property bool settingsBackupEncrypted: false
    property string settingsBackupStatus: ""

    property string sequencerTab: "blocks"
    property string deliveryAppTab: "messages"
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
    property int networkConfigurationRevision: 0
    property var footerFieldSelections: defaultFooterFieldSelections()
    property int footerFieldRevision: 0
    property var dashboardGraphSelections: defaultDashboardGraphSelections()
    property int dashboardGraphRevision: 0
    property var dashboardMetricHistory: ({})
    property var dashboardMetricLastSeen: ({})
    property int dashboardMetricHistoryRevision: 0
    property var networkConnectionPending: ({})
    property int networkConnectionPendingRevision: 0
    property bool dashboardRefreshing: false
    property int dashboardRefreshSerial: 0
    property var blockchainModuleReport: null
    property var storageModuleReport: null
    property var messagingModuleReport: null

    property ListModel registeredIdls: ListModel {}
    property ListModel socialIdentities: ListModel {}
    property bool idlStateLoaded: false
    property bool walletStateLoaded: false
    property bool settingsStateLoaded: false
    property string settingsStateError: ""
    property string walletProfileLabel: "Local wallet"
    property string walletBinary: ""
    property string walletHome: ""
    property string walletPublicKeyProbe: ""
    property string walletCreatePrivacy: "public"
    property string walletCreateLabel: ""
    property string walletSendFrom: ""
    property string walletSendTo: ""
    property string walletSendToKeys: ""
    property string walletSendToNpk: ""
    property string walletSendToVpk: ""
    property string walletSendToIdentifier: ""
    property string walletSendAmount: ""
    property string walletAdvancedCommand: ""
    property string bedrockWalletBalanceTip: ""
    property var localWalletStatus: null
    property string localWalletStatusError: ""
    property var localWalletOperations: []
    property var localWalletAccountsValue: null
    property string localWalletAccountsError: ""
    property StorageAppState storageApp: StorageAppState {
        id: storageAppState

        gateway: QtObject {
            function call(method, args, label) {
                return root.callInspector(method, args, label)
            }

            function request(method, args, label, showResult, callback) {
                return root.requestModuleAsync(root.inspectorModule, method, args, label, showResult === true, callback)
            }

            function setResult(title, text, isError, value) {
                return root.setResult(title, text, isError, value)
            }

            function clearResult() {
                return root.clearResult()
            }

            function appendOperationHistory(operation, detail) {
                return root.appendNodeOperationHistory(operation, detail)
            }

            function openSettings(section, subSection) {
                return root.openSettings(section, subSection)
            }

            function valueText(value) {
                return root.valueText(value)
            }
        }
        busy: root.busy
        sourceMode: root.storageSourceMode
        effectiveSourceMode: root.effectiveStorageSourceMode(root.storageSourceMode)
        sourceLabel: root.storageSourceLabel()
        sourceTarget: root.storageSourceTarget()
        sourceTargetKind: root.sourceModeTargetKind("storage", root.storageSourceMode)
        usesRestEndpoint: root.sourceModeUsesEndpoint("storage", root.storageSourceMode, "rest")
        supportsMutatingDiagnostics: root.sourceModeSupportsMutatingDiagnostics("storage", root.storageSourceMode)
        restEndpoint: root.configuredStorageRestUrl()
        moduleName: root.storageModule
        networkPreset: root.storageNetworkPreset
        mutatingDiagnosticsEnabled: root.storageMutatingDiagnosticsEnabled
        currentView: root.currentView
        resultTitle: root.resultTitle
        resultText: root.resultText
        resultIsError: root.resultIsError
        resultOwner: root.resultOwner
    }
    property alias storageAppTab: storageAppState.currentTab
    property alias storageCidProbe: storageAppState.cidProbe
    property alias storageActiveOperation: storageAppState.activeOperation
    property alias storageActiveOperationRevision: storageAppState.activeOperationRevision
    property LocalNodesState localNodes: LocalNodesState {
        id: localNodesState

        gateway: QtObject {
            function request(method, args, label, showResult, callback) {
                return root.requestModuleAsync(root.inspectorModule, method, args, label, showResult, callback)
            }

            function setBusy(value, label) {
                root.busy = value === true
                const labelText = String(label || "")
                if (root.busy && labelText.length) {
                    root.statusText = labelText
                }
            }

            function setResult(title, text, isError, value) {
                return root.setResult(title, text, isError, value)
            }

            function appendOperationHistory(operation, detail) {
                return root.appendNodeOperationHistory(operation, detail)
            }
        }
        networkProfile: root.networkProfile
        busy: root.busy
    }
    property alias localNodesReport: localNodesState.report
    property alias localNodesError: localNodesState.error
    property alias localNodesOperations: localNodesState.operations
    property alias localNodesRevision: localNodesState.revision
    property alias localDevnets: localNodesState.devnets
    property var idlInstructionPreviewValue: null
    property string idlInstructionError: ""
    property var bedrockWalletBalanceValue: null
    property string bedrockWalletBalanceError: ""
    property string bedrockWalletModuleError: ""
    property var accountIdlSelections: ({})
    property int accountIdlSelectionRevision: 0
    property var knownProgramIds: ({})
    property int knownProgramIdsRevision: 0
    property int accountAutoDecodeSerial: 0
    property int transactionAutoDecodeSerial: 0
    property int searchResolveSerial: 0
    property int programOpenSerial: 0
    property var navExpanded: ({ l1: true, l2: true, network: true, diagnostics: false, local: true, system: true })
    property int navRevision: 0
    property var navigationBackStack: []
    property var navigationForwardStack: []
    property int navigationRevision: 0
    property bool navigationRestoring: false
    readonly property int navigationHistoryLimit: 80
    property FavoritesState favoriteStore: FavoritesState {
        onRevisionChanged: root.saveSettingsState()
        onOpenRequested: function (openKind, value) {
            root.openReference(openKind, value)
        }
    }

    onCurrentViewChanged: expandNavGroupForView(currentView)
    onNetworkProfileChanged: handleNetworkConfigurationChanged()
    onSequencerUrlChanged: handleNetworkConfigurationChanged()
    onIndexerUrlChanged: handleNetworkConfigurationChanged()
    onNodeUrlChanged: handleNetworkConfigurationChanged()
    onBlockchainSourceModeChanged: handleNetworkConfigurationChanged()
    onIndexerSourceModeChanged: handleNetworkConfigurationChanged()
    onExecutionSourceModeChanged: handleNetworkConfigurationChanged()
    onMessagingSourceModeChanged: handleMessagingConfigurationChanged()
    onMessagingRestUrlChanged: handleMessagingConfigurationChanged()
    onMessagingMetricsUrlChanged: handleMessagingConfigurationChanged()
    onMessagingNetworkPresetChanged: handleMessagingConfigurationChanged()
    onMessagingRollingWindowChanged: saveSettingsState()
    onMessagingAdminRestEnabledChanged: saveSettingsState()
    onMessagingMutatingDiagnosticsEnabledChanged: saveSettingsState()
    onSocialCommentPageSizeChanged: saveSettingsState()
    onSocialIdentityDefaultModeChanged: saveSettingsState()
    onSelectedSocialIdentityKeyChanged: saveSettingsState()
    onSharedIdlPolicyChanged: saveSettingsState()
    onSharedIdlAutoShareChanged: saveSettingsState()
    onStorageSourceModeChanged: handleStorageConfigurationChanged()
    onStorageRestUrlChanged: handleStorageConfigurationChanged()
    onStorageMetricsUrlChanged: handleStorageConfigurationChanged()
    onStorageNetworkPresetChanged: handleStorageConfigurationChanged()
    onStorageDataDirChanged: handleStorageConfigurationChanged()
    onStorageCidProbeChanged: saveSettingsState()
    onStorageRollingWindowChanged: saveSettingsState()
    onStorageLocalDiagnosticsEnabledChanged: handleStorageConfigurationChanged()
    onStoragePrivilegedDebugEnabledChanged: handleStorageConfigurationChanged()
    onStorageMutatingDiagnosticsEnabledChanged: saveSettingsState()
    onSettingsBackupCidChanged: saveSettingsState()
    onSettingsBackupEncryptedChanged: saveSettingsState()
    onBlockchainRefreshRateChanged: saveSettingsState()
    onIndexerRefreshRateChanged: saveSettingsState()
    onExecutionRefreshRateChanged: saveSettingsState()
    onMessagingRefreshRateChanged: saveSettingsState()
    onStorageRefreshRateChanged: saveSettingsState()
    onFooterFieldRevisionChanged: saveSettingsState()
    onDashboardGraphRevisionChanged: saveSettingsState()

    function handleNetworkConfigurationChanged() { return AppModelCore.handleNetworkConfigurationChanged(root) }

    function handleMessagingConfigurationChanged() { return AppModelCore.handleMessagingConfigurationChanged(root) }

    function handleStorageConfigurationChanged() { return AppModelCore.handleStorageConfigurationChanged(root) }

    function navTreeItems() { return AppModelCore.navTreeItems(root) }

    function navRows() { return AppModelCore.navRows(root) }

    function navGroupExpanded(key) { return AppModelCore.navGroupExpanded(root, key) }

    function toggleNavGroup(key) { return AppModelCore.toggleNavGroup(root, key) }

    function expandNavGroupForView(view) { return AppModelCore.expandNavGroupForView(root, view) }

    function parentNavKeyForView(view) { return AppModelCore.parentNavKeyForView(root, view) }

    function navItemForView(view) { return AppModelCore.navItemForView(root, view) }

    function layerForView(view) { return AppModelCore.layerForView(root, view) }

    function navLabelForView(view) { return AppModelCore.navLabelForView(root, view) }

    function navTokenForView(view) { return AppModelCore.navTokenForView(root, view) }

    function navItemForQuery(query) { return AppModelCore.navItemForQuery(root, query) }

    function navItemMatches(item, normalized) { return AppModelCore.navItemMatches(root, item, normalized) }

    function viewTitle() { return AppModelCore.viewTitle(root) }

    function normalizedNavigationView(view) { return AppModelCore.normalizedNavigationView(root, view) }

    function navigationSnapshot() { return AppModelCore.navigationSnapshot(root) }

    function pushNavigationHistory() { return AppModelCore.pushNavigationHistory(root) }

    function restoreNavigationSnapshot(snapshot) { return AppModelCore.restoreNavigationSnapshot(root, snapshot) }

    function canNavigateBack() { return AppModelCore.canNavigateBack(root) }

    function canNavigateForward() { return AppModelCore.canNavigateForward(root) }

    function navigateBack() { return AppModelCore.navigateBack(root) }

    function navigateForward() { return AppModelCore.navigateForward(root) }

    function navigationBackLabel() { return AppModelCore.navigationBackLabel(root) }

    function navigationForwardLabel() { return AppModelCore.navigationForwardLabel(root) }

    function selectView(view, recordHistory) { return AppModelCore.selectView(root, view, recordHistory) }

    function openSettings(section, subsection, recordHistory) { return AppModelCore.openSettings(root, section, subsection, recordHistory) }

    function clearResult() { return AppModelCore.clearResult(root) }

    function setResult(title, text, isError, value, owner) { return AppModelCore.setResult(root, title, text, isError, value, owner) }

    function pageHasOutput(view) { return AppModelCore.pageHasOutput(root, view) }

    function callInspector(method, args, label) { return AppModelCore.callInspector(root, method, args, label) }

    function callModule(moduleName, method, args, label) { return AppModelCore.callModule(root, moduleName, method, args, label) }

    function blockchainArgs(extra) { return AppModelNetwork.blockchainArgs(root, extra) }

    function indexerArgs(extra) { return AppModelNetwork.indexerArgs(root, extra) }

    function executionArgs(extra) { return AppModelNetwork.executionArgs(root, extra) }

    function blockchainRpcArgs(extra) { return AppModelNetwork.blockchainRpcArgs(root, extra) }

    function executionRpcArgs(extra) { return AppModelNetwork.executionRpcArgs(root, extra) }

    function accountLookupArgs(account, idlJson, accountType) { return AppModelNetwork.accountLookupArgs(root, account, idlJson, accountType) }

    function requestModule(moduleName, method, args, label, showResult, cacheResult) { return AppModelCore.requestModule(root, moduleName, method, args, label, showResult, cacheResult) }

    function requestModuleAsync(moduleName, method, args, label, showResult, callback, acceptResponse) { return AppModelCore.requestModuleAsync(root, moduleName, method, args, label, showResult, callback, acceptResponse) }

    function nodeOperationStart(request, showResult, callback) { return AppModelCore.nodeOperationStart(root, request, showResult, callback) }

    function nodeOperationStatus(operationId, showResult, callback) { return AppModelCore.nodeOperationStatus(root, operationId, showResult, callback) }

    function nodeOperationEvents(operationId, afterSeq, showResult, callback) { return AppModelCore.nodeOperationEvents(root, operationId, afterSeq, showResult, callback) }

    function nodeOperationCancel(operationId, showResult, callback) { return AppModelCore.nodeOperationCancel(root, operationId, showResult, callback) }

    function updateNodeOperation(operation) { return AppModelCore.updateNodeOperation(root, operation) }

    function nodeOperationTerminal(operation) { return AppModelCore.nodeOperationTerminal(root, operation) }

    function nodeOperationResponse(operation) { return AppModelCore.nodeOperationResponse(root, operation) }

    function appendNodeOperationHistory(operation, detail) { return AppModelCore.appendNodeOperationHistory(root, operation, detail) }

    function nodeOperationHistoryRows(domain) { return AppModelCore.nodeOperationHistoryRows(root, domain) }

    function bridgeSupportsAsync() { return bridge.hasAsyncCalls() }

    function decodeAccountData(dataHex, idlJson, accountType) { return AppModelCore.decodeAccountData(root, dataHex, idlJson, accountType) }

    function decodeAccountDataAsync(dataHex, idlJson, accountType, callback) { return AppModelCore.decodeAccountDataAsync(root, dataHex, idlJson, accountType, callback) }

    function decodeTransactionSummaryAsync(summary, idlJson, callback) { return AppModelCore.decodeTransactionSummaryAsync(root, summary, idlJson, callback) }

    function loadIdlState() { return AppModelIdentity.loadIdlState(root) }

    function saveIdlState() { return AppModelIdentity.saveIdlState(root) }

    function idlStatePayload() { return AppModelIdentity.idlStatePayload(root) }

    function loadSettingsState() { return AppModelIdentity.loadSettingsState(root) }

    function saveSettingsState() { return AppModelIdentity.saveSettingsState(root) }

    function settingsStatePayload() { return AppModelIdentity.settingsStatePayload(root) }

    function socialCommentTopic(layer, entity, id) { return AppModelSocial.socialCommentTopic(root, layer, entity, id) }

    function socialLezAccountIdlTopic(accountId) { return AppModelSocial.socialLezAccountIdlTopic(root, accountId) }

    function socialComments(topic) { return AppModelSocial.socialComments(root, topic) }

    function socialCommentStateForTopic(topic) { return AppModelSocial.socialCommentState(root, topic) }

    function loadSocialComments(topic, reset, pageSize, expectedAccountId) { return AppModelSocial.loadSocialComments(root, topic, reset, pageSize, expectedAccountId) }

    function setSocialCommentState(topic, state) { return AppModelSocial.setSocialCommentState(root, topic, state) }

    function socialCommentRowsFromMessages(messages) { return AppModelSocial.socialCommentRowsFromMessages(root, messages) }

    function mergeSocialCommentRows(existingRows, incomingRows) { return AppModelSocial.mergeSocialCommentRows(root, existingRows, incomingRows) }

    function socialStoreCursor(value) { return AppModelSocial.socialStoreCursor(root, value) }

    function lastSocialMessageCursor(messages) { return AppModelSocial.lastSocialMessageCursor(root, messages) }

    function postSocialComment(topic, body, identityKey) { return AppModelSocial.postSocialComment(root, topic, body, identityKey) }

    function socialDeliveryArgs(extra) { return AppModelSocial.socialDeliveryArgs(root, extra) }

    function socialMessageSourceAvailable() { return AppModelSocial.socialMessageSourceAvailable(root) }

    function socialStoreAvailable() { return AppModelSocial.socialStoreAvailable(root) }

    function socialCommentSendAvailable(topic) { return AppModelSocial.socialCommentSendAvailable(root, topic) }

    function validSocialTopic(topic) { return AppModelSocial.validSocialTopic(root, topic) }

    function socialPageSize(pageSize) { return AppModelSocial.socialPageSize(root, pageSize) }

    function loadSocialSettings(value) { return AppModelSocial.loadSocialSettings(root, value) }

    function socialSettingsPayload() { return AppModelSocial.socialSettingsPayload(root) }

    function socialIdentityRows() { return AppModelSocial.socialIdentityRows(root) }

    function createSocialIdentity(displayName) { return AppModelSocial.createSocialIdentity(root, displayName) }

    function socialIdentityForKey(key) { return AppModelSocial.socialIdentityForKey(root, key) }

    function socialIdentityForConversation(topic, key) { return AppModelSocial.socialIdentityForConversation(root, topic, key) }

    function selectSocialIdentity(key) { return AppModelSocial.selectSocialIdentity(root, key) }

    function setSocialIdentityDefaultMode(mode) { return AppModelSocial.setSocialIdentityDefaultMode(root, mode) }

    function normalizedSocialIdentityDefaultMode(value) { return AppModelSocial.normalizedSocialIdentityDefaultMode(value) }

    function socialIdentityPayload(identity) { return AppModelSocial.socialIdentityPayload(root, identity) }

    function setSharedIdlPolicy(policy) { return AppModelSocial.setSharedIdlPolicy(root, policy) }

    function normalizedSharedIdlPolicy(value) { return AppModelSocial.normalizedSharedIdlPolicy(value) }

    function setSharedIdlAutoShare(enabled) { return AppModelSocial.setSharedIdlAutoShare(root, enabled) }

    function refreshSharedIdlsForAccount(accountId, dataHex, ownerProgramId) { return AppModelSocial.refreshSharedIdlsForAccount(root, accountId, dataHex, ownerProgramId) }

    function verifiedSharedIdlEntry(accountId, dataHex, ownerProgramId, topic, payload) { return AppModelSocial.verifiedSharedIdlEntry(root, accountId, dataHex, ownerProgramId, topic, payload) }

    function applySharedIdlPolicy(accountId, entry) { return AppModelSocial.applySharedIdlPolicy(root, accountId, entry) }

    function sharedIdlSuggestions(accountId) { return AppModelSocial.sharedIdlSuggestions(root, accountId) }

    function sharedIdlEntriesForAccount(accountId, ownerProgramId) { return AppModelSocial.sharedIdlEntriesForAccount(root, accountId, ownerProgramId) }

    function publishAccountIdl(accountId, ownerProgramId, idlEntry) { return AppModelSocial.publishAccountIdl(root, accountId, ownerProgramId, idlEntry) }

    function maybeAutoShareAccountIdl(accountId, ownerProgramId, idlEntry) { return AppModelSocial.maybeAutoShareAccountIdl(root, accountId, ownerProgramId, idlEntry) }

    function backupSettingsToStorage(encrypted) { return AppModelIdentity.backupSettingsToStorage(root, encrypted) }

    function restoreSettingsFromStorage(cid, useWallet) { return AppModelIdentity.restoreSettingsFromStorage(root, cid, useWallet) }

    function settingsBackupAvailable() { return AppModelIdentity.settingsBackupAvailable(root) }

    function loadWalletState() { return AppModelIdentity.loadWalletState(root) }

    function detectWalletProfile(saveDetected) { return AppModelIdentity.detectWalletProfile(root, saveDetected) }

    function saveWalletState() { return AppModelIdentity.saveWalletState(root) }

    function walletStatePayload() { return AppModelIdentity.walletStatePayload(root) }

    function walletProfile() { return AppModelIdentity.walletProfile(root) }

    function walletProfileConfigured() { return AppModelIdentity.walletProfileConfigured(root) }

    function walletHomeConfigured() { return AppModelIdentity.walletHomeConfigured(root) }

    function bedrockWalletSourceConfigured() { return AppModelIdentity.bedrockWalletSourceConfigured(root) }

    function walletProfileUsable() { return AppModelIdentity.walletProfileUsable(root) }

    function clearLocalWalletStatus() { return AppModelIdentity.clearLocalWalletStatus(root) }

    function walletHomeFallbackLabel() { return AppModelIdentity.walletHomeFallbackLabel(root) }

    function walletHomeSourceLabel() { return AppModelIdentity.walletHomeSourceLabel(root) }

    function walletBinaryDisplayLabel() { return AppModelIdentity.walletBinaryDisplayLabel(root) }

    function walletHomeDisplayLabel() { return AppModelIdentity.walletHomeDisplayLabel(root) }

    function redactedPath(path) { return AppModelIdentity.redactedPath(root, path) }

    function storageDisplayPath(path) { return AppModelIdentity.storageDisplayPath(root, path) }

    function updateStorageActiveOperation(value) {
        storageAppState.updateActiveOperation(value)
    }

    function clearStorageActiveOperation() {
        storageAppState.clearActiveOperation()
    }

    function checkLocalWalletProfile(showResult) { return AppModelIdentity.checkLocalWalletProfile(root, showResult) }

    function checkedLocalWalletProfile() { return AppModelIdentity.checkedLocalWalletProfile(root) }

    function createWalletAccount() { return AppModelIdentity.createWalletAccount(root) }

    function sendWalletTransaction() { return AppModelIdentity.sendWalletTransaction(root) }

    function readIncomingWalletTransactions() { return AppModelIdentity.readIncomingWalletTransactions(root) }

    function runWalletCommand(commandArgs) { return AppModelIdentity.runWalletCommand(root, commandArgs) }

    function walletCommandOperationDetail(value) { return AppModelIdentity.walletCommandOperationDetail(root, value) }

    function deployProgramBinary(programPath) { return AppModelIdentity.deployProgramBinary(root, programPath) }

    function deployProgramOperationDetail(value) { return AppModelIdentity.deployProgramOperationDetail(root, value) }

    function syncPrivateWallet() { return AppModelIdentity.syncPrivateWallet(root) }

    function queryLocalWalletAccounts(showResult) { return AppModelIdentity.queryLocalWalletAccounts(root, showResult) }

    function privateSyncOperationDetail(value) { return AppModelIdentity.privateSyncOperationDetail(root, value) }

    function queryBedrockWalletBalance() { return AppModelIdentity.queryBedrockWalletBalance(root) }

    function isBedrockHexId(value) { return AppModelIdentity.isBedrockHexId(root, value) }

    function appendLocalWalletOperation(label, status, detail) { return AppModelIdentity.appendLocalWalletOperation(root, label, status, detail) }

    function previewIdlInstruction(request) { return AppModelIdentity.previewIdlInstruction(root, request) }

    function sendIdlInstruction(request) { return AppModelIdentity.sendIdlInstruction(root, request) }

    function idlInstructionOperationDetail(value) { return AppModelIdentity.idlInstructionOperationDetail(root, value) }

    function refreshBedrockWalletModule(address) { return AppModelIdentity.refreshBedrockWalletModule(root, address) }

    function refreshLocalNodes(showResult) { return localNodes.refresh(showResult) }

    function refreshLocalDevnets() { return localNodes.refreshDevnets() }

    function runLocalNodeAction(action, node, networkId, workspacePath, label) { return localNodes.runAction(action, node, networkId, workspacePath, label) }

    function appendLocalNodeOperation(label, status, detail) { return localNodes.appendOperation(label, status, detail) }

    function localNodeActionLabel(action) { return localNodes.actionLabel(action) }

    function localNodeByKind(kind) { return localNodes.nodeByKind(kind) }

    function localNodeActionEnabled(kind, action) { return localNodes.actionEnabled(kind, action) }

    function localNodeNetworkActionEnabled(action) { return localNodes.networkActionEnabled(action) }

    function localNodeModeLabel() { return localNodes.modeLabel() }

    function localNodeSummaryText() { return localNodes.summaryText() }

    function localNodeToolProblem() { return localNodes.toolProblem() }

    function bedrockWalletModuleKnownAddressRows() { return AppModelIdentity.bedrockWalletModuleKnownAddressRows(root) }

    function bedrockWalletModuleNoteRows() { return AppModelIdentity.bedrockWalletModuleNoteRows(root) }

    function bedrockWalletModuleVoucherRows() { return AppModelIdentity.bedrockWalletModuleVoucherRows(root) }

    function bedrockWalletModuleBalance() { return AppModelIdentity.bedrockWalletModuleBalance(root) }

    function bedrockWalletModuleBalanceSummary() { return AppModelIdentity.bedrockWalletModuleBalanceSummary(root) }

    function bedrockWalletModuleRawText(method) { return AppModelIdentity.bedrockWalletModuleRawText(root, method) }

    function bedrockWalletModuleListKnown(method) { return AppModelIdentity.bedrockWalletModuleListKnown(root, method) }

    function bedrockWalletModuleReadOnlyMethods() { return AppModelIdentity.bedrockWalletModuleReadOnlyMethods(root) }

    function registeredIdlEntries() { return AppModelIdentity.registeredIdlEntries(root) }

    function normalizedIdlEntry(entry, fallbackIndex) { return AppModelIdentity.normalizedIdlEntry(root, entry, fallbackIndex) }

    function idlEntryAt(index) { return AppModelIdentity.idlEntryAt(root, index) }

    function idlNameFromJson(json) { return AppModelIdentity.idlNameFromJson(root, json) }

    function idlKey(name, programId, json) { return AppModelIdentity.idlKey(root, name, programId, json) }

    function idlEntryForKey(key) { return AppModelIdentity.idlEntryForKey(root, key) }

    function idlEntriesForProgram(programId) { return AppModelIdentity.idlEntriesForProgram(root, programId) }

    function cacheAccountIdlSelection(accountId, idlEntry, accountType, ownerProgramId) { return AppModelIdentity.cacheAccountIdlSelection(root, accountId, idlEntry, accountType, ownerProgramId) }

    function accountIdlSelection(accountId, ownerProgramId) { return AppModelIdentity.accountIdlSelection(root, accountId, ownerProgramId) }

    function cachedIdlEntryForAccount(accountId, ownerProgramId) { return AppModelIdentity.cachedIdlEntryForAccount(root, accountId, ownerProgramId) }

    function cachedAccountType(accountId, ownerProgramId) { return AppModelIdentity.cachedAccountType(root, accountId, ownerProgramId) }

    function accountCacheKey(accountId, ownerProgramId) { return AppModelIdentity.accountCacheKey(root, accountId, ownerProgramId) }

    function accountNetworkCacheScope() { return AppModelIdentity.accountNetworkCacheScope(root) }

    function accountOwnerCacheKey(ownerProgramId) { return AppModelIdentity.accountOwnerCacheKey(root, ownerProgramId) }

    function accountDecodeFullyConsumed(value) { return AppModelIdentity.accountDecodeFullyConsumed(root, value) }

    function transactionDecodeFullyConsumed(value) { return AppModelIdentity.transactionDecodeFullyConsumed(root, value) }

    function transactionDecodedInstruction(value) { return AppModelIdentity.transactionDecodedInstruction(root, value) }

    function transactionSummaryFromDetail(value) { return AppModelIdentity.transactionSummaryFromDetail(root, value) }

    function normalizedHexText(value) { return AppModelIdentity.normalizedHexText(root, value) }

    function canonicalProgramIdHex(value) { return AppModelIdentity.canonicalProgramIdHex(root, value) }

    function autoDecodeAccountData(dataHex, accountId, ownerProgramId, callback) { return AppModelIdentity.autoDecodeAccountData(root, dataHex, accountId, ownerProgramId, callback) }

    function accountDecodeCandidates(accountId, ownerProgramId) { return AppModelIdentity.accountDecodeCandidates(root, accountId, ownerProgramId) }

    function tryAccountDecodeCandidate(serial, dataHex, candidates, index, firstError, callback) { return AppModelIdentity.tryAccountDecodeCandidate(root, serial, dataHex, candidates, index, firstError, callback) }

    function autoDecodeTransactionDetail(detail) { return AppModelIdentity.autoDecodeTransactionDetail(root, detail) }

    function transactionDecodeCandidates(summary) { return AppModelIdentity.transactionDecodeCandidates(root, summary) }

    function candidateListHasEntry(candidates, key) { return AppModelIdentity.candidateListHasEntry(root, candidates, key) }

    function tryTransactionDecodeCandidate(serial, summary, candidates, index, partialValue) { return AppModelIdentity.tryTransactionDecodeCandidate(root, serial, summary, candidates, index, partialValue) }

    function refreshInterval(seconds) { return AppModelNetwork.refreshInterval(root, seconds) }

    function dashboardRefreshInterval() { return AppModelNetwork.dashboardRefreshInterval(root) }

    function canonicalRefreshRate(seconds) { return AppModelNetwork.canonicalRefreshRate(root, seconds) }

    function loadSourcePolicy() { return AppModelNetwork.loadSourcePolicy(root) }

    function sourcePolicyDefault(key, fallback) { return AppModelNetwork.sourcePolicyDefault(root, key, fallback) }

    function sourceModePolicy(family, value) { return AppModelNetwork.sourceModePolicy(root, family, value) }

    function sourceModePolicies(family) { return AppModelNetwork.sourceModePolicies(root, family) }

    function sourceModeOptions(family) { return AppModelNetwork.sourceModeOptions(root, family) }

    function sourceModeIndexFor(family, value, options) { return AppModelNetwork.sourceModeIndexFor(root, family, value, options) }

    function sourceModeAt(index, options) { return AppModelNetwork.sourceModeAt(root, index, options) }

    function sourceModeAdapter(family, value) { return AppModelNetwork.sourceModeAdapter(root, family, value) }

    function sourceModeTargetKind(family, value) { return AppModelNetwork.sourceModeTargetKind(root, family, value) }

    function sourceModeUsesEndpoint(family, value, endpointKind) { return AppModelNetwork.sourceModeUsesEndpoint(root, family, value, endpointKind) }

    function sourceModeSupportsCidProbe(family, value) { return AppModelNetwork.sourceModeSupportsCidProbe(root, family, value) }

    function sourceModeSupportsMutatingDiagnostics(family, value) { return AppModelNetwork.sourceModeSupportsMutatingDiagnostics(root, family, value) }

    function networkConnectionRate(kind) { return AppModelNetwork.networkConnectionRate(root, kind) }

    function setNetworkConnectionRate(kind, seconds) { return AppModelNetwork.setNetworkConnectionRate(root, kind, seconds) }

    function queryNetworkConnection(kind, showResult, includeSensitiveProbe) { return AppModelNetwork.queryNetworkConnection(root, kind, showResult, includeSensitiveProbe) }

    function networkConnectionRequest(kind, includeSensitiveProbe) { return AppModelNetwork.networkConnectionRequest(root, kind, includeSensitiveProbe) }

    function updateNetworkConnectionStatusForMethod(method, response) { return AppModelNetwork.updateNetworkConnectionStatusForMethod(root, method, response) }

    function networkConnectionKindForMethod(method) { return AppModelNetwork.networkConnectionKindForMethod(root, method) }

    function setNetworkConnectionPending(kind, pending) { return AppModelNetwork.setNetworkConnectionPending(root, kind, pending) }

    function networkConnectionIsPending(kind) { return AppModelNetwork.networkConnectionIsPending(root, kind) }

    function refreshIndexerStatus() { return AppModelNetwork.refreshIndexerStatus(root) }

    function indexerStatusNeedsFallback(value) { return AppModelNetwork.indexerStatusNeedsFallback(root, value) }

    function probeFieldFromResponse(response) { return AppModelNetwork.probeFieldFromResponse(root, response) }

    function updateNetworkConnectionStatus(kind, response) { return AppModelNetwork.updateNetworkConnectionStatus(root, kind, response) }

    function cacheNetworkConnectionResult(kind, response) { return AppModelNetwork.cacheNetworkConnectionResult(root, kind, response) }

    function networkConnectionSummary(kind, value) { return AppModelNetwork.networkConnectionSummary(root, kind, value) }

    function connectionValueOk(kind, value) { return AppModelNetwork.connectionValueOk(root, kind, value) }

    function storageReportReady(report) { return AppModelNetwork.storageReportReady(root, report) }

    function moduleReportReachable(report) { return AppModelNetwork.moduleReportReachable(root, report) }

    function sourceHealth(report) { return AppModelNetwork.sourceHealth(root, report) }

    function sourceHealthReady(report) { return AppModelNetwork.sourceHealthReady(root, report) }

    function sourceCapability(report, key) { return AppModelNetwork.sourceCapability(root, report, key) }

    function sourceCapabilityAvailable(report, key) { return AppModelNetwork.sourceCapabilityAvailable(root, report, key) }

    function sourceCapabilityEvidence(report, key) { return AppModelNetwork.sourceCapabilityEvidence(root, report, key) }

    function sourceCapabilityValue(report, key) { return AppModelNetwork.sourceCapabilityValue(root, report, key) }

    function sourceProbeFact(report, key) { return AppModelNetwork.sourceProbeFact(root, report, key) }

    function sourceProbeValue(report, key) { return AppModelNetwork.sourceProbeValue(root, report, key) }

    function reportProbeValue(report, method) { return AppModelNetwork.reportProbeValue(root, report, method) }

    function reportProbeOk(report, method) { return AppModelNetwork.reportProbeOk(root, report, method) }

    function reportProbe(report, method) { return AppModelNetwork.reportProbe(root, report, method) }

    function deliveryReportHealthy(report) { return AppModelNetwork.deliveryReportHealthy(root, report) }

    function deliveryHealthValueOk(value, unknownOk) { return AppModelNetwork.deliveryHealthValueOk(root, value, unknownOk) }

    function moduleReportError(report) { return AppModelNetwork.moduleReportError(root, report) }

    function deliverySourceReportArgs() { return AppModelNetwork.deliverySourceReportArgs(root) }

    function deliverySourceLabel() { return AppModelNetwork.deliverySourceLabel(root) }

    function deliverySourceTarget() { return AppModelNetwork.deliverySourceTarget(root) }

    function configuredMessagingRestUrl() { return AppModelNetwork.configuredMessagingRestUrl(root) }

    function normalizedMessagingSourceMode(value) { return AppModelNetwork.normalizedMessagingSourceMode(root, value) }

    function effectiveMessagingSourceMode(value) { return AppModelNetwork.effectiveMessagingSourceMode(root, value === undefined ? messagingSourceMode : value) }

    function normalizedCoreSourceMode(value) { return AppModelNetwork.normalizedCoreSourceMode(root, value) }

    function effectiveCoreSourceMode(value) { return AppModelNetwork.effectiveCoreSourceMode(root, value) }

    function blockchainSourceLabel() { return AppModelNetwork.blockchainSourceLabel(root) }

    function blockchainSourceTarget() { return AppModelNetwork.blockchainSourceTarget(root) }

    function indexerSourceLabel() { return AppModelNetwork.indexerSourceLabel(root) }

    function indexerSourceTarget() { return AppModelNetwork.indexerSourceTarget(root) }

    function executionSourceLabel() { return AppModelNetwork.executionSourceLabel(root) }

    function executionSourceTarget() { return AppModelNetwork.executionSourceTarget(root) }

    function storageSourceReportArgs(includeCidProbe) { return AppModelNetwork.storageSourceReportArgs(root, includeCidProbe) }

    function storageSourceLabel() { return AppModelNetwork.storageSourceLabel(root) }

    function storageSourceTarget() { return AppModelNetwork.storageSourceTarget(root) }

    function configuredStorageRestUrl() { return AppModelNetwork.configuredStorageRestUrl(root) }

    function normalizedStorageSourceMode(value) { return AppModelNetwork.normalizedStorageSourceMode(root, value) }

    function effectiveStorageSourceMode(value) { return AppModelNetwork.effectiveStorageSourceMode(root, value === undefined ? storageSourceMode : value) }

    function networkConnectionState(kind) { return AppModelNetwork.networkConnectionState(root, kind) }

    function setFooterFieldEnabled(key, enabled) { return AppModelNetwork.setFooterFieldEnabled(root, key, enabled) }

    function footerFieldEnabled(key) { return AppModelNetwork.footerFieldEnabled(root, key) }

    function setDashboardGraphEnabled(key, enabled) { return AppModelNetwork.setDashboardGraphEnabled(root, key, enabled) }

    function dashboardGraphEnabled(key) { return AppModelNetwork.dashboardGraphEnabled(root, key) }

    function copyMap(source) { return AppModelNetwork.copyMap(root, source) }

    function mergeMap(base, overrides) { return AppModelNetwork.mergeMap(root, base, overrides) }

    function stringSetting(value, key, fallback) { return AppModelNetwork.stringSetting(root, value, key, fallback) }

    function numberSetting(value, key, fallback) { return AppModelNetwork.numberSetting(root, value, key, fallback) }

    function boolSetting(value, key, fallback) { return AppModelNetwork.boolSetting(root, value, key, fallback) }

    function normalizedNetworkProfile(value) { return AppModelNetwork.normalizedNetworkProfile(root, value) }

    function resolvedNetworkProfile(storedProfile, sequencer, indexer, node) { return AppModelNetwork.resolvedNetworkProfile(root, storedProfile, sequencer, indexer, node) }

    function inferNetworkProfileFromEndpoints(sequencer, indexer, node) { return AppModelNetwork.inferNetworkProfileFromEndpoints(root, sequencer, indexer, node) }

    function normalizeEndpoint(value) { return AppModelNetwork.normalizeEndpoint(root, value) }

    function normalizedMessagingNetworkPreset(value) { return AppModelNetwork.normalizedMessagingNetworkPreset(root, value) }

    function scalarValue(value) { return AppModelNetwork.scalarValue(root, value) }

    function valueText(value) { return AppModelMetrics.valueText(root, value) }

    function valueToString(value) { return AppModelMetrics.valueToString(root, value) }

    function moduleReport(kind) { return AppModelMetrics.moduleReport(root, kind) }

    function moduleProbe(kind, method) { return AppModelMetrics.moduleProbe(root, kind, method) }

    function moduleProbeValue(kind, method) { return AppModelMetrics.moduleProbeValue(root, kind, method) }

    function moduleProbeError(kind, method) { return AppModelMetrics.moduleProbeError(root, kind, method) }

    function moduleLastError(kind) { return AppModelMetrics.moduleLastError(root, kind) }

    function openMetricsText(kind) { return AppModelMetrics.openMetricsText(root, kind) }

    function openMetricsTextFromValue(value) { return AppModelMetrics.openMetricsTextFromValue(root, value) }

    function openMetricValue(kind, names) { return AppModelMetrics.openMetricValue(root, kind, names) }

    function openMetricLabels(text) { return AppModelMetrics.openMetricLabels(root, text) }

    function metricJsonValue(value, names) { return AppModelMetrics.metricJsonValue(root, value, names) }

    function metricSpecName(spec) { return AppModelMetrics.metricSpecName(root, spec) }

    function metricSpecLabels(spec) { return AppModelMetrics.metricSpecLabels(root, spec) }

    function metricJsonLabels(value) { return AppModelMetrics.metricJsonLabels(root, value) }

    function metricLabelsMatch(actual, wanted) { return AppModelMetrics.metricLabelsMatch(root, actual, wanted) }

    function metricNumber(value) { return AppModelMetrics.metricNumber(root, value) }

    function overviewProbeValue(section, field) { return AppModelMetrics.overviewProbeValue(root, section, field) }

    function indexerHeadValue() { return AppModelMetrics.indexerHeadValue(root) }

    function sequencerHeadValue() { return AppModelMetrics.sequencerHeadValue(root) }

    function nodeProbeValue(name) { return AppModelMetrics.nodeProbeValue(root, name) }

    function cryptarchiaInfo() { return AppModelMetrics.cryptarchiaInfo(root) }

    function cryptarchiaValue(key) { return AppModelMetrics.cryptarchiaValue(root, key) }

    function networkInfo() { return AppModelMetrics.networkInfo(root) }

    function networkValue(key) { return AppModelMetrics.networkValue(root, key) }

    function mantleMetrics() { return AppModelMetrics.mantleMetrics(root) }

    function mantleValue(keys) { return AppModelMetrics.mantleValue(root, keys) }

    function tipMinusLib() { return AppModelMetrics.tipMinusLib(root) }

    function finalityLagSeconds() { return AppModelMetrics.finalityLagSeconds(root) }

    function indexerLag() { return AppModelMetrics.indexerLag(root) }

    function moduleMetricValue(kind, names) { return AppModelMetrics.moduleMetricValue(root, kind, names) }

    function moduleMetricSum(kind, names) { return AppModelMetrics.moduleMetricSum(root, kind, names) }

    function storageManifestCount() { return AppModelMetrics.storageManifestCount(root) }

    function dashboardMetricRawValue(key) { return AppModelMetrics.dashboardMetricRawValue(root, key) }

    function dashboardMetricValue(key) { return AppModelMetrics.dashboardMetricValue(root, key) }

    function dashboardMetricUsesWindow(key) { return AppModelMetrics.dashboardMetricUsesWindow(root, key) }

    function dashboardMetricWindowDelta(key) { return AppModelMetrics.dashboardMetricWindowDelta(root, key) }

    function dashboardMetricText(key) { return AppModelMetrics.dashboardMetricText(root, key) }

    function recordDashboardSnapshot() { return AppModelMetrics.recordDashboardSnapshot(root) }

    function dashboardMetricSamples(key) { return AppModelMetrics.dashboardMetricSamples(root, key) }

    function normalizedDashboardSamples(samples) { return AppModelMetrics.normalizedDashboardSamples(root, samples) }

    function dashboardMetricWindowSamples(key) { return AppModelMetrics.dashboardMetricWindowSamples(root, key) }

    function windowDeltaFromSamples(samples, timestamp, windowMs) { return AppModelMetrics.windowDeltaFromSamples(root, samples, timestamp, windowMs) }

    function defaultFooterFieldSelections() { return AppModelMetrics.defaultFooterFieldSelections(root) }

    function defaultDashboardGraphSelections() { return AppModelMetrics.defaultDashboardGraphSelections(root) }

    function refreshBlocksPage(anchorSlot) { return AppModelPages.refreshBlocksPage(root, anchorSlot) }

    function startBlocksLiveMode() { return AppModelPages.startBlocksLiveMode(root) }

    function stopBlocksLiveMode() { return AppModelPages.stopBlocksLiveMode(root) }

    function refreshBlocksLivePage() { return AppModelPages.refreshBlocksLivePage(root) }

    function mergeLiveBlocks(liveBlocks, existingBlocks, limit) { return AppModelPages.mergeLiveBlocks(root, liveBlocks, existingBlocks, limit) }

    function blocksLiveStatusText() { return AppModelPages.blocksLiveStatusText(root) }

    function olderBlocksPage() { return AppModelPages.olderBlocksPage(root) }

    function newerBlocksPage() { return AppModelPages.newerBlocksPage(root) }

    function setBlocksPageLimit(limit) { return AppModelPages.setBlocksPageLimit(root, limit) }

    function sortedBlocks(blocks) { return AppModelPages.sortedBlocks(root, blocks) }

    function blockSlot(block) { return AppModelPages.blockSlot(root, block) }

    function blockHash(block) { return AppModelPages.blockHash(root, block) }

    function blockParent(block) { return AppModelPages.blockParent(root, block) }

    function blockProof(block) { return AppModelPages.blockProof(root, block) }

    function blockRoot(block) { return AppModelPages.blockRoot(root, block) }

    function blockHeight(block) { return AppModelPages.blockHeight(root, block) }

    function blockVersion(block) { return AppModelPages.blockVersion(root, block) }

    function blockSignature(block) { return AppModelPages.blockSignature(root, block) }

    function blockStatus(block) { return AppModelPages.blockStatus(root, block) }

    function blockchainInfo() { return AppModelPages.blockchainInfo(root) }

    function sourceEmptyText(source, error, fallback) { return AppModelPages.sourceEmptyText(root, source, error, fallback) }

    function sourceProblemTitle(source, error, fallback) { return AppModelPages.sourceProblemTitle(root, source, error, fallback) }

    function blockTransactions(block) { return AppModelPages.blockTransactions(root, block) }

    function blockchainBlockDetail(block) { return AppModelPages.blockchainBlockDetail(root, block) }

    function blockchainBlockDetailById(value) { return AppModelPages.blockchainBlockDetailById(root, value) }

    function normalizedHashOrValue(value) { return AppModelPages.normalizedHashOrValue(root, value) }

    function refreshTransactionsPage(beforeBlock) { return AppModelPages.refreshTransactionsPage(root, beforeBlock) }

    function olderTransactionsPage() { return AppModelPages.olderTransactionsPage(root) }

    function newerTransactionsPage() { return AppModelPages.newerTransactionsPage(root) }

    function setTransactionsPageLimit(limit) { return AppModelPages.setTransactionsPageLimit(root, limit) }

    function refreshLezBlocksPage(beforeBlock) { return AppModelPages.refreshLezBlocksPage(root, beforeBlock) }

    function finishLezBlocksPage(beforeBlock, sequencerResponse, indexerResponse) { return AppModelPages.finishLezBlocksPage(root, beforeBlock, sequencerResponse, indexerResponse) }

    function olderLezBlocksPage() { return AppModelPages.olderLezBlocksPage(root) }

    function newerLezBlocksPage() { return AppModelPages.newerLezBlocksPage(root) }

    function setLezBlocksPageLimit(limit) { return AppModelPages.setLezBlocksPageLimit(root, limit) }

    function refreshLezTransactionsPage(beforeBlock) { return AppModelPages.refreshLezTransactionsPage(root, beforeBlock) }

    function olderLezTransactionsPage() { return AppModelPages.olderLezTransactionsPage(root) }

    function newerLezTransactionsPage() { return AppModelPages.newerLezTransactionsPage(root) }

    function setLezTransactionsPageLimit(limit) { return AppModelPages.setLezTransactionsPageLimit(root, limit) }

    function sortedIndexerBlocks(blocks) { return AppModelPages.sortedIndexerBlocks(root, blocks) }

    function mergedLezBlocks(sequencerBlocks, indexerBlocks, limit) { return AppModelPages.mergedLezBlocks(root, sequencerBlocks, indexerBlocks, limit) }

    function indexerBlockId(block) { return AppModelPages.indexerBlockId(root, block) }

    function indexerBlockHash(block) { return AppModelPages.indexerBlockHash(root, block) }

    function nextIndexerBlocksCursor(blocks) { return AppModelPages.nextIndexerBlocksCursor(root, blocks) }

    function normalizedPositiveInteger(value) { return AppModelPages.normalizedPositiveInteger(root, value) }

    function lezTransactionRowsFromBlocks(blocks) { return AppModelPages.lezTransactionRowsFromBlocks(root, blocks) }

    function lezTransactionHash(tx) { return AppModelPages.lezTransactionHash(root, tx) }

    function transactionProgramIdHex(tx) { return AppModelPages.transactionProgramIdHex(root, tx) }

    function lezTransactionOpCount(tx) { return AppModelPages.lezTransactionOpCount(root, tx) }

    function transactionRowsFromBlocks(blocks) { return AppModelPages.transactionRowsFromBlocks(root, blocks) }

    function sortedBlockchainBlocks(blocks) { return AppModelPages.sortedBlockchainBlocks(root, blocks) }

    function transactionHash(tx) { return AppModelPages.transactionHash(root, tx) }

    function transactionOps(tx) { return AppModelPages.transactionOps(root, tx) }

    function operationSummary(op, tx, index) { return AppModelPages.operationSummary(root, op, tx, index) }

    function byteHex(value) { return AppModelPages.byteHex(root, value) }

    function operationName(opcode) { return AppModelPages.operationName(root, opcode) }

    function refreshTransferActivityPage(beforeBlock, preserveHistory) { return AppModelPages.refreshTransferActivityPage(root, beforeBlock, preserveHistory) }

    function nextTransferActivityPage() { return AppModelPages.nextTransferActivityPage(root) }

    function previousTransferActivityPage() { return AppModelPages.previousTransferActivityPage(root) }

    function setTransferActivityPageLimit(limit) { return AppModelPages.setTransferActivityPageLimit(root, limit) }

    function nextTransferActivityBlock(recipients) { return AppModelPages.nextTransferActivityBlock(root, recipients) }

    function transferRecipientDetail(row) { return AppModelPages.transferRecipientDetail(root, row) }

    function transferRecipientDetailById(value) { return AppModelPages.transferRecipientDetailById(root, value) }

    function refreshChannelsPage(anchorSlot) { return AppModelPages.refreshChannelsPage(root, anchorSlot) }

    function olderChannelsPage() { return AppModelPages.olderChannelsPage(root) }

    function newerChannelsPage() { return AppModelPages.newerChannelsPage(root) }

    function setChannelsPageLimit(limit) { return AppModelPages.setChannelsPageLimit(root, limit) }

    function channelDetail(row) { return AppModelPages.channelDetail(root, row) }

    function channelDetailById(value) { return AppModelPages.channelDetailById(root, value) }

    function refreshDashboard() { return AppModelSearch.refreshDashboard(root) }

    function updateDashboardCache(method, value) { return AppModelSearch.updateDashboardCache(root, method, value) }

    function routeSearch(query) { return AppModelSearch.routeSearch(root, query) }

    function openStorageCid(cid) { return AppModelSearch.openStorageCid(root, cid) }

    function isStorageCid(value) { return AppModelSearch.isStorageCid(root, value) }

    function numericSearchUsesLezBlock() { return AppModelSearch.numericSearchUsesLezBlock(root) }

    function routePrefixedSearch(query) { return AppModelSearch.routePrefixedSearch(root, query) }

    function searchPrefix(query) { return AppModelSearch.searchPrefix(root, query) }

    function isSearchPrefix(prefix) { return AppModelSearch.isSearchPrefix(root, prefix) }

    function routeModuleSearchTarget(target) { return AppModelSearch.routeModuleSearchTarget(root, target) }

    function resolveSearchHash(hash) { return AppModelSearch.resolveSearchHash(root, hash) }

    function resolveSearchTransaction(serial, hash, recordHistory) { return AppModelSearch.resolveSearchTransaction(root, serial, hash, recordHistory) }

    function resolveSearchAccount(serial, account, recordHistory) { return AppModelSearch.resolveSearchAccount(root, serial, account, recordHistory) }

    function viewKeyForQuery(query) { return AppModelSearch.viewKeyForQuery(root, query) }

    function settingsTargetForQuery(query) { return AppModelSearch.settingsTargetForQuery(root, query) }

    function openReference(kind, value, payload) { return AppModelOpeners.openReference(root, kind, value, payload) }

    function openMantleTransaction(hash) { return AppModelOpeners.openMantleTransaction(root, hash) }

    function openAccount(account) { return AppModelOpeners.openAccount(root, account) }

    function openPrivateAccountReference(account) { return AppModelOpeners.openPrivateAccountReference(root, account) }

    function openTransaction(hash) { return AppModelOpeners.openTransaction(root, hash) }

    function openLezSearchTarget(target) { return AppModelOpeners.openLezSearchTarget(root, target) }

    function openLezBlock(blockId) { return AppModelOpeners.openLezBlock(root, blockId) }

    function resolveLezHash(hash) { return AppModelOpeners.resolveLezHash(root, hash) }

    function openLezTransaction(hash, recordHistory) { return AppModelOpeners.openLezTransaction(root, hash, recordHistory) }

    function inspectTransaction(hash, idl, recordHistory) { return AppModelOpeners.inspectTransaction(root, hash, idl, recordHistory) }

    function openBlockchainBlock(blockOrId) { return AppModelOpeners.openBlockchainBlock(root, blockOrId) }

    function loadBlockchainBlockById(blockId) { return AppModelOpeners.loadBlockchainBlockById(root, blockId) }

    function loadBlockchainBlockBySlot(slot) { return AppModelOpeners.loadBlockchainBlockBySlot(root, slot) }

    function openBlockchainTransaction(transaction, block) { return AppModelOpeners.openBlockchainTransaction(root, transaction, block) }

    function transactionDetail(hash) { return AppModelOpeners.transactionDetail(root, hash) }

    function blockchainTransactionDetail(value, fallbackHash) { return AppModelOpeners.blockchainTransactionDetail(root, value, fallbackHash) }

    function openIndexerBlock(headerHash, payload) { return AppModelOpeners.openIndexerBlock(root, headerHash, payload) }

    function indexerBlockDetail(value, source) { return AppModelOpeners.indexerBlockDetail(root, value, source) }

    function openLocalWallet(wallet, tab) { return AppModelOpeners.openLocalWallet(root, wallet, tab) }

    function showLocalWalletRequired(wallet) { return AppModelOpeners.showLocalWalletRequired(root, wallet) }

    function openProgram(programId) { return AppModelOpeners.openProgram(root, programId) }

    function programContextDetail(programId) { return AppModelOpeners.programContextDetail(root, programId) }

    function programContextFromParts(input, normalized, knownRow, accountResponse, lookupError) { return AppModelOpeners.programContextFromParts(root, input, normalized, knownRow, accountResponse, lookupError) }

    function knownProgramRow(programId) { return AppModelOpeners.knownProgramRow(root, programId) }

    function programRecentTransactions(programId) { return AppModelOpeners.programRecentTransactions(root, programId) }

    function looksLikeHexId(value) { return AppModelOpeners.looksLikeHexId(root, value) }

    function openRecipient(recipient) { return AppModelOpeners.openRecipient(root, recipient) }

    function openChannel(channel) { return AppModelOpeners.openChannel(root, channel) }

    function programIdKnown(programId) { return AppModelRegistry.programIdKnown(root, programId) }

    function knownProgramCacheScope() { return AppModelRegistry.knownProgramCacheScope(root) }

    function knownProgramIdRows() { return AppModelRegistry.knownProgramIdRows(root) }

    function updateKnownProgramIds(value) { return AppModelRegistry.updateKnownProgramIds(root, value) }

    function registerIdl(name, programId, json, programBinary) { return AppModelRegistry.registerIdl(root, name, programId, json, programBinary) }

    function removeIdl(index) { return AppModelRegistry.removeIdl(root, index) }

    function profileIndex() { return AppModelRegistry.profileIndex(root) }

    function applyProfile(index) { return AppModelRegistry.applyProfile(root, index) }

    function clearDashboardMetricHistoryForPrefix(prefix) { return AppModelMetrics.clearDashboardMetricHistoryForPrefix(root, prefix) }
}
