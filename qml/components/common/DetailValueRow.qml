pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import ".."
import "../../theme"

Item {
    id: root

    required property Theme theme
    property string label: ""
    property string value: "-"
    property string subvalue: ""
    property string linkKind: ""
    property string linkValue: ""
    property string copyText: linkValue.length > 0 ? linkValue : value
    property bool monospace: true
    property bool copyable: linkKind.length > 0
    property int labelWidth: 132
    property int labelPixelSize: theme.labelText
    property int valuePixelSize: theme.dataText
    signal activated(string kind, string value)

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
            wrapMode: Text.WrapAnywhere
            font.pixelSize: root.labelPixelSize
            font.weight: Font.DemiBold
            font.capitalization: Font.AllUppercase
            Layout.preferredWidth: root.labelWidth
            Layout.maximumWidth: root.labelWidth
            Layout.alignment: Qt.AlignTop
        }

        ColumnLayout {
            spacing: 2
            Layout.fillWidth: true

            LinkCell {
                text: root.value
                theme: root.theme
                link: root.linkKind.length > 0
                copyable: root.copyable
                copyText: root.copyText.length > 0 ? root.copyText : root.value
                monospace: root.monospace
                wrap: true
                textPixelSize: root.valuePixelSize
                Layout.fillWidth: true
                onActivated: root.activated(root.linkKind, root.linkValue)
            }

            Text {
                visible: root.subvalue.length > 0
                text: root.subvalue
                color: root.theme.textDim
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
                font.family: "monospace"
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }
        }
    }
}
