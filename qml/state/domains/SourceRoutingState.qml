import QtQuick
import "../source_routing/ConnectorConfigAdapter.js" as ConnectorConfigAdapter
import "../source_routing/SourcePolicyCatalog.js" as SourcePolicyCatalog
import "../source_routing/SourcePolicyProjection.js" as SourcePolicyProjection
import "../source_routing/SourceRoutingUi.js" as SourceRoutingUi

QtObject {
    id: root

    required property var gateway

    property var sourcePolicy: ({})
    property bool sourcePolicyLoaded: false
    property string blockchainModule: ""
    property string deliveryModule: ""
    property string storageModule: ""
    property string blockchainSourceMode: "rpc"
    property string messagingSourceMode: "rest"
    property string storageSourceMode: "rest"
    property string nodeUrl: ""
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
    property var connectorConfig: ({})

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

    function sourceModeDescriptor(family, value) {
        return SourcePolicyProjection.sourceModeDescriptor(root, family, value)
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

    function sourceModeUsesInput(family, value, inputKey) {
        return SourcePolicyProjection.sourceModeUsesInput(root, family, value, inputKey)
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
        return coreSourceArgs(connectorSourceMode("l1", blockchainSourceMode), connectorEndpoint("l1", nodeUrl), extra)
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
        const endpoint = String(value === undefined ? connectorEndpoint("delivery", messagingRestUrl) : (value || "")).trim()
        return endpoint.length ? endpoint : sourcePolicyDefault("delivery_rest_endpoint", "http://127.0.0.1:8645")
    }

    function configuredStorageRestUrl(value) {
        const endpoint = String(value === undefined ? connectorEndpoint("storage", storageRestUrl) : (value || "")).trim()
        return endpoint.length ? endpoint : sourcePolicyDefault("storage_rest_endpoint", "http://127.0.0.1:8080/api/storage/v1")
    }

    function normalizedCoreSourceMode(value) {
        const source = sourceModePolicy("core", value)
        return String(source.key || "rpc")
    }

    function effectiveCoreSourceMode(value) {
        const source = sourceModePolicy("core", resolvedSourceModeKey("core", connectorSourceMode("l1", value)))
        return String(source.effective || "rpc")
    }

    function normalizedMessagingSourceMode(value) {
        const source = sourceModePolicy("delivery", value)
        return String(source.key || "rest")
    }

    function effectiveMessagingSourceMode(value) {
        const source = sourceModePolicy("delivery", resolvedSourceModeKey("delivery", connectorSourceMode("delivery", value)))
        return String(source.effective || "rest")
    }

    function normalizedStorageSourceMode(value) {
        const source = sourceModePolicy("storage", value)
        return String(source.key || "rest")
    }

    function effectiveStorageSourceMode(value) {
        const source = sourceModePolicy("storage", resolvedSourceModeKey("storage", connectorSourceMode("storage", value)))
        return String(source.effective || "rest")
    }

    function blockchainSourceLabel() {
        return coreSourceLabel(connectorSourceMode("l1", blockchainSourceMode), qsTr("Bedrock RPC"))
    }

    function blockchainSourceTarget() {
        return effectiveCoreSourceMode(blockchainSourceMode) === "module" ? blockchainModule : connectorEndpoint("l1", nodeUrl)
    }

    function deliverySourceLabel() {
        return sourceLabel("delivery", connectorSourceMode("delivery", messagingSourceMode), qsTr("Direct Waku REST"))
    }

    function deliverySourceTarget() {
        return sourceTarget("delivery", connectorSourceMode("delivery", messagingSourceMode), {
            module: deliveryModule,
            rest: configuredMessagingRestUrl(),
            metrics: messagingMetricsUrl
        })
    }

    function storageSourceLabel() {
        return sourceLabel("storage", connectorSourceMode("storage", storageSourceMode), qsTr("Standalone REST"))
    }

    function storageSourceTarget() {
        return sourceTarget("storage", connectorSourceMode("storage", storageSourceMode), {
            module: storageModule,
            rest: configuredStorageRestUrl(),
            metrics: storageMetricsUrl
        })
    }

    function connectorScope(scope) {
        return ConnectorConfigAdapter.connectorScope(connectorConfig, scope)
    }

    function connectorSourceMode(scope, fallbackMode) {
        return ConnectorConfigAdapter.connectorSourceMode(connectorConfig, scope, fallbackMode)
    }

    function connectorEndpoint(scope, fallbackEndpoint) {
        return ConnectorConfigAdapter.connectorEndpoint(connectorConfig, scope, fallbackEndpoint)
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

    function sourceFamilyView(family, role, report) {
        return SourceRoutingUi.sourceFamilyView(root, family, role, report)
    }

    function deliveryReportView(report) {
        return SourceRoutingUi.deliveryReportView(root, report)
    }

    function storageReportView(report) {
        return SourceRoutingUi.storageReportView(root, report)
    }
}
