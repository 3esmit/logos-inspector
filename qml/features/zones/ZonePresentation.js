.pragma library

function text(value, fallback) {
    if (value === undefined || value === null || String(value).length === 0) {
        return fallback === undefined ? "-" : String(fallback)
    }
    return String(value)
}

function numberText(value) {
    if (value === undefined || value === null || value === "") {
        return "-"
    }
    return Number(value).toLocaleString(Qt.locale(), "f", 0)
}

function words(value) {
    const source = String(value || "")
    if (source.length === 0) {
        return "-"
    }
    return source.split("_").map(function (part) {
        return part.length > 0 ? part.charAt(0).toUpperCase() + part.slice(1) : part
    }).join(" ")
}

function kindLabel(kind) {
    switch (String(kind || "unknown")) {
    case "sequencer_zone":
        return qsTr("Sequencer Zone")
    case "data_channel":
        return qsTr("Data Channel")
    default:
        return qsTr("Unknown")
    }
}

function stateTone(zone, stale) {
    if (stale) {
        return "neutral"
    }
    const activity = String(zone && zone.activity_state || "unknown")
    switch (activity) {
    case "active":
        return "success"
    case "raw":
        return "info"
    case "idle":
    case "clock_only":
        return "neutral"
    case "finalizing":
        return "warning"
    case "degraded":
        return "error"
    default:
        return "warning"
    }
}

function finalityTone(value) {
    switch (String(value || "unknown")) {
    case "final":
        return "success"
    case "safe":
        return "info"
    case "finalizing":
    case "pending":
        return "warning"
    default:
        return "neutral"
    }
}

function activityLabel(zone) {
    return String(zone && zone.kind || "") === "data_channel"
        ? qsTr("Raw inscriptions")
        : qsTr("L2 head")
}

function activityValue(zone) {
    if (!zone) {
        return "-"
    }
    if (String(zone.kind || "") === "data_channel") {
        return numberText(zone.raw_activity && zone.raw_activity.inscription_count)
    }
    return numberText(zone.l2_zone && zone.l2_zone.latest_block_id)
}

function zoneFinality(zone) {
    if (!zone) {
        return "-"
    }
    if (String(zone.kind || "") === "data_channel" && zone.raw_activity) {
        return words(zone.raw_activity.finality_state)
    }
    if (String(zone.kind || "") === "sequencer_zone" && zone.l2_zone
            && String(zone.l2_zone.finality_state || "unknown") !== "unknown") {
        return words(zone.l2_zone.finality_state)
    }
    return words(zone.l1_channel && zone.l1_channel.finality_state)
}

function title(zone) {
    return text(zone && zone.display && zone.display.title, qsTr("Unnamed Zone"))
}

function alias(zone) {
    return text(zone && zone.display && zone.display.alias, "")
}

function filterRows(rows, filter, query, stale) {
    const source = Array.isArray(rows) ? rows : []
    const needle = String(query || "").trim().toLowerCase()
    return source.filter(function (zone) {
        const kind = String(zone && zone.kind || "unknown")
        const tone = stateTone(zone, stale)
        const filterMatches = filter === "all"
            || filter === "sequencer" && kind === "sequencer_zone"
            || filter === "data" && kind === "data_channel"
            || filter === "attention" && (tone === "warning" || tone === "error")
        if (!filterMatches) {
            return false
        }
        if (needle.length === 0) {
            return true
        }
        return String(zone.channel_id || "").toLowerCase().indexOf(needle) >= 0
            || title(zone).toLowerCase().indexOf(needle) >= 0
            || alias(zone).toLowerCase().indexOf(needle) >= 0
    })
}

function targetText(target) {
    if (!target) {
        return "-"
    }
    return String(target.kind || "") === "module"
        ? text(target.module_id)
        : text(target.endpoint)
}

function remoteInsecureHttp(value) {
    const endpoint = String(value || "").trim().toLowerCase()
    if (endpoint.indexOf("http://") !== 0) {
        return false
    }
    let authority = endpoint.substring(7)
    const pathStart = authority.search(/[/?#]/)
    if (pathStart >= 0) {
        authority = authority.substring(0, pathStart)
    }
    const credentialsEnd = authority.lastIndexOf("@")
    if (credentialsEnd >= 0) {
        authority = authority.substring(credentialsEnd + 1)
    }
    let host = authority
    if (authority.indexOf("[") === 0) {
        const bracketEnd = authority.indexOf("]")
        host = bracketEnd >= 0 ? authority.substring(1, bracketEnd) : authority
    } else {
        const portStart = authority.lastIndexOf(":")
        if (portStart >= 0) {
            host = authority.substring(0, portStart)
        }
    }
    const localhost = host === "localhost"
        || host.length > 10 && host.substring(host.length - 10) === ".localhost"
    const ipv4Loopback = /^127(?:\.\d{1,3}){3}$/.test(host)
    return !localhost && !ipv4Loopback && host !== "::1"
}

function observationFor(observations, sourceId) {
    const source = Array.isArray(observations) ? observations : []
    for (let i = 0; i < source.length; ++i) {
        if (String(source[i] && source[i].source_id || "") === String(sourceId || "")) {
            return source[i]
        }
    }
    return null
}

function bindingState(configured, observation) {
    return String(observation && observation.binding_state
        || configured && configured.binding_state
        || "pending")
}

function sourceTone(configured, observation) {
    const binding = bindingState(configured, observation)
    const health = String(observation && observation.health || "unknown")
    if (binding === "channel_mismatch" || health === "channel_mismatch") {
        return "error"
    }
    if (health === "reachable") {
        return "success"
    }
    if (health === "unreachable") {
        return "error"
    }
    if (health === "stale" || binding === "pending") {
        return "warning"
    }
    return "neutral"
}

function catalogTone(verification, coverage) {
    if (String(verification || "") === "mismatch") {
        return "error"
    }
    if (String(verification || "") !== "verified") {
        return "warning"
    }
    switch (String(coverage && coverage.status || "unknown")) {
    case "complete":
        return "success"
    case "partial":
        return "warning"
    case "rebuilding":
        return "info"
    default:
        return "neutral"
    }
}

function statusItems(zone, stale) {
    if (!zone) {
        return []
    }
    const l2 = zone.l2_zone || ({})
    const link = zone.settlement_link || ({})
    const l1 = zone.l1_channel || ({})
    return [{
        label: qsTr("L2 Sequencer"),
        value: String(zone.kind || "") === "sequencer_zone"
            ? numberText(l2.latest_block_id)
            : qsTr("Not applicable"),
        detail: String(zone.kind || "") === "sequencer_zone"
            ? words(l2.source_status)
            : kindLabel(zone.kind),
        tone: String(zone.kind || "") === "sequencer_zone"
            ? stateTone(zone, stale)
            : "neutral"
    }, {
        label: qsTr("Settlement Link"),
        value: words(link.status),
        detail: words(link.source),
        tone: link.status === "linked" ? "success"
            : (link.status === "raw_data" ? "info" : "warning")
    }, {
        label: qsTr("L1 Channel"),
        value: numberText(l1.tip_slot),
        detail: words(l1.finality_state),
        tone: finalityTone(l1.finality_state)
    }, {
        label: qsTr("Activity"),
        value: words(zone.activity_state),
        detail: String(zone.kind || "") === "data_channel"
            ? qsTr("Raw L1 data")
            : qsTr("Channel stream"),
        tone: stateTone(zone, stale)
    }]
}

function evidenceKindLabel(kind) {
    switch (String(kind || "")) {
    case "channel_created":
        return qsTr("Channel created")
    case "channel_configuration":
        return qsTr("Configuration")
    case "channel_operation":
        return qsTr("Channel operation")
    case "sequencer_block":
        return qsTr("Sequencer block")
    case "raw_inscription":
        return qsTr("Raw inscription")
    default:
        return words(kind)
    }
}
