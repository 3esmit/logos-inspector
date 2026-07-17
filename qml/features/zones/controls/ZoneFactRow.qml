import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../theme"

RowLayout {
    id: root

    required property Theme theme
    property string label: ""
    property string value: "-"
    property string tone: "neutral"
    property bool copyable: false
    property bool monospace: false
    property bool fitSingleLine: copyable || value.length > 38

    spacing: root.theme.gapSmall
    Layout.fillWidth: true
    Layout.minimumHeight: 28

    Text {
        text: root.label
        color: root.theme.textMuted
        textFormat: Text.PlainText
        elide: Text.ElideRight
        font.pixelSize: root.theme.dataText
        Layout.preferredWidth: 112

        Accessible.role: Accessible.StaticText
        Accessible.name: root.label
    }

    LinkCell {
        theme: root.theme
        text: root.value
        copyText: root.value
        copyable: root.copyable
        copyInline: false
        monospace: root.monospace
        fitSingleLine: root.fitSingleLine
        minimumPixelSize: 8
        textPixelSize: root.theme.dataText
        textColor: root.theme.text
        Layout.fillWidth: true
    }

    ToneDot {
        visible: root.tone !== "neutral"
        theme: root.theme
        tone: root.tone
        Layout.preferredWidth: 7
        Layout.preferredHeight: 7
        Layout.alignment: Qt.AlignVCenter
    }
}
