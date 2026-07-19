function referenceTarget(session, kind, value, payload) {
    const target = session.model.metrics.valueToString(value).trim()
    if (!target.length && payload === undefined) {
        return { command: "", target: "", payload: undefined }
    }

    switch (String(kind || "")) {
    case "block":
    case "blockHash":
    case "blockNumber":
    case "slot":
        return { command: "blockchainBlock", target: target, payload: payload === undefined ? target : payload }
    case "mantleTransaction":
        return { command: "mantleTransaction", target: target, payload: payload }
    case "wallet":
        return { command: "localWallet", target: target, tab: "profiles", payload: undefined }
    case "private":
    case "privateAccount":
        return { command: "privateAccount", target: target, payload: undefined }
    case "bedrockWallet":
    case "note":
        return { command: "localWallet", target: target, tab: "bedrockNotes", payload: undefined }
    case "indexerBlock":
    case "lezBlock":
        return { command: "search", target: "l2:" + target, payload: undefined }
    case "transaction":
    case "transactionHash":
    case "tx":
        return { command: "search", target: "tx:" + target, payload: undefined }
    case "channel":
        return { command: "search", target: "channel:" + target, payload: undefined }
    case "account":
    case "signer":
        return { command: "search", target: "account:" + target, payload: undefined }
    case "program":
        return { command: "search", target: "program:" + target, payload: undefined }
    default:
        return { command: "search", target: target, payload: undefined }
    }
}

function openReference(session, kind, value, payload) {
    const target = referenceTarget(session, kind, value, payload)
    switch (target.command) {
    case "blockchainBlock":
        session.openBlockchainBlock(target.payload)
        return
    case "mantleTransaction":
        session.openMantleTransaction(target.target, target.payload)
        return
    case "localWallet":
        session.openLocalWallet(target.target, target.tab)
        return
    case "privateAccount":
        session.openPrivateAccountReference(target.target)
        return
    case "search":
        session.routeSearch(target.target)
    }
}
