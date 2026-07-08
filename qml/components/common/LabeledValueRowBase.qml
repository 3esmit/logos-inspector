pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../theme"

Item {
    id: root

    required property Theme theme
    property string label: ""
    property int labelWidth: 132
    property int labelPixelSize: theme.labelText
    property int labelMaximumLineCount: 0
    property int labelWrapMode: Text.WrapAnywhere

    default property alias valueContent: valueColumn.data

    Layout.fillWidth: true
    implicitHeight: Math.max(42, rowGrid.implicitHeight + 18)

    GridLayout {
        id: rowGrid

        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 12
        anchors.topMargin: 8
        anchors.bottomMargin: 8
        columns: 2
        columnSpacing: 14
        rowSpacing: 3

        Text {
            text: root.label
            color: root.theme.textMuted
            textFormat: Text.PlainText
            wrapMode: root.labelWrapMode
            maximumLineCount: root.labelMaximumLineCount
            elide: root.labelMaximumLineCount > 0 ? Text.ElideRight : Text.ElideNone
            font.pixelSize: root.labelPixelSize
            font.weight: Font.DemiBold
            font.capitalization: Font.AllUppercase
            Layout.preferredWidth: root.labelWidth
            Layout.maximumWidth: root.labelWidth
            Layout.alignment: Qt.AlignTop
        }

        ColumnLayout {
            id: valueColumn
            spacing: 2
            Layout.fillWidth: true
        }
    }
}
