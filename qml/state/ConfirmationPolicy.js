function token(key) {
    switch (String(key || "")) {
    case "local-node-action":
        return "confirm-local-node-action"
    case "wallet-create-account":
        return "confirm-create-account"
    case "wallet-send-transaction":
        return "confirm-send-transaction"
    case "wallet-instruction-submit":
        return "confirm-idl-instruction"
    case "wallet-command":
        return "confirm-wallet-command"
    case "wallet-deploy-program":
        return "confirm-deploy-program"
    case "wallet-sync-private":
        return "confirm-sync-private"
    default:
        return ""
    }
}
