import QtQuick
import QtQml.Models
import "../services/BridgeHelpers.js" as BridgeHelpers
import "../services"

QtObject {
    id: root

    required property BridgeClient bridge

    readonly property string inspectorModule: "logos_inspector"
    readonly property string blockchainModule: "blockchain_module"
    readonly property string storageModule: "storage_module"
    readonly property string deliveryModule: "delivery_module"
    readonly property string capabilityModule: "capability_module"

    property string currentView: "overview"
    property string statusText: qsTr("Ready")
    property bool busy: false
    property string resultTitle: qsTr("Result")
    property string resultText: ""
    property bool resultIsError: false

    property string networkProfile: "default"
    property string sequencerUrl: "https://testnet.lez.logos.co/"
    property string indexerUrl: "http://127.0.0.1:8779/"
    property string nodeUrl: "https://testnet.lez.logos.co/"

    property string sequencerTab: "blocks"
    property string accountTab: "lookup"
    property string programTab: "idls"
    property string indexerTab: "status"

    property ListModel registeredIdls: ListModel {}

    property ListModel navItems: ListModel {
        ListElement { key: "overview"; label: "Dashboard" }
        ListElement { key: "blockchain"; label: "Blockchain" }
        ListElement { key: "channels"; label: "Channels" }
        ListElement { key: "storage"; label: "Storage" }
        ListElement { key: "messaging"; label: "Messaging" }
        ListElement { key: "capabilities"; label: "Capabilities" }
        ListElement { key: "sequencer"; label: "Sequencer" }
        ListElement { key: "accounts"; label: "Accounts" }
        ListElement { key: "programs"; label: "SPEL" }
        ListElement { key: "indexer"; label: "Indexer" }
        ListElement { key: "settings"; label: "Settings" }
    }

    function viewTitle() {
        for (let i = 0; i < navItems.count; ++i) {
            const item = navItems.get(i)
            if (item.key === currentView) {
                return item.label
            }
        }
        return qsTr("Dashboard")
    }

    function selectView(view) {
        currentView = view
        statusText = qsTr("Ready")
    }

    function clearResult() {
        resultTitle = qsTr("Result")
        resultText = ""
        resultIsError = false
    }

    function setResult(title, text, isError) {
        resultTitle = title
        resultText = text
        resultIsError = isError
        statusText = isError ? qsTr("Error") : qsTr("Ready")
    }

    function callInspector(method, args, label) {
        callModule(inspectorModule, method, args, label)
    }

    function callModule(moduleName, method, args, label) {
        if (busy) {
            return
        }

        const targetModule = moduleName === inspectorModule ? moduleName : inspectorModule
        const targetMethod = moduleName === inspectorModule ? method : "callModule"
        const targetArgs = moduleName === inspectorModule ? args : [moduleName, method, args || []]

        busy = true
        statusText = label
        const response = bridge.callModule(targetModule, targetMethod, targetArgs)
        busy = false

        if (response.ok) {
            setResult(label, response.text, false)
        } else {
            setResult(label, response.error, true)
        }
    }

    function routeSearch(query) {
        const value = query.trim()
        if (!value.length) {
            return
        }

        if (/^[0-9]+$/.test(value)) {
            currentView = "sequencer"
            sequencerTab = "blocks"
            callInspector("block", [sequencerUrl, Number(value)], qsTr("Block lookup"))
            return
        }

        if (/^(0x)?[0-9a-fA-F]{64}$/.test(value)) {
            currentView = "sequencer"
            sequencerTab = "transactions"
            callInspector("transaction", [sequencerUrl, value], qsTr("Transaction lookup"))
            return
        }

        currentView = "accounts"
        accountTab = "lookup"
        callInspector("account", [sequencerUrl, indexerUrl, value], qsTr("Account lookup"))
    }

    function registerIdl(name, programId, json) {
        if (!json.trim().length) {
            setResult(qsTr("IDL registry"), qsTr("IDL JSON is required."), true)
            return
        }

        const parsed = BridgeHelpers.parseJson(json)
        if (!parsed.ok) {
            setResult(qsTr("IDL registry"), qsTr("Invalid IDL JSON: %1").arg(parsed.error), true)
            return
        }

        const idl = parsed.value
        const resolvedName = name.trim().length ? name.trim() : (idl.name || qsTr("IDL %1").arg(registeredIdls.count + 1))
        registeredIdls.append({
            name: resolvedName,
            programId: programId.trim(),
            json: json
        })
        setResult(qsTr("IDL registry"), qsTr("Saved %1.").arg(resolvedName), false)
    }

    function removeIdl(index) {
        registeredIdls.remove(index)
    }

    function applyProfile(index) {
        if (index === 2) {
            networkProfile = "local"
            sequencerUrl = "http://127.0.0.1:3040/"
            indexerUrl = "http://127.0.0.1:8779/"
            nodeUrl = "http://127.0.0.1:3040/"
            return
        }

        networkProfile = index === 1 ? "testnet-indexer-local" : "default"
        sequencerUrl = "https://testnet.lez.logos.co/"
        indexerUrl = "http://127.0.0.1:8779/"
        nodeUrl = "https://testnet.lez.logos.co/"
    }
}
