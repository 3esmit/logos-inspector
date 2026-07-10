pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/features/lez/pages"
import "../../qml/services"
import "../../qml/state"
import "../../qml/theme"
import "fixtures"

TestCase {
    id: testRoot

    name: "LezDiagnosticsPages"
    when: windowShown
    width: 1000
    height: 760

    property var activePage: null
    property alias appModel: model
    property alias uiTheme: theme

    BridgeHostFixture {
        id: fakeHost
    }

    BridgeClient {
        id: bridgeClient

        host: fakeHost
    }

    Theme {
        id: theme
    }

    AppModel {
        id: model

        bridge: bridgeClient
    }

    ApplicationWindow {
        id: testWindow

        width: testRoot.width
        height: testRoot.height
        visible: true
        color: theme.background

        Item {
            id: pageHost

            anchors.fill: parent
        }
    }

    Component {
        id: indexerPageComponent

        IndexerPage {
            theme: testRoot.uiTheme
            model: testRoot.appModel
            width: pageHost.width
        }
    }

    Component {
        id: sequencerPageComponent

        SequencerPage {
            theme: testRoot.uiTheme
            model: testRoot.appModel
            width: pageHost.width
        }
    }

    function init() {
        fakeHost.reset()
        fakeHost.strictUnexpectedCalls = true
        model.currentView = "overview"
        model.busy = false
        model.statusText = "Ready"
        model.resultTitle = "Output"
        model.resultText = ""
        model.resultValue = null
        model.resultIsError = false
        model.resultOwner = ""
        model.indexerTab = "status"
        model.sequencerTab = "blocks"
        model.capabilityRegistryLoaded = true
        model.capabilityRegistryReport = diagnosticsRegistry("available", [])
    }

    function cleanup() {
        if (activePage) {
            activePage.destroy()
            activePage = null
        }
    }

    function diagnosticsRegistry(status, unavailable, warnings) {
        return {
            schema_version: 1,
            capabilities: [{
                key: "diagnostics",
                label: "Diagnostics",
                status: status,
                sub_capabilities: ["diagnostics.lez.indexer.read", "diagnostics.lez.sequencer.read"],
                unavailable_sub_capabilities: unavailable || [],
                warnings: warnings || []
            }]
        }
    }

    function createIndexerPage() {
        activePage = indexerPageComponent.createObject(pageHost)
        verify(!!activePage)
        wait(0)
        return activePage
    }

    function createSequencerPage() {
        activePage = sequencerPageComponent.createObject(pageHost)
        verify(!!activePage)
        wait(0)
        return activePage
    }

    function waitForChild(parent, objectName) {
        let child = null
        tryVerify(function () {
            child = findChild(parent, objectName)
            return child !== null
        })
        verify(!!child, "Object exists")
        return child
    }

    function test_indexer_diagnostics_gate_allows_available_action() {
        fakeHost.responses = {
            indexerHealth: { ok: true, value: { ok: true }, text: "OK", error: "" }
        }

        const page = createIndexerPage()
        const button = waitForChild(page, "indexerDeepHealthButton")

        verify(page.diagnosticsGateEnabled())
        verify(button.enabled)
        mouseClick(button, button.width / 2, button.height / 2)

        compare(fakeHost.callCount, 1)
        compare(fakeHost.lastMethod, "indexerHealth")
    }

    function test_indexer_diagnostics_gate_blocks_unavailable_action_without_clearing_output() {
        model.capabilityRegistryReport = diagnosticsRegistry("degraded", ["diagnostics.lez.indexer.read"])
        model.setResult("Indexer head", "123", false, 123, "indexer")

        const page = createIndexerPage()
        const button = waitForChild(page, "indexerDeepHealthButton")
        const message = waitForChild(page, "indexerDiagnosticsGateMessage")

        verify(!page.diagnosticsGateEnabled())
        verify(!button.enabled)
        verify(message.visible)
        verify(message.message.indexOf("diagnostics.lez.indexer.read") >= 0)
        verify(model.pageHasOutput("indexer"))
        mouseClick(button, button.width / 2, button.height / 2)

        compare(fakeHost.callCount, 0)
        verify(model.pageHasOutput("indexer"))
        compare(model.resultTitle, "Indexer head")
    }

    function test_sequencer_diagnostics_gate_allows_available_action() {
        fakeHost.responses = {
            head: { ok: true, value: 42, text: "42", error: "" }
        }

        const page = createSequencerPage()
        const button = waitForChild(page, "sequencerHeaderHeadButton")

        verify(page.diagnosticsGateEnabled())
        verify(button.enabled)
        mouseClick(button, button.width / 2, button.height / 2)

        compare(fakeHost.callCount, 1)
        compare(fakeHost.lastMethod, "head")
    }

    function test_sequencer_diagnostics_gate_blocks_unavailable_action_without_clearing_output() {
        model.capabilityRegistryReport = diagnosticsRegistry("degraded", ["diagnostics.lez.sequencer.read"])
        model.setResult("Sequencer head", "42", false, 42, "sequencer")

        const page = createSequencerPage()
        const button = waitForChild(page, "sequencerHeaderHeadButton")
        const message = waitForChild(page, "sequencerDiagnosticsGateMessage")

        verify(!page.diagnosticsGateEnabled())
        verify(!button.enabled)
        verify(message.visible)
        verify(message.message.indexOf("diagnostics.lez.sequencer.read") >= 0)
        verify(model.pageHasOutput("sequencer"))
        mouseClick(button, button.width / 2, button.height / 2)

        compare(fakeHost.callCount, 0)
        verify(model.pageHasOutput("sequencer"))
        compare(model.resultTitle, "Sequencer head")
    }

    function test_degraded_gate_keeps_action_enabled_and_surfaces_warning() {
        model.capabilityRegistryReport = diagnosticsRegistry("degraded", [], ["Runtime diagnostics degraded."])

        const page = createSequencerPage()
        const button = waitForChild(page, "sequencerHeaderHeadButton")
        const message = waitForChild(page, "sequencerDiagnosticsGateMessage")

        verify(page.diagnosticsGateEnabled())
        verify(button.enabled)
        verify(message.visible)
        verify(message.message.indexOf("Runtime diagnostics degraded.") >= 0)
    }
}
