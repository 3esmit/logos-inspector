import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../state"

Panel {
    id: root

    required property AppModel model

    title: root.model.resultTitle

    RowLayout {
        spacing: 8
        Layout.fillWidth: true

        Text {
            text: model.resultIsError ? qsTr("Error") : qsTr("Output")
            color: model.resultIsError ? theme.error : theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: 13
            font.weight: Font.Medium
            Layout.fillWidth: true
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Clear")
            enabled: model.resultText.length > 0
            Layout.preferredWidth: 84
            onClicked: model.clearResult()
        }
    }

    TextArea {
        readOnly: true
        text: model.resultText.length ? model.resultText : qsTr("Run an inspection to see structured output.")
        wrapMode: TextArea.Wrap
        color: model.resultText.length ? theme.text : theme.textMuted
        selectedTextColor: "#21160F"
        selectionColor: theme.accent
        textFormat: Text.PlainText
        font.family: "monospace"
        font.pixelSize: 13
        leftPadding: 12
        rightPadding: 12
        topPadding: 10
        bottomPadding: 10
        Layout.fillWidth: true
        Layout.preferredHeight: 260

        background: Rectangle {
            color: model.resultIsError ? "#2D1917" : theme.field
            radius: theme.radius
            border.width: 1
            border.color: model.resultIsError ? theme.error : theme.outline
        }
    }
}
