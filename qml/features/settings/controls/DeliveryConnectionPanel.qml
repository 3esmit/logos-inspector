pragma ComponentBehavior: Bound

import QtQuick
import "../../../components"
import "../../../state"

SourceSettingsPanel {
    id: root

    property real pageWidth: 900
    property AppModel modelRef
    property var sourceOptions

    busy: root.modelRef ? root.modelRef.shell.busy : false
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
                || root.deliverySource().usesHealthEndpoint
            theme: root.theme
            label: root.deliverySource().usesHealthEndpoint
                ? qsTr("Waku REST health URL") : qsTr("Waku REST URL")
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
            onTextEdited: text => root.modelRef.messagingNetworkPreset = root.modelRef.sourceRouting.normalizedMessagingNetworkPreset(text)
        }

        RefreshRateField {
            theme: root.theme
            accessibleName: qsTr("Delivery auto refresh")
            accessibleDescription: qsTr("Automatic Delivery status refresh interval in seconds. Set to 0 to turn it off.")
            value: root.modelRef.metrics.messagingRefreshRate
            onRateEdited: value => root.modelRef.metrics.setNetworkConnectionRate("messaging", value)
        }

        SecondsField {
            theme: root.theme
            label: qsTr("Rolling window")
            accessibleName: qsTr("Delivery rolling window")
            accessibleDescription: qsTr("Delivery metrics rolling window in seconds.")
            value: root.modelRef.messagingRollingWindow
            onValueEdited: value => root.modelRef.messagingRollingWindow = value
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
