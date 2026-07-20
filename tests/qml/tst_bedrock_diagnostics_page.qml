pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/features/bedrock/pages"
import "../../qml/services"
import "../../qml/state"
import "../../qml/theme"
import "fixtures"

TestCase {
    id: testRoot

    name: "BedrockDiagnosticsPage"
    when: windowShown
    width: 1280
    height: 900

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

    Component {
        id: pageComponent

        BedrockDiagnosticsPage {
            theme: theme
            model: model
        }
    }

    ApplicationWindow {
        id: testWindow

        visible: true
        width: testRoot.width
        height: testRoot.height
        color: theme.background

        Loader {
            id: pageLoader

            anchors.fill: parent
            active: false
            sourceComponent: pageComponent
        }
    }

    function healthyReport() {
        return {
            endpoint: "http://bedrock.test:8080/",
            cryptarchia_info: {
                ok: true,
                source: "/cryptarchia/info",
                value: {
                    cryptarchia_info: {
                        mode: "Online",
                        slot: 42,
                        lib_slot: 40,
                        tip: "a".repeat(64),
                        lib: "b".repeat(64)
                    }
                },
                error: ""
            },
            headers: {
                ok: true,
                source: "/cryptarchia/headers",
                value: [],
                error: ""
            },
            network_info: {
                ok: true,
                source: "/network/info",
                value: { n_peers: 3 },
                error: ""
            },
            mantle_metrics: {
                ok: true,
                source: "/mantle/metrics",
                value: { blocks: 7 },
                error: ""
            }
        }
    }

    function setObservation(report, status) {
        model.metrics.setSourceReport("blockchain", report, {
            origin: "test",
            checkedAtMs: 1
        })
        model.metrics.networkConnectionStatus = ({ blockchain: status })
        model.metrics.networkConnectionStatusRevision += 1
    }

    function completedNodeOperation(args) {
        const request = args && args[0] ? args[0] : ({})
        const sourceArgs = Array.isArray(request.args) ? request.args : []
        const first = String(sourceArgs[0] || "")
        const context = {
            source: "rpc",
            configurationGeneration: Number(request.configurationGeneration || 0),
            endpoint: first
        }
        return {
            ok: true,
            value: {
                operationId: "bedrock-diagnostics-" + String(request.clientRequestId || "test"),
                clientRequestId: request.clientRequestId,
                domain: "blockchain",
                backend: "rpc",
                method: request.method,
                label: request.label,
                status: "completed",
                eventCursor: 1,
                context: context,
                result: healthyReport(),
                error: ""
            },
            text: "OK",
            error: ""
        }
    }

    function init() {
        pageLoader.active = false
        fakeHost.reset()
        model.metrics.invalidateConfiguration("blockchain", "test reset")
        model.metrics.networkConnectionStatus = ({})
        model.metrics.networkConnectionStatusRevision += 1
        model.networkConnectorConfig = model.defaultNetworkConnectorConfig()
        model.nodeUrl = "http://bedrock.test:8080/"
        model.blockchainSourceMode = "rpc"
        model.shell.currentView = "diagnosticsBedrock"
    }

    function test_cached_report_is_structured_and_settings_are_reachable() {
        setObservation(healthyReport(), {
            known: true,
            ok: true,
            transportOk: true,
            text: "OK",
            detail: "slot 42",
            checkedAt: "10:01:02",
            checkedAtMs: 1,
            stale: false
        })
        pageLoader.active = true

        tryVerify(function () { return pageLoader.item !== null })
        verify(findAccessibleByName(pageLoader.item,
                "Bedrock source reachable. All reported connection checks completed successfully.") !== null)
        verify(findAccessibleByName(pageLoader.item,
                "Cryptarchia information: available. /cryptarchia/info: 1 field(s)") !== null)
        verify(findAccessibleByName(pageLoader.item,
                "Observed tip slot: 42") !== null)
        verify(findAccessibleByName(pageLoader.item,
                "Observed LIB slot: 40") !== null)
        verify(findAccessibleByName(pageLoader.item,
                "Target: http://bedrock.test:8080/") !== null)

        const settings = findAccessibleByName(pageLoader.item, "Open Bedrock settings")
        verify(settings !== null)
        mouseClick(settings, settings.width / 2, settings.height / 2)
        compare(model.shell.currentView, "settings")
        compare(model.shell.settingsSection, "network")
        compare(model.shell.settingsNetworkSection, "blockchain")
    }

    function test_opening_without_a_cached_report_refreshes_the_source() {
        fakeHost.responses = {
            runtimeOperationStart: completedNodeOperation
        }
        pageLoader.active = true

        tryVerify(function () {
            return model.metrics.sourceObservation("blockchain").sourceReport !== null
                && !model.metrics.sourceObservation("blockchain").pending
        })
        const starts = fakeHost.calls.filter(function (call) {
            const request = call.method === "runtimeOperationStart" && call.args
                ? call.args[0] || null : null
            return request && String(request.method || "") === "blockchainNode"
        })
        compare(starts.length, 1)
        verify(findAccessibleByName(pageLoader.item,
                "Bedrock source reachable. All reported connection checks completed successfully.") !== null)
    }

    function test_stale_report_and_connector_limits_are_explained() {
        const report = healthyReport()
        report.headers = {
            ok: false,
            source: "blockchain_module",
            value: null,
            error: "blockchain_module does not expose header-list reads"
        }
        report.network_info = {
            ok: false,
            source: "blockchain_module",
            value: null,
            error: "blockchain_module does not expose network-info reads"
        }
        report.mantle_metrics = {
            ok: false,
            source: "blockchain_module",
            value: null,
            error: "blockchain_module does not expose Mantle metrics"
        }
        model.networkConnectorConfig = {
            scopes: Object.assign({}, model.networkConnectorConfig.scopes, {
                l1: {
                    connector_id: "logoscore_cli_blockchain_module",
                    source_mode: "logoscore_cli",
                    provenance: "test"
                }
            })
        }
        model.blockchainSourceMode = "logoscore_cli"
        setObservation(report, {
            known: true,
            ok: true,
            transportOk: true,
            text: "OK",
            detail: "slot 42",
            checkedAt: "10:01:02",
            checkedAtMs: 2,
            stale: false
        })
        pageLoader.active = true

        tryVerify(function () { return pageLoader.item !== null })
        verify(findAccessibleByName(pageLoader.item,
                "Connector capability limits. Some read-only Bedrock APIs are unavailable through the selected connector.") !== null)
        verify(findAccessibleByName(pageLoader.item,
                "Block headers: unavailable. blockchain_module does not expose header-list reads") !== null)
        verify(findAccessibleByName(pageLoader.item,
                "Transport: LogosCore CLI") !== null)

        model.metrics.networkConnectionStatus = ({
            blockchain: {
                known: true,
                ok: false,
                transportOk: false,
                text: "Error",
                detail: "connection refused",
                checkedAt: "10:01:03",
                checkedAtMs: 3,
                stale: true
            }
        })
        model.metrics.networkConnectionStatusRevision += 1

        tryVerify(function () {
            return findAccessibleByPrefix(pageLoader.item,
                "Latest check failed. Showing the last completed report from ") !== null
        })
    }

    function findAccessibleByName(item, expectedName) {
        if (!item) {
            return null
        }
        if (item.Accessible && String(item.Accessible.name || "") === expectedName
                && item.visible) {
            return item
        }
        const children = item.children || []
        for (let index = 0; index < children.length; ++index) {
            const match = findAccessibleByName(children[index], expectedName)
            if (match) {
                return match
            }
        }
        return null
    }

    function findAccessibleByPrefix(item, expectedPrefix) {
        if (!item) {
            return null
        }
        if (item.Accessible && String(item.Accessible.name || "").indexOf(expectedPrefix) === 0
                && item.visible) {
            return item
        }
        const children = item.children || []
        for (let index = 0; index < children.length; ++index) {
            const match = findAccessibleByPrefix(children[index], expectedPrefix)
            if (match) {
                return match
            }
        }
        return null
    }
}
