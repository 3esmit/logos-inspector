pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../theme"

ComboBox {
    id: root

    required property Theme theme
    property var options
    property string accessibleName: qsTr("Network profile")
    signal profileActivated(int index)

    model: root.options
    textRole: "label"
    valueRole: "key"
    hoverEnabled: true
    implicitHeight: root.theme.controlHeight
    Accessible.role: Accessible.ComboBox
    Accessible.name: root.accessibleName
    onActivated: index => root.profileActivated(index)

    contentItem: Text {
        text: root.displayText
        color: root.enabled ? root.theme.text : root.theme.textDim
        textFormat: Text.PlainText
        font.pixelSize: root.theme.primaryText
        font.weight: Font.Medium
        elide: Text.ElideRight
        verticalAlignment: Text.AlignVCenter
        leftPadding: 12
        rightPadding: 36
    }

    indicator: Text {
        x: root.width - width - 14
        y: (root.height - height) / 2
        text: "v"
        color: root.enabled ? root.theme.textMuted : root.theme.textDim
        textFormat: Text.PlainText
        font.pixelSize: root.theme.secondaryText
        font.weight: Font.DemiBold
    }

    background: Rectangle {
        radius: root.theme.radius
        color: root.hovered || root.activeFocus ? root.theme.surfaceRaised : root.theme.field
        border.width: root.activeFocus ? 2 : 1
        border.color: root.activeFocus ? root.theme.accent : root.theme.outlineMuted
    }

    delegate: ItemDelegate {
        id: delegateRoot

        required property int index
        required property string label
        required property string summary

        width: root.width
        implicitHeight: 54
        hoverEnabled: true
        highlighted: root.highlightedIndex === index
        Accessible.role: Accessible.ListItem
        Accessible.name: delegateRoot.label
        Accessible.description: delegateRoot.summary

        contentItem: ColumnLayout {
            spacing: root.theme.gapTiny

            Text {
                text: delegateRoot.label
                color: delegateRoot.highlighted ? root.theme.selectedText : root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
                font.weight: Font.DemiBold
                elide: Text.ElideRight
                Layout.fillWidth: true
            }

            Text {
                text: delegateRoot.summary
                color: delegateRoot.highlighted ? root.theme.selectedText : root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: root.theme.dataText
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }

        background: Rectangle {
            color: delegateRoot.highlighted ? root.theme.accent : (delegateRoot.hovered ? root.theme.hover : "transparent")
            radius: root.theme.radius
        }
    }

    popup: Popup {
        y: root.height + root.theme.gapTiny
        width: root.width
        implicitHeight: Math.min(contentItem.implicitHeight + 8, 296)
        padding: 4

        contentItem: ListView {
            clip: true
            implicitHeight: contentHeight
            model: root.popup.visible ? root.delegateModel : null
            currentIndex: root.highlightedIndex
        }

        background: Rectangle {
            color: root.theme.surfaceRaised
            radius: root.theme.radius
            border.width: 1
            border.color: root.theme.outline
        }
    }
}
