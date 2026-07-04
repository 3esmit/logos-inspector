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
            accessibleName: qsTr("Delivery source mode")
            options: root.sourceOptions
            currentIndex: root.sourceIndexFor(root.modelRef.messagingSourceMode)
            onActivated: index => root.modelRef.messagingSourceMode = root.sourceModeAt(index)
        }

        InfoField {
            theme: root.theme
            label: qsTr("Module API")
            value: root.modelRef.deliveryModule
        }

        FieldRow {
            theme: root.theme
            label: qsTr("Waku REST URL")
            sourceText: root.modelRef.messagingRestUrl
            syncSourceText: true
            placeholderText: qsTr("http://127.0.0.1:8645")
            onTextEdited: text => root.modelRef.messagingRestUrl = String(text || "").trim()
        }

        FieldRow {
            theme: root.theme
            label: qsTr("Metrics URL")
            sourceText: root.modelRef.messagingMetricsUrl
            syncSourceText: true
            placeholderText: qsTr("http://127.0.0.1:8008/metrics")
            onTextEdited: text => root.modelRef.messagingMetricsUrl = String(text || "").trim()
        }

        FieldRow {
            theme: root.theme
            label: qsTr("Network preset")
            sourceText: root.modelRef.messagingNetworkPreset
            syncSourceText: true
            placeholderText: qsTr("logos.test")
            onTextEdited: text => root.modelRef.messagingNetworkPreset = root.modelRef.normalizedMessagingNetworkPreset(text)
        }

        FieldRow {
            theme: root.theme
            label: qsTr("Node info id")
            sourceText: root.modelRef.messagingNodeInfoId
            syncSourceText: true
            placeholderText: qsTr("Optional getNodeInfo id")
            onTextEdited: text => root.modelRef.messagingNodeInfoId = String(text || "").trim()
        }

        RefreshRateField {
            theme: root.theme
            value: root.modelRef.messagingRefreshRate
            onRateEdited: value => root.modelRef.setNetworkConnectionRate("messaging", value)
        }

        SecondsField {
            theme: root.theme
            label: qsTr("Rolling window")
            value: root.modelRef.messagingRollingWindow
            onValueEdited: value => root.modelRef.messagingRollingWindow = value
        }
    }

    Flow {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        SafetyToggle {
            theme: root.theme
            text: qsTr("Admin REST")
            detail: qsTr("Allows privileged read-only admin endpoints when a future adapter uses them.")
            checked: root.modelRef.messagingAdminRestEnabled
            onToggled: root.modelRef.messagingAdminRestEnabled = checked
        }

        SafetyToggle {
            theme: root.theme
            text: qsTr("Mutating diagnostics")
            detail: qsTr("Allows future publish, subscribe, dial, and lightpush probes after per-action confirmation.")
            checked: root.modelRef.messagingMutatingDiagnosticsEnabled
            onToggled: root.modelRef.messagingMutatingDiagnosticsEnabled = checked
        }
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
            accessibleName: qsTr("Query Delivery status")
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
        const source = String(value || "module")
        for (let i = 0; i < root.sourceOptions.count; ++i) {
            if (root.sourceOptions.get(i).key === source) {
                return i
            }
        }
        return 0
    }

    function sourceModeAt(index) {
        if (index < 0 || index >= root.sourceOptions.count) {
            return "module"
        }
        return root.sourceOptions.get(index).key
    }
}
