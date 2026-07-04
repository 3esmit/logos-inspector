pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../theme"
import "../../utils/UiTone.js" as UiTone

Rectangle {
    id: root

    required property Theme theme
    property string label: ""
    property string value: "-"
    property string detail: ""
    property string tone: "neutral"
    property bool showIndicator: false
    property bool compact: false
    property bool valueMonospace: value.length > 18

    radius: root.theme.radius
    color: UiTone.toneFill(root.theme, root.tone)
    border.width: 1
    border.color: UiTone.toneBorder(root.theme, root.tone)
    implicitHeight: root.compact ? 46 : 50
    Layout.minimumWidth: 150

    RowLayout {
        anchors.fill: parent
        anchors.margins: root.theme.gapSmall
        spacing: root.theme.gapSmall

        Rectangle {
            visible: root.showIndicator
            radius: width / 2
            color: UiTone.toneColor(root.theme, root.tone)
            Layout.preferredWidth: 8
            Layout.preferredHeight: 8
            Layout.alignment: Qt.AlignTop
            Layout.topMargin: 5
            Accessible.ignored: true
        }

        ColumnLayout {
            spacing: root.compact ? 1 : 2
            Layout.fillWidth: true

            Text {
                text: root.label
                color: root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: root.theme.labelText
                font.weight: Font.DemiBold
                font.capitalization: Font.AllUppercase
                elide: Text.ElideRight
                Layout.fillWidth: true
            }

            Text {
                text: root.value.length ? root.value : "-"
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
                font.weight: root.compact ? Font.Normal : Font.Medium
                font.family: root.valueMonospace ? "monospace" : ""
                elide: Text.ElideMiddle
                Layout.fillWidth: true
            }
        }
    }

    Accessible.role: Accessible.StaticText
    Accessible.name: root.detail.length ? "%1: %2, %3".arg(root.label).arg(root.value).arg(root.detail) : "%1: %2".arg(root.label).arg(root.value)
}
