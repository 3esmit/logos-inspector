.import "../../services/BridgeHelpers.js" as BridgeHelpers

function applyResolvedTarget(root, response, errorTitle) {
    with (root) {
        if (!response.ok || response.value === null || response.value === undefined) {
            return false
        }
        const resolved = response.value || {}
        const payload = resolved.payload === undefined ? null : resolved.payload
        switch (String(resolved.kind || "")) {
        case "block":
            if (payload !== null) {
                selectView("l2BlockDetail", false)
                blockDetailValue = root.indexerBlockDetail(payload)
                setResult(qsTr("LEZ block"), BridgeHelpers.formatValue(blockDetailValue), false, blockDetailValue)
                return true
            }
            break
        case "transaction":
            if (payload !== null) {
                selectView("l2TransactionDetail", false)
                transactionDetailValue = payload
                lezTransactionsPageError = ""
                setResult(qsTr("LEZ transaction"), BridgeHelpers.formatValue(payload), false, payload)
                root.autoDecodeTransactionDetail(payload)
                return true
            }
            break
        case "account":
            selectView("accounts", false)
            accountTab = "lookup"
            accountDetailValue = payload
            setResult(qsTr("Account lookup"), BridgeHelpers.formatValue(payload), false, payload)
            return true
        default:
            break
        }
        setResult(errorTitle || qsTr("Search"), qsTr("No block, transaction, or account found."), true, null)
        return true
    }
}
