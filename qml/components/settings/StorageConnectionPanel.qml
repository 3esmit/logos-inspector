pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import ".."
import "../../state"

Panel {
    id: root

    property real pageWidth: 900
    property AppModel modelRef
    property string subtitle: ""
    property string statusText: qsTr("Unknown")
    property string statusDetail: ""
    property color statusColor: theme.textMuted
    property var sourceOptions

    signal queryClicked()

    RowLayout {
        spacing: root.theme.gap
        Layout.fillWidth: true

        Text {
            text: root.subtitle
            color: root.theme.textMuted
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: root.theme.secondaryText
            Layout.fillWidth: true
        }

        StatusPill {
            theme: root.theme
            text: root.statusText
            colorToken: root.statusColor
        }
    }

    GridLayout {
        columns: root.pageWidth < 760 ? 1 : 2
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gap
        Layout.fillWidth: true

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
        message: qsTr("The saved source mode no longer has an adapter. Select Auto, Standalone REST, or Metrics only.")
        Layout.fillWidth: true
    }

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        ActionButton {
            theme: root.theme
            text: qsTr("Query status")
            primary: true
            enabled: !root.modelRef.busy
            Layout.preferredWidth: 132
            accessibleName: qsTr("Query Storage status")
            onClicked: root.queryClicked()
        }

        Text {
            text: root.statusDetail
            color: root.theme.textMuted
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: root.theme.dataText
            Layout.fillWidth: true
        }
    }

    function sourceIndexFor(value) {
        const source = root.modelRef.normalizedStorageSourceMode(value)
        for (let i = 0; i < root.sourceOptions.count; ++i) {
            if (root.sourceOptions.get(i).key === source) {
                return i
            }
        }
        return 0
    }

    function sourceModeAt(index) {
        if (index < 0 || index >= root.sourceOptions.count) {
            return "auto"
        }
        return root.sourceOptions.get(index).key
    }

    function storageSourceMode() {
        return root.modelRef.effectiveStorageSourceMode(root.modelRef.storageSourceMode)
    }

    function storageRestEnabled() {
        return root.storageSourceMode() === "rest"
    }

    function storageMetricsEnabled() {
        const source = root.storageSourceMode()
        return source === "rest" || source === "metrics"
    }

    function storageDataEnabled() {
        return root.storageSourceMode() === "rest"
    }
}
