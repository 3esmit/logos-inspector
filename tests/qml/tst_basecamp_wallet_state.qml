import QtQuick
import QtTest
import "../../qml/state/wallet"

TestCase {
    id: testRoot

    name: "BasecampWalletState"

    QtObject {
        id: provider

        property var calls: []
        property bool connectApproved: false
        property bool actionApproved: false
        property int jobPolls: 0
        property var currentPermissions: []
        property bool statusFails: false
        property bool cliFound: true

        function reset() {
            calls = []
            connectApproved = false
            actionApproved = false
            jobPolls = 0
            currentPermissions = []
            statusFails = false
            cliFound = true
        }

        function callModuleAsync(moduleName, method, args, callback) {
            calls.push({ module: String(moduleName), method: String(method), args: args || [] })
            let value = ({})
            switch (String(method)) {
            case "getStatus":
                value = statusFails ? { error: "wallet module is unavailable" } : {
                    cliFound: cliFound,
                    cliPath: cliFound ? "/provider/bin/wallet" : ""
                }
                break
            case "connectRequest":
                currentPermissions = JSON.parse(String(args[1] || "[]"))
                value = { requestId: "connect-request" }
                break
            case "actionStatus":
                if (String(args[0]) === "connect-request") {
                    value = connectApproved
                        ? { status: "approved", sessionId: "session-1" }
                        : { status: "pending" }
                } else {
                    value = actionApproved
                        ? { status: "approved", jobId: "job-1" }
                        : { status: "pending" }
                }
                break
            case "sessionInfo":
                value = {
                    active: true,
                    accounts: ["Public/sender"],
                    granted: currentPermissions,
                    zone: "0101"
                }
                break
            case "requestAction":
                value = { requestId: "transfer-request" }
                break
            case "getJob":
                jobPolls += 1
                value = jobPolls === 1
                    ? { state: "running" }
                    : { state: "done", txId: "tx-123" }
                break
            case "revokeSession":
                value = { ok: true }
                break
            default:
                value = { error: "unexpected method: " + method }
                break
            }
            Qt.callLater(function () {
                callback({ ok: true, value: value, error: "" })
            })
        }
    }

    BasecampWalletState {
        id: wallet

        bridge: provider
        pollIntervalMs: 100000
    }

    function init() {
        provider.reset()
        wallet.connectionEpoch += 1
        wallet.clearConnection()
        wallet.error = ""
        wallet.notice = ""
        wallet.availability = "unknown"
        wallet.availabilityDetail = ""
        wallet.operations = []
        wallet.callInFlight = false
        wallet.pendingPollInFlight = false
        wallet.jobPollInFlight = false
    }

    function callsFor(method) {
        return provider.calls.filter(function (call) {
            return call.method === method
        })
    }

    function approveConnect() {
        provider.connectApproved = true
        wallet.pollPendingRequest()
        tryVerify(function () {
            return wallet.connected && wallet.sessionId === "session-1"
        })
    }

    function test_availability_treats_provider_error_as_failure() {
        provider.statusFails = true

        verify(wallet.checkAvailability())
        tryCompare(wallet, "availability", "unavailable")
        compare(wallet.availabilityDetail, "wallet module is unavailable")
    }

    function test_availability_requires_the_wallet_cli() {
        provider.cliFound = false

        verify(wallet.checkAvailability())
        tryCompare(wallet, "availability", "unavailable")
        compare(wallet.availabilityDetail, qsTr("Wallet CLI is unavailable."))
    }

    function test_availability_reports_a_ready_wallet_cli() {
        verify(wallet.checkAvailability())
        tryCompare(wallet, "availability", "available")
        compare(wallet.availabilityDetail, qsTr("Wallet CLI ready"))
    }

    function test_connect_requests_only_accounts_and_waits_for_wallet_approval() {
        verify(wallet.connectAccounts())
        tryCompare(wallet, "pendingRequestId", "connect-request")
        compare(wallet.pendingKind, "connect")
        compare(callsFor("connectRequest").length, 1)
        const request = callsFor("connectRequest")[0]
        compare(request.module, "medusa_core")
        compare(JSON.parse(String(request.args[1])), ["accounts"])

        wallet.pollPendingRequest()
        wait(0)
        compare(wallet.pendingRequestId, "connect-request")
        verify(!wallet.connected)

        approveConnect()
        compare(wallet.accounts, ["Public/sender"])
        verify(!wallet.hasPermission("send"))
        verify(wallet.hasPermission("accounts"))
    }

    function test_native_transfer_escalates_permissions_and_waits_for_two_wallet_steps() {
        verify(wallet.connectAccounts())
        tryCompare(wallet, "pendingRequestId", "connect-request")
        approveConnect()

        verify(wallet.startNativeTransfer("Public/sender", "Public/recipient", "17"))
        tryVerify(function () {
            return callsFor("revokeSession").length === 1
        })
        tryCompare(wallet, "pendingRequestId", "connect-request")
        compare(JSON.parse(String(callsFor("connectRequest")[1].args[1])), ["accounts", "send"])

        approveConnect()
        tryCompare(wallet, "pendingRequestId", "transfer-request")
        compare(wallet.pendingKind, "transfer")
        const action = JSON.parse(String(callsFor("requestAction")[0].args[1]))
        compare(action, {
            op: "send",
            asset: "native",
            from: "Public/sender",
            to: "Public/recipient",
            amount: "17"
        })

        provider.actionApproved = true
        wallet.pollPendingRequest()
        tryCompare(wallet, "pendingJobId", "job-1")

        wallet.pollTransferJob()
        wait(0)
        compare(wallet.pendingJobId, "job-1")
        wallet.pollTransferJob()
        tryCompare(wallet, "pendingJobId", "")
        compare(wallet.transferResult.txId, "tx-123")
        compare(wallet.notice, "tx-123")
        verify(wallet.operations.some(function (operation) {
            return operation.label === qsTr("Native transfer")
                && operation.status === qsTr("completed")
        }))
    }

    function test_disconnect_forgets_session_and_requests_provider_revocation() {
        verify(wallet.connectAccounts())
        tryCompare(wallet, "pendingRequestId", "connect-request")
        approveConnect()

        verify(wallet.disconnect())
        tryVerify(function () {
            return callsFor("revokeSession").length === 1
        })
        tryCompare(wallet, "sessionId", "")
        compare(wallet.accounts, [])
        compare(wallet.grantedPermissions, [])
    }
}
