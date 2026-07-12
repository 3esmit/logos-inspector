pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../theme"
import "../ZonePresentation.js" as Presentation

Rectangle {
    id: root

    required property Theme theme
    required property var zone
    property bool stale: false
    readonly property bool twoRows: width < 480
    readonly property var items: Presentation.statusItems(root.zone, root.stale)

    objectName: "zoneCompactStatus"
    implicitHeight: root.twoRows ? 132 : 78
    radius: root.theme.radius
    color: root.theme.surface
    border.width: 1
    border.color: root.theme.outlineMuted

    GridLayout {
        id: statusGrid

        anchors.fill: parent
        anchors.margins: root.theme.gapSmall
        columns: root.twoRows ? 2 : 4
        columnSpacing: 0
        rowSpacing: root.theme.gapSmall

        Repeater {
            model: root.items

            Item {
                id: statusItem

                required property var modelData
                required property int index

                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.minimumWidth: 104

                Rectangle {
                    visible: statusItem.index % statusGrid.columns !== 0
                    anchors.left: parent.left
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    width: 1
                    color: root.theme.outlineMuted
                }

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: statusItem.index % statusGrid.columns === 0
                        ? root.theme.gapTiny : root.theme.gap
                    anchors.rightMargin: root.theme.gapSmall
                    spacing: root.theme.gapSmall

                    ToneDot {
                        theme: root.theme
                        tone: statusItem.modelData.tone
                        Layout.preferredWidth: 8
                        Layout.preferredHeight: 8
                        Layout.alignment: Qt.AlignTop
                        Layout.topMargin: 6
                    }

                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true

                        Text {
                            text: statusItem.modelData.label
                            color: root.theme.textMuted
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.pixelSize: root.theme.labelText
                            font.weight: Font.DemiBold
                            Layout.fillWidth: true
                        }

                        Text {
                            text: statusItem.modelData.value
                            color: root.theme.text
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.pixelSize: root.theme.primaryText
                            font.weight: Font.DemiBold
                            Layout.fillWidth: true
                        }

                        Text {
                            text: statusItem.modelData.detail
                            color: root.theme.textDim
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.pixelSize: root.theme.dataText
                            Layout.fillWidth: true
                        }
                    }
                }
            }
        }
    }
}
