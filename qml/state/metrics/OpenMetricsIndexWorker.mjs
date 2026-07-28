function openMetricLabels(text) {
    const labels = {}
    const pattern = /([A-Za-z_:][A-Za-z0-9_:]*)\s*=\s*"((?:\\.|[^"\\])*)"/g
    let match = pattern.exec(String(text || ""))
    while (match !== null) {
        labels[match[1]] = match[2].replace(/\\"/g, "\"").replace(/\\\\/g, "\\")
        match = pattern.exec(String(text || ""))
    }
    return labels
}

function appendInvalidOpenMetricSample(samplesByName, name, labels) {
    if (!Array.isArray(samplesByName[name])) {
        samplesByName[name] = []
    }
    samplesByName[name].push({ labels: labels })
}

function buildOpenMetricsIndex(value, revision, workerGeneration) {
    const samplesByName = {}
    const malformedNames = {}
    const invalidSamplesByName = {}
    const lines = String(value || "").split(/\r?\n/)
    let order = 0
    for (let i = 0; i < lines.length; ++i) {
        const line = lines[i].trim()
        if (!line.length || line[0] === "#") {
            continue
        }
        const sample = line.match(/^([^{\s]+)(?:\{([^}]*)\})?(?:\s+(.+))?$/)
        if (!sample) {
            const prefix = line.match(/^([^{\s]+)/)
            if (prefix && prefix[1]) {
                malformedNames[prefix[1]] = true
            }
            continue
        }
        const name = sample[1]
        const labels = openMetricLabels(sample[2] || "")
        const numeric = String(sample[3] || "").match(
            /^([+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:e[+-]?[0-9]+)?)(?:\s|$)/i)
        if (!numeric) {
            appendInvalidOpenMetricSample(invalidSamplesByName, name, labels)
            continue
        }
        const parsed = Number(numeric[1])
        if (!Number.isFinite(parsed)) {
            appendInvalidOpenMetricSample(invalidSamplesByName, name, labels)
            continue
        }
        const entry = {
            name: name,
            labels: labels,
            value: parsed,
            order: order
        }
        order += 1
        if (samplesByName[name] === undefined) {
            samplesByName[name] = []
        }
        samplesByName[name].push(entry)
    }
    return {
        revision: Number(revision || 0),
        workerGeneration: Number(workerGeneration || 0),
        samplesByName: samplesByName,
        malformedNames: malformedNames,
        invalidSamplesByName: invalidSamplesByName
    }
}

WorkerScript.onMessage = function(message) {
    WorkerScript.sendMessage(buildOpenMetricsIndex(
        message && message.value,
        message && message.revision,
        message && message.workerGeneration))
}
