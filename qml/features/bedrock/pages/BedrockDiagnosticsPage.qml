pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../modules/controls"
import "../../../state"
import "../../../state/source_routing/BedrockDiagnosticsProjection.js" as BedrockDiagnosticsProjection
import "../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model
    readonly property var sourceView: BedrockDiagnosticsProjection.build(
        root.model, root.theme)

    width: parent ? parent.width : 900
    spacing: root.theme.gapLarge

    Component.onCompleted: {
        if (!root.sourceView.report && !root.sourceView.pending) {
            root.refreshSource(false)
        }
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Diagnostics / Bedrock")
        title: qsTr("Bedrock Diagnostics")
        layerLabel: qsTr("Diagnostics")
        subtitle: qsTr("Connection checks for the configured L1 Bedrock source. This page never changes node state.")
        Layout.fillWidth: true
    }

    SourceStrip {
        theme: root.theme
        sources: root.sourceView.sourceBadges
        Layout.fillWidth: true
    }

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        ActionButton {
            theme: root.theme
            text: qsTr("Refresh source")
            primary: true
            enabled: !root.sourceView.pending
            Layout.preferredWidth: 162
            accessibleName: qsTr("Refresh Bedrock source")
            onClicked: root.refreshSource(true)
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Open settings")
            enabled: !root.sourceView.pending
            Layout.preferredWidth: 126
            accessibleName: qsTr("Open Bedrock settings")
            onClicked: root.model.openSettings("network", "blockchain")
        }

        Text {
            text: root.sourceView.statusLine
            color: root.toneColor(root.sourceView.sourceStateTone)
            textFormat: Text.PlainText
            elide: Text.ElideRight
            font.pixelSize: root.theme.secondaryText
            font.weight: Font.Medium
            Layout.fillWidth: true
        }
    }

    GridLayout {
        columns: root.width < 760 ? 2 : 4
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gap
        Layout.fillWidth: true

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Connection")
            value: root.sourceView.sourceState
            delta: root.sourceView.freshnessText
            deltaColor: root.toneColor(root.sourceView.sourceStateTone)
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Source")
            value: root.sourceView.sourceShortLabel
            delta: root.sourceView.sourceTargetShort
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("API checks")
            value: root.sourceView.checksText
            delta: root.sourceView.checksDetail
            deltaColor: root.toneColor(root.sourceView.checksTone)
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Latest result")
            value: root.sourceView.status.known
                ? String(root.sourceView.status.text || qsTr("Completed"))
                : qsTr("Waiting")
            delta: root.sourceView.status.known
                ? String(root.sourceView.status.checkedAt || qsTr("now"))
                : qsTr("Opening check")
            deltaColor: root.toneColor(root.sourceView.sourceStateTone)
        }
    }

    StatusMessage {
        theme: root.theme
        tone: root.sourceView.notice.tone
        title: root.sourceView.notice.title
        message: root.sourceView.notice.message
        Layout.fillWidth: true
    }

    GridLayout {
        columns: root.width < 980 ? 1 : 2
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gap
        Layout.fillWidth: true

        StatusRowsPanel {
            theme: root.theme
            title: qsTr("Bedrock API checks")
            rows: root.sourceView.probeRows
        }

        DetailRowsPanel {
            theme: root.theme
            title: qsTr("Connection source")
            rows: root.sourceView.sourceRows
        }
    }

    DetailRowsPanel {
        theme: root.theme
        title: qsTr("Consensus facts")
        rows: root.sourceView.consensusRows
        Layout.fillWidth: true
    }

    function refreshSource(showResult) {
        return root.model.metrics.queryNetworkConnection(
            "blockchain", showResult === true, false, "bedrock-diagnostics")
    }

    function toneColor(tone) {
        if (tone === "success") {
            return root.theme.success
        }
        if (tone === "warning") {
            return root.theme.warning
        }
        if (tone === "error") {
            return root.theme.error
        }
        return root.theme.textMuted
    }
}
