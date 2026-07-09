import QtQml
import "source_routing/SourceDiagnosticsProjection.js" as SourceDiagnostics

QtObject {
    id: root

    required property var model
    property var theme: null
    property string family: ""

    function networkKind() {
        return family === "storage" ? "storage" : "messaging"
    }

    function moduleFamily() {
        return family === "storage" ? "storage" : "messaging"
    }

    function report() {
        return model.moduleReport(moduleFamily())
    }

    function status() {
        return model.networkConnectionState(networkKind())
    }

    function pending() {
        return model.networkConnectionIsPending(networkKind())
    }

    function refresh(showResult, includeCidProbe) {
        return model.queryNetworkConnection(networkKind(), showResult === true, includeCidProbe === true)
    }

    function sourceLabel() {
        return family === "storage" ? model.storageSourceLabel() : model.deliverySourceLabel()
    }

    function sourceTarget() {
        return family === "storage" ? model.storageSourceTarget() : model.deliverySourceTarget()
    }

    function sourceMode() {
        return family === "storage" ? model.storageSource.resolvedMode : model.deliverySource.resolvedMode
    }

    function probeValue(method) {
        return SourceDiagnostics.probeValue(model, moduleFamily(), report(), method)
    }

    function probe(method) {
        return SourceDiagnostics.probe(model, moduleFamily(), report(), method)
    }

    function probeKnown(method) {
        return SourceDiagnostics.probeKnown(root, moduleFamily(), method)
    }

    function metricDisplay(key) {
        return SourceDiagnostics.metricDisplay(model, key)
    }

    function metricKnown(key) {
        return SourceDiagnostics.metricKnown(model, key)
    }

    function metricTone(key) {
        return SourceDiagnostics.metricTone(theme, model, key)
    }

    function failedProbeCount() {
        return SourceDiagnostics.failedProbeCount(report())
    }

    function diagnosticsGateDetailText(gate, fallbackLabel) {
        return SourceDiagnostics.diagnosticsGateDetailText(gate, fallbackLabel)
    }

    function statusLine() {
        return SourceDiagnostics.statusLine(root)
    }

    function freshnessText() {
        return SourceDiagnostics.freshnessText(root)
    }

    function freshnessCompactText() {
        return SourceDiagnostics.freshnessCompactText(root)
    }

    function sourceBadges(preset, windowText) {
        return SourceDiagnostics.sourceBadges(root, preset, windowText)
    }

    function moduleInfoProbe() {
        return SourceDiagnostics.moduleInfoProbe(report())
    }

    function sourceFactAvailable(key) {
        return SourceDiagnostics.sourceFactAvailable(model, report(), key)
    }

    function sourceFactEvidence(key, fallback) {
        return SourceDiagnostics.sourceFactEvidence(model, report(), key, fallback)
    }

    function evidenceRows(page, emptyMessage) {
        return SourceDiagnostics.evidenceRows(page, emptyMessage)
    }

    function statusRow(label, state, evidence, tone) {
        return SourceDiagnostics.statusRow(root, label, state, evidence, tone)
    }

    function probeRow(probe, fallbackLabel) {
        return SourceDiagnostics.probeRow(root, probe, fallbackLabel)
    }

    function detailRow(label, value, extraCopySkips) {
        return SourceDiagnostics.detailRow(root, label, value, extraCopySkips)
    }

    function valueSummary(value) {
        return SourceDiagnostics.valueSummary(value)
    }

    function copyValue(value) {
        return SourceDiagnostics.copyValue(value)
    }

    function shortText(value, maxLength) {
        return SourceDiagnostics.shortText(value, maxLength)
    }
}
