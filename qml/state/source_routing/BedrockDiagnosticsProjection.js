.import "SourceDiagnosticsProjection.js" as SourceDiagnostics

const probeDefinitions = [
    { key: "cryptarchia_info", label: qsTr("Cryptarchia information") },
    { key: "headers", label: qsTr("Block headers") },
    { key: "network_info", label: qsTr("Network information") },
    { key: "mantle_metrics", label: qsTr("Mantle metrics") }
]

function build(model, theme) {
    const observation = model && model.metrics
        ? model.metrics.sourceObservation("blockchain") : ({})
    const report = observation.sourceReport || null
    const status = observation.status && typeof observation.status === "object"
        ? observation.status : ({ known: false, ok: false })
    const pending = observation.pending === true
    const route = model && model.sourceRouting
        ? (model.sourceRouting.coreSourceView("blockchain") || ({})) : ({})
    const sourceLabel = String(route.label || qsTr("Bedrock"))
    const sourceTarget = String(route.target || (report && report.endpoint) || "")
    const probes = probeSnapshot(report)
    const sourceState = connectionState(status, pending, probes)
    const reportCheckedAt = String(observation.reportCheckedAt || "")

    return {
        report: report,
        status: status,
        pending: pending,
        route: route,
        sourceLabel: sourceLabel,
        sourceTarget: sourceTarget,
        sourceShortLabel: sourceShortLabel(route),
        sourceTargetShort: SourceDiagnostics.shortText(sourceTarget, 34),
        statusLine: statusLine(status, pending, sourceLabel, reportCheckedAt),
        freshnessText: freshnessText(status, reportCheckedAt),
        sourceState: sourceState,
        sourceStateTone: sourceStateTone(sourceState),
        checksText: qsTr("%1/%2 available").arg(probes.available).arg(probes.total),
        checksDetail: checksDetail(probes),
        checksTone: checksTone(probes),
        sourceBadges: sourceBadges(sourceLabel, sourceTarget, status, pending,
            reportCheckedAt),
        sourceRows: sourceRows(route, sourceLabel, sourceTarget, status,
            pending, reportCheckedAt),
        probeRows: probeRows(report, sourceLabel, status, pending,
            reportCheckedAt),
        consensusRows: consensusRows(report, sourceLabel),
        notice: notice(status, pending, probes, reportCheckedAt)
    }
}

function probeSnapshot(report) {
    let available = 0
    let unsupported = 0
    let failed = 0
    for (let index = 0; index < probeDefinitions.length; ++index) {
        const probe = probeFor(report, probeDefinitions[index].key)
        if (probe && probe.ok === true) {
            available += 1
        } else if (isUnsupported(probe)) {
            unsupported += 1
        } else if (probe) {
            failed += 1
        }
    }
    return {
        total: probeDefinitions.length,
        available: available,
        unsupported: unsupported,
        failed: failed
    }
}

function connectionState(status, pending, probes) {
    if (pending) {
        return qsTr("Working")
    }
    if (!status.known) {
        return qsTr("Not queried")
    }
    if (status.stale === true) {
        return qsTr("Last known")
    }
    if (status.ok === true && probes.failed === 0) {
        return qsTr("Reachable")
    }
    if (probes.available > 0) {
        return qsTr("Degraded")
    }
    return qsTr("Unavailable")
}

function sourceStateTone(state) {
    if (state === qsTr("Reachable")) {
        return "success"
    }
    if (state === qsTr("Working") || state === qsTr("Not queried")) {
        return "neutral"
    }
    if (state === qsTr("Last known") || state === qsTr("Degraded")) {
        return "warning"
    }
    return "error"
}

function checksDetail(probes) {
    if (probes.failed > 0) {
        return qsTr("%1 failed").arg(probes.failed)
    }
    if (probes.unsupported > 0) {
        return qsTr("%1 unavailable on this connector").arg(probes.unsupported)
    }
    if (probes.available === 0) {
        return qsTr("No probe evidence")
    }
    return qsTr("All reported checks available")
}

function checksTone(probes) {
    if (probes.failed > 0) {
        return "error"
    }
    if (probes.unsupported > 0) {
        return "warning"
    }
    return probes.available > 0 ? "success" : "neutral"
}

function sourceBadges(label, target, status, pending, reportCheckedAt) {
    const rows = [
        label,
        SourceDiagnostics.shortText(target, 42),
        freshnessText(status, reportCheckedAt)
    ]
    if (pending) {
        rows.push(qsTr("refreshing"))
    } else if (!status.known) {
        rows.push(qsTr("not queried"))
    } else if (status.stale === true) {
        rows.push(qsTr("last known"))
    } else {
        rows.push(status.ok === true ? qsTr("reachable") : qsTr("problem"))
    }
    return rows
}

function sourceRows(route, sourceLabel, sourceTarget, status, pending,
        reportCheckedAt) {
    const target = sourceTarget.length > 0 ? sourceTarget
        : qsTr("No target configured")
    const latest = pending ? qsTr("Refreshing")
        : (!status.known ? qsTr("Not queried")
            : String(status.detail || status.text || qsTr("Completed")))
    const completed = reportCheckedAt.length > 0 ? reportCheckedAt
        : qsTr("No completed report")
    return [
        detailRow(qsTr("Connection"), sourceLabel, "", qsTr("Current source")),
        detailRow(qsTr("Target"), target,
            sourceTarget.length > 0 ? sourceTarget : "", sourceLabel),
        detailRow(qsTr("Transport"), sourceTransportLabel(route), "",
            qsTr("Current source")),
        detailRow(qsTr("Latest check"), latest, "", sourceLabel),
        detailRow(qsTr("Last completed report"), completed, "", sourceLabel)
    ]
}

function sourceTransportLabel(route) {
    const mode = String(route && (route.mode || route.configuredMode
        || route.effectiveMode) || "")
        .toLowerCase()
    if (mode === "rpc") {
        return qsTr("Direct Bedrock RPC")
    }
    if (mode === "module") {
        return qsTr("LogosCore module")
    }
    if (mode === "logoscore_cli" || mode === "logoscore-cli") {
        return qsTr("LogosCore CLI")
    }
    return qsTr("Configured Bedrock source")
}

function sourceShortLabel(route) {
    const mode = String(route && (route.mode || route.configuredMode
        || route.effectiveMode) || "")
        .toLowerCase()
    if (mode === "rpc") {
        return qsTr("RPC")
    }
    if (mode === "module") {
        return qsTr("Module")
    }
    if (mode === "logoscore_cli" || mode === "logoscore-cli") {
        return qsTr("CLI")
    }
    return qsTr("Source")
}

function probeRows(report, sourceLabel, status, pending, reportCheckedAt) {
    const rows = []
    const freshness = freshnessCompactText(status, pending, reportCheckedAt)
    for (let index = 0; index < probeDefinitions.length; ++index) {
        const definition = probeDefinitions[index]
        const probe = probeFor(report, definition.key)
        rows.push({
            label: definition.label,
            state: probeState(probe),
            evidence: probeEvidence(probe),
            source: sourceLabel,
            freshness: freshness,
            tone: probeTone(probe)
        })
    }
    return rows
}

function consensusRows(report, sourceLabel) {
    const info = cryptarchiaInfo(report)
    if (!info) {
        return [detailRow(qsTr("Consensus information"), qsTr("Not available"), "",
            sourceLabel)]
    }
    return [
        consensusRow(qsTr("Node mode"), info.mode, sourceLabel),
        consensusRow(qsTr("Observed tip slot"), firstValue(info, ["slot", "height"]),
            sourceLabel),
        consensusRow(qsTr("Observed LIB slot"), info.lib_slot, sourceLabel),
        consensusRow(qsTr("Tip identifier"), firstValue(info, ["tip", "tip_hash"]),
            sourceLabel),
        consensusRow(qsTr("LIB identifier"), firstValue(info, ["lib", "lib_hash"]),
            sourceLabel)
    ]
}

function consensusRow(label, value, sourceLabel) {
    const present = value !== undefined && value !== null && String(value).length > 0
    const text = present ? String(value) : qsTr("Not reported")
    return detailRow(label, text, present ? String(value) : "", sourceLabel)
}

function detailRow(label, value, copyText, source) {
    return {
        label: label,
        value: value,
        copyText: copyText,
        source: source
    }
}

function cryptarchiaInfo(report) {
    const probe = probeFor(report, "cryptarchia_info")
    if (!probe || probe.ok !== true || !probe.value
            || typeof probe.value !== "object") {
        return null
    }
    const nested = probe.value.cryptarchia_info
    return nested && typeof nested === "object" ? nested : probe.value
}

function firstValue(value, keys) {
    const source = value && typeof value === "object" ? value : ({})
    for (let index = 0; index < keys.length; ++index) {
        const candidate = source[keys[index]]
        if (candidate !== undefined && candidate !== null && String(candidate).length > 0) {
            return candidate
        }
    }
    return null
}

function probeFor(report, key) {
    const value = report && report[key]
    return value && typeof value === "object" ? value : null
}

function probeState(probe) {
    if (!probe) {
        return qsTr("not reported")
    }
    if (probe.ok === true) {
        return qsTr("available")
    }
    return isUnsupported(probe) ? qsTr("unavailable") : qsTr("problem")
}

function probeTone(probe) {
    if (!probe) {
        return "neutral"
    }
    if (probe.ok === true) {
        return "success"
    }
    return isUnsupported(probe) ? "warning" : "error"
}

function probeEvidence(probe) {
    if (!probe) {
        return qsTr("No response data")
    }
    if (probe.ok !== true) {
        return String(probe.error || qsTr("No response"))
    }
    const source = String(probe.source || "")
    const summary = SourceDiagnostics.valueSummary(probe.value)
    return source.length > 0 ? qsTr("%1: %2").arg(source).arg(summary) : summary
}

function isUnsupported(probe) {
    if (!probe || probe.ok === true) {
        return false
    }
    const error = String(probe.error || "").toLowerCase()
    return error.indexOf("does not expose") >= 0
        || error.indexOf("not supported") >= 0
        || error.indexOf("unsupported") >= 0
}

function statusLine(status, pending, sourceLabel, reportCheckedAt) {
    if (pending) {
        return qsTr("Refreshing %1").arg(sourceLabel)
    }
    if (!status.known) {
        return qsTr("Not queried")
    }
    if (status.stale === true) {
        return qsTr("Latest check failed %1; showing last completed report from %2")
            .arg(String(status.checkedAt || qsTr("at an unknown time")))
            .arg(reportCheckedAt.length > 0 ? reportCheckedAt : qsTr("an earlier check"))
    }
    return qsTr("%1, checked %2")
        .arg(String(status.detail || status.text || qsTr("Completed")))
        .arg(String(status.checkedAt || qsTr("now")))
}

function freshnessText(status, reportCheckedAt) {
    if (!status.known) {
        return qsTr("No source check")
    }
    if (status.stale === true) {
        return reportCheckedAt.length > 0
            ? qsTr("Last known %1").arg(reportCheckedAt) : qsTr("Last known report")
    }
    return status.checkedAt && String(status.checkedAt).length > 0
        ? qsTr("Updated %1").arg(status.checkedAt) : qsTr("Updated")
}

function freshnessCompactText(status, pending, reportCheckedAt) {
    if (pending) {
        return qsTr("refreshing")
    }
    if (!status.known) {
        return qsTr("not queried")
    }
    if (status.stale === true) {
        return reportCheckedAt.length > 0
            ? qsTr("last known %1").arg(reportCheckedAt) : qsTr("last known")
    }
    return status.checkedAt && String(status.checkedAt).length > 0
        ? String(status.checkedAt) : qsTr("updated")
}

function notice(status, pending, probes, reportCheckedAt) {
    if (pending) {
        return {
            tone: "info",
            title: qsTr("Checking Bedrock"),
            message: qsTr("Refreshing the configured source without changing node state.")
        }
    }
    if (!status.known) {
        return {
            tone: "info",
            title: qsTr("Connection check pending"),
            message: qsTr("A Bedrock source check starts automatically when this page opens.")
        }
    }
    if (status.stale === true) {
        return {
            tone: "warning",
            title: qsTr("Latest check failed"),
            message: reportCheckedAt.length > 0
                ? qsTr("Showing the last completed report from %1 while the source is retried.")
                    .arg(reportCheckedAt)
                : qsTr("The source did not return a new report.")
        }
    }
    if (probes.failed > 0) {
        return {
            tone: "warning",
            title: qsTr("Some Bedrock API checks failed"),
            message: qsTr("Review the affected check for its exact response error.")
        }
    }
    if (probes.unsupported > 0) {
        return {
            tone: "info",
            title: qsTr("Connector capability limits"),
            message: qsTr("Some read-only Bedrock APIs are unavailable through the selected connector.")
        }
    }
    if (status.ok === true) {
        return {
            tone: "success",
            title: qsTr("Bedrock source reachable"),
            message: qsTr("All reported connection checks completed successfully.")
        }
    }
    return {
        tone: "error",
        title: qsTr("Bedrock source unavailable"),
        message: String(status.detail || qsTr("The source did not provide Cryptarchia information."))
    }
}
