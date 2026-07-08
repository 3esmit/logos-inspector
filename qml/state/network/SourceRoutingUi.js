.import "SourceHealthProjection.js" as SourceHealthProjection
.import "SourcePolicyProjection.js" as SourcePolicyProjection

function coreSourceView(root, role) {
    const key = String(role || "execution")
    const mode = key === "indexer" ? root.indexerSourceMode : (key === "blockchain" ? root.blockchainSourceMode : root.executionSourceMode)
    const endpoint = key === "indexer" ? root.indexerUrl : (key === "blockchain" ? root.nodeUrl : root.sequencerUrl)
    const label = key === "indexer"
        ? root.indexerSourceLabel()
        : (key === "blockchain" ? root.blockchainSourceLabel() : root.executionSourceLabel())
    return {
        role: key,
        mode: String(mode || "auto"),
        resolvedMode: SourcePolicyProjection.resolvedSourceModeKey(root, "core", mode),
        effectiveMode: String(SourcePolicyProjection.sourceModePolicy(root, "core", SourcePolicyProjection.resolvedSourceModeKey(root, "core", mode)).effective || "rpc"),
        label: label,
        target: key === "indexer" ? root.indexerSourceTarget() : (key === "blockchain" ? root.blockchainSourceTarget() : root.executionSourceTarget()),
        endpoint: endpoint,
        args: function (extra) {
            return SourcePolicyProjection.coreSourceArgs(root, mode, endpoint, extra)
        }
    }
}

function deliverySourceView(root) {
    const mode = root.messagingSourceMode
    const options = SourcePolicyProjection.sourceModeOptions(root, "delivery")
    return sourceView(root, "delivery", mode, options, {
        label: root.deliverySourceLabel(),
        target: root.deliverySourceTarget(),
        restEndpoint: root.configuredMessagingRestUrl(),
        metricsEndpoint: root.messagingMetricsUrl,
        moduleName: root.deliveryModule,
        networkPreset: root.messagingNetworkPreset,
        mutatingDiagnosticsEnabled: root.messagingMutatingDiagnosticsEnabled === true,
        reportArgs: function () { return SourcePolicyProjection.deliverySourceReportArgs(root, mode, root.configuredMessagingRestUrl(), root.messagingMetricsUrl) },
        actionArgs: function (extra) {
            const source = deliverySourceView(root)
            return [
                source.effectiveMode,
                source.usesRestEndpoint ? source.restEndpoint : ""
            ].concat(extra || [])
        }
    })
}

function storageSourceView(root) {
    const mode = root.storageSourceMode
    const options = SourcePolicyProjection.sourceModeOptions(root, "storage")
    return sourceView(root, "storage", mode, options, {
        label: root.storageSourceLabel(),
        target: root.storageSourceTarget(),
        restEndpoint: root.configuredStorageRestUrl(),
        metricsEndpoint: root.storageMetricsUrl,
        moduleName: root.storageModule,
        networkPreset: root.storageNetworkPreset,
        mutatingDiagnosticsEnabled: root.storageMutatingDiagnosticsEnabled === true,
        reportArgs: function (includeCidProbe) {
            return SourcePolicyProjection.storageSourceReportArgs(
                root,
                mode,
                root.configuredStorageRestUrl(),
                root.storageMetricsUrl,
                root.storageCidProbe,
                includeCidProbe === true,
                root.storagePrivilegedDebugEnabled
            )
        },
        actionArgs: function (extra) {
            const source = storageSourceView(root)
            return [
                source.effectiveMode,
                source.usesRestEndpoint ? source.restEndpoint : ""
            ].concat(extra || [])
        }
    })
}

function sourceView(root, family, mode, options, details) {
    const resolvedMode = SourcePolicyProjection.resolvedSourceModeKey(root, family, mode)
    const policy = SourcePolicyProjection.sourceModePolicy(root, family, resolvedMode)
    const adapter = SourcePolicyProjection.sourceModeAdapter(root, family, mode)
    return {
        family: family,
        mode: String(mode || "auto"),
        resolvedMode: resolvedMode,
        effectiveMode: String(policy.effective || resolvedMode),
        label: String(details.label || ""),
        target: String(details.target || ""),
        targetKind: String(adapter.target || "none"),
        options: options,
        currentIndex: function (candidateOptions) {
            return SourcePolicyProjection.sourceModeIndexFor(root, family, mode, candidateOptions || options)
        },
        modeAt: function (index, candidateOptions) {
            return SourcePolicyProjection.sourceModeAt(index, candidateOptions || options)
        },
        usesRestEndpoint: adapter.uses_rest_endpoint === true,
        usesMetricsEndpoint: adapter.uses_metrics_endpoint === true,
        supportsCidProbe: adapter.supports_cid_probe === true,
        supportsMutatingDiagnostics: adapter.supports_mutating_diagnostics === true,
        restEndpoint: String(details.restEndpoint || ""),
        metricsEndpoint: String(details.metricsEndpoint || ""),
        moduleName: String(details.moduleName || ""),
        networkPreset: String(details.networkPreset || ""),
        mutatingDiagnosticsEnabled: details.mutatingDiagnosticsEnabled === true,
        reportArgs: details.reportArgs,
        actionArgs: details.actionArgs
    }
}

function storageReportView(root, report) {
    return reportView(root, "storage", report)
}

function deliveryReportView(root, report) {
    return reportView(root, "delivery", report)
}

function reportView(root, family, report) {
    const health = SourceHealthProjection.sourceHealth(report)
    const ready = SourceHealthProjection.sourceHealthReady(report)
    const reachable = health && health.reachable !== undefined
        ? health.reachable === true
        : SourceHealthProjection.moduleReportReachable(root, report)
    return {
        family: family,
        report: report || null,
        reachable: reachable,
        ready: ready !== null ? ready : reachable,
        health: health,
        summary: SourceHealthProjection.networkConnectionSummary(root, family === "storage" ? "storage" : "messaging", report),
        error: SourceHealthProjection.moduleReportError(report),
        capability: function (key) { return SourceHealthProjection.sourceCapability(report, key) },
        capabilityAvailable: function (key) { return SourceHealthProjection.sourceCapabilityAvailable(report, key) },
        capabilityEvidence: function (key) { return SourceHealthProjection.sourceCapabilityEvidence(report, key) },
        capabilityValue: function (key) { return SourceHealthProjection.sourceCapabilityValue(report, key) },
        probe: function (key) { return SourceHealthProjection.reportProbe(report, key) },
        probeOk: function (key) { return SourceHealthProjection.reportProbeOk(report, key) },
        probeValue: function (key) { return SourceHealthProjection.reportProbeValue(report, key) }
    }
}
