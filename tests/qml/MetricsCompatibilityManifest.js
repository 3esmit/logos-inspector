.pragma library

function manifest() {
    const properties = [
        "dashboardError", "blockchainRefreshRate", "messagingRefreshRate",
        "storageRefreshRate", "networkConnectionStatus",
        "networkConnectionStatusRevision", "footerFieldSelections",
        "footerFieldRevision", "dashboardGraphSelections",
        "dashboardGraphRevision", "dashboardMetricHistory",
        "dashboardMetricLastSeen", "dashboardMetricHistoryRevision",
        "networkConnectionPending", "networkConnectionPendingRevision",
        "dashboardRefreshing", "dashboardRefreshSerial",
        "blockchainModuleReport", "storageModuleReport",
        "messagingModuleReport", "storageSourceReport",
        "messagingSourceReport"
    ]
    const methods = [
        "refreshInterval", "dashboardRefreshInterval", "canonicalRefreshRate",
        "networkConnectionRate", "setNetworkConnectionRate",
        "queryNetworkConnection", "networkConnectionRequest",
        "updateNetworkConnectionStatusForMethod",
        "networkConnectionKindForMethod", "setNetworkConnectionPending",
        "networkConnectionIsPending", "updateNetworkConnectionStatus",
        "cacheNetworkConnectionResult", "networkConnectionSummary",
        "connectionValueOk", "storageReportReady", "moduleReportReachable",
        "sourceHealthReady", "sourceCapabilityAvailable",
        "sourceCapabilityEvidence", "sourceCapabilityValue", "sourceProbeFact",
        "reportProbeValue", "reportProbeOk", "reportProbe",
        "deliveryReportHealthy", "deliveryHealthValueOk", "moduleReportError",
        "networkConnectionState", "setFooterFieldEnabled", "footerFieldEnabled",
        "setDashboardGraphEnabled", "dashboardGraphEnabled", "scalarValue",
        "valueText", "valueToString", "moduleProbe", "moduleProbeError",
        "moduleLastError", "openMetricsText", "openMetricsTextFromValue",
        "openMetricLabels", "metricJsonValue", "metricSpecName",
        "metricSpecLabels", "metricJsonLabels", "metricLabelsMatch",
        "metricNumber", "overviewProbeValue", "indexerHeadValue",
        "sequencerHeadValue", "nodeProbeValue", "cryptarchiaInfo",
        "cryptarchiaValue", "networkInfo", "networkValue", "mantleMetrics",
        "mantleValue", "tipMinusLib", "finalityLagSeconds", "indexerLag",
        "openMetricSeries", "moduleMetricValue", "moduleMetricSeries",
        "moduleMetricSum", "storageManifestCount",
        "dashboardMetricRawValue", "dashboardMetricUsesWindow",
        "dashboardMetricWindowDelta", "recordDashboardSnapshot",
        "dashboardMetricSamples", "normalizedDashboardSamples",
        "dashboardMetricWindowSamples", "windowDeltaFromSamples",
        "defaultDashboardGraphSelections", "refreshDashboard",
        "updateDashboardCache", "clearDashboardMetricHistoryForPrefix"
    ]
    const retiredMembers = []
    for (let propertyIndex = 0; propertyIndex < properties.length; ++propertyIndex) {
        retiredMembers.push(retired(properties[propertyIndex], "property"))
    }
    for (let methodIndex = 0; methodIndex < methods.length; ++methodIndex) {
        retiredMembers.push(retired(methods[methodIndex], "method"))
    }
    return {
        ownerPath: "metrics",
        retainedMembers: [],
        retainedDecision: "No AppModel metrics compatibility member has a current consumer.",
        retiredMembers: retiredMembers,
        requiredFacadeProperties: properties,
        requiredFacadeMethods: methods.map(ownerMember)
    }
}

function retired(name, kind) {
    const consumers = formerConsumers(name)
    return {
        name: name,
        kind: kind,
        ownerPath: "metrics",
        ownerMember: ownerMember(name),
        formerConsumers: consumers,
        reason: consumers.length > 0
            ? "Consumers moved to the focused MetricsState facade."
            : "No current caller requires an AppModel compatibility member."
    }
}

function ownerMember(name) {
    return name === "updateDashboardCache" ? "cacheResponseValue" : name
}

function formerConsumers(name) {
    const consumers = ({
        blockchainRefreshRate: ["tst_app_model.qml"],
        messagingRefreshRate: ["tst_app_model.qml"],
        storageRefreshRate: ["tst_app_model.qml"],
        networkConnectionStatus: ["tst_app_model.qml", "tst_source_inspection_session.qml"],
        networkConnectionStatusRevision: ["tst_app_model.qml", "tst_source_inspection_session.qml"],
        footerFieldSelections: ["tst_app_model.qml"],
        dashboardGraphSelections: ["tst_app_model.qml"],
        dashboardMetricHistory: ["tst_app_model.qml"],
        dashboardMetricLastSeen: ["tst_app_model.qml"],
        dashboardMetricHistoryRevision: ["tst_app_model.qml"],
        networkConnectionPending: ["tst_app_model.qml", "tst_source_inspection_session.qml"],
        networkConnectionPendingRevision: ["tst_source_inspection_session.qml"],
        blockchainModuleReport: ["tst_app_model.qml"],
        storageModuleReport: ["tst_app_model.qml", "tst_source_inspection_session.qml"],
        messagingModuleReport: ["tst_app_model.qml", "tst_source_inspection_session.qml"],
        storageSourceReport: ["tst_app_model.qml", "tst_source_inspection_session.qml"],
        messagingSourceReport: ["tst_app_model.qml", "tst_source_inspection_session.qml"],
        queryNetworkConnection: ["tst_app_model.qml"],
        networkConnectionIsPending: ["tst_app_model.qml"],
        networkConnectionSummary: ["tst_app_model.qml"],
        storageReportReady: ["tst_app_model.qml"],
        moduleReportReachable: ["tst_app_model.qml"],
        sourceCapabilityAvailable: ["tst_app_model.qml"],
        sourceCapabilityEvidence: ["tst_app_model.qml"],
        deliveryReportHealthy: ["tst_app_model.qml"],
        scalarValue: ["AppModel dashboard gateway", "AppModelIdentity.js", "ModuleReportPresentation.js"],
        valueText: ["AppModel storage gateway", "AppModelIdentity.js", "BlockchainModuleEvents.js"],
        valueToString: ["AppModel chain gateway", "AccountDecodeSection.qml", "EntityTargetOpening.js"],
        moduleProbe: ["AppModelIdentity.js"],
        moduleProbeError: ["tst_app_model.qml"],
        moduleLastError: ["AppModelIdentity.js"],
        cryptarchiaValue: ["tst_app_model.qml"],
        networkValue: ["tst_app_model.qml"],
        dashboardMetricRawValue: ["tst_app_model.qml"],
        recordDashboardSnapshot: ["tst_app_model.qml"],
        refreshDashboard: ["tst_app_model.qml"],
        clearDashboardMetricHistoryForPrefix: ["tst_app_model.qml"]
    })
    return consumers[name] || []
}
