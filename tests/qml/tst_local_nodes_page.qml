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
            state.clearNodeConfig()
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

        function test_basecamp_uses_host_modules_without_standalone_controls() {
            gateway.basecampModules = true
            const page = createPage(basecampReport(), null)
            const devnet = findChild(page, "localDevnetConfiguration")
            const runtime = findChild(page, "logoscoreRuntimeConfiguration")
            const packagePanel = findChild(page, "indexerPackageConfiguration")
            verify(!!devnet, "Local Devnet panel exists")
            verify(!!runtime, "LogosCore Runtime panel exists")
            verify(!!packagePanel, "Indexer package panel exists")

            compare(devnet.visible, false)
            compare(runtime.visible, false)
            compare(packagePanel.visible, false)
            compare(page.actionRows().length, 3)
            compare(page.actionRows()[0].key, "bedrock")
            compare(page.actionRows()[1].key, "storage")
            compare(page.actionRows()[2].key, "messaging")
            compare(gateway.calls.filter(function (call) {
                return call.method === "localDevnetList"
            }).length, 0)
            compare(gateway.calls.filter(function (call) {
                return call.method === "localNodePackageCatalog"
            }).length, 0)
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
            verify(start.width >= start.implicitWidth)
            verify(stop.width >= stop.implicitWidth)
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
            verify(stop.width >= stop.implicitWidth)
            mouseClick(stop, stop.width / 2, stop.height / 2)
            tryCompare(popup, "opened", true)
            compare(popup.title, "Stop local service")
            verify(popup.message.indexOf("does not alter node configuration") >= 0)
            verify(!state.pendingAllowIdentityRotation)
            popup.close()
        }

        function test_node_configuration_forces_save_or_undo_before_tab_switch() {
            const page = createPage(sampleReport("stopped"), samplePackageCatalog(null))
            const configure = findChild(page, "nodeConfigurebedrock")
            const panel = findChild(page, "nodeConfigurationPanel")
            verify(!!configure, "Configuration control exists")
            verify(!!panel, "Configuration panel exists")
            verify(configure.enabled)

            mouseClick(configure, configure.width / 2, configure.height / 2)
            tryCompare(panel, "visible", true)
            tryCompare(panel, "activeNode", "bedrock")
            compare(panel.currentTab, "common")
            compare(panel.nodeLabel(), "Bedrock")
            verify(panel.editable)
            tryCompare(panel, "validDraft", true)

            panel.updateCommonText({
                path: "/http_addr",
                kind: "string",
                label: "HTTP API address"
            }, "127.0.0.1:9090")
            verify(panel.dirty)
            verify(panel.draftText.indexOf("127.0.0.1:9090") >= 0)

            panel.requestTab("raw")
            compare(panel.currentTab, "common")
            verify(panel.tabGuardMessage.indexOf("Save or undo") >= 0)

            panel.undoDraft()
            verify(!panel.dirty)
            panel.requestTab("raw")
            compare(panel.currentTab, "raw")
            const raw = findChild(panel, "nodeConfigRawInput")
            const save = findChild(panel, "nodeConfigSaveButton")
            verify(!!raw, "Raw editor exists")
            verify(!!save, "Save control exists")
            compare(raw.Accessible.name, "Raw JSON node configuration")

            panel.setDraftText("{")
            verify(panel.validationError.length > 0)
            verify(!save.enabled)
        }

        function test_node_configuration_common_field_accepts_full_text() {
            const page = createPage(sampleReport("stopped"), samplePackageCatalog(null))
            const configure = findChild(page, "nodeConfigurebedrock")
            const panel = findChild(page, "nodeConfigurationPanel")
            verify(!!configure, "Configuration control exists")
            verify(!!panel, "Configuration panel exists")

            mouseClick(configure, configure.width / 2, configure.height / 2)
            tryCompare(panel, "activeNode", "bedrock")
            const address = findChild(panel, "nodeConfigCommonField0")
            verify(!!address, "Common text field exists")

            address.forceActiveFocus()
            address.selectAll()
            verify(address.activeFocus, "Common text field has focus")
            keyClick(Qt.Key_D)
            compare(address.text, "d")
            keyClick(Qt.Key_E)
            compare(address.text, "de")
            keyClick(Qt.Key_B)
            compare(address.text, "deb")
            keyClick(Qt.Key_U)
            compare(address.text, "debu")
            keyClick(Qt.Key_G)

            tryVerify(function () {
                const current = findChild(panel, "nodeConfigCommonField0")
                return !!current && current.text === "debug"
            })
            verify(panel.draftText.indexOf("debug") >= 0)
        }

        function test_node_configuration_raw_field_accepts_full_text() {
            const page = createPage(sampleReport("stopped"), samplePackageCatalog(null))
            const configure = findChild(page, "nodeConfigurebedrock")
            const panel = findChild(page, "nodeConfigurationPanel")
            verify(!!configure, "Configuration control exists")
            verify(!!panel, "Configuration panel exists")

            mouseClick(configure, configure.width / 2, configure.height / 2)
            tryCompare(panel, "activeNode", "bedrock")
            panel.requestTab("raw")
            compare(panel.currentTab, "raw")
            const raw = findChild(panel, "nodeConfigRawInput")
            verify(!!raw, "Raw configuration input exists")

            raw.forceActiveFocus()
            raw.cursorPosition = raw.text.length
            verify(raw.activeFocus, "Raw configuration input has focus")
            keyClick(Qt.Key_D)
            keyClick(Qt.Key_E)
            keyClick(Qt.Key_B)
            keyClick(Qt.Key_U)
            keyClick(Qt.Key_G)

            tryVerify(function () {
                return raw.text.endsWith("debug")
            })
            verify(panel.draftText.endsWith("debug"))
        }

        function test_node_configuration_retries_same_node_after_load_error() {
            const page = createPage(sampleReport("stopped"), samplePackageCatalog(null))
            const configure = findChild(page, "nodeConfigurebedrock")
            const panel = findChild(page, "nodeConfigurationPanel")
            verify(!!configure, "Configuration control exists")
            verify(!!panel, "Configuration panel exists")

            gateway.responses.localNodeConfig = {
                ok: false,
                value: null,
                text: "Unavailable",
                error: "Temporary configuration failure"
            }
            mouseClick(configure, configure.width / 2, configure.height / 2)
            tryCompare(state, "nodeConfigError", "Temporary configuration failure")
            compare(panel.activeNode, "bedrock")

            gateway.responses.localNodeConfig = {
                ok: true,
                value: sampleBedrockConfig(),
                text: "OK",
                error: ""
            }
            mouseClick(configure, configure.width / 2, configure.height / 2)
            tryCompare(state, "nodeConfigError", "")
            tryCompare(panel, "editable", true)
            const configCalls = gateway.calls.filter(function (call) {
                return call.method === "localNodeConfig"
            })
            compare(configCalls.length, 2)
        }

        function test_node_configuration_retries_same_node_after_becoming_editable() {
            const page = createPage(sampleReport("stopped"), samplePackageCatalog(null))
            const configure = findChild(page, "nodeConfigurebedrock")
            const panel = findChild(page, "nodeConfigurationPanel")
            verify(!!configure, "Configuration control exists")
            verify(!!panel, "Configuration panel exists")

            const readOnlySnapshot = sampleBedrockConfig()
            readOnlySnapshot.editable = false
            readOnlySnapshot.blocked_reason = "Stop this node before editing configuration."
            gateway.responses.localNodeConfig = {
                ok: true,
                value: readOnlySnapshot,
                text: "OK",
                error: ""
            }
            mouseClick(configure, configure.width / 2, configure.height / 2)
            tryCompare(panel, "editable", false)
            verify(!panel.validDraft)

            gateway.responses.localNodeConfig = {
                ok: true,
                value: sampleBedrockConfig(),
                text: "OK",
                error: ""
            }
            mouseClick(configure, configure.width / 2, configure.height / 2)
            tryCompare(panel, "editable", true)
            tryCompare(panel, "validDraft", true)
            const configCalls = gateway.calls.filter(function (call) {
                return call.method === "localNodeConfig"
            })
            compare(configCalls.length, 2)
        }

        function test_node_configuration_resets_after_profile_change_and_can_reopen() {
            const page = createPage(sampleReport("stopped"), samplePackageCatalog(null))
            const configure = findChild(page, "nodeConfigurebedrock")
            const panel = findChild(page, "nodeConfigurationPanel")
            verify(!!configure, "Configuration control exists")
            verify(!!panel, "Configuration panel exists")

            mouseClick(configure, configure.width / 2, configure.height / 2)
            tryCompare(panel, "activeNode", "bedrock")
            verify(panel.visible)

            state.networkProfile = "local"
            tryCompare(panel, "activeNode", "")
            verify(!panel.visible)

            state.report = sampleReport("stopped")
            state.revision += 1
            const reopenedConfigure = findChild(page, "nodeConfigurebedrock")
            verify(!!reopenedConfigure, "Configuration control is recreated")
            mouseClick(reopenedConfigure,
                       reopenedConfigure.width / 2,
                       reopenedConfigure.height / 2)
            tryCompare(panel, "activeNode", "bedrock")
            verify(panel.visible)
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
                },
                localNodeConfig: {
                    ok: true,
                    value: sampleBedrockConfig(),
                    text: "OK",
                    error: ""
                },
                localNodeConfigValidate: {
                    ok: true,
                    value: {
                        valid: true,
                        error: "",
                        common_fields: sampleBedrockConfig().common_fields
                    },
                    text: "OK",
                    error: ""
                },
                localNodeConfigSave: {
                    ok: true,
                    value: sampleBedrockConfig(),
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
                    ownership: "inspector_managed",
                    config_path: "/tmp/logos-testnet/configs/bedrock.init.json"
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

        function basecampReport() {
            return {
                profile: "default",
                mode: "public_testnet",
                available_network_actions: [],
                available_runtime_actions: [],
                active_devnet: "logos-testnet",
                workspace_root: "/tmp/logos-testnet",
                summary: {
                    total: 3,
                    installed: 3,
                    running: 3,
                    needs_configuration: 0
                },
                nodes: [{
                    key: "bedrock",
                    kind: "bedrock",
                    label: "Bedrock",
                    available_actions: ["stop"],
                    install_state: "installed",
                    run_state: "running",
                    ownership: "inspector_managed"
                }, {
                    key: "storage",
                    kind: "storage",
                    label: "Storage",
                    available_actions: ["stop", "destroy"],
                    install_state: "installed",
                    run_state: "running",
                    ownership: "inspector_managed"
                }, {
                    key: "messaging",
                    kind: "messaging",
                    label: "Messaging",
                    available_actions: ["stop"],
                    install_state: "installed",
                    run_state: "running",
                    ownership: "inspector_managed"
                }],
                operations: [],
                runtime: {
                    ownership: "basecamp_host",
                    run_state: "running",
                    detail: "Basecamp owns module loading and process lifecycle."
                }
            }
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

        function sampleBedrockConfig() {
            return {
                profile: "default",
                topology_id: "logos-testnet",
                node: "bedrock",
                node_label: "Bedrock",
                config_path: "/tmp/logos-testnet/configs/bedrock.init.json",
                config_role: "Initialization source",
                format: "json",
                raw_text: "{\n  \"initial_peers\": [],\n  \"net_port\": 3000,\n  \"blend_port\": 3001,\n  \"http_addr\": \"127.0.0.1:8080\",\n  \"skip_ibd\": false,\n  \"state_path\": \"/tmp/logos-testnet/data/bedrock/state\",\n  \"storage_path\": \"/tmp/logos-testnet/data/bedrock/storage\",\n  \"logs_path\": \"/tmp/logos-testnet/data/bedrock/logs\",\n  \"log_filter\": \"info\",\n  \"output\": \"/tmp/logos-testnet/configs/bedrock.yaml\"\n}",
                revision: "config-revision-1",
                editable: true,
                validation_scope: "JSON syntax and Inspector-managed field checks",
                common_fields: [{
                    path: "/http_addr",
                    label: "HTTP API address",
                    section: "API",
                    kind: "string",
                    value: "127.0.0.1:8080",
                    required: true
                }, {
                    path: "/skip_ibd",
                    label: "Skip initial block download",
                    section: "Protocol",
                    kind: "boolean",
                    value: false,
                    required: true
                }],
                protected_fields: ["Generated Bedrock runtime keys"]
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
