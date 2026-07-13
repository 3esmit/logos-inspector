.import "../OperationHistoryVocabulary.js" as OperationHistoryVocabulary

function runtimeOperationStart(root, request, showResult, callback) {
    with (root) {
        const operationRequest = request && typeof request === "object" ? request : ({})
        const label = String(operationRequest.label || operationRequest.method || qsTr("Runtime operation"))
        return requestModuleAsync(inspectorModule, "runtimeOperationStart", [operationRequest], label, showResult === true, function (response) {
            if (response && response.ok) {
                coreUpdateRuntimeOperation(root, response.value)
            }
            if (callback) {
                callback(response)
            }
        })
    }
}

function runtimeOperationStatus(root, operationId, showResult, callback) {
    with (root) {
        const id = String(operationId || "")
        if (!id.length) {
            return null
        }
        return requestModuleAsync(inspectorModule, "runtimeOperationStatus", [id], qsTr("Runtime operation"), showResult === true, function (response) {
            if (response && response.ok && matchesOperationId(response.value, id)) {
                coreUpdateRuntimeOperation(root, response.value)
            }
            if (callback) {
                callback(response)
            }
        })
    }
}

function runtimeOperationEvents(root, operationId, afterSeq, showResult, callback) {
    with (root) {
        const id = String(operationId || "")
        if (!id.length) {
            return null
        }
        return requestModuleAsync(inspectorModule, "runtimeOperationEvents", [id, Number(afterSeq || 0)], qsTr("Runtime operation events"), showResult === true, function (response) {
            if (response && response.ok && response.value
                    && matchesOperationId(response.value.operation, id)) {
                coreUpdateRuntimeOperation(root, response.value.operation)
                const eventCursor = responseEventCursor(response.value)
                if (root.operationHistory && typeof root.operationHistory.setEventSeq === "function") {
                    root.operationHistory.setEventSeq(id, eventCursor)
                } else {
                    const currentValue = runtimeOperationEventSeq[id]
                    const current = Number(currentValue)
                    if (currentValue === undefined || !Number.isFinite(current) || eventCursor > current) {
                        const next = copyObject(runtimeOperationEventSeq)
                        next[id] = eventCursor
                        runtimeOperationEventSeq = next
                    }
                }
            }
            if (callback) {
                callback(response)
            }
        })
    }
}

function runtimeOperationCancel(root, operationId, showResult, callback) {
    with (root) {
        const id = String(operationId || "")
        if (!id.length) {
            return null
        }
        return requestModuleAsync(inspectorModule, "runtimeOperationCancel", [id], qsTr("Cancel operation"), showResult === true, function (response) {
            if (response && response.ok && matchesOperationId(response.value, id)) {
                coreUpdateRuntimeOperation(root, response.value)
            }
            if (callback) {
                callback(response)
            }
        })
    }
}

function runtimeOperationModuleEvent(root, event, showResult, callback) {
    with (root) {
        const value = event && typeof event === "object" ? event : ({})
        const envelope = {
            moduleName: String(value.moduleName || ""),
            eventName: String(value.eventName || ""),
            args: Array.isArray(value.args) ? value.args.slice(0) : []
        }
        if (!envelope.moduleName.length || !envelope.eventName.length) {
            return null
        }
        return requestModuleAsync(inspectorModule, "runtimeOperationModuleEvent", [envelope], qsTr("Runtime module event"), showResult === true, function (response) {
            const operation = response && response.ok && response.value
                ? response.value.operation || null
                : null
            if (operation) {
                coreUpdateRuntimeOperation(root, operation)
            }
            if (callback) {
                callback(response)
            }
        })
    }
}

function updateRuntimeOperation(root, operation) {
    coreUpdateRuntimeOperation(root, operation)
}

function coreUpdateRuntimeOperation(root, operation) {
    with (root) {
        const value = operation || null
        const operationId = String(value && value.operationId ? value.operationId : "")
        if (!operationId.length) {
            return
        }
        if (root.operationHistory && typeof root.operationHistory.updateOperation === "function") {
            root.operationHistory.updateOperation(value)
            return
        }
        const current = runtimeOperations[operationId] || null
        if (!OperationHistoryVocabulary.runtimeSnapshotIsNewer(current, value)) {
            return
        }
        const next = copyObject(runtimeOperations)
        next[operationId] = value
        runtimeOperations = next
        runtimeOperationsRevision += 1
    }
}

function runtimeOperationTerminal(root, operation) {
    return OperationHistoryVocabulary.isRuntimeTerminalStatus(operation && operation.status)
}

function runtimeOperationResponse(root, operation) {
    const status = String(operation && operation.status ? operation.status : "")
    const ok = OperationHistoryVocabulary.isRuntimeSuccessfulTerminalStatus(status)
    return {
        ok: ok,
        value: operation && operation.result !== undefined && operation.result !== null ? operation.result : operation,
        text: "",
        error: ok ? "" : String(operation && operation.error ? operation.error : "")
    }
}

function appendRuntimeOperationHistory(root, operation, detail) {
    appendOperationHistory(root, operation, detail)
}

function appendOperationHistory(root, operation, detail) {
    with (root) {
        if (root.operationHistory && typeof root.operationHistory.append === "function") {
            root.operationHistory.append(operation || {}, detail)
            return
        }
        const value = operation || {}
        const rows = Array.isArray(runtimeOperationHistory) ? runtimeOperationHistory.slice(-99) : []
        rows.push(OperationHistoryVocabulary.historyRecord(
            value,
            detail,
            new Date().toLocaleTimeString(Qt.locale(), "hh:mm:ss")
        ))
        runtimeOperationHistory = rows
        runtimeOperationsRevision += 1
    }
}

function runtimeOperationHistoryRows(root, domain) {
    return operationHistoryRows(root, domain)
}

function operationHistoryRows(root, domain) {
    with (root) {
        if (root.operationHistory && typeof root.operationHistory.rows === "function") {
            return root.operationHistory.rows(domain)
        }
        const revision = runtimeOperationsRevision
        const wanted = String(domain || "")
        const rows = Array.isArray(runtimeOperationHistory) ? runtimeOperationHistory.slice(0) : []
        const filtered = wanted.length ? rows.filter(row => String(row.domain || "") === wanted) : rows
        return filtered.reverse()
    }
}

function runtimeOperationDetail(root, operation) {
    return OperationHistoryVocabulary.historyDetail(operation)
}

function copyObject(value) {
    const next = ({})
    const source = value && typeof value === "object" && !Array.isArray(value) ? value : ({})
    const keys = Object.keys(source)
    for (let i = 0; i < keys.length; ++i) {
        next[keys[i]] = source[keys[i]]
    }
    return next
}

function matchesOperationId(operation, expectedOperationId) {
    const operationId = String(operation && operation.operationId ? operation.operationId : "")
    return operationId.length > 0 && operationId === String(expectedOperationId || "")
}

function responseEventCursor(value) {
    const response = value && typeof value === "object" ? value : ({})
    const raw = response.eventCursor !== undefined && response.eventCursor !== null
        ? response.eventCursor
        : response.nextSeq
    const cursor = Number(raw)
    return Number.isFinite(cursor) && cursor >= 0 ? cursor : 0
}
