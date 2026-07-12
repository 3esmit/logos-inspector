.import "SourceHealthProjection.js" as SourceHealthProjection
.import "SourcePolicyProjection.js" as SourcePolicyProjection

function coreSourceView(root, role) {
    const key = "blockchain"
    const scope = "l1"
    const configuredMode = root.blockchainSourceMode
    const configuredEndpoint = root.nodeUrl
    const mode = root.connectorSourceMode(scope, configuredMode)
    const endpoint = root.connectorEndpoint(scope, configuredEndpoint)
    const label = root.blockchainSourceLabel()
    const connector = root.connectorScope(scope)
    return {
        role: key,
        mode: String(mode || "rpc"),
        configuredMode: String(configuredMode || "rpc"),
        connector: connector,
        resolvedMode: SourcePolicyProjection.resolvedSourceModeKey(root, "core", mode),
        effectiveMode: String(SourcePolicyProjection.sourceModePolicy(root, "core", SourcePolicyProjection.resolvedSourceModeKey(root, "core", mode)).effective || "rpc"),
        label: label,
        target: root.blockchainSourceTarget(),
        endpoint: endpoint,
        args: function (extra) {
            return SourcePolicyProjection.coreSourceArgs(root, mode, endpoint, extra)
        }
    }
}

function deliverySourceView(root) {
    const mode = root.connectorSourceMode("delivery", root.messagingSourceMode)
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
    const mode = root.connectorSourceMode("storage", root.storageSourceMode)
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

function sourceFamilyView(root, family, role, report) {
    const key = sourceFamilyKey(family, role)
    const route = sourceFamilyRouteView(root, key)
    const reportFacts = reportView(root, key, report)
    return {
        family: key,
        role: String(route.role || key),
        route: route,
        report: reportFacts,
        connector: route.connector,
        mode: route.mode,
        configuredMode: route.configuredMode,
        resolvedMode: route.resolvedMode,
        effectiveMode: route.effectiveMode,
        label: route.label,
        target: route.target,
        ready: reportFacts.ready,
        reachable: reportFacts.reachable,
        health: reportFacts.health,
        summary: reportFacts.summary,
        error: reportFacts.error,
        capability: function (capabilityKey) { return reportFacts.capability(capabilityKey) },
        capabilityAvailable: function (capabilityKey) { return reportFacts.capabilityAvailable(capabilityKey) },
        capabilityEvidence: function (capabilityKey) { return reportFacts.capabilityEvidence(capabilityKey) },
        capabilityValue: function (capabilityKey) { return reportFacts.capabilityValue(capabilityKey) },
        probe: function (probeKey) { return reportFacts.probe(probeKey) },
        probeOk: function (probeKey) { return reportFacts.probeOk(probeKey) },
        probeValue: function (probeKey) { return reportFacts.probeValue(probeKey) }
    }
}

function sourceFamilyRouteView(root, family) {
    switch (family) {
    case "l1":
        return coreSourceView(root, "blockchain")
    case "delivery":
        return deliverySourceView(root)
    case "storage":
        return storageSourceView(root)
    default:
        return coreSourceView(root, "blockchain")
    }
}

function sourceFamilyKey(family, role) {
    const key = String(family || "").trim()
    switch (key) {
    case "blockchain":
    case "node":
    case "l1":
        return "l1"
    case "messaging":
    case "delivery":
        return "delivery"
    case "storage_source":
    case "storage":
        return "storage"
    default:
        return sourceFamilyKey(role || "l1", "")
    }
}

function sourceView(root, family, mode, options, details) {
    const resolvedMode = SourcePolicyProjection.resolvedSourceModeKey(root, family, mode)
    const policy = SourcePolicyProjection.sourceModePolicy(root, family, resolvedMode)
    const adapter = SourcePolicyProjection.sourceModeAdapter(root, family, mode)
    return {
        family: family,
        mode: String(mode || "rest"),
        configuredMode: String(root[family === "delivery" ? "messagingSourceMode" : "storageSourceMode"] || mode || "rest"),
        connector: root.connectorScope(family),
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
    const reportKind = sourceFamilyReportKind(family)
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
        summary: SourceHealthProjection.networkConnectionSummary(root, reportKind, report),
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

function sourceFamilyReportKind(family) {
    switch (String(family || "")) {
    case "l1":
        return "blockchain"
    case "storage":
        return "storage"
    case "delivery":
        return "messaging"
    default:
        return "messaging"
    }
}
