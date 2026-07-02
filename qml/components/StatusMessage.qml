import QtQuick
import QtQuick.Layouts
import "../theme"

Rectangle {
    id: root

    required property Theme theme
    property string title: ""
    property string message: ""
    property string tone: "info"

    radius: theme.radius
    color: root.fillColor()
    border.width: 1
    border.color: root.borderColor()
    Layout.fillWidth: true
    implicitHeight: body.implicitHeight + theme.gapLarge

    RowLayout {
        id: body

        anchors.fill: parent
        anchors.margins: root.theme.gap
        spacing: root.theme.gapSmall

        Rectangle {
            radius: 4
            color: root.dotColor()
            Layout.preferredWidth: 8
            Layout.preferredHeight: 8
            Layout.alignment: Qt.AlignTop
            Layout.topMargin: 5
            Accessible.ignored: true
        }

        ColumnLayout {
            spacing: root.theme.gapTiny
            Layout.fillWidth: true

            Text {
                visible: root.title.length > 0
                text: root.title
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            Text {
                text: root.message
                color: root.theme.textMuted
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: root.theme.secondaryText
                Layout.fillWidth: true
            }
        }
    }

    function fillColor() {
        if (root.tone === "error") {
            return root.theme.errorMuted;
        }
        if (root.tone === "warning") {
            return root.theme.warningMuted;
        }
        if (root.tone === "success") {
            return root.theme.successMuted;
        }
        return root.theme.infoMuted;
    }

    function borderColor() {
        if (root.tone === "error") {
            return root.theme.error;
        }
        if (root.tone === "warning") {
            return root.theme.warning;
        }
        if (root.tone === "success") {
            return root.theme.success;
        }
        return root.theme.outlineMuted;
    }

    function dotColor() {
        if (root.tone === "error") {
            return root.theme.error;
        }
        if (root.tone === "warning") {
            return root.theme.warning;
        }
        if (root.tone === "success") {
            return root.theme.success;
        }
        return root.theme.info;
    }

    Accessible.role: Accessible.StaticText
    Accessible.name: root.title.length > 0 ? qsTr("%1. %2").arg(root.title).arg(root.message) : root.message
}
