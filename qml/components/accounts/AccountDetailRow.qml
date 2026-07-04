pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import ".."
import "../../theme"

Item {
    id: root

    required property Theme theme
    property string label: ""
    property string value: ""
    property string subvalue: ""
    property string subvalueCopyText: ""
    property string linkKind: ""
    property string linkValue: ""
    property string tooltipText: ""
    property bool monospace: true

    signal activated()

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
            wrapMode: Text.Wrap
            maximumLineCount: 2
            elide: Text.ElideRight
            font.pixelSize: 11
            font.weight: Font.DemiBold
            font.capitalization: Font.AllUppercase
            Layout.preferredWidth: 132
            Layout.maximumWidth: 132
            Layout.alignment: Qt.AlignTop
        }

        ColumnLayout {
            spacing: 2
            Layout.fillWidth: true

            LinkCell {
                text: root.value
                theme: root.theme
                link: root.linkKind.length > 0
                copyText: root.linkValue.length > 0 ? root.linkValue : root.value
                tooltipText: root.tooltipText
                monospace: root.monospace
                wrap: true
                Layout.fillWidth: true
                onActivated: root.activated()
            }

            LinkCell {
                visible: root.subvalue.length > 0
                text: root.subvalue
                theme: root.theme
                copyable: root.subvalueCopyText.length > 0
                copyText: root.subvalueCopyText
                monospace: true
                wrap: true
                textColor: root.theme.textDim
                textPixelSize: 11
                Layout.fillWidth: true
            }
        }
    }
}
