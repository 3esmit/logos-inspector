pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import ".."
import "../../theme"

Item {
    id: root

    required property Theme theme
    property string idlName: ""
    property string programIdText: ""
    property int fieldCount: 0
    property bool compact: false

    signal removeRequested()

    Layout.fillWidth: true
    implicitHeight: Math.max(52, rowLayout.implicitHeight + root.theme.gapLarge)

    GridLayout {
        id: rowLayout

        anchors.fill: parent
        anchors.leftMargin: root.theme.gap
        anchors.rightMargin: root.theme.gap
        anchors.topMargin: root.theme.gapSmall
        anchors.bottomMargin: root.theme.gapSmall
        columns: root.compact ? 2 : 4
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gapTiny

        ColumnLayout {
            spacing: root.theme.gapTiny
            Layout.fillWidth: true

            Text {
                text: root.idlName.length ? root.idlName : qsTr("Unnamed IDL")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
                font.weight: Font.DemiBold
                elide: Text.ElideRight
                Layout.fillWidth: true
            }

            Text {
                text: root.programIdText.length ? root.programIdText : qsTr("No program binding")
                color: root.programIdText.length ? root.theme.textMuted : root.theme.textDim
                textFormat: Text.PlainText
                font.family: "monospace"
                font.pixelSize: root.theme.dataText
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }

        Text {
            text: qsTr("%1 field(s)").arg(root.fieldCount)
            color: root.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: root.theme.dataText
            Layout.preferredWidth: 96
            Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Remove")
            Layout.preferredWidth: 96
            Layout.alignment: Qt.AlignVCenter | Qt.AlignRight
            onClicked: root.removeRequested()
        }
    }
}
