import QtQml

QtObject {
    id: root

    property var bridge: null
    readonly property string providerModule: "medusa_core"
    readonly property string providerLabel: qsTr("Medusa wallet")
    property int pollIntervalMs: 1000

    property string availability: "unknown"
    property string availabilityDetail: ""
    property string sessionId: ""
    property var session: null
    property var accounts: []
    property var grantedPermissions: []
    property string pendingRequestId: ""
    property string pendingKind: ""
    property string pendingJobId: ""
    property bool callInFlight: false
    property bool pendingPollInFlight: false
    property bool jobPollInFlight: false
    property int connectionEpoch: 0
    property string error: ""
    property string notice: ""
    property var transferResult: null
    property var operations: []

    readonly property bool connected: sessionId.length > 0
        && session !== null
        && session.active !== false
    readonly property bool awaitingApproval: pendingRequestId.length > 0
    readonly property bool trackingTransfer: pendingJobId.length > 0
    readonly property bool busy: callInFlight || pendingPollInFlight || jobPollInFlight

    property list<QtObject> timers: [
        Timer {
            interval: root.pollIntervalMs
            repeat: true
            running: root.pendingRequestId.length > 0 && !root.pendingPollInFlight
            onTriggered: root.pollPendingRequest()
        },
        Timer {
            interval: root.pollIntervalMs
            repeat: true
            running: root.pendingJobId.length > 0 && !root.jobPollInFlight
            onTriggered: root.pollTransferJob()
        }
    ]

    function checkAvailability() {
        if (callInFlight) {
            return false
        }
        availability = "checking"
        availabilityDetail = ""
        callInFlight = true
        invoke("getStatus", [], function (response) {
            callInFlight = false
            if (!response.ok) {
                availability = "unavailable"
                availabilityDetail = response.error
                return
            }
            const value = response.value && typeof response.value === "object"
                ? response.value : ({})
            if (value.cliFound !== true) {
                availability = "unavailable"
                availabilityDetail = qsTr("Wallet CLI is unavailable.")
                return
            }
            availability = "available"
            availabilityDetail = providerDetail(value)
        })
        return true
    }

    function connectAccounts() {
        return connectWithPermissions(["accounts"], null)
    }

    function startNativeTransfer(from, to, amount) {
        const action = nativeTransferAction(from, to, amount)
        if (!action.ok) {
            error = action.error
            notice = ""
            return false
        }
        error = ""
        if (connected && hasPermission("send")) {
            issueTransfer(action.value)
            return true
        }
        return connectWithPermissions(["accounts", "send"], action.value)
    }

    function disconnect() {
        const activeSession = sessionId
        connectionEpoch += 1
        clearConnection()
        if (activeSession.length === 0) {
            notice = qsTr("No wallet session is connected.")
            return true
        }
        callInFlight = true
        invoke("revokeSession", [activeSession], function (response) {
            callInFlight = false
            if (!response.ok) {
                error = response.error
                notice = qsTr("Inspector forgot the wallet session. Revoke it in the wallet if it remains active.")
                return
            }
            error = ""
            notice = qsTr("Wallet session disconnected.")
        })
        return true
    }

    function forgetPendingRequest() {
        if (pendingRequestId.length === 0) {
            return false
        }
        connectionEpoch += 1
        clearConnection()
        error = ""
        notice = qsTr("Inspector stopped polling. Reject the pending request in the wallet to remove it there.")
        return true
    }

    function hasPermission(permission) {
        return Array.isArray(grantedPermissions)
            && grantedPermissions.indexOf(String(permission || "")) >= 0
    }

    function availabilityLabel() {
        switch (availability) {
        case "checking":
            return qsTr("Checking")
        case "available":
            return qsTr("Available")
        case "unavailable":
            return qsTr("Unavailable")
        default:
            return qsTr("Not checked")
        }
    }

    function availabilityTone() {
        if (availability === "available") {
            return "success"
        }
        if (availability === "unavailable") {
            return "error"
        }
        if (availability === "checking") {
            return "info"
        }
        return "neutral"
    }

    function sessionLabel() {
        if (awaitingApproval) {
            return qsTr("Approval pending")
        }
        if (!connected) {
            return qsTr("Disconnected")
        }
        return qsTr("%1 accounts").arg(Array.isArray(accounts) ? accounts.length : 0)
    }

    function sessionTone() {
        if (awaitingApproval) {
            return "warning"
        }
        return connected ? "success" : "neutral"
    }

    function approvalLabel() {
        if (pendingKind === "connect") {
            return qsTr("Approve connection")
        }
        if (pendingKind === "transfer") {
            return qsTr("Approve transfer")
        }
        if (trackingTransfer) {
            return qsTr("Waiting for transaction")
        }
        return hasPermission("send") ? qsTr("Transfers approved") : qsTr("Accounts only")
    }

    function approvalTone() {
        if (awaitingApproval || trackingTransfer) {
            return "warning"
        }
        return hasPermission("send") ? "success" : "neutral"
    }

    function accountRows() {
        const rows = Array.isArray(accounts) ? accounts : []
        if (rows.length === 0) {
            return [{
                accountId: "",
                cells: [
                    { text: qsTr("No accounts authorized"), width: 320, fill: true, monospace: false },
                    { text: "-", width: 160, monospace: false }
                ]
            }]
        }
        return rows.map(function (account) {
            const accountId = String(account || "")
            return {
                accountId: accountId,
                cells: [
                    { text: accountId, width: 320, fill: true, link: accountId.length > 0, copyText: accountId },
                    { text: qsTr("Authorized"), width: 160, monospace: false, tone: "success" }
                ]
            }
        })
    }

    function operationRows() {
        const rows = Array.isArray(operations) ? operations : []
        if (rows.length === 0) {
            return [{
                cells: [
                    { text: qsTr("No provider operations yet"), width: 170, monospace: false },
                    { text: "-", width: 180, fill: true, monospace: false },
                    { text: "-", width: 120, monospace: false },
                    { text: "-", width: 260, fill: true, monospace: false }
                ]
            }]
        }
        return rows.map(function (row) {
            return {
                cells: [
                    { text: String(row.time || "-"), width: 100, monospace: false },
                    { text: String(row.label || "-"), width: 180, fill: true, monospace: false },
                    { text: String(row.status || "-"), width: 120, monospace: false, tone: String(row.tone || "neutral") },
                    { text: String(row.detail || "-"), width: 260, fill: true, monospace: false }
                ]
            }
        })
    }

    function connectWithPermissions(permissions, deferredTransfer) {
        if (busy || awaitingApproval) {
            return false
        }
        const requestedPermissions = Array.isArray(permissions) ? permissions.slice() : []
        if (requestedPermissions.length === 0) {
            error = qsTr("Wallet permissions are required.")
            return false
        }
        const issue = function () {
            issueConnect(requestedPermissions, deferredTransfer)
        }
        if (!connected) {
            issue()
            return true
        }
        const activeSession = sessionId
        connectionEpoch += 1
        clearConnection()
        callInFlight = true
        invoke("revokeSession", [activeSession], function (response) {
            callInFlight = false
            if (!response.ok) {
                error = response.error
                notice = qsTr("Reconnect was not started because the previous wallet session could not be revoked.")
                return
            }
            issue()
        })
        return true
    }

    function issueConnect(permissions, deferredTransfer) {
        const requestEpoch = connectionEpoch
        error = ""
        notice = qsTr("Approve the connection in the Basecamp wallet.")
        callInFlight = true
        invoke("connectRequest", [JSON.stringify(applicationMetadata()), JSON.stringify(permissions)], function (response) {
            callInFlight = false
            if (requestEpoch !== connectionEpoch) {
                return
            }
            if (!response.ok) {
                failConnection(response.error)
                return
            }
            const requestId = response.value && response.value.requestId !== undefined
                ? String(response.value.requestId || "") : ""
            if (requestId.length === 0) {
                failConnection(qsTr("Wallet did not return a connection request ID."))
                return
            }
            pendingRequestId = requestId
            pendingKind = "connect"
            pendingTransfer = deferredTransfer || null
            appendOperation(qsTr("Wallet connection"), qsTr("pending"), qsTr("Awaiting wallet approval"), "warning")
        })
    }

    property var pendingTransfer: null

    function pollPendingRequest() {
        const requestId = pendingRequestId
        const kind = pendingKind
        const requestEpoch = connectionEpoch
        if (requestId.length === 0 || pendingPollInFlight) {
            return false
        }
        pendingPollInFlight = true
        invoke("actionStatus", [requestId], function (response) {
            pendingPollInFlight = false
            if (requestEpoch !== connectionEpoch || requestId !== pendingRequestId || kind !== pendingKind) {
                return
            }
            if (!response.ok) {
                failConnection(response.error)
                return
            }
            const value = response.value && typeof response.value === "object" ? response.value : ({})
            const status = String(value.status || "").toLowerCase()
            if (status === "pending") {
                return
            }
            pendingRequestId = ""
            pendingKind = ""
            if (status !== "approved") {
                failConnection(String(value.error || qsTr("Wallet request was rejected.")))
                return
            }
            if (kind === "connect") {
                const nextSessionId = String(value.sessionId || "")
                if (nextSessionId.length === 0) {
                    failConnection(qsTr("Wallet approval did not return a session ID."))
                    return
                }
                sessionId = nextSessionId
                loadSession(function () {
                    const transfer = pendingTransfer
                    pendingTransfer = null
                    if (transfer) {
                        issueTransfer(transfer)
                    }
                })
                return
            }
            const jobId = String(value.jobId || "")
            if (jobId.length === 0) {
                failConnection(qsTr("Wallet approval did not return a transaction job ID."))
                return
            }
            pendingJobId = jobId
            notice = qsTr("Wallet approved the transfer. Waiting for transaction completion.")
            appendOperation(qsTr("Native transfer"), qsTr("approved"), jobId, "success")
        })
        return true
    }

    function loadSession(afterLoad) {
        const requestedSessionId = sessionId
        const requestEpoch = connectionEpoch
        if (requestedSessionId.length === 0) {
            failConnection(qsTr("Wallet session is unavailable."))
            return false
        }
        callInFlight = true
        invoke("sessionInfo", [requestedSessionId], function (response) {
            callInFlight = false
            if (requestEpoch !== connectionEpoch || requestedSessionId !== sessionId) {
                return
            }
            if (!response.ok) {
                failConnection(response.error)
                return
            }
            const value = response.value && typeof response.value === "object" ? response.value : ({})
            if (value.active === false) {
                failConnection(qsTr("Wallet session is no longer active."))
                return
            }
            session = value
            accounts = Array.isArray(value.accounts) ? value.accounts.map(function (account) {
                return String(account || "")
            }).filter(function (account) {
                return account.length > 0
            }) : []
            grantedPermissions = Array.isArray(value.granted) ? value.granted.map(function (permission) {
                return String(permission || "")
            }) : []
            error = ""
            notice = qsTr("Wallet connected with %1 authorized accounts.").arg(accounts.length)
            appendOperation(qsTr("Wallet connection"), qsTr("connected"), notice, "success")
            if (afterLoad) {
                afterLoad()
            }
        })
        return true
    }

    function issueTransfer(action) {
        const requestedSessionId = sessionId
        const requestEpoch = connectionEpoch
        if (!connected || !hasPermission("send")) {
            failConnection(qsTr("Wallet transfer permission is required."))
            return false
        }
        if (accounts.indexOf(String(action.from || "")) < 0) {
            failConnection(qsTr("The sending account is not authorized for this wallet session."))
            return false
        }
        error = ""
        notice = qsTr("Approve the transfer in the Basecamp wallet.")
        callInFlight = true
        invoke("requestAction", [requestedSessionId, JSON.stringify(action)], function (response) {
            callInFlight = false
            if (requestEpoch !== connectionEpoch || requestedSessionId !== sessionId) {
                return
            }
            if (!response.ok) {
                failConnection(response.error)
                return
            }
            const requestId = response.value && response.value.requestId !== undefined
                ? String(response.value.requestId || "") : ""
            if (requestId.length === 0) {
                failConnection(qsTr("Wallet did not return a transfer request ID."))
                return
            }
            pendingRequestId = requestId
            pendingKind = "transfer"
            appendOperation(qsTr("Native transfer"), qsTr("pending"), qsTr("Awaiting wallet approval"), "warning")
        })
        return true
    }

    function pollTransferJob() {
        const jobId = pendingJobId
        const requestEpoch = connectionEpoch
        if (jobId.length === 0 || jobPollInFlight) {
            return false
        }
        jobPollInFlight = true
        invoke("getJob", [jobId], function (response) {
            jobPollInFlight = false
            if (requestEpoch !== connectionEpoch || jobId !== pendingJobId) {
                return
            }
            if (!response.ok) {
                failConnection(response.error)
                return
            }
            const value = response.value && typeof response.value === "object" ? response.value : ({})
            const state = String(value.state || "").toLowerCase()
            if (state === "running" || state === "pending" || state === "queued" || state.length === 0) {
                return
            }
            pendingJobId = ""
            transferResult = value
            if (String(value.error || "").length > 0 || state === "error" || state === "failed") {
                failConnection(String(value.error || qsTr("Wallet transaction failed.")))
                return
            }
            error = ""
            notice = String(value.txId || value.tx_id || qsTr("Wallet transaction completed."))
            appendOperation(qsTr("Native transfer"), qsTr("completed"), notice, "success")
        })
        return true
    }

    function nativeTransferAction(from, to, amount) {
        const sender = String(from || "").trim()
        const recipient = String(to || "").trim()
        const value = String(amount || "").trim()
        if (sender.length === 0 || recipient.length === 0 || value.length === 0) {
            return { ok: false, error: qsTr("Sender, recipient, and amount are required.") }
        }
        if (!/^[0-9]+$/.test(value)) {
            return { ok: false, error: qsTr("Amounts must be whole numbers.") }
        }
        return {
            ok: true,
            value: {
                op: "send",
                asset: "native",
                from: sender,
                to: recipient,
                amount: value
            }
        }
    }

    function applicationMetadata() {
        return {
            appName: "Logos Inspector",
            icon: "",
            origin: "logos-inspector"
        }
    }

    function invoke(method, args, callback) {
        if (!bridge) {
            callback(failedResponse(qsTr("Basecamp wallet bridge is unavailable.")))
            return false
        }
        if (typeof bridge.callModuleAsync === "function") {
            try {
                bridge.callModuleAsync(providerModule, method, args || [], function (response) {
                    callback(normalizeResponse(response))
                })
                return true
            } catch (exception) {
                callback(failedResponse(String(exception)))
                return false
            }
        }
        if (typeof bridge.callModule === "function") {
            Qt.callLater(function () {
                callback(normalizeResponse(bridge.callModule(providerModule, method, args || [])))
            })
            return true
        }
        callback(failedResponse(qsTr("Basecamp wallet bridge is unavailable.")))
        return false
    }

    function normalizeResponse(response) {
        if (!response || response.ok !== true) {
            return failedResponse(response && response.error
                ? String(response.error) : qsTr("Basecamp wallet call failed."))
        }
        let value = response.value
        if (typeof value === "string") {
            try {
                value = JSON.parse(value)
            } catch (exception) {
            }
        }
        if (value && typeof value === "object" && !Array.isArray(value)
                && String(value.error || "").length > 0) {
            return failedResponse(String(value.error))
        }
        return { ok: true, value: value, error: "" }
    }

    function failedResponse(message) {
        return { ok: false, value: null, error: String(message || qsTr("Basecamp wallet call failed.")) }
    }

    function failConnection(message) {
        pendingRequestId = ""
        pendingKind = ""
        pendingJobId = ""
        pendingTransfer = null
        error = String(message || qsTr("Wallet operation failed."))
        notice = ""
        appendOperation(qsTr("Wallet provider"), qsTr("failed"), error, "error")
    }

    function clearConnection() {
        sessionId = ""
        session = null
        accounts = []
        grantedPermissions = []
        pendingRequestId = ""
        pendingKind = ""
        pendingJobId = ""
        pendingTransfer = null
        transferResult = null
    }

    function appendOperation(label, status, detail, tone) {
        const rows = Array.isArray(operations) ? operations.slice(-49) : []
        rows.push({
            time: new Date().toLocaleTimeString(Qt.locale(), "hh:mm:ss"),
            label: String(label || ""),
            status: String(status || ""),
            detail: String(detail || ""),
            tone: String(tone || "neutral")
        })
        operations = rows
    }

    function providerDetail(value) {
        if (value && typeof value === "object") {
            if (value.cliFound === true) {
                return qsTr("Wallet CLI ready")
            }
            const status = String(value.status || value.state || "")
            const detail = String(value.detail || value.message || "")
            return detail.length > 0 ? detail : status
        }
        return String(value || "")
    }
}
