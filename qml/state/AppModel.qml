import QtQuick
import "../services"
import "app/AppModelCore.js" as AppModelCore
import "domains" as Domains
import "identity/AppModelIdentity.js" as AppModelIdentity
import "network/AppModelNetwork.js" as AppModelNetwork
import "metrics/AppModelMetrics.js" as AppModelMetrics
import "programs" as Programs
import "programs/AppModelRegistry.js" as AppModelRegistry
import "social/AppModelSocial.js" as AppModelSocial
import "programs/ProgramDecodeSession.js" as ProgramDecodeSession

QtObject {
    id: root

    required property BridgeClient bridge

    readonly property string inspectorModule: "logos_inspector"
    readonly property string blockchainModule: "blockchain_module"
    readonly property string indexerModule: "lez_indexer_module"
    readonly property string storageModule: "storage_module"
    readonly property string deliveryModule: "delivery_module"
    readonly property string capabilityModule: "capability_module"
    property Domains.SourceRoutingState sourceRouting: Domains.SourceRoutingState {
        id: sourceRoutingState

        blockchainModule: root.blockchainModule
        indexerModule: root.indexerModule
        deliveryModule: root.deliveryModule
        storageModule: root.storageModule
        blockchainSourceMode: root.blockchainSourceMode
        indexerSourceMode: root.indexerSourceMode
        executionSourceMode: root.executionSourceMode
        messagingSourceMode: root.messagingSourceMode
        storageSourceMode: root.storageSourceMode
        nodeUrl: root.nodeUrl
        indexerUrl: root.indexerUrl
        sequencerUrl: root.sequencerUrl
        messagingRestUrl: root.messagingRestUrl
        messagingMetricsUrl: root.messagingMetricsUrl
        messagingNetworkPreset: root.messagingNetworkPreset
        messagingMutatingDiagnosticsEnabled: root.messagingMutatingDiagnosticsEnabled
        storageRestUrl: root.storageRestUrl
        storageMetricsUrl: root.storageMetricsUrl
        storageNetworkPreset: root.storageNetworkPreset
        storageCidProbe: root.storageCidProbe
        storagePrivilegedDebugEnabled: root.storagePrivilegedDebugEnabled
        storageMutatingDiagnosticsEnabled: root.storageMutatingDiagnosticsEnabled
        gateway: QtObject {
            function callInspector(method, args) {
                return root.bridge.callModule(root.inspectorModule, method, Array.isArray(args) ? args : [])
            }

            function prefersBasecampModules() {
                return root.prefersBasecampModules()
            }
        }
    }
    property alias sourcePolicy: sourceRoutingState.sourcePolicy
    property alias sourcePolicyLoaded: sourceRoutingState.sourcePolicyLoaded
    property Domains.NetworkProfileState networkProfiles: Domains.NetworkProfileState {
        id: networkProfileState
        sourcePolicy: root.sourcePolicy
    }
    readonly property var storageSource: sourceRouting.storageSourceView()
    readonly property var deliverySource: sourceRouting.deliverySourceView()

    property AppShellState shell: AppShellState {
        id: appShellState
        model: root
    }
    property alias currentView: appShellState.currentView
    property alias statusText: appShellState.statusText
    property alias busy: appShellState.busy
    property alias resultTitle: appShellState.resultTitle
    property alias resultText: appShellState.resultText
    property alias resultValue: appShellState.resultValue
    property alias resultIsError: appShellState.resultIsError
    property alias resultOwner: appShellState.resultOwner
    property Domains.NetworkInspectionState chainPages: Domains.NetworkInspectionState {
        id: chainPageState

        inspectorModule: root.inspectorModule
        gateway: QtObject {
            function requestModule(moduleName, method, args, label, showResult, cacheResult) {
                return root.requestModule(moduleName, method, args, label, showResult, cacheResult)
            }

            function requestModuleAsync(moduleName, method, args, label, showResult, callback, acceptResponse) {
                return root.requestModuleAsync(moduleName, method, args, label, showResult, callback, acceptResponse)
            }

            function setResult(title, text, isError, value, owner) {
                return root.setResult(title, text, isError, value, owner)
            }

            function blockchainArgs(extra) { return root.blockchainArgs(extra) }

            function indexerArgs(extra) { return root.indexerArgs(extra) }

            function executionArgs(extra) { return root.executionArgs(extra) }

            function blockchainRpcArgs(extra) { return root.blockchainRpcArgs(extra) }

            function networkConnectionState(kind) { return root.networkConnectionState(kind) }

            function valueToString(value) { return root.valueToString(value) }

            function canonicalProgramIdHex(value) { return root.canonicalProgramIdHex(value) }

            function normalizedHexText(value) { return root.normalizedHexText(value) }
        }
    }
    property alias dashboardOverview: chainPageState.dashboardOverview
    property alias dashboardNode: chainPageState.dashboardNode
    property alias dashboardL1Blocks: chainPageState.dashboardL1Blocks
    property alias dashboardBlocks: chainPageState.dashboardBlocks
    property alias dashboardSequencerBlocks: chainPageState.dashboardSequencerBlocks
    property alias dashboardError: chainPageState.dashboardError
    property alias blockDetailValue: chainPageState.blockDetailValue
    property alias blockDetailError: chainPageState.blockDetailError
    property alias transactionDetailValue: chainPageState.transactionDetailValue
    property alias transactionDetailError: chainPageState.transactionDetailError
    property alias accountDetailValue: chainPageState.accountDetailValue
    property alias transferRecipientDetailValue: chainPageState.transferRecipientDetailValue
    property alias channelDetailValue: chainPageState.channelDetailValue
    property alias channelDetailError: chainPageState.channelDetailError
    property alias blocksPageRows: chainPageState.blocksPageRows
    property alias blocksPageSlotFrom: chainPageState.blocksPageSlotFrom
    property alias blocksPageSlotTo: chainPageState.blocksPageSlotTo
    property alias blocksPageWindow: chainPageState.blocksPageWindow
    property alias blocksPageLimit: chainPageState.blocksPageLimit
    property alias blocksPageError: chainPageState.blocksPageError
    property alias blocksLiveEnabled: chainPageState.blocksLiveEnabled
    property alias blocksLiveError: chainPageState.blocksLiveError
    property alias blocksLiveSource: chainPageState.blocksLiveSource
    property alias blocksLiveUnknownEvents: chainPageState.blocksLiveUnknownEvents
    property alias blocksLiveCheckedAt: chainPageState.blocksLiveCheckedAt
    property alias transactionsPageRows: chainPageState.transactionsPageRows
    property alias transactionsPageBeforeBlock: chainPageState.transactionsPageBeforeBlock
    property alias transactionsPageNextBeforeBlock: chainPageState.transactionsPageNextBeforeBlock
    property alias transactionsPageBlockBatch: chainPageState.transactionsPageBlockBatch
    property alias transactionsPageLimit: chainPageState.transactionsPageLimit
    property alias transactionsPageError: chainPageState.transactionsPageError
    property alias lezBlocksPageRows: chainPageState.lezBlocksPageRows
    property alias lezBlocksPageBeforeBlock: chainPageState.lezBlocksPageBeforeBlock
    property alias lezBlocksPageNextBeforeBlock: chainPageState.lezBlocksPageNextBeforeBlock
    property alias lezBlocksPageLimit: chainPageState.lezBlocksPageLimit
    property alias lezBlocksPageError: chainPageState.lezBlocksPageError
    property alias lezBlocksPageLoading: chainPageState.lezBlocksPageLoading
    property alias lezBlocksPageRequestSerial: chainPageState.lezBlocksPageRequestSerial
    property alias lezTransactionsPageRows: chainPageState.lezTransactionsPageRows
    property alias lezTransactionsPageBeforeBlock: chainPageState.lezTransactionsPageBeforeBlock
    property alias lezTransactionsPageNextBeforeBlock: chainPageState.lezTransactionsPageNextBeforeBlock
    property alias lezTransactionsPageOverflowRows: chainPageState.lezTransactionsPageOverflowRows
    property alias lezTransactionsPageOverflowNextBeforeBlock: chainPageState.lezTransactionsPageOverflowNextBeforeBlock
    property alias lezTransactionsBlockBatch: chainPageState.lezTransactionsBlockBatch
    property alias lezTransactionsPageLimit: chainPageState.lezTransactionsPageLimit
    property alias lezTransactionsPageError: chainPageState.lezTransactionsPageError
    property alias transferActivityRows: chainPageState.transferActivityRows
    property alias transferActivityBeforeBlock: chainPageState.transferActivityBeforeBlock
    property alias transferActivityNextBeforeBlock: chainPageState.transferActivityNextBeforeBlock
    property alias transferActivityOverflowRows: chainPageState.transferActivityOverflowRows
    property alias transferActivityOverflowNextBeforeBlock: chainPageState.transferActivityOverflowNextBeforeBlock
    property alias transferActivityBlockBatch: chainPageState.transferActivityBlockBatch
    property alias transferActivityLimit: chainPageState.transferActivityLimit
    property alias transferActivityHistory: chainPageState.transferActivityHistory
    property alias transferActivityError: chainPageState.transferActivityError
    property alias channelsPageRows: chainPageState.channelsPageRows
    property alias channelsPageSlotFrom: chainPageState.channelsPageSlotFrom
    property alias channelsPageSlotTo: chainPageState.channelsPageSlotTo
    property alias channelsPageWindow: chainPageState.channelsPageWindow
    property alias channelsPageLimit: chainPageState.channelsPageLimit
    property alias channelsPageError: chainPageState.channelsPageError

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
    property Domains.SocialCollaborationState social: Domains.SocialCollaborationState {
        id: socialState
    }
    property alias socialCommentPageSize: socialState.commentPageSize
    property alias socialIdentityDefaultMode: socialState.identityDefaultMode
    property alias selectedSocialIdentityKey: socialState.selectedIdentityKey
    property alias socialConversationIdentityKeys: socialState.conversationIdentityKeys
    property alias socialIdentityRevision: socialState.identityRevision
    property alias socialCommentState: socialState.commentState
    property alias socialCommentRevision: socialState.commentRevision
    property alias socialSharedIdls: socialState.sharedIdls
    property alias sharedIdlPolicy: socialState.sharedIdlPolicy
    property alias sharedIdlAutoShare: socialState.sharedIdlAutoShare
    property alias socialAutoSharedIdls: socialState.autoSharedIdls
    property alias sharedIdlRevision: socialState.sharedIdlRevision
    property string storageSourceMode: "auto"
    property string storageRestUrl: "http://127.0.0.1:8080/api/storage/v1"
    property string storageMetricsUrl: "http://127.0.0.1:8008/metrics"
    property string storageNetworkPreset: "logos.test"
    property string storageDataDir: ""
    property int storageRollingWindow: 120
    property bool storageLocalDiagnosticsEnabled: false
    property bool storagePrivilegedDebugEnabled: false
    property bool storageMutatingDiagnosticsEnabled: false
    property var runtimeOperations: ({})
    property var runtimeOperationEventSeq: ({})
    property var runtimeOperationHistory: []
    property int runtimeOperationsRevision: 0
    property string settingsBackupCid: ""
    property string settingsRestoreCid: ""
    property bool settingsBackupEncrypted: false
    property string settingsBackupStatus: ""

    property string sequencerTab: "blocks"
    property string accountTab: "lookup"
    property string programTab: "programIds"
    property string indexerTab: "status"
    property string localWalletTab: "profiles"
    property string localWalletLookupTarget: ""
    property alias settingsSection: appShellState.settingsSection
    property alias settingsNetworkSection: appShellState.settingsNetworkSection
    property alias settingsUiSection: appShellState.settingsUiSection
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
    property var storageSourceReport: null
    property var messagingSourceReport: null
    property int blockchainModuleEventRevision: 0
    property string blockchainLastEventText: ""

    property Domains.ProgramDecodeState programDecode: Domains.ProgramDecodeState {
        id: programDecodeState

        registryGateway: QtObject {
            function canonicalProgramIdHex(value) {
                return root.canonicalProgramIdHex(value)
            }

            function normalizedHexText(value) {
                return root.normalizedHexText(value)
            }
        }
    }
    property alias idlRegistry: programDecodeState.idlRegistry
    property alias registeredIdls: programDecodeState.registeredIdls
    property alias socialIdentities: socialState.identities
    property alias idlStateLoaded: programDecodeState.loaded
    property LocalWalletAppState wallet: LocalWalletAppState {
        id: walletState

        gateway: QtObject {
            function call(method, args) {
                return root.bridge.callModule(root.inspectorModule, method, args || [])
            }

            function request(method, args, label, showResult, callback) {
                return root.requestModuleAsync(root.inspectorModule, method, args || [], label, showResult === true, callback)
            }

            function requestBlocking(method, args, label, showResult) {
                return root.requestModule(root.inspectorModule, method, args || [], label, showResult === true)
            }

            function setStatus(value) {
                root.statusText = String(value || "")
            }

            function busy() {
                return root.busy
            }

            function setBusy(value) {
                root.busy = value === true
            }

            function setResult(title, text, isError, value) {
                root.setResult(title, text, isError, value)
            }

            function openLocalWallet(wallet, tab) {
                return root.openLocalWallet(wallet, tab)
            }

            function networkProfile() {
                return root.networkProfile
            }

            function nodeUrl() {
                return String(root.nodeUrl || "")
            }

            function redactedPath(path) {
                return root.redactedPath(path)
            }

            function setBedrockWalletBalance(value, error) {
                root.bedrockWalletBalanceValue = value
                root.bedrockWalletBalanceError = String(error || "")
            }

            function setIdlInstructionState(value, error) {
                root.idlInstructionPreviewValue = value
                root.idlInstructionError = String(error || "")
            }

            function appendRuntimeOperationHistory(operation, detail) {
                return root.appendOperationHistory(operation, detail)
            }

            function appendNodeOperationHistory(operation, detail) {
                return root.appendOperationHistory(operation, detail)
            }

            function appendOperationHistory(operation, detail) {
                return root.appendOperationHistory(operation, detail)
            }
        }
    }
    property alias walletStateLoaded: walletState.loaded
    property bool settingsStateLoaded: false
    property string settingsStateError: ""
    property alias walletProfileLabel: walletState.profileLabel
    property alias walletBinary: walletState.binary
    property alias walletHome: walletState.home
    property alias walletPublicKeyProbe: walletState.publicKeyProbe
    property alias walletCreatePrivacy: walletState.createPrivacy
    property alias walletCreateLabel: walletState.createLabel
    property alias walletSendFrom: walletState.sendFrom
    property alias walletSendTo: walletState.sendTo
    property alias walletSendToKeys: walletState.sendToKeys
    property alias walletSendToNpk: walletState.sendToNpk
    property alias walletSendToVpk: walletState.sendToVpk
    property alias walletSendToIdentifier: walletState.sendToIdentifier
    property alias walletSendAmount: walletState.sendAmount
    property alias walletAdvancedCommand: walletState.advancedCommand
    property alias bedrockWalletBalanceTip: walletState.bedrockBalanceTip
    property alias localWalletStatus: walletState.status
    property alias localWalletStatusError: walletState.statusError
    property alias localWalletOperations: walletState.operations
    property alias localWalletAccountsValue: walletState.accountsValue
    property alias localWalletAccountsError: walletState.accountsError
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
                return root.appendOperationHistory(operation, detail)
            }

            function openSettings(section, subSection) {
                return root.openSettings(section, subSection)
            }

            function valueText(value) {
                return root.valueText(value)
            }
        }
        busy: root.busy
        sourceMode: root.storageSource.mode
        effectiveSourceMode: root.storageSource.effectiveMode
        sourceLabel: root.storageSource.label
        sourceTarget: root.storageSource.target
        sourceTargetKind: root.storageSource.targetKind
        usesRestEndpoint: root.storageSource.usesRestEndpoint
        supportsMutatingDiagnostics: root.storageSource.supportsMutatingDiagnostics
        restEndpoint: root.storageSource.restEndpoint
        moduleName: root.storageSource.moduleName
        networkPreset: root.storageSource.networkPreset
        mutatingDiagnosticsEnabled: root.storageMutatingDiagnosticsEnabled
        currentView: root.currentView
        resultTitle: root.resultTitle
        resultText: root.resultText
        resultIsError: root.resultIsError
        resultOwner: root.resultOwner
        sourceReport: root.storageSourceReport
    }
    property alias storageAppTab: storageAppState.currentTab
    property alias storageCidProbe: storageAppState.cidProbe
    property alias storageActiveOperation: storageAppState.activeOperation
    property alias storageActiveOperationRevision: storageAppState.activeOperationRevision
    property DeliveryAppState deliveryApp: DeliveryAppState {
        id: deliveryAppState

        gateway: QtObject {
            function call(method, args, label) {
                return root.callInspector(method, args, label)
            }

            function startRuntimeOperation(request, showResult, callback) {
                return root.runtimeOperationStart(request, showResult === true, callback)
            }

            function runtimeOperationStatus(operationId, showResult, callback) {
                return root.runtimeOperationStatus(operationId, showResult === true, callback)
            }

            function appendOperationHistory(operation, detail) {
                return root.appendOperationHistory(operation, detail)
            }
        }
        busy: root.busy
        sourceMode: root.deliverySource.mode
        effectiveSourceMode: root.deliverySource.effectiveMode
        sourceLabel: root.deliverySource.label
        sourceTarget: root.deliverySource.target
        sourceTargetKind: root.deliverySource.targetKind
        usesRestEndpoint: root.deliverySource.usesRestEndpoint
        supportsMutatingDiagnostics: root.deliverySource.supportsMutatingDiagnostics
        restEndpoint: root.deliverySource.restEndpoint
        moduleName: root.deliverySource.moduleName
        networkPreset: root.deliverySource.networkPreset
        mutatingDiagnosticsEnabled: root.messagingMutatingDiagnosticsEnabled
    }
    property alias deliveryAppTab: deliveryAppState.currentTab
    property alias deliveryActiveTopic: deliveryAppState.activeTopic
    property alias deliveryActiveOperation: deliveryAppState.activeOperation
    property alias deliveryActiveOperationRevision: deliveryAppState.activeOperationRevision
    property alias deliveryModuleEvents: deliveryAppState.deliveryModuleEvents
    property alias deliveryModuleEventRevision: deliveryAppState.deliveryModuleEventRevision
    property alias deliveryConnectionStatus: deliveryAppState.deliveryConnectionStatus
    property alias deliveryNodeStatus: deliveryAppState.deliveryNodeStatus
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
                return root.appendOperationHistory(operation, detail)
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
    property Programs.ProgramExecutionState programExecution: Programs.ProgramExecutionState {
        id: programExecutionState

        gateway: QtObject {
            function request(method, args, label, showResult, callback) {
                return root.requestModuleAsync(root.inspectorModule, method, args || [], label, showResult === true, callback)
            }

            function busy() {
                return root.busy
            }

            function setBusy(value) {
                root.busy = value === true
            }

            function setStatus(value) {
                root.statusText = String(value || "")
            }

            function setResult(title, text, isError, value) {
                return root.setResult(title, text, isError, value)
            }

            function walletProfile() {
                return root.wallet.profile(root.networkProfile)
            }

            function walletProfileConfigured() {
                return root.wallet.profileConfigured()
            }

            function walletHomeConfigured() {
                return root.wallet.homeConfigured()
            }

            function openLocalWallet(tab) {
                return root.openLocalWallet("", tab)
            }

            function appendOperationHistory(operation, detail) {
                return root.appendOperationHistory(operation, detail)
            }
        }
    }
    property alias idlInstructionPreviewValue: programExecutionState.idlInstructionPreviewValue
    property alias idlInstructionError: programExecutionState.idlInstructionError
    property var bedrockWalletBalanceValue: null
    property string bedrockWalletBalanceError: ""
    property string bedrockWalletModuleError: ""
    property alias accountIdlSelections: programDecodeState.accountIdlSelections
    property alias accountIdlSelectionRevision: programDecodeState.accountIdlSelectionRevision
    property alias knownProgramIds: programDecodeState.knownProgramIds
    property alias knownProgramIdsRevision: programDecodeState.knownProgramIdsRevision
    property alias accountAutoDecodeSerial: programDecodeState.accountAutoDecodeSerial
    property alias transactionAutoDecodeSerial: programDecodeState.transactionAutoDecodeSerial
    property alias searchResolveSerial: programDecodeState.searchResolveSerial
    property alias programOpenSerial: programDecodeState.programOpenSerial
    property alias navExpanded: appShellState.navExpanded
    property alias navRevision: appShellState.navRevision
    property alias navigationBackStack: appShellState.navigationBackStack
    property alias navigationForwardStack: appShellState.navigationForwardStack
    property alias navigationRevision: appShellState.navigationRevision
    property alias navigationRestoring: appShellState.navigationRestoring
    readonly property alias navigationHistoryLimit: appShellState.navigationHistoryLimit
    property EntityNavigationSession entityNavigation: EntityNavigationSession {
        id: entityNavigationState
        model: root
    }
    property FavoritesState favoriteStore: FavoritesState {
        onRevisionChanged: root.saveSettingsState()
        onOpenRequested: function (openKind, value) {
            entityNavigationState.openReference(openKind, value)
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

    function navTreeItems() { return appShellState.navTreeItems() }

    function navRows() { return appShellState.navRows() }

    function navGroupExpanded(key) { return appShellState.navGroupExpanded(key) }

    function toggleNavGroup(key) { return appShellState.toggleNavGroup(key) }

    function expandNavGroupForView(view) { return appShellState.expandNavGroupForView(view) }

    function parentNavKeyForView(view) { return appShellState.parentNavKeyForView(view) }

    function navItemForView(view) { return appShellState.navItemForView(view) }

    function layerForView(view) { return appShellState.layerForView(view) }

    function navLabelForView(view) { return appShellState.navLabelForView(view) }

    function navTokenForView(view) { return appShellState.navTokenForView(view) }

    function navItemForQuery(query) { return appShellState.navItemForQuery(query) }

    function navItemMatches(item, normalized) { return appShellState.navItemMatches(item, normalized) }

    function viewTitle() { return appShellState.viewTitle() }

    function normalizedNavigationView(view) { return appShellState.normalizedNavigationView(view) }

    function navigationSnapshot() { return appShellState.navigationSnapshot() }

    function pushNavigationHistory() { return appShellState.pushNavigationHistory() }

    function restoreNavigationSnapshot(snapshot) { return appShellState.restoreNavigationSnapshot(snapshot) }

    function canNavigateBack() { return appShellState.canNavigateBack() }

    function canNavigateForward() { return appShellState.canNavigateForward() }

    function navigateBack() { return appShellState.navigateBack() }

    function navigateForward() { return appShellState.navigateForward() }

    function navigationBackLabel() { return appShellState.navigationBackLabel() }

    function navigationForwardLabel() { return appShellState.navigationForwardLabel() }

    function selectView(view, recordHistory) { return appShellState.selectView(view, recordHistory) }

    function openSettings(section, subsection, recordHistory) { return appShellState.openSettings(section, subsection, recordHistory) }

    function clearResult() { return appShellState.clearResult() }

    function setResult(title, text, isError, value, owner) { return appShellState.setResult(title, text, isError, value, owner) }

    function pageHasOutput(view) { return appShellState.pageHasOutput(view) }

    function callInspector(method, args, label) { return AppModelCore.callInspector(root, method, args, label) }

    function callModule(moduleName, method, args, label) { return AppModelCore.callModule(root, moduleName, method, args, label) }

    function blockchainArgs(extra) { return sourceRouting.blockchainArgs(extra) }

    function indexerArgs(extra) { return sourceRouting.indexerArgs(extra) }

    function executionArgs(extra) { return sourceRouting.executionArgs(extra) }

    function blockchainRpcArgs(extra) { return AppModelNetwork.blockchainRpcArgs(root, extra) }

    function executionRpcArgs(extra) { return AppModelNetwork.executionRpcArgs(root, extra) }

    function accountLookupArgs(account, idlJson, accountType) {
        return sourceRouting.accountArgs(account, idlJson, accountType)
    }

    function lezLookupArgs(target) {
        return sourceRouting.lezArgs(target)
    }

    function requestModule(moduleName, method, args, label, showResult, cacheResult) { return AppModelCore.requestModule(root, moduleName, method, args, label, showResult, cacheResult) }

    function requestModuleAsync(moduleName, method, args, label, showResult, callback, acceptResponse) { return AppModelCore.requestModuleAsync(root, moduleName, method, args, label, showResult, callback, acceptResponse) }

    function runtimeOperationStart(request, showResult, callback) { return AppModelCore.runtimeOperationStart(root, request, showResult, callback) }

    function runtimeOperationStatus(operationId, showResult, callback) { return AppModelCore.runtimeOperationStatus(root, operationId, showResult, callback) }

    function runtimeOperationEvents(operationId, afterSeq, showResult, callback) { return AppModelCore.runtimeOperationEvents(root, operationId, afterSeq, showResult, callback) }

    function runtimeOperationCancel(operationId, showResult, callback) { return AppModelCore.runtimeOperationCancel(root, operationId, showResult, callback) }

    function updateRuntimeOperation(operation) { return AppModelCore.updateRuntimeOperation(root, operation) }

    function runtimeOperationTerminal(operation) { return AppModelCore.runtimeOperationTerminal(root, operation) }

    function runtimeOperationResponse(operation) { return AppModelCore.runtimeOperationResponse(root, operation) }

    function appendRuntimeOperationHistory(operation, detail) { return AppModelCore.appendRuntimeOperationHistory(root, operation, detail) }

    function runtimeOperationHistoryRows(domain) { return AppModelCore.runtimeOperationHistoryRows(root, domain) }

    function nodeOperationStart(request, showResult, callback) { return runtimeOperationStart(request, showResult, callback) }

    function nodeOperationStatus(operationId, showResult, callback) { return runtimeOperationStatus(operationId, showResult, callback) }

    function nodeOperationEvents(operationId, afterSeq, showResult, callback) { return runtimeOperationEvents(operationId, afterSeq, showResult, callback) }

    function nodeOperationCancel(operationId, showResult, callback) { return runtimeOperationCancel(operationId, showResult, callback) }

    function updateNodeOperation(operation) { return updateRuntimeOperation(operation) }

    function nodeOperationTerminal(operation) { return runtimeOperationTerminal(operation) }

    function nodeOperationResponse(operation) { return runtimeOperationResponse(operation) }

    function appendNodeOperationHistory(operation, detail) { return appendRuntimeOperationHistory(operation, detail) }

    function nodeOperationHistoryRows(domain) { return runtimeOperationHistoryRows(domain) }

    function appendOperationHistory(operation, detail) { return AppModelCore.appendOperationHistory(root, operation, detail) }

    function operationHistoryRows(domain) { return AppModelCore.operationHistoryRows(root, domain) }

    function bridgeSupportsAsync() { return bridge.hasAsyncCalls() }

    function prefersBasecampModules() { return bridge.prefersBasecampModules() }

    function decodeAccountData(dataHex, idlJson, accountType) { return AppModelCore.decodeAccountData(root, dataHex, idlJson, accountType) }

    function decodeAccountDataAsync(dataHex, idlJson, accountType, callback) { return AppModelCore.decodeAccountDataAsync(root, dataHex, idlJson, accountType, callback) }

    function decodeTransactionSummaryAsync(summary, idlJson, callback) { return AppModelCore.decodeTransactionSummaryAsync(root, summary, idlJson, callback) }

    function resolveAccountDecodeSessionAsync(dataHex, accountId, candidates, callback) { return AppModelCore.resolveAccountDecodeSessionAsync(root, dataHex, accountId, candidates, callback) }

    function selectAccountDecodeSessionAsync(dataHex, accountId, ownerProgramId, candidates, callback) { return AppModelCore.selectAccountDecodeSessionAsync(root, dataHex, accountId, ownerProgramId, candidates, callback) }

    function resolveTransactionDecodeSessionAsync(summary, candidates, callback) { return AppModelCore.resolveTransactionDecodeSessionAsync(root, summary, candidates, callback) }

    function selectTransactionDecodeSessionAsync(summary, candidates, callback) { return AppModelCore.selectTransactionDecodeSessionAsync(root, summary, candidates, callback) }

    function loadIdlState() {
        const response = bridge.callModule(inspectorModule, "loadIdlState", [])
        if (!response.ok || !response.value || typeof response.value !== "object") {
            idlRegistry.loaded = true
            return
        }
        idlRegistry.load(response.value)
        accountIdlSelections = response.value.account_idl_selections && typeof response.value.account_idl_selections === "object"
            ? response.value.account_idl_selections
            : ({})
        accountIdlSelectionRevision += 1
    }

    function saveIdlState() { return AppModelIdentity.saveIdlState(root) }

    function idlStatePayload() {
        return {
            version: 1,
            idls: idlRegistry.entries(),
            account_idl_selections: accountIdlSelections || {}
        }
    }

    function loadSettingsState() { return AppModelIdentity.loadSettingsState(root) }

    function saveSettingsState() { return AppModelIdentity.saveSettingsState(root) }

    function settingsStatePayload() { return AppModelIdentity.settingsStatePayload(root) }

    function socialCommentTopic(layer, entity, id) { return AppModelSocial.socialCommentTopic(root, layer, entity, id) }

    function socialLezAccountIdlTopic(accountId) { return AppModelSocial.socialLezAccountIdlTopic(root, accountId) }

    function socialComments(topic) { return AppModelSocial.socialComments(root, topic) }

    function socialCommentStateForTopic(topic) { return AppModelSocial.socialCommentState(root, topic) }

    function loadSocialComments(topic, reset, pageSize, expectedAccountId) { return AppModelSocial.loadSocialComments(root, topic, reset, pageSize, expectedAccountId) }

    function setSocialCommentState(topic, state) { return AppModelSocial.setSocialCommentState(root, topic, state) }

    function applyIncomingSocialComment(event) { return AppModelSocial.applyIncomingComment(root, event) }

    function applyIncomingSocialMessage(message) { return AppModelSocial.applyIncomingDeliveryMessage(root, message) }

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

    function applySharedIdlPolicy(accountId, entry) { return AppModelSocial.applySharedIdlPolicy(root, accountId, entry) }

    function sharedIdlSuggestions(accountId) { return AppModelSocial.sharedIdlSuggestions(root, accountId) }

    function sharedIdlEntriesForAccount(accountId, ownerProgramId) { return AppModelSocial.sharedIdlEntriesForAccount(root, accountId, ownerProgramId) }

    function publishAccountIdl(accountId, ownerProgramId, idlEntry) { return AppModelSocial.publishAccountIdl(root, accountId, ownerProgramId, idlEntry) }

    function maybeAutoShareAccountIdl(accountId, ownerProgramId, idlEntry) { return AppModelSocial.maybeAutoShareAccountIdl(root, accountId, ownerProgramId, idlEntry) }

    function backupSettingsToStorage(encrypted) { return AppModelIdentity.backupSettingsToStorage(root, encrypted) }

    function restoreSettingsFromStorage(cid, useWallet) { return AppModelIdentity.restoreSettingsFromStorage(root, cid, useWallet) }

    function settingsBackupAvailable() { return AppModelIdentity.settingsBackupAvailable(root) }

    function loadWalletState() {
        const response = bridge.callModule(inspectorModule, "loadWalletState", [])
        wallet.loadPersisted(response && response.ok ? response.value : null)
    }

    function detectWalletProfile(saveDetected) { return wallet.detectProfile(saveDetected) }

    function saveWalletState() { return wallet.savePersisted(networkProfile) }

    function walletStatePayload() { return wallet.payload(networkProfile) }

    function walletProfile() { return wallet.profile(networkProfile) }

    function walletProfileConfigured() { return wallet.profileConfigured() }

    function walletHomeConfigured() { return wallet.homeConfigured() }

    function bedrockWalletSourceConfigured() { return AppModelIdentity.bedrockWalletSourceConfigured(root) }

    function walletProfileUsable() { return wallet.profileUsable() }

    function clearLocalWalletStatus() { return wallet.clearStatus() }

    function walletHomeFallbackLabel() { return wallet.homeFallbackLabel() }

    function walletHomeSourceLabel() { return wallet.homeSourceLabel() }

    function walletBinaryDisplayLabel() { return wallet.binaryDisplayLabel() }

    function walletHomeDisplayLabel() { return wallet.homeDisplayLabel() }

    function redactedPath(path) { return AppModelIdentity.redactedPath(root, path) }

    function storageDisplayPath(path) { return AppModelIdentity.storageDisplayPath(root, path) }

    function updateStorageActiveOperation(value) {
        storageAppState.updateActiveOperation(value)
    }

    function clearStorageActiveOperation() {
        storageAppState.clearActiveOperation()
    }

    function deliveryModuleEventRows() { return deliveryAppState.moduleEventRows() }

    function deliveryModuleEventSummary() { return deliveryAppState.moduleEventSummary() }

    function checkLocalWalletProfile(showResult) { return wallet.checkProfile(showResult) }

    function checkedLocalWalletProfile() { return wallet.checkedProfile() }

    function createWalletAccount() { return wallet.createAccount() }

    function sendWalletTransaction() { return wallet.sendTransaction() }

    function readIncomingWalletTransactions() { return wallet.readIncomingTransactions() }

    function runWalletCommand(commandArgs) { return wallet.runCommand(commandArgs) }

    function walletCommandOperationDetail(value) { return wallet.commandOperationDetail(value) }

    function deployProgramBinary(programPath) { return programExecution.deployProgramBinary(programPath) }

    function deployProgramOperationDetail(value) { return programExecution.deployProgramOperationDetail(value) }

    function syncPrivateWallet() { return wallet.syncPrivate() }

    function queryLocalWalletAccounts(showResult) { return wallet.queryAccounts(showResult) }

    function privateSyncOperationDetail(value) { return wallet.privateSyncOperationDetail(value) }

    function queryBedrockWalletBalance() { return wallet.queryBedrockBalance() }

    function isBedrockHexId(value) { return wallet.isBedrockHexId(value) }

    function appendLocalWalletOperation(label, status, detail) { return wallet.appendHistory(label, status, detail) }

    function previewIdlInstruction(request) { return programExecution.previewIdlInstruction(request) }

    function sendIdlInstruction(request) { return programExecution.sendIdlInstruction(request) }

    function idlInstructionOperationDetail(value) { return programExecution.idlInstructionOperationDetail(value) }

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

    function registeredIdlEntries() { return idlRegistry.entries() }

    function normalizedIdlEntry(entry, fallbackIndex) { return idlRegistry.normalizedEntry(entry, fallbackIndex) }

    function idlEntryAt(index) { return idlRegistry.entryAt(index) }

    function idlNameFromJson(json) { return idlRegistry.nameFromJson(json) }

    function idlKey(name, programId, json) { return idlRegistry.key(name, programId, json) }

    function idlEntryForKey(key) { return idlRegistry.entryForKey(key) }

    function idlEntriesForProgram(programId) { return idlRegistry.entriesForProgram(programId) }

    function cacheAccountIdlSelection(accountId, idlEntry, accountType, ownerProgramId) { return AppModelIdentity.cacheAccountIdlSelection(root, accountId, idlEntry, accountType, ownerProgramId) }

    function accountIdlSelection(accountId, ownerProgramId) { return AppModelIdentity.accountIdlSelection(root, accountId, ownerProgramId) }

    function cachedIdlEntryForAccount(accountId, ownerProgramId) { return ProgramDecodeSession.cachedIdlEntryForAccount(root, accountId, ownerProgramId) }

    function cachedAccountType(accountId, ownerProgramId) { return ProgramDecodeSession.cachedAccountType(root, accountId, ownerProgramId) }

    function accountCacheKey(accountId, ownerProgramId) { return AppModelIdentity.accountCacheKey(root, accountId, ownerProgramId) }

    function accountNetworkCacheScope() { return AppModelIdentity.accountNetworkCacheScope(root) }

    function accountOwnerCacheKey(ownerProgramId) { return AppModelIdentity.accountOwnerCacheKey(root, ownerProgramId) }

    function accountDecodeFullyConsumed(value) { return ProgramDecodeSession.accountDecodeFullyConsumed(root, value) }

    function transactionDecodeFullyConsumed(value) { return ProgramDecodeSession.transactionDecodeFullyConsumed(root, value) }

    function transactionDecodedInstruction(value) { return ProgramDecodeSession.transactionDecodedInstruction(root, value) }

    function transactionSummaryFromDetail(value) { return ProgramDecodeSession.transactionSummaryFromDetail(root, value) }

    function normalizedHexText(value) { return AppModelIdentity.normalizedHexText(root, value) }

    function canonicalProgramIdHex(value) { return AppModelIdentity.canonicalProgramIdHex(root, value) }

    function autoDecodeAccountData(dataHex, accountId, ownerProgramId, callback) { return ProgramDecodeSession.autoDecodeAccountData(root, dataHex, accountId, ownerProgramId, callback) }

    function accountDecodeCandidates(accountId, ownerProgramId) { return ProgramDecodeSession.accountDecodeCandidates(root, accountId, ownerProgramId) }

    function tryAccountDecodeCandidate(serial, dataHex, candidates, index, firstError, callback) { return ProgramDecodeSession.tryAccountDecodeCandidate(root, serial, dataHex, candidates, index, firstError, callback) }

    function autoDecodeTransactionDetail(detail) { return ProgramDecodeSession.autoDecodeTransactionDetail(root, detail) }

    function transactionDecodeCandidates(summary) { return ProgramDecodeSession.transactionDecodeCandidates(root, summary) }

    function candidateListHasEntry(candidates, key) { return ProgramDecodeSession.candidateListHasEntry(root, candidates, key) }

    function tryTransactionDecodeCandidate(serial, summary, candidates, index, partialValue) { return ProgramDecodeSession.tryTransactionDecodeCandidate(root, serial, summary, candidates, index, partialValue) }

    function transactionDecodeSessionReport(response) { return ProgramDecodeSession.transactionDecodeSessionReport(root, response) }

    function transactionDecodeSessionInstruction(response) { return ProgramDecodeSession.transactionDecodeSessionInstruction(root, response) }

    function programDecodeCandidatePayload(candidates) { return ProgramDecodeSession.programDecodeCandidatePayload(root, candidates) }

    function decodeSelectionEntry(selection, candidates) { return ProgramDecodeSession.decodeSelectionEntry(root, selection, candidates) }

    function refreshInterval(seconds) { return AppModelNetwork.refreshInterval(root, seconds) }

    function dashboardRefreshInterval() { return AppModelNetwork.dashboardRefreshInterval(root) }

    function canonicalRefreshRate(seconds) { return AppModelNetwork.canonicalRefreshRate(root, seconds) }

    function loadSourcePolicy() { return sourceRouting.loadSourcePolicy() }

    function sourcePolicyDefault(key, fallback) { return sourceRouting.sourcePolicyDefault(key, fallback) }

    function sourceModePolicy(family, value) { return sourceRouting.sourceModePolicy(family, value) }

    function sourceModePolicies(family) { return sourceRouting.sourceModePolicies(family) }

    function sourceModeOptions(family) { return sourceRouting.sourceModeOptions(family) }

    function sourceModeIndexFor(family, value, options) { return sourceRouting.sourceModeIndexFor(family, value, options) }

    function sourceModeAt(index, options) { return sourceRouting.sourceModeAt(index, options) }

    function sourceModeAdapter(family, value) { return sourceRouting.sourceModeAdapter(family, value) }

    function resolvedSourceModeKey(family, value) { return sourceRouting.resolvedSourceModeKey(family, value) }

    function sourceModeTargetKind(family, value) { return sourceRouting.sourceModeTargetKind(family, value) }

    function sourceModeUsesEndpoint(family, value, endpointKind) { return sourceRouting.sourceModeUsesEndpoint(family, value, endpointKind) }

    function sourceModeSupportsCidProbe(family, value) { return sourceRouting.sourceModeSupportsCidProbe(family, value) }

    function sourceModeSupportsMutatingDiagnostics(family, value) { return sourceRouting.sourceModeSupportsMutatingDiagnostics(family, value) }

    function coreSourceView(role) { return sourceRouting.coreSourceView(role) }

    function deliverySourceView() { return sourceRouting.deliverySourceView() }

    function storageSourceView() { return sourceRouting.storageSourceView() }

    function deliveryReportView(report) { return sourceRouting.deliveryReportView(report) }

    function storageReportView(report) { return sourceRouting.storageReportView(report) }

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

    function deliverySourceReportArgs() { return sourceRouting.deliverySourceReportArgs() }

    function deliverySourceLabel() { return sourceRouting.deliverySourceLabel() }

    function deliverySourceTarget() { return sourceRouting.deliverySourceTarget() }

    function configuredMessagingRestUrl() { return sourceRouting.configuredMessagingRestUrl() }

    function normalizedMessagingSourceMode(value) { return sourceRouting.normalizedMessagingSourceMode(value) }

    function effectiveMessagingSourceMode(value) { return sourceRouting.effectiveMessagingSourceMode(value === undefined ? messagingSourceMode : value) }

    function normalizedCoreSourceMode(value) { return sourceRouting.normalizedCoreSourceMode(value) }

    function effectiveCoreSourceMode(value) { return sourceRouting.effectiveCoreSourceMode(value) }

    function blockchainSourceLabel() { return sourceRouting.blockchainSourceLabel() }

    function blockchainSourceTarget() { return sourceRouting.blockchainSourceTarget() }

    function indexerSourceLabel() { return sourceRouting.indexerSourceLabel() }

    function indexerSourceTarget() { return sourceRouting.indexerSourceTarget() }

    function executionSourceLabel() { return sourceRouting.executionSourceLabel() }

    function executionSourceTarget() { return sourceRouting.executionSourceTarget() }

    function storageSourceReportArgs(includeCidProbe) { return sourceRouting.storageSourceReportArgs(includeCidProbe === true) }

    function storageSourceLabel() { return sourceRouting.storageSourceLabel() }

    function storageSourceTarget() { return sourceRouting.storageSourceTarget() }

    function configuredStorageRestUrl() { return sourceRouting.configuredStorageRestUrl() }

    function normalizedStorageSourceMode(value) { return sourceRouting.normalizedStorageSourceMode(value) }

    function effectiveStorageSourceMode(value) { return sourceRouting.effectiveStorageSourceMode(value === undefined ? storageSourceMode : value) }

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

    function normalizedNetworkProfile(value) { return networkProfileState.normalizedProfile(value) }

    function resolvedNetworkProfile(storedProfile, sequencer, indexer, node) { return networkProfileState.resolvedProfile(storedProfile, sequencer, indexer, node) }

    function inferNetworkProfileFromEndpoints(sequencer, indexer, node) { return networkProfileState.inferProfile(sequencer, indexer, node) }

    function normalizeEndpoint(value) { return networkProfileState.normalizeEndpoint(value) }

    function loadNetworkProfileSettings(value) {
        const settings = networkProfileState.settingsFromPayload(value, networkProfile, sequencerUrl, indexerUrl, nodeUrl)
        networkProfile = settings.profile
        sequencerUrl = settings.sequencerUrl
        indexerUrl = settings.indexerUrl
        nodeUrl = settings.nodeUrl
    }

    function networkProfileSettingsPayload() { return networkProfileState.settingsPayload(networkProfile, sequencerUrl, indexerUrl, nodeUrl) }

    function networkProfileOptions() { return networkProfileState.optionRows() }

    function profileIndexFor(value) { return networkProfileState.profileIndex(value) }

    function profileIndex() { return profileIndexFor(networkProfile) }

    function applyProfileIndex(index) { return applyProfile(index) }

    function applyProfile(index) {
        const profile = networkProfileState.profileAt(index)
        if (profile === "custom") {
            networkProfile = inferNetworkProfileFromEndpoints(sequencerUrl, indexerUrl, nodeUrl)
            return
        }
        const endpoints = networkProfileState.applyProfile(profile)
        if (!endpoints) {
            return
        }
        networkProfile = endpoints.profile
        sequencerUrl = endpoints.sequencerUrl
        indexerUrl = endpoints.indexerUrl
        nodeUrl = endpoints.nodeUrl
        messagingNetworkPreset = "logos.test"
    }

    function networkProfileLabel(value) { return networkProfileState.profileLabel(value) }

    function networkProfileSummary(value) { return networkProfileState.profileSummary(value) }

    function networkProfileDetail() { return networkProfileState.profileDetail(sequencerUrl, indexerUrl, nodeUrl) }

    function networkProfileCacheScope() { return networkProfileState.cacheScope(networkProfile, sequencerUrl) }

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

    function refreshDashboard() { return entityNavigation.refreshDashboard() }

    function updateDashboardCache(method, value) { return entityNavigation.updateDashboardCache(method, value) }

    function routeSearch(query) { return entityNavigation.routeSearch(query) }

    function openStorageCid(cid) { return entityNavigation.openStorageCid(cid) }

    function isStorageCid(value) { return entityNavigation.isStorageCid(value) }

    function numericSearchUsesLezBlock() { return entityNavigation.numericSearchUsesLezBlock() }

    function routePrefixedSearch(query) { return entityNavigation.routePrefixedSearch(query) }

    function searchPrefix(query) { return entityNavigation.searchPrefix(query) }

    function isSearchPrefix(prefix) { return entityNavigation.isSearchPrefix(prefix) }

    function routeModuleSearchTarget(target) { return entityNavigation.routeModuleSearchTarget(target) }

    function resolveSearchHash(hash) { return entityNavigation.resolveSearchHash(hash) }

    function applyResolvedLezTarget(response, errorTitle) { return entityNavigation.applyResolvedLezTarget(response, errorTitle) }

    function resolveSearchTransaction(serial, hash, recordHistory) { return entityNavigation.resolveSearchTransaction(serial, hash, recordHistory) }

    function resolveSearchAccount(serial, account, recordHistory) { return entityNavigation.resolveSearchAccount(serial, account, recordHistory) }

    function viewKeyForQuery(query) { return entityNavigation.viewKeyForQuery(query) }

    function settingsTargetForQuery(query) { return entityNavigation.settingsTargetForQuery(query) }

    function openReference(kind, value, payload) { return entityNavigation.openReference(kind, value, payload) }

    function openMantleTransaction(hash) { return entityNavigation.openMantleTransaction(hash) }

    function openAccount(account) { return entityNavigation.openAccount(account) }

    function openPrivateAccountReference(account) { return entityNavigation.openPrivateAccountReference(account) }

    function openTransaction(hash) { return entityNavigation.openTransaction(hash) }

    function openLezSearchTarget(target) { return entityNavigation.openLezSearchTarget(target) }

    function openLezBlock(blockId) { return entityNavigation.openLezBlock(blockId) }

    function resolveLezHash(hash) { return entityNavigation.resolveLezHash(hash) }

    function openLezTransaction(hash, recordHistory) { return entityNavigation.openLezTransaction(hash, recordHistory) }

    function inspectTransaction(hash, idl, recordHistory) { return entityNavigation.inspectTransaction(hash, idl, recordHistory) }

    function openBlockchainBlock(blockOrId) { return entityNavigation.openBlockchainBlock(blockOrId) }

    function loadBlockchainBlockById(blockId) { return entityNavigation.loadBlockchainBlockById(blockId) }

    function loadBlockchainBlockBySlot(slot) { return entityNavigation.loadBlockchainBlockBySlot(slot) }

    function openBlockchainTransaction(transaction, block) { return entityNavigation.openBlockchainTransaction(transaction, block) }

    function transactionDetail(hash) { return entityNavigation.transactionDetail(hash) }

    function blockchainTransactionDetail(value, fallbackHash) { return entityNavigation.blockchainTransactionDetail(value, fallbackHash) }

    function openIndexerBlock(headerHash, payload) { return entityNavigation.openIndexerBlock(headerHash, payload) }

    function indexerBlockDetail(value, source) { return entityNavigation.indexerBlockDetail(value, source) }

    function openLocalWallet(wallet, tab) { return entityNavigation.openLocalWallet(wallet, tab) }

    function showLocalWalletRequired(wallet) { return entityNavigation.showLocalWalletRequired(wallet) }

    function openProgram(programId) { return entityNavigation.openProgram(programId) }

    function programContextDetail(programId) { return entityNavigation.programContextDetail(programId) }

    function programContextFromParts(input, normalized, knownRow, accountResponse, lookupError) { return entityNavigation.programContextFromParts(input, normalized, knownRow, accountResponse, lookupError) }

    function knownProgramRow(programId) { return entityNavigation.knownProgramRow(programId) }

    function programRecentTransactions(programId) { return entityNavigation.programRecentTransactions(programId) }

    function looksLikeHexId(value) { return entityNavigation.looksLikeHexId(value) }

    function openRecipient(recipient) { return entityNavigation.openRecipient(recipient) }

    function openChannel(channel) { return entityNavigation.openChannel(channel) }

    function programIdKnown(programId) { return AppModelRegistry.programIdKnown(root, programId) }

    function knownProgramCacheScope() { return networkProfileCacheScope() }

    function knownProgramIdRows() { return AppModelRegistry.knownProgramIdRows(root) }

    function updateKnownProgramIds(value) { return AppModelRegistry.updateKnownProgramIds(root, value) }

    function registerIdl(name, programId, json, programBinary) { return AppModelRegistry.registerIdl(root, name, programId, json, programBinary) }

    function removeIdl(index) { return AppModelRegistry.removeIdl(root, index) }

    function clearDashboardMetricHistoryForPrefix(prefix) { return AppModelMetrics.clearDashboardMetricHistoryForPrefix(root, prefix) }
}
