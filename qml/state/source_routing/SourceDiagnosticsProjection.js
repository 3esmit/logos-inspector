.import "InspectionFacts.js" as InspectionFacts

function probe(model, family, report, method) {
    return model.sourceProbeFact(report, method) || model.moduleProbe(family, method)
}

function probeValue(model, family, report, method) {
    const value = model.reportProbeValue(report, method)
    return value !== null ? value : model.moduleProbeValue(family, method)
}

function probeKnown(page, family, method) {
    const item = probe(page.model, family, page.report(), method)
    if (!item || item.ok !== true || item.value === undefined || item.value === null) {
        return false
    }
    return !probeSkipped(item)
}

function probeSkipped(item) {
    const value = item && item.value
    return value && typeof value === "object" && value.skipped === true
}

function metricDisplay(model, key) {
    const value = model.dashboardMetricValue(key)
    return value === null || value === undefined ? qsTr("n/a") : model.valueText(value)
}

function metricKnown(model, key) {
    const value = model.dashboardMetricValue(key)
    return value !== null && value !== undefined
}

function metricTone(theme, model, key) {
    const value = Number(model.dashboardMetricValue(key))
    if (!Number.isFinite(value)) {
        return theme.textMuted
    }
    if (key.indexOf("failed") >= 0 || key.indexOf("error") >= 0) {
        return value > 0 ? theme.error : theme.success
    }
    return value > 0 ? theme.success : theme.textMuted
}

function failedProbeCount(report) {
    return InspectionFacts.failedProbeCount(report)
}

function sourceFactAvailable(model, report, key) {
    return model.sourceCapabilityAvailable(report, key) === true
}

function sourceFactEvidence(model, report, key, fallback) {
    const evidence = model.sourceCapabilityEvidence(report, key)
    return evidence.length > 0 ? evidence : fallback
}

function evidenceRows(page, emptyMessage) {
    const rows = []
    const info = page.moduleInfoProbe()
    if (info) {
        rows.push(page.probeRow(info, qsTr("Source check")))
    }
    const report = page.report()
    const probes = report && Array.isArray(report.probes) ? report.probes : []
    const facts = report && Array.isArray(report.probe_facts) ? report.probe_facts : []
    const evidence = probes.length > 0 ? probes : facts
    for (let i = 0; i < evidence.length; ++i) {
        rows.push(page.probeRow(evidence[i], qsTr("Probe")))
    }
    if (rows.length === 0) {
        rows.push(page.statusRow(qsTr("Probe evidence"), qsTr("empty"), emptyMessage, "neutral"))
    }
    return rows
}
