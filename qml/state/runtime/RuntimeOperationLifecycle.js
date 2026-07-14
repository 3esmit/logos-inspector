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
        const after = safeEventSequence(afterSeq === undefined || afterSeq === null ? 0 : afterSeq)
        if (!id.length || after === null) {
            return null
        }
        const history = root.operationHistory || null
        const guarded = history
            && typeof history.beginEventPoll === "function"
            && typeof history.finishEventPoll === "function"
            && typeof history.abandonEventPoll === "function"
        const ticket = guarded
            ? history.beginEventPoll(id, after, runtimeOperationPollContext(root, id)) : null
        if (guarded && !ticket) {
            return null
        }
        let completion = null
        let callbackDelivered = false
        let dispatch = null
        try {
            dispatch = requestModuleAsync(inspectorModule, "runtimeOperationEvents", [id, after], qsTr("Runtime operation events"), showResult === true, function (response) {
            if (callbackDelivered) {
                return
            }
            if (guarded && completion === null) {
                completion = history.finishEventPoll(
                    ticket,
                    response,
                    runtimeOperationPollContext(root, id)
                )
            }
            normalizeInvalidEventResponse(response, completion)
            if (guarded && (!completion
                    || (completion.accepted !== true && completion.invalid !== true))) {
                return
            }
            callbackDelivered = true
            if (response && response.ok && response.value
                    && matchesOperationId(response.value.operation, id)) {
                if (!guarded) {
                    coreUpdateRuntimeOperation(root, response.value.operation)
                    const eventCursor = responseEventCursor(response.value)
                    const currentValue = runtimeOperationEventSeq[id]
                    const current = Number(currentValue)
                    if (eventCursor !== null
                            && (currentValue === undefined
                                || !Number.isSafeInteger(current)
                                || eventCursor > current)) {
                        const next = copyObject(runtimeOperationEventSeq)
                        next[id] = eventCursor
                        runtimeOperationEventSeq = next
                    }
                }
            }
            if (callback) {
                callback(response)
            }
        }, guarded ? function (response) {
            completion = history.finishEventPoll(
                ticket,
                response,
                runtimeOperationPollContext(root, id)
            )
            if (completion && completion.invalid === true) {
                normalizeInvalidEventResponse(response, completion)
                return true
            }
            return completion && completion.accepted === true
            } : null)
        } catch (error) {
            if (guarded) {
                history.abandonEventPoll(ticket)
            }
            throw error
        }
        if (guarded && (dispatch === null || dispatch === undefined || dispatch === false)) {
            history.abandonEventPoll(ticket)
        }
        return dispatch
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
    const payloadPurged = operation && (status === "completed"
        ? operation.resultPurged === true
        : status === "dispatched" && operation.acknowledgementPurged === true)
    return {
        ok: ok && !payloadPurged,
        value: operation && operation.result !== undefined && operation.result !== null ? operation.result : operation,
        text: "",
        error: payloadPurged
            ? qsTr("Runtime operation result is no longer retained in bounded history.")
            : ok ? "" : String(operation && operation.error ? operation.error : "")
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
    return safeEventSequence(raw)
}

function runtimeOperationPollContext(root, operationId) {
    const history = root && root.operationHistory ? root.operationHistory : null
    const operations = history && history.runtimeOperations
        ? history.runtimeOperations : root && root.runtimeOperations ? root.runtimeOperations : ({})
    const operation = operations[String(operationId || "")] || null
    const bridge = root && root.bridge ? root.bridge : null
    const hostEpoch = bridge ? safeEventSequence(bridge.hostEpoch) : 0
    const backendIdentity = operation && history
            && typeof history.operationBackendIdentity === "function"
        ? history.operationBackendIdentity(operation) : ""
    const backendRevision = operation && history
            && typeof history.operationBackendRevision === "function"
        ? history.operationBackendRevision(operation) : ""
    return {
        hostEpoch: hostEpoch === null ? 0 : hostEpoch,
        hostIdentity: bridge && bridge.host !== undefined ? bridge.host : null,
        configurationIdentity: runtimeConfigurationIdentity(root),
        backendIdentity: backendIdentity,
        backendRevision: backendRevision
    }
}

function runtimeConfigurationIdentity(root) {
    const value = root || ({})
    return JSON.stringify([
        numericIdentity(value.networkConfigurationRevision),
        numericIdentity(value.blockchainConfigurationRevision),
        String(value.blockchainConfigurationSignature || ""),
        String(value.networkProfile || ""),
        String(value.blockchainSourceMode || ""),
        String(value.nodeUrl || ""),
        String(value.messagingSourceMode || ""),
        String(value.messagingRestUrl || ""),
        String(value.messagingNetworkPreset || ""),
        String(value.storageSourceMode || ""),
        String(value.storageRestUrl || ""),
        String(value.storageNetworkPreset || "")
    ])
}

function numericIdentity(value) {
    const number = Number(value)
    return Number.isSafeInteger(number) ? number : 0
}

function safeEventSequence(value) {
    if (value === undefined || value === null || value === "" || typeof value === "boolean") {
        return null
    }
    const number = Number(value)
    return Number.isSafeInteger(number) && number >= 0 ? number : null
}

function normalizeInvalidEventResponse(response, completion) {
    if (!response || !completion || completion.invalid !== true) {
        return false
    }
    response.ok = false
    response.value = null
    response.text = ""
    response.error = "invalid runtime operation event window: "
        + String(completion.error || "invalid_event_window")
    return true
}
