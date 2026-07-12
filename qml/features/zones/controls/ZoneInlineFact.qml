import QtQuick
import QtQuick.Layouts
import "../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property string label: ""
    property string value: "-"
    property string tone: "neutral"

    spacing: 1
    Layout.fillWidth: true
    Layout.minimumWidth: 68

    Text {
        text: root.label
        color: root.theme.textDim
        textFormat: Text.PlainText
        elide: Text.ElideRight
        font.pixelSize: root.theme.labelText
        Layout.fillWidth: true
    }

    RowLayout {
        spacing: root.theme.gapTiny
        Layout.fillWidth: true

        ToneDot {
            theme: root.theme
            tone: root.tone
            Layout.preferredWidth: 6
            Layout.preferredHeight: 6
            Layout.alignment: Qt.AlignVCenter
        }

        Text {
            text: root.value
            color: root.theme.text
            textFormat: Text.PlainText
            elide: Text.ElideRight
            font.pixelSize: root.theme.dataText
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }
    }
}
