import QtQuick
import QtTest
import "../../qml/state"

TestCase {
    id: testRoot

    name: "EntityNavigationSession"

    QtObject {
        id: fakeModel

        property string currentView: "overview"
        property string localWalletTab: "profiles"
        property string localWalletLookupTarget: ""
        property string routedQuery: ""
        property var resultValue: null

        function valueToString(value) { return value === undefined || value === null ? "" : String(value) }
        function pushNavigationHistory() {}
        function selectView(view, recordHistory) { currentView = String(view || "") }
        function setResult(title, text, isError, value, owner) { resultValue = value }
        function walletProfileConfigured() { return false }
        function checkLocalWalletProfile(showResult) {}
        function routeSearch(query) { routedQuery = String(query || "") }
    }

    EntityNavigationSession {
        id: session

        model: fakeModel
    }

    function init() {
        fakeModel.currentView = "overview"
        fakeModel.localWalletTab = "profiles"
        fakeModel.localWalletLookupTarget = ""
        fakeModel.routedQuery = ""
        fakeModel.resultValue = null
    }

    function test_private_account_opens_local_wallet_sync() {
        session.openReference("privateAccount", "account-1")

        compare(fakeModel.currentView, "localWallet")
        compare(fakeModel.localWalletTab, "privateSync")
        compare(fakeModel.localWalletLookupTarget, "Private/account-1")
        compare(fakeModel.resultValue.account_id, "Private/account-1")
    }

}
