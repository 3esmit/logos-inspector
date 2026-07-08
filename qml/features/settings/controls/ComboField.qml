pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property string label: ""
    property string accessibleName: label
    property var options
    property int currentIndex: 0
    signal activated(int index)

    spacing: 6
    Layout.fillWidth: true

    Text {
        text: root.label
        color: root.theme.textMuted
        textFormat: Text.PlainText
        font.pixelSize: root.theme.secondaryText
        font.weight: Font.Medium
        Layout.fillWidth: true
    }

    ProfileComboBox {
        theme: root.theme
        options: root.options
        accessibleName: root.accessibleName
        currentIndex: root.currentIndex
        Layout.fillWidth: true
        onProfileActivated: index => root.activated(index)
    }
}
