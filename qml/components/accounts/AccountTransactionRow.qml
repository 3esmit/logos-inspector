pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import ".."
import "../../theme"

Item {
    id: root

    required property Theme theme
    property var columns: []
    property string txHash: ""
    property string programId: ""
    property bool header: false

    signal cellActivated(int column)

    Layout.fillWidth: true
    Layout.preferredHeight: root.header ? 36 : 42

    Rectangle {
        anchors.fill: parent
        color: root.header ? root.theme.field : "transparent"
        border.width: 0
    }

    GridLayout {
        anchors.fill: parent
        anchors.leftMargin: 14
        anchors.rightMargin: 14
        columns: root.columns.length > 0 ? root.columns.length : 5
        columnSpacing: 10

        Repeater {
            model: root.columns.length > 0 ? root.columns.length : 5

            LinkCell {
                required property int index

                theme: root.theme
                text: String(root.columns[index] || "-")
                header: root.header
                link: root.linkFor(index)
                copyText: root.copyValueFor(index)
                monospace: !root.header
                Layout.preferredWidth: root.columnWidth(index)
                Layout.fillWidth: index === 0 || index === 2 || index === 3
                onActivated: root.cellActivated(index)
            }
        }
    }

    function linkFor(index) {
        return !root.header
            && ((index === 0 && root.txHash.length > 0)
                || (index === 3 && root.programId.length > 0))
    }

    function columnWidth(index) {
        if (index === 1 || index === 4) {
            return 92
        }
        if (index === 2) {
            return 160
        }
        return 180
    }

    function copyValueFor(index) {
        if (index === 0 && root.txHash.length > 0) {
            return root.txHash
        }
        if (index === 3 && root.programId.length > 0) {
            return root.programId
        }
        return String(root.columns[index] || "")
    }
}
