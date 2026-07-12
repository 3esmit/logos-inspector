pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../theme"
import "../ZonePresentation.js" as Presentation

Rectangle {
    id: root

    required property Theme theme
    required property var zone
    property bool selected: false
    property bool stale: false
    property bool interactive: !stale
    readonly property string tone: Presentation.stateTone(root.zone, root.stale)
    signal activated()

    objectName: "zoneListRow_" + String(root.zone && root.zone.channel_id || "")
    implicitHeight: 112
    radius: root.theme.radius
    color: root.selected ? root.theme.accentMuted
        : (rowMouse.containsMouse && root.interactive ? root.theme.hover : root.theme.field)
    border.width: 1
    border.color: root.selected ? root.theme.accent : root.theme.outlineMuted
    activeFocusOnTab: root.interactive

    Keys.onEnterPressed: root.activated()
    Keys.onReturnPressed: root.activated()
    Keys.onSpacePressed: root.activated()

    MouseArea {
        id: rowMouse

        anchors.fill: parent
        enabled: root.interactive
        hoverEnabled: true
        cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.activated()
    }

    RowLayout {
        anchors.fill: parent
        anchors.margins: root.theme.gap
        spacing: root.theme.gapSmall

        ToneDot {
            theme: root.theme
            tone: root.tone
            Layout.preferredWidth: 9
            Layout.preferredHeight: 9
            Layout.alignment: Qt.AlignTop
            Layout.topMargin: 7
        }

        ColumnLayout {
            spacing: root.theme.gapTiny
            Layout.fillWidth: true

            RowLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                Text {
                    text: Presentation.title(root.zone)
                    color: root.theme.text
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    font.pixelSize: root.theme.primaryText
                    font.weight: Font.DemiBold
                    Layout.fillWidth: true
                }

                ZoneKindChip {
                    theme: root.theme
                    label: Presentation.kindLabel(root.zone && root.zone.kind)
                    tone: root.tone
                }
            }

            LinkCell {
                theme: root.theme
                text: String(root.zone && root.zone.channel_id || "")
                copyText: text
                copyable: true
                copyInline: false
                monospace: true
                fitSingleLine: true
                minimumPixelSize: 8
                textPixelSize: root.theme.dataText
                textColor: root.stale ? root.theme.textDim : root.theme.textMuted
                Layout.fillWidth: true
            }

            RowLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                ZoneInlineFact {
                    theme: root.theme
                    label: qsTr("L1 tip")
                    value: Presentation.numberText(root.zone && root.zone.l1_channel && root.zone.l1_channel.tip_slot)
                    tone: Presentation.finalityTone(root.zone && root.zone.l1_channel && root.zone.l1_channel.finality_state)
                }

                ZoneInlineFact {
                    theme: root.theme
                    label: Presentation.activityLabel(root.zone)
                    value: Presentation.activityValue(root.zone)
                    tone: root.tone
                }

                ZoneInlineFact {
                    theme: root.theme
                    label: qsTr("Link")
                    value: Presentation.words(root.zone && root.zone.settlement_link && root.zone.settlement_link.status)
                    tone: root.zone && root.zone.settlement_link && root.zone.settlement_link.status === "linked"
                        ? "success" : root.tone
                }

                ZoneInlineFact {
                    theme: root.theme
                    label: qsTr("Finality")
                    value: Presentation.zoneFinality(root.zone)
                    tone: Presentation.finalityTone(root.zone && root.zone.l1_channel && root.zone.l1_channel.finality_state)
                    Layout.minimumWidth: 82
                }
            }
        }
    }

    Rectangle {
        visible: root.activeFocus
        anchors.fill: parent
        color: "transparent"
        radius: root.theme.radius
        border.width: 1
        border.color: root.theme.accentHover
    }

    Accessible.role: Accessible.Button
    Accessible.name: qsTr("Open Zone %1").arg(String(root.zone && root.zone.channel_id || ""))
    Accessible.description: root.stale ? qsTr("Cached and unavailable until catalog verification completes") : ""
}
