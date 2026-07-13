function operationMetadata(operation) {
    const value = operation || {}
    const explicitClass = String(value.operationClass || value.operation_class || "")
    const explicitRestart = String(value.restartPolicy || value.restart_policy || "")
    const explicitConfirmation = value.confirmationRequired !== undefined
        ? value.confirmationRequired
        : value.confirmation_required
    const facts = policyFacts(value)
    const affected = explicitAffectedInputs(value)
    const factAffected = Array.isArray(facts.affectedInputs) ? facts.affectedInputs : []
    const classification = classifyOperation(value)
    return {
        operationClass: explicitClass.length ? explicitClass : String(facts.operationClass || classification.operationClass),
        affectedInputs: affected.length ? affected : (factAffected.length ? factAffected : classification.affectedInputs),
        restartPolicy: explicitRestart.length ? explicitRestart : String(facts.restartPolicy || classification.restartPolicy),
        confirmationRequired: explicitConfirmation !== undefined
            ? explicitConfirmation === true
            : (facts.confirmationRequired !== undefined ? facts.confirmationRequired === true : classification.confirmationRequired)
    }
}

function policyFacts(operation) {
    const value = operation || {}
    const source = value.policyFacts && typeof value.policyFacts === "object"
        ? value.policyFacts
        : (value.policy_facts && typeof value.policy_facts === "object" ? value.policy_facts : ({}))
    return {
        operationClass: String(source.operationClass || source.operation_class || ""),
        affectedInputs: Array.isArray(source.affectedInputs)
            ? source.affectedInputs.slice(0)
            : (Array.isArray(source.affected_inputs) ? source.affected_inputs.slice(0) : []),
        restartPolicy: String(source.restartPolicy || source.restart_policy || ""),
        confirmationRequired: source.confirmationRequired !== undefined
            ? source.confirmationRequired
            : source.confirmation_required
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
    if (domain === "local_nodes" || domain === "localNodes" || method.indexOf("node.start") >= 0 || method.indexOf("node.stop") >= 0 || method.indexOf("delete") >= 0 || method.indexOf("purge") >= 0) {
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
    const adapter = operation && operation.adapter && typeof operation.adapter === "object" ? operation.adapter : ({})
    const adapterInputs = adapter.inputs && typeof adapter.inputs === "object" ? adapter.inputs : ({})
    pushInput(inputs, "domain", operation.domain)
    pushInput(inputs, "method", operation.method)
    pushInput(inputs, "sourceMode", adapter.source_mode)
    pushInput(inputs, "endpoint", adapterInputs.rest_endpoint || adapterInputs.rpc_endpoint)
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
