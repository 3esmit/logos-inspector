pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../components"
import "../../../theme"
import "../ZonePresentation.js" as Presentation

Rectangle {
    id: root

    required property Theme theme
    required property var source
    property var observation: null
    property string role: "sequencer"
    property bool selected: false
    property bool actionsEnabled: true
    readonly property string tone: Presentation.sourceTone(root.source, root.observation)
    readonly property string target: Presentation.targetText(root.source && root.source.target)
    readonly property bool insecureRemoteHttp: Presentation.remoteInsecureHttp(root.target)
    readonly property string binding: root.role === "indexer"
        ? "configured" : Presentation.bindingState(root.source, root.observation)
    signal selectRequested()
    signal editRequested()
    signal removeRequested()
    signal retryRequested()

    objectName: "channelSourceRow_" + String(root.source && root.source.source_id || "")
    implicitHeight: sourceColumn.implicitHeight + root.theme.gapLarge
    radius: root.theme.radius
    color: root.theme.field
    border.width: 1
    border.color: root.selected ? root.theme.accent : root.theme.outlineMuted

    RowLayout {
        anchors.fill: parent
        anchors.margins: root.theme.gapSmall
        spacing: root.theme.gapSmall

        ToneDot {
            theme: root.theme
            tone: root.tone
            Layout.preferredWidth: 8
            Layout.preferredHeight: 8
            Layout.alignment: Qt.AlignTop
            Layout.topMargin: 7
        }

        RadioButton {
            visible: root.role === "sequencer"
            checked: root.selected
            enabled: root.actionsEnabled
            text: ""
            focusPolicy: Qt.TabFocus
            padding: 0
            Layout.preferredWidth: 24
            Layout.preferredHeight: 24
            Layout.alignment: Qt.AlignTop
            onClicked: root.selectRequested()

            indicator: Rectangle {
                x: 4
                y: 4
                width: 16
                height: 16
                radius: 8
                color: "transparent"
                border.width: 1
                border.color: root.selected ? root.theme.accent : root.theme.outline

                Rectangle {
                    visible: root.selected
                    anchors.centerIn: parent
                    width: 8
                    height: 8
                    radius: 4
                    color: root.theme.accent
                }
            }

            contentItem: Item {}

            Accessible.name: checked
                ? qsTr("Selected Sequencer source") : qsTr("Select Sequencer source")
        }

        ColumnLayout {
            id: sourceColumn

            spacing: root.theme.gapTiny
            Layout.fillWidth: true

            RowLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                Text {
                    text: Presentation.text(root.source && root.source.label,
                        root.role === "sequencer" ? qsTr("Sequencer") : qsTr("Indexer"))
                    color: root.theme.text
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    font.pixelSize: root.theme.secondaryText
                    font.weight: Font.DemiBold
                    Layout.fillWidth: true
                }

                Text {
                    text: root.insecureRemoteHttp
                        ? qsTr("%1 / Insecure HTTP").arg(Presentation.words(root.binding))
                        : Presentation.words(root.binding)
                    color: root.insecureRemoteHttp ? root.theme.warning : root.theme.textMuted
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    font.pixelSize: root.theme.labelText
                    Layout.maximumWidth: 140
                }
            }

            LinkCell {
                theme: root.theme
                text: root.target
                copyText: root.target
                copyable: root.target !== "-"
                copyInline: false
                monospace: true
                fitSingleLine: true
                minimumPixelSize: 8
                textPixelSize: root.theme.dataText
                textColor: root.theme.textMuted
                Layout.fillWidth: true
            }

            RowLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                Text {
                    text: qsTr("Head %1").arg(Presentation.numberText(
                        root.observation && root.observation.head_block_id
                    ))
                    color: root.theme.textMuted
                    textFormat: Text.PlainText
                    font.pixelSize: root.theme.dataText
                    Layout.preferredWidth: 90
                }

                LinkCell {
                    theme: root.theme
                    text: Presentation.text(root.observation && root.observation.head_block_hash)
                    copyText: text === "-" ? "" : text
                    copyable: copyText.length > 0
                    copyInline: false
                    monospace: true
                    fitSingleLine: true
                    minimumPixelSize: 8
                    textPixelSize: root.theme.dataText
                    textColor: root.theme.textDim
                    Layout.fillWidth: true
                }

                Text {
                    text: Presentation.words(root.observation && root.observation.health)
                    color: root.theme.textMuted
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    font.pixelSize: root.theme.dataText
                    Layout.maximumWidth: 96
                }
            }

            Text {
                visible: text.length > 0
                text: String(root.observation && root.observation.last_error || "")
                color: root.theme.error
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }
        }

        ToolButton {
            id: sourceMenuButton

            enabled: root.actionsEnabled
            text: "\u22ee"
            hoverEnabled: true
            focusPolicy: Qt.TabFocus
            padding: 0
            Layout.preferredWidth: 30
            Layout.preferredHeight: 30
            Layout.alignment: Qt.AlignTop
            onClicked: sourceMenu.open()

            ToolTip.visible: hovered
            ToolTip.delay: 500
            ToolTip.text: qsTr("Source actions")

            background: Rectangle {
                radius: root.theme.radius
                color: sourceMenuButton.down ? root.theme.accentMuted
                    : (sourceMenuButton.hovered || sourceMenuButton.activeFocus
                        ? root.theme.hover : "transparent")
                border.width: sourceMenuButton.activeFocus ? 1 : 0
                border.color: root.theme.accent
            }

            contentItem: Text {
                text: sourceMenuButton.text
                color: root.theme.textMuted
                textFormat: Text.PlainText
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                font.pixelSize: 20
            }

            Accessible.name: qsTr("Source actions")
        }
    }

    Menu {
        id: sourceMenu

        MenuItem {
            text: qsTr("Edit")
            onTriggered: root.editRequested()
        }

        MenuItem {
            visible: root.role === "sequencer"
                && (root.binding === "pending" || root.binding === "channel_mismatch")
            text: qsTr("Retry attestation")
            onTriggered: root.retryRequested()
        }

        MenuSeparator {}

        MenuItem {
            text: qsTr("Remove")
            onTriggered: root.removeRequested()
        }
    }
}
