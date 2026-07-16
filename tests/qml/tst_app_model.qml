import QtQuick
import QtTest
import "../../qml/services"
import "../../qml/state"
import "../../qml/state/source_routing/SourcePolicyCatalog.js" as SourcePolicyCatalog
import "../../qml/state/source_routing/SourceDiagnosticsProjection.js" as SourceDiagnostics
import "../../qml/state/status/StatusFactsProjection.js" as StatusFactsProjection
import "MetricsCompatibilityManifest.js" as MetricsCompatibilityManifest
import "SourceRoutingCompatibilityManifest.js" as SourceRoutingCompatibilityManifest
import "fixtures"

TestCase {
    id: testRoot

    name: "AppModel"

    BridgeHostFixture {
        id: fakeHost
    }

    AsyncBridgeHostFixture {
        id: asyncImportHost
    }

    Timer {
        id: importHeartbeat

        property int ticks: 0

        interval: 1
        repeat: true
        onTriggered: ticks += 1
    }

    QtObject {
        id: basecampHost

        property int callCount: 0
        property string lastModule: ""
        property string lastMethod: ""
        property var lastArgs: []
        property bool serializeResults: false

        function callModule(moduleName, method, args) {
            callCount += 1
            lastModule = String(moduleName || "")
            lastMethod = String(method || "")
            lastArgs = args || []
            if (lastModule === "logos_inspector" && lastMethod === "call") {
                const response = JSON.stringify({
                    ok: true,
                    value: {
                        method: lastArgs[0],
                        args: JSON.parse(String(lastArgs[1] || "[]"))
                    },
                    text: "OK",
                    error: ""
                })
                return serializeResults ? JSON.stringify(response) : response
            }
            return JSON.stringify("direct")
        }
    }

    BridgeClient {
        id: bridgeClient

        host: fakeHost
    }

    BridgeClient {
        id: asyncImportBridgeClient

        host: asyncImportHost
    }

    BridgeClient {
        id: basecampBridgeClient

        host: basecampHost
    }

    AppModel {
        id: model

        bridge: bridgeClient
    }

    ModuleEventIntake {
        id: moduleEventIntake

        bridge: bridgeClient
        model: model
    }

    AppModel {
        id: basecampModel

        bridge: basecampBridgeClient
    }

    function init() {
        importHeartbeat.stop()
        importHeartbeat.ticks = 0
        model.bridge = bridgeClient
        model.chainPages.invalidateOperations("test reset")
        basecampModel.chainPages.invalidateOperations("test reset")
        wait(0)
        fakeHost.reset()
        asyncImportHost.reset()
        basecampHost.callCount = 0
        basecampHost.lastModule = ""
        basecampHost.lastMethod = ""
        basecampHost.lastArgs = []
        basecampHost.serializeResults = false
        model.capabilityRegistryLoaded = false
        model.capabilityRegistryReport = ({ schema_version: 1, capabilities: [] })
        basecampModel.capabilityRegistryLoaded = false
        basecampModel.capabilityRegistryReport = ({ schema_version: 1, capabilities: [] })
        model.shell.currentView = "overview"
        model.shell.statusText = "Ready"
        model.shell.busy = false
        model.shell.resultTitle = "Output"
        model.shell.resultText = ""
        model.shell.resultValue = null
        model.shell.resultIsError = false
        model.shell.resultOwner = ""
        model.navigationBackStack = []
        model.navigationForwardStack = []
        model.navigationRevision = 0
        model.navigationRestoring = false
        model.favoriteStore.clear()
        model.zoneInspection.activeZoneContext = null
        model.zoneInspection.networkScope = null
        model.zoneInspection.zoneSummaries = []
        model.zoneInspection.zoneDetail = null
        model.dashboardNode = null
        model.dashboardProvisionalBlocks = []
        model.metrics.blockchainSourceReport = null
        model.metrics.blockchainModuleReport = null
        model.metrics.storageModuleReport = null
        model.metrics.messagingModuleReport = null
        model.metrics.storageSourceReport = null
        model.metrics.messagingSourceReport = null
        model.deliveryModuleEvents = []
        model.deliveryModuleEventRevision = 0
        model.deliveryConnectionStatus = ""
        model.deliveryNodeStatus = ""
        model.blockchainModuleEventRevision = 0
        model.blockchainLastEventText = ""
        model.storageApp.operationSession.clearActive()
        model.backupCatalog.invalidateUpload("")
        model.backupCatalog.invalidateDownload("")
        model.backupCatalog.importGeneration += 1
        model.backupCatalog.pendingImportCatalogId = ""
        model.backupCatalog.importCompletion = null
        model.backupCatalog.entries = []
        model.backupCatalog.loaded = false
        model.backupCatalog.error = ""
        model.backupCatalog.revision = 0
        model.settingsBackupCid = ""
        model.settingsRestoreCid = ""
        model.settingsBackupStatus = ""
        model.runtimeOperations = ({})
        model.runtimeOperationEventSeq = ({})
        model.runtimeOperationHistory = []
        model.runtimeOperationsRevision = 0
        model.operationHistory.runtimeOperationEventFacts = ({})
        model.operationHistory.runtimeOperationPollGenerations = ({})
        model.operationHistory.runtimeOperationPendingPolls = ({})
        model.operationHistory.runtimeOperationTerminalOrder = []
        model.operationHistory.runtimeOperationCursorOrder = []
        model.metrics.networkConnectionStatus = ({})
        model.metrics.networkConnectionStatusRevision = 0
        model.metrics.dashboardMetricHistory = ({})
        model.metrics.dashboardMetricLastSeen = ({})
        model.metrics.dashboardMetricHistoryRevision = 0
        model.metrics.observationReportRequestIdentities = ({})
        model.metrics.blockchainRefreshRate = 30
        model.metrics.messagingRefreshRate = 30
        model.metrics.storageRefreshRate = 30
        model.metrics.footerFieldSelections = model.metrics.defaultFooterFieldSelections()
        model.metrics.dashboardGraphSelections = model.metrics.defaultDashboardGraphSelections()
        model.blocksPageRows = []
        model.blocksPageSlotFrom = 0
        model.blocksPageSlotTo = 0
        model.blocksPageError = ""
        model.blocksLiveEnabled = false
        model.blocksLiveError = ""
        model.blocksLiveSource = ""
        model.blocksLiveUnknownEvents = 0
        model.blocksLiveCheckedAt = ""
        model.transactionsPageRows = []
        model.transactionsPageBeforeBlock = 0
        model.transactionsPageNextBeforeBlock = 0
        model.transactionsPageError = ""
        model.blockDetailValue = null
        model.blockDetailError = ""
        model.transactionDetailValue = null
        model.transactionDetailError = ""
        model.networkConnectorConfig = ({
            scopes: {
                l1: { connector_id: "direct_l1_rpc", provenance: "test" },
                delivery: { connector_id: "direct_delivery_rest", provenance: "test" },
                storage: { connector_id: "direct_storage_rest", provenance: "test" }
            }
        })
        model.blockchainSourceMode = "rpc"
        model.messagingSourceMode = "rest"
        model.storageSourceMode = "rest"
        model.sourceRouting.sourcePolicy = ({})
        model.sourceRouting.sourcePolicyLoaded = false
        model.capabilityRegistryLoaded = true
        model.capabilityRegistryReport = appModelTestCapabilityRegistry()
        model.settingsBackupContents = model.defaultSettingsBackupContents()
        model.settingsBackupEncrypted = false
        basecampModel.networkConnectorConfig = basecampModel.defaultNetworkConnectorConfig()
        basecampModel.blockchainSourceMode = "module"
        basecampModel.messagingSourceMode = "module"
        basecampModel.storageSourceMode = "module"
        basecampModel.sourceRouting.sourcePolicy = ({})
        basecampModel.sourceRouting.sourcePolicyLoaded = false
        model.networkConfigurationRevision = 0
        model.blockchainConfigurationRevision = 0
        basecampModel.capabilityRegistryLoaded = true
        basecampModel.capabilityRegistryReport = appModelTestCapabilityRegistry()
        model.registeredIdls.clear()
        model.social.socialIdentities.clear()
        model.idlStateLoaded = false
        model.walletStateLoaded = false
        model.settingsStateLoaded = false
        model.social.socialIdentityDefaultMode = "perConversation"
        model.social.selectedSocialIdentityKey = ""
        model.social.socialConversationIdentityKeys = ({})
        model.social.socialIdentityRevision = 0
        model.social.socialCommentState = ({})
        model.social.socialCommentRevision = 0
        model.social.invalidateSourceRequests()
        model.social.socialSharedIdls = ({})
        model.social.sharedIdlPolicy = "suggestion"
        model.social.sharedIdlAutoShare = false
        model.social.socialAutoSharedIdls = ({})
        model.social.sharedIdlRevision = 0
        model.accountIdlSelections = ({})
        model.accountIdlSelectionRevision = 0
        model.idlInstructionPreviewValue = null
        model.idlInstructionError = ""
        model.walletPublicKeyProbe = ""
        model.bedrockWalletModuleError = ""
        model.walletBinary = ""
        model.walletHome = ""
        model.walletCreatePrivacy = "public"
        model.walletCreateLabel = ""
        model.walletSendFrom = ""
        model.walletSendTo = ""
        model.walletSendToKeys = ""
        model.walletSendToNpk = ""
        model.walletSendToVpk = ""
        model.walletSendToIdentifier = ""
        model.walletSendAmount = ""
        model.walletAdvancedCommand = ""
        model.walletConnectorConfig = ({})
        model.localWalletStatus = null
        model.localWalletStatusError = ""
        model.localWalletOperations = []
        model.localNodesEnabled = false
        model.localDevnetEnabled = false
        model.localNodesReport = null
        model.localNodesError = ""
        model.localNodesOperations = []
        model.localNodesRevision = 0
        model.localDevnets = []
        basecampModel.walletConnectorConfig = ({})
    }

    function readyWalletStatus(homeSource) {
        return {
            status: "ok",
            home_source: String(homeSource || "profile"),
            readiness: {
                wallet_binary_ready: true,
                wallet_home_ready: true,
                wallet_config_ready: true,
                wallet_storage_ready: true,
                command_ready: true,
                accounts_ready: true,
                instruction_submit_ready: true,
                backup_encryption_ready: true
            }
        }
    }

    function configureReadyWallet() {
        model.walletStateLoaded = true
        model.walletBinary = "/usr/bin/lee-wallet"
        model.walletHome = "/tmp/wallet-home"
        model.localWalletStatus = readyWalletStatus("profile")
    }

    function backupUploadOperation(id, status, result, cursor) {
        const initialization = model.sourceRouting.storageOperationAdapter()
        const inputs = initialization.inputs || ({})
        const catalogId = String(result && result.backup_catalog_id || "backup-1")
        return {
            operationId: String(id || "backup-upload-1"),
            domain: "storage",
            method: "storageUploadBackupCatalogEntry",
            backend: String(initialization.source_mode || ""),
            label: "Backup upload",
            status: String(status || "completed"),
            eventCursor: Number(cursor || 1),
            context: {
                source: String(initialization.source_mode || ""),
                endpoint: String(inputs.rest_endpoint || ""),
                mutatingEnabled: model.storageMutatingDiagnosticsEnabled === true,
                backupCatalogId: catalogId
            },
            result: result === undefined ? null : result,
            error: status === "failed" ? "upload failed" : ""
        }
    }

    function backupUploadResult(catalogId, cid, payloadId) {
        const initialization = model.sourceRouting.storageOperationAdapter()
        const inputs = initialization.inputs || ({})
        const source = String(initialization.source_mode || "")
        const endpoint = String(inputs.rest_endpoint || "").length
            ? String(inputs.rest_endpoint)
            : source === "module"
                ? "module storage_module"
                : source === "logoscore_cli"
                    ? "logoscore call storage_module" : ""
        return {
            cid: String(cid || "cid-backup"),
            bytes: 128,
            endpoint: endpoint,
            backup_catalog_id: String(catalogId || "backup-1"),
            catalog_entry: {
                backup_catalog_id: String(catalogId || "backup-1"),
                payload_id: String(payloadId || "sha256:upload"),
                encrypted: false,
                remote: {
                    cid: String(cid || "cid-backup"),
                    provider: "logos_storage"
                }
            }
        }
    }

    function backupDownloadOperation(id, status, cid, result, cursor) {
        return {
            operationId: String(id || "backup-download-1"),
            domain: "storage",
            method: "storageDownloadBackupCatalogEntry",
            backend: "rest",
            label: "Backup download",
            status: String(status || "completed"),
            eventCursor: Number(cursor || 1),
            context: {
                source: "rest",
                endpoint: model.sourceRouting.configuredStorageRestUrl(),
                cid: String(cid || "cid-restore"),
                downloadScope: "network"
            },
            result: result === undefined ? null : result,
            error: status === "failed" ? "download failed" : ""
        }
    }

    function socialUploadOperation(id, result) {
        return {
            operationId: String(id || "social-upload-1"),
            domain: "storage",
            method: "storageUploadPayload",
            label: "Upload shared IDL",
            status: "completed",
            eventCursor: 1,
            context: {
                source: "rest",
                filename: "logos-inspector-shared-idl.json"
            },
            result: result,
            error: ""
        }
    }

    function socialSendOperation(id, topic, result) {
        return {
            operationId: String(id || "social-send-1"),
            domain: "delivery",
            method: "deliverySend",
            label: "Share IDL",
            status: "completed",
            eventCursor: 1,
            context: {
                source: "rest",
                contentTopic: String(topic || "")
            },
            result: result,
            error: ""
        }
    }

    function chainOperationContext(request) {
        const args = request && Array.isArray(request.args) ? request.args : []
        const first = String(args[0] || "")
        let source = "rpc"
        let endpoint = first
        let offset = 1
        if (first === "module" || first === "logoscore_cli") {
            source = first
            endpoint = ""
        } else if (first === "rpc") {
            endpoint = String(args[1] || "")
            offset = 2
        }
        const context = { source: source }
        if (request && request.configurationGeneration !== undefined) {
            context.configurationGeneration = Number(request.configurationGeneration)
        }
        if (endpoint.length) {
            context.endpoint = endpoint
        }
        switch (String(request && request.method || "")) {
        case "blockchainBlocks":
            context.slotFrom = Number(args[offset])
            context.slotTo = Number(args[offset + 1])
            context.slotRange = String(context.slotFrom) + ":" + String(context.slotTo)
            if (typeof args[offset + 2] === "number") {
                context.limit = Number(args[offset + 2])
            }
            break
        case "blockchainLiveBlocks":
            context.slotFrom = Number(args[offset])
            context.slotTo = Number(args[offset + 1])
            context.slotRange = String(context.slotFrom) + ":" + String(context.slotTo)
            context.limit = typeof args[offset + 2] === "number"
                ? Number(args[offset + 2]) : 50
            break
        case "blockchainBlock":
            context.blockId = String(args[offset] || "")
            break
        case "blockchainTransaction":
            context.transactionId = String(args[offset] || "")
            break
        }
        return context
    }

    function chainRuntimeStart(results) {
        return function (args) {
            const request = args && args[0] ? args[0] : ({})
            const configured = results && results[request.method]
            const result = typeof configured === "function"
                ? configured(request) : configured
            const context = chainOperationContext(request)
            return {
                ok: true,
                value: {
                    operationId: "chain-op-" + String(request.clientRequestId || "unknown"),
                    clientRequestId: request.clientRequestId,
                    domain: "blockchain",
                    backend: context.source,
                    method: request.method,
                    label: request.label,
                    status: "completed",
                    eventCursor: 1,
                    context: context,
                    result: result,
                    error: ""
                },
                text: "OK",
                error: ""
            }
        }
    }

    function setActiveZone(channelId) {
        const zoneId = String(channelId || "22".repeat(32))
        const scope = { kind: "genesis_id", genesis_id: "11".repeat(32) }
        model.zoneInspection.networkScope = scope
        model.zoneInspection.verification = "verified"
        model.zoneInspection.zoneSummaries = [{
            channel_id: zoneId,
            kind: "sequencer_zone",
            l1_channel: {},
            l2_zone: {},
            activity_detail: {}
        }]
        model.zoneInspection.activeZoneContext = {
            network_scope: scope,
            channel_id: zoneId,
            zone_kind: "sequencer_zone",
            selected_sequencer_source_id: "seq-a",
            indexer_source_id: "idx-a",
            source_config_revision: 7,
            context_revision: 1
        }
        return zoneId
    }

    function zoneEntityRef(kind, canonicalKey, sourceId, sourceRole) {
        const channelId = model.zoneInspection.activeZoneId.length > 0
            ? model.zoneInspection.activeZoneId : setActiveZone("")
        return {
            network_scope: model.zoneInspection.activeZoneContext.network_scope,
            channel_id: channelId,
            zone_kind: "sequencer_zone",
            entity_kind: String(kind || ""),
            canonical_key: String(canonicalKey || ""),
            source: sourceId ? {
                kind: "exact",
                source_id: String(sourceId),
                source_role: String(sourceRole || "sequencer")
            } : { kind: "policy" }
        }
    }

    function appModelTestCapabilityRegistry() {
        return {
            schema_version: 1,
            capabilities: [
                {
                    key: "storage",
                    label: "Storage",
                    status: "available",
                    sub_capabilities: [
                        "storage.identity.read",
                        "storage.manifests.read",
                        "storage.content.exists",
                        "storage.content.read_by_cid",
                        "storage.content.upload",
                        "storage.backup.sync_read_by_cid",
                        "storage.backup.sync_upload",
                        "storage.shared_idl.sync_read",
                        "storage.shared_idl.sync_upload",
                        "storage.rest.upload",
                        "storage.rest.read_by_cid",
                        "storage.content.download_to_file",
                        "storage.content.remove"
                    ]
                },
                {
                    key: "delivery",
                    label: "Delivery",
                    status: "available",
                    sub_capabilities: ["delivery.store.query", "delivery.send", "delivery.subscribe"]
                },
                {
                    key: "wallet.l1",
                    label: "L1 Wallet",
                    status: "available",
                    sub_capabilities: ["wallet.l1.accounts.read", "wallet.l1.sign", "wallet.l1.submit", "wallet.command.run"]
                },
                {
                    key: "wallet.l2",
                    label: "L2 Wallet",
                    status: "available",
                    sub_capabilities: ["wallet.l2.instruction.preview", "wallet.l2.instruction.submit", "wallet.l2.program.deploy", "wallet.command.run"]
                }
            ]
        }
    }

    function installSourceModePolicy(targetModel) {
        targetModel.sourceRouting.sourcePolicy = SourcePolicyCatalog.fallbackPolicy()
        targetModel.sourceRouting.sourcePolicyLoaded = true
    }

    function sourceOption(options, key) {
        for (let i = 0; i < options.length; ++i) {
            const option = options[i] || {}
            if (String(option.key || "") === key) {
                return option
            }
        }
        return ({})
    }

    function callIndexFor(method) {
        return callIndexForHost(fakeHost, method)
    }

    function callIndexForHost(host, method) {
        for (let i = 0; i < host.calls.length; ++i) {
            if (String(host.calls[i].method || "") === method) {
                return i
            }
        }
        return -1
    }

    function callCountFor(method) {
        return fakeHost.calls.filter(function (call) {
            return String(call.method || "") === method
        }).length
    }

    function runtimeOperationCallIndex(method) {
        for (let i = 0; i < fakeHost.calls.length; ++i) {
            const call = fakeHost.calls[i] || {}
            const request = call.method === "runtimeOperationStart" && call.args
                ? call.args[0] || null : null
            if (request && String(request.method || "") === String(method || "")) {
                return i
            }
        }
        return -1
    }

    function runtimeOperationCallCount(method) {
        return fakeHost.calls.filter(function (call) {
            const request = call.method === "runtimeOperationStart" && call.args
                ? call.args[0] || null : null
            return request && String(request.method || "") === String(method || "")
        }).length
    }

    function runtimeEventsResponse(operationId, backend, eventCursor) {
        const cursor = Number(eventCursor)
        return {
            ok: true,
            value: {
                operation: {
                    operationId: String(operationId || ""),
                    domain: "storage",
                    backend: String(backend || "rest"),
                    method: "storageManifests",
                    status: "running",
                    eventCursor: cursor,
                    progress: 0.5,
                    bytesWritten: 50,
                    updatedAt: 2
                },
                events: [{
                    operationId: String(operationId || ""),
                    seq: cursor,
                    eventCursor: cursor,
                    phase: "running"
                }],
                oldestSeq: cursor,
                nextSeq: cursor + 1,
                eventCursor: cursor,
                droppedCount: 0,
                coalescedCount: 0,
                retainedCount: 1,
                retainedBytes: 128,
                historyTruncated: false,
                resetRequired: false
            },
            text: "OK",
            error: ""
        }
    }

    function test_basecamp_bridge_routes_inspector_calls_through_generic_call() {
        const response = basecampBridgeClient.callModule("logos_inspector", "blockchainLiveBlocks", ["http://127.0.0.1:8080", 1, 2, 3])

        compare(basecampHost.callCount, 1)
        compare(basecampHost.lastModule, "logos_inspector")
        compare(basecampHost.lastMethod, "call")
        compare(basecampHost.lastArgs[0], "blockchainLiveBlocks")
        compare(JSON.parse(basecampHost.lastArgs[1])[3], 3)
        verify(response.ok)
        compare(response.value.method, "blockchainLiveBlocks")
        compare(response.value.args[1], 1)
    }

    function test_appmodel_composition_exposes_focused_facades() {
        verify(model.shell !== null)
        verify(model.sourceRouting !== null)
        verify(model.backupImport !== null)
        verify(model.wallet !== null)
        verify(model.metrics !== null)
        verify(model.entityNavigation !== null)
        const retiredMembers = [
            "currentView", "statusText", "busy", "resultTitle", "resultText", "resultValue",
            "setResult", "clearResult", "deliverySourceReportArgs", "deliverySourceLabel",
            "storageSourceReportArgs", "storageSourceLabel", "effectiveStorageSourceMode",
            "sourceHealth", "sourceCapability", "sourceProbeValue",
            "previewLocalSettingsImportPlan", "restoreLocalSettingsBackup",
            "backupImportDecisionSummaryText", "uploadBackupCatalogEntry",
            "createWalletAccount", "sendWalletTransaction", "readIncomingWalletTransactions",
            "runWalletCommand", "syncPrivateWallet", "queryLocalWalletAccounts",
            "queryBedrockWalletBalance", "dashboardMetricValue", "dashboardMetricText",
            "openMetricValue", "moduleReport", "moduleProbeValue", "defaultFooterFieldSelections",
            "routeSearch", "openStorageCid", "openBlockchainBlock", "openMantleTransaction",
            "openLocalWallet"
        ]
        for (let i = 0; i < retiredMembers.length; ++i) {
            compare(model[retiredMembers[i]], undefined, retiredMembers[i])
        }
    }

    function test_metrics_compatibility_manifest_matches_appmodel() {
        const inventory = MetricsCompatibilityManifest.manifest()
        const seen = ({})
        const requiredProperties = ({})
        const requiredMethods = ({})
        let propertyCount = 0
        let methodCount = 0

        compare(inventory.ownerPath, "metrics")
        compare(inventory.retainedMembers.length, 0)
        verify(inventory.retainedDecision.length > 0)
        compare(inventory.retiredMembers.length, 98)
        for (let i = 0; i < inventory.retiredMembers.length; ++i) {
            const member = inventory.retiredMembers[i]
            verify(member.name.length > 0)
            verify(seen[member.name] !== true, member.name)
            seen[member.name] = true
            compare(member.ownerPath, "metrics", member.name)
            verify(member.ownerMember.length > 0, member.name)
            verify(Array.isArray(member.formerConsumers), member.name)
            verify(member.reason.length > 0, member.name)
            compare(model[member.name], undefined, member.name)
            if (member.kind === "property") {
                propertyCount += 1
                verify(model.metrics[member.ownerMember] !== undefined, member.name)
                requiredProperties[member.ownerMember] = true
            } else if (member.kind === "method") {
                methodCount += 1
                compare(typeof model.metrics[member.ownerMember], "function", member.name)
                requiredMethods[member.ownerMember] = true
            } else {
                fail("Unknown inventory kind: " + member.kind)
            }
        }
        compare(propertyCount, 22)
        compare(methodCount, 76)
        compare(Object.keys(requiredProperties).sort().join("|"),
            inventory.requiredFacadeProperties.slice(0).sort().join("|"))
        compare(Object.keys(requiredMethods).sort().join("|"),
            inventory.requiredFacadeMethods.slice(0).sort().join("|"))
    }

    function test_source_routing_compatibility_manifest_matches_appmodel() {
        const inventory = SourceRoutingCompatibilityManifest.manifest()
        const seen = ({})
        const methodClasses = ({ production: 0, test_only: 0, none: 0 })
        let methodCount = 0
        let aliasCount = 0

        compare(inventory.retainedAliases.length, 0)
        verify(inventory.retainedAliasDecision.length > 0)
        compare(inventory.retiredMembers.length, 34)
        for (let i = 0; i < inventory.retiredMembers.length; ++i) {
            const member = inventory.retiredMembers[i]
            verify(member.name.length > 0)
            verify(seen[member.name] !== true, member.name)
            seen[member.name] = true
            verify(Array.isArray(member.formerConsumers), member.name)
            compare(model[member.name], undefined, member.name)
            if (member.kind === "method") {
                methodCount += 1
                methodClasses[member.consumerClass] += 1
            } else if (member.kind === "alias") {
                aliasCount += 1
            } else {
                fail("Unknown inventory kind: " + member.kind)
            }
        }
        compare(methodCount, 32)
        compare(aliasCount, 2)
        compare(methodClasses.production, 11)
        compare(methodClasses.test_only, 9)
        compare(methodClasses.none, 12)

        for (let j = 0; j < inventory.retainedCompositionMembers.length; ++j) {
            const retained = inventory.retainedCompositionMembers[j]
            verify(retained.reason.length > 0, retained.name)
            verify(retained.consumers.length > 0, retained.name)
            verify(model[retained.name] !== undefined, retained.name)
        }
        for (let k = 0; k < inventory.requiredFacadeMethods.length; ++k) {
            const method = inventory.requiredFacadeMethods[k]
            compare(typeof model.sourceRouting[method], "function", method)
        }
        for (let n = 0; n < inventory.requiredFacadeProperties.length; ++n) {
            const propertyName = inventory.requiredFacadeProperties[n]
            verify(model.sourceRouting[propertyName] !== undefined, propertyName)
        }
    }

    function test_basecamp_bridge_decodes_json_serialized_inspector_response() {
        basecampHost.serializeResults = true

        const response = basecampBridgeClient.callModule("logos_inspector", "blockchainLiveBlocks", ["http://127.0.0.1:8080", 1, 2, 3])

        compare(basecampHost.callCount, 1)
        compare(basecampHost.lastModule, "logos_inspector")
        compare(basecampHost.lastMethod, "call")
        verify(response.ok)
        compare(response.value.method, "blockchainLiveBlocks")
        compare(response.value.args[3], 3)
    }

    function test_basecamp_bridge_keeps_inspector_module_version_direct() {
        const response = basecampBridgeClient.callModule("logos_inspector", "moduleVersion", [])

        compare(basecampHost.callCount, 1)
        compare(basecampHost.lastModule, "logos_inspector")
        compare(basecampHost.lastMethod, "moduleVersion")
        verify(response.ok)
        compare(response.value, "direct")
    }

    function test_core_source_args_keep_rpc_shape_in_standalone_rpc_connector() {
        compare(model.sourceRouting.effectiveCoreSourceMode(model.blockchainSourceMode), "rpc")

        const args = model.sourceRouting.blockchainArgs([1, 2])

        compare(args.length, 3)
        compare(args[0], model.nodeUrl)
        compare(args[1], 1)
        compare(args[2], 2)
    }

    function test_local_node_action_wrapper_dispatches_confirmation_token() {
        model.networkProfile = "local"
        fakeHost.callCount = 0
        fakeHost.lastMethod = ""
        fakeHost.lastArgs = []
        fakeHost.calls = []
        fakeHost.responses = ({
            localNodesAction: {
                ok: true,
                value: {
                    active_devnet: "devnet",
                    summary: { total: 0, installed: 0, running: 0, needs_configuration: 0 },
                    nodes: [],
                    operations: [{ action: "start", node: "bedrock", status: "started", detail: "ok" }],
                    tools: {}
                },
                text: "OK",
                error: ""
            },
            localDevnetList: {
                ok: true,
                value: { devnets: ["devnet"] },
                text: "OK",
                error: ""
            }
        })

        model.runLocalNodeAction("start", "bedrock", "", "", "Start Bedrock")

        tryCompare(fakeHost, "callCount", 2)
        compare(fakeHost.calls[0].method, "localNodesAction")
        compare(fakeHost.calls[0].args[0], "local")
        compare(fakeHost.calls[0].args[1].action, "start")
        compare(fakeHost.calls[0].args[1].node, "bedrock")
        compare(fakeHost.calls[0].args[2], "confirm-local-node-action")
        compare(fakeHost.calls[1].method, "localDevnetList")
        compare(model.localNodesOperations.length, 1)
        const localNodeHistory = model.runtimeOperationHistoryRows("localNodes")
        compare(localNodeHistory.length, 1)
        compare(localNodeHistory[0].label, "Start Bedrock")
        compare(localNodeHistory[0].status, "completed")
    }

    function test_local_node_observations_join_source_health_and_l2_heads() {
        model.metrics.networkConnectionStatus = ({
            blockchain: { known: true, ok: true, detail: "Online", checkedAt: "10:00" },
            storage: { known: true, ok: true, detail: "25 DHT peers", checkedAt: "10:00" },
            messaging: { known: true, ok: true, detail: "10 relay peers", checkedAt: "10:00" }
        })
        model.metrics.networkConnectionStatusRevision += 1
        model.metrics.storageSourceReport = ({
            health: { ready: true, status: "healthy", detail: "25 DHT peers" }
        })
        model.metrics.messagingSourceReport = ({
            health: { ready: true, status: "healthy", detail: "10 relay peers" }
        })
        model.dashboardOverview = ({
            sequencer: { head: { ok: true, value: 22418 } },
            indexer: {
                health: { ok: true, value: "reachable" },
                head: { ok: true, value: 22352 }
            }
        })

        const observations = model.localNodeObservedNodes()

        compare(observations.bedrock.status, "healthy")
        compare(observations.storage.status, "healthy")
        compare(observations.messaging.status, "healthy")
        compare(observations.indexer.status, "reachable")
        compare(observations.indexer.head, 22352)
        compare(observations.indexer.upstream_head, 22418)
        compare(model.localNodes.observedRunState("indexer"), "online")
    }

    function test_messaging_and_storage_use_standalone_connectors_without_basecamp() {
        compare(model.sourceRouting.normalizedMessagingSourceMode(model.messagingSourceMode), "rest")
        compare(model.sourceRouting.effectiveMessagingSourceMode(model.messagingSourceMode), "rest")
        const deliveryArgs = model.sourceRouting.deliverySourceReportArgs()
        compare(deliveryArgs.length, 1)
        compare(deliveryArgs[0].source_mode, "rest")
        compare(deliveryArgs[0].inputs.rest_endpoint, model.sourceRouting.configuredMessagingRestUrl())
        compare(deliveryArgs[0].inputs.metrics_endpoint, model.messagingMetricsUrl)
        compare(model.sourceRouting.deliverySourceTarget(), model.sourceRouting.configuredMessagingRestUrl())

        compare(model.sourceRouting.normalizedStorageSourceMode(model.storageSourceMode), "rest")
        compare(model.sourceRouting.effectiveStorageSourceMode(model.storageSourceMode), "rest")
        const storageArgs = model.sourceRouting.storageSourceReportArgs(false)
        compare(storageArgs.length, 1)
        compare(storageArgs[0].source_mode, "rest")
        compare(storageArgs[0].inputs.rest_endpoint, model.sourceRouting.configuredStorageRestUrl())
        compare(storageArgs[0].inputs.metrics_endpoint, model.storageMetricsUrl)
        compare(storageArgs[0].options.privileged_debug_enabled, false)
        compare(model.sourceRouting.storageSourceTarget(), model.sourceRouting.configuredStorageRestUrl())
    }

    function test_source_routing_state_owns_runtime_source_views() {
        const delivery = model.sourceRouting.deliverySourceView()
        compare(delivery.mode, "rest")
        compare(delivery.effectiveMode, "rest")
        compare(delivery.label, "Direct Waku REST")
        compare(delivery.target, model.sourceRouting.configuredMessagingRestUrl())
        const deliveryArgs = delivery.reportArgs()
        compare(deliveryArgs[0].source_mode, "rest")
        compare(deliveryArgs[0].inputs.rest_endpoint, model.sourceRouting.configuredMessagingRestUrl())

        const storage = model.sourceRouting.storageSourceView()
        compare(storage.mode, "rest")
        compare(storage.effectiveMode, "rest")
        compare(storage.target, model.sourceRouting.configuredStorageRestUrl())
        const storageArgs = storage.reportArgs(false)
        compare(storageArgs[0].source_mode, "rest")
        compare(storageArgs[0].inputs.rest_endpoint, model.sourceRouting.configuredStorageRestUrl())

    }

    function test_source_policy_catalog_fallback_supports_pending_modes_without_bridge_policy() {
        model.sourceRouting.sourcePolicy = ({})
        model.sourceRouting.sourcePolicyLoaded = false

        compare(model.sourceRouting.normalizedMessagingSourceMode("metrics"), "metrics")
        compare(model.sourceRouting.normalizedMessagingSourceMode("network-monitor"), "network-monitor")
        compare(model.sourceRouting.normalizedMessagingSourceMode("delivery network monitor"), "network-monitor")
        compare(model.sourceRouting.normalizedMessagingSourceMode("network monitor"), "rest")
        compare(model.sourceRouting.normalizedStorageSourceMode("metrics"), "metrics")

        model.setNetworkConnectorMode("delivery", "network-monitor")
        compare(model.sourceRouting.effectiveMessagingSourceMode(model.messagingSourceMode), "network-monitor")
        compare(model.sourceRouting.deliverySourceReportArgs()[0].source_mode, "network-monitor")
        compare(model.sourceRouting.deliverySourceTarget(), model.sourceRouting.configuredMessagingRestUrl())

        model.setNetworkConnectorMode("storage", "metrics")
        compare(model.sourceRouting.effectiveStorageSourceMode(model.storageSourceMode), "metrics")
        compare(model.sourceRouting.storageSourceReportArgs(false)[0].source_mode, "metrics")
        compare(model.sourceRouting.storageSourceTarget(), model.storageMetricsUrl)

        const deliveryOptions = model.sourceRouting.sourceModeOptions("delivery")
        const deliveryKeys = deliveryOptions.map(option => option.key)
        verify(deliveryKeys.indexOf("metrics") >= 0)
        verify(deliveryKeys.indexOf("network-monitor") >= 0)
        compare(sourceOption(deliveryOptions, "network-monitor").label, "Delivery Network Monitor")
    }

    function test_messaging_and_storage_use_module_connectors_in_basecamp() {
        compare(basecampModel.sourceRouting.effectiveMessagingSourceMode(basecampModel.messagingSourceMode), "module")
        const deliveryArgs = basecampModel.sourceRouting.deliverySourceReportArgs()
        compare(deliveryArgs.length, 1)
        compare(deliveryArgs[0].source_mode, "module")
        compare(Object.keys(deliveryArgs[0].inputs).length, 0)
        compare(basecampModel.sourceRouting.deliverySourceTarget(), basecampModel.deliveryModule)

        compare(basecampModel.sourceRouting.effectiveStorageSourceMode(basecampModel.storageSourceMode), "module")
        const storageArgs = basecampModel.sourceRouting.storageSourceReportArgs(false)
        compare(storageArgs.length, 1)
        compare(storageArgs[0].source_mode, "module")
        compare(Object.keys(storageArgs[0].inputs).length, 0)
        compare(storageArgs[0].options.privileged_debug_enabled, false)
        compare(basecampModel.sourceRouting.storageSourceTarget(), basecampModel.storageModule)
    }

    function test_standalone_defaults_l1_to_direct_rpc_and_hides_host_module_connector() {
        const defaults = model.defaultNetworkConnectorConfig().scopes
        compare(defaults.l1.connector_id, "direct_l1_rpc")
        compare(defaults.delivery.connector_id, "logoscore_cli_delivery_module")
        compare(defaults.storage.connector_id, "logoscore_cli_storage_module")

        const options = model.sourceRouting.sourceModeOptions("storage")
        verify(sourceOption(options, "logoscore_cli") !== null)
        compare(String(sourceOption(options, "module").key || ""), "")

        const basecampOptions = basecampModel.sourceRouting.sourceModeOptions("storage")
        verify(sourceOption(basecampOptions, "module") !== null)
        verify(sourceOption(basecampOptions, "logoscore_cli") !== null)
    }

    function test_standalone_normalizes_persisted_host_modules_to_build_defaults() {
        const persisted = {
            scopes: {
                l1: { connector_id: "blockchain_module", provenance: "network_profile" },
                delivery: { connector_id: "delivery_module", provenance: "network_profile" },
                storage: { connector_id: "storage_module", provenance: "network_profile" }
            }
        }

        const standalone = model.normalizedNetworkConnectorConfig(persisted).scopes
        compare(standalone.l1.connector_id, "direct_l1_rpc")
        compare(standalone.delivery.connector_id, "logoscore_cli_delivery_module")
        compare(standalone.storage.connector_id, "logoscore_cli_storage_module")
        compare(standalone.storage.provenance, "build_default")

        const basecamp = basecampModel.normalizedNetworkConnectorConfig(persisted).scopes
        compare(basecamp.l1.connector_id, "blockchain_module")
        compare(basecamp.delivery.connector_id, "delivery_module")
        compare(basecamp.storage.connector_id, "storage_module")
    }

    function test_basecamp_translates_only_testnet_default_connector_scopes() {
        basecampModel.loadNetworkConnectorConfig({
            network_connector_config: {
                scopes: {
                    l1: {
                        connector_id: "logoscore_cli_blockchain_module",
                        provenance: "testnet_default"
                    },
                    delivery: {
                        connector_id: "direct_delivery_rest",
                        endpoint: "https://delivery.custom.example/",
                        provenance: "network_profile"
                    },
                    storage: {
                        connector_id: "logoscore_cli_storage_module",
                        provenance: "testnet_default"
                    }
                }
            }
        })

        const scopes = basecampModel.networkConnectorConfig.scopes
        compare(scopes.l1.connector_id, "blockchain_module")
        compare(scopes.delivery.connector_id, "direct_delivery_rest")
        compare(scopes.delivery.endpoint, "https://delivery.custom.example/")
        compare(scopes.storage.connector_id, "storage_module")
    }

    function test_local_network_profile_remains_selectable_when_endpoint_matches_default() {
        const options = model.networkProfileOptions()
        const local = options.find(function (option) { return option.key === "local" })

        verify(local !== undefined)
        compare(local.label, "Local node")
    }

    function test_source_policy_load_supplies_defaults_and_profile_matching() {
        fakeHost.responses = ({
            sourcePolicy: {
                ok: true,
                value: {
                    defaults: {
                        node_endpoint: "http://policy-node.invalid/",
                        delivery_rest_endpoint: "http://policy-delivery.invalid:8645",
                        delivery_metrics_endpoint: "http://policy-delivery.invalid:8008/metrics",
                        storage_rest_endpoint: "http://policy-storage.invalid/api/storage/v1",
                        storage_metrics_endpoint: "http://policy-storage.invalid:8008/metrics"
                    },
                    network_profiles: [
                        {
                            id: "default",
                            node_endpoint: "http://policy-node.invalid/"
                        },
                        {
                            id: "local",
                            node_endpoint: "http://policy-local-node.invalid/"
                        }
                    ],
                    source_modes: {
                        core: [
                            { key: "rpc", aliases: ["rpc"], effective: "rpc", adapter: { connector_id: "direct_l1_rpc", inputs: [{ key: "rpc_endpoint" }] } },
                            { key: "module", aliases: ["basecamp"], effective: "module", adapter: { connector_id: "blockchain_module", inputs: [] } }
                        ],
                        delivery: [
                            { key: "rest", aliases: ["direct waku rest"], effective: "rest" },
                            { key: "network-monitor", aliases: ["discovery crawler"], effective: "network-monitor" }
                        ],
                        storage: [
                            { key: "rest", aliases: ["standalone rest"], effective: "rest" },
                            { key: "module", aliases: ["basecamp module"], effective: "module" }
                        ]
                    }
                },
                text: "OK",
                error: ""
            }
        })

        verify(model.sourceRouting.loadSourcePolicy())
        compare(fakeHost.lastMethod, "sourcePolicy")
        verify(model.sourceRouting.sourcePolicyLoaded)

        model.messagingRestUrl = ""
        model.storageRestUrl = ""
        compare(model.sourceRouting.configuredMessagingRestUrl(), "http://policy-delivery.invalid:8645")
        compare(model.sourceRouting.configuredStorageRestUrl(), "http://policy-storage.invalid/api/storage/v1")
        compare(model.sourceRouting.normalizedCoreSourceMode("basecamp"), "module")
        compare(model.sourceRouting.effectiveCoreSourceMode("basecamp"), "rpc")
        model.setNetworkConnectorMode("l1", "module")
        compare(model.sourceRouting.effectiveCoreSourceMode("basecamp"), "module")
        compare(model.sourceRouting.normalizedMessagingSourceMode("direct waku rest"), "rest")
        compare(model.sourceRouting.normalizedStorageSourceMode("standalone rest"), "rest")

        model.applyProfile(1)
        compare(model.nodeUrl, "http://policy-local-node.invalid/")
        compare(model.inferNetworkProfileFromEndpoint(model.nodeUrl), "local")

        model.applyProfile(0)
        compare(model.nodeUrl, "http://policy-node.invalid/")
        compare(model.inferNetworkProfileFromEndpoint(model.nodeUrl), "default")
    }

    function test_settings_query_caches_blockchain_node_for_footer_metrics() {
        const nodeResult = {
            cryptarchia_info: {
                ok: true,
                value: { cryptarchia_info: { slot: 77, lib_slot: 70 } },
                error: null
            },
            network_info: {
                ok: true,
                value: { n_peers: 4 },
                error: null
            }
        }
        fakeHost.responses = {
            runtimeOperationStart: chainRuntimeStart({ blockchainNode: nodeResult })
        }

        model.metrics.queryNetworkConnection("blockchain", false)

        tryVerify(function () { return model.metrics.networkConnectionIsPending("blockchain") === false })
        compare(model.metrics.cryptarchiaValue("slot"), 77)
        compare(model.metrics.networkValue("n_peers"), 4)
        compare(runtimeOperationCallCount("blockchainNode"), 1)
        compare(callCountFor("blockchainNode"), 0)
    }

    function test_default_footer_storage_failure_field_is_registered_recent_key() {
        const defaults = model.metrics.defaultFooterFieldSelections()

        verify(defaults["storage.failed_transfers_recent"] === true)
        verify(defaults["storage.failed_transfers_total"] !== true)
    }

    function test_status_facts_dashboard_projection_owns_graph_keys_and_labels() {
        const keys = StatusFactsProjection.dashboardGraphKeys()

        verify(keys.indexOf("storage.failed_transfers_recent") >= 0)
        verify(keys.indexOf("storage.failed_transfers_total") >= 0)
        verify(keys.indexOf("messaging.network_ingress_recent") >= 0)
        compare(StatusFactsProjection.dashboardMetricLabel("storage.failed_transfers_total"), "transfer failures total")
        compare(StatusFactsProjection.dashboardMetricGroup("messaging.network_ingress_recent"), "Messaging / Delivery")
        compare(StatusFactsProjection.dashboardMetricTone("storage.failed_transfers_recent", 2), "error")
        compare(StatusFactsProjection.dashboardMetricTone("storage.failed_transfers_total", 2), "neutral")
    }

    function test_explicit_rest_blank_urls_use_visible_defaults() {
        model.setNetworkConnectorMode("delivery", "rest")
        model.messagingRestUrl = ""
        const deliveryArgs = model.sourceRouting.deliverySourceReportArgs()
        compare(deliveryArgs[0].source_mode, "rest")
        compare(deliveryArgs[0].inputs.rest_endpoint, "http://127.0.0.1:8645")
        compare(model.sourceRouting.deliverySourceTarget(), "http://127.0.0.1:8645")

        model.setNetworkConnectorMode("storage", "rest")
        model.storageRestUrl = ""
        const storageArgs = model.sourceRouting.storageSourceReportArgs(false)
        compare(storageArgs[0].source_mode, "rest")
        compare(storageArgs[0].inputs.rest_endpoint, "http://127.0.0.1:8080/api/storage/v1")
        compare(model.sourceRouting.storageSourceTarget(), "http://127.0.0.1:8080/api/storage/v1")
    }

    function test_storage_module_connector_uses_module_route() {
        installSourceModePolicy(model)

        compare(model.sourceRouting.normalizedStorageSourceMode("module"), "module")
        model.setNetworkConnectorMode("storage", "module")
        compare(model.sourceRouting.effectiveStorageSourceMode(model.storageSourceMode), "module")
        const storageArgs = model.sourceRouting.storageSourceReportArgs(false)
        compare(storageArgs[0].source_mode, "module")
        compare(Object.keys(storageArgs[0].inputs).length, 0)
        compare(storageArgs[0].options.privileged_debug_enabled, false)
        compare(model.sourceRouting.storageSourceTarget(), model.storageModule)
    }

    function test_delivery_network_monitor_source_is_supported() {
        installSourceModePolicy(model)

        compare(model.sourceRouting.normalizedMessagingSourceMode("network-monitor"), "network-monitor")
        compare(model.sourceRouting.normalizedMessagingSourceMode("delivery network monitor"), "network-monitor")
        compare(model.sourceRouting.normalizedMessagingSourceMode("discovery crawler"), "network-monitor")
        compare(model.sourceRouting.normalizedMessagingSourceMode("network monitor"), "rest")
        compare(model.sourceRouting.normalizedMessagingSourceMode("crawler"), "rest")
        model.setNetworkConnectorMode("delivery", "network-monitor")

        compare(model.sourceRouting.effectiveMessagingSourceMode(model.messagingSourceMode), "network-monitor")
        const deliveryArgs = model.sourceRouting.deliverySourceReportArgs()
        compare(deliveryArgs[0].source_mode, "network-monitor")
        compare(deliveryArgs[0].inputs.rest_endpoint, model.sourceRouting.configuredMessagingRestUrl())
        compare(deliveryArgs[0].inputs.metrics_endpoint, model.messagingMetricsUrl)
        compare(model.sourceRouting.deliverySourceTarget(), model.sourceRouting.configuredMessagingRestUrl())
    }

    function test_source_mode_options_labels_and_targets_come_from_policy() {
        installSourceModePolicy(model)

        const storageOptions = model.sourceRouting.sourceModeOptions("storage")
        verify(storageOptions.length >= 2)
        compare(sourceOption(storageOptions, "rest").label, "Standalone REST")

        model.setNetworkConnectorMode("storage", "module")
        compare(model.sourceRouting.storageSourceLabel(), "Storage module")
        compare(model.sourceRouting.storageSourceTarget(), model.storageModule)
        verify(model.sourceRouting.sourceModeSupportsCidProbe("storage", model.storageSourceMode))
        verify(model.sourceRouting.sourceModeSupportsMutatingDiagnostics("storage", model.storageSourceMode))

        model.setNetworkConnectorMode("delivery", "metrics")
        compare(model.sourceRouting.deliverySourceLabel(), "Metrics only")
        compare(model.sourceRouting.deliverySourceTarget(), model.messagingMetricsUrl)
        verify(model.sourceRouting.sourceModeUsesEndpoint("delivery", model.messagingSourceMode, "metrics"))
    }

    function test_source_report_health_facts_drive_connection_state_without_probes() {
        const report = {
            module: "delivery_rest",
            health: {
                reachable: true,
                ready: true,
                status: "healthy",
                summary: "delivery source ready",
                detail: "node health Ready; connection Connected"
            },
            capability_facts: [
                { key: "metrics", label: "Metrics", available: true, evidence: "known Waku metric family observed" }
            ],
            probes: []
        }

        verify(model.metrics.moduleReportReachable(report))
        verify(model.metrics.deliveryReportHealthy(report))
        compare(model.metrics.networkConnectionSummary("messaging", report), "delivery source ready")
        verify(model.metrics.sourceCapabilityAvailable(report, "metrics"))
        compare(model.metrics.sourceCapabilityEvidence(report, "metrics"), "known Waku metric family observed")
    }

    function test_source_report_health_facts_mark_storage_not_ready_without_probe_names() {
        const report = {
            module: "storage_rest",
            health: {
                reachable: true,
                ready: false,
                status: "degraded",
                summary: "storage source degraded",
                detail: "required storage facts missing"
            },
            capability_facts: [
                { key: "identity", label: "Identity", available: false, evidence: "not observed" }
            ],
            probes: []
        }

        verify(model.metrics.moduleReportReachable(report))
        verify(!model.metrics.storageReportReady(report))
        compare(model.metrics.networkConnectionSummary("storage", report), "required storage facts missing")
        compare(model.metrics.sourceCapabilityAvailable(report, "identity"), false)
    }

    function test_module_probe_lookup_ignores_source_facts_without_probe_names() {
        model.metrics.storageModuleReport = {
            module: "storage_rest",
            probe_facts: [
                {
                    key: "peerId",
                    label: "renamed",
                    source: "opaque",
                    ok: true,
                    value: "peer-from-fact",
                    error: null
                },
                {
                    key: "collectMetrics",
                    label: "renamed metrics",
                    source: "opaque",
                    ok: false,
                    value: null,
                    error: "metrics unavailable"
                }
            ],
            probes: [
                {
                    label: "unrelated",
                    source: "opaque",
                    ok: true,
                    value: "wrong"
                }
            ]
        }

        compare(model.metrics.moduleProbeValue("storage", "peerId"), null)
        compare(model.metrics.moduleProbeError("storage", "collectMetrics"), "")
    }

    function test_source_diagnostics_prefer_current_report_facts() {
        model.metrics.storageModuleReport = {
            module: "storage_rest",
            probe_facts: [
                {
                    key: "space",
                    label: "stale",
                    source: "old",
                    ok: true,
                    value: "stale-space",
                    error: null
                }
            ],
            probes: []
        }
        const report = {
            module: "storage_rest",
            probe_facts: [
                {
                    key: "space",
                    label: "current",
                    source: "new",
                    ok: true,
                    value: "current-space",
                    error: null
                },
                {
                    key: "manifests",
                    label: "failed",
                    source: "new",
                    ok: false,
                    value: null,
                    error: "manifest probe failed"
                }
            ],
            probes: []
        }

        compare(SourceDiagnostics.probeValue(model, "storage", report, "space"), "current-space")
        compare(SourceDiagnostics.failedProbeCount(report), 1)
    }

    function test_source_report_view_uses_fact_only_source_evidence() {
        const report = {
            module: "storage_rest",
            health: {
                reachable: true,
                ready: true,
                status: "healthy",
                summary: "storage source ready",
                detail: "space observed"
            },
            probe_facts: [
                {
                    key: "space",
                    label: "Repository space",
                    source: "storage",
                    ok: true,
                    value: { used: 1 },
                    error: null
                }
            ],
            capability_facts: [
                {
                    key: "space",
                    label: "Repository space",
                    available: true,
                    evidence: "1 field(s)",
                    value: { used: 1 }
                }
            ],
            probes: []
        }
        const view = model.sourceRouting.storageReportView(report)

        compare(view.probeValue("space").used, 1)
        verify(view.capabilityAvailable("space"))
    }

    function test_delivery_source_throughput_metric_aliases() {
        model.metrics.messagingSourceReport = {
            module: "delivery_metrics",
            probes: [
                {
                    label: "delivery_metrics.collectOpenMetricsText",
                    ok: true,
                    value: [
                        "libp2p_network_bytes_total{direction=\"in\"} 20",
                        "waku_service_requests_total{service=\"/vac/waku/store-query/3.0.0\"} 4",
                        "waku_store_messages 7"
                    ].join("\n")
                }
            ]
        }

        compare(model.metrics.dashboardMetricRawValue("messaging.network_ingress_recent"), 20)
        compare(model.metrics.dashboardMetricRawValue("messaging.store_query_requests_recent"), 4)
        compare(model.metrics.dashboardMetricRawValue("messaging.store_messages"), 7)
    }

    function test_node_operation_start_dispatches_generic_request() {
        fakeHost.responses = {
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "op-2",
                    domain: "delivery",
                    method: "deliverySend",
                    status: "running",
                    label: "Send message"
                },
                text: "OK",
                error: ""
            }
        }
        let seen = null

        model.runtimeOperationStart({
            domain: "delivery",
            method: "deliverySend",
            args: ["rest", "http://127.0.0.1:8645", true, "/topic/1/a/proto", "hello"],
            label: "Send message"
        }, false, function (response) {
            seen = response
        })

        tryVerify(function () { return seen !== null })
        compare(fakeHost.lastMethod, "runtimeOperationStart")
        compare(fakeHost.lastArgs[0].method, "deliverySend")
        compare(model.runtimeOperations["op-2"].domain, "delivery")
    }

    function test_runtime_events_poll_is_completion_paced_per_operation() {
        const operationId = "events-one-in-flight"
        model.updateRuntimeOperation({
            operationId: operationId,
            domain: "storage",
            backend: "rest",
            method: "storageManifests",
            status: "running",
            eventCursor: 0,
            progress: 0,
            bytesWritten: 0,
            updatedAt: 1
        })
        fakeHost.responses = {
            runtimeOperationEvents: runtimeEventsResponse(operationId, "rest", 1)
        }
        let callbackCount = 0

        const first = model.runtimeOperationEvents(operationId, 0, false, function () {
            callbackCount += 1
        })
        const duplicate = model.runtimeOperationEvents(operationId, 0, false, function () {
            callbackCount += 1
        })

        verify(first !== null)
        compare(duplicate, null)
        compare(model.operationHistory.pendingEventPollCount, 1)
        tryCompare(model.operationHistory, "pendingEventPollCount", 0)
        compare(callbackCount, 1)
        compare(callCountFor("runtimeOperationEvents"), 1)
        compare(model.runtimeOperationEventSeq[operationId], 1)
        compare(model.operationHistory.eventFacts(operationId).nextSeq, 2)
    }

    function test_runtime_events_poll_rejects_stale_configuration_and_backend() {
        const operationId = "events-stale-context"
        model.updateRuntimeOperation({
            operationId: operationId,
            domain: "storage",
            backend: "rest",
            method: "storageManifests",
            status: "running",
            eventCursor: 0,
            progress: 0,
            bytesWritten: 0,
            updatedAt: 1
        })
        fakeHost.responses = {
            runtimeOperationEvents: runtimeEventsResponse(operationId, "rest", 1)
        }
        let callbackCount = 0

        model.runtimeOperationEvents(operationId, 0, false, function () {
            callbackCount += 1
        })
        model.networkConfigurationRevision += 1
        tryCompare(model.operationHistory, "pendingEventPollCount", 0)
        compare(callbackCount, 0)
        compare(model.runtimeOperationEventSeq[operationId], undefined)

        model.runtimeOperationEvents(operationId, 0, false, function () {
            callbackCount += 1
        })
        model.updateRuntimeOperation({
            operationId: operationId,
            domain: "storage",
            backend: "module",
            method: "storageManifests",
            status: "running",
            eventCursor: 1,
            progress: 0.1,
            bytesWritten: 10,
            updatedAt: 2
        })
        tryCompare(model.operationHistory, "pendingEventPollCount", 0)
        compare(callbackCount, 0)
        compare(model.runtimeOperationEventSeq[operationId], undefined)
        compare(model.runtimeOperations[operationId].backend, "module")
    }

    function test_runtime_events_poll_surfaces_malformed_typed_window() {
        const operationId = "events-malformed-window"
        model.updateRuntimeOperation({
            operationId: operationId,
            domain: "storage",
            backend: "rest",
            method: "storageManifests",
            status: "running",
            eventCursor: 0,
            progress: 0,
            bytesWritten: 0,
            updatedAt: 1
        })
        const malformed = runtimeEventsResponse(operationId, "rest", 1)
        malformed.value.nextSeq = "2"
        fakeHost.responses = { runtimeOperationEvents: malformed }
        let received = null

        model.runtimeOperationEvents(operationId, 0, false, function (response) {
            received = response
        })

        tryVerify(function () { return received !== null })
        verify(!received.ok)
        verify(String(received.error || "").indexOf(
            "invalid runtime operation event window: cursor_facts") >= 0)
        compare(model.operationHistory.pendingEventPollCount, 0)
        compare(model.runtimeOperationEventSeq[operationId], undefined)
        compare(model.runtimeOperations[operationId].eventCursor, 0)
    }

    function test_storage_operation_session_projects_through_runtime_gateway() {
        fakeHost.responses = {
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "storage-op-1",
                    domain: "storage",
                    method: "storageUploadUrl",
                    status: "awaiting_external",
                    label: "Upload",
                    moduleSessionId: "session-1"
                },
                text: "OK",
                error: ""
            },
            runtimeOperationStatus: {
                ok: true,
                value: {
                    operationId: "storage-op-1",
                    domain: "storage",
                    method: "storageUploadUrl",
                    status: "completed",
                    label: "Upload",
                    result: { cid: "cid-1" }
                },
                text: "OK",
                error: ""
            }
        }

        model.storageApp.operationSession.start(
            "storageUploadUrl",
            ["/tmp/file.bin", 65536],
            "Upload"
        )
        tryVerify(function () {
            return model.runtimeOperations["storage-op-1"] !== undefined
        })
        compare(model.runtimeOperations["storage-op-1"].status, "awaiting_external")

        model.storageApp.operationSession.poll(false)
        tryVerify(function () {
            return model.runtimeOperations["storage-op-1"].status === "completed"
        })
    }

    function test_delivery_store_result_keeps_messaging_owner_after_navigation() {
        model.shell.currentView = "overview"

        model.deliveryApp.setDeliveryStoreQueryResult({
            operationId: "delivery-store-owner",
            domain: "delivery",
            method: "deliveryStoreQuery",
            status: "completed",
            label: "Store query",
            result: { value: { messages: [] } }
        })

        compare(model.shell.resultTitle, "Store query")
        compare(model.shell.resultOwner, "messaging")
        verify(!model.shell.resultIsError)
    }

    function test_runtime_module_event_projects_only_returned_operation() {
        fakeHost.responses = {
            runtimeOperationModuleEvent: {
                ok: true,
                value: {
                    disposition: "applied",
                    operation: {
                        operationId: "op-event",
                        domain: "storage",
                        method: "storageUploadUrl",
                        status: "completed",
                        moduleSessionId: "session-1"
                    }
                },
                text: "OK",
                error: ""
            }
        }
        let seen = null

        model.runtimeOperationModuleEvent({
            moduleName: "storage_module",
            eventName: "storageUploadDone",
            args: [JSON.stringify({ sessionId: "session-1", success: true })]
        }, false, function (response) {
            seen = response
        })

        tryVerify(function () { return seen !== null })
        compare(fakeHost.lastMethod, "runtimeOperationModuleEvent")
        compare(fakeHost.lastArgs[0].moduleName, "storage_module")
        compare(fakeHost.lastArgs[0].eventName, "storageUploadDone")
        compare(model.runtimeOperations["op-event"].status, "completed")
        compare(model.runtimeOperations["op-event"].moduleSessionId, "session-1")
    }

    function test_node_operation_history_filters_by_domain() {
        const storageOperation = {
            operationId: "op-storage",
            domain: "storage",
            method: "storageFetch",
            status: "completed",
            label: "Cache CID",
            result: { cid: "z-storage" }
        }
        const deliveryOperation = {
            operationId: "op-delivery",
            domain: "delivery",
            method: "deliverySend",
            status: "failed",
            label: "Send message",
            error: "send failed"
        }

        model.appendRuntimeOperationHistory(storageOperation, "")
        model.appendRuntimeOperationHistory(deliveryOperation, "")

        const storageRows = model.runtimeOperationHistoryRows("storage")
        compare(storageRows.length, 1)
        compare(storageRows[0].operationId, "op-storage")
        compare(storageRows[0].detail, "z-storage")
        compare(model.runtimeOperationHistoryRows("delivery")[0].detail, "send failed")
    }

    function test_wallet_profile_configured_accepts_checked_env_home_source() {
        model.walletBinary = "/usr/bin/lee-wallet"
        model.walletHome = ""
        model.localWalletStatus = readyWalletStatus("LEE_WALLET_HOME_DIR")

        verify(model.walletHomeConfigured())
        verify(model.walletProfileConfigured())
    }

    function test_wallet_profile_status_rejects_stale_profile_response() {
        model.walletBinary = "/usr/bin/lee-wallet"
        model.walletHome = "/tmp/wallet-home"
        fakeHost.responses = {
            localWalletProfileStatus: {
                ok: true,
                value: readyWalletStatus("profile"),
                text: "OK",
                error: ""
            }
        }

        model.checkLocalWalletProfile(false)
        model.walletHome = "/tmp/new-wallet-home"

        tryVerify(function () {
            return fakeHost.calls.some(function (call) {
                return call.method === "localWalletProfileStatus"
            })
        })
        compare(model.localWalletStatus, null)
    }

    function test_program_execution_reads_wallet_capability_facade() {
        model.networkProfile = "wallet-test"
        configureReadyWallet()

        const profile = model.programExecution.walletProfile()

        compare(profile.wallet_binary, "/usr/bin/lee-wallet")
        compare(profile.wallet_home, "/tmp/wallet-home")
        compare(profile.network_profile, "wallet-test")
        verify(model.programExecution.walletProfileConfigured())
        verify(model.programExecution.walletHomeConfigured())
    }

    function test_navigation_delegates() {
        compare(model.viewTitle(), "Dashboard")
        verify(model.navRows().length > 0)

        model.selectView("programs")

        compare(model.shell.currentView, "programs")
        compare(model.parentNavKeyForView("programs"), "local")
        compare(model.navTokenForView("programs"), "IDL")
    }

    function test_favorites_toggle_and_filter_rows() {
        setActiveZone("")
        const blockEntry = model.favoriteStore.blockEntry({
            type: "blockchain_block",
            hash: "block-hash",
            slot: 12,
            height: 12
        })
        const txEntry = model.favoriteStore.transactionEntry({
            mode: "lez",
            hash: "tx-hash",
            kind: "transfer"
        }, zoneEntityRef("transaction", "tx-hash", "seq-a", "sequencer"))

        verify(blockEntry !== null)
        compare(blockEntry.kind, "block")
        compare(blockEntry.layer, "l1")
        verify(txEntry !== null)
        compare(txEntry.kind, "transaction")
        compare(txEntry.layer, "l2")

        verify(model.favoriteStore.add(blockEntry))
        verify(model.favoriteStore.add(txEntry))
        compare(model.favoriteStore.count("all"), 2)
        compare(model.favoriteStore.count("block"), 1)
        compare(model.favoriteStore.rows("block")[0].value, "block-hash")
        verify(model.favoriteStore.isFavoriteEntry(blockEntry))

        verify(model.favoriteStore.toggle(blockEntry))
        verify(!model.favoriteStore.isFavoriteEntry(blockEntry))
        compare(model.favoriteStore.count("all"), 1)
    }

    function test_favorites_persist_in_settings_state() {
        fakeHost.responses = {
            loadSettingsState: {
                ok: true,
                value: {
                    favorites: [
                        {
                            kind: "account",
                            layer: "l2",
                            value: "account-1",
                            open_kind: "account",
                            title: "Account account-1",
                            created_at: "2026-07-05T00:00:00.000Z"
                        }
                    ]
                },
                text: "OK",
                error: ""
            }
        }

        model.loadSettingsState()

        compare(model.favoriteStore.entries.length, 0)
        const accountRef = zoneEntityRef("account", "account-1", "idx-a", "indexer")
        accountRef.network_scope = Object.assign({}, accountRef.network_scope, {
            endpoint: "https://forbidden.example"
        })
        const accountEntry = model.favoriteStore.l2EntityEntry(accountRef,
            "Account account-1", "")
        verify(model.favoriteStore.add(accountEntry))
        compare(model.favoriteStore.entries.length, 1)
        compare(model.favoriteStore.entries[0].value, "account-1")
        const settingsPayload = model.settingsStatePayload()
        compare(settingsPayload.version, 2)
        compare(settingsPayload.favorites.length, 1)
        compare(settingsPayload.favorites[0].entity_ref.channel_id,
            model.zoneInspection.activeZoneId)
        verify(settingsPayload.favorites[0].entity_ref.network_scope.endpoint === undefined)
        verify(settingsPayload.sequencer_url === undefined)
        verify(settingsPayload.indexer_url === undefined)
        verify(settingsPayload.channel_source_configs === undefined)

        fakeHost.callCount = 0
        fakeHost.lastMethod = ""
        fakeHost.lastArgs = []
        const txEntry = {
            kind: "transaction",
            layer: "l1",
            value: "tx-1",
            open_kind: "mantleTransaction",
            title: "Mantle transaction tx-1",
            created_at: "2026-07-05T00:01:00.000Z"
        }

        verify(model.favoriteStore.add(txEntry))

        compare(fakeHost.lastMethod, "saveSettingsState")
        compare(fakeHost.lastArgs[0].favorites.length, 2)
    }

    function test_metrics_preferences_round_trip_settings_state() {
        const footerRevision = model.metrics.footerFieldRevision
        const graphRevision = model.metrics.dashboardGraphRevision
        fakeHost.responses = {
            loadSettingsState: {
                ok: true,
                value: {
                    blockchain_refresh_rate: 11,
                    messaging_refresh_rate: 22,
                    storage_refresh_rate: 33,
                    footer_fields: {
                        "storage.module": false,
                        "overall.status": true
                    },
                    dashboard_graphs: {
                        "bedrock.peer_count": true,
                        "storage.peer_count": false
                    }
                },
                text: "OK",
                error: ""
            }
        }

        model.loadSettingsState()

        compare(model.metrics.blockchainRefreshRate, 11)
        compare(model.metrics.messagingRefreshRate, 22)
        compare(model.metrics.storageRefreshRate, 33)
        verify(!model.metrics.footerFieldSelections["storage.module"])
        verify(model.metrics.footerFieldSelections["overall.status"])
        verify(model.metrics.dashboardGraphSelections["bedrock.peer_count"])
        verify(!model.metrics.dashboardGraphSelections["storage.peer_count"])
        compare(model.metrics.footerFieldRevision, footerRevision + 1)
        compare(model.metrics.dashboardGraphRevision, graphRevision + 1)

        const payload = model.settingsStatePayload()
        compare(payload.blockchain_refresh_rate, 11)
        compare(payload.messaging_refresh_rate, 22)
        compare(payload.storage_refresh_rate, 33)
        verify(!payload.footer_fields["storage.module"])
        verify(payload.footer_fields["overall.status"])
        verify(payload.dashboard_graphs["bedrock.peer_count"])
        verify(!payload.dashboard_graphs["storage.peer_count"])

        fakeHost.calls = []
        model.saveSettingsState()
        compare(fakeHost.lastMethod, "saveSettingsState")
        compare(fakeHost.lastArgs[0].storage_refresh_rate, 33)
        verify(fakeHost.lastArgs[0].footer_fields["overall.status"])
    }

    function test_restore_defaults_loads_testnet_without_wallet_calls() {
        model.settingsStateLoaded = true
        model.networkProfile = "custom"
        model.nodeUrl = "https://custom.example/"
        model.localNodesEnabled = false
        model.localDevnetEnabled = true
        model.walletProfileLabel = "Wallet sentinel"
        model.walletHome = "/wallet/sentinel"
        fakeHost.reset()
        fakeHost.responses = {
            restoreDefaultSettingsState: {
                ok: true,
                value: {
                    version: 2,
                    network_profile: "default",
                    node_url: "http://127.0.0.1:8080/",
                    network_connector_config: {
                        scopes: {
                            l1: { connector_id: "logoscore_cli_blockchain_module", provenance: "testnet_default" },
                            delivery: { connector_id: "logoscore_cli_delivery_module", provenance: "testnet_default" },
                            storage: { connector_id: "logoscore_cli_storage_module", provenance: "testnet_default" }
                        }
                    },
                    messaging_network_preset: "logos.test",
                    storage_network_preset: "logos.test",
                    local_nodes_enabled: true,
                    local_devnet_enabled: false,
                    blockchain_refresh_rate: 30,
                    messaging_refresh_rate: 30,
                    storage_refresh_rate: 30,
                    social_identities: [],
                    favorites: []
                },
                text: "OK",
                error: ""
            }
        }

        verify(model.restoreDefaultSettings())

        compare(model.networkProfile, "default")
        compare(model.nodeUrl, "http://127.0.0.1:8080/")
        verify(model.localNodesEnabled)
        verify(!model.localDevnetEnabled)
        compare(model.networkConnectorConfig.scopes.l1.connector_id,
            "direct_l1_rpc")
        compare(model.zoneCatalogL1SourceDescriptor().default_topology, "logos_testnet")
        compare(model.walletProfileLabel, "Wallet sentinel")
        compare(model.walletHome, "/wallet/sentinel")
        verify(fakeHost.calls.some(function (call) {
            return call.method === "restoreDefaultSettingsState"
        }))
        verify(!fakeHost.calls.some(function (call) {
            return call.method === "loadWalletState" || call.method === "saveWalletState"
        }))
    }

    function test_social_settings_round_trip_identity_and_shared_idl_policy() {
        fakeHost.responses = {
            loadSettingsState: {
                ok: true,
                value: {
                    social_identities: [
                        {
                            key: "local-1",
                            display_name: "Ada",
                            local_id: "local-1",
                            key_material: "secret",
                            created_at: "2026-07-05T00:00:00.000Z"
                        }
                    ],
                    social_identity_default_mode: "manual",
                    social_selected_identity_key: "local-1",
                    social_conversation_identity_keys: {
                        "/lez/account/a/comments": "local-1"
                    },
                    shared_idl_policy: "sessionOnly",
                    shared_idl_auto_share: true
                },
                text: "OK",
                error: ""
            }
        }

        model.loadSettingsState()

        compare(model.social.socialIdentities.count, 1)
        compare(model.social.socialIdentities.get(0).displayName, "Ada")
        compare(model.social.socialIdentityDefaultMode, "manual")
        compare(model.social.selectedSocialIdentityKey, "local-1")
        compare(model.social.sharedIdlPolicy, "sessionOnly")
        compare(model.social.sharedIdlAutoShare, true)
        const payload = model.settingsStatePayload()
        compare(payload.social_identities.length, 1)
        compare(payload.social_identity_default_mode, "manual")
        compare(payload.shared_idl_policy, "sessionOnly")
        compare(payload.shared_idl_auto_share, true)
    }

    function test_local_node_settings_round_trip_and_drive_runtime_inputs() {
        fakeHost.responses = {
            loadSettingsState: {
                ok: true,
                value: {
                    network_profile: "local",
                    local_nodes_enabled: true,
                    local_devnet_enabled: true,
                    favorites: []
                },
                text: "OK",
                error: ""
            }
        }

        model.loadSettingsState()

        verify(model.localNodesEnabled)
        verify(model.localDevnetEnabled)
        let payload = model.settingsStatePayload()
        compare(payload.local_nodes_enabled, true)
        compare(payload.local_devnet_enabled, true)

        model.localNodesEnabled = false
        model.localDevnetEnabled = false
        model.networkProfile = "local"
        const inputs = model.capabilityRegistryRuntimeInputs()
        compare(inputs.local_nodes_enabled, false)
        compare(inputs.local_devnet_enabled, false)
    }

    function test_legacy_settings_backup_cid_is_ignored_without_resave() {
        fakeHost.responses = {
            loadSettingsState: {
                ok: true,
                value: {
                    settings_backup_cid: "legacy-cid"
                },
                text: "OK",
                error: ""
            }
        }

        model.loadSettingsState()

        compare(model.settingsBackupCid, "")
        compare(model.settingsRestoreCid, "")
        verify(model.settingsStatePayload().settings_backup_cid === undefined)
    }

    function test_wallet_connector_runtime_inputs_follow_build_mode() {
        const standalone = model.capabilityRegistryRuntimeInputs().wallet_connector_config
        compare(standalone.scopes["wallet.l1"].connector_id, "composed_wallet")
        compare(standalone.scopes["wallet.l2"].connector_id, "composed_wallet")

        const basecamp = basecampModel.capabilityRegistryRuntimeInputs().wallet_connector_config
        compare(basecamp.scopes["wallet.l1"].connector_id, "blockchain_module")
        compare(basecamp.scopes["wallet.l2"].connector_id, "lez_core")
    }

    function test_source_routing_does_not_fabricate_shared_idl_capabilities() {
        model.storageSourceMode = "rest"
        model.capabilityRegistryLoaded = true
        model.capabilityRegistryReport = ({
            schema_version: 1,
            capabilities: [
                {
                    key: "delivery",
                    label: "Delivery",
                    status: "available",
                    sub_capabilities: ["delivery.store.query"]
                },
                {
                    key: "storage",
                    label: "Storage",
                    status: "available",
                    sub_capabilities: ["storage.content.read_by_cid"]
                }
            ]
        })

        const localAvailability = model.capabilityLocalAvailability()
        const gate = model.socialGate("shared_idl.read")

        verify(localAvailability["social.identity.local"] !== undefined)
        verify(localAvailability["storage.shared_idl.sync_read"] === undefined)
        verify(localAvailability["storage.shared_idl.sync_upload"] === undefined)
        verify(!gate.enabled)
        compare(gate.missing.length, 1)
        compare(gate.missing[0].dependency, "storage.shared_idl.sync_read")
        compare(gate.missing[0].provenance, "capability_registry")
    }

    function test_zone_scopes_isolate_programs_decode_choices_and_dashboard() {
        const firstZone = setActiveZone("22".repeat(32))
        const owner = "ab".repeat(32)
        model.updateKnownProgramIds([{ hex: owner, base58: "program-a", label: "A" }])
        model.cacheAccountIdlSelection("account-a", { key: "idl-a" }, "State", owner)
        model.dashboardLezBlockRows = [{ block_id: 7 }]
        model.dashboardProvisionalBlocks = [{ block_id: 7 }]
        compare(model.knownProgramIdRows().length, 1)
        verify(model.accountIdlSelection("account-a", owner) !== null)

        const secondZone = "33".repeat(32)
        model.zoneInspection.zoneSummaries = model.zoneInspection.zoneSummaries.concat([{
            channel_id: secondZone,
            kind: "sequencer_zone",
            l1_channel: {},
            l2_zone: {},
            activity_detail: {}
        }])
        model.zoneInspection.activeZoneContext = Object.assign(
            {}, model.zoneInspection.activeZoneContext, {
                channel_id: secondZone,
                source_config_revision: 8,
                context_revision: 2
            })

        verify(firstZone !== model.zoneInspection.activeZoneId)
        compare(model.knownProgramIdRows().length, 0)
        compare(model.accountIdlSelection("account-a", owner), null)
        compare(model.dashboardLezBlockRows.length, 0)
        compare(model.dashboardProvisionalBlocks.length, 0)
    }

    function test_chain_search_uses_typed_candidate_resolver_and_keeps_local_shortcuts() {
        setActiveZone("")
        fakeHost.responses = {
            inspectionResolveTarget: function(args) {
                const request = args[0]
                return {
                    ok: true,
                    value: {
                        report_kind: "inspection.target_resolution",
                        schema_version: 1,
                        query: request.query,
                        request_revision: request.request_revision,
                        context_revision: request.active_zone_context.context_revision,
                        status: "ambiguous",
                        candidates: [{
                            entity_ref: {
                                layer: "l1",
                                network_scope: request.active_zone_context.network_scope,
                                entity_kind: "block",
                                canonical_key: "block:42",
                                block_id: 42,
                                block_hash: null
                            }
                        }, {
                            entity_ref: Object.assign({ layer: "l2" },
                                zoneEntityRef("block", "block:42:" + "a".repeat(64),
                                    "idx-a", "indexer"))
                        }],
                        recovery: null,
                        warnings: []
                    },
                    text: "OK",
                    error: ""
                }
            }
        }

        model.entityNavigation.routeSearch("42")

        tryCompare(model.zoneInspection, "targetResolutionStatus", "ambiguous")
        compare(model.zoneInspection.targetResolutionCandidates.length, 2)
        compare(model.shell.currentView, "zones")
        compare(callCountFor("resolveLezTarget"), 0)
        compare(callCountFor("inspectionResolveTarget"), 1)

        fakeHost.callCount = 0
        fakeHost.calls = []
        model.entityNavigation.routeSearch("settings")
        compare(model.shell.currentView, "settings")
        compare(callCountFor("inspectionResolveTarget"), 0)
    }

    function test_typed_navigation_rejects_wrong_network_references() {
        setActiveZone("")
        const wrongScope = { kind: "genesis_id", genesis_id: "ff".repeat(32) }
        const l2Ref = Object.assign({ layer: "l2" },
            zoneEntityRef("transaction", "aa".repeat(32), "seq-a", "sequencer"), {
                network_scope: wrongScope
            })
        const l1Ref = {
            layer: "l1",
            network_scope: wrongScope,
            entity_kind: "block",
            canonical_key: "block:7",
            block_id: 7,
            block_hash: null
        }

        compare(model.openInspectionEntityRef(l2Ref, false), false)
        compare(model.openInspectionEntityRef(l1Ref, false), false)
        compare(callCountFor("zoneL2Transaction"), 0)
        compare(callCountFor("blockchainBlock"), 0)
    }

    function test_social_comment_topics_for_supported_detail_kinds() {
        fakeHost.responses = {
            socialCommentTopic: function(args) {
                const layer = String(args[0] || "")
                const entity = String(args[1] || "")
                const id = String(args[2] || "")
                if (layer === "lez" || id.indexOf("/") >= 0) {
                    return { ok: true, value: "", text: "", error: "" }
                }
                return { ok: true, value: "/" + layer + "/" + entity + "/" + id + "/comments", text: "OK", error: "" }
            },
            socialZoneCommentTopic: {
                ok: true,
                value: "/lez/account/" + "a".repeat(64) + "/comments",
                text: "OK",
                error: ""
            },
            socialZoneAccountIdlTopic: {
                ok: true,
                value: "/lez/account/" + "a".repeat(64) + "/idl",
                text: "OK",
                error: ""
            }
        }
        setActiveZone("")
        const accountRef = zoneEntityRef("account", "account-2", "idx-a", "indexer")

        compare(model.social.commentTopic("cryptarchia", "transaction", "tx-1"), "/cryptarchia/transaction/tx-1/comments")
        compare(model.social.commentTopic("cryptarchia", "block", "block-1"), "/cryptarchia/block/block-1/comments")
        compare(model.social.commentTopic("cryptarchia", "account", "account-1"), "/cryptarchia/account/account-1/comments")
        compare(model.social.commentTopic("lez", "transaction", "tx-2"), "")
        compare(model.social.zoneCommentTopic(accountRef),
            "/lez/account/" + "a".repeat(64) + "/comments")
        compare(model.social.zoneAccountIdlTopic(accountRef),
            "/lez/account/" + "a".repeat(64) + "/idl")
        compare(model.social.commentTopic("lez", "account", "bad/id"), "")
    }

    function test_social_store_gate_detail_names_missing_delivery_dependency() {
        model.capabilityRegistryReport = ({
            schema_version: 1,
            capabilities: [{
                key: "delivery",
                label: "Delivery",
                status: "unavailable",
                sub_capabilities: ["delivery.store.query"],
                unavailable_sub_capabilities: ["delivery.store.query"]
            }]
        })

        const view = model.social.commentsView("/lez/account/a/comments")
        const gate = view.readGate
        const detail = view.readError

        verify(!gate.enabled)
        compare(gate.missing[0].dependency, "delivery.store.query")
        verify(detail.indexOf("Delivery") >= 0)
        verify(detail.indexOf("delivery.store.query") >= 0)
    }

    function test_social_comment_read_uses_runtime_operation_conversation() {
        const topic = "/cryptarchia/account/account-1/comments"
        fakeHost.responses = {
            socialTopicValid: {
                ok: true,
                value: true,
                text: "OK",
                error: ""
            },
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "social-store-1",
                    domain: "delivery",
                    method: "deliveryStoreQuery",
                    label: "Comments",
                    status: "completed",
                    eventCursor: 1,
                    result: { messages: [] }
                },
                text: "OK",
                error: ""
            },
            socialCommentPageFromStore: {
                ok: true,
                value: {
                    rows: [{ key: "comment-1", body: "hello" }],
                    cursor: "cursor-1"
                },
                text: "OK",
                error: ""
            }
        }

        verify(model.social.loadComments(topic, true, 20, ""))
        tryVerify(function () {
            return model.social.commentsView(topic).state.loading === false
        })

        const starts = fakeHost.calls.filter(function (call) {
            return call.method === "runtimeOperationStart"
        })
        compare(starts.length, 1)
        compare(starts[0].args[0].method, "deliveryStoreQuery")
        compare(starts[0].args[0].payload.content_topics, topic)
        compare(fakeHost.calls.filter(function (call) {
            return call.method === "deliveryStoreQuery"
        }).length, 0)
        compare(model.social.commentsView(topic).rows[0].body, "hello")
    }

    function test_social_write_gate_detail_names_missing_local_identity_dependency() {
        model.social.socialIdentityDefaultMode = "manual"

        const view = model.social.commentsView("/lez/account/a/comments")
        const gate = view.writeGate
        const detail = view.writeError

        verify(!gate.enabled)
        compare(gate.missing[0].dependency, "social.identity.local")
        verify(detail.indexOf("social.identity.local") >= 0)
    }

    function test_shared_idl_policies_store_register_or_ignore_verified_entries() {
        model.idlStateLoaded = true
        setActiveZone("")
        const sharedEntry = {
            key: "shared-1",
            name: "Shared",
            programId: "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
            programIdHex: "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
            json: "{\"name\":\"Shared\",\"accounts\":[]}",
            source: "shared",
            sharedTopic: "/lez/account/account-1/idl",
            sharedIdentity: { display_name: "Ada" },
            sharedAccountId: "account-1",
            accountType: "State"
        }

        model.social.setSharedIdlPolicy("disabled")
        verify(!model.social.applySharedIdlPolicy("account-1", sharedEntry))
        compare(model.social.sharedIdlSuggestions("account-1", sharedEntry.programIdHex).length, 0)

        model.social.setSharedIdlPolicy("suggestion")
        verify(model.social.applySharedIdlPolicy("account-1", sharedEntry))
        compare(model.social.sharedIdlSuggestions("account-1", sharedEntry.programIdHex).length, 1)
        compare(model.registeredIdls.count, 0)

        model.social.socialSharedIdls = ({})
        model.social.setSharedIdlPolicy("sessionOnly")
        verify(model.social.applySharedIdlPolicy("account-1", sharedEntry))
        compare(model.social.sharedIdlEntriesForAccount("account-1", sharedEntry.programIdHex).length, 1)
        compare(model.registeredIdls.count, 0)

        model.social.setSharedIdlPolicy("autoRegister")
        verify(model.social.applySharedIdlPolicy("account-1", sharedEntry))
        compare(model.registeredIdls.count, 1)
        compare(model.registeredIdls.get(0).source, "shared")
        compare(model.idlEntryAt(0).accountType, "State")
    }

    function test_shared_idl_policy_rejects_wrong_account_or_non_shared_entries() {
        setActiveZone("")
        const sharedEntry = {
            key: "shared-1",
            name: "Shared",
            programIdHex: "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
            json: "{\"name\":\"Shared\",\"accounts\":[]}",
            source: "shared",
            sharedAccountId: "account-2",
            accountType: "State"
        }
        const localEntry = {
            key: "local-1",
            name: "Local",
            programIdHex: sharedEntry.programIdHex,
            json: "{\"name\":\"Local\",\"accounts\":[]}",
            source: "local",
            sharedAccountId: "account-1",
            accountType: "State"
        }
        model.social.setSharedIdlPolicy("suggestion")

        verify(!model.social.applySharedIdlPolicy("account-1", sharedEntry))
        verify(!model.social.applySharedIdlPolicy("account-1", localEntry))
        compare(model.social.sharedIdlSuggestions("account-1").length, 0)
        compare(model.registeredIdls.count, 0)
    }

    function test_successful_account_inspection_hydrates_shared_idl_without_displacing_local() {
        setActiveZone("")
        const accountId = "a".repeat(64)
        const programId = "12".repeat(32)
        const topic = "/lez/account/" + accountId + "/idl"
        const localEntry = {
            key: "local-account-idl",
            name: "Local",
            programId: "0x" + programId,
            programIdHex: programId,
            programBinary: "",
            json: "{\"name\":\"Local\",\"accounts\":[]}",
            source: "local",
            sharedTopic: "",
            sharedIdentity: {},
            sharedAccountId: "",
            accountType: ""
        }
        const sharedEntry = {
            key: "shared-account-idl",
            name: "Shared",
            programId: "0x" + programId,
            programIdHex: programId,
            programBinary: "",
            json: "{\"name\":\"Shared\",\"accounts\":[]}",
            source: "shared",
            sharedTopic: topic,
            sharedIdentity: { display_name: "Testnet peer" },
            sharedAccountId: accountId,
            accountType: "State"
        }
        model.registeredIdls.append(localEntry)
        model.social.sharedIdlPolicy = "autoRegister"
        fakeHost.responses = {
            zoneL2Account: function (args) {
                const request = args[0]
                return {
                    ok: true,
                    value: {
                        report_kind: "lez.account",
                        schema_version: 1,
                        context: request.context,
                        request_revision: request.request_revision,
                        route: { policy: "composite", attempts: [] },
                        route_completeness: "all_configured",
                        warnings: [],
                        data: {
                            outcome: "found",
                            value: {
                                account: {
                                    account_id: accountId,
                                    account_id_base58: accountId,
                                    account_id_hex: "34".repeat(32),
                                    balance: "17",
                                    nonce: "4",
                                    owner_program_base58: "owner-program",
                                    owner_program_hex: programId,
                                    data_hex: "0102",
                                    existence: "unknown"
                                },
                                anchor: {
                                    block_id: 12,
                                    block_hash: "56".repeat(32)
                                },
                                after_anchor: null,
                                anchor_state: "exact",
                                source: {
                                    source_id: "idx-a",
                                    source_role: "indexer",
                                    source_config_revision: 7,
                                    finality: "finalized",
                                    retrieval: "live"
                                }
                            }
                        }
                    },
                    text: "OK",
                    error: ""
                }
            },
            zoneL2AccountActivity: {
                ok: false,
                value: null,
                text: "",
                error: "activity unavailable"
            },
            socialZoneAccountIdlTopic: {
                ok: true,
                value: topic,
                text: "OK",
                error: ""
            },
            runtimeOperationStart: function (args) {
                const request = args[0]
                return {
                    ok: true,
                    value: {
                        operationId: "shared-idl-store",
                        domain: "delivery",
                        method: request.method,
                        label: request.label,
                        status: "completed",
                        eventCursor: 1,
                        result: { messages: [] },
                        error: ""
                    },
                    text: "OK",
                    error: ""
                }
            },
            acceptedSharedIdlEntriesFromStoreWithStorage: {
                ok: true,
                value: [sharedEntry],
                text: "OK",
                error: ""
            }
        }

        verify(model.zoneInspection.l2.accounts.inspectL2AccountReference(accountId, {
            kind: "exact",
            source_id: "idx-a",
            source_role: "indexer"
        }))

        tryCompare(model.registeredIdls, "count", 2)
        const topicCalls = fakeHost.calls.filter(function (call) {
            return call.method === "socialZoneAccountIdlTopic"
        })
        compare(topicCalls.length, 1)
        compare(topicCalls[0].args[0].network_scope.kind, "genesis_id")
        compare(topicCalls[0].args[0].channel_id, model.zoneInspection.activeZoneId)
        compare(topicCalls[0].args[0].entity_kind, "account")
        compare(topicCalls[0].args[0].canonical_key, accountId)
        compare(topicCalls[0].args[0].source.kind, "exact")
        compare(topicCalls[0].args[0].source.source_id, "idx-a")
        compare(topicCalls[0].args[0].source.source_role, "indexer")
        const hydrationCalls = fakeHost.calls.filter(function (call) {
            return call.method === "acceptedSharedIdlEntriesFromStoreWithStorage"
        })
        compare(hydrationCalls.length, 1)
        compare(hydrationCalls[0].args[0], topic)
        compare(hydrationCalls[0].args[2], accountId)
        compare(hydrationCalls[0].args[3], "0102")
        compare(hydrationCalls[0].args[4], programId)
        const entries = model.idlEntriesForProgram(programId)
        compare(entries.length, 2)
        compare(entries[0].key, "local-account-idl")
        compare(entries[1].key, "shared-account-idl")
    }

    function test_local_idl_priority_beats_shared_match() {
        setActiveZone("")
        const programIdHex = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
        const localEntry = {
            key: "local-1",
            name: "Local",
            programId: "0x" + programIdHex,
            programIdHex: programIdHex,
            programBinary: "",
            json: "{\"name\":\"Local\",\"accounts\":[]}",
            source: "local",
            sharedTopic: "",
            sharedIdentity: {},
            sharedAccountId: ""
        }
        const sharedEntry = {
            key: "shared-1",
            name: "Shared",
            programId: "0x" + programIdHex,
            programIdHex: programIdHex,
            programBinary: "",
            json: "{\"name\":\"Shared\",\"accounts\":[]}",
            source: "shared",
            sharedTopic: "/lez/account/account-1/idl",
            sharedIdentity: {},
            sharedAccountId: "account-1",
            accountType: "State"
        }
        model.registeredIdls.append(localEntry)
        model.social.setSharedIdlPolicy("sessionOnly")
        model.social.applySharedIdlPolicy("account-1", sharedEntry)
        model.cacheAccountIdlSelection("account-1", sharedEntry, "State", programIdHex)

        const candidates = model.accountDecodeCandidates("account-1", programIdHex)

        compare(candidates.length, 2)
        compare(candidates[0].entry.key, "local-1")
        compare(candidates[1].entry.key, "shared-1")
    }

    function test_registered_shared_idl_keeps_local_priority_and_account_type() {
        const programIdHex = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
        const sharedEntry = {
            key: "shared-1",
            name: "Shared",
            programId: "0x" + programIdHex,
            programIdHex: programIdHex,
            programBinary: "",
            json: "{\"name\":\"Shared\",\"accounts\":[]}",
            source: "shared",
            sharedTopic: "/lez/account/account-1/idl",
            sharedIdentity: {},
            sharedAccountId: "account-1",
            accountType: "State"
        }
        const localEntry = {
            key: "local-1",
            name: "Local",
            programId: "0x" + programIdHex,
            programIdHex: programIdHex,
            programBinary: "",
            json: "{\"name\":\"Local\",\"accounts\":[]}",
            source: "local",
            sharedTopic: "",
            sharedIdentity: {},
            sharedAccountId: "",
            accountType: ""
        }

        model.social.setSharedIdlPolicy("autoRegister")
        verify(model.social.applySharedIdlPolicy("account-1", sharedEntry))
        model.registeredIdls.append(localEntry)

        const entries = model.idlEntriesForProgram(programIdHex)
        compare(entries.length, 2)
        compare(entries[0].key, "local-1")
        compare(entries[1].key, "shared-1")
        compare(entries[1].accountType, "State")

        const candidates = model.accountDecodeCandidates("account-1", programIdHex)
        const payload = model.programDecodeCandidatePayload(candidates)
        compare(candidates[0].entry.key, "local-1")
        compare(candidates[1].entry.key, "shared-1")
        compare(payload[1].accountType, "State")
    }

    function test_program_decode_transaction_runs_when_runtime_capabilities_unavailable() {
        model.capabilityRegistryLoaded = true
        model.capabilityRegistryReport = ({
            schema_version: 1,
            capabilities: [
                {
                    key: "storage",
                    label: "Storage",
                    status: "unavailable",
                    sub_capabilities: ["storage.content.upload"],
                    unavailable_sub_capabilities: ["storage.content.upload"]
                },
                {
                    key: "wallet.l2",
                    label: "L2 Wallet",
                    status: "unavailable",
                    sub_capabilities: ["wallet.l2.instruction.submit"],
                    unavailable_sub_capabilities: ["wallet.l2.instruction.submit"]
                }
            ]
        })
        fakeHost.responses = {
            decodeTransactionSummary: {
                ok: true,
                value: {
                    inspection: {
                        hash: "tx-1",
                        kind: "Public",
                        sections: [],
                        raw_summary: {
                            hash: "tx-1",
                            kind: "Public",
                            program_id_hex: "program",
                            account_ids: [],
                            instruction_data: [0]
                        }
                    },
                    decoded_instruction: {
                        instruction: "set_value",
                        remaining_words: []
                    },
                    decode_enrichment: {
                        status: "applied",
                        provenance: "program_decode_static"
                    }
                },
                text: "OK",
                error: ""
            }
        }

        let callbackResponse = null
        model.decodeTransactionSummaryAsync({
            hash: "tx-1",
            kind: "Public",
            program_id_hex: "program",
            account_ids: [],
            instruction_data: [0]
        }, "{\"instructions\":[]}", function (response) {
            callbackResponse = response
        })

        tryVerify(function () { return callbackResponse !== null })
        verify(callbackResponse.ok)
        compare(callCountFor("decodeTransactionSummary"), 1)
        compare(fakeHost.calls[fakeHost.calls.length - 1].method, "decodeTransactionSummary")
    }

    function test_publish_account_idl_uploads_artifact_before_delivery_send() {
        setActiveZone("")
        model.storageMutatingDiagnosticsEnabled = true
        model.messagingMutatingDiagnosticsEnabled = true
        fakeHost.responses = {
            socialZoneAccountIdlTopic: {
                ok: true,
                value: "/lez/account/" + "a".repeat(64) + "/idl",
                text: "OK",
                error: ""
            },
            socialTopicValid: {
                ok: true,
                value: true,
                text: "OK",
                error: ""
            },
            runtimeOperationStart: function (args) {
                const request = args[0] || {}
                if (request.method === "storageUploadPayload") {
                    return {
                        ok: true,
                        value: socialUploadOperation("idl-upload", {
                            cid: "cid-idl",
                            filename: "logos-inspector-shared-idl.json",
                            endpoint: model.sourceRouting.configuredStorageRestUrl()
                        }),
                        text: "OK",
                        error: ""
                    }
                }
                return {
                    ok: true,
                    value: socialSendOperation("idl-send", request.payload.topic, {
                        messageHash: "hash-1"
                    }),
                    text: "OK",
                    error: ""
                }
            }
        }

        const accountRef = zoneEntityRef("account", "account-1", "idx-a", "indexer")
        verify(model.social.publishAccountIdl(accountRef, "program-1", {
            name: "Shared",
            json: "{\"name\":\"Shared\",\"accounts\":[]}"
        }))

        tryVerify(function () {
            return runtimeOperationCallIndex("deliverySend")
                > runtimeOperationCallIndex("storageUploadPayload")
        })
        const uploadIndex = runtimeOperationCallIndex("storageUploadPayload")
        const sendIndex = runtimeOperationCallIndex("deliverySend")
        verify(uploadIndex >= 0)
        verify(sendIndex > uploadIndex)
        const uploadRequest = fakeHost.calls[uploadIndex].args[0]
        compare(uploadRequest.payload.filename, "logos-inspector-shared-idl.json")
        const deliveryRequest = fakeHost.calls[sendIndex].args[0]
        const deliveryPayload = JSON.parse(deliveryRequest.payload.payload)
        compare(deliveryPayload.idl_cid, "cid-idl")
        compare(deliveryPayload.version, 2)
        compare(deliveryPayload.scope.zone_id, model.zoneInspection.activeZoneId)
        verify(deliveryPayload.idl_json === undefined)
        compare(callCountFor("storageUploadPayload"), 0)
        compare(callCountFor("deliverySend"), 0)
    }

    function test_publish_account_idl_uploads_through_logoscore_cli() {
        setActiveZone("")
        model.setNetworkConnectorMode("storage", "logoscore_cli")
        model.storageMutatingDiagnosticsEnabled = true
        model.messagingMutatingDiagnosticsEnabled = true
        fakeHost.responses = {
            socialZoneAccountIdlTopic: {
                ok: true,
                value: "/lez/account/" + "a".repeat(64) + "/idl",
                text: "OK",
                error: ""
            },
            socialTopicValid: {
                ok: true,
                value: true,
                text: "OK",
                error: ""
            },
            runtimeOperationStart: function (args) {
                const request = args[0] || {}
                return request.method === "storageUploadPayload" ? {
                    ok: true,
                    value: socialUploadOperation("idl-upload-cli", {
                        cid: "cid-idl",
                        filename: "logos-inspector-shared-idl.json",
                        endpoint: "logoscore call storage_module"
                    }),
                    text: "OK",
                    error: ""
                } : {
                    ok: true,
                    value: socialSendOperation("idl-send-cli", request.payload.topic, {
                        messageHash: "hash-1"
                    }),
                    text: "OK",
                    error: ""
                }
            }
        }

        verify(model.social.publishAccountIdl(
            zoneEntityRef("account", "account-1", "idx-a", "indexer"), "program-1", {
                name: "Shared",
                json: "{\"name\":\"Shared\",\"accounts\":[]}"
            }))

        tryVerify(function () {
            return runtimeOperationCallIndex("deliverySend")
                > runtimeOperationCallIndex("storageUploadPayload")
        })
        const uploadIndex = runtimeOperationCallIndex("storageUploadPayload")
        verify(uploadIndex >= 0)
        compare(fakeHost.calls[uploadIndex].args[0].adapter.source_mode, "logoscore_cli")
        verify(runtimeOperationCallIndex("deliverySend") > uploadIndex)
        compare(callCountFor("storageUploadPayload"), 0)
        compare(callCountFor("deliverySend"), 0)
    }

    function test_verified_local_idl_selection_honors_auto_share_setting() {
        setActiveZone("")
        model.storageMutatingDiagnosticsEnabled = true
        model.messagingMutatingDiagnosticsEnabled = true
        model.social.createIdentity("Auto publisher")
        model.social.sharedIdlAutoShare = true
        fakeHost.responses = {
            socialZoneAccountIdlTopic: {
                ok: true,
                value: "/lez/account/" + "a".repeat(64) + "/idl",
                text: "OK",
                error: ""
            },
            socialTopicValid: {
                ok: true,
                value: true,
                text: "OK",
                error: ""
            },
            runtimeOperationStart: function (args) {
                const request = args[0] || {}
                return request.method === "storageUploadPayload" ? {
                    ok: true,
                    value: socialUploadOperation("auto-idl-upload", {
                        cid: "cid-auto-idl",
                        filename: "logos-inspector-shared-idl.json",
                        endpoint: model.sourceRouting.configuredStorageRestUrl()
                    }),
                    text: "OK",
                    error: ""
                } : {
                    ok: true,
                    value: socialSendOperation("auto-idl-send", request.payload.topic, {
                        messageHash: "hash-auto-idl"
                    }),
                    text: "OK",
                    error: ""
                }
            }
        }
        const programId = "12".repeat(32)
        const entry = {
            key: "auto-local-idl",
            name: "Auto local",
            programIdHex: programId,
            json: "{\"name\":\"Auto local\",\"accounts\":[]}",
            source: "local"
        }

        model.cacheAccountIdlSelection("a".repeat(64), entry, "State", programId)

        tryVerify(function () {
            return runtimeOperationCallIndex("deliverySend")
                > runtimeOperationCallIndex("storageUploadPayload")
        })
    }

    function test_settings_backup_blocks_when_upload_gate_unavailable() {
        model.capabilityRegistryLoaded = true
        model.capabilityRegistryReport = ({
            schema_version: 1,
            capabilities: [{
                key: "storage",
                label: "Storage",
                status: "unavailable",
                sub_capabilities: ["storage.content.upload"],
                unavailable_sub_capabilities: ["storage.content.upload"]
            }]
        })
        model.storageSourceMode = "rest"
        model.storageMutatingDiagnosticsEnabled = true
        fakeHost.responses = {
            createLocalSettingsBackup: {
                ok: true,
                value: { backup_catalog_id: "backup-blocked" },
                text: "OK",
                error: ""
            },
            runtimeOperationStart: {
                ok: true,
                value: backupUploadOperation("blocked", "completed", { cid: "cid-blocked" })
            }
        }

        verify(!model.backupSettingsToStorage(true))
        verify(!fakeHost.calls.some(function (call) { return call.method === "createLocalSettingsBackup" }))
        verify(!fakeHost.calls.some(function (call) { return call.method === "runtimeOperationStart" }))
        verify(model.settingsBackupStatus.indexOf("upload capability") >= 0)
    }

    function test_upload_backup_catalog_entry_blocks_when_upload_gate_unavailable() {
        model.capabilityRegistryLoaded = true
        model.capabilityRegistryReport = ({
            schema_version: 1,
            capabilities: [{
                key: "storage",
                label: "Storage",
                status: "unavailable",
                sub_capabilities: ["storage.content.upload"],
                unavailable_sub_capabilities: ["storage.content.upload"]
            }]
        })
        model.storageSourceMode = "rest"
        model.storageMutatingDiagnosticsEnabled = true
        fakeHost.responses = {
            runtimeOperationStart: {
                ok: true,
                value: backupUploadOperation("blocked", "completed", { cid: "cid-blocked" })
            }
        }

        verify(!model.backupImport.uploadBackupCatalogEntry("backup-blocked"))
        verify(!fakeHost.calls.some(function (call) { return call.method === "runtimeOperationStart" }))
        verify(model.settingsBackupStatus.indexOf("upload capability") >= 0)
    }

    function test_settings_backup_requires_backup_sync_upload_subcap() {
        model.capabilityRegistryLoaded = true
        model.capabilityRegistryReport = ({
            schema_version: 1,
            capabilities: [{
                key: "storage",
                label: "Storage",
                status: "available",
                sub_capabilities: ["storage.content.upload"]
            }]
        })
        model.storageSourceMode = "rest"
        fakeHost.responses = {
            createLocalSettingsBackup: {
                ok: true,
                value: { backup_catalog_id: "backup-blocked" },
                text: "OK",
                error: ""
            }
        }

        verify(!model.settingsBackupAvailable())
        verify(!model.backupSettingsToStorage(false))
        verify(!fakeHost.calls.some(function (call) { return call.method === "createLocalSettingsBackup" }))
    }

    function test_settings_backup_available_without_rest_source_predicate_when_gate_enabled() {
        model.settingsStateLoaded = true
        model.idlStateLoaded = true
        model.walletStateLoaded = true
        model.setNetworkConnectorMode("storage", "metrics")
        model.settingsBackupContents = ({
            settings: true,
            favorites: false,
            idl_registry: false,
            wallet_profile: false
        })
        fakeHost.responses = {
            createLocalSettingsBackup: {
                ok: true,
                value: { backup_catalog_id: "backup-module" },
                text: "OK",
                error: ""
            },
            runtimeOperationStart: {
                ok: true,
                value: backupUploadOperation(
                    "backup-module-op",
                    "completed",
                    backupUploadResult("backup-module", "cid-module", "sha256:module"))
            }
        }

        verify(model.settingsBackupAvailable())
        verify(model.backupSettingsToStorage(false))
        tryVerify(function () { return callCountFor("runtimeOperationStart") === 1 })
        const calls = fakeHost.calls.filter(function (call) { return call.method === "runtimeOperationStart" })
        compare(calls.length, 1)
        compare(calls[0].args[0].adapter.source_mode, "metrics")
        verify(calls[0].args[0].adapter.inputs.rest_endpoint === undefined)
    }

    function test_settings_restore_from_storage_blocks_when_read_gate_unavailable() {
        model.capabilityRegistryLoaded = true
        model.capabilityRegistryReport = ({
            schema_version: 1,
            capabilities: [{
                key: "storage",
                label: "Storage",
                status: "unavailable",
                sub_capabilities: ["storage.content.read_by_cid"],
                unavailable_sub_capabilities: ["storage.content.read_by_cid"]
            }]
        })
        model.storageSourceMode = "rest"
        fakeHost.responses = {
            storageRestoreSettings: {
                ok: true,
                value: { downloaded: true },
                text: "OK",
                error: ""
            }
        }

        verify(!model.restoreSettingsFromStorage("cid-blocked", true))
        verify(!fakeHost.calls.some(function (call) { return call.method === "storageRestoreSettings" }))
        verify(model.settingsBackupStatus.indexOf("read-by-CID capability") >= 0)
    }

    function test_settings_restore_requires_backup_sync_read_subcap() {
        model.capabilityRegistryLoaded = true
        model.capabilityRegistryReport = ({
            schema_version: 1,
            capabilities: [{
                key: "storage",
                label: "Storage",
                status: "available",
                sub_capabilities: ["storage.content.read_by_cid"]
            }]
        })
        model.storageSourceMode = "rest"

        verify(!model.settingsBackupDownloadAvailable())
        verify(!model.restoreSettingsFromStorage("cid-blocked", true))
        verify(!fakeHost.calls.some(function (call) { return call.method === "storageRestoreSettings" }))
    }

    function test_settings_backup_to_storage_uses_wallet_profile_and_catalog_remote_metadata() {
        model.settingsStateLoaded = true
        model.idlStateLoaded = true
        model.walletStateLoaded = true
        model.storageMutatingDiagnosticsEnabled = true
        configureReadyWallet()
        model.settingsBackupEncrypted = true
        fakeHost.responses = {
            createLocalSettingsBackup: {
                ok: true,
                value: {
                    backup_catalog_id: "backup-1",
                    payload_id: "sha256:abc",
                    backup_version_label: "Encrypted settings backup"
                },
                text: "OK",
                error: ""
            },
            runtimeOperationStart: {
                ok: true,
                value: backupUploadOperation(
                    "backup-upload-1",
                    "completed",
                    backupUploadResult("backup-1", "cid-backup", "sha256:abc"))
            }
        }

        verify(model.backupSettingsToStorage(true))
        tryVerify(function () { return model.settingsBackupCid === "cid-backup" })

        const backupCalls = fakeHost.calls.filter(function (call) {
            return call.method === "runtimeOperationStart"
        })
        compare(backupCalls.length, 1)
        const backupRequest = backupCalls[0].args[0]
        compare(backupRequest.adapter.source_mode, "rest")
        compare(backupRequest.adapter.inputs.rest_endpoint, model.sourceRouting.configuredStorageRestUrl())
        compare(backupRequest.mutating_enabled, true)
        compare(backupRequest.payload.backup_catalog_id, "backup-1")
        const localCalls = fakeHost.calls.filter(function (call) {
            return call.method === "createLocalSettingsBackup"
        })
        compare(localCalls.length, 1)
        compare(localCalls[0].args[1], true)
        compare(localCalls[0].args[2].wallet_home, "/tmp/wallet-home")
        verify(localCalls[0].args[3].settings)
        verify(localCalls[0].args[3].favorites)
        verify(localCalls[0].args[3].idl_registry)
        verify(localCalls[0].args[3].wallet_profile)
        compare(model.settingsBackupCid, "cid-backup")
        compare(model.settingsRestoreCid, "cid-backup")
        verify(model.settingsStatePayload().settings_backup_cid === undefined)

        const saveCalls = fakeHost.calls.filter(function (call) {
            return call.method === "saveSettingsState"
        })
        for (let i = 0; i < saveCalls.length; ++i) {
            verify(saveCalls[i].args[0].settings_backup_cid === undefined)
        }
        let uploadCallIndex = -1
        for (let i = 0; i < fakeHost.calls.length; ++i) {
            if (fakeHost.calls[i].method === "runtimeOperationStart") {
                uploadCallIndex = i
                break
            }
        }
        verify(uploadCallIndex >= 0)
        for (let i = uploadCallIndex + 1; i < fakeHost.calls.length; ++i) {
            verify(fakeHost.calls[i].method !== "saveSettingsState")
        }
    }

    function test_create_local_settings_backup_passes_partial_contents() {
        fakeHost.responses = {
            createLocalSettingsBackup: {
                ok: true,
                value: {
                    backup_catalog_id: "backup-partial",
                    payload_id: "sha256:partial",
                    backup_version_label: "Partial"
                },
                text: "OK",
                error: ""
            }
        }

        const contents = {
            settings: false,
            favorites: true,
            idl_registry: false,
            wallet_profile: false
        }
        const entry = model.createLocalSettingsBackup("Partial", false, contents)

        verify(entry !== null)
        const calls = fakeHost.calls.filter(function (item) { return item.method === "createLocalSettingsBackup" })
        compare(calls.length, 1)
        const call = calls[0]
        compare(call.args[0], "Partial")
        compare(call.args[3].settings, false)
        compare(call.args[3].favorites, true)
        compare(call.args[3].idl_registry, false)
        compare(call.args[3].wallet_profile, false)
        compare(entry.backup_version_label, "Partial")
    }

    function test_create_local_settings_backup_passes_empty_label_for_backend_default() {
        fakeHost.responses = {
            createLocalSettingsBackup: {
                ok: true,
                value: {
                    backup_catalog_id: "backup-default",
                    payload_id: "sha256:default",
                    backup_version_label: "1720000000",
                    created_at: "1720000000"
                },
                text: "OK",
                error: ""
            }
        }

        const entry = model.createLocalSettingsBackup("", false, model.defaultSettingsBackupContents())

        verify(entry !== null)
        const calls = fakeHost.calls.filter(function (item) { return item.method === "createLocalSettingsBackup" })
        compare(calls.length, 1)
        compare(calls[0].args[0], "")
        compare(entry.backup_version_label, "1720000000")
    }

    function test_settings_download_to_catalog_downloads_catalog_only() {
        model.settingsStateLoaded = true
        model.idlStateLoaded = true
        model.walletStateLoaded = true
        model.storageMutatingDiagnosticsEnabled = false
        configureReadyWallet()
        fakeHost.responses = {
            runtimeOperationStart: {
                ok: true,
                value: backupDownloadOperation(
                    "backup-download-1", "completed", "cid-restore", {
                        downloaded: true,
                        restored: false,
                        encrypted: true,
                        cid: "cid-restore",
                        backup_catalog_id: "backup-restore",
                        payload_id: "sha256:remote",
                        catalog_entry: {
                            backup_catalog_id: "backup-restore",
                            payload_id: "sha256:remote",
                            encrypted: true,
                            remote: {
                                cid: "cid-restore",
                                provider: "logos_storage"
                            }
                        },
                        bytes: 128,
                        endpoint: model.sourceRouting.configuredStorageRestUrl(),
                        source: "network"
                    }),
                text: "OK",
                error: ""
            }
        }

        const previousBackupCid = model.settingsBackupCid
        fakeHost.calls = []
        verify(model.downloadSettingsBackupToCatalog("cid-restore"))
        tryVerify(function () {
            return fakeHost.calls.some(function (call) {
                return call.method === "runtimeOperationStart"
            })
        })
        tryVerify(function () { return model.backupCatalogRows().length === 1 })

        const downloadCalls = fakeHost.calls.filter(function (call) {
            return call.method === "runtimeOperationStart"
        })
        compare(downloadCalls.length, 1)
        compare(downloadCalls[0].args[0].domain, "storage")
        compare(downloadCalls[0].args[0].method, "storageDownloadBackupCatalogEntry")
        compare(downloadCalls[0].args[0].adapter.source_mode, "rest")
        compare(downloadCalls[0].args[0].adapter.inputs.rest_endpoint, model.sourceRouting.configuredStorageRestUrl())
        compare(downloadCalls[0].args[0].mutating_enabled, false)
        compare(downloadCalls[0].args[0].payload.cid, "cid-restore")
        compare(downloadCalls[0].args[0].payload.local_only, false)
        verify(!fakeHost.calls.some(function (call) { return call.method === "storageRestoreSettings" }))
        verify(!fakeHost.calls.some(function (call) { return call.method === "loadBackupCatalog" }))
        verify(!fakeHost.calls.some(function (call) { return call.method === "loadSettingsState" }))
        verify(!fakeHost.calls.some(function (call) { return call.method === "loadIdlState" }))
        verify(!fakeHost.calls.some(function (call) { return call.method === "loadWalletState" }))
        verify(!fakeHost.calls.some(function (call) { return call.method === "saveSettingsState" }))
        verify(!fakeHost.calls.some(function (call) { return call.method === "saveIdlState" }))
        verify(!fakeHost.calls.some(function (call) { return call.method === "saveWalletState" }))
        verify(!fakeHost.calls.some(function (call) { return call.method === "settingsBackupImportPreview" }))
        verify(!fakeHost.calls.some(function (call) { return call.method === "settingsBackupImportApply" }))
        compare(model.settingsBackupCid, previousBackupCid)
        compare(model.backupCatalogRows().length, 1)
        compare(model.backupCatalogRows()[0].backup_catalog_id, "backup-restore")
        verify(model.settingsBackupStatus.indexOf("Downloaded") >= 0)
    }

    function test_settings_download_to_catalog_reports_async_start_then_terminal_catalog_record() {
        fakeHost.responses = {
            runtimeOperationStart: {
                ok: true,
                value: backupDownloadOperation(
                    "backup-download-running", "running", "cid-async", null, 1)
            },
            runtimeOperationStatus: {
                ok: true,
                value: backupDownloadOperation(
                    "backup-download-running", "completed", "cid-async", {
                        downloaded: true,
                        restored: false,
                        encrypted: false,
                        cid: "cid-async",
                        backup_catalog_id: "backup-async",
                        payload_id: "sha256:async",
                        catalog_entry: {
                            backup_catalog_id: "backup-async",
                            payload_id: "sha256:async",
                            encrypted: false,
                            remote: {
                                cid: "cid-async",
                                provider: "logos_storage"
                            }
                        },
                        bytes: 64,
                        endpoint: model.sourceRouting.configuredStorageRestUrl(),
                        source: "network"
                    }, 2)
            }
        }

        verify(model.downloadSettingsBackupToCatalog("cid-async"))
        verify(model.backupCatalogDownloadRunning)
        verify(model.backupCatalogTransferRunning)
        verify(model.settingsBackupStatus.indexOf("started") >= 0)
        tryVerify(function () {
            return model.backupCatalog.downloadSession.view.running
                && !model.backupCatalog.downloadSession.view.startPending
        })

        model.backupCatalog.pollDownload()

        tryVerify(function () { return !model.backupCatalogDownloadRunning })
        verify(!model.backupCatalogTransferRunning)
        tryVerify(function () { return model.settingsBackupStatus.indexOf("Downloaded") >= 0 })
        tryVerify(function () { return model.backupCatalogRows().length === 1 })
        compare(model.backupCatalogRows()[0].backup_catalog_id, "backup-async")
        verify(!fakeHost.calls.some(function (call) {
            return call.method === "storageRestoreSettings"
                || call.method === "settingsBackupImportApply"
                || call.method === "saveSettingsState"
                || call.method === "saveIdlState"
                || call.method === "saveWalletState"
        }))
    }

    function test_backup_import_preview_uses_backend_transaction_plan() {
        fakeHost.responses = {
            settingsBackupImportPreview: {
                ok: true,
                value: {
                    import_plan: true,
                    blocked: false,
                    selectedAreas: ["settings"],
                    settings: true,
                    operation_decisions: []
                },
                text: "OK",
                error: ""
            }
        }

        const plan = model.backupImport.previewLocalSettingsImportPlan("backup-1", {
            settings: "replace",
            favorites: "skip",
            idl_registry: "skip",
            wallet_profile: "skip"
        })

        verify(plan !== null)
        compare(fakeHost.lastMethod, "settingsBackupImportPreview")
        compare(fakeHost.lastArgs[0], "backup-1")
        compare(fakeHost.lastArgs[2].settings, "replace")
        verify(plan.import_plan)
    }

    function test_backup_import_apply_projects_backend_transaction_result() {
        model.bridge = asyncImportBridgeClient
        asyncImportHost.deferAsyncRequests = true
        asyncImportHost.responses = {
            settingsBackupImportApply: {
                ok: true,
                value: {
                    terminal: true,
                    phase: "Applied",
                    outcome: "applied",
                    applied: true,
                    blocked: false,
                    encrypted: false,
                    favorites: 1,
                    idl_count: 0,
                    selectedAreas: ["settings"],
                    appliedAreas: ["settings"],
                    importId: "backup_import:backup-1",
                    backupCatalogId: "backup-1",
                    operationEvents: [{
                        domain: "backup",
                        method: "settingsBackupImportPolicy",
                        status: "stopped_for_import",
                        label: "Backup import policy",
                        operationId: "op-read",
                        operationClass: "read_poll",
                        affectedInputs: [],
                        restartPolicy: "safe_read_polling",
                        importId: "backup_import:backup-1",
                        backupCatalogId: "backup-1",
                        reason: "affected_operation_stopped_for_import",
                        detail: "Stopped affected operation before backup import."
                    }, {
                        domain: "backup",
                        method: "settingsBackupImportApply",
                        status: "applied_for_import",
                        label: "Settings backup import",
                        operationId: "backup_import:backup-1",
                        operationClass: "backup",
                        restartPolicy: "manual_required",
                        importId: "backup_import:backup-1",
                        backupCatalogId: "backup-1",
                        phase: "Applied",
                        outcome: "applied",
                        reason: "backup_import_applied",
                        terminal: true,
                        detail: ""
                    }]
                },
                text: "OK",
                error: ""
            },
            loadSettingsState: {
                ok: true,
                value: { favorites: [] },
                text: "OK",
                error: ""
            },
            capabilityRegistryReport: {
                ok: true,
                value: appModelTestCapabilityRegistry(),
                text: "OK",
                error: ""
            }
        }

        const admitted = model.backupImport.restoreLocalSettingsBackup("backup-1", {
            settings: "replace",
            favorites: "skip",
            idl_registry: "skip",
            wallet_profile: "skip"
        })

        verify(admitted)
        compare(callIndexForHost(asyncImportHost, "settingsBackupImportApply"), 0)
        verify(model.backupCatalogImportRunning)
        compare(asyncImportHost.pendingAsyncRequests.length, 1)
        importHeartbeat.start()
        tryVerify(function () { return importHeartbeat.ticks > 0 }, 100)
        importHeartbeat.stop()
        verify(model.backupCatalogImportRunning)
        compare(callIndexForHost(asyncImportHost, "loadSettingsState"), -1)

        verify(asyncImportHost.completeAsyncAt(0))
        tryVerify(function () {
            return callIndexForHost(asyncImportHost, "loadSettingsState") > 0
        })
        verify(!model.backupCatalogImportRunning)
        verify(!asyncImportHost.calls.some(function (call) {
            return call.method === "runtimeOperationCancel"
                || call.method === "previewLocalSettingsRestore"
                || call.method === "restoreLocalSettingsBackup"
        }))
        const backupRows = model.runtimeOperationHistoryRows("backup")
        verify(backupRows.some(function (row) {
            return row.status === "stopped_for_import"
                && row.importId === "backup_import:backup-1"
        }))
        const appliedRows = backupRows.filter(function (row) {
            return row.status === "applied_for_import"
        })
        compare(appliedRows.length, 1)
        compare(appliedRows[0].reason, "backup_import_applied")
    }

    function test_backup_import_apply_reports_backend_block_decision() {
        model.bridge = asyncImportBridgeClient
        asyncImportHost.responses = {
            settingsBackupImportApply: {
                ok: true,
                value: {
                    terminal: true,
                    phase: "RolledBack",
                    outcome: "blocked",
                    applied: false,
                    blocked: true,
                    blockedOperationLabel: "Submit transaction",
                    selectedAreas: ["wallet_profile"],
                    appliedAreas: [],
                    importId: "backup_import:backup-wallet",
                    backupCatalogId: "backup-wallet",
                    operationEvents: [{
                        domain: "backup",
                        method: "settingsBackupImportPolicy",
                        status: "blocked_for_import",
                        label: "Backup import policy",
                        operationId: "op-wallet",
                        operationClass: "signing_submission",
                        affectedInputs: [],
                        restartPolicy: "manual_required",
                        importId: "backup_import:backup-wallet",
                        backupCatalogId: "backup-wallet",
                        reason: "affected_operation_blocked_for_import",
                        detail: "Blocked backup import while affected operation is running."
                    }, {
                        domain: "backup",
                        method: "settingsBackupImportApply",
                        status: "blocked_for_import",
                        label: "Settings backup import",
                        operationId: "backup_import:backup-wallet",
                        operationClass: "backup",
                        restartPolicy: "manual_required",
                        importId: "backup_import:backup-wallet",
                        backupCatalogId: "backup-wallet",
                        phase: "RolledBack",
                        outcome: "blocked",
                        reason: "backup_import_blocked",
                        terminal: true,
                        detail: ""
                    }]
                },
                text: "OK",
                error: ""
            }
        }

        const admitted = model.backupImport.restoreLocalSettingsBackup("backup-wallet", {
            settings: "skip",
            favorites: "skip",
            idl_registry: "skip",
            wallet_profile: "replace"
        })

        verify(admitted)
        tryVerify(function () {
            return model.settingsBackupStatus.indexOf("Submit transaction") >= 0
        })
        verify(!asyncImportHost.calls.some(function (call) {
            return call.method === "loadWalletState"
                || call.method === "runtimeOperationCancel"
        }))
        const backupRows = model.runtimeOperationHistoryRows("backup")
        verify(backupRows.some(function (row) {
            return row.status === "blocked_for_import"
                && row.reason === "affected_operation_blocked_for_import"
        }))
    }

    function test_navigation_history_tracks_page_selection() {
        verify(!model.canNavigateBack())
        verify(!model.canNavigateForward())

        model.selectView("blocks")

        compare(model.shell.currentView, "blocks")
        verify(model.canNavigateBack())
        compare(model.navigationBackLabel(), "Dashboard")
        verify(!model.canNavigateForward())

        model.selectView("transactions")

        compare(model.shell.currentView, "transactions")
        compare(model.navigationBackStack.length, 2)

        model.navigateBack()

        compare(model.shell.currentView, "blocks")
        verify(model.canNavigateBack())
        verify(model.canNavigateForward())
        compare(model.navigationForwardLabel(), "Mantle Tx")

        model.selectView("programs")

        compare(model.shell.currentView, "programs")
        verify(!model.canNavigateForward())
    }

    function test_navigation_history_restores_detail_state() {
        model.shell.currentView = "blockDetail"
        model.blockDetailValue = { type: "blockchain_block", hash: "old-block", slot: 1 }
        model.shell.resultTitle = "Block"
        model.shell.resultText = "old result"
        model.shell.resultValue = { hash: "old-block" }
        model.shell.resultOwner = "blockDetail"

        model.pushNavigationHistory()

        model.blockDetailValue = { type: "blockchain_block", hash: "new-block", slot: 2 }
        model.shell.resultText = "new result"
        model.shell.resultValue = { hash: "new-block" }

        compare(model.navigationBackLabel(), "Block old-block")

        model.navigateBack()

        compare(model.shell.currentView, "blockDetail")
        verify(model.blockDetailValue !== null)
        compare(model.blockDetailValue.hash, "old-block")
        compare(model.shell.resultText, "old result")
        compare(model.shell.resultOwner, "blockDetail")
        verify(model.canNavigateForward())

        model.navigateForward()

        compare(model.shell.currentView, "blockDetail")
        verify(model.blockDetailValue !== null)
        compare(model.blockDetailValue.hash, "new-block")
        compare(model.shell.resultText, "new result")
    }

    function test_navigation_history_records_deep_block_opener() {
        model.shell.currentView = "blockDetail"
        model.blockDetailValue = { type: "blockchain_block", hash: "old-block", slot: 1 }
        model.shell.resultTitle = "Block"
        model.shell.resultText = "old result"
        model.shell.resultValue = { hash: "old-block" }
        model.shell.resultOwner = "blockDetail"
        model.blocksPageRows = [
            { header: { slot: 7, id: "new-block" }, transactions: [] }
        ]

        model.entityNavigation.openBlockchainBlock("7")

        compare(model.shell.currentView, "blockDetail")
        verify(model.blockDetailValue !== null)
        compare(model.blockDetailValue.hash, "new-block")
        compare(model.navigationBackStack.length, 1)

        model.navigateBack()

        compare(model.shell.currentView, "blockDetail")
        verify(model.blockDetailValue !== null)
        compare(model.blockDetailValue.hash, "old-block")
        compare(model.shell.resultText, "old result")

        model.navigateForward()

        compare(model.shell.currentView, "blockDetail")
        verify(model.blockDetailValue !== null)
        compare(model.blockDetailValue.hash, "new-block")
    }

    function test_dashboard_metric_history_prefix_clear() {
        model.metrics.dashboardMetricHistory = {
            "messaging.messages": [{ timestamp: 1, value: 1 }],
            "storage.files": [{ timestamp: 1, value: 2 }],
            "chain.height": [{ timestamp: 1, value: 3 }]
        }
        model.metrics.dashboardMetricLastSeen = {
            "messaging.messages": { timestamp: 2, value: 1 },
            "storage.files": { timestamp: 2, value: 2 }
        }

        model.metrics.clearDashboardMetricHistoryForPrefix("messaging.")

        compare(model.metrics.dashboardMetricHistory["messaging.messages"], undefined)
        compare(model.metrics.dashboardMetricLastSeen["messaging.messages"], undefined)
        verify(model.metrics.dashboardMetricHistory["storage.files"] !== undefined)
        verify(model.metrics.dashboardMetricLastSeen["storage.files"] !== undefined)
        verify(model.metrics.dashboardMetricHistory["chain.height"] !== undefined)
        compare(model.metrics.dashboardMetricHistoryRevision, 1)
    }

    function test_dashboard_metric_history_keeps_pre_change_sample() {
        const values = [100, 100, 100, 100, 100, 101, 101, 101, 101, 102, 101, 101, 101, 102]
        for (let i = 0; i < values.length; ++i) {
            setTipMinusLib(values[i])
            model.metrics.recordDashboardSnapshot()
        }

        const samples = model.metrics.dashboardMetricHistory["bedrock.tip_minus_lib"]
        const storedValues = samples.map(function (sample) {
            return sample.value
        })

        compare(storedValues.length, 8)
        compare(JSON.stringify(storedValues), JSON.stringify([100, 100, 101, 101, 102, 101, 101, 102]))
        for (let j = 1; j < samples.length; ++j) {
            verify(samples[j].timestamp > samples[j - 1].timestamp)
        }
    }

    function test_dashboard_metric_history_keeps_300_samples() {
        for (let i = 0; i < 310; ++i) {
            setTipMinusLib(i)
            model.metrics.recordDashboardSnapshot()
        }

        const samples = model.metrics.dashboardMetricHistory["bedrock.tip_minus_lib"]

        compare(samples.length, 300)
        compare(samples[0].value, 10)
        compare(samples[299].value, 309)
    }

    function test_idl_registration_delegates() {
        const programId = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
        const idlJson = JSON.stringify({
            name: "Sample",
            instructions: [],
            accounts: []
        })

        model.idlStateLoaded = true
        model.registerIdl("", programId, idlJson)

        compare(model.registeredIdls.count, 1)
        compare(model.registeredIdls.get(0).name, "Sample")
        compare(model.registeredIdls.get(0).programIdHex, programId.slice(2))
        compare(fakeHost.lastMethod, "saveIdlState")
    }

    function test_deploy_program_binary_uses_wallet_confirmation_and_logs_execution_operation() {
        configureReadyWallet()
        fakeHost.responses = {
            localWalletDeployProgram: {
                ok: true,
                value: {
                    source: "local_wallet_cli",
                    status: "submitted",
                    program_id_hex: "abc123",
                    deployment_tx_hash: "tx123"
                },
                text: "OK",
                error: ""
            }
        }

        model.deployProgramBinary("/tmp/program.bin")

        tryVerify(function () {
            return fakeHost.calls.some(function (call) {
                return call.method === "localWalletDeployProgram"
            })
        })
        const deployCalls = fakeHost.calls.filter(function (call) {
            return call.method === "localWalletDeployProgram"
        })
        compare(deployCalls.length, 1)
        compare(deployCalls[0].args[0].wallet_binary, "/usr/bin/lee-wallet")
        compare(deployCalls[0].args[0].wallet_home, "/tmp/wallet-home")
        compare(deployCalls[0].args[1], "/tmp/program.bin")
        compare(deployCalls[0].args[2], "confirm-deploy-program")
        compare(model.localWalletOperations.length, 0)
        const history = model.runtimeOperationHistoryRows("execution")
        compare(history.length, 1)
        compare(history[0].label, "Program deploy")
        compare(history[0].status, "completed")
    }

    function test_preview_idl_instruction_uses_execution_adapter() {
        fakeHost.responses = {
            localWalletInstructionPreview: {
                ok: true,
                value: {
                    mode: "preview",
                    instruction: "transfer",
                    instruction_words: ["0x01", "0x02"]
                },
                text: "OK",
                error: ""
            }
        }

        model.previewIdlInstruction({ instruction: "transfer" })

        tryVerify(function () {
            return fakeHost.calls.some(function (call) {
                return call.method === "localWalletInstructionPreview"
            })
        })
        const previewCalls = fakeHost.calls.filter(function (call) {
            return call.method === "localWalletInstructionPreview"
        })
        compare(previewCalls.length, 1)
        compare(previewCalls[0].args[0].instruction, "transfer")
        compare(model.idlInstructionPreviewValue.mode, "preview")
        compare(model.idlInstructionError, "")
        compare(model.localWalletOperations.length, 0)
    }

    function test_send_idl_instruction_uses_execution_confirmation_and_logs_operation() {
        configureReadyWallet()
        fakeHost.responses = {
            localWalletInstructionSubmit: {
                ok: true,
                value: {
                    mode: "tx",
                    instruction: "transfer",
                    tx_hash: "0xabcdef123456"
                },
                text: "OK",
                error: ""
            }
        }

        model.sendIdlInstruction({ instruction: "transfer" })

        tryVerify(function () {
            return fakeHost.calls.some(function (call) {
                return call.method === "localWalletInstructionSubmit"
            })
        })
        const instructionCalls = fakeHost.calls.filter(function (call) {
            return call.method === "localWalletInstructionSubmit"
        })
        compare(instructionCalls.length, 1)
        compare(instructionCalls[0].args[0].wallet_binary, "/usr/bin/lee-wallet")
        compare(instructionCalls[0].args[0].wallet_home, "/tmp/wallet-home")
        compare(instructionCalls[0].args[1].instruction, "transfer")
        compare(instructionCalls[0].args[2], "confirm-idl-instruction")
        compare(model.localWalletOperations.length, 0)
        compare(model.idlInstructionPreviewValue.mode, "tx")
        compare(model.idlInstructionError, "")
        const history = model.runtimeOperationHistoryRows("execution")
        compare(history.length, 1)
        compare(history[0].label, "IDL instruction")
        compare(history[0].status, "completed")
    }

    function test_create_wallet_account_uses_confirmation_and_logs_operation() {
        configureReadyWallet()
        model.walletCreatePrivacy = "private"
        model.walletCreateLabel = "receiver"
        fakeHost.responses = {
            localWalletCreateAccount: {
                ok: true,
                value: {
                    source: "local_wallet_cli",
                    status: "created",
                    command: "wallet account new private",
                    account_id: "Private/abc123"
                },
                text: "OK",
                error: ""
            }
        }

        model.wallet.createAccount()

        tryVerify(function () {
            return fakeHost.calls.some(function (call) {
                return call.method === "localWalletCreateAccount"
            })
        })
        const calls = fakeHost.calls.filter(function (call) {
            return call.method === "localWalletCreateAccount"
        })
        compare(calls.length, 1)
        compare(calls[0].args[0].wallet_binary, "/usr/bin/lee-wallet")
        compare(calls[0].args[1], "private")
        compare(calls[0].args[2], "receiver")
        compare(calls[0].args[3], "confirm-create-account")
        compare(model.walletCreateLabel, "")
        compare(model.localWalletOperations[0].label, "Create account")
        compare(model.localWalletOperations[0].status, "created")
        compare(model.runtimeOperationHistoryRows("wallet")[0].label, "Create account")
    }

    function test_send_wallet_transaction_uses_confirmation_and_logs_operation() {
        configureReadyWallet()
        model.walletSendFrom = "Public/source"
        model.walletSendTo = "Private/recipient"
        model.walletSendAmount = "37"
        fakeHost.responses = {
            localWalletSendTransaction: {
                ok: true,
                value: {
                    source: "local_wallet_cli",
                    status: "submitted",
                    command: "wallet auth-transfer send",
                    tx_hash: "tx123"
                },
                text: "OK",
                error: ""
            }
        }

        model.wallet.sendTransaction()

        tryVerify(function () {
            return fakeHost.calls.some(function (call) {
                return call.method === "localWalletSendTransaction"
            })
        })
        const calls = fakeHost.calls.filter(function (call) {
            return call.method === "localWalletSendTransaction"
        })
        compare(calls.length, 1)
        compare(calls[0].args[1].from, "Public/source")
        compare(calls[0].args[1].to, "Private/recipient")
        compare(calls[0].args[1].amount, "37")
        compare(calls[0].args[2], "confirm-send-transaction")
        compare(model.localWalletOperations[0].label, "Send transaction")
        compare(model.localWalletOperations[0].status, "submitted")
    }

    function test_read_incoming_wallet_transactions_uses_private_sync_confirmation() {
        configureReadyWallet()
        fakeHost.responses = {
            localWalletSyncPrivate: {
                ok: true,
                value: {
                    source: "local_wallet_cli",
                    status: "submitted",
                    wallet_home_source: "profile"
                },
                text: "OK",
                error: ""
            }
        }

        model.wallet.readIncomingTransactions()

        tryVerify(function () {
            return fakeHost.calls.some(function (call) {
                return call.method === "localWalletSyncPrivate"
            })
        })
        const syncCalls = fakeHost.calls.filter(function (call) {
            return call.method === "localWalletSyncPrivate"
        })
        compare(syncCalls.length, 1)
        compare(syncCalls[0].args[1], "confirm-sync-private")
        compare(model.localWalletOperations[0].label, "Read incoming")
        compare(model.localWalletOperations[0].status, "submitted")
    }

    function test_run_wallet_command_uses_confirmation_and_logs_operation() {
        configureReadyWallet()
        fakeHost.responses = {
            localWalletCommand: {
                ok: true,
                value: {
                    source: "local_wallet_cli",
                    status: "completed",
                    command: "wallet account get --account-id Public/abc"
                },
                text: "OK",
                error: ""
            }
        }

        model.wallet.runCommand(["account", "get", "--account-id", "Public/abc"])

        tryVerify(function () {
            return fakeHost.calls.some(function (call) {
                return call.method === "localWalletCommand"
            })
        })
        const calls = fakeHost.calls.filter(function (call) {
            return call.method === "localWalletCommand"
        })
        compare(calls.length, 1)
        compare(calls[0].args[1][0], "account")
        compare(calls[0].args[1][2], "--account-id")
        compare(calls[0].args[2], "confirm-wallet-command")
        compare(model.localWalletOperations[0].label, "Wallet command")
        compare(model.localWalletOperations[0].status, "completed")
    }

    function test_blocks_page_uses_tip_range_and_blocks_backend() {
        const nodeResult = {
            cryptarchia_info: {
                value: {
                    cryptarchia_info: {
                        slot: 30,
                        lib_slot: 20
                    }
                }
            }
        }
        const blocksResult = [
            { header: { slot: 30, id: "tip" }, transactions: [], _chain: { status: "pending" } },
            { header: { slot: 20, id: "lib" }, transactions: [], _chain: { status: "finalized" } }
        ]
        fakeHost.responses = {
            runtimeOperationStart: chainRuntimeStart({
                blockchainNode: nodeResult,
                blockchainBlocks: blocksResult
            })
        }

        model.chainPages.refreshBlocksPage()

        tryCompare(model, "blocksPageRows", blocksResult)
        compare(fakeHost.lastMethod, "runtimeOperationStart")
        compare(fakeHost.lastArgs[0].method, "blockchainBlocks")
        compare(fakeHost.lastArgs[0].args[1], 0)
        compare(fakeHost.lastArgs[0].args[2], 30)
        compare(fakeHost.lastArgs[0].args[3], 20)
        compare(callCountFor("blockchainNode"), 0)
        compare(callCountFor("blockchainBlocks"), 0)
        compare(runtimeOperationCallCount("blockchainNode"), 1)
        compare(runtimeOperationCallCount("blockchainBlocks"), 1)
        compare(model.blocksPageRows.length, 2)
        compare(model.blocksPageRows[0].header.slot, 30)
        compare(model.chainPages.blockStatus(model.blocksPageRows[0]), "pending")
        compare(model.chainPages.blockStatus(model.blocksPageRows[1]), "finalized")
    }

    function test_chain_workflow_flags_cover_delayed_page_operations() {
        fakeHost.responses = {
            runtimeOperationStart: function (args) {
                const request = args[0]
                const context = chainOperationContext(request)
                return {
                    ok: true,
                    value: {
                        operationId: "delayed-" + request.clientRequestId,
                        clientRequestId: request.clientRequestId,
                        domain: "blockchain",
                        backend: context.source,
                        method: request.method,
                        label: request.label,
                        status: "awaiting_external",
                        eventCursor: 1,
                        context: context,
                        result: null,
                        error: ""
                    },
                    text: "OK",
                    error: ""
                }
            }
        }

        model.chainPages.refreshBlocksPage()
        tryVerify(function () { return model.chainPages.blocksWorkflowRunning })
        verify(model.chainPages.operationsRunning)
        verify(!model.chainPages.transactionsWorkflowRunning)

        model.chainPages.invalidateOperations("switch workflow")
        tryVerify(function () { return !model.chainPages.blocksWorkflowRunning })
        model.chainPages.refreshTransactionsPage()
        tryVerify(function () { return model.chainPages.transactionsWorkflowRunning })
        verify(model.chainPages.operationsRunning)
        verify(!model.chainPages.blocksWorkflowRunning)

        model.chainPages.invalidateOperations("test cleanup")
        tryVerify(function () { return !model.chainPages.operationsRunning })
    }

    function test_chain_operations_ignore_unrelated_source_configuration_changes() {
        fakeHost.responses = {
            runtimeOperationStart: function (args) {
                const request = args[0]
                const context = chainOperationContext(request)
                return {
                    ok: true,
                    value: {
                        operationId: "delayed-" + request.clientRequestId,
                        clientRequestId: request.clientRequestId,
                        domain: "blockchain",
                        backend: context.source,
                        method: request.method,
                        label: request.label,
                        status: "awaiting_external",
                        eventCursor: 1,
                        context: context,
                        result: null,
                        error: ""
                    },
                    text: "OK",
                    error: ""
                }
            }
        }

        model.chainPages.refreshBlocksPage()
        tryVerify(function () { return model.chainPages.blocksWorkflowRunning })
        model.metrics.queryNetworkConnection("blockchain", false)
        tryVerify(function () {
            return model.metrics.networkConnectionPending.blockchain === true
        })
        model.blocksLiveEnabled = true
        const chainRevision = model.blockchainConfigurationRevision

        model.storageRestUrl = "http://127.0.0.1:18080/api/storage/v1"

        compare(model.blockchainConfigurationRevision, chainRevision)
        verify(model.chainPages.blocksWorkflowRunning)
        verify(model.metrics.networkConnectionPending.blockchain === true)
        verify(model.blocksLiveEnabled)
        compare(fakeHost.calls.filter(function (call) {
            return call.method === "runtimeOperationCancel"
        }).length, 0)

        model.nodeUrl = "http://127.0.0.1:18081/"
        tryVerify(function () { return !model.chainPages.operationsRunning })
        verify(model.blockchainConfigurationRevision > chainRevision)
        verify(model.metrics.networkConnectionPending.blockchain !== true)
        verify(!model.blocksLiveEnabled)
        tryVerify(function () {
            return fakeHost.calls.filter(function (call) {
                return call.method === "runtimeOperationCancel"
            }).length === 2
        })
        model.storageRestUrl = "http://127.0.0.1:8080/api/storage/v1"
        model.nodeUrl = "http://127.0.0.1:8080/"
    }

    function test_cached_chain_detail_supersedes_pending_remote_intent() {
        fakeHost.responses = {
            runtimeOperationStart: function (args) {
                const request = args[0]
                const context = chainOperationContext(request)
                return {
                    ok: true,
                    value: {
                        operationId: "pending-" + request.clientRequestId,
                        clientRequestId: request.clientRequestId,
                        domain: "blockchain",
                        backend: context.source,
                        method: request.method,
                        label: request.label,
                        status: "awaiting_external",
                        eventCursor: 1,
                        context: context,
                        result: null,
                        error: ""
                    },
                    text: "OK",
                    error: ""
                }
            }
        }

        model.entityNavigation.openMantleTransaction("remote-transaction")
        tryVerify(function () {
            return model.chainPages.operationPending("detail.transaction")
        })
        model.transactionsPageRows = [{
            hash: "cached-transaction",
            block: "cached-block",
            slot: 7,
            index: 0,
            operations: [],
            raw: { hash: "cached-transaction" }
        }]

        model.entityNavigation.openMantleTransaction("cached-transaction")

        verify(!model.chainPages.operationPending("detail.transaction"))
        compare(model.transactionDetailValue.hash, "cached-transaction")
        compare(model.shell.resultValue.hash, "cached-transaction")

        model.loadBlockchainBlockById("remote-block")
        tryVerify(function () {
            return model.chainPages.operationPending("detail.block")
        })
        model.entityNavigation.openBlockchainBlock({
            header: { id: "cached-block", slot: 8 },
            transactions: []
        })

        verify(!model.chainPages.operationPending("detail.block"))
        compare(model.blockDetailValue.hash, "cached-block")
        compare(model.shell.resultValue.hash, "cached-block")
        tryVerify(function () {
            return fakeHost.calls.filter(function (call) {
                return call.method === "runtimeOperationCancel"
            }).length === 2
        })
    }

    function test_delayed_block_failure_does_not_restore_stale_navigation() {
        let pendingRequest = null
        fakeHost.responses = {
            runtimeOperationStart: function (args) {
                pendingRequest = args[0]
                const context = chainOperationContext(pendingRequest)
                return {
                    ok: true,
                    value: {
                        operationId: "pending-block",
                        clientRequestId: pendingRequest.clientRequestId,
                        domain: "blockchain",
                        backend: context.source,
                        method: pendingRequest.method,
                        label: pendingRequest.label,
                        status: "awaiting_external",
                        eventCursor: 1,
                        context: context,
                        result: null,
                        error: ""
                    },
                    text: "OK",
                    error: ""
                }
            },
            runtimeOperationStatus: function () {
                const context = chainOperationContext(pendingRequest)
                return {
                    ok: true,
                    value: {
                        operationId: "pending-block",
                        clientRequestId: pendingRequest.clientRequestId,
                        domain: "blockchain",
                        backend: context.source,
                        method: pendingRequest.method,
                        label: pendingRequest.label,
                        status: "failed",
                        eventCursor: 2,
                        context: context,
                        result: null,
                        error: "not found"
                    },
                    text: "OK",
                    error: ""
                }
            }
        }

        model.loadBlockchainBlockById("missing-block")
        tryVerify(function () {
            const entry = model.chainPages.operationCoordinator.pendingOperations["detail.block"]
            return entry && String(entry.operationId || "").length > 0
        })
        model.shell.selectView("settings")

        model.chainPages.pollOperations()

        tryVerify(function () {
            return !model.chainPages.operationPending("detail.block")
        })
        compare(model.shell.currentView, "settings")
        verify(model.blockDetailError.length > 0)
    }

    function test_blocks_live_mode_merges_and_dedupes_snapshot() {
        model.shell.currentView = "blocks"
        model.blocksPageRows = [
            { header: { slot: 30, id: "slot-30" }, transactions: [] }
        ]
        model.blocksPageSlotFrom = 30
        model.blocksPageSlotTo = 30
        const liveResult = {
            source: "blocks_range",
            blocks: [
                { header: { slot: 31, id: "slot-31" }, transactions: [] },
                { header: { slot: 30, id: "slot-30-live" }, transactions: [] }
            ],
            unknown_events: [
                { kind: "heartbeat" }
            ]
        }
        fakeHost.responses = {
            runtimeOperationStart: chainRuntimeStart({
                blockchainNode: {
                    cryptarchia_info: {
                        value: {
                            cryptarchia_info: {
                                slot: 31,
                                lib_slot: 20
                            }
                        }
                    }
                },
                blockchainLiveBlocks: liveResult
            })
        }

        compare(model.chainPages.mergeLiveBlocks(liveResult.blocks, model.blocksPageRows, 20).length, 2)
        model.chainPages.startBlocksLiveMode()

        compare(model.blocksLiveEnabled, true)
        tryCompare(model, "blocksLiveSource", "blocks_range")
        compare(fakeHost.lastMethod, "runtimeOperationStart")
        compare(fakeHost.lastArgs[0].method, "blockchainLiveBlocks")
        compare(fakeHost.lastArgs[0].args[1], 30)
        compare(fakeHost.lastArgs[0].args[2], 31)
        compare(callCountFor("blockchainNode"), 0)
        compare(callCountFor("blockchainLiveBlocks"), 0)
        compare(model.blocksPageRows.length, 2)
        compare(model.blocksPageRows[0].header.id, "slot-31")
        compare(model.blocksPageRows[1].header.id, "slot-30-live")
        compare(model.blocksLiveSource, "blocks_range")
        compare(model.blocksLiveUnknownEvents, 1)
        compare(model.shell.resultOwner, "blocks")
        compare(model.shell.resultValue.unknown_events.length, 1)
    }

    function test_blocks_live_mode_waits_for_initial_page_workflow() {
        model.shell.currentView = "blocks"
        const nodeResult = {
            cryptarchia_info: {
                value: {
                    cryptarchia_info: { slot: 31, lib_slot: 20 }
                }
            }
        }
        fakeHost.responses = {
            runtimeOperationStart: chainRuntimeStart({
                blockchainNode: nodeResult,
                blockchainBlocks: [
                    { header: { slot: 30, id: "slot-30" }, transactions: [] }
                ],
                blockchainLiveBlocks: {
                    source: "blocks_range",
                    blocks: [
                        { header: { slot: 31, id: "slot-31" }, transactions: [] }
                    ],
                    unknown_events: []
                }
            })
        }

        model.chainPages.startBlocksLiveMode()
        tryVerify(function () {
            return model.blocksPageRows.length === 2
                && model.blocksPageRows[0].header.id === "slot-31"
        })
        const methods = fakeHost.calls.filter(function (call) {
            return call.method === "runtimeOperationStart"
        }).map(function (call) {
            return String(call.args[0].method || "")
        })
        compare(methods.join(","),
            "blockchainNode,blockchainBlocks,blockchainNode,blockchainLiveBlocks")
    }

    function test_blocks_live_continuation_cannot_supersede_newer_output() {
        let pendingNodeRequest = null
        const nodeResult = {
            cryptarchia_info: {
                value: {
                    cryptarchia_info: { slot: 31, lib_slot: 20 }
                }
            }
        }
        fakeHost.responses = {
            runtimeOperationStart: function (args) {
                const request = args[0]
                if (request.method === "blockchainNode" && pendingNodeRequest === null) {
                    pendingNodeRequest = request
                    const context = chainOperationContext(request)
                    return {
                        ok: true,
                        value: {
                            operationId: "pending-live-node",
                            clientRequestId: request.clientRequestId,
                            domain: "blockchain",
                            backend: context.source,
                            method: request.method,
                            label: request.label,
                            status: "awaiting_external",
                            eventCursor: 1,
                            context: context,
                            result: null,
                            error: ""
                        },
                        text: "OK",
                        error: ""
                    }
                }
                return chainRuntimeStart({
                    blockchainBlocks: [
                        { header: { slot: 30, id: "slot-30" }, transactions: [] }
                    ],
                    blockchainLiveBlocks: {
                        source: "blocks_range",
                        blocks: [{ header: { slot: 31, id: "slot-31" }, transactions: [] }],
                        unknown_events: []
                    }
                })(args)
            },
            runtimeOperationStatus: function () {
                const context = chainOperationContext(pendingNodeRequest)
                return {
                    ok: true,
                    value: {
                        operationId: "pending-live-node",
                        clientRequestId: pendingNodeRequest.clientRequestId,
                        domain: "blockchain",
                        backend: context.source,
                        method: pendingNodeRequest.method,
                        label: pendingNodeRequest.label,
                        status: "completed",
                        eventCursor: 2,
                        context: context,
                        result: nodeResult,
                        error: ""
                    },
                    text: "OK",
                    error: ""
                }
            }
        }

        model.chainPages.startBlocksLiveMode()
        tryVerify(function () {
            const entry = model.chainPages.operationCoordinator.pendingOperations["blocks.page.node"]
            return entry && String(entry.operationId || "").length > 0
        })
        model.shell.setResult("Newer output", "newer", false, { owner: "newer" }, "settings")

        model.chainPages.pollOperations()

        tryCompare(model, "blocksPageRows", [{
            header: { slot: 30, id: "slot-30" },
            transactions: []
        }])
        verify(!model.blocksLiveEnabled)
        compare(model.shell.resultTitle, "Newer output")
        compare(model.shell.resultValue.owner, "newer")
        const methods = fakeHost.calls.filter(function (call) {
            return call.method === "runtimeOperationStart"
        }).map(function (call) {
            return String(call.args[0].method || "")
        })
        compare(methods.join(","), "blockchainNode,blockchainBlocks")
    }

    function test_transactions_and_detail_lookups_use_runtime_operations() {
        const blocksResult = [{
            header: { slot: 40, id: "block-40" },
            transactions: [{ mantle_tx: { hash: "tx-page", ops: [] } }]
        }]
        fakeHost.responses = {
            runtimeOperationStart: chainRuntimeStart({
                blockchainNode: {
                    cryptarchia_info: {
                        value: {
                            cryptarchia_info: { slot: 50, lib_slot: 40 }
                        }
                    }
                },
                blockchainBlocks: blocksResult,
                blockchainBlock: {
                    header: { slot: 41, id: "block-lookup" },
                    transactions: []
                },
                blockchainTransaction: {
                    mantle_tx: { hash: "tx-lookup", ops: [] },
                    block_hash: "block-lookup",
                    slot: 41,
                    index: 0
                }
            })
        }

        model.chainPages.refreshTransactionsPage()
        tryVerify(function () { return model.transactionsPageRows.length === 1 })
        compare(model.transactionsPageRows[0].hash, "tx-page")

        model.loadBlockchainBlockById("block-lookup")
        tryVerify(function () {
            return model.blockDetailValue && model.blockDetailValue.hash === "block-lookup"
        })

        model.entityNavigation.openMantleTransaction("tx-lookup")
        tryVerify(function () {
            return model.transactionDetailValue
                && model.transactionDetailValue.hash === "tx-lookup"
        })

        compare(runtimeOperationCallCount("blockchainNode"), 1)
        compare(runtimeOperationCallCount("blockchainBlocks"), 1)
        compare(runtimeOperationCallCount("blockchainBlock"), 1)
        compare(runtimeOperationCallCount("blockchainTransaction"), 1)
        compare(callCountFor("blockchainNode"), 0)
        compare(callCountFor("blockchainBlocks"), 0)
        compare(callCountFor("blockchainBlock"), 0)
        compare(callCountFor("blockchainTransaction"), 0)
    }

    function test_block_lookup_retries_failed_prefixed_id_with_normalized_id() {
        const rawId = "0x" + "AB".repeat(32)
        const normalizedId = "ab".repeat(32)
        const attemptedIds = []
        fakeHost.responses = {
            runtimeOperationStart: function (args) {
                const request = args[0]
                const context = chainOperationContext(request)
                const failed = attemptedIds.length === 0
                attemptedIds.push(context.blockId)
                return {
                    ok: true,
                    value: {
                        operationId: "chain-op-" + String(request.clientRequestId || "unknown"),
                        clientRequestId: request.clientRequestId,
                        domain: "blockchain",
                        backend: context.source,
                        method: request.method,
                        label: request.label,
                        status: failed ? "failed" : "completed",
                        eventCursor: 1,
                        context: context,
                        result: failed ? null : {
                            header: { slot: 41, id: normalizedId },
                            transactions: []
                        },
                        error: failed ? "not found" : ""
                    },
                    text: failed ? "" : "OK",
                    error: ""
                }
            }
        }

        model.loadBlockchainBlockById(rawId)

        tryVerify(function () {
            return model.blockDetailValue
                && model.blockDetailValue.hash === normalizedId
        })
        compare(attemptedIds.join(","), rawId + "," + normalizedId)
        compare(runtimeOperationCallCount("blockchainBlock"), 2)
        compare(callCountFor("blockchainBlock"), 0)
        compare(model.blockDetailError, "")
    }

    function test_blockchain_module_new_block_event_updates_live_rows() {
        model.blocksPageRows = [
            { header: { slot: 30, id: "slot-30" }, transactions: [] }
        ]
        model.blocksPageSlotFrom = 30
        model.blocksPageSlotTo = 30

        moduleEventIntake.ingest(model.blockchainModule, "newBlock", [
            JSON.stringify({ header: { slot: 31, id: "slot-31-event" }, transactions: [] })
        ])

        compare(model.blocksPageRows.length, 2)
        compare(model.blocksPageRows[0].header.id, "slot-31-event")
        compare(model.blocksLiveSource, "module_event")
        compare(model.blocksPageSlotTo, 31)
        verify(model.blockchainModuleEventRevision > 0)
    }

    function test_delivery_module_message_event_merges_social_comment() {
        const topic = "/cryptarchia/account/account-1/comments"
        const payload = {
            kind: "comment",
            version: 1,
            identity: { display_name: "Peer" },
            body: "hello",
            created_at: "2026-07-07T00:00:00Z",
            conversation_id: topic
        }
        fakeHost.responses = {
            socialCommentRowFromEvent: function(args) {
                const event = args[0]
                return {
                    ok: true,
                    value: {
                        key: "event|hash-1",
                        cursor: "",
                        topic: event.topic,
                        identity: event.payload.identity,
                        displayName: "Peer",
                        body: event.payload.body,
                        createdAt: event.payload.created_at,
                        conversationId: event.payload.conversation_id
                    },
                    text: "OK",
                    error: ""
                }
            }
        }

        moduleEventIntake.ingest(model.deliveryModule, "messageReceived", [
            "hash-1",
            topic,
            JSON.stringify(payload),
            "1000"
        ])

        compare(model.deliveryModuleEventRows()[0].label, "messageReceived")
        compare(model.social.commentsView(topic).rows.length, 1)
        compare(model.social.commentsView(topic).rows[0].body, "hello")
    }

    function test_delivery_module_rejects_unvalidated_zone_comment() {
        const topic = "/lez/account/" + "a".repeat(64) + "/comments"
        fakeHost.responses = {
            socialCommentRowFromEvent: {
                ok: true,
                value: null,
                text: "",
                error: ""
            }
        }

        const accepted = model.social.applyIncomingComment({
            topic: topic,
            messageHash: "hash-invalid",
            payload: {
                kind: "comment",
                version: 2,
                identity: { display_name: "Peer" },
                body: "wrong scope",
                created_at: "2026-07-07T00:00:00Z",
                conversation_id: topic,
                scope: {
                    network_scope: { kind: "genesis_id", genesis_id: "1".repeat(64) },
                    zone_id: "2".repeat(64),
                    entity_kind: "account",
                    canonical_entity_key: "account-1"
                }
            }
        })

        compare(accepted, false)
        compare(model.social.commentsView(topic).rows.length, 0)
    }

    function test_stop_blocks_live_mode_keeps_paged_rows() {
        model.blocksLiveEnabled = true
        model.blocksLiveSource = "blocks_range+stream"
        model.blocksLiveUnknownEvents = 1
        model.blocksLiveCheckedAt = "10:00:00"
        model.blocksPageRows = [
            { header: { slot: 30, id: "slot-30" }, transactions: [] }
        ]

        model.chainPages.stopBlocksLiveMode()

        compare(model.blocksLiveEnabled, false)
        compare(model.blocksLiveError, "")
        compare(model.blocksLiveSource, "")
        compare(model.blocksLiveUnknownEvents, 0)
        compare(model.blocksLiveCheckedAt, "")
        compare(model.blocksPageRows.length, 1)
        compare(model.blocksPageRows[0].header.id, "slot-30")
    }

    function test_blockchain_module_probe_value_reads_peer_id() {
        model.metrics.blockchainModuleReport = {
            module: model.blockchainModule,
            module_info: { ok: true, value: {}, label: "module", source: "logoscore modules" },
            probes: [
                {
                    label: "blockchain_module.get_peer_id",
                    source: "blockchain_module get_peer_id",
                    ok: true,
                    value: "peer-123",
                    error: null
                }
            ]
        }

        compare(model.metrics.blockchainModuleReport.probes.length, 1)
        compare(model.metrics.moduleReport("blockchain").probes.length, 1)
        compare(model.metrics.moduleProbe("blockchain", "get_peer_id").value, "peer-123")
        compare(model.metrics.moduleProbeValue("blockchain", "get_peer_id"), "peer-123")
    }

    function test_bedrock_wallet_known_addresses_unwraps_module_payload() {
        model.metrics.blockchainModuleReport = blockchainWalletReport("wallet_get_known_addresses", {
            runner: "plain logoscore",
            value: {
                result: {
                    value: {
                        addresses: [
                            "addr-1",
                            { address: "addr-2", label: "default" }
                        ]
                    }
                }
            }
        })

        const rows = model.bedrockWalletModuleKnownAddressRows()

        compare(rows.length, 2)
        compare(rows[0].address, "addr-1")
        compare(rows[1].address, "addr-2")
        compare(rows[1].label, "default")
    }

    function test_bedrock_wallet_empty_known_addresses_are_known_shape() {
        model.metrics.blockchainModuleReport = blockchainWalletReport("wallet_get_known_addresses", {
            result: {
                value: []
            }
        })

        compare(model.bedrockWalletModuleKnownAddressRows().length, 0)
        compare(model.bedrockWalletModuleListKnown("wallet_get_known_addresses"), true)
    }

    function test_bedrock_wallet_notes_rows_format_note_fields() {
        model.metrics.blockchainModuleReport = blockchainWalletReport("wallet_get_notes", {
            result: {
                value: {
                    notes: [
                        {
                            note_id: "note-1",
                            value: "42",
                            commitment: "cm-1",
                            nullifier: "nf-1",
                            tip: "tip-1"
                        }
                    ]
                }
            }
        })

        const rows = model.bedrockWalletModuleNoteRows()

        compare(rows.length, 1)
        compare(rows[0].id, "note-1")
        compare(rows[0].value, "42")
        compare(rows[0].commitment, "cm-1")
        compare(rows[0].nullifier, "nf-1")
        compare(rows[0].tip, "tip-1")
    }

    function test_bedrock_wallet_voucher_rows_format_commitments() {
        model.metrics.blockchainModuleReport = blockchainWalletReport("wallet_get_claimable_vouchers", {
            result: {
                value: {
                    claimable_vouchers: [
                        {
                            voucher_commitment: "voucher-cm",
                            nullifier_hash: "voucher-nf",
                            amount: "7",
                            header_id: "header-1"
                        }
                    ]
                }
            }
        })

        const rows = model.bedrockWalletModuleVoucherRows()

        compare(rows.length, 1)
        compare(rows[0].commitment, "voucher-cm")
        compare(rows[0].nullifier, "voucher-nf")
        compare(rows[0].value, "7")
        compare(rows[0].tip, "header-1")
    }

    function test_bedrock_wallet_module_failure_keeps_other_probes_readable() {
        model.metrics.blockchainModuleReport = {
            module: model.blockchainModule,
            module_info: { ok: true, value: {}, label: "module", source: "logoscore modules" },
            probes: [
                {
                    label: "blockchain_module.wallet_get_known_addresses",
                    source: "blockchain_module wallet_get_known_addresses",
                    ok: true,
                    value: { result: { value: ["addr-ok"] } },
                    error: null
                },
                {
                    label: "blockchain_module.wallet_get_notes(addr-ok)",
                    source: "blockchain_module wallet_get_notes addr-ok",
                    ok: false,
                    value: null,
                    error: "module unavailable"
                }
            ]
        }

        compare(model.bedrockWalletModuleKnownAddressRows().length, 1)
        compare(model.bedrockWalletModuleNoteRows().length, 0)
        compare(model.metrics.moduleProbeError("blockchain", "wallet_get_notes"), "module unavailable")
    }

    function test_bedrock_wallet_module_methods_are_read_only() {
        const methods = model.bedrockWalletModuleReadOnlyMethods()

        verify(methods.indexOf("wallet_get_known_addresses") >= 0)
        verify(methods.indexOf("wallet_get_balance") >= 0)
        verify(methods.indexOf("wallet_get_notes") >= 0)
        verify(methods.indexOf("wallet_get_claimable_vouchers") >= 0)
        compare(methods.filter(function (method) {
            return method.indexOf("wallet_get_") !== 0
        }).length, 0)
    }

    function test_bedrock_network_summary_unwraps_probe_slot() {
        const value = {
            cryptarchia_info: {
                ok: true,
                value: {
                    cryptarchia_info: {
                        slot: 42
                    }
                }
            }
        }

        compare(model.metrics.networkConnectionSummary("blockchain", value), "slot 42")
    }

    function test_dashboard_refresh_uses_active_zone_projection_and_l1_sources() {
        const channelId = setActiveZone("")
        model.zoneInspection.zoneDetail = {
            summary: {
                channel_id: channelId,
                kind: "sequencer_zone",
                l2_zone: {
                    source_status: "reachable",
                    latest_block_id: 104,
                    latest_block_hash: "a".repeat(64),
                    finalized_block_id: 101,
                    finality_state: "provisional"
                },
                activity_detail: { last_seen_unix: 1000 }
            }
        }
        model.zoneInspection.l2.blocks.l2BlockRows = [104, 101].map(function (id) {
            const sequencer = id === 104
            return {
                summary: {
                    block_id: id,
                    block_hash: String(id).padStart(64, "0"),
                    parent_hash: "0".repeat(64),
                    timestamp: id,
                    bedrock_status: sequencer ? "Submitted" : "Finalized",
                    transaction_count: 0
                },
                observations: [{
                    source_id: sequencer ? "seq-a" : "idx-a",
                    source_role: sequencer ? "sequencer" : "indexer",
                    source_config_revision: 7,
                    finality: sequencer ? "provisional" : "finalized",
                    retrieval: "live"
                }]
            }
        })
        fakeHost.strictUnexpectedCalls = true
        const nodeResult = {
            cryptarchia_info: {
                value: {
                    cryptarchia_info: { slot: 30, lib_slot: 20 }
                }
            }
        }
        const liveResult = {
            blocks: [
                { header: { slot: 30, id: "l1-tip" }, transactions: [] }
            ]
        }
        fakeHost.responses = {
            runtimeOperationStart: chainRuntimeStart({
                blockchainNode: nodeResult,
                blockchainLiveBlocks: liveResult
            }),
            storageSourceReport: { ok: true, value: {}, text: "OK", error: "" },
            deliverySourceReport: { ok: true, value: {}, text: "OK", error: "" }
        }

        model.metrics.refreshDashboard()

        tryCompare(model.metrics, "dashboardRefreshing", false)
        compare(model.dashboardProvisionalBlocks.length, 1)
        compare(model.dashboardProvisionalBlocks[0].block_id, 104)
        compare(model.dashboardBlocks.length, 1)
        compare(model.dashboardBlocks[0].block_id, 101)
        compare(model.dashboardLezBlockRows.length, 2)
        compare(model.dashboardL1Blocks.length, 1)
        compare(model.metrics.sequencerHeadValue(), 104)
        compare(model.metrics.indexerHeadValue(), 101)
        compare(model.metrics.indexerLag(), 3)
        compare(model.metrics.dashboardMetricRawValue(
            "indexer.indexer_lag_vs_sequencer_head"), 3)
        compare(callCountFor("blockchainNode"), 0)
        compare(callCountFor("blockchainLiveBlocks"), 0)
        compare(runtimeOperationCallCount("blockchainNode"), 1)
        compare(runtimeOperationCallCount("blockchainLiveBlocks"), 1)
        compare(callCountFor("sequencerBlocks"), 0)
        compare(callCountFor("indexerBlocks"), 0)
        compare(callCountFor("lezBlockListReport"), 0)
    }

    function setTipMinusLib(value) {
        model.dashboardNode = {
            cryptarchia_info: {
                value: {
                    cryptarchia_info: {
                        slot: value,
                        lib_slot: 0
                    }
                }
            }
        }
    }

    function blockchainWalletReport(method, value) {
        return {
            module: model.blockchainModule,
            module_info: { ok: true, value: {}, label: "module", source: "logoscore modules" },
            probes: [
                {
                    label: "blockchain_module." + method,
                    source: "blockchain_module " + method,
                    ok: true,
                    value: value,
                    error: null
                }
            ]
        }
    }
}
