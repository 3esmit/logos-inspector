pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../theme"

Item {
    id: root

    required property Theme theme
    property string label: ""
    property string value: ""
    property string copyText: ""
    property string source: ""
    readonly property real layoutWidth: root.width > 0 ? root.width : (parent ? parent.width : 900)

    Layout.fillWidth: true
    implicitHeight: Math.max(48, rowGrid.implicitHeight + root.theme.gapSmall * 2)

    GridLayout {
        id: rowGrid

        anchors.fill: parent
        anchors.leftMargin: root.theme.gapSmall
        anchors.rightMargin: root.theme.gapSmall
        columns: root.layoutWidth < 720 ? 1 : 3
        columnSpacing: root.theme.gap
        rowSpacing: 2

        Text {
            text: root.label
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: root.theme.secondaryText
            font.weight: Font.DemiBold
            elide: Text.ElideRight
            Layout.preferredWidth: root.layoutWidth < 720 ? 0 : 150
            Layout.fillWidth: root.layoutWidth < 720
            Accessible.ignored: true
        }

        LinkCell {
            theme: root.theme
            text: root.value
            copyable: root.copyText.length > 0
            copyText: root.copyText
            link: false
            wrap: root.layoutWidth < 720
            accessibleName: root.rowAccessibleName()
            accessibleDescription: root.source
            copyAccessibleName: root.copyActionAccessibleName()
            copyAccessibleDescription: root.copyActionAccessibleDescription()
            Layout.fillWidth: true
        }

        Text {
            visible: root.layoutWidth >= 720
            text: root.source
            color: root.theme.textDim
            textFormat: Text.PlainText
            font.pixelSize: root.theme.dataText
            elide: Text.ElideRight
            Layout.preferredWidth: 180
            Accessible.ignored: true
        }
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 1
        color: root.theme.outlineMuted
        Accessible.ignored: true
    }

    function rowAccessibleName() {
        const labelText = root.label.trim()
        const valueText = root.value.trim()
        if (labelText.length > 0 && valueText.length > 0) {
            return qsTr("%1: %2").arg(labelText).arg(valueText)
        }
        return labelText.length > 0 ? labelText : valueText
    }

    function copyActionAccessibleName() {
        const field = root.label.trim()
        const valueText = root.value.trim()
        const target = field.length > 0 ? field
            : (valueText.length > 0 ? valueText : qsTr("detail"))
        return qsTr("Copy %1").arg(target)
    }

    function copyActionAccessibleDescription() {
        const field = root.label.trim().length > 0 ? root.label.trim() : qsTr("detail")
        const sourceText = root.source.trim()
        return sourceText.length > 0
            ? qsTr("Copies exact %1 value from %2.").arg(field).arg(sourceText)
            : qsTr("Copies exact %1 value.").arg(field)
    }
}
