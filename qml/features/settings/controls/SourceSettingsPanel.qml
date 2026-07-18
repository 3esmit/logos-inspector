pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"

Panel {
    id: root

    property string subtitle: ""
    property string statusText: qsTr("Unknown")
    property string statusDetail: ""
    property color statusColor: theme.textMuted
    property bool busy: false
    property bool queryEnabled: true
    property string queryAccessibleName: qsTr("Query %1 status").arg(root.title)
    default property alias bodyContent: body.data

    signal queryClicked()

    RowLayout {
        spacing: root.theme.gap
        Layout.fillWidth: true

        Text {
            text: root.subtitle
            color: root.theme.textMuted
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: root.theme.secondaryText
            Layout.fillWidth: true
        }

        StatusPill {
            theme: root.theme
            text: root.statusText
            colorToken: root.statusColor
            accessibleName: qsTr("%1 status: %2").arg(root.title).arg(
                root.statusText.length ? root.statusText : qsTr("Unknown"))
            accessibleDescription: root.statusDetail
        }
    }

    ColumnLayout {
        id: body

        spacing: root.theme.gap
        Layout.fillWidth: true
    }

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        ActionButton {
            theme: root.theme
            text: qsTr("Query status")
            primary: true
            enabled: root.queryEnabled && !root.busy
            Layout.preferredWidth: 132
            accessibleName: root.queryAccessibleName
            onClicked: root.queryClicked()
        }

        Text {
            text: root.statusDetail
            color: root.theme.textMuted
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: root.theme.dataText
            Layout.fillWidth: true
        }
    }
}
