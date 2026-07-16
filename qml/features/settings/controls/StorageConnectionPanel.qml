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

    busy: root.modelRef ? root.modelRef.shell.busy : false
    queryAccessibleName: qsTr("Query Storage status")

    SourceSettingsGrid {
        theme: root.theme
        pageWidth: root.pageWidth

        ComboField {
            theme: root.theme
            label: qsTr("Connector")
            accessibleName: qsTr("Storage connector")
            options: root.sourceOptions
            currentIndex: root.sourceIndexFor(root.modelRef.currentConnectorSourceMode("storage", "rest"))
            onActivated: index => root.modelRef.setNetworkConnectorMode("storage", root.sourceModeAt(index))
        }

        FieldRow {
            visible: root.storageRestEnabled()
            theme: root.theme
            label: qsTr("REST URL")
            sourceText: root.modelRef.storageRestUrl
            syncSourceText: true
            placeholderText: qsTr("http://127.0.0.1:8080/api/storage/v1")
            onTextEdited: text => root.modelRef.storageRestUrl = String(text || "").trim()
        }

        FieldRow {
            visible: root.storageMetricsEnabled()
            theme: root.theme
            label: qsTr("Metrics URL")
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
            visible: root.storageDataEnabled()
            theme: root.theme
            label: qsTr("CID local exists")
            sourceText: root.modelRef.storageCidProbe
            syncSourceText: true
            placeholderText: qsTr("Optional CID")
            onTextEdited: text => root.modelRef.storageCidProbe = String(text || "").trim()
        }

        RefreshRateField {
            theme: root.theme
            value: root.modelRef.metrics.storageRefreshRate
            onRateEdited: value => root.modelRef.metrics.setNetworkConnectionRate("storage", value)
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
    }

    StatusMessage {
        visible: root.storageSourceMode() === "unsupported"
        theme: root.theme
        tone: "warning"
        title: qsTr("Source unavailable")
        message: qsTr("The configured connector no longer has an adapter. Select Storage module, Standalone REST, or Metrics only.")
        Layout.fillWidth: true
    }

    function sourceIndexFor(value) {
        return root.modelRef.sourceRouting.sourceModeIndexFor("storage", value, root.sourceOptions)
    }

    function sourceModeAt(index) {
        return root.modelRef.sourceRouting.sourceModeAt(index, root.sourceOptions)
    }

    function storageSource() {
        return root.modelRef.sourceRouting.storageSourceView()
    }

    function storageSourceMode() {
        return root.storageSource().effectiveMode
    }

    function storageRestEnabled() {
        return root.storageSource().usesRestEndpoint
    }

    function storageMetricsEnabled() {
        return root.storageSource().usesMetricsEndpoint
    }

    function storageDataEnabled() {
        return root.storageSource().supportsCidProbe
    }
}
