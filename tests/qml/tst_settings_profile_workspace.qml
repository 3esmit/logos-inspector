import QtQuick
import QtQml.Models
import QtTest
import "../../qml/state/settings/SettingsProfileWorkspace.js" as SettingsProfileWorkspace
import "../../qml/state/storage/StorageCidValidation.js" as StorageCidValidation

TestCase {
    name: "SettingsProfileWorkspace"

    QtObject {
        id: sourceRoutingStub

        property bool supportsStorageCidProbe: true

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

        function storageSourceView() {
            return { supportsCidProbe: supportsStorageCidProbe }
        }
    }

    QtObject {
        id: metricsStub

        property string lastQueryKind: ""
        property bool lastQueryShowResult: false
        property bool lastQueryIncludeSensitiveProbe: false
        property int queryCount: 0

        function networkConnectionState(kind) {
            return kind === "ok"
                ? { known: true, ok: true, detail: "ready", checkedAt: "now" }
                : { known: false }
        }

        function networkConnectionRate(kind) { return kind === "slow" ? 15 : 0 }

        function queryNetworkConnection(kind, showResult, includeSensitiveProbe) {
            lastQueryKind = String(kind || "")
            lastQueryShowResult = showResult === true
            lastQueryIncludeSensitiveProbe = includeSensitiveProbe === true
            queryCount += 1
            return true
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
        property string storageCidProbe: ""
        property QtObject metrics: metricsStub

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
        sourceRoutingStub.supportsStorageCidProbe = true
        model.nodeUrl = "https://node/"
        model.networkProfile = "default"
        model.localWalletStatus = null
        model.localWalletStatusError = ""
        model.settingsBackupEncrypted = false
        model.storageCidProbe = ""
        metricsStub.lastQueryKind = ""
        metricsStub.lastQueryShowResult = false
        metricsStub.lastQueryIncludeSensitiveProbe = false
        metricsStub.queryCount = 0
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

    function test_storage_status_query_includes_only_configured_cid_probe() {
        verify(SettingsProfileWorkspace.queryStorageStatus(root))
        compare(metricsStub.lastQueryKind, "storage")
        verify(metricsStub.lastQueryShowResult)
        verify(!metricsStub.lastQueryIncludeSensitiveProbe)

        model.storageCidProbe = "  z-cid-probe  "
        verify(SettingsProfileWorkspace.queryStorageStatus(root))
        compare(metricsStub.queryCount, 2)
        compare(metricsStub.lastQueryKind, "storage")
        verify(metricsStub.lastQueryShowResult)
        verify(metricsStub.lastQueryIncludeSensitiveProbe)
    }

    function test_storage_status_query_rejects_invalid_cid_before_metrics() {
        model.storageCidProbe = "cid/child"

        let response = SettingsProfileWorkspace.queryStorageStatus(root)

        verify(!response.ok)
        compare(
            response.error,
            "Storage CID must contain only ASCII letters, digits, `-`, or `_`.")
        compare(metricsStub.queryCount, 0)

        model.storageCidProbe = "a".repeat(257)
        response = SettingsProfileWorkspace.queryStorageStatus(root)

        verify(!response.ok)
        compare(response.error, "Storage CID exceeds 256-byte limit.")
        compare(metricsStub.queryCount, 0)

        sourceRoutingStub.supportsStorageCidProbe = false
        verify(SettingsProfileWorkspace.queryStorageStatus(root))
        compare(metricsStub.queryCount, 1)
        verify(!metricsStub.lastQueryIncludeSensitiveProbe)

        sourceRoutingStub.supportsStorageCidProbe = true
        model.storageCidProbe = "valid-CID_123"
        verify(SettingsProfileWorkspace.queryStorageStatus(root))
        compare(metricsStub.queryCount, 2)
        verify(metricsStub.lastQueryIncludeSensitiveProbe)
    }

    function test_storage_cid_validation_matches_route_safety_contract() {
        compare(StorageCidValidation.optionalError(""), "")
        compare(StorageCidValidation.optionalError("   "), "")
        compare(StorageCidValidation.optionalError("A0-_"), "")
        compare(StorageCidValidation.optionalError("a".repeat(256)), "")
        compare(
            StorageCidValidation.optionalError("a".repeat(257)),
            "Storage CID exceeds 256-byte limit.")
        compare(
            StorageCidValidation.optionalError("😀".repeat(65)),
            "Storage CID exceeds 256-byte limit.")
        compare(
            StorageCidValidation.optionalError("cid/child"),
            "Storage CID must contain only ASCII letters, digits, `-`, or `_`.")
        compare(
            StorageCidValidation.optionalError("cid-é"),
            "Storage CID must contain only ASCII letters, digits, `-`, or `_`.")
    }
}
