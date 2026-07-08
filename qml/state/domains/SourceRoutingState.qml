import QtQuick
import "../network/SourcePolicyCatalog.js" as SourcePolicyCatalog
import "../network/SourcePolicyProjection.js" as SourcePolicyProjection
import "../network/SourceRoutingUi.js" as SourceRoutingUi

QtObject {
    id: root

    required property var gateway

    property var sourcePolicy: ({})
    property bool sourcePolicyLoaded: false
    property string blockchainModule: ""
    property string indexerModule: ""
    property string deliveryModule: ""
    property string storageModule: ""
    property string blockchainSourceMode: "auto"
    property string indexerSourceMode: "auto"
    property string executionSourceMode: "rpc"
    property string messagingSourceMode: "auto"
    property string storageSourceMode: "auto"
    property string nodeUrl: ""
    property string indexerUrl: ""
    property string sequencerUrl: ""
    property string messagingRestUrl: ""
    property string messagingMetricsUrl: ""
    property string messagingNetworkPreset: ""
    property bool messagingMutatingDiagnosticsEnabled: false
    property string storageRestUrl: ""
    property string storageMetricsUrl: ""
    property string storageNetworkPreset: ""
    property string storageCidProbe: ""
    property bool storagePrivilegedDebugEnabled: false
    property bool storageMutatingDiagnosticsEnabled: false

    function loadSourcePolicy() {
        const response = gateway.callInspector("sourcePolicy", [])
        if (response.ok === true && response.value && typeof response.value === "object") {
            sourcePolicy = response.value
            sourcePolicyLoaded = true
            return true
        }
        sourcePolicy = SourcePolicyCatalog.fallbackPolicy()
        sourcePolicyLoaded = false
        return false
    }

    function prefersBasecampModules() {
        return gateway.prefersBasecampModules()
    }

    function sourcePolicyDefault(key, fallback) {
        return SourcePolicyProjection.sourcePolicyDefault(root, key, fallback)
    }

    function sourceModePolicy(family, value) {
        return SourcePolicyProjection.sourceModePolicy(root, family, value)
    }

    function sourceModePolicies(family) {
        return SourcePolicyProjection.sourceModePolicies(root, family)
    }

    function sourceModeOptions(family) {
        return SourcePolicyProjection.sourceModeOptions(root, family)
    }

    function sourceModeIndexFor(family, value, options) {
        return SourcePolicyProjection.sourceModeIndexFor(root, family, value, options)
    }

    function sourceModeAt(index, options) {
        return SourcePolicyProjection.sourceModeAt(index, options)
    }

    function sourceModeAdapter(family, value) {
        return SourcePolicyProjection.sourceModeAdapter(root, family, value)
    }

    function resolvedSourceModeKey(family, value) {
        return SourcePolicyProjection.resolvedSourceModeKey(root, family, value)
    }

    function sourceModeTargetKind(family, value) {
        return SourcePolicyProjection.sourceModeTargetKind(root, family, value)
    }

    function sourceModeUsesEndpoint(family, value, endpointKind) {
        return SourcePolicyProjection.sourceModeUsesEndpoint(root, family, value, endpointKind)
    }

    function sourceModeSupportsCidProbe(family, value) {
        return SourcePolicyProjection.sourceModeSupportsCidProbe(root, family, value)
    }

    function sourceModeSupportsMutatingDiagnostics(family, value) {
        return SourcePolicyProjection.sourceModeSupportsMutatingDiagnostics(root, family, value)
    }

    function coreSourceArgs(sourceMode, endpoint, extra) {
        return SourcePolicyProjection.coreSourceArgs(root, sourceMode, endpoint, extra)
    }

    function blockchainArgs(extra) {
        return coreSourceArgs(blockchainSourceMode, nodeUrl, extra)
    }

    function indexerArgs(extra) {
        return coreSourceArgs(indexerSourceMode, indexerUrl, extra)
    }

    function executionArgs(extra) {
        return coreSourceArgs(executionSourceMode, sequencerUrl, extra)
    }

    function accountLookupArgs(executionSourceMode, sequencerEndpoint, indexerSourceMode, indexerEndpoint, account, idlJson, accountType) {
        return SourcePolicyProjection.accountLookupArgs(
            root,
            executionSourceMode,
            sequencerEndpoint,
            indexerSourceMode,
            indexerEndpoint,
            account,
            idlJson,
            accountType
        )
    }

    function accountArgs(account, idlJson, accountType) {
        return accountLookupArgs(executionSourceMode, sequencerUrl, indexerSourceMode, indexerUrl, account, idlJson, accountType)
    }

    function lezLookupArgs(executionSourceMode, sequencerEndpoint, indexerSourceMode, indexerEndpoint, target) {
        return SourcePolicyProjection.lezLookupArgs(
            root,
            executionSourceMode,
            sequencerEndpoint,
            indexerSourceMode,
            indexerEndpoint,
            target
        )
    }

    function lezArgs(target) {
        return lezLookupArgs(executionSourceMode, sequencerUrl, indexerSourceMode, indexerUrl, target)
    }

    function deliverySourceReportArgs(sourceMode, restEndpoint, metricsEndpoint) {
        if (arguments.length === 0) {
            return SourceRoutingUi.deliverySourceView(root).reportArgs()
        }
        return SourcePolicyProjection.deliverySourceReportArgs(root, sourceMode, restEndpoint, metricsEndpoint)
    }

    function storageSourceReportArgs(sourceMode, restEndpoint, metricsEndpoint, cid, includeCidProbe, privilegedDebugEnabled) {
        if (arguments.length <= 1) {
            return SourceRoutingUi.storageSourceView(root).reportArgs(sourceMode === true)
        }
        return SourcePolicyProjection.storageSourceReportArgs(
            root,
            sourceMode,
            restEndpoint,
            metricsEndpoint,
            cid,
            includeCidProbe,
            privilegedDebugEnabled
        )
    }

    function sourceTarget(family, sourceMode, targets) {
        return SourcePolicyProjection.sourceTarget(root, family, sourceMode, targets)
    }

    function sourceLabel(family, sourceMode, fallbackLabel) {
        return SourcePolicyProjection.sourceLabel(root, family, sourceMode, fallbackLabel)
    }

    function coreSourceLabel(sourceMode, rpcLabel) {
        return SourcePolicyProjection.coreSourceLabel(root, sourceMode, rpcLabel)
    }

    function configuredMessagingRestUrl(value) {
        const endpoint = String(value === undefined ? messagingRestUrl : (value || "")).trim()
        return endpoint.length ? endpoint : sourcePolicyDefault("delivery_rest_endpoint", "http://127.0.0.1:8645")
    }

    function configuredStorageRestUrl(value) {
        const endpoint = String(value === undefined ? storageRestUrl : (value || "")).trim()
        return endpoint.length ? endpoint : sourcePolicyDefault("storage_rest_endpoint", "http://127.0.0.1:8080/api/storage/v1")
    }

    function normalizedCoreSourceMode(value) {
        const source = sourceModePolicy("core", value)
        return String(source.key || "auto")
    }

    function effectiveCoreSourceMode(value) {
        const source = sourceModePolicy("core", resolvedSourceModeKey("core", value))
        return String(source.effective || "rpc")
    }

    function normalizedMessagingSourceMode(value) {
        const source = sourceModePolicy("delivery", value)
        return String(source.key || "auto")
    }

    function effectiveMessagingSourceMode(value) {
        const source = sourceModePolicy("delivery", resolvedSourceModeKey("delivery", value))
        return String(source.effective || "rest")
    }

    function normalizedStorageSourceMode(value) {
        const source = sourceModePolicy("storage", value)
        return String(source.key || "auto")
    }

    function effectiveStorageSourceMode(value) {
        const source = sourceModePolicy("storage", resolvedSourceModeKey("storage", value))
        return String(source.effective || "rest")
    }

    function blockchainSourceLabel() {
        return coreSourceLabel(blockchainSourceMode, qsTr("Bedrock RPC"))
    }

    function blockchainSourceTarget() {
        return effectiveCoreSourceMode(blockchainSourceMode) === "module" ? blockchainModule : String(nodeUrl || "")
    }

    function indexerSourceLabel() {
        return coreSourceLabel(indexerSourceMode, qsTr("Indexer RPC"))
    }

    function indexerSourceTarget() {
        return effectiveCoreSourceMode(indexerSourceMode) === "module" ? indexerModule : String(indexerUrl || "")
    }

    function executionSourceLabel() {
        return effectiveCoreSourceMode(executionSourceMode) === "module" ? qsTr("LEZ core module") : qsTr("Sequencer RPC")
    }

    function executionSourceTarget() {
        return effectiveCoreSourceMode(executionSourceMode) === "module" ? "lez_core" : String(sequencerUrl || "")
    }

    function deliverySourceLabel() {
        return sourceLabel("delivery", messagingSourceMode, qsTr("Direct Waku REST"))
    }

    function deliverySourceTarget() {
        return sourceTarget("delivery", messagingSourceMode, {
            module: deliveryModule,
            rest: configuredMessagingRestUrl(),
            metrics: messagingMetricsUrl
        })
    }

    function storageSourceLabel() {
        return sourceLabel("storage", storageSourceMode, qsTr("Standalone REST"))
    }

    function storageSourceTarget() {
        return sourceTarget("storage", storageSourceMode, {
            module: storageModule,
            rest: configuredStorageRestUrl(),
            metrics: storageMetricsUrl
        })
    }

    function coreSourceView(role) {
        return SourceRoutingUi.coreSourceView(root, role)
    }

    function deliverySourceView() {
        return SourceRoutingUi.deliverySourceView(root)
    }

    function storageSourceView() {
        return SourceRoutingUi.storageSourceView(root)
    }

    function deliveryReportView(report) {
        return SourceRoutingUi.deliveryReportView(root, report)
    }

    function storageReportView(report) {
        return SourceRoutingUi.storageReportView(root, report)
    }
}
