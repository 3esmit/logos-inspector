.import "../../services/BridgeHelpers.js" as BridgeHelpers

function targetCommand(response, errorTitle) {
    if (!response || !response.ok || response.value === null || response.value === undefined) {
        return { handled: false }
    }
    const resolved = response.value || {}
    const payload = resolved.payload === undefined ? null : resolved.payload
    switch (String(resolved.kind || "")) {
    case "block":
        if (payload !== null) {
            return {
                handled: true,
                kind: "block",
                view: "l2BlockDetail",
                title: qsTr("LEZ block"),
                payload: payload
            }
        }
        break
    case "transaction":
        if (payload !== null) {
            return {
                handled: true,
                kind: "transaction",
                view: "l2TransactionDetail",
                title: qsTr("LEZ transaction"),
                payload: payload,
                autoDecode: true
            }
        }
        break
    case "account":
        return {
            handled: true,
            kind: "account",
            view: "accounts",
            accountTab: "lookup",
            title: qsTr("Account lookup"),
            payload: payload
        }
    default:
        break
    }
    return {
        handled: true,
        kind: "not_found",
        title: errorTitle || qsTr("Search"),
        message: qsTr("No block, transaction, or account found."),
        payload: null,
        error: true
    }
}

function applyResolvedTarget(root, response, errorTitle) {
    const command = targetCommand(response, errorTitle)
    if (!command.handled) {
        return false
    }
    return applyCommand(root, command)
}

function applyCommand(root, command) {
    with (root) {
        switch (String(command.kind || "")) {
        case "block":
            selectView(command.view, false)
            blockDetailValue = root.indexerBlockDetail(command.payload)
            setResult(command.title, BridgeHelpers.formatValue(blockDetailValue), false, blockDetailValue)
            return true
        case "transaction":
            selectView(command.view, false)
            transactionDetailValue = command.payload
            lezTransactionsPageError = ""
            setResult(command.title, BridgeHelpers.formatValue(command.payload), false, command.payload)
            if (command.autoDecode === true) {
                root.autoDecodeTransactionDetail(command.payload)
            }
            return true
        case "account":
            selectView(command.view, false)
            accountTab = command.accountTab
            accountDetailValue = command.payload
            setResult(command.title, BridgeHelpers.formatValue(command.payload), false, command.payload)
            return true
        default:
            setResult(command.title || qsTr("Search"), command.message || qsTr("No block, transaction, or account found."), true, null)
            return true
        }
    }
}
