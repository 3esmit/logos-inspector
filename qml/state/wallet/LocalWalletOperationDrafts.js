.import "../ConfirmationPolicy.js" as ConfirmationPolicy

function invalid(title, message, tab) {
    return {
        ok: false,
        title: title,
        message: String(message || ""),
        tab: String(tab || "")
    }
}

function request(title, method, args, historyLabel, successStatus, fallback, failureMessage, showResult) {
    return {
        ok: true,
        title: title,
        method: method,
        args: args,
        historyLabel: historyLabel,
        successStatus: successStatus,
        fallback: fallback,
        failureMessage: failureMessage,
        showResult: showResult === true
    }
}

function busyDraft(root, title) {
    if (root.gateway && root.gateway.busy() === true) {
        return invalid(title, qsTr("Another inspection is already running."))
    }
    return null
}

function profileDraft(root, title, message, tab) {
    if (root.profileConfigured()) {
        return null
    }
    return invalid(title, message, tab || "profiles")
}

function createAccount(root) {
    const title = qsTr("Wallet account")
    const busy = busyDraft(root, title)
    if (busy) {
        return busy
    }
    const missingProfile = profileDraft(root, title, qsTr("Configure wallet binary and wallet home before creating an account."), "profiles")
    if (missingProfile) {
        return missingProfile
    }
    const privacy = String(root.createPrivacy || "public").toLowerCase() === "private" ? "private" : "public"
    const label = String(root.createLabel || "").trim()
    return request(
        title,
        "localWalletCreateAccount",
        [root.currentProfile(), privacy, label, ConfirmationPolicy.token("wallet-create-account")],
        qsTr("Create account"),
        "created",
        "command",
        qsTr("Account creation failed."),
        true
    )
}

function sendTransaction(root) {
    const title = qsTr("Wallet send")
    const busy = busyDraft(root, title)
    if (busy) {
        return busy
    }
    const missingProfile = profileDraft(root, title, qsTr("Configure wallet binary and wallet home before sending a transaction."), "profiles")
    if (missingProfile) {
        return missingProfile
    }
    const payload = {
        from: String(root.sendFrom || "").trim(),
        to: String(root.sendTo || "").trim(),
        to_keys: String(root.sendToKeys || "").trim(),
        to_npk: String(root.sendToNpk || "").trim(),
        to_vpk: String(root.sendToVpk || "").trim(),
        to_identifier: String(root.sendToIdentifier || "").trim(),
        amount: String(root.sendAmount || "").trim()
    }
    if (!payload.from.length || !payload.amount.length) {
        return invalid(title, qsTr("Sender and amount are required."))
    }
    if (!payload.to.length && !payload.to_keys.length && (!payload.to_npk.length || !payload.to_vpk.length)) {
        return invalid(title, qsTr("Recipient account, keys file, or NPK/VPK pair is required."))
    }
    return request(
        title,
        "localWalletSendTransaction",
        [root.currentProfile(), payload, ConfirmationPolicy.token("wallet-send-transaction")],
        qsTr("Send transaction"),
        "submitted",
        "command",
        qsTr("Wallet send failed."),
        true
    )
}

function readIncomingTransactions(root) {
    const title = qsTr("Read incoming")
    const busy = busyDraft(root, title)
    if (busy) {
        return busy
    }
    const missingProfile = profileDraft(root, title, qsTr("Configure wallet binary and wallet home before reading incoming transactions."), "profiles")
    if (missingProfile) {
        return missingProfile
    }
    return request(
        title,
        "localWalletSyncPrivate",
        [root.currentProfile(), ConfirmationPolicy.token("wallet-sync-private")],
        qsTr("Read incoming"),
        "submitted",
        "privateSync",
        qsTr("Incoming transaction read failed."),
        true
    )
}

function runCommand(root, commandArgs) {
    const title = qsTr("Wallet command")
    const args = Array.isArray(commandArgs) ? commandArgs : []
    const busy = busyDraft(root, title)
    if (busy) {
        return busy
    }
    const missingProfile = profileDraft(root, title, qsTr("Configure wallet binary and wallet home before running wallet commands."), "profiles")
    if (missingProfile) {
        return missingProfile
    }
    if (!args.length) {
        return invalid(title, qsTr("Wallet command arguments are required."))
    }
    return request(
        title,
        "localWalletCommand",
        [root.currentProfile(), args, ConfirmationPolicy.token("wallet-command")],
        qsTr("Wallet command"),
        "completed",
        "command",
        qsTr("Wallet command failed."),
        true
    )
}

function syncPrivate(root) {
    const title = qsTr("Private sync")
    const busy = busyDraft(root, title)
    if (busy) {
        return busy
    }
    const missingProfile = profileDraft(root, title, "", "profiles")
    if (missingProfile) {
        return missingProfile
    }
    return request(
        title,
        "localWalletSyncPrivate",
        [root.currentProfile(), ConfirmationPolicy.token("wallet-sync-private")],
        qsTr("Private sync"),
        "submitted",
        "privateSync",
        qsTr("Private sync failed."),
        true
    )
}

function queryAccounts(root, showResult) {
    const title = qsTr("Wallet accounts")
    const busy = busyDraft(root, title)
    if (busy) {
        return busy
    }
    if (!root.profileConfigured()) {
        return invalid(title, qsTr("Configure wallet binary and wallet home, or check a profile that resolves $LEE_WALLET_HOME_DIR."))
    }
    const draft = request(
        title,
        "localWalletAccounts",
        [root.currentProfile()],
        qsTr("Wallet accounts"),
        "loaded",
        "accounts",
        qsTr("Wallet account list failed."),
        showResult === true
    )
    draft.kind = "accounts"
    return draft
}

function queryBedrockBalance(root) {
    const publicKey = String(root.publicKeyProbe || "").trim()
    if (!publicKey.length) {
        return {
            ok: false,
            title: qsTr("Bedrock wallet"),
            balanceError: qsTr("Wallet public key is required.")
        }
    }
    if (!root.isBedrockHexId(publicKey)) {
        return {
            ok: false,
            title: qsTr("Bedrock wallet"),
            balanceError: qsTr("Wallet public key must be 64 hex characters.")
        }
    }
    const tip = String(root.bedrockBalanceTip || "").trim()
    if (tip.length > 0 && !root.isBedrockHexId(tip)) {
        return {
            ok: false,
            title: qsTr("Bedrock wallet"),
            balanceError: qsTr("Balance tip must be a 64-hex header id.")
        }
    }
    const draft = request(
        qsTr("Bedrock wallet"),
        "bedrockWalletBalance",
        [root.gateway.nodeUrl(), publicKey, tip],
        qsTr("Bedrock balance"),
        "ok",
        "",
        qsTr("Balance query failed."),
        false
    )
    draft.kind = "bedrockBalance"
    draft.publicKey = publicKey
    return draft
}
