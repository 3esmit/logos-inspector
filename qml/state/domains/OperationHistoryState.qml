import QtQml
import "../OperationHistoryVocabulary.js" as OperationHistoryVocabulary

QtObject {
    id: root

    property var runtimeOperations: ({})
    property var runtimeOperationEventSeq: ({})
    property var runtimeOperationHistory: []
    property int runtimeOperationsRevision: 0

    function updateOperation(operation) {
        const value = operation || null
        const operationId = String(value && value.operationId ? value.operationId : "")
        if (!operationId.length) {
            return false
        }
        const next = copyObject(runtimeOperations)
        next[operationId] = value
        runtimeOperations = next
        runtimeOperationsRevision += 1
        return true
    }

    function setEventSeq(operationId, seq) {
        const id = String(operationId || "")
        if (!id.length) {
            return false
        }
        const next = copyObject(runtimeOperationEventSeq)
        next[id] = Number(seq || 0)
        runtimeOperationEventSeq = next
        runtimeOperationsRevision += 1
        return true
    }

    function append(operation, detail) {
        const value = operation || {}
        const rows = Array.isArray(runtimeOperationHistory) ? runtimeOperationHistory.slice(-99) : []
        rows.push(historyRecord(value, detail))
        runtimeOperationHistory = rows
        runtimeOperationsRevision += 1
    }

    function rows(domain) {
        const revision = runtimeOperationsRevision
        const wanted = String(domain || "")
        const values = Array.isArray(runtimeOperationHistory) ? runtimeOperationHistory.slice(0) : []
        const filtered = wanted.length ? values.filter(row => String(row.domain || "") === wanted) : values
        return filtered.reverse()
    }

    function historyRecord(operation, detail) {
        const record = OperationHistoryVocabulary.historyRecord(
            operation || {},
            detail,
            new Date().toLocaleTimeString(Qt.locale(), "hh:mm:ss")
        )
        const metadata = operationMetadata(operation || {})
        record.operationClass = metadata.operationClass
        record.affectedInputs = metadata.affectedInputs
        record.restartPolicy = metadata.restartPolicy
        record.confirmationRequired = metadata.confirmationRequired
        if (operation && operation.importId !== undefined) {
            record.importId = String(operation.importId || "")
        }
	        if (operation && operation.backupCatalogId !== undefined) {
	            record.backupCatalogId = String(operation.backupCatalogId || "")
	        }
	        if (operation && operation.previousOperationId !== undefined) {
	            record.previousOperationId = String(operation.previousOperationId || "")
	        }
	        if (operation && operation.restartOperationId !== undefined) {
	            record.restartOperationId = String(operation.restartOperationId || "")
	        }
	        if (operation && operation.reason !== undefined) {
	            record.reason = String(operation.reason || "")
	        }
        if (operation && Array.isArray(operation.provenance)) {
            record.provenance = operation.provenance.slice(0)
        }
        if (operation && operation.result !== undefined) {
            record.result = operation.result
        }
        return record
    }

    function operationMetadata(operation) {
        const value = operation || {}
        const explicitClass = String(value.operationClass || value.operation_class || "")
        const explicitRestart = String(value.restartPolicy || value.restart_policy || "")
        const affected = explicitAffectedInputs(value)
        const classification = classifyOperation(value)
        return {
            operationClass: explicitClass.length ? explicitClass : classification.operationClass,
            affectedInputs: affected.length ? affected : classification.affectedInputs,
            restartPolicy: explicitRestart.length ? explicitRestart : classification.restartPolicy,
            confirmationRequired: value.confirmationRequired !== undefined
                ? value.confirmationRequired === true
                : (value.confirmation_required !== undefined ? value.confirmation_required === true : classification.confirmationRequired)
        }
    }

    function explicitAffectedInputs(operation) {
        if (Array.isArray(operation.affectedInputs)) {
            return operation.affectedInputs.slice(0)
        }
        if (Array.isArray(operation.affected_inputs)) {
            return operation.affected_inputs.slice(0)
        }
        return []
    }

    function classifyOperation(operation) {
        const domain = String(operation.domain || "")
        const method = String(operation.method || operation.label || "").toLowerCase()
        const inputs = affectedInputs(operation)
        if (domain === "backup" || method.indexOf("backup") >= 0 || method.indexOf("restore") >= 0 || method.indexOf("import") >= 0) {
            return metadata("backup", inputs, "manual_required", true)
        }
        if (domain === "wallet" || method.indexOf("wallet") >= 0 || method.indexOf("sign") >= 0 || method.indexOf("submit") >= 0 || method.indexOf("deploy") >= 0) {
            return metadata("signing_submission", inputs, "manual_required", true)
        }
        if (domain === "local_nodes" || method.indexOf("node.start") >= 0 || method.indexOf("node.stop") >= 0 || method.indexOf("delete") >= 0 || method.indexOf("purge") >= 0) {
            return metadata("lifecycle", inputs, "manual_required", true)
        }
        if (method.indexOf("remove") >= 0 || method.indexOf("delete") >= 0) {
            return metadata("destructive", inputs, "manual_required", true)
        }
        if (method.indexOf("upload") >= 0 || method.indexOf("download") >= 0 || method.indexOf("fetch") >= 0 || method.indexOf("send") >= 0) {
            return metadata("mutating", inputs, "manual_required", true)
        }
        if (method.indexOf("status") >= 0 || method.indexOf("query") >= 0 || method.indexOf("read") >= 0 || method.indexOf("list") >= 0 || method.indexOf("manifests") >= 0 || method.indexOf("exists") >= 0 || method.indexOf("health") >= 0 || method.indexOf("probe") >= 0) {
            return metadata("read_poll", inputs, "safe_read_polling", false)
        }
        return metadata("unknown", inputs, "manual_required", true)
    }

    function metadata(operationClass, affectedInputs, restartPolicy, confirmationRequired) {
        return {
            operationClass: operationClass,
            affectedInputs: Array.isArray(affectedInputs) ? affectedInputs : [],
            restartPolicy: restartPolicy,
            confirmationRequired: confirmationRequired === true
        }
    }

    function affectedInputs(operation) {
        const inputs = []
        pushInput(inputs, "domain", operation.domain)
        pushInput(inputs, "method", operation.method)
        pushInput(inputs, "sourceMode", operation.sourceMode)
        pushInput(inputs, "endpoint", operation.endpoint)
        pushInput(inputs, "module", operation.module)
        pushInput(inputs, "cid", operation.cid)
        pushInput(inputs, "path", operation.path)
        return inputs
    }

    function pushInput(inputs, key, value) {
        const text = String(value || "")
        if (text.length > 0) {
            inputs.push({ key: key, value: text })
        }
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
}
