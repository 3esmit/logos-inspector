.import "ModuleEventUtils.js" as ModuleEventUtils

function eventRows(root) {
    const revision = root.deliveryModuleEventRevision
    const rows = Array.isArray(root.deliveryModuleEvents) ? root.deliveryModuleEvents : []
    if (rows.length > 0) {
        return rows
    }
    return [{
        time: "-",
        label: qsTr("No module events"),
        status: "-",
        detail: "-"
    }]
}

function eventSummary(root) {
    const revision = root.deliveryModuleEventRevision
    if (root.deliveryConnectionStatus.length > 0) {
        return root.deliveryConnectionStatus
    }
    if (root.deliveryNodeStatus.length > 0) {
        return root.deliveryNodeStatus
    }
    return qsTr("No events")
}

function handle(root, eventName, args) {
    with (root) {
        const record = deliveryEventRecord(eventName, args)
        appendDeliveryEvent(root, record)
        if (eventName === "connectionStateChanged") {
            deliveryConnectionStatus = record.statusValue
            queryNetworkConnection("messaging", false)
        } else if (eventName === "nodeStarted" || eventName === "nodeStopped") {
            deliveryNodeStatus = record.statusValue
            queryNetworkConnection("messaging", false)
        } else if (eventName === "messageReceived") {
            applyDeliveryMessage(root, record)
        }
        return true
    }
}

function deliveryEventRecord(eventName, args) {
    const values = ModuleEventUtils.eventValues(args)
    const object = ModuleEventUtils.eventObject(args)
    const timestamp = ModuleEventUtils.fieldText(object, ["timestamp", "time"]) || String(values[3] || "")
    const topic = ModuleEventUtils.fieldText(object, ["contentTopic", "content_topic", "topic"]) || String(values[1] || "")
    const payload = object.payload !== undefined ? object.payload : values[2]
    const hash = ModuleEventUtils.fieldText(object, ["messageHash", "message_hash", "hash"]) || String(values[0] || "")
    const requestId = ModuleEventUtils.fieldText(object, ["requestId", "request_id"]) || String(values[0] || "")
    const error = ModuleEventUtils.fieldText(object, ["error", "message"]) || String(values[2] || "")
    let statusValue = ""
    if (eventName === "connectionStateChanged") {
        statusValue = ModuleEventUtils.fieldText(object, ["connectionStatus", "connection_status", "status"]) || String(values[0] || "")
    } else if (eventName === "nodeStarted" || eventName === "nodeStopped") {
        const ok = object.success !== undefined ? object.success : values[0]
        statusValue = (ok === true || String(ok).toLowerCase() === "true") ? qsTr("ok") : qsTr("error")
        const text = ModuleEventUtils.fieldText(object, ["message"]) || String(values[1] || "")
        if (text.length > 0) {
            statusValue += ": " + text
        }
    }
    return {
        eventName: eventName,
        time: ModuleEventUtils.eventTimeText(timestamp),
        contentTopic: topic,
        payload: payload,
        messageHash: hash,
        requestId: requestId,
        error: error,
        status: eventTone(eventName, error),
        statusValue: statusValue,
        detail: deliveryEventDetail(eventName, requestId, hash, topic, error, payload)
    }
}

function appendDeliveryEvent(root, record) {
    const rows = Array.isArray(root.deliveryModuleEvents) ? root.deliveryModuleEvents.slice(0) : []
    rows.unshift({
        time: record.time,
        label: String(record.eventName || ""),
        status: String(record.status || ""),
        detail: String(record.detail || "")
    })
    root.deliveryModuleEvents = rows.slice(0, 50)
    root.deliveryModuleEventRevision += 1
}

function deliveryEventDetail(eventName, requestId, hash, topic, error, payload) {
    if (eventName === "messageError") {
        return ModuleEventUtils.compactParts([requestId, ModuleEventUtils.shortText(hash, 20), error]).join(" / ")
    }
    if (eventName === "messageReceived") {
        return ModuleEventUtils.compactParts([ModuleEventUtils.shortText(topic, 44), ModuleEventUtils.shortText(hash, 20), ModuleEventUtils.payloadSummary(payload)]).join(" / ")
    }
    if (eventName === "connectionStateChanged") {
        return ModuleEventUtils.compactParts([requestId || hash]).join(" / ")
    }
    return ModuleEventUtils.compactParts([requestId, ModuleEventUtils.shortText(hash, 20), ModuleEventUtils.shortText(topic, 44)]).join(" / ")
}

function applyDeliveryMessage(root, record) {
    const payload = ModuleEventUtils.parsedPayload(record.payload)
    if (!payload || typeof payload !== "object") {
        return
    }
    if (String(payload.kind || "") !== "comment") {
        return
    }
    const topic = String(record.contentTopic || payload.conversation_id || "")
    if (!topic.length) {
        return
    }
    const current = root.socialCommentStateForTopic(topic)
    const row = {
        key: "event|" + String(record.messageHash || "") + "|" + String(payload.created_at || ""),
        cursor: "",
        topic: topic,
        identity: payload.identity || {},
        displayName: socialIdentityDisplayName(payload.identity),
        body: String(payload.body || ""),
        createdAt: String(payload.created_at || ""),
        conversationId: String(payload.conversation_id || topic)
    }
    root.setSocialCommentState(topic, {
        rows: root.mergeSocialCommentRows(current.rows || [], [row]),
        cursor: String(current.cursor || ""),
        loading: false,
        error: "",
        exhausted: current.exhausted === true
    })
}

function eventTone(eventName, error) {
    if (eventName === "messageError" || String(error || "").length > 0) {
        return qsTr("error")
    }
    if (eventName === "messageReceived" || eventName === "messagePropagated") {
        return qsTr("event")
    }
    return qsTr("ok")
}

function socialIdentityDisplayName(identity) {
    const value = identity || {}
    return String(value.display_name || value.displayName || value.name || value.local_id || value.localId || qsTr("Pseudonym"))
}
