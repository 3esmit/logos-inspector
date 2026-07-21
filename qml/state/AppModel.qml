import QtQuick
import "../services"
import "app/AppModelCore.js" as AppModelCore
import "domains" as Domains
import "identity/AppModelIdentity.js" as AppModelIdentity
import "network/AppModelNetwork.js" as AppModelNetwork
import "programs" as Programs
import "programs/AppModelRegistry.js" as AppModelRegistry
import "programs/ProgramDecodeSession.js" as ProgramDecodeSession
import "wallet" as Wallet

QtObject {
    id: root

    required property BridgeClient bridge

    readonly property string inspectorModule: "logos_inspector"
    readonly property string blockchainModule: "blockchain_module"
    readonly property string storageModule: "storage_module"
    readonly property string deliveryModule: "delivery_module"
    readonly property string capabilityModule: "capability_module"
    property bool attachedRuntimeObservationRefreshQueued: false
    property Domains.SourceRoutingState sourceRouting: Domains.SourceRoutingState {
        id: sourceRoutingState

        blockchainModule: root.blockchainModule
        deliveryModule: root.deliveryModule
        storageModule: root.storageModule
        blockchainSourceMode: root.blockchainSourceMode
        messagingSourceMode: root.messagingSourceMode
        storageSourceMode: root.storageSourceMode
        nodeUrl: root.nodeUrl
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
        connectorConfig: root.networkConnectorConfig
        gateway: QtObject {
            function callInspector(method, args) {
                return root.bridge.callModule(root.inspectorModule, method, Array.isArray(args) ? args : [])
            }

            function prefersBasecampModules() {
                return root.prefersBasecampModules()
            }

        }
    }
    property Domains.ZoneInspectionState zoneInspection: Domains.ZoneInspectionState {
        id: zoneInspectionState

        appModel: root
        sourceDescriptor: root.zoneCatalogL1SourceDescriptor()
        gateway: QtObject {
            function request(method, args, callback) {
                return root.requestModuleAsync(
                    root.inspectorModule,
                    method,
                    Array.isArray(args) ? args : [],
                    "",
                    false,
                    callback
                )
            }
        }
    }
    property Domains.NetworkProfileState networkProfiles: Domains.NetworkProfileState {
        id: networkProfileState
        sourcePolicy: root.sourceRouting.sourcePolicy
    }
    readonly property var storageSource: sourceRouting.storageSourceView()
    readonly property var deliverySource: sourceRouting.deliverySourceView()

    property AppShellState shell: AppShellState {
        id: appShellState
        model: root
    }
    property var navigationGuard: null
    property Domains.MetricsState metrics: Domains.MetricsState {
        id: metricsState

        sourceRouting: sourceRoutingState
        inspectorModule: root.inspectorModule
        nodeUrl: root.nodeUrl
        storageRollingWindow: root.storageRollingWindow
        messagingRollingWindow: root.messagingRollingWindow
        dashboardOverview: chainPageState.dashboardOverview
        dashboardNode: chainPageState.dashboardNode
        dashboardL1Blocks: chainPageState.dashboardL1Blocks
        dashboardL1BlocksSlotTo: chainPageState.dashboardL1BlocksSlotTo
        dashboardBlocks: chainPageState.dashboardBlocks
        dashboardProvisionalBlocks: chainPageState.dashboardProvisionalBlocks
        gateway: QtObject {
            function requestModuleAsyncUnobserved(moduleName, method, args, label,
                    showResult, callback, acceptResponse) {
                return appRequestState.requestModuleAsyncUnobserved(
                    moduleName,
                    method,
                    args,
                    label,
                    showResult,
                    callback,
                    acceptResponse
                )
            }

            function startBlockchainObservation(showResult, request, callback) {
                if (showResult === true) {
                    return chainPageState.presentOperation(
                        "network.blockchain",
                        request.method,
                        request.args,
                        request.label,
                        appShellState.currentView,
                        callback
                    )
                }
                return chainPageState.startOperation(
                    "network.blockchain",
                    request.method,
                    request.args,
                    request.label,
                    callback
                )
            }

            function startDashboardBlockchainOperation(request, callback) {
                return chainPageState.startOperation(
                    "dashboard.live",
                    request.method,
                    request.args,
                    request.label,
                    callback
                )
            }

            function beginObservationPresentation(label, owner) {
                return appRequestState.beginPresentation(
                    String(label || ""),
                    owner === undefined
                        ? appShellState.currentView : String(owner || "")
                )
            }

            function completeObservationPresentation(lease, title, text,
                    isError, value) {
                return appRequestState.completePresentation(
                    lease,
                    title,
                    text,
                    isError === true,
                    value
                )
            }

            function cacheBlockchainResult(method, value, slotTo) {
                if (method === "blockchainLiveBlocks") {
                    chainPageState.dashboardL1Blocks = value && Array.isArray(value.blocks)
                        ? value.blocks : []
                    const anchor = Number(slotTo || 0)
                    chainPageState.dashboardL1BlocksSlotTo =
                        Number.isSafeInteger(anchor) && anchor > 0 ? anchor : 0
                    return
                }
                if (method !== "blockchainNode") {
                    return
                }
                chainPageState.dashboardNode = value || null
                const probe = value && value.cryptarchia_info
                    ? value.cryptarchia_info : null
                const overview = root.copyMap(chainPageState.dashboardOverview || {})
                const node = root.copyMap(overview.node || {})
                if (probe) {
                    node.consensus = {
                        ok: probe.ok === true,
                        value: probe.value === undefined ? null : probe.value,
                        error: probe.error === undefined ? null : probe.error
                    }
                }
                node.endpoint = root.nodeUrl
                overview.node = node
                chainPageState.dashboardOverview = overview
            }

            function clearBlockchainObservation() {
                chainPageState.dashboardNode = null
                const overview = root.copyMap(chainPageState.dashboardOverview || {})
                delete overview.node
                chainPageState.dashboardOverview = Object.keys(overview).length > 0
                    ? overview : null
            }

            function projectZoneDashboard() {
                return entityNavigationState.projectZoneDashboard()
            }

            function resetDashboardProjection() {
                chainPageState.dashboardOverview = null
                chainPageState.dashboardNode = null
                chainPageState.dashboardL1Blocks = []
                chainPageState.dashboardL1BlocksSlotTo = 0
                chainPageState.dashboardBlocks = []
                chainPageState.dashboardProvisionalBlocks = []
                chainPageState.dashboardLezBlockRows = []
                chainPageState.dashboardChannelStatuses = []
            }

            function invalidateDashboardOperations(reason) {
                chainPageState.invalidateOperationCaller("dashboard.node", reason)
                chainPageState.invalidateOperationCaller("dashboard.live", reason)
            }

            function setDashboardResult(ok, text, value) {
                appShellState.setResult(qsTr("Dashboard"), text, ok !== true, value)
            }

            function refreshCapabilityRegistryIfLoaded() {
                return root.refreshCapabilityRegistryIfLoaded()
            }

            function dashboardGate(key) {
                return root.dashboardGate(key)
            }
        }
    }
    property AppRequestState requests: AppRequestState {
        id: appRequestState

        bridge: root.bridge
        shell: appShellState
        inspectorModule: root.inspectorModule
        projectObservationResponse: function (method, response, cacheResult) {
            return metricsState.projectResponse(method, response, cacheResult)
        }
    }
    readonly property bool asyncPresentationBusy: appRequestState.presentationBusy
    property Domains.NetworkInspectionState chainPages: Domains.NetworkInspectionState {
        id: chainPageState

        inspectorModule: root.inspectorModule
        capabilityFacade: root.capabilities
        configurationGeneration: root.blockchainConfigurationRevision
        gateway: QtObject {
            function startRuntimeOperation(request, showResult, callback) {
                return root.runtimeOperationStart(request, showResult === true, callback)
            }

            function runtimeOperationStatus(operationId, showResult, callback) {
                return root.runtimeOperationStatus(operationId, showResult === true, callback)
            }

            function runtimeOperationCancel(operationId, showResult, callback) {
                return root.runtimeOperationCancel(operationId, showResult === true, callback)
            }

            function appendOperationHistory(operation, detail) {
                return root.appendOperationHistory(operation, detail)
            }

            function beginPresentation(label, owner) {
                return appRequestState.beginPresentation(label, owner)
            }

            function completePresentation(lease, title, text, isError, value) {
                return appRequestState.completePresentation(lease, title, text, isError, value)
            }

            function abandonPresentation(lease) {
                return appRequestState.abandonPresentation(lease)
            }

            function setResult(title, text, isError, value, owner) {
                return appShellState.setResult(title, text, isError, value, owner)
            }

            function blockchainArgs(extra) { return root.sourceRouting.blockchainArgs(extra) }

            function blockchainRpcArgs(extra) { return root.blockchainRpcArgs(extra) }

            function networkConnectionState(kind) { return metricsState.networkConnectionState(kind) }

            function valueToString(value) { return metricsState.valueToString(value) }

            function canonicalProgramIdHex(value) { return root.canonicalProgramIdHex(value) }

            function normalizedHexText(value) { return root.normalizedHexText(value) }
        }
    }
    property alias dashboardOverview: chainPageState.dashboardOverview
    property alias dashboardNode: chainPageState.dashboardNode
    property alias dashboardL1Blocks: chainPageState.dashboardL1Blocks
    property alias dashboardL1BlocksSlotTo: chainPageState.dashboardL1BlocksSlotTo
    property alias dashboardBlocks: chainPageState.dashboardBlocks
    property alias dashboardProvisionalBlocks: chainPageState.dashboardProvisionalBlocks
    property alias dashboardLezBlockRows: chainPageState.dashboardLezBlockRows
    property alias dashboardChannelStatuses: chainPageState.dashboardChannelStatuses
    property alias blockDetailValue: chainPageState.blockDetailValue
    property alias blockDetailError: chainPageState.blockDetailError
    property alias transactionDetailValue: chainPageState.transactionDetailValue
    property alias transactionDetailError: chainPageState.transactionDetailError
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
    property alias transactionsPageAtLatest: chainPageState.transactionsPageAtLatest
    property alias transactionsPageBlockBatch: chainPageState.transactionsPageBlockBatch
    property alias transactionsPageLimit: chainPageState.transactionsPageLimit
    property alias transactionsPageError: chainPageState.transactionsPageError
    property string networkProfile: "default"
    property string nodeUrl: "http://127.0.0.1:8080/"
    property var networkConnectorConfig: defaultNetworkConnectorConfig()
    property string blockchainSourceMode: "rpc"
    readonly property string blockchainConfigurationSignature: JSON.stringify([
        networkProfile,
        nodeUrl,
        blockchainSourceMode,
        networkConnectorConfig && networkConnectorConfig.scopes
            ? networkConnectorConfig.scopes.l1 || null : null
    ])
    property string messagingSourceMode: "rest"
    property string messagingRestUrl: "http://127.0.0.1:8645"
    property string messagingMetricsUrl: "http://127.0.0.1:8008/metrics"
    property string messagingNetworkPreset: "logos.test"
    property int messagingRollingWindow: 120
    property bool messagingAdminRestEnabled: false
    readonly property bool messagingMutatingDiagnosticsEnabled: true
    property Domains.SocialCollaborationState social: Domains.SocialCollaborationState {
        id: socialState

        bridge: root.bridge
        inspectorModule: root.inspectorModule
        sourceRouting: sourceRoutingState
        registeredIdls: programDecodeState.registeredIdls
        busy: appShellState.busy
        messagingSourceMode: root.messagingSourceMode
        messagingMutatingDiagnosticsEnabled: root.messagingMutatingDiagnosticsEnabled
        storageMutatingDiagnosticsEnabled: root.storageMutatingDiagnosticsEnabled
        gateway: QtObject {
            function requestModuleAsync(moduleName, method, args, label, showResult, callback, acceptResponse) {
                return root.requestModuleAsync(moduleName, method, args, label, showResult, callback, acceptResponse)
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

            function saveSettingsState() { return root.saveSettingsState() }
            function saveIdlState() { return root.saveIdlState() }
            function socialGate(action) { return root.socialGate(action) }
            function normalizedIdlEntry(entry, fallbackIndex) { return root.normalizedIdlEntry(entry, fallbackIndex) }
            function idlEntryForKey(key) { return root.idlEntryForKey(key) }
            function zoneAccountEntityRef(accountId) {
                return zoneInspectionState.l2.l2EntityRef("account", accountId, null)
            }
            function idlNameFromJson(value) { return root.idlNameFromJson(value) }
            function canonicalProgramIdHex(value) { return root.canonicalProgramIdHex(value) }
            function normalizedHexText(value) { return root.normalizedHexText(value) }
            function accountOwnerCacheKey(value) { return root.accountOwnerCacheKey(value) }
            function zoneScopeKey() { return root.zoneScopeKey() }
        }
    }
    property string storageSourceMode: "rest"
    property string storageRestUrl: "http://127.0.0.1:8080/api/storage/v1"
    property string storageMetricsUrl: "http://127.0.0.1:8008/metrics"
    property string storageNetworkPreset: "logos.test"
    property int storageRollingWindow: 120
    property bool storageLocalDiagnosticsEnabled: false
    property bool storagePrivilegedDebugEnabled: false
    readonly property bool storageMutatingDiagnosticsEnabled: true
    property bool localNodesEnabled: true
    property bool localDevnetEnabled: false
    property Domains.OperationHistoryState operationHistory: Domains.OperationHistoryState {
        id: operationHistoryState
    }
    property alias runtimeOperations: operationHistoryState.runtimeOperations
    property alias runtimeOperationEventSeq: operationHistoryState.runtimeOperationEventSeq
    property alias runtimeOperationHistory: operationHistoryState.runtimeOperationHistory
    property alias runtimeOperationsRevision: operationHistoryState.runtimeOperationsRevision
    property string settingsBackupCid: ""
    property string settingsRestoreCid: ""
    property bool settingsBackupEncrypted: false
    property string settingsBackupStatus: ""
    property var settingsBackupContents: defaultSettingsBackupContents()
    property Domains.BackupCatalogState backupCatalog: Domains.BackupCatalogState {
        id: backupCatalogState

        storageAdapterInitialization: root.sourceRouting.storageOperationAdapter()
        storageMutatingDiagnosticsEnabled: root.storageMutatingDiagnosticsEnabled
        gateway: QtObject {
            function call(method, args, label) {
                return root.callInspector(method, args || [], label)
            }

            function supportsAsync() {
                return root.bridgeSupportsAsync()
            }

            function request(method, args, label, showResult, callback) {
                return root.requestModuleAsync(
                    root.inspectorModule,
                    method,
                    args || [],
                    label,
                    showResult === true,
                    callback
                )
            }

            function startRuntimeOperation(request, showResult, callback) {
                return root.runtimeOperationStart(request, showResult === true, callback)
            }

            function runtimeOperationStatus(operationId, showResult, callback) {
                return root.runtimeOperationStatus(operationId, showResult === true, callback)
            }

            function appendOperationHistory(operation, detail) {
                root.appendOperationHistory(operation, detail)
            }
        }
    }
    property alias backupCatalogEntries: backupCatalogState.entries
    property alias backupCatalogLoaded: backupCatalogState.loaded
    property alias backupCatalogError: backupCatalogState.error
    property alias backupCatalogRevision: backupCatalogState.revision
    property alias backupCatalogUploadRunning: backupCatalogState.uploadRunning
    property alias backupCatalogDownloadRunning: backupCatalogState.downloadRunning
    property alias backupCatalogTransferRunning: backupCatalogState.transferRunning
    property alias backupCatalogImportRunning: backupCatalogState.importRunning
    property Domains.BackupImportState backupImport: Domains.BackupImportState {
        id: backupImportState

        model: root
        catalog: backupCatalogState
        operationHistory: operationHistoryState
    }

    property string programTab: "idls"
    property string localWalletTab: "profiles"
    property string localWalletLookupTarget: ""
    property alias settingsSection: appShellState.settingsSection
    property alias settingsNetworkSection: appShellState.settingsNetworkSection
    property alias settingsUiSection: appShellState.settingsUiSection
    property int networkConfigurationRevision: 0
    property int blockchainConfigurationRevision: 0
    property int blockchainModuleEventRevision: 0
    property string blockchainLastEventText: ""

    property Domains.ProgramDecodeState programDecode: Domains.ProgramDecodeState {
        id: programDecodeState

        capabilityFacade: root.capabilities
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
    property alias idlStateLoaded: programDecodeState.loaded
    property LocalWalletAppState wallet: LocalWalletAppState {
        id: walletState

        gateway: QtObject {
            function call(method, args) {
                return root.bridge.callModule(root.inspectorModule, method, args || [])
            }

            function request(method, args, label, showResult, callback, acceptResponse) {
                return root.requestModuleAsync(root.inspectorModule, method, args || [], label, showResult === true, callback, acceptResponse)
            }

            function setStatus(value) {
                appShellState.statusText = String(value || "")
            }

            function busy() {
                return appShellState.busy
            }

            function setBusy(value) {
                appShellState.busy = value === true
            }

            function setResult(title, text, isError, value) {
                appShellState.setResult(title, text, isError, value)
            }

            function openLocalWallet(wallet, tab) {
                return entityNavigationState.openLocalWallet(wallet, tab)
            }

            function networkProfile() {
                return root.networkProfile
            }

            function prefersBasecampModules() {
                return root.prefersBasecampModules()
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
    property alias walletConnectorConfig: walletState.connectorConfig
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

            function request(method, args, label, showResult, callback, acceptResponse) {
                return root.requestModuleAsync(
                    root.inspectorModule, method, args, label,
                    showResult === true, callback, acceptResponse)
            }

            function startRuntimeOperation(request, showResult, callback) {
                return root.runtimeOperationStart(request, showResult === true, callback)
            }

            function runtimeOperationStatus(operationId, showResult, callback) {
                return root.runtimeOperationStatus(operationId, showResult === true, callback)
            }

            function runtimeOperationCancel(operationId, showResult, callback) {
                return root.runtimeOperationCancel(operationId, showResult === true, callback)
            }

            function runtimeOperationModuleEvent(event, showResult, callback) {
                return root.runtimeOperationModuleEvent(event, showResult === true, callback)
            }

            function refreshStorageObservations(cid) {
                metricsState.queryStorageAfterMutation(cid)
                return root.storageApp.refreshManifests(false)
            }

            function observeStorage(callback) {
                return metricsState.observeNetworkConnection(
                    "storage", false, false, callback, "source-inspection")
            }

            function setResult(title, text, isError, value, owner) {
                return appShellState.setResult(
                    title, text, isError, value, String(owner || "storage"))
            }

            function clearResult() {
                return appShellState.clearResult()
            }

            function appendOperationHistory(operation, detail) {
                return root.appendOperationHistory(operation, detail)
            }

            function openSettings(section, subSection) {
                return root.openSettings(section, subSection)
            }

            function valueText(value) {
                return metricsState.valueText(value)
            }
        }
        busy: appShellState.busy
        sourceMode: root.storageSource.mode
        effectiveSourceMode: root.storageSource.effectiveMode
        sourceLabel: root.storageSource.label
        sourceTarget: root.storageSource.target
        sourceTargetKind: root.storageSource.targetKind
        usesRestEndpoint: root.storageSource.usesRestEndpoint
        supportsMutatingDiagnostics: root.storageSource.supportsMutatingDiagnostics
        restEndpoint: root.storageSource.restEndpoint
        adapterInitialization: root.sourceRouting.storageOperationAdapter()
        moduleName: root.storageSource.moduleName
        networkPreset: root.storageSource.networkPreset
        mutatingDiagnosticsEnabled: root.storageMutatingDiagnosticsEnabled
        currentView: appShellState.currentView
        resultTitle: appShellState.resultTitle
        resultText: appShellState.resultText
        resultIsError: appShellState.resultIsError
        resultOwner: appShellState.resultOwner
        sourceReport: metricsState.sourceReport("storage")
        gateFacade: root.capabilities
    }
    property alias storageAppTab: storageAppState.currentTab
    property alias storageCidProbe: storageAppState.cidProbe
    property string storageDiagnosticsTab: "overview"
    property DeliveryAppState deliveryApp: DeliveryAppState {
        id: deliveryAppState

        gateway: QtObject {
            function startRuntimeOperation(request, showResult, callback) {
                return root.runtimeOperationStart(request, showResult === true, callback)
            }

            function runtimeOperationStatus(operationId, showResult, callback) {
                return root.runtimeOperationStatus(operationId, showResult === true, callback)
            }

            function runtimeOperationCancel(operationId, showResult, callback) {
                return root.runtimeOperationCancel(operationId, showResult === true, callback)
            }

            function runtimeOperationModuleEvent(event, showResult, callback) {
                return root.runtimeOperationModuleEvent(event, showResult === true, callback)
            }

            function appendOperationHistory(operation, detail) {
                return root.appendOperationHistory(operation, detail)
            }

            function setResult(title, text, isError, value) {
                return appShellState.setResult(title, text, isError, value, "messaging")
            }
        }
        busy: appShellState.busy
        sourceMode: root.deliverySource.mode
        effectiveSourceMode: root.deliverySource.effectiveMode
        sourceLabel: root.deliverySource.label
        sourceTarget: root.deliverySource.target
        sourceTargetKind: root.deliverySource.targetKind
        usesRestEndpoint: root.deliverySource.usesRestEndpoint
        supportsMutatingDiagnostics: root.deliverySource.supportsMutatingDiagnostics
        restEndpoint: root.deliverySource.restEndpoint
        adapterInitialization: root.sourceRouting.deliveryOperationAdapter()
        moduleName: root.deliverySource.moduleName
        networkPreset: root.deliverySource.networkPreset
        mutatingDiagnosticsEnabled: root.messagingMutatingDiagnosticsEnabled
        managedNodes: localNodesState
    }
    property alias deliveryAppTab: deliveryAppState.currentTab
    property string deliveryDiagnosticsTab: "overview"
    property alias deliveryActiveTopic: deliveryAppState.activeTopic
    property alias deliveryModuleEvents: deliveryAppState.deliveryModuleEvents
    property alias deliveryModuleEventRevision: deliveryAppState.deliveryModuleEventRevision
    property alias deliveryConnectionStatus: deliveryAppState.deliveryConnectionStatus
    property alias deliveryNodeStatus: deliveryAppState.deliveryNodeStatus
    property Domains.CapabilityGateState capabilities: Domains.CapabilityGateState {
        id: capabilityGateState

        gateway: QtObject {
            function callInspector(method, args) {
                return root.bridge.callModule(root.inspectorModule, method, Array.isArray(args) ? args : [])
            }
        }
        compatibilityAvailability: root.capabilityLocalAvailability()
    }
    property alias capabilityRegistryReport: capabilityGateState.registryReport
    property alias capabilityRegistryLoaded: capabilityGateState.registryLoaded
    property alias capabilityRegistryError: capabilityGateState.registryError
    property Wallet.WalletCapabilityFacade walletCapability: Wallet.WalletCapabilityFacade {
        id: walletCapabilityFacade

        wallet: root.wallet
        capabilityFacade: root.capabilities
        networkProfile: root.networkProfile
        prefersBasecampModules: root.prefersBasecampModules() === true
        gateway: QtObject {
            function openLocalWallet(tab) {
                return entityNavigationState.openLocalWallet("", tab)
            }
        }
    }
    property Domains.StatusFacadeState statusFacade: Domains.StatusFacadeState {
        id: statusFacadeState

        capabilityFacade: root.capabilities
        operationHistory: root.operationHistory
        reports: ({
            blockchain: metricsState.moduleReport("blockchain"),
            storage: metricsState.moduleReport("storage"),
            delivery: metricsState.moduleReport("messaging"),
            storage_source: metricsState.sourceReport("storage"),
            delivery_source: metricsState.sourceReport("messaging")
        })
        events: ({
            blockchain_revision: root.blockchainModuleEventRevision,
            delivery_revision: root.deliveryModuleEventRevision
        })
    }
    property LocalNodesState localNodes: LocalNodesState {
        id: localNodesState

        gateway: QtObject {
            function request(method, args, label, showResult, callback) {
                return root.requestModuleAsync(root.inspectorModule, method, args, label, showResult, callback)
            }

            function setBusy(value, label) {
                appShellState.busy = value === true
                const labelText = String(label || "")
                if (appShellState.busy && labelText.length) {
                    appShellState.statusText = labelText
                }
            }

            function setResult(title, text, isError, value) {
                return appShellState.setResult(title, text, isError, value)
            }

            function appendOperationHistory(operation, detail) {
                return root.appendOperationHistory(operation, detail)
            }

            function invalidateAttachedRuntimeObservations() {
                return root.invalidateAttachedRuntimeObservations()
            }

            function activateLocalProfile() {
                root.localNodesEnabled = true
                root.localDevnetEnabled = true
                root.applyProfileIndex(root.profileIndexFor("local"))
                root.saveSettingsState()
                return root.networkProfile === "local"
            }
        }
        networkProfile: root.networkProfile
        busy: appShellState.busy
        observedNodes: root.localNodeObservedNodes()
    }
    property alias localNodesReport: localNodesState.report
    property alias localNodesError: localNodesState.error
    property alias localNodesOperations: localNodesState.operations
    property alias localNodesRevision: localNodesState.revision
    property alias localDevnets: localNodesState.devnets
    property Programs.ProgramExecutionState programExecution: Programs.ProgramExecutionState {
        id: programExecutionState

        capabilityFacade: root.capabilities
        walletCapability: root.walletCapability
        gateway: QtObject {
            function request(method, args, label, showResult, callback) {
                return root.requestModuleAsync(root.inspectorModule, method, args || [], label, showResult === true, callback)
            }

            function busy() {
                return appShellState.busy
            }

            function setBusy(value) {
                appShellState.busy = value === true
            }

            function setStatus(value) {
                appShellState.statusText = String(value || "")
            }

            function setResult(title, text, isError, value) {
                return appShellState.setResult(title, text, isError, value)
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
                return entityNavigationState.openLocalWallet("", tab)
            }

            function appendOperationHistory(operation, detail) {
                return root.appendOperationHistory(operation, detail)
            }

            function activeZoneContext() {
                return root.zoneInspection.activeZoneContext
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
    property alias navExpanded: appShellState.navExpanded
    property alias navRevision: appShellState.navRevision
    property alias zoneMenuSelections: appShellState.zoneMenuSelections
    property alias zoneMenuRevision: appShellState.zoneMenuRevision
    property alias navigationBackStack: appShellState.navigationBackStack
    property alias navigationForwardStack: appShellState.navigationForwardStack
    property alias navigationRevision: appShellState.navigationRevision
    property alias navigationRestoring: appShellState.navigationRestoring
    readonly property alias navigationHistoryLimit: appShellState.navigationHistoryLimit
    property var pendingInspectionEntityRef: null
    property var currentInspectionEntityRef: null
    property EntityNavigationSession entityNavigation: EntityNavigationSession {
        id: entityNavigationState
        model: root
    }
    property FavoritesState favoriteStore: FavoritesState {
        onRevisionChanged: root.saveSettingsState()
        onOpenRequested: function (openKind, value, entityRef,
                navigationContext) {
            if (entityRef) {
                entityNavigationState.openInspectionEntityRef(entityRef, true)
            } else {
                entityNavigationState.openReference(openKind, value,
                    navigationContext)
            }
        }
    }

    property Connections programExecutionConnections: Connections {
        target: programExecutionState

        function onIdlInstructionSubmitted(response, backendTarget) {
            root.openSubmittedIdlInstruction(response, backendTarget)
        }
    }

    property Connections zoneInspectionConnections: Connections {
        target: zoneInspectionState

        function onActiveZoneContextChanged() {
            root.currentInspectionEntityRef = null
            root.knownProgramIds = ({})
            root.knownProgramIdsRevision += 1
            root.shell.navRevision += 1
            metricsState.clearDashboardMetricHistoryForPrefixes([
                "lez.blocks_produced_recent",
                "indexer."
            ])
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
            entityNavigationState.projectZoneDashboard()
        }

        function onZoneDetailChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
            entityNavigationState.projectZoneDashboard()
        }

        function onZoneSummariesChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
            entityNavigationState.projectZoneDashboard()
        }

        function onCatalogConfiguredChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }

        function onStartedChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }

        function onDesiredSourceKeyChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }

        function onCatalogStatusChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }

        function onVerificationChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }

        function onNetworkScopeChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }

        function onConfigureInFlightChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }

        function onStatusInFlightChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }

        function onSummaryInFlightChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }

        function onControlInFlightChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }

        function onAutomaticRetryPendingChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }

        function onConfigureErrorChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }

        function onStatusErrorChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }

        function onSummaryErrorChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }

        function onCurrentErrorChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }

        function onSummaryStaleChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }

        function onDetailInFlightChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }

        function onDetailStaleChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }

        function onDetailErrorChanged() {
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }
    }

    property Connections zoneL2NavigationConnections: Connections {
        target: zoneInspectionState.l2

        function onL2SequencerReadEnabledChanged() {
            root.shell.navRevision += 1
            Qt.callLater(entityNavigationState.resumePendingInspectionEntityRef)
        }
    }

    property Connections zoneL2BlockConnections: Connections {
        target: zoneInspectionState.l2.blocks

        function onL2BlockRowsChanged() {
            entityNavigationState.projectZoneDashboard()
        }

        function onL2BlockDetailChanged() {
            root.captureCurrentZoneEntityRef(zoneInspectionState.l2.blocks.l2BlockEntityRef(
                zoneInspectionState.l2.blocks.l2BlockDetail))
        }

        function onL2TransactionDetailChanged() {
            root.captureCurrentZoneEntityRef(zoneInspectionState.l2.blocks.l2TransactionEntityRef(
                zoneInspectionState.l2.blocks.l2TransactionDetail))
        }
    }

    property Connections zoneL2AccountConnections: Connections {
        target: zoneInspectionState.l2.accounts

        function onL2AccountFinalizedChanged() {
            const snapshot = zoneInspectionState.l2.accounts.l2AccountFinalized
            const entityRef = zoneInspectionState.l2.accounts.l2AccountEntityRef(snapshot)
            root.captureCurrentZoneEntityRef(entityRef)
            root.refreshSharedIdlsFromAccountSnapshot(snapshot, entityRef)
        }

        function onL2AccountProvisionalChanged() {
            const snapshot = zoneInspectionState.l2.accounts.l2AccountProvisional
            const entityRef = zoneInspectionState.l2.accounts.l2AccountEntityRef(snapshot)
            if (!zoneInspectionState.l2.accounts.l2AccountFinalized) {
                root.captureCurrentZoneEntityRef(entityRef)
            }
            root.refreshSharedIdlsFromAccountSnapshot(snapshot, entityRef)
        }
    }

    property Connections zoneL2ToolConnections: Connections {
        target: zoneInspectionState.l2.tools

        function onL2ProgramsChanged() {
            if (zoneInspectionState.l2.tools.l2ProgramsLoaded) {
                root.updateKnownProgramIds(zoneInspectionState.l2.tools.l2Programs)
            }
        }
    }

    property Connections shellConnections: Connections {
        target: appShellState

        function onCurrentViewChanged() {
            root.expandNavGroupForView(appShellState.currentView)
        }

        function onZoneMenuRevisionChanged() {
            root.saveSettingsState()
        }
    }

    property Connections socialSettingsConnections: Connections {
        target: socialState

        function onSocialCommentPageSizeChanged() { root.saveSettingsState() }
        function onSocialIdentityDefaultModeChanged() { root.saveSettingsState() }
        function onSelectedSocialIdentityKeyChanged() { root.saveSettingsState() }
        function onSharedIdlPolicyChanged() { root.saveSettingsState() }
        function onSharedIdlAutoShareChanged() { root.saveSettingsState() }
    }

    onBlockchainConfigurationSignatureChanged: handleBlockchainConfigurationChanged()
    onNetworkConnectorConfigChanged: {
        syncSourceModesFromConnectorConfig()
        refreshCapabilityRegistryIfLoaded()
    }
    onMessagingSourceModeChanged: handleMessagingConfigurationChanged()
    onMessagingRestUrlChanged: handleMessagingConfigurationChanged()
    onMessagingMetricsUrlChanged: handleMessagingConfigurationChanged()
    onMessagingNetworkPresetChanged: handleMessagingConfigurationChanged()
    onMessagingRollingWindowChanged: saveSettingsState()
    onMessagingAdminRestEnabledChanged: saveSettingsState()
    onStorageSourceModeChanged: handleStorageConfigurationChanged()
    onStorageRestUrlChanged: handleStorageConfigurationChanged()
    onStorageMetricsUrlChanged: handleStorageConfigurationChanged()
    onStorageNetworkPresetChanged: {
        const normalized = sourceRouting.normalizedStorageNetworkPreset(
            storageNetworkPreset)
        if (storageNetworkPreset !== normalized) {
            storageNetworkPreset = normalized
        } else {
            handleStorageConfigurationChanged()
        }
    }
    onStorageCidProbeChanged: saveSettingsState()
    onStorageRollingWindowChanged: saveSettingsState()
    onStorageLocalDiagnosticsEnabledChanged: saveSettingsState()
    onStoragePrivilegedDebugEnabledChanged: handleStorageConfigurationChanged()
    onLocalNodesEnabledChanged: {
        if (!localNodesEnabled && localDevnetEnabled) {
            localDevnetEnabled = false
        }
        saveSettingsState()
        refreshCapabilityRegistryIfLoaded()
    }
    onLocalDevnetEnabledChanged: {
        saveSettingsState()
        refreshCapabilityRegistryIfLoaded()
    }
    onSettingsBackupEncryptedChanged: saveSettingsState()

    property Connections metricsSettingsConnections: Connections {
        target: metricsState

        function onBlockchainRefreshRateChanged() { root.saveSettingsState() }
        function onMessagingRefreshRateChanged() { root.saveSettingsState() }
        function onStorageRefreshRateChanged() { root.saveSettingsState() }
        function onFooterFieldRevisionChanged() { root.saveSettingsState() }
        function onDashboardGraphRevisionChanged() { root.saveSettingsState() }
    }

    function handleNetworkConfigurationChanged() { return AppModelCore.handleNetworkConfigurationChanged(root) }

    function handleBlockchainConfigurationChanged() {
        return AppModelCore.handleBlockchainConfigurationChanged(root)
    }

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

    function zoneMenuEnabled(key) { return appShellState.zoneMenuEnabled(key) }

    function setZoneMenuEnabled(key, enabled) {
        return appShellState.setZoneMenuEnabled(key, enabled)
    }

    function zoneMenuGroups() { return appShellState.zoneMenuGroups() }

    function viewTitle() { return appShellState.viewTitle() }

    function normalizedNavigationView(view) { return appShellState.normalizedNavigationView(view) }

    function navigationSnapshot() { return appShellState.navigationSnapshot() }

    function pushNavigationHistory() { return appShellState.pushNavigationHistory() }

    function restoreNavigationSnapshot(snapshot) { return appShellState.restoreNavigationSnapshot(snapshot) }

    function canNavigateBack() { return appShellState.canNavigateBack() }

    function canNavigateForward() { return appShellState.canNavigateForward() }

    function navigateBack() {
        if (root.navigationGuarded("back", null)) {
            return false
        }
        return appShellState.navigateBack()
    }

    function navigateForward() {
        if (root.navigationGuarded("forward", null)) {
            return false
        }
        return appShellState.navigateForward()
    }

    function navigationBackLabel() { return appShellState.navigationBackLabel() }

    function navigationForwardLabel() { return appShellState.navigationForwardLabel() }

    function selectView(view, recordHistory) {
        if (root.navigationGuarded("select_view", {
                view: view,
                recordHistory: recordHistory
            })) {
            return false
        }
        return appShellState.selectView(view, recordHistory)
    }

    function openZoneDashboard(channelId, recordHistory) {
        const target = String(channelId || "")
        if (root.navigationGuarded("open_zone_dashboard", {
                channelId: target,
                recordHistory: recordHistory
            })) {
            return false
        }
        return entityNavigationState.openZoneDashboard(target, recordHistory)
    }

    function openSettings(section, subsection, recordHistory) {
        if (root.navigationGuarded("open_settings", {
                section: section,
                subsection: subsection,
                recordHistory: recordHistory
            })) {
            return false
        }
        return appShellState.openSettings(section, subsection, recordHistory)
    }

    function navigationGuarded(kind, payload) {
        return typeof root.navigationGuard === "function"
            && root.navigationGuard(String(kind || ""), payload) === true
    }

    function pageHasOutput(view) { return appShellState.pageHasOutput(view) }

    function callInspector(method, args, label) { return appRequestState.callInspector(method, args, label) }

    function callInspectorAsync(method, args, label, callback, acceptResponse) { return appRequestState.callInspectorAsync(method, args, label, callback, acceptResponse) }

    function callModule(moduleName, method, args, label) { return appRequestState.callModule(moduleName, method, args, label) }

    function startZoneInspection() { zoneInspection.start() }

    function stopZoneInspection() { zoneInspection.stop() }

    function zoneCatalogL1SourceDescriptor() {
        const source = sourceRouting.coreSourceView("blockchain")
        const endpoint = String(source && source.endpoint || "").trim()
        if (source && String(source.effectiveMode || "") === "rpc" && endpoint.length > 0) {
            const descriptor = {
                kind: "direct_http",
                endpoint: endpoint
            }
            if (localNodesEnabled && networkProfile === "default"
                    && normalizeEndpoint(endpoint) === normalizeEndpoint(nodeUrl)) {
                descriptor.default_topology = "logos_testnet"
            }
            return descriptor
        }
        const sourceLabel = String(source && source.label || qsTr("selected Bedrock source"))
        if (source && String(source.effectiveMode || "") !== "rpc") {
            return {
                kind: "unavailable",
                reason: qsTr("Zone Catalog requires a Direct RPC Bedrock source. %1 does not expose the finalized range and time data required to verify Zones.")
                    .arg(sourceLabel)
            }
        }
        return {
            kind: "unavailable",
            reason: qsTr("Zone Catalog requires a Direct RPC Bedrock endpoint. Configure one in Network settings.")
        }
    }

    function startBlockchainOperation(callerKey, method, args, label, callback) {
        return chainPages.startOperation(callerKey, method, args, label, callback)
    }

    function presentBlockchainOperation(callerKey, method, args, label, owner, callback) {
        return chainPages.presentOperation(callerKey, method, args, label, owner, callback)
    }

    function blockchainRpcArgs(extra) { return AppModelNetwork.blockchainRpcArgs(root, extra) }

    function requestModule(moduleName, method, args, label, showResult, cacheResult) { return appRequestState.requestModule(moduleName, method, args, label, showResult, cacheResult) }

    function requestModuleAsync(moduleName, method, args, label, showResult, callback, acceptResponse) { return appRequestState.requestModuleAsync(moduleName, method, args, label, showResult, callback, acceptResponse) }

    function runtimeOperationStart(request, showResult, callback) { return AppModelCore.runtimeOperationStart(root, request, showResult, callback) }

    function runtimeOperationStatus(operationId, showResult, callback) { return AppModelCore.runtimeOperationStatus(root, operationId, showResult, callback) }

    function runtimeOperationEvents(operationId, afterSeq, showResult, callback) { return AppModelCore.runtimeOperationEvents(root, operationId, afterSeq, showResult, callback) }

    function runtimeOperationCancel(operationId, showResult, callback) { return AppModelCore.runtimeOperationCancel(root, operationId, showResult, callback) }

    function runtimeOperationModuleEvent(event, showResult, callback) { return AppModelCore.runtimeOperationModuleEvent(root, event, showResult, callback) }

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

    function decodeInstructionAsync(programId, instructionWords, idlJson, accounts, callback) { return AppModelCore.decodeInstructionAsync(root, programId, instructionWords, idlJson, accounts, callback) }

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

    function restoreDefaultSettings() { return AppModelIdentity.restoreDefaultSettings(root) }

    function saveSettingsState() { return AppModelIdentity.saveSettingsState(root) }

    function settingsStatePayload() { return AppModelIdentity.settingsStatePayload(root) }

    function backupSettingsToStorage(encrypted, contents) { return AppModelIdentity.backupSettingsToStorage(root, encrypted, contents || settingsBackupContents) }

    function downloadSettingsBackupToCatalog(cid) { return AppModelIdentity.downloadSettingsBackupToCatalog(root, cid) }

    function restoreSettingsFromStorage(cid, useWallet) { return downloadSettingsBackupToCatalog(cid) }

    function settingsBackupAvailable() { return AppModelIdentity.settingsBackupAvailable(root) }

    function settingsBackupDownloadAvailable() { return AppModelIdentity.settingsBackupDownloadAvailable(root) }

    function defaultSettingsBackupContents() { return AppModelIdentity.defaultSettingsBackupContents(root) }

    function normalizedBackupContents(contents) { return backupImportState.normalizedBackupContents(contents) }

    function backupContentsSelected(contents) { return backupImportState.backupContentsSelected(contents) }

    function setSettingsBackupContent(area, enabled) { return backupImportState.setSettingsBackupContent(area, enabled) }

    function loadBackupCatalog() { return backupCatalogState.load() }

    function createLocalSettingsBackup(label, encrypted, contents) { return backupCatalogState.createLocal(label, encrypted === true, walletProfile(), backupImportState.normalizedBackupContents(contents || settingsBackupContents)) }

    function attachBackupRemote(backupCatalogId, cid, provider) { return backupCatalogState.attachRemote(backupCatalogId, cid, provider) }

    function backupCatalogRows() { return backupImportState.backupCatalogRows() }

    function recordSettingsBackupCatalogEntry(encrypted, cid) { return backupImportState.recordSettingsBackupCatalogEntry(encrypted, cid) }

    function loadWalletState() {
        const response = bridge.callModule(inspectorModule, "loadWalletState", [])
        wallet.loadPersisted(response && response.ok ? response.value : null)
    }

    function detectWalletProfile(saveDetected) { return wallet.detectProfile(saveDetected) }

    function saveWalletState() { return wallet.savePersisted(networkProfile, prefersBasecampModules()) }

    function walletStatePayload() { return wallet.payload(networkProfile, prefersBasecampModules()) }

    function walletProfile() { return wallet.profile(networkProfile, prefersBasecampModules()) }

    function walletConnectorConfigPayload() { return wallet.connectorConfigPayload(prefersBasecampModules()) }

    function walletProfileConfigured() { return wallet.profileConfigured() }

    function walletActionReady(action) { return wallet.actionReady(action) }

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

    function deliveryModuleEventRows() { return deliveryAppState.moduleEventRows() }

    function deliveryModuleEventSummary() { return deliveryAppState.moduleEventSummary() }

    function checkLocalWalletProfile(showResult) { return wallet.checkProfile(showResult) }

    function walletCommandOperationDetail(value) { return wallet.commandOperationDetail(value) }

    function deployProgramBinary(programPath) { return programExecution.deployProgramBinary(programPath) }

    function deployProgramOperationDetail(value) { return programExecution.deployProgramOperationDetail(value) }

    function privateSyncOperationDetail(value) { return wallet.privateSyncOperationDetail(value) }

    function isBedrockHexId(value) { return wallet.isBedrockHexId(value) }

    function appendLocalWalletOperation(label, status, detail) { return wallet.appendHistory(label, status, detail) }

    function previewIdlInstruction(request) { return programExecution.previewIdlInstruction(request) }

    function sendIdlInstruction(request) { return programExecution.sendIdlInstruction(request) }

    function idlInstructionOperationDetail(value) { return programExecution.idlInstructionOperationDetail(value) }

    function openSubmittedIdlInstruction(response, backendTarget) {
        const transactionId = String(response && response.ok === true
            && response.value && response.value.tx_hash || "").trim()
        const target = backendTarget || ({})
        const receiptTraceInput = root.programExecution
            ? root.programExecution.idlInstructionReceiptTraceInput : null
        const matchingReceiptTraceInput = receiptTraceInput
                && String(receiptTraceInput.txHash || "") === transactionId
            ? receiptTraceInput : null
        const context = root.zoneInspection.activeZoneContext
        const sourceId = String(target.source_id || "").trim()
        if (!transactionId.length || !sourceId.length || !context
                || root.zoneInspection.l2.scopeKey(target.network_scope)
                    !== root.zoneInspection.l2.scopeKey(context.network_scope)
                || String(target.channel_id || "") !== String(context.channel_id || "")
                || sourceId !== String(context.selected_sequencer_source_id || "")
                || Number(target.source_config_revision || 0)
                    !== Number(context.source_config_revision || 0)
                || Number(target.context_revision || 0)
                    !== Number(context.context_revision || 0)) {
            return false
        }
        root.zoneInspection.requestedDetailTab = "l2"
        root.zoneInspection.requestedL2View = "transaction"
        root.zoneInspection.l2.blocks.openSubmittedL2Transaction(transactionId,
            sourceId, matchingReceiptTraceInput)
        root.selectView("sequencerDashboard")
        return true
    }

    function refreshBedrockWalletModule(address) { return AppModelIdentity.refreshBedrockWalletModule(root, address) }

    function refreshLocalNodes(showResult) { return localNodes.refresh(showResult) }

    function invalidateAttachedRuntimeObservations() {
        if (!localNodes.localAttachedRuntime()) {
            return false
        }
        const reason = qsTr("Local LogosCore service state changed.")
        const kinds = ["blockchain", "storage", "messaging"]
        for (let index = 0; index < kinds.length; ++index) {
            metricsState.invalidateConfiguration(kinds[index], reason)
        }
        metricsState.invalidateDashboard(reason)
        scheduleAttachedRuntimeObservationRefresh()
        return true
    }

    function scheduleAttachedRuntimeObservationRefresh() {
        if (attachedRuntimeObservationRefreshQueued) {
            return false
        }
        attachedRuntimeObservationRefreshQueued = true
        Qt.callLater(function () {
            root.attachedRuntimeObservationRefreshQueued = false
            root.metrics.refreshDashboard()
        })
        return true
    }

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

    function normalizedHexText(value) { return AppModelIdentity.normalizedHexText(root, value) }

    function canonicalProgramIdHex(value) { return AppModelIdentity.canonicalProgramIdHex(root, value) }

    function autoDecodeAccountData(dataHex, accountId, ownerProgramId, callback) { return ProgramDecodeSession.autoDecodeAccountData(root, dataHex, accountId, ownerProgramId, callback) }

    function accountDecodeCandidates(accountId, ownerProgramId) { return ProgramDecodeSession.accountDecodeCandidates(root, accountId, ownerProgramId) }

    function tryAccountDecodeCandidate(serial, dataHex, candidates, index, firstError, callback) { return ProgramDecodeSession.tryAccountDecodeCandidate(root, serial, dataHex, candidates, index, firstError, callback) }

    function programDecodeCandidatePayload(candidates) { return ProgramDecodeSession.programDecodeCandidatePayload(root, candidates) }

    function decodeSelectionEntry(selection, candidates) { return ProgramDecodeSession.decodeSelectionEntry(root, selection, candidates) }

    function loadCapabilityRegistry() { return capabilityGateState.loadRegistry(root.prefersBasecampModules(), capabilityRegistryRuntimeInputs()) }

    function refreshCapabilityRegistryIfLoaded() {
        if (capabilityRegistryLoaded) {
            loadCapabilityRegistry()
        }
    }

    function gateState(expression, options) { return capabilityGateState.gateFor(expression, options || {}) }

    function storageGate(action, options) { return capabilityGateState.storageGate(action, options || {}) }

    function deliveryGate(action, options) { return capabilityGateState.deliveryGate(action, options || {}) }

    function socialGate(action, options) { return capabilityGateState.socialGate(action, options || {}) }

    function walletGate(action, options) { return capabilityGateState.walletGate(action, options || {}) }

    function diagnosticsGate(action, options) { return capabilityGateState.diagnosticsGate(action, options || {}) }

    function programDecodeGate(options) { return capabilityGateState.programDecodeGate(options || {}) }

    function statusFacts() { return statusFacadeState.facts() }

    function localNodeSourceObservation(kind) {
        const observation = metricsState.sourceObservation(kind) || ({})
        const report = observation.sourceReport || null
        const health = report && report.health && typeof report.health === "object"
            ? report.health : null
        const status = observation.status || ({})
        let observedStatus = "unknown"
        if (health && health.ready === true) {
            observedStatus = "healthy"
        } else if (health && health.ready === false) {
            observedStatus = "unavailable"
        } else if (status.known === true) {
            observedStatus = status.ok === true ? "healthy" : "unavailable"
        }
        return {
            status: observedStatus,
            detail: String(health && (health.detail || health.summary)
                || status.detail || ""),
            checked_at: String(observation.checkedAt || ""),
            checked_at_ms: Number(observation.checkedAtMs || observation.reportCheckedAtMs || 0),
            provenance: observation.provenance || ["metrics_source_observation", String(kind || "")]
        }
    }

    function localNodeIndexerObservation() {
        const channelStatuses = Array.isArray(dashboardChannelStatuses)
            ? dashboardChannelStatuses : []
        const channelIndexers = []
        for (let i = 0; i < channelStatuses.length; ++i) {
            const channel = channelStatuses[i] || ({})
            const indexer = channel.indexer || ({})
            if (indexer.configured !== true) {
                continue
            }
            const sequencer = channel.sequencer || ({})
            channelIndexers.push({
                channel_id: String(channel.channel_id || ""),
                short_channel_id: String(channel.short_channel_id || channel.channel_id || ""),
                label: String(channel.label || channel.short_channel_id || channel.channel_id || ""),
                status: String(indexer.status || "unknown").toLowerCase(),
                indexer_state: String(indexer.indexer_state || "").toLowerCase(),
                head: indexer.head === undefined ? null : indexer.head,
                upstream_head: sequencer.head === undefined ? null : sequencer.head
            })
        }
        if (channelStatuses.length > 0) {
            return channelIndexerObservation(channelIndexers)
        }
        const overview = metricsState.dashboardOverview || ({})
        const indexer = overview.indexer || ({})
        const health = indexer.health || null
        const healthValue = String(health && health.value || "unknown").toLowerCase()
        let status = "unknown"
        if (health && health.ok === true
                && (healthValue === "reachable" || healthValue === "healthy"
                    || healthValue === "ready" || healthValue === "running")) {
            status = "reachable"
        } else if (health && health.ok === false) {
            status = "unavailable"
        }
        return {
            status: status,
            indexer_state: String(indexer.indexer_state || "").toLowerCase(),
            head: metricsState.overviewProbeValue("indexer", "head"),
            upstream_head: metricsState.overviewProbeValue("sequencer", "head"),
            detail: healthValue === "unknown" ? "" : healthValue,
            provenance: ["zone_source_observation", "indexer"]
        }
    }

    function channelIndexerObservation(channels) {
        const rows = Array.isArray(channels) ? channels : []
        if (!rows.length) {
            return {
                status: "unknown",
                head: null,
                upstream_head: null,
                channels: [],
                detail: qsTr("No Channel Indexer is configured."),
                provenance: ["channel_source_observation", "indexer", "multi_channel"]
            }
        }
        let status = "reachable"
        const details = []
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i] || ({})
            const sourceStatus = String(row.status || "unknown").toLowerCase()
            if (sourceStatus === "unreachable") {
                status = "unavailable"
            } else if (status !== "unavailable"
                    && (sourceStatus === "degraded" || sourceStatus === "stale")) {
                status = "degraded"
            } else if (status === "reachable" && sourceStatus !== "reachable") {
                status = "unknown"
            }
            details.push(qsTr("%1: %2")
                .arg(String(row.short_channel_id || row.channel_id || qsTr("Channel")))
                .arg(sourceStatus))
        }
        return {
            status: status,
            head: null,
            upstream_head: null,
            channels: rows,
            detail: details.join(" · "),
            provenance: ["channel_source_observation", "indexer", "multi_channel"]
        }
    }

    function localNodeObservedNodes() {
        const observationRevision = metricsState.observationRevision
        const dashboardRevision = metricsState.dashboardSnapshotRevision
        return {
            bedrock: localNodeSourceObservation("blockchain"),
            indexer: localNodeIndexerObservation(),
            storage: localNodeSourceObservation("storage"),
            messaging: localNodeSourceObservation("messaging")
        }
    }

    function dashboardGate(key) { return statusFacadeState.dashboardGate(key) }

    function capabilityRegistryRuntimeInputs() {
        return {
            configuration_generations: {
                l1: metricsState.familyConfigurationGeneration("blockchain"),
                storage: metricsState.familyConfigurationGeneration("storage"),
                delivery: metricsState.familyConfigurationGeneration("messaging")
            },
            network_connector_config: networkConnectorConfigPayload(),
            wallet_connector_config: walletConnectorConfigPayload(),
            node_url: String(nodeUrl || ""),
            storage_rest_url: sourceRouting.configuredStorageRestUrl(),
            storage_metrics_url: sourceRouting.configuredStorageMetricsUrl(),
            messaging_rest_url: sourceRouting.configuredMessagingRestUrl(),
            messaging_metrics_url: String(messagingMetricsUrl || ""),
            storage_mutating_diagnostics_enabled: true,
            messaging_mutating_diagnostics_enabled: true,
            wallet_profile_configured: walletProfileConfigured(),
            wallet_home_configured: walletHomeConfigured(),
            wallet_instruction_submit_ready: wallet.actionReady("instruction_submit"),
            local_nodes_enabled: localNodesEnabled === true,
            local_devnet_enabled: localNodesEnabled === true && localDevnetEnabled === true
        }
    }

    function capabilityLocalAvailability() {
        const identity = socialState.identitiesView()
        const localIdentityReady = identity.rows.length > 0
            || identity.selectedKey.length > 0 || identity.defaultMode !== "manual"
        return {
            "social.identity.local": {
                status: localIdentityReady ? "available" : "input_required",
                provenance: "local_identity"
            }
        }
    }

    function defaultNetworkConnectorConfig() {
        return {
            scopes: {
                "l1": {
                    connector_id: prefersBasecampModules() ? "blockchain_module" : "direct_l1_rpc",
                    provenance: "build_default"
                },
                "delivery": {
                    connector_id: prefersBasecampModules() ? "delivery_module" : "direct_delivery_rest",
                    provenance: "build_default"
                },
                "storage": {
                    connector_id: prefersBasecampModules() ? "storage_module" : "logoscore_cli_storage_module",
                    provenance: "build_default"
                }
            }
        }
    }

    function loadNetworkConnectorConfig(value) {
        const raw = value && value.network_connector_config && typeof value.network_connector_config === "object"
            ? value.network_connector_config
            : defaultNetworkConnectorConfig()
        const normalized = normalizedNetworkConnectorConfig(raw)
        const scopes = raw && raw.scopes && typeof raw.scopes === "object" ? raw.scopes : ({})
        const defaults = defaultNetworkConnectorConfig().scopes
        const keys = ["l1", "delivery", "storage"]
        for (let i = 0; i < keys.length; ++i) {
            const key = keys[i]
            const entry = scopes[key] && typeof scopes[key] === "object" ? scopes[key] : ({})
            const provenance = String(entry.provenance || "")
            if (provenance === "testnet_default" || provenance === "build_default") {
                normalized.scopes[key] = defaults[key]
            }
        }
        networkConnectorConfig = normalized
        syncSourceModesFromConnectorConfig()
    }

    function normalizedNetworkConnectorConfig(value) {
        const defaults = defaultNetworkConnectorConfig().scopes
        const source = value && typeof value === "object" ? value : ({})
        const scopes = source.scopes && typeof source.scopes === "object" ? source.scopes : source
        const result = { scopes: ({}) }
        const keys = ["l1", "delivery", "storage"]
        for (let i = 0; i < keys.length; ++i) {
            const key = keys[i]
            const fallback = defaults[key] || {}
            const entry = scopes[key] && typeof scopes[key] === "object" ? scopes[key] : fallback
            const requestedConnectorId = String(entry.connector_id || entry.connectorId || entry.id || "")
            const connectorId = networkConnectorSupported(key, requestedConnectorId)
                ? requestedConnectorId
                : String(fallback.connector_id || "")
            const usesFallback = connectorId !== requestedConnectorId
            result.scopes[key] = {
                connector_id: connectorId,
                endpoint: usesFallback
                    ? String(fallback.endpoint || "")
                    : String(entry.endpoint || entry.url || entry.rest_endpoint || entry.rpc_endpoint || ""),
                provenance: usesFallback
                    ? "build_default"
                    : String(entry.provenance || entry.connector_provenance || (entry === fallback ? "build_default" : "network_profile"))
            }
        }
        return result
    }

    function networkConnectorSupported(scope, connectorId) {
        const candidate = String(connectorId || "")
        if (!candidate.length) {
            return false
        }
        const family = String(scope || "") === "l1" ? "core" : String(scope || "")
        const policies = sourceRouting.sourceModePolicies(family)
        for (let i = 0; i < policies.length; ++i) {
            const descriptor = sourceRouting.sourceModeDescriptor(family, policies[i].key)
            if (descriptor.connectorId === candidate) {
                return descriptor.connectionType !== "module" || prefersBasecampModules()
            }
        }
        return false
    }

    function networkConnectorConfigPayload() {
        return normalizedNetworkConnectorConfig(networkConnectorConfig)
    }

    function setNetworkConnectorMode(scope, mode) {
        const key = String(scope || "")
        if (key === "storage" && storageApp.sourceSettingsLocked) {
            return false
        }
        const family = key === "l1" ? "core" : key
        const descriptor = sourceRouting.sourceModeDescriptor(family, mode)
        const connectorId = String(descriptor.connectorId || "")
        if (!connectorId.length) {
            return false
        }
        const next = normalizedNetworkConnectorConfig(networkConnectorConfig)
        next.scopes[key] = {
            connector_id: connectorId,
            endpoint: "",
            provenance: "network_profile"
        }
        networkConnectorConfig = next
        setSourceModeProperty(key, String(descriptor.key || mode || ""))
        return true
    }

    function setNetworkConnectorEndpoint(scope, endpoint) {
        const key = String(scope || "")
        if (key !== "storage" || storageApp.sourceSettingsLocked) {
            return false
        }
        const mode = currentConnectorSourceMode("storage", storageSourceMode)
        if (mode !== "rest" && mode !== "metrics") {
            return false
        }
        const value = String(endpoint || "").trim()
        const next = normalizedNetworkConnectorConfig(networkConnectorConfig)
        const current = next.scopes.storage || {}
        const configChanged = String(current.endpoint || "") !== value
            || String(current.provenance || "") !== "network_profile"
        const fallbackChanged = mode === "metrics"
            ? storageMetricsUrl !== value : storageRestUrl !== value
        if (configChanged) {
            next.scopes.storage = {
                connector_id: String(current.connector_id || ""),
                endpoint: value,
                provenance: "network_profile"
            }
            networkConnectorConfig = next
        }
        if (mode === "metrics") {
            if (fallbackChanged) {
                storageMetricsUrl = value
            }
        } else if (fallbackChanged) {
            storageRestUrl = value
        }
        if (configChanged && !fallbackChanged) {
            handleStorageConfigurationChanged()
        }
        return true
    }

    function setSourceModeProperty(scope, mode) {
        const value = String(mode || "")
        switch (String(scope || "")) {
        case "l1":
            blockchainSourceMode = value
            break
        case "delivery":
            messagingSourceMode = value
            break
        case "storage":
            storageSourceMode = value
            break
        }
    }

    function syncSourceModesFromConnectorConfig() {
        blockchainSourceMode = sourceRouting.connectorSourceMode("l1", "rpc")
        messagingSourceMode = sourceRouting.connectorSourceMode("delivery", "rest")
        storageSourceMode = sourceRouting.connectorSourceMode("storage", "rest")
    }

    function currentConnectorSourceMode(scope, fallback) {
        return sourceRouting.connectorSourceMode(scope, fallback)
    }

    function copyMap(source) { return AppModelNetwork.copyMap(root, source) }

    function mergeMap(base, overrides) { return AppModelNetwork.mergeMap(root, base, overrides) }

    function stringSetting(value, key, fallback) { return AppModelNetwork.stringSetting(root, value, key, fallback) }

    function numberSetting(value, key, fallback) { return AppModelNetwork.numberSetting(root, value, key, fallback) }

    function boolSetting(value, key, fallback) { return AppModelNetwork.boolSetting(root, value, key, fallback) }

    function normalizedNetworkProfile(value) { return networkProfileState.normalizedProfile(value) }

    function resolvedNetworkProfile(storedProfile, node) { return networkProfileState.resolvedProfile(storedProfile, node) }

    function inferNetworkProfileFromEndpoint(node) { return networkProfileState.inferProfile(node) }

    function normalizeEndpoint(value) { return networkProfileState.normalizeEndpoint(value) }

    function loadNetworkProfileSettings(value) {
        const settings = networkProfileState.settingsFromPayload(value, networkProfile, nodeUrl)
        networkProfile = settings.profile
        nodeUrl = settings.nodeUrl
    }

    function networkProfileSettingsPayload() { return networkProfileState.settingsPayload(networkProfile, nodeUrl) }

    function networkProfileOptions() { return networkProfileState.optionRows() }

    function profileIndexFor(value) { return networkProfileState.profileIndex(value) }

    function profileIndex() { return profileIndexFor(networkProfile) }

    function applyProfileIndex(index) { return applyProfile(index) }

    function applyProfile(index) {
        const profile = networkProfileState.profileAt(index)
        if (profile === "custom") {
            networkProfile = inferNetworkProfileFromEndpoint(nodeUrl)
            return
        }
        const endpoints = networkProfileState.applyProfile(profile)
        if (!endpoints) {
            return
        }
        networkProfile = endpoints.profile
        nodeUrl = endpoints.nodeUrl
        messagingNetworkPreset = "logos.test"
    }

    function networkProfileLabel(value) { return networkProfileState.profileLabel(value) }

    function networkProfileSummary(value) { return networkProfileState.profileSummary(value) }

    function networkProfileDetail() { return networkProfileState.profileDetail(nodeUrl) }

    function networkProfileCacheScope() { return networkProfileState.cacheScope(networkProfile, nodeUrl) }

    function zoneScopeKey() {
        const context = zoneInspection.activeZoneContext
        if (!context) {
            return ""
        }
        return "zone:" + zoneInspection.scopeKey(context.network_scope)
            + ":" + String(context.channel_id || "")
    }

    function zoneSourceScopeKey() {
        const context = zoneInspection.activeZoneContext
        const zoneScope = zoneScopeKey()
        const sourceId = String(context && context.selected_sequencer_source_id || "")
        if (!zoneScope.length || !sourceId.length) {
            return ""
        }
        return zoneScope + ":source:" + sourceId + ":revision:"
            + String(context.source_config_revision || 0)
    }

    function zoneL2Capability(sourceRole) { return zoneInspection.l2.l2Capability(sourceRole) }

    function zoneCollaborationCapability() { return zoneInspection.l2.collaborationCapability() }

    function refreshSharedIdlsFromAccountSnapshot(snapshot, entityRef) {
        const account = snapshot && snapshot.account ? snapshot.account : null
        if (!account || !entityRef) {
            return false
        }
        const ownerProgram = String(account.owner_program_hex
            || account.owner_program_base58 || "").trim()
        return social.refreshSharedIdlsForAccount(
            entityRef,
            String(account.data_hex || "").trim(),
            ownerProgram
        )
    }

    function resolveInspectionTarget(query) { return entityNavigation.resolveInspectionTarget(query) }

    function openInspectionCandidate(candidate, recordHistory) { return entityNavigation.openInspectionCandidate(candidate, recordHistory) }

    function openInspectionEntityRef(entity, recordHistory) { return entityNavigation.openInspectionEntityRef(entity, recordHistory) }

    function captureCurrentZoneEntityRef(entity) {
        if (!entity) {
            return
        }
        currentInspectionEntityRef = {
            layer: "l2",
            network_scope: entity.network_scope,
            channel_id: String(entity.channel_id || ""),
            zone_kind: String(entity.zone_kind || "unknown"),
            entity_kind: String(entity.entity_kind || ""),
            canonical_key: String(entity.canonical_key || ""),
            source: entity.source || { kind: "policy" }
        }
    }

    function isStorageCid(value) { return entityNavigation.isStorageCid(value) }

    function routePrefixedSearch(query) { return entityNavigation.routePrefixedSearch(query) }

    function searchPrefix(query) { return entityNavigation.searchPrefix(query) }

    function isSearchPrefix(prefix) { return entityNavigation.isSearchPrefix(prefix) }

    function routeModuleSearchTarget(target) { return entityNavigation.routeModuleSearchTarget(target) }

    function viewKeyForQuery(query) { return entityNavigation.viewKeyForQuery(query) }

    function settingsTargetForQuery(query) { return entityNavigation.settingsTargetForQuery(query) }

    function openReference(kind, value, payload) { return entityNavigation.openReference(kind, value, payload) }

    function openPrivateAccountReference(account) { return entityNavigation.openPrivateAccountReference(account) }

    function loadBlockchainBlockById(blockId) { return entityNavigation.loadBlockchainBlockById(blockId) }

    function loadBlockchainBlockBySlot(slot) { return entityNavigation.loadBlockchainBlockBySlot(slot) }

    function openBlockchainTransaction(transaction, block) { return entityNavigation.openBlockchainTransaction(transaction, block) }

    function transactionDetail(hash) { return entityNavigation.transactionDetail(hash) }

    function blockchainTransactionDetail(value, fallbackHash) { return entityNavigation.blockchainTransactionDetail(value, fallbackHash) }

    function showLocalWalletRequired(wallet) { return entityNavigation.showLocalWalletRequired(wallet) }

    function programIdKnown(programId) { return AppModelRegistry.programIdKnown(root, programId) }

    function knownProgramCacheScope() { return networkProfileCacheScope() }

    function knownProgramIdRows() { return AppModelRegistry.knownProgramIdRows(root) }

    function updateKnownProgramIds(value) { return AppModelRegistry.updateKnownProgramIds(root, value) }

    function registerIdl(name, programId, json, programBinary) { return AppModelRegistry.registerIdl(root, name, programId, json, programBinary) }

    function removeIdl(index) { return AppModelRegistry.removeIdl(root, index) }

}
