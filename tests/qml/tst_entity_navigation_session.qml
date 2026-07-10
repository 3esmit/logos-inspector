import QtQuick
import QtTest
import "../../qml/state"

TestCase {
    id: testRoot

    name: "EntityNavigationSession"

    QtObject {
        id: chainPagesFixture

        function transferRecipientDetailById(value) {
            const id = String(value || "")
            return id === "recipient-1" ? { type: "transfer_recipient", recipient: id, amount: "12" } : null
        }

        function channelDetail(channel) {
            const value = channel || {}
            const id = String(value.channel_id || value.channel || "")
            return id.length ? { type: "channel", channel_id: id, source: String(value.source || "") } : null
        }

        function channelDetailById(value) {
            const id = String(value || "")
            return id === "channel-1" ? { type: "channel", channel_id: id, source: "fixture" } : null
        }
    }

    QtObject {
        id: fakeModel

        property string inspectorModule: "logos_inspector"
        property string currentView: "overview"
        property string statusText: ""
        property string accountTab: ""
        property string localWalletTab: "profiles"
        property string localWalletLookupTarget: ""
        property string channelDetailError: ""
        property string resultTitle: ""
        property string resultText: ""
        property bool resultIsError: false
        property string resultOwner: ""
        property int searchResolveSerial: 0
        property int navigationPushCount: 0
        property var accountDetailValue: null
        property var transferRecipientDetailValue: null
        property var channelDetailValue: null
        property var resultValue: null
        property var asyncRequests: []
        property var chainPages: chainPagesFixture
        property string appliedLezTarget: ""

        function reset() {
            currentView = "overview"
            statusText = ""
            accountTab = ""
            localWalletTab = "profiles"
            localWalletLookupTarget = ""
            channelDetailError = ""
            resultTitle = ""
            resultText = ""
            resultIsError = false
            resultOwner = ""
            searchResolveSerial = 0
            navigationPushCount = 0
            accountDetailValue = null
            transferRecipientDetailValue = null
            channelDetailValue = null
            resultValue = null
            asyncRequests = []
            appliedLezTarget = ""
        }

        function valueToString(value) {
            return value === undefined || value === null ? "" : String(value)
        }

        function pushNavigationHistory() {
            navigationPushCount += 1
        }

        function selectView(view, recordHistory) {
            currentView = String(view || "")
        }

        function setResult(title, text, isError, value, owner) {
            resultTitle = String(title || "")
            resultText = String(text || "")
            resultIsError = isError === true
            resultValue = value === undefined ? null : value
            resultOwner = String(owner || "")
        }

        function walletProfileConfigured() {
            return false
        }

        function checkLocalWalletProfile(showResult) {
        }

        function accountLookupArgs(account) {
            return [String(account || "")]
        }

        function lezLookupArgs(target) {
            return [String(target || "")]
        }

        function requestModuleAsync(moduleName, method, args, label, showResult, callback, acceptResponse) {
            const requests = asyncRequests.slice()
            requests.push({
                moduleName: String(moduleName || ""),
                method: String(method || ""),
                args: Array.isArray(args) ? args : [],
                callback: callback
            })
            asyncRequests = requests
            return "request-" + requests.length
        }

        function applyResolvedLezTarget(response, errorTitle) {
            if (!response || response.ok !== true) {
                return false
            }
            appliedLezTarget = String(response.value && response.value.kind ? response.value.kind : "")
            setResult(String(errorTitle || ""), "applied " + appliedLezTarget, false, response.value)
            return true
        }
    }

    EntityNavigationSession {
        id: session

        model: fakeModel
    }

    function init() {
        fakeModel.reset()
    }

    function respond(index, response) {
        fakeModel.asyncRequests[index].callback(response)
    }

    function test_open_reference_dispatches_private_account_to_wallet_sync() {
        session.openReference("privateAccount", "account-1")

        compare(fakeModel.currentView, "localWallet")
        compare(fakeModel.localWalletTab, "privateSync")
        compare(fakeModel.localWalletLookupTarget, "Private/account-1")
        compare(fakeModel.resultTitle, "Private account reference")
        compare(fakeModel.resultValue.account_id, "Private/account-1")
        verify(fakeModel.resultIsError)
    }

    function test_open_reference_dispatches_recipient_detail() {
        session.openReference("recipient", "recipient-1")

        compare(fakeModel.currentView, "transferActivity")
        compare(fakeModel.transferRecipientDetailValue.recipient, "recipient-1")
        compare(fakeModel.resultTitle, "Transfer recipient")
        verify(!fakeModel.resultIsError)
    }

    function test_open_reference_dispatches_payload_backed_channel() {
        session.openReference("channel", "ignored-channel", { channel_id: "channel-1", source: "fixture" })

        compare(fakeModel.currentView, "channels")
        compare(fakeModel.channelDetailValue.channel_id, "channel-1")
        compare(fakeModel.channelDetailValue.source, "fixture")
        compare(fakeModel.channelDetailError, "")
        compare(fakeModel.resultTitle, "Channel")
        verify(!fakeModel.resultIsError)
    }

    function test_stale_account_open_response_is_ignored() {
        session.openReference("account", "Public/first")
        session.openReference("account", "Public/second")

        compare(fakeModel.asyncRequests.length, 2)
        compare(fakeModel.asyncRequests[0].method, "account")
        compare(fakeModel.asyncRequests[1].method, "account")
        compare(fakeModel.searchResolveSerial, 2)

        respond(0, {
            ok: true,
            value: { account_id: "Public/first" },
            text: "first",
            error: ""
        })

        compare(fakeModel.accountDetailValue, null)
        compare(fakeModel.resultTitle, "")

        respond(1, {
            ok: true,
            value: { account_id: "Public/second" },
            text: "second",
            error: ""
        })

        compare(fakeModel.accountDetailValue.account_id, "Public/second")
        compare(fakeModel.resultTitle, "Account lookup")
        compare(fakeModel.resultText, "second")
        verify(!fakeModel.resultIsError)
    }

    function test_stale_search_hash_response_is_ignored() {
        session.resolveSearchHash("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        session.resolveSearchHash("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")

        compare(fakeModel.asyncRequests.length, 2)
        compare(fakeModel.asyncRequests[0].method, "resolveLezTarget")
        compare(fakeModel.asyncRequests[1].method, "resolveLezTarget")
        compare(fakeModel.searchResolveSerial, 2)

        respond(0, {
            ok: true,
            value: { kind: "first" },
            text: "first",
            error: ""
        })

        compare(fakeModel.appliedLezTarget, "")
        compare(fakeModel.resultTitle, "")

        respond(1, {
            ok: true,
            value: { kind: "second" },
            text: "second",
            error: ""
        })

        compare(fakeModel.appliedLezTarget, "second")
        compare(fakeModel.resultTitle, "Search")
        compare(fakeModel.resultText, "applied second")
        verify(!fakeModel.resultIsError)
    }
}
