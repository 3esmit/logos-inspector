pragma ComponentBehavior: Bound

import QtQuick
import QtTest
import "../../qml/features/local/pages"
import "../../qml/state"
import "../../qml/theme"
import "fixtures"

Item {
    id: root

    width: 1280
    height: 1200

    Theme {
        id: theme
    }

    StateGatewayFixture {
        id: gateway
    }

    LocalNodesState {
        id: state

        gateway: gateway
        networkProfile: "default"
        busy: gateway.busy
    }

    Component {
        id: pageComponent

        LocalNodesPage {
            theme: theme
            model: state
            width: root.width
        }
    }

    TestCase {
        id: testCase

        name: "LocalNodesPage"
        when: windowShown

        function init() {
            gateway.reset()
            state.networkProfile = "default"
            state.report = null
            state.error = ""
            state.operations = []
            state.revision = 0
            state.devnets = []
            state.packageCatalog = null
            state.packageCatalogError = ""
            state.packageCatalogLoading = false
            state.packageCatalogGeneration = 0
            state.clearActionDraft()
        }

        function test_package_panel_prefills_modules_dir_and_exact_release() {
            const page = createPage(sampleReport("stopped"), samplePackageCatalog({
                version: "1.0.0",
                root_hash: "root-hash-1.0.0"
            }))
            const modulesField = findChild(page, "runtimeModulesDirectoryInput")
            const versionSelector = findChild(page, "indexerPackageVersionSelector")
            verify(!!modulesField, "Object exists")
            verify(!!versionSelector, "Object exists")

            compare(page.runtimeModulesDir, "/tmp/runtime-modules")
            compare(modulesField.text, "/tmp/runtime-modules")
            compare(versionSelector.count, 3)
            compare(page.selectedIndexerPackage.version, "1.0.0")
            compare(page.selectedIndexerPackage.root_hash, "root-hash-1.0.0")
            compare(
                versionSelector.currentText,
                "1.0.0 · root-h…-1.0.0 · 2026-06-01")
            compare(
                versionSelector.model[1].label,
                "1.0.0 · root-h…repack · 2026-07-01")
            verify(versionSelector.model[1].label !== versionSelector.model[2].label)
            compare(versionSelector.Accessible.role, Accessible.ComboBox)
            compare(versionSelector.Accessible.name, "Indexer package exact release")

            const catalogCalls = gateway.calls.filter(function (call) {
                return call.method === "localNodePackageCatalog"
            })
            compare(catalogCalls.length, 1)
            compare(catalogCalls[0].args.length, 1)
            compare(catalogCalls[0].args[0], "/tmp/runtime-modules")
        }

        function test_install_confirmation_owns_exact_release_and_package_identity() {
            const page = createPage(sampleReport("stopped"), samplePackageCatalog(null))
            const versionSelector = findChild(page, "indexerPackageVersionSelector")
            const installButton = findChild(page, "indexerPackageInstallButton")
            const popup = findChild(page, "localNodesConfirmPopup")
            verify(!!versionSelector, "Object exists")
            verify(!!installButton, "Object exists")
            verify(!!popup, "Object exists")

            page.selectIndexerPackage(versionSelector.model[1])
            compare(page.selectedIndexerPackage.version, "1.0.0")
            compare(page.selectedIndexerPackage.root_hash, "root-hash-1.0.0-repack")
            compare(
                versionSelector.currentText,
                "1.0.0 · root-h…repack · 2026-07-01")

            state.packageCatalog = samplePackageCatalog(null)
            compare(page.selectedIndexerPackage.version, "1.0.0")
            compare(page.selectedIndexerPackage.root_hash, "root-hash-1.0.0-repack")
            verify(installButton.enabled)

            mouseClick(installButton, installButton.width / 2, installButton.height / 2)

            tryCompare(state, "pendingAction", "install")
            compare(state.pendingNode, "indexer")
            compare(state.pendingPackageVersion, "1.0.0")
            compare(state.pendingPackageRootHash, "root-hash-1.0.0-repack")
            compare(state.pendingRuntimeModulesDir, "/tmp/runtime-modules")
            tryCompare(popup, "opened", true)
            compare(popup.title, "Install Indexer 1.0.0")
            compare(
                popup.message,
                "This downloads official lez_indexer_module 1.0.0, verifies root hash root-hash-1.0.0-repack, and installs it into /tmp/runtime-modules. LogosCore Runtime must be stopped. After installation, start the runtime, then use Zone Sources to start the selected Channel Indexer.")
            popup.close()
        }

        function test_indexer_lifecycle_controls_stay_out_of_local_nodes() {
            const page = createPage(sampleReport("running"), samplePackageCatalog(null))
            const bedrockStart = findVisibleAccessibleByName(page, "Start Bedrock")
            const indexerStart = findVisibleAccessibleByName(page, "Start Indexer")
            const indexerStop = findVisibleAccessibleByName(page, "Stop Indexer")
            const installButton = findChild(page, "indexerPackageInstallButton")
            verify(!!bedrockStart, "Object exists")
            verify(!!installButton, "Object exists")

            compare(page.actionRows().length, 1)
            compare(page.actionRows()[0].key, "bedrock")
            compare(indexerStart, null)
            compare(indexerStop, null)
            verify(!installButton.enabled)
        }

        function test_node_status_projects_channel_indexers_individually() {
            const page = createPage(sampleReport("running"), samplePackageCatalog(null))
            state.observedNodes = ({
                indexer: {
                    status: "unavailable",
                    detail: "aaaa…aaaa: reachable · bbbb…bbbb: unreachable",
                    channels: [{
                        channel_id: "a".repeat(64),
                        short_channel_id: "aaaa…aaaa",
                        status: "reachable",
                        head: 101,
                        upstream_head: 104
                    }, {
                        channel_id: "b".repeat(64),
                        short_channel_id: "bbbb…bbbb",
                        status: "unreachable",
                        head: null,
                        upstream_head: 90
                    }]
                }
            })

            const rows = page.nodeTableRows()
            const indexer = rows.filter(function (row) { return row.key === "indexer" })[0]

            verify(indexer !== undefined)
            compare(indexer.cells[0].text, "Channel Indexers")
            compare(indexer.cells[2].text, "Unavailable")
            compare(indexer.cells[3].text, "2 configured Channels")
            compare(indexer.cells[4].copyText, "aaaa…aaaa 101 · bbbb…bbbb unreachable")
            compare(indexer.cells[5].text, "aaaa…aaaa: reachable · bbbb…bbbb: unreachable")
        }

        function test_canceling_messaging_stop_clears_identity_acknowledgement() {
            const page = createPage(sampleReport("running"), samplePackageCatalog(null))
            const popup = findChild(page, "localNodesConfirmPopup")
            verify(!!popup, "Object exists")

            page.openNodeConfirm("stop", "messaging")
            tryCompare(popup, "opened", true)
            compare(state.pendingAction, "stop")
            compare(state.pendingNode, "messaging")
            verify(state.pendingAllowIdentityRotation)
            verify(popup.message.indexOf("one-time rotation is unavoidable") >= 0)

            popup.close()
            tryCompare(popup, "opened", false)
            tryCompare(state, "pendingAction", "")
            compare(state.pendingNode, "")
            verify(!state.pendingAllowIdentityRotation)
            compare(gateway.calls.filter(function (call) {
                return call.method === "localNodesAction"
            }).length, 0)
        }

        function test_attached_service_runtime_controls_have_unambiguous_accessibility() {
            const page = createPage(attachedServiceReport("stopped"), samplePackageCatalog(null))
            const start = findChild(page, "runtimeStartButton")
            const stop = findChild(page, "runtimeStopButton")
            const popup = findChild(page, "localNodesConfirmPopup")
            verify(!!start, "Object exists")
            verify(!!stop, "Object exists")
            verify(!!popup, "Object exists")

            verify(start.enabled)
            verify(!stop.enabled)
            compare(start.text, "Start service")
            compare(start.Accessible.name, "Start local service")
            mouseClick(start, start.width / 2, start.height / 2)
            tryCompare(popup, "opened", true)
            compare(popup.title, "Start local service")
            compare(
                popup.message,
                "This starts the local service logos-node.service. Inspector does not alter node configuration, module contexts, or Messaging identity.")
            verify(!state.pendingAllowIdentityRotation)
            popup.close()
        }

        function test_attached_running_service_exposes_stop_without_identity_rotation() {
            const page = createPage(attachedServiceReport("running"), samplePackageCatalog(null))
            const stop = findChild(page, "runtimeStopButton")
            const popup = findChild(page, "localNodesConfirmPopup")
            verify(!!stop, "Object exists")
            verify(!!popup, "Object exists")

            verify(stop.enabled)
            compare(stop.text, "Stop service")
            compare(stop.Accessible.name, "Stop local service")
            mouseClick(stop, stop.width / 2, stop.height / 2)
            tryCompare(popup, "opened", true)
            compare(popup.title, "Stop local service")
            verify(popup.message.indexOf("does not alter node configuration") >= 0)
            verify(!state.pendingAllowIdentityRotation)
            popup.close()
        }

        function createPage(report, catalog) {
            gateway.responses = ({
                localNodesStatus: {
                    ok: true,
                    value: report,
                    text: "OK",
                    error: ""
                },
                localDevnetList: {
                    ok: true,
                    value: { devnets: [] },
                    text: "OK",
                    error: ""
                },
                localNodePackageCatalog: {
                    ok: true,
                    value: catalog,
                    text: "OK",
                    error: ""
                }
            })
            const page = createTemporaryObject(pageComponent, root)
            verify(!!page, "Component exists")
            wait(0)
            return page
        }

        function sampleReport(runtimeState) {
            return {
                profile: "default",
                mode: "public_testnet",
                available_network_actions: [],
                available_runtime_actions: runtimeState === "running"
                    ? ["stop_runtime"] : ["start_runtime"],
                active_devnet: "logos-testnet",
                workspace_root: "/tmp/logos-testnet",
                summary: {
                    total: 2,
                    installed: 1,
                    running: runtimeState === "running" ? 1 : 0,
                    needs_configuration: 1
                },
                nodes: [{
                    key: "bedrock",
                    kind: "bedrock",
                    label: "Bedrock",
                    available_actions: ["start"],
                    install_state: "installed",
                    run_state: "stopped",
                    ownership: "inspector_managed"
                }, {
                    key: "indexer",
                    kind: "indexer",
                    label: "Indexer",
                    available_actions: ["install", "start", "stop"],
                    install_state: "needs_configuration",
                    run_state: "stopped",
                    ownership: "inspector_managed"
                }],
                operations: [],
                runtime: {
                    ownership: "inspector_managed",
                    run_state: runtimeState,
                    modules_dir: "/tmp/runtime-modules",
                    binary_path: "/usr/bin/logoscore",
                    detail: "Test runtime"
                }
            }
        }

        function attachedServiceReport(runtimeState) {
            const report = sampleReport(runtimeState)
            report.runtime = {
                ownership: "local_attached",
                run_state: runtimeState,
                service_unit: "logos-node.service",
                detail: "local LogosCore daemon is running under system service `logos-node.service`"
            }
            return report
        }

        function samplePackageCatalog(installed) {
            return {
                modules_dir: "/tmp/runtime-modules",
                package: {
                    name: "lez_indexer_module",
                    versions: [{
                        version: "1.1.0",
                        released_at: "2026-07-17T12:00:00Z",
                        root_hash: "root-hash-1.1.0"
                    }, {
                        version: "1.0.0",
                        released_at: "2026-07-01T12:00:00Z",
                        root_hash: "root-hash-1.0.0-repack"
                    }, {
                        version: "1.0.0",
                        released_at: "2026-06-01T12:00:00Z",
                        root_hash: "root-hash-1.0.0"
                    }]
                },
                installed: installed
            }
        }

        function findVisibleAccessibleByName(item, expectedName) {
            if (!item) {
                return null
            }
            if (item.visible && item.Accessible
                    && String(item.Accessible.name || "") === expectedName) {
                return item
            }
            const children = item.children || []
            for (let i = 0; i < children.length; ++i) {
                const match = findVisibleAccessibleByName(children[i], expectedName)
                if (match) {
                    return match
                }
            }
            return null
        }
    }
}
