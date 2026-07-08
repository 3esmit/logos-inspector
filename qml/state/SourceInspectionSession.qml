import QtQml
import "network/SourceDiagnosticsProjection.js" as SourceDiagnostics

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
        return SourceDiagnostics.probeValue(model, moduleFamily(), method)
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

    function sourceFactAvailable(key) {
        return SourceDiagnostics.sourceFactAvailable(model, report(), key)
    }

    function sourceFactEvidence(key, fallback) {
        return SourceDiagnostics.sourceFactEvidence(model, report(), key, fallback)
    }

    function evidenceRows(page, emptyMessage) {
        return SourceDiagnostics.evidenceRows(page, emptyMessage)
    }
}
