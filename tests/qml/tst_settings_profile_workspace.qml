import QtQuick
import QtQml.Models
import QtTest
import "../../qml/state/settings/SettingsProfileWorkspace.js" as SettingsProfileWorkspace

TestCase {
    name: "SettingsProfileWorkspace"

    QtObject {
        id: sourceRoutingStub

        function sourceModeOptions(family) {
            return [
                { value: family + "-auto", label: "Auto" },
                { value: family + "-rest", label: "REST" }
            ]
        }

        function sourceModeIndexFor(family, value, options) {
            return String(value || "") === family + "-rest" ? 1 : 0
        }

        function sourceModeAt(index, options) {
            return options.get(index).value
        }
    }

    QtObject {
        id: model

        property string nodeUrl: "https://node/"
        property string networkProfile: "default"
        property var sourceRouting: sourceRoutingStub
        property var localWalletStatus: null
        property string localWalletStatusError: ""
        property bool settingsBackupEncrypted: false
        property QtObject metrics: QtObject {
            function networkConnectionState(kind) {
                return kind === "ok"
                    ? { known: true, ok: true, detail: "ready", checkedAt: "now" }
                    : { known: false }
            }

            function networkConnectionRate(kind) { return kind === "slow" ? 15 : 0 }
        }

        function inferNetworkProfileFromEndpoint(node) {
            return String(node || "").indexOf("custom") >= 0
                ? "custom"
                : "default"
        }

        function networkProfileOptions() { return [{ key: "default", label: "Default" }, { key: "custom", label: "Custom" }] }
        function profileIndexFor(value) { return value === "custom" ? 1 : 0 }
        function networkProfileLabel(value) { return value === "custom" ? "Custom" : "Default" }
        function networkProfileSummary(value) { return value === "custom" ? "Manual endpoints" : "Default endpoints" }
        function networkProfileDetail() { return "Profile detail" }
        function normalizeEndpoint(value) { return String(value || "").trim() }
        function walletHomeConfigured() { return false }
    }

    QtObject {
        id: root

        property var model: model
        property var theme: ({
            textMuted: "muted",
            success: "success",
            warning: "warning",
            error: "error"
        })

        function inferProfile(node) {
            return model.inferNetworkProfileFromEndpoint(node)
        }
    }

    ListModel { id: profileOptions }
    ListModel { id: sourceOptions }

    function init() {
        profileOptions.clear()
        sourceOptions.clear()
        model.nodeUrl = "https://node/"
        model.networkProfile = "default"
        model.localWalletStatus = null
        model.localWalletStatusError = ""
        model.settingsBackupEncrypted = false
    }

    function test_profile_and_source_options_are_populated() {
        SettingsProfileWorkspace.refreshProfileOptions(root, profileOptions)
        SettingsProfileWorkspace.populateSourceOptions(root, sourceOptions, "storage")

        compare(profileOptions.count, 2)
        compare(sourceOptions.count, 2)
        compare(SettingsProfileWorkspace.sourceIndexFor(root, "storage", "storage-rest", sourceOptions), 1)
        compare(SettingsProfileWorkspace.sourceModeAt(root, 1, sourceOptions), "storage-rest")
    }

    function test_endpoint_update_syncs_profile() {
        SettingsProfileWorkspace.updateEndpoint(root, "nodeUrl", "https://custom-node/")

        compare(model.nodeUrl, "https://custom-node/")
        compare(model.networkProfile, "custom")
    }

    function test_status_projection_and_hints() {
        compare(SettingsProfileWorkspace.connectionStatusText(root, "ok"), "OK")
        compare(SettingsProfileWorkspace.connectionStatusColor(root, "ok"), "success")
        verify(SettingsProfileWorkspace.connectionStatusDetail(root, "slow").indexOf("15") >= 0)

        compare(SettingsProfileWorkspace.walletSourceStatusText(root), "Unknown")
        compare(SettingsProfileWorkspace.walletBackupHint(root), "Plain backup. Use wallet encryption for private or portable profiles.")
        compare(SettingsProfileWorkspace.shortEndpoint("https://example.test/"), "example.test")
    }
}
