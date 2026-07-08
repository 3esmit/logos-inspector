pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property string readTitle: ""
    property var refreshActions: []
    property bool pending: false
    property string statusText: ""
    property string guardedTitle: ""
    property bool permissionEnabled: false
    property string permissionEnabledTitle: qsTr("Permission enabled")
    property string permissionDisabledTitle: qsTr("Permission disabled")
    property string guardedMessage: ""
    property var guardedActions: []
    property string evidenceTitle: qsTr("Probe evidence")
    property var evidenceRows: []

    signal refreshRequested()

    spacing: root.theme.gap
    Layout.fillWidth: true

    Panel {
        theme: root.theme
        title: root.readTitle

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Repeater {
                model: root.refreshActions

                ActionButton {
                    required property var modelData

                    theme: root.theme
                    text: String(modelData.text || "")
                    enabled: !root.pending
                    Layout.preferredWidth: Number(modelData.width || 140)
                    accessibleName: String(modelData.accessibleName || modelData.text || "")
                    onClicked: root.refreshRequested()
                }
            }

            Text {
                text: root.statusText
                color: root.theme.textMuted
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font.pixelSize: root.theme.secondaryText
                Layout.fillWidth: true
            }
        }
    }

    Panel {
        theme: root.theme
        title: root.guardedTitle

        StatusMessage {
            theme: root.theme
            tone: root.permissionEnabled ? "warning" : "info"
            title: root.permissionEnabled ? root.permissionEnabledTitle : root.permissionDisabledTitle
            message: root.guardedMessage
            Layout.fillWidth: true
        }

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Repeater {
                model: root.guardedActions

                ActionButton {
                    required property var modelData

                    theme: root.theme
                    text: String(modelData.text || "")
                    enabled: false
                    Layout.preferredWidth: Number(modelData.width || 130)
                }
            }

            Text {
                text: qsTr("Adapters pending")
                color: root.theme.textDim
                textFormat: Text.PlainText
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }
        }
    }

    Panel {
        theme: root.theme
        title: root.evidenceTitle

        Repeater {
            model: root.evidenceRows

            StatusRow {
                required property var modelData

                theme: root.theme
                label: String(modelData.label || "")
                stateText: String(modelData.state || "")
                evidence: String(modelData.evidence || "")
                source: String(modelData.source || "")
                freshness: String(modelData.freshness || "")
                tone: String(modelData.tone || "neutral")
            }
        }
    }
}
