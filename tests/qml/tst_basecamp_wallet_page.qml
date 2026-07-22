import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/features/wallet/pages"
import "../../qml/services"
import "../../qml/state"
import "../../qml/theme"

TestCase {
    id: testRoot

    name: "BasecampWalletPage"
    when: windowShown
    width: 900
    height: 700

    QtObject {
        id: basecampHost

        property var calls: []

        function reset() {
            calls = []
        }

        function callModule(moduleName, method, args) {
            return JSON.stringify({ status: "ready" })
        }

        function callModuleAsync(moduleName, method, args, callback) {
            calls.push({ module: String(moduleName), method: String(method), args: args || [] })
            let value = ({ status: "ready" })
            if (String(method) === "connectRequest") {
                value = { requestId: "connect-request" }
            }
            Qt.callLater(function () {
                callback(JSON.stringify(value))
            })
        }
    }

    BridgeClient {
        id: bridge

        host: basecampHost
    }

    Theme {
        id: theme
    }

    AppModel {
        id: model

        bridge: bridge
    }

    ApplicationWindow {
        id: testWindow

        width: testRoot.width
        height: testRoot.height
        visible: true
        color: theme.background

        WalletPage {
            id: page

            theme: theme
            model: model
            width: testWindow.width
        }
    }

    function init() {
        basecampHost.reset()
        model.basecampWallet.connectionEpoch += 1
        model.basecampWallet.clearConnection()
        model.basecampWallet.error = ""
        model.basecampWallet.notice = ""
        model.basecampWallet.availability = "unknown"
        model.basecampWallet.availabilityDetail = ""
        model.basecampWallet.operations = []
        model.basecampWallet.callInFlight = false
        model.basecampWallet.pendingPollInFlight = false
        model.basecampWallet.jobPollInFlight = false
        model.basecampWalletTab = "provider"
    }

    function waitForChild(parent, objectName) {
        let child = null
        tryVerify(function () {
            child = findChild(parent, objectName)
            return child !== null && child.visible
        })
        verify(!!child, "Object exists")
        return child
    }

    function test_basecamp_host_selects_provider_page_not_local_wallet_page() {
        const connect = waitForChild(page, "basecampWalletConnectButton")
        verify(connect.enabled)
        verify(findChild(page, "createAccountButton") === null)

        mouseClick(connect, connect.width / 2, connect.height / 2)
        tryVerify(function () {
            return basecampHost.calls.some(function (call) {
                return call.module === "medusa_core" && call.method === "connectRequest"
            })
        })
        const request = basecampHost.calls.filter(function (call) {
            return call.method === "connectRequest"
        })[0]
        compare(JSON.parse(String(request.args[1])), ["accounts"])
    }

    function test_transfer_tab_requires_explicit_confirmation() {
        model.basecampWalletTab = "transfer"
        const transfer = waitForChild(page, "basecampWalletTransferButton")
        compare(transfer.enabled, false)

        const providerPage = page.loadedPage
        verify(!!providerPage)
        providerPage.transferFrom = "Public/sender"
        providerPage.transferTo = "Public/recipient"
        providerPage.transferAmount = "17"
        tryCompare(transfer, "enabled", true)

        const popup = findChild(page, "basecampWalletTransferConfirm")
        verify(!!popup)
        compare(popup.opened, false)
        mouseClick(transfer, transfer.width / 2, transfer.height / 2)
        tryCompare(popup, "opened", true)
        compare(popup.title, qsTr("Request wallet transfer"))
    }
}
