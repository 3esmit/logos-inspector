pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQml.Models
import QtTest
import "../../qml/features/settings/controls"
import "../../qml/services"
import "../../qml/state"
import "../../qml/theme"
import "fixtures"

TestCase {
    id: testRoot

    name: "StorageConnectionPanel"
    when: windowShown
    width: 1180
    height: 760

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

    ListModel {
        id: sourceOptions

        ListElement {
            key: "logoscore_cli"
            label: "LogosCore CLI"
            summary: "Call storage_module with logoscore call"
        }

        ListElement {
            key: "rest"
            label: "Standalone REST"
            summary: "Inspect Storage through its REST API"
        }

        ListElement {
            key: "metrics"
            label: "Metrics only"
            summary: "Scrape a Prometheus endpoint"
        }
    }

    ApplicationWindow {
        id: testWindow

        visible: true
        width: testRoot.width
        height: testRoot.height
        color: theme.background

        StorageConnectionPanel {
            id: panel

            theme: theme
            title: qsTr("Storage")
            subtitle: qsTr("Configure the Storage inspection source.")
            pageWidth: testWindow.width
            modelRef: model
            sourceOptions: sourceOptions
            width: testWindow.width
        }
    }

    function init() {
        fakeHost.reset()
        model.storageApp.operationSession.clearActive()
        model.sourceRouting.sourcePolicy = ({})
        model.loadNetworkConnectorConfig({})
        model.loadSettingsState()
        model.setNetworkConnectorMode("storage", "logoscore_cli")
        model.metrics.storageRefreshRate = 30
        model.storageRollingWindow = 120
        model.storageLocalDiagnosticsEnabled = false
        model.storagePrivilegedDebugEnabled = false
        fakeHost.reset()
        wait(0)
    }

    function test_source_controls_lock_while_storage_operation_runs() {
        model.setNetworkConnectorMode("storage", "rest")
        tryCompare(model, "storageSourceMode", "rest")

        verify(model.storageApp.operationSession.acceptUpdate({
            operationId: "storage-download-lock",
            domain: "storage",
            method: "storageDownloadToUrl",
            status: "running",
            label: "Download",
            cancellable: true
        }))
        tryVerify(function () { return model.storageApp.operation.busy })

        const lockedControls = [
            "Storage connector",
            "REST URL",
            "Metrics URL",
            "Network preset",
            "Include network debug details"
        ]
        for (let i = 0; i < lockedControls.length; ++i) {
            const control = findAccessibleByName(panel, lockedControls[i])
            verify(control !== null, "Missing source control " + lockedControls[i])
            verify(!control.enabled, lockedControls[i] + " remained enabled")
        }

        const safeControls = [
            "CID local exists",
            "Show local paths",
            "Query Storage status"
        ]
        for (let j = 0; j < safeControls.length; ++j) {
            const control = findAccessibleByName(panel, safeControls[j])
            verify(control !== null, "Missing safe control " + safeControls[j])
            verify(control.enabled, safeControls[j] + " was unnecessarily disabled")
        }
        verify(hasVisibleText(panel, "Storage source locked"))

        compare(model.setNetworkConnectorMode("storage", "metrics"), false)
        compare(model.setNetworkConnectorEndpoint(
                    "storage", "http://locked-storage.example/api/storage/v1"), false)
        compare(model.storageSourceMode, "rest")
        model.storageRestUrl = "http://storage-changed.example/api/storage/v1"
        wait(0)
        verify(model.storageApp.operation.busy)
        compare(model.storageApp.operation.active.operationId,
                "storage-download-lock")
        verify(model.storageApp.operation.cancelable)

        model.storageApp.operationSession.clearActive()
        model.storageRestUrl = "http://127.0.0.1:8080/api/storage/v1"
        tryVerify(function () {
            const connector = findAccessibleByName(panel, "Storage connector")
            return connector !== null && connector.enabled
        })
    }

    function test_rest_field_edits_canonical_connector_endpoint() {
        const configuredEndpoint = "http://configured-storage.example/api/storage/v1"
        const fallbackEndpoint = "http://fallback-storage.example/api/storage/v1"
        const editedEndpoint = "http://edited-storage.example/api/storage/v1"
        model.loadNetworkConnectorConfig({
            network_connector_config: {
                scopes: {
                    l1: {
                        connector_id: "direct_l1_rpc",
                        provenance: "network_profile"
                    },
                    delivery: {
                        connector_id: "direct_delivery_rest",
                        provenance: "network_profile"
                    },
                    storage: {
                        connector_id: "direct_storage_rest",
                        endpoint: configuredEndpoint,
                        provenance: "network_profile"
                    }
                }
            }
        })
        model.storageRestUrl = fallbackEndpoint
        wait(0)

        const restField = findAccessibleByName(panel, "REST URL")
        verify(restField !== null)
        compare(restField.text, configuredEndpoint)
        restField.text = editedEndpoint
        restField.textEdited()

        tryCompare(restField, "text", editedEndpoint)
        compare(model.storageRestUrl, editedEndpoint)
        compare(model.networkConnectorConfig.scopes.storage.endpoint,
                editedEndpoint)
        compare(model.sourceRouting.configuredStorageRestUrl(), editedEndpoint)
        compare(model.sourceRouting.storageOperationAdapter().inputs.rest_endpoint,
                editedEndpoint)
    }

    function test_metrics_field_edits_canonical_connector_endpoint() {
        const restEndpoint = "http://configured-storage.example/api/storage/v1"
        const configuredEndpoint = "http://configured-storage.example/metrics"
        const fallbackEndpoint = "http://fallback-storage.example/metrics"
        const editedEndpoint = "http://edited-storage.example/metrics"
        model.loadNetworkConnectorConfig({
            network_connector_config: {
                scopes: {
                    l1: {
                        connector_id: "direct_l1_rpc",
                        provenance: "network_profile"
                    },
                    delivery: {
                        connector_id: "direct_delivery_rest",
                        provenance: "network_profile"
                    },
                    storage: {
                        connector_id: "storage_metrics",
                        endpoint: configuredEndpoint,
                        provenance: "network_profile"
                    }
                }
            }
        })
        model.storageRestUrl = restEndpoint
        model.storageMetricsUrl = fallbackEndpoint
        wait(0)

        const metricsField = findAccessibleByName(panel, "Metrics URL")
        verify(metricsField !== null)
        compare(metricsField.text, configuredEndpoint)
        metricsField.text = editedEndpoint
        metricsField.textEdited()

        tryCompare(metricsField, "text", editedEndpoint)
        compare(model.storageMetricsUrl, editedEndpoint)
        compare(model.networkConnectorConfig.scopes.storage.endpoint,
                editedEndpoint)
        compare(model.sourceRouting.configuredStorageMetricsUrl(), editedEndpoint)
        compare(model.sourceRouting.configuredStorageRestUrl(), restEndpoint)
        compare(model.sourceRouting.storageOperationAdapter().inputs.metrics_endpoint,
                editedEndpoint)
    }

    function test_rest_optional_metrics_edit_preserves_rest_endpoint() {
        const restEndpoint = "http://configured-storage.example/api/storage/v1"
        const metricsEndpoint = "http://edited-storage.example/metrics"
        model.loadNetworkConnectorConfig({
            network_connector_config: {
                scopes: {
                    l1: {
                        connector_id: "direct_l1_rpc",
                        provenance: "network_profile"
                    },
                    delivery: {
                        connector_id: "direct_delivery_rest",
                        provenance: "network_profile"
                    },
                    storage: {
                        connector_id: "direct_storage_rest",
                        endpoint: restEndpoint,
                        provenance: "network_profile"
                    }
                }
            }
        })
        wait(0)

        const metricsField = findAccessibleByName(panel, "Metrics URL")
        verify(metricsField !== null)
        metricsField.text = metricsEndpoint
        metricsField.textEdited()

        tryCompare(metricsField, "text", metricsEndpoint)
        compare(model.storageMetricsUrl, metricsEndpoint)
        compare(model.networkConnectorConfig.scopes.storage.endpoint,
                restEndpoint)
        compare(model.sourceRouting.configuredStorageRestUrl(), restEndpoint)
        compare(model.sourceRouting.storageSourceReportArgs(false)[0].inputs.metrics_endpoint,
                metricsEndpoint)
    }

    function test_unavailable_source_names_only_visible_connector_choices() {
        model.sourceRouting.sourcePolicy = ({
            source_modes: {
                storage: [{
                    key: "unsupported",
                    effective: "unsupported",
                    label: "Retired connector",
                    implemented: false,
                    adapter: {
                        connector_id: "removed_storage_connector",
                        connection_type: "retired",
                        target: "none",
                        inputs: []
                    }
                }]
            }
        })
        model.networkConnectorConfig = ({
            scopes: {
                storage: {
                    connector_id: "removed_storage_connector",
                    source_mode: "unsupported",
                    provenance: "network_profile"
                }
            }
        })
        model.storageSourceMode = "unsupported"
        wait(0)

        compare(panel.storageSourceMode(), "unsupported")
        verify(hasVisibleText(panel, "Source unavailable"))
        verify(hasVisibleText(
                   panel,
                   "The configured connector no longer has an adapter. Select LogosCore CLI, Standalone REST, or Metrics only."))
        verify(!hasVisibleText(
                   panel,
                   "The configured connector no longer has an adapter. Select Storage module, Standalone REST, or Metrics only."))
    }

    function test_connector_popup_exposes_option_semantics() {
        const connector = findAccessibleByName(panel, "Storage connector")
        verify(connector !== null)
        mouseClick(connector, connector.width / 2, connector.height / 2)
        tryVerify(function () { return connector.popup.visible })

        const expected = [
            ["LogosCore CLI", "Call storage_module with logoscore call"],
            ["Standalone REST", "Inspect Storage through its REST API"],
            ["Metrics only", "Scrape a Prometheus endpoint"]
        ]
        for (let i = 0; i < expected.length; ++i) {
            let option = null
            tryVerify(function () {
                option = connector.popup.contentItem.itemAtIndex(i)
                return option !== null
            })
            compare(String(option.Accessible.name), expected[i][0])
            compare(option.Accessible.role, Accessible.ListItem)
            compare(String(option.Accessible.description), expected[i][1])
        }
        connector.popup.close()
        tryVerify(function () { return !connector.popup.visible })
    }

    function test_numeric_controls_expose_semantics_and_persist_interactions() {
        const refresh = findAccessibleByName(panel, "Storage auto refresh")
        const rolling = findAccessibleByName(panel, "Storage rolling window")
        verify(refresh !== null)
        verify(rolling !== null)
        compare(
            String(refresh.Accessible.description),
            "Automatic Storage status refresh interval in seconds. Set to 0 to turn it off.")
        compare(
            String(rolling.Accessible.description),
            "Storage metrics rolling window in seconds.")
        compare(refresh.value, 30)
        compare(refresh.from, 0)
        compare(refresh.to, 3600)
        compare(refresh.stepSize, 5)
        compare(refresh.focusPolicy, Qt.StrongFocus)
        compare(rolling.value, 120)
        compare(rolling.from, 5)
        compare(rolling.to, 3600)
        compare(rolling.stepSize, 5)
        compare(rolling.focusPolicy, Qt.StrongFocus)

        fakeHost.reset()
        mouseClick(
            refresh.up.indicator,
            refresh.up.indicator.width / 2,
            refresh.up.indicator.height / 2)
        tryCompare(model.metrics, "storageRefreshRate", 35)
        compare(latestSavedSetting("storage_refresh_rate"), 35)

        mouseClick(
            refresh.down.indicator,
            refresh.down.indicator.width / 2,
            refresh.down.indicator.height / 2)
        tryCompare(model.metrics, "storageRefreshRate", 30)
        compare(latestSavedSetting("storage_refresh_rate"), 30)

        mouseClick(
            rolling.down.indicator,
            rolling.down.indicator.width / 2,
            rolling.down.indicator.height / 2)
        tryCompare(model, "storageRollingWindow", 115)
        compare(latestSavedSetting("storage_rolling_window"), 115)

        mouseClick(
            rolling.up.indicator,
            rolling.up.indicator.width / 2,
            rolling.up.indicator.height / 2)
        tryCompare(model, "storageRollingWindow", 120)
        compare(latestSavedSetting("storage_rolling_window"), 120)
    }

    function test_path_privacy_control_is_truthful_and_has_no_fake_path_editor() {
        compare(
            model.storageDisplayPath("/tmp/legacy-storage-data"),
            ".../legacy-storage-data")
        let showLocalPaths = null
        tryVerify(function () {
            showLocalPaths = findAccessibleByName(panel, "Show local paths")
            return showLocalPaths !== null
        })

        compare(showLocalPaths.Accessible.role, Accessible.CheckBox)
        compare(
            String(showLocalPaths.Accessible.description),
            "Shows full local storage paths in diagnostics and enables their Copy actions.")
        verify(findAccessibleByName(panel, "Local OS diagnostics") === null)
        verify(findAccessibleByName(panel, "Data directory") === null)
        verify(!hasVisibleText(panel, "Data directory"))

        mouseClick(showLocalPaths,
                   showLocalPaths.width / 2,
                   showLocalPaths.height / 2)

        tryCompare(model, "storageLocalDiagnosticsEnabled", true)
        compare(model.storageDisplayPath("/tmp/legacy-storage-data"),
                "/tmp/legacy-storage-data")
        verify(findAccessibleByName(panel, "Data directory") === null)
        verify(!hasVisibleText(panel, "Data directory"))
    }

    function test_network_debug_toggle_describes_real_read_only_probe() {
        let networkDebug = null
        tryVerify(function () {
            networkDebug = findAccessibleByName(
                    panel, "Include network debug details")
            return networkDebug !== null
        })

        compare(networkDebug.Accessible.role, Accessible.CheckBox)
        compare(
            String(networkDebug.Accessible.description),
            "Queries peer identity, addresses, public records, and the DHT routing table during Storage status checks. Read-only; may expose network topology.")
        verify(findAccessibleByName(panel, "Privileged debug") === null)

        mouseClick(networkDebug,
                   networkDebug.width / 2,
                   networkDebug.height / 2)

        tryCompare(model, "storagePrivilegedDebugEnabled", true)
    }

    function findAccessibleByName(item, expectedName) {
        if (!item) {
            return null
        }
        if (item.Accessible
                && String(item.Accessible.name || "") === expectedName
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

    function latestSavedSetting(key) {
        for (let i = fakeHost.calls.length - 1; i >= 0; --i) {
            const call = fakeHost.calls[i]
            if (call.method === "saveSettingsState"
                    && call.args.length > 0) {
                return call.args[0][key]
            }
        }
        return undefined
    }

    function hasVisibleText(item, expectedText) {
        if (!item) {
            return false
        }
        if (item.text !== undefined
                && String(item.text) === expectedText
                && item.visible) {
            return true
        }
        const children = item.children || []
        for (let i = 0; i < children.length; ++i) {
            if (hasVisibleText(children[i], expectedText)) {
                return true
            }
        }
        return false
    }
}
