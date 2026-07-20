pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../theme"

RowLayout {
    id: root

    required property Theme theme
    property var items: []
    property bool first: false
    property bool compact: false

    spacing: root.theme.gapSmall

    Rectangle {
        visible: !root.first && !root.compact
        color: root.theme.outline
        radius: width / 2
        Layout.preferredWidth: 1
        Layout.preferredHeight: 14
        Layout.alignment: Qt.AlignVCenter
        Accessible.ignored: true
    }

    Repeater {
        model: root.items

        FooterStatusToken {
            required property var modelData

            visible: !root.compact || String(modelData.priority || "normal") !== "low"
            theme: root.theme
            label: String(modelData.label || "")
            value: String(modelData.value || "")
            accessibleValue: String(modelData.accessibleValue || modelData.value || "-")
            tone: String(modelData.tone || "neutral")
            fullName: String(modelData.fullName || modelData.label || "")
            maximumTokenWidth: modelData.maximumWidth || 150
            valueVisible: modelData.valueVisible !== false
            showDot: modelData.showDot !== false
            Layout.alignment: Qt.AlignVCenter
        }
    }

    component FooterStatusToken: Control {
        id: token

        required property Theme theme
        property string label: ""
        property string value: "-"
        property string accessibleValue: value
        property string tone: "neutral"
        property string fullName: ""
        property int maximumTokenWidth: 140
        property bool valueVisible: true
        property bool showDot: true

        hoverEnabled: true
        padding: 0
        implicitWidth: Math.min(tokenRow.implicitWidth, maximumTokenWidth)
        implicitHeight: 22

        background: Item {}

        contentItem: RowLayout {
            id: tokenRow

            spacing: token.theme.gapTiny

            Rectangle {
                visible: token.showDot
                color: token.toneColor()
                radius: width / 2
                Layout.preferredWidth: 7
                Layout.preferredHeight: 7
                Layout.alignment: Qt.AlignVCenter
                Accessible.ignored: true
            }

            Text {
                text: token.label
                color: token.theme.textDim
                textFormat: Text.PlainText
                font.pixelSize: token.theme.labelText
                font.weight: Font.DemiBold
                font.capitalization: Font.AllUppercase
                elide: Text.ElideRight
                Layout.maximumWidth: Math.max(74, token.maximumTokenWidth - 70)
            }

            Text {
                text: token.value
                visible: token.valueVisible && token.value.length > 0
                color: token.valueColor()
                textFormat: Text.PlainText
                font.pixelSize: token.theme.dataText
                font.family: "monospace"
                font.weight: Font.Medium
                elide: Text.ElideRight
                Layout.maximumWidth: Math.max(44, token.maximumTokenWidth - 84)
            }
        }

        ToolTip.visible: hovered && token.fullName.length > 0
        ToolTip.delay: 350
        ToolTip.text: qsTr("%1: %2").arg(token.fullName).arg(token.accessibleValue)

        Accessible.role: Accessible.StaticText
        Accessible.name: qsTr("%1: %2").arg(token.fullName.length > 0 ? token.fullName : token.label).arg(token.accessibleValue)

        function toneColor() {
            if (token.tone === "success") {
                return token.theme.success
            }
            if (token.tone === "warning") {
                return token.theme.warning
            }
            if (token.tone === "error") {
                return token.theme.error
            }
            if (token.tone === "info") {
                return token.theme.info
            }
            return token.theme.textDim
        }

        function valueColor() {
            if (token.tone === "error") {
                return token.theme.error
            }
            if (token.tone === "warning") {
                return token.theme.warning
            }
            if (token.tone === "success") {
                return token.theme.success
            }
            return token.theme.text
        }
    }
}
