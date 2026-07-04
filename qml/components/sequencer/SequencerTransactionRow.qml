pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import ".."
import "../../state"
import "../../theme"

Item {
    id: root

    required property Theme theme
    property var columns: []
    property string hash: ""
    property string program: ""
    property bool header: false
    property AppModel modelRef

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
        columns: 4
        columnSpacing: 10

        Repeater {
            model: 4

            LinkCell {
                required property int index

                theme: root.theme
                text: String(root.columns[index] || "-")
                header: root.header
                link: root.linkFor(index)
                copyText: root.copyValueFor(index)
                monospace: !root.header
                Layout.preferredWidth: root.columnWidth(index)
                Layout.fillWidth: index === 1 || index === 3
                onActivated: {
                    if (root.modelRef === null) {
                        return
                    }
                    if (index === 1) {
                        root.modelRef.openReference("transaction", root.hash)
                    } else if (index === 3) {
                        root.modelRef.openReference("program", root.program)
                    }
                }
            }
        }
    }

    function linkFor(index) {
        if (root.header) {
            return false
        }
        if (index === 1) {
            return root.hash.length > 0
        }
        if (index === 3) {
            return root.program.length > 0
        }
        return false
    }

    function copyValueFor(index) {
        if (index === 1 && root.hash.length > 0) {
            return root.hash
        }
        if (index === 3 && root.program.length > 0) {
            return root.program
        }
        return String(root.columns[index] || "")
    }

    function columnWidth(index) {
        if (index === 0) {
            return 68
        }
        if (index === 2) {
            return 96
        }
        return 180
    }
}
