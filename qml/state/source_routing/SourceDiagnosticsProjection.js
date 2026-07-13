.import "InspectionFacts.js" as InspectionFacts
.import "../../utils/UiFormat.js" as UiFormat

function probe(model, family, report, method) {
    return model.sourceProbeFact(report, method) || model.moduleProbe(family, method)
}

function probeValue(model, family, report, method) {
    const value = model.reportProbeValue(report, method)
    return value !== null ? value : model.metrics.moduleProbeValue(family, method)
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
    const value = model.metrics.dashboardMetricValue(key)
    return value === null || value === undefined ? qsTr("n/a") : model.valueText(value)
}

function metricKnown(model, key) {
    const value = model.metrics.dashboardMetricValue(key)
    return value !== null && value !== undefined
}

function metricTone(theme, model, key) {
    const value = Number(model.metrics.dashboardMetricValue(key))
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

function diagnosticsGateDetailText(gate, fallbackLabel) {
    const missing = gate && Array.isArray(gate.missing) ? gate.missing : []
    if (missing.length > 0) {
        const first = missing[0] || {}
        const dependency = String(first.dependency || first.capability || "")
        const label = String(first.label || fallbackLabel || qsTr("Diagnostics"))
        return dependency.length ? qsTr("%1 unavailable: %2").arg(label).arg(dependency) : qsTr("%1 unavailable").arg(label)
    }
    return qsTr("%1 unavailable").arg(String(fallbackLabel || qsTr("Diagnostics")))
}

function statusLine(session) {
    if (session.pending()) {
        return qsTr("Refreshing %1").arg(session.sourceLabel())
    }
    const status = session.status()
    if (!status.known) {
        return qsTr("Not queried")
    }
    return qsTr("%1, checked %2").arg(status.detail || status.text).arg(status.checkedAt || qsTr("now"))
}

function freshnessText(session) {
    const status = session.status()
    if (!status.known) {
        return qsTr("No source check")
    }
    return status.checkedAt && status.checkedAt.length ? qsTr("Updated %1").arg(status.checkedAt) : qsTr("Updated")
}

function freshnessCompactText(session) {
    const status = session.status()
    if (!status.known) {
        return qsTr("not queried")
    }
    return status.checkedAt && status.checkedAt.length ? status.checkedAt : qsTr("updated")
}

function sourceBadges(session, preset, windowText) {
    const rows = [
        session.sourceLabel(),
        shortText(session.sourceTarget(), 42),
        String(preset || ""),
        windowText
    ]
    const status = session.status()
    rows.push(status.known ? freshnessText(session) : qsTr("not queried"))
    rows.push(status.known ? (status.ok ? qsTr("reachable") : qsTr("problem")) : qsTr("unknown"))
    return rows
}

function moduleInfoProbe(report) {
    return report && report.module_info ? report.module_info : null
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

function statusRow(session, label, state, evidence, tone) {
    return {
        label: label,
        state: state,
        evidence: evidence,
        source: session.sourceLabel(),
        freshness: freshnessCompactText(session),
        tone: tone
    }
}

function probeRow(session, probe, fallbackLabel) {
    const ok = probe && probe.ok === true
    return statusRow(
        session,
        String(probe && probe.label ? probe.label : fallbackLabel),
        ok ? qsTr("ok") : qsTr("problem"),
        ok ? valueSummary(probe.value) : String(probe && probe.error ? probe.error : qsTr("No response")),
        ok ? "success" : "error"
    )
}

function detailRow(session, label, value, extraCopySkips) {
    const text = valueSummary(value)
    const skips = {
        "-": true
    }
    skips[qsTr("n/a")] = true
    const extra = Array.isArray(extraCopySkips) ? extraCopySkips : []
    for (let i = 0; i < extra.length; ++i) {
        skips[String(extra[i])] = true
    }
    return {
        label: label,
        value: text,
        copyText: skips[text] === true ? "" : copyValue(value),
        source: session.sourceLabel()
    }
}

function valueSummary(value) {
    return UiFormat.valueSummary(value, {
        emptyText: qsTr("n/a"),
        emptyArrayText: qsTr("empty"),
        shortArrayLimit: 3,
        unwrapKeys: ["result", "value"],
        objectSummary: "fields"
    })
}

function copyValue(value) {
    return UiFormat.copyValue(value)
}

function shortText(value, maxLength) {
    return UiFormat.shortText(value, {
        emptyText: qsTr("n/a"),
        limit: maxLength || 32,
        minimum: 12,
        tailLength: 5
    })
}
