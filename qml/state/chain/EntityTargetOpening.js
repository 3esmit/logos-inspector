function referenceTarget(session, kind, value, payload) {
    const target = session.model.valueToString(value).trim()
    if (!target.length && payload === undefined) {
        return { command: "", target: "", payload: undefined }
    }

    switch (String(kind || "")) {
    case "block":
    case "blockHash":
    case "blockNumber":
    case "slot":
        return { command: "blockchainBlock", target: target, payload: payload === undefined ? target : payload }
    case "indexerBlock":
        return { command: "indexerBlock", target: target, payload: payload }
    case "lezBlock":
        return { command: "lezBlock", target: target, payload: undefined }
    case "transaction":
    case "transactionHash":
    case "tx":
        return { command: "transaction", target: target, payload: undefined }
    case "mantleTransaction":
        return { command: "mantleTransaction", target: target, payload: undefined }
    case "wallet":
        return { command: "localWallet", target: target, tab: "profiles", payload: undefined }
    case "private":
    case "privateAccount":
        return { command: "privateAccount", target: target, payload: undefined }
    case "bedrockWallet":
    case "note":
        return { command: "localWallet", target: target, tab: "bedrockNotes", payload: undefined }
    case "recipient":
    case "transferRecipient":
        return { command: "recipient", target: target, payload: undefined }
    case "channel":
        return { command: "channel", target: target, payload: payload === undefined ? target : payload }
    case "account":
    case "signer":
        return { command: "account", target: target, payload: undefined }
    case "program":
        return { command: "program", target: target, payload: undefined }
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
    case "indexerBlock":
        session.openIndexerBlock(target.target, target.payload)
        return
    case "lezBlock":
        session.openLezBlock(target.target)
        return
    case "transaction":
        session.openTransaction(target.target)
        return
    case "mantleTransaction":
        session.openMantleTransaction(target.target)
        return
    case "localWallet":
        session.openLocalWallet(target.target, target.tab)
        return
    case "privateAccount":
        session.openPrivateAccountReference(target.target)
        return
    case "recipient":
        session.openRecipient(target.target)
        return
    case "channel":
        session.openChannel(target.payload)
        return
    case "account":
        session.openAccount(target.target)
        return
    case "program":
        session.openProgram(target.target)
        return
    case "search":
        session.routeSearch(target.target)
    }
}
