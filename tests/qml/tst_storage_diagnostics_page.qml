pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/features/storage/pages"
import "../../qml/services"
import "../../qml/state"
import "../../qml/theme"
import "fixtures"

TestCase {
    id: testRoot

    name: "StorageDiagnosticsPage"
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

    ApplicationWindow {
        id: testWindow

        visible: true
        width: testRoot.width
        height: testRoot.height
        color: theme.background

        Loader {
            id: pageLoader

            sourceComponent: model.shell.currentView === "storage"
                ? storageWorkspaceComponent : storageDiagnosticsComponent
            width: testWindow.width
        }
    }

    Component {
        id: storageDiagnosticsComponent

        StoragePage {
            theme: theme
            model: model
            width: testWindow.width
        }
    }

    Component {
        id: storageWorkspaceComponent

        StorageAppPage {
            theme: theme
            model: model.storageApp
            width: testWindow.width
        }
    }

    function init() {
        fakeHost.reset()
        model.metrics.storageRefreshRate = 0
        model.storageAppTab = "files"
        model.storageDiagnosticsTab = "diagnostics"
        model.storageCidProbe = "z-selected-diagnostic-cid"
        model.metrics.storageSourceReport = null
        model.shell.currentView = "diagnosticsStorage"
        model.navigationBackStack = []
        model.navigationForwardStack = []
        tryVerify(function () { return pageLoader.item !== null })
    }

    function test_live_storage_workflow_routes_replace_dead_placeholders() {
        let cidTools = null
        let transferTools = null
        tryVerify(function () {
            cidTools = findAccessibleByName(pageLoader.item, "Open Storage CID tools")
            transferTools = findAccessibleByName(pageLoader.item, "Open Storage transfer tools")
            return cidTools !== null && transferTools !== null
        })

        verify(cidTools.enabled)
        verify(transferTools.enabled)
        verify(findAccessibleByName(pageLoader.item, "Manifest fetch") === null)
        verify(findAccessibleByName(pageLoader.item, "Provider lookup") === null)
        verify(findAccessibleByName(pageLoader.item, "Download probe") === null)

        mouseClick(cidTools, cidTools.width / 2, cidTools.height / 2)

        compare(model.storageAppTab, "cid")
        compare(model.storageCidProbe, "z-selected-diagnostic-cid")
        compare(model.shell.currentView, "storage")
        verify(model.canNavigateBack())
        tryVerify(function () {
            return findAccessibleByName(pageLoader.item, "CID selected") !== null
        })

        model.navigateBack()
        compare(model.shell.currentView, "diagnosticsStorage")
        compare(model.storageAppTab, "files")
        compare(model.storageDiagnosticsTab, "diagnostics")

        tryVerify(function () {
            transferTools = findAccessibleByName(
                pageLoader.item, "Open Storage transfer tools")
            return transferTools !== null
        })

        mouseClick(transferTools, transferTools.width / 2, transferTools.height / 2)

        compare(model.storageAppTab, "transfer")
        compare(model.storageCidProbe, "z-selected-diagnostic-cid")
        compare(model.shell.currentView, "storage")
        tryVerify(function () {
            return findAccessibleByName(pageLoader.item, "Transfer selected") !== null
        })
    }

    function test_topology_exposes_structured_network_debug_rows() {
        const debug = {
            id: "debug-peer-id",
            addrs: ["/ip4/127.0.0.1/tcp/8070"],
            announceAddresses: ["/dns4/storage.test/tcp/443/wss"],
            libp2pPubKey: "debug-public-key",
            mixPubKey: null,
            providerRecord: "debug-provider-record",
            spr: "debug-self-peer-record",
            storage: { version: "0.1.0-test", revision: "debug-revision" },
            table: {
                localNode: {
                    peerId: "debug-peer-id",
                    address: "/ip4/127.0.0.1/tcp/8070",
                    nodeId: "debug-local-node"
                },
                nodes: [{
                    peerId: "routing-peer-1",
                    address: "/ip4/10.0.0.1/tcp/3000",
                    nodeId: "routing-node-1"
                }]
            }
        }
        tryVerify(function () {
            return !model.metrics.networkConnectionIsPending("storage")
        })
        model.metrics.setSourceReport("storage", {
            health: {
                ready: true,
                status: "healthy",
                summary: "source ready",
                detail: "ready"
            },
            probes: [{
                probe_key: "debug",
                label: "storage_module.debug",
                ok: true,
                value: debug,
                error: null
            }]
        }, { origin: "test" })
        model.storageDiagnosticsTab = "topology"

        tryVerify(function () {
            return findAccessibleByName(
                pageLoader.item,
                "Copy 9 field(s); 1 routing node(s)") !== null
        })
        verify(findAccessibleByName(pageLoader.item, "Copy debug-peer-id") !== null)
        verify(findAccessibleByName(
            pageLoader.item,
            "Copy /ip4/127.0.0.1/tcp/8070") !== null)
        verify(findAccessibleByName(
            pageLoader.item,
            "Copy 1 node(s); showing 1") !== null)
        verify(findAccessibleByName(
            pageLoader.item,
            "Copy routing-peer-1 | /ip4/10.0.0.1/tcp/3000 | routing-node-1") !== null)
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
        for (let i = 0; i < children.length; ++i) {
            const match = findAccessibleByName(children[i], expectedName)
            if (match) {
                return match
            }
        }
        return null
    }

}
