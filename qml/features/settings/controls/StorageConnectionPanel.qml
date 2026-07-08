pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../state"

SourceSettingsPanel {
    id: root

    property real pageWidth: 900
    property AppModel modelRef
    property var sourceOptions

    busy: root.modelRef ? root.modelRef.busy : false
    queryAccessibleName: qsTr("Query Storage status")

    SourceSettingsGrid {
        theme: root.theme
        pageWidth: root.pageWidth

        ComboField {
            theme: root.theme
            label: qsTr("Source mode")
            accessibleName: qsTr("Storage source mode")
            options: root.sourceOptions
            currentIndex: root.sourceIndexFor(root.modelRef.storageSourceMode)
            onActivated: index => root.modelRef.storageSourceMode = root.sourceModeAt(index)
        }

        FieldRow {
            theme: root.theme
            label: qsTr("REST URL")
            enabled: root.storageRestEnabled()
            opacity: enabled ? 1 : 0.56
            sourceText: root.modelRef.storageRestUrl
            syncSourceText: true
            placeholderText: qsTr("http://127.0.0.1:8080/api/storage/v1")
            onTextEdited: text => root.modelRef.storageRestUrl = String(text || "").trim()
        }

        FieldRow {
            theme: root.theme
            label: qsTr("Metrics URL")
            enabled: root.storageMetricsEnabled()
            opacity: enabled ? 1 : 0.56
            sourceText: root.modelRef.storageMetricsUrl
            syncSourceText: true
            placeholderText: qsTr("http://127.0.0.1:8008/metrics")
            onTextEdited: text => root.modelRef.storageMetricsUrl = String(text || "").trim()
        }

        FieldRow {
            theme: root.theme
            label: qsTr("Network preset")
            sourceText: root.modelRef.storageNetworkPreset
            syncSourceText: true
            placeholderText: qsTr("logos.test")
            onTextEdited: text => root.modelRef.storageNetworkPreset = String(text || "").trim()
        }

        FieldRow {
            visible: root.modelRef.storageLocalDiagnosticsEnabled === true
            theme: root.theme
            label: qsTr("Data directory")
            sourceText: root.modelRef.storageDataDir
            syncSourceText: true
            placeholderText: qsTr("Optional local diagnostics path")
            onTextEdited: text => root.modelRef.storageDataDir = String(text || "").trim()
        }

        InfoField {
            visible: root.modelRef.storageLocalDiagnosticsEnabled !== true
            theme: root.theme
            label: qsTr("Data directory")
            value: root.modelRef.storageDataDir.length
                ? root.modelRef.storageDisplayPath(root.modelRef.storageDataDir)
                : qsTr("Local diagnostics disabled")
        }

        FieldRow {
            theme: root.theme
            label: qsTr("CID local exists")
            enabled: root.storageDataEnabled()
            opacity: enabled ? 1 : 0.56
            sourceText: root.modelRef.storageCidProbe
            syncSourceText: true
            placeholderText: qsTr("Optional CID")
            onTextEdited: text => root.modelRef.storageCidProbe = String(text || "").trim()
        }

        RefreshRateField {
            theme: root.theme
            value: root.modelRef.storageRefreshRate
            onRateEdited: value => root.modelRef.setNetworkConnectionRate("storage", value)
        }

        SecondsField {
            theme: root.theme
            label: qsTr("Rolling window")
            value: root.modelRef.storageRollingWindow
            onValueEdited: value => root.modelRef.storageRollingWindow = value
        }
    }

    Flow {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        SafetyToggle {
            theme: root.theme
            text: qsTr("Local OS diagnostics")
            detail: qsTr("Allows future process, disk, and port checks from the local machine.")
            checked: root.modelRef.storageLocalDiagnosticsEnabled
            onToggled: root.modelRef.storageLocalDiagnosticsEnabled = checked
        }

        SafetyToggle {
            theme: root.theme
            text: qsTr("Privileged debug")
            detail: qsTr("Allows future privileged debug endpoints after source-specific confirmation.")
            checked: root.modelRef.storagePrivilegedDebugEnabled
            onToggled: root.modelRef.storagePrivilegedDebugEnabled = checked
        }

        SafetyToggle {
            theme: root.theme
            text: qsTr("Mutating diagnostics")
            detail: qsTr("Allows future upload, download, connect, remove, and lifecycle probes after per-action confirmation.")
            checked: root.modelRef.storageMutatingDiagnosticsEnabled
            onToggled: root.modelRef.storageMutatingDiagnosticsEnabled = checked
        }
    }

    StatusMessage {
        visible: root.storageSourceMode() === "unsupported"
        theme: root.theme
        tone: "warning"
        title: qsTr("Source unavailable")
        message: qsTr("The saved source mode no longer has an adapter. Select Auto, Storage module, Standalone REST, or Metrics only.")
        Layout.fillWidth: true
    }

    function sourceIndexFor(value) {
        return root.modelRef.sourceModeIndexFor("storage", value, root.sourceOptions)
    }

    function sourceModeAt(index) {
        return root.modelRef.sourceModeAt(index, root.sourceOptions)
    }

    function storageSourceMode() {
        return root.modelRef.effectiveStorageSourceMode(root.modelRef.storageSourceMode)
    }

    function storageRestEnabled() {
        return root.modelRef.sourceModeUsesEndpoint("storage", root.modelRef.storageSourceMode, "rest")
    }

    function storageMetricsEnabled() {
        return root.modelRef.sourceModeUsesEndpoint("storage", root.modelRef.storageSourceMode, "metrics")
    }

    function storageDataEnabled() {
        return root.modelRef.sourceModeSupportsCidProbe("storage", root.modelRef.storageSourceMode)
    }
}
