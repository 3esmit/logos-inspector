const CONTRACTS = [
    {
        method: "storageUploadUrl",
        progress: "storageUploadProgress",
        terminal: "storageUploadDone",
        match: "session",
        refreshOnTerminal: true
    },
    {
        method: "storageDownloadToUrl",
        progress: "storageDownloadProgress",
        terminal: "storageDownloadDone",
        match: "sessionOrCid",
        refreshOnTerminal: true
    },
    {
        method: "storageDownloadManifest",
        progress: "",
        terminal: "storageDownloadManifestDone",
        match: "cid",
        refreshOnTerminal: false
    },
    {
        method: "storageRemove",
        progress: "",
        terminal: "storageRemoveDone",
        match: "cid",
        refreshOnTerminal: true
    }
]

const LIFECYCLE_EVENTS = [
    "storageStart",
    "storageStop",
    "storageConnect"
]

function eventContract(method) {
    const name = String(method || "")
    for (let i = 0; i < CONTRACTS.length; ++i) {
        if (CONTRACTS[i].method === name) {
            return Object.assign({}, CONTRACTS[i])
        }
    }
    return null
}

function contractForEvent(eventName) {
    const name = String(eventName || "")
    for (let i = 0; i < CONTRACTS.length; ++i) {
        const contract = CONTRACTS[i]
        if (contract.progress === name || contract.terminal === name) {
            return Object.assign({}, contract)
        }
    }
    return null
}

function methodForEvent(eventName) {
    const contract = contractForEvent(eventName)
    return contract ? contract.method : String(eventName || "storageModuleEvent")
}

function refreshAfterTerminalEvent(eventName) {
    const name = String(eventName || "")
    for (let i = 0; i < CONTRACTS.length; ++i) {
        const contract = CONTRACTS[i]
        if (contract.terminal === name) {
            return contract.refreshOnTerminal === true
        }
    }
    return false
}

function subscriptionEvents() {
    const events = LIFECYCLE_EVENTS.slice(0)
    for (let i = 0; i < CONTRACTS.length; ++i) {
        const contract = CONTRACTS[i]
        if (contract.progress.length) {
            events.push(contract.progress)
        }
        events.push(contract.terminal)
    }
    return events
}
