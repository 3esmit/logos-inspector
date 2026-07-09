import QtQml
import "../OperationHistoryVocabulary.js" as OperationHistoryVocabulary
import "../runtime/RuntimeOperationPolicy.js" as RuntimeOperationPolicy

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
        return RuntimeOperationPolicy.operationMetadata(operation || {})
    }

    function explicitAffectedInputs(operation) {
        return RuntimeOperationPolicy.explicitAffectedInputs(operation || {})
    }

    function classifyOperation(operation) {
        return RuntimeOperationPolicy.classifyOperation(operation || {})
    }

    function metadata(operationClass, affectedInputs, restartPolicy, confirmationRequired) {
        return RuntimeOperationPolicy.metadata(operationClass, affectedInputs, restartPolicy, confirmationRequired)
    }

    function affectedInputs(operation) {
        return RuntimeOperationPolicy.affectedInputs(operation || {})
    }

    function pushInput(inputs, key, value) {
        return RuntimeOperationPolicy.pushInput(inputs, key, value)
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
