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
    queryAccessibleName: qsTr("Query Delivery status")

    SourceSettingsGrid {
        theme: root.theme
        pageWidth: root.pageWidth

        ComboField {
            theme: root.theme
            label: qsTr("Connector")
            accessibleName: qsTr("Delivery connector")
            options: root.sourceOptions
            currentIndex: root.sourceIndexFor(root.modelRef.currentConnectorSourceMode("delivery", "rest"))
            onActivated: index => root.modelRef.setNetworkConnectorMode("delivery", root.sourceModeAt(index))
        }

        FieldRow {
            visible: root.deliverySource().usesRestEndpoint
            theme: root.theme
            label: qsTr("Waku REST URL")
            sourceText: root.modelRef.messagingRestUrl
            syncSourceText: true
            placeholderText: qsTr("http://127.0.0.1:8645")
            onTextEdited: text => root.modelRef.messagingRestUrl = String(text || "").trim()
        }

        FieldRow {
            visible: root.messagingMetricsEnabled()
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
            text: qsTr("Mutating diagnostics")
            detail: qsTr("Allows publish, subscribe, and unsubscribe actions after per-action confirmation.")
            checked: root.modelRef.messagingMutatingDiagnosticsEnabled
            onToggled: root.modelRef.messagingMutatingDiagnosticsEnabled = checked
        }
    }

    function sourceIndexFor(value) {
        return root.modelRef.sourceRouting.sourceModeIndexFor("delivery", value, root.sourceOptions)
    }

    function sourceModeAt(index) {
        return root.modelRef.sourceRouting.sourceModeAt(index, root.sourceOptions)
    }

    function deliverySource() {
        return root.modelRef.sourceRouting.deliverySourceView()
    }

    function messagingMetricsEnabled() {
        return root.deliverySource().usesMetricsEndpoint
    }
}
