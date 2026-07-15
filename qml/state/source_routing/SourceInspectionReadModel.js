.import "SourceDiagnosticsProjection.js" as SourceDiagnostics
.import "SourceObservationProjection.js" as SourceObservation

function build(model, theme, family) {
    const storage = String(family || "") === "storage"
    const moduleFamily = storage ? "storage" : "messaging"
    const networkKind = storage ? "storage" : "messaging"
    const observation = model.metrics.sourceObservation(moduleFamily)
    const report = observation.sourceReport
    const status = observation.status || ({ known: false, ok: false })
    const pending = observation.pending === true
    const sourceName = storage ? model.sourceRouting.storageSourceLabel()
        : model.sourceRouting.deliverySourceLabel()
    const sourceTarget = storage ? model.sourceRouting.storageSourceTarget()
        : model.sourceRouting.deliverySourceTarget()
    const sourceMode = storage
        ? model.sourceRouting.effectiveStorageSourceMode(model.storageSourceMode)
        : model.sourceRouting.effectiveMessagingSourceMode(model.messagingSourceMode)

    const page = {
        model: model,
        theme: theme
    }
    page.report = function () { return report }
    page.status = function () { return status }
    page.pending = function () { return pending }
    page.reportCheckedAt = function () { return String(observation.reportCheckedAt || "") }
    page.reportCheckedAtMs = function () { return Number(observation.reportCheckedAtMs || 0) }
    page.sourceLabel = function () { return sourceName }
    page.sourceName = function () { return sourceName }
    page.sourceTarget = function () { return sourceTarget }
    page.probe = function (method) {
        return SourceDiagnostics.probe(model, moduleFamily, report, method)
    }
    page.probeValue = function (method) {
        return SourceDiagnostics.probeValue(model, moduleFamily, report, method)
    }
    page.probeKnown = function (method) {
        return SourceDiagnostics.probeKnown(page, moduleFamily, method)
    }
    page.metricDisplay = function (key) {
        return SourceDiagnostics.metricDisplay(model, key)
    }
    page.metricKnown = function (key) {
        return SourceDiagnostics.metricKnown(model, key)
    }
    page.failedProbeCount = function () {
        return SourceDiagnostics.failedProbeCount(report)
    }
    page.sourceFactAvailable = function (key) {
        return SourceDiagnostics.sourceFactAvailable(model, report, key)
    }
    page.sourceFactEvidence = function (key, fallback) {
        return SourceDiagnostics.sourceFactEvidence(model, report, key, fallback)
    }
    page.statusTone = function () {
        if (!status.known) {
            return "neutral"
        }
        return status.ok ? "success" : "error"
    }
    page.statusRow = function (label, state, evidence, tone) {
        return SourceDiagnostics.statusRow(page, label, state, evidence, tone)
    }
    page.probeRow = function (probe, fallbackLabel) {
        return SourceDiagnostics.probeRow(page, probe, fallbackLabel)
    }
    page.detailRow = function (label, value) {
        const skips = storage ? [qsTr("Not queried"), qsTr("Not fetched"), qsTr("Idle")] : []
        return SourceDiagnostics.detailRow(page, label, value, skips)
    }
    page.valueSummary = function (value) {
        return SourceDiagnostics.valueSummary(value)
    }
    page.copyValue = function (value) {
        return SourceDiagnostics.copyValue(value)
    }
    page.shortText = function (value, maxLength) {
        return SourceDiagnostics.shortText(value, maxLength)
    }
    page.moduleInfoProbe = function () {
        return SourceDiagnostics.moduleInfoProbe(report)
    }

    const common = commonView(page, theme, storage, sourceMode, sourceTarget)
    return storage ? storageView(page, common) : deliveryView(page, common)
}

function commonView(page, theme, storage, sourceMode, sourceTarget) {
    const status = page.status()
    const preset = storage ? page.model.storageNetworkPreset
        : page.model.normalizedMessagingNetworkPreset(page.model.messagingNetworkPreset)
    const windowSeconds = storage ? page.model.storageRollingWindow : page.model.messagingRollingWindow
    let sourceShortLabel = qsTr("Module")
    if (sourceMode === "rest") {
        sourceShortLabel = qsTr("REST")
    } else if (sourceMode === "metrics") {
        sourceShortLabel = qsTr("Metrics")
    } else if (sourceMode === "network-monitor") {
        sourceShortLabel = qsTr("Monitor")
    } else if (sourceMode === "unsupported") {
        sourceShortLabel = qsTr("Unsupported")
    }

    let healthText = qsTr("Healthy")
    if (page.pending()) {
        healthText = qsTr("Working")
    } else if (!status.known) {
        healthText = qsTr("Unknown")
    } else if (!status.ok) {
        healthText = qsTr("Problem")
    } else if (page.failedProbeCount() > 0) {
        healthText = qsTr("Partial")
    }

    return {
        report: page.report(),
        status: status,
        pending: page.pending(),
        statusLine: SourceDiagnostics.statusLine(page),
        statusColor: !status.known ? theme.textMuted : (status.ok ? theme.success : theme.error),
        statusTone: page.statusTone(),
        healthText: healthText,
        sourceName: page.sourceName(),
        sourceShortLabel: sourceShortLabel,
        sourceTarget: sourceTarget,
        sourceTargetShort: page.shortText(sourceTarget, 34),
        freshnessText: SourceDiagnostics.freshnessText(page),
        freshnessCompactText: SourceDiagnostics.freshnessCompactText(page),
        sourceBadges: SourceDiagnostics.sourceBadges(page, preset, qsTr("%1 s window").arg(windowSeconds)),
        moduleInfoProbe: page.moduleInfoProbe(),
        failedProbeCount: page.failedProbeCount(),
        evidenceRows: SourceDiagnostics.evidenceRows(page, qsTr("Refresh source to load probe evidence."))
    }
}

function storageView(page, common) {
    page.identityEvidence = function () {
        return SourceObservation.storageIdentityEvidence(page)
    }
    page.capacitySummary = function () {
        return SourceObservation.storageCapacitySummary(page)
    }
    page.transferSummary = function () {
        return SourceObservation.storageTransferSummary(page)
    }
    page.metricEvidence = function (key) {
        return SourceObservation.storageMetricEvidence(page, key)
    }
    page.metricRow = function (label, key) {
        return SourceObservation.storageMetricRow(page, label, key)
    }
    page.activeStorageOperation = function () {
        const app = page.model.storageApp
        return app && app.operation ? app.operation.active : null
    }
    page.activeStorageOperationDetail = function (operation) {
        return SourceObservation.storageActiveStorageOperationDetail(page, operation)
    }
    page.activeDownloadRow = function () {
        return SourceObservation.storageActiveDownloadRow(page)
    }
    page.manifestCountRow = function () {
        return SourceObservation.storageManifestCountRow(page)
    }
    page.spaceRow = function (label, keys) {
        return SourceObservation.storageSpaceRow(page, label, keys)
    }
    page.protocolRow = function (label, protocolId, observed, evidence) {
        return SourceObservation.storageProtocolRow(label, protocolId, observed, evidence)
    }
    page.pathDetailRow = function (label, value) {
        return SourceObservation.storagePathDetailRow(page, label, value)
    }
    page.storageSourceMode = function () {
        return page.model.sourceRouting.effectiveStorageSourceMode(page.model.storageSourceMode)
    }
    page.metricsEndpointConfigured = function () {
        return String(page.model.storageMetricsUrl || "").trim().length > 0
    }
    page.restMetricsState = function () {
        return SourceObservation.storageRestMetricsState(page)
    }
    page.restMetricsEvidence = function () {
        return SourceObservation.storageRestMetricsEvidence(page)
    }
    page.restMetricsTone = function () {
        return SourceObservation.storageRestMetricsTone(page)
    }

    common.identityEvidence = page.identityEvidence()
    common.peerCount = page.metricDisplay("storage.peer_count")
    common.peerColor = SourceDiagnostics.metricTone(page.theme, page.model, "storage.peer_count")
    common.capacitySummary = page.capacitySummary()
    common.capacityEvidence = page.valueSummary(page.probeValue("space"))
    common.transferSummary = page.transferSummary()
    common.transferFailures = page.metricDisplay("storage.failed_transfers_recent")
    common.transferFailureColor = SourceObservation.storageTransferFailureTone(page)
    common.reliabilityText = SourceObservation.storageReliabilityText(page)
    common.reliabilityDetail = page.model.metrics.moduleReportError(page.report())
        || page.sourceName()
    common.reliabilityColor = SourceObservation.storageReliabilityTone(page)
    common.healthRows = SourceObservation.storageHealthRows(page)
    common.activeOperationRows = SourceObservation.storageActiveOperationRows(page)
    common.topologyRows = SourceObservation.storageTopologyRows(page)
    common.capacityRows = SourceObservation.storageCapacityRows(page)
    common.repositoryRows = SourceObservation.storageRepositoryRows(page)
    common.transferRows = SourceObservation.storageTransferRows(page)
    common.cidRows = SourceObservation.storageCidRows(page)
    common.protocolRows = SourceObservation.storageProtocolRows(page)
    common.identityRows = SourceObservation.storageIdentityRows(page)
    return common
}

function deliveryView(page, common) {
    page.identityValue = function (kind) {
        return SourceObservation.deliveryIdentityValue(page, kind)
    }
    page.identityEvidence = function () {
        return SourceObservation.deliveryIdentityEvidence(page)
    }
    page.sourceFactObservedState = function (key, fallbackKnown) {
        return SourceObservation.deliverySourceFactObservedState(page, key, fallbackKnown)
    }
    page.sourceFactObservedTone = function (key, fallbackKnown) {
        return SourceObservation.deliverySourceFactObservedTone(page, key, fallbackKnown)
    }
    page.metricEvidence = function (key) {
        return SourceObservation.deliveryMetricEvidence(page, key)
    }
    page.metricRow = function (label, key) {
        return SourceObservation.deliveryMetricRow(page, label, key)
    }
    page.protocolHealthDetail = function (protocol, description) {
        return SourceObservation.deliveryProtocolHealthDetail(page, protocol, description)
    }
    page.protocolHealthEntry = function (item) {
        return SourceObservation.deliveryProtocolHealthEntry(page, item)
    }
    page.protocolLabel = function (key) {
        return SourceObservation.deliveryProtocolLabel(key)
    }
    page.healthValueTone = function (value) {
        return SourceObservation.deliveryHealthValueTone(page, value)
    }
    page.combinedHealthTone = function (left, right) {
        return SourceObservation.deliveryCombinedHealthTone(page, left, right)
    }
    page.protocolHealthRows = function () {
        return SourceObservation.deliveryProtocolHealthRows(page)
    }
    page.protocolStatusRow = function (label, protocol, metricKey) {
        return SourceObservation.deliveryProtocolStatusRow(page, label, protocol, metricKey)
    }
    page.protocolRow = function (label, protocolId, signalKey) {
        return SourceObservation.deliveryProtocolRow(page, label, protocolId, signalKey)
    }
    page.deliverySourceMode = function () {
        return page.model.sourceRouting.effectiveMessagingSourceMode(page.model.messagingSourceMode)
    }
    page.moduleMetricsText = function () {
        return SourceObservation.deliveryModuleMetricsText(page)
    }
    page.restMetricsState = function () {
        return SourceObservation.deliveryRestMetricsState(page)
    }
    page.restMetricsEvidence = function () {
        return SourceObservation.deliveryRestMetricsEvidence(page)
    }
    page.restMetricsTone = function () {
        return SourceObservation.deliveryRestMetricsTone(page)
    }
    page.networkMonitorPeerCount = function () {
        return SourceObservation.deliveryNetworkMonitorPeerCount(page)
    }
    page.networkMonitorTopicCount = function () {
        return SourceObservation.deliveryNetworkMonitorTopicCount(page)
    }
    page.servicePeerCount = function () {
        return SourceObservation.deliveryServicePeerCount(page)
    }

    common.identityEvidence = page.identityEvidence()
    common.peerCount = page.metricDisplay("messaging.peer_count")
    common.peerColor = SourceDiagnostics.metricTone(page.theme, page.model, "messaging.peer_count")
    common.messageCount = page.metricDisplay("messaging.message_received_events_recent")
    common.errorCount = page.metricDisplay("messaging.message_error_events_recent")
    common.errorDetail = page.model.metrics.moduleReportError(page.report())
        || page.sourceName()
    common.errorColor = SourceDiagnostics.metricTone(page.theme, page.model, "messaging.message_error_events_recent")
    common.healthRows = SourceObservation.deliveryHealthRows(page)
    common.topologyRows = SourceObservation.deliveryTopologyRows(page)
    common.throughputRows = SourceObservation.deliveryThroughputRows(page)
    common.protocolRows = SourceObservation.deliveryProtocolRows(page)
    common.topicRows = SourceObservation.deliveryTopicRows(page)
    common.storeRows = SourceObservation.deliveryStoreRows(page)
    common.identityRows = SourceObservation.deliveryIdentityRows(page)
    return common
}
