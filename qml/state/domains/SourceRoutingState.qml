import QtQuick
import "../network/SourcePolicyCatalog.js" as SourcePolicyCatalog
import "../network/SourcePolicyProjection.js" as SourcePolicyProjection

QtObject {
    id: root

    required property var gateway

    property var sourcePolicy: ({})
    property bool sourcePolicyLoaded: false

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

    function deliverySourceReportArgs(sourceMode, restEndpoint, metricsEndpoint) {
        return SourcePolicyProjection.deliverySourceReportArgs(root, sourceMode, restEndpoint, metricsEndpoint)
    }

    function storageSourceReportArgs(sourceMode, restEndpoint, metricsEndpoint, cid, includeCidProbe, privilegedDebugEnabled) {
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
        const endpoint = String(value || "").trim()
        return endpoint.length ? endpoint : sourcePolicyDefault("delivery_rest_endpoint", "http://127.0.0.1:8645")
    }

    function configuredStorageRestUrl(value) {
        const endpoint = String(value || "").trim()
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
}
