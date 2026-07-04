pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property string titleText: ""
    property string copyText: ""
    property string tooltipText: ""
    property string alternateText: ""

    spacing: 6
    Layout.fillWidth: true

    AccountCopyTextLine {
        text: root.titleText
        theme: root.theme
        copyText: root.copyText
        tooltipText: root.tooltipText
        monospace: true
        textColor: root.theme.text
        textPixelSize: 22
        textWeight: Font.DemiBold
        Layout.fillWidth: true
    }

    AccountCopyTextLine {
        visible: root.alternateText.length > 0
        text: root.alternateText
        theme: root.theme
        copyText: root.alternateText
        monospace: true
        textColor: root.theme.textMuted
        textPixelSize: 12
        Layout.fillWidth: true
    }
}
