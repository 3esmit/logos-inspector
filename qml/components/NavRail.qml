pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../state"
import "../theme"

Pane {
    id: root

    objectName: "mainNavigation"

    required property Theme theme
    required property AppModel model
    property bool compact: false
    readonly property var requestedNavigationRows: root.model
        ? root.model.navRows() : []
    signal navigationRequested(string view, string channelId)

    onRequestedNavigationRowsChanged: synchronizeNavigationRows()

    padding: 18

    background: Rectangle {
        color: root.theme.sidebar
    }

    ListModel {
        id: navigationRowsModel
        dynamicRoles: true
    }

    contentItem: ColumnLayout {
        spacing: 14

        RowLayout {
            spacing: 10
            Layout.fillWidth: true

            Item {
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34

                Image {
                    anchors.centerIn: parent
                    source: Qt.resolvedUrl("../../icons/inspector.svg")
                    sourceSize.width: 34
                    sourceSize.height: 34
                    fillMode: Image.PreserveAspectFit
                    asynchronous: true
                    Accessible.ignored: true
                }
            }

            ColumnLayout {
                visible: !root.compact
                spacing: 1
                Layout.fillWidth: true

                Text {
                    text: qsTr("Logos Inspector")
                    color: root.theme.text
                    textFormat: Text.PlainText
                    font.pixelSize: 16
                    font.weight: Font.DemiBold
                    Layout.fillWidth: true
                }

                Text {
                    text: root.model.shell.statusText
                    color: root.theme.textMuted
                    elide: Text.ElideRight
                    textFormat: Text.PlainText
                    font.pixelSize: 12
                    Layout.fillWidth: true
                }
            }
        }

        ScrollView {
            contentWidth: availableWidth
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
            Layout.fillWidth: true
            Layout.fillHeight: true

            ColumnLayout {
                width: parent ? parent.width : 180
                spacing: 3

                Repeater {
                    model: navigationRowsModel

                    delegate: Component {
                        RowLayout {
                            id: navRow

                            required property int index
                            required property var row

                            readonly property bool isGroup: String(row.type || "") === "group"
                            readonly property int depth: Number(row.depth || 0)

                            Layout.fillWidth: true
                            spacing: 4

                            Item {
                                visible: !root.compact && navRow.depth > 0
                                Layout.preferredWidth: navRow.depth * 12
                                Layout.preferredHeight: 1
                            }

                            Button {
                                id: groupButton

                                visible: navRow.isGroup
                                hoverEnabled: true
                                activeFocusOnTab: true
                                padding: 0
                                Layout.fillWidth: true
                                Layout.preferredHeight: root.compact ? 38 : 30
                                onClicked: {
                                    const key = String(navRow.row.key || "")
                                    Qt.callLater(function () {
                                        root.model.toggleNavGroup(key)
                                    })
                                }

                                contentItem: RowLayout {
                                    spacing: root.theme.gapSmall

                                    Text {
                                        visible: !root.compact
                                        text: navRow.row.expanded === true ? "v" : ">"
                                        color: navRow.row.active === true ? root.theme.accent : root.theme.textDim
                                        textFormat: Text.PlainText
                                        horizontalAlignment: Text.AlignHCenter
                                        verticalAlignment: Text.AlignVCenter
                                        font.family: "monospace"
                                        font.pixelSize: root.theme.dataText
                                        font.weight: Font.DemiBold
                                        Layout.preferredWidth: 12
                                        Layout.fillHeight: true
                                    }

                                    Text {
                                        text: root.groupText(navRow.row)
                                        color: navRow.row.active === true ? root.theme.text : root.theme.textMuted
                                        textFormat: Text.PlainText
                                        elide: Text.ElideRight
                                        verticalAlignment: Text.AlignVCenter
                                        horizontalAlignment: root.compact ? Text.AlignHCenter : Text.AlignLeft
                                        font.pixelSize: root.compact ? root.theme.dataText : root.theme.labelText
                                        font.weight: Font.DemiBold
                                        font.capitalization: root.compact ? Font.MixedCase : Font.AllUppercase
                                        Layout.fillWidth: true
                                        Layout.fillHeight: true
                                    }
                                }

                                background: Rectangle {
                                    radius: root.theme.radius
                                    color: groupButton.down
                                        ? root.theme.surfaceRaised
                                        : (groupButton.hovered || groupButton.activeFocus ? root.theme.hover : "transparent")
                                    border.width: groupButton.activeFocus ? 1 : 0
                                    border.color: root.theme.accent
                                }

                                ToolTip.visible: (groupButton.hovered || groupButton.activeFocus) && root.compact
                                ToolTip.text: String(navRow.row.label || "")
                                Accessible.role: Accessible.Button
                                Accessible.name: qsTr("%1 navigation group").arg(String(navRow.row.label || ""))
                            }

                            ActionButton {
                                id: navButton

                                objectName: "navButton_" + (String(navRow.row.channelId || "").length > 0
                                    ? "zone_" + String(navRow.row.channelId || "")
                                    : String(navRow.row.view || ""))
                                visible: !navRow.isGroup
                                theme: root.theme
                                text: root.navText(navRow.row)
                                accessibleName: String(navRow.row.accessibleName
                                    || navRow.row.label || "")
                                selected: navRow.row.active === true
                                enabled: navRow.row.enabled !== false
                                Layout.fillWidth: true
                                onClicked: {
                                    const view = String(navRow.row.view || "")
                                    root.navigationRequested(view,
                                        String(navRow.row.channelId || ""))
                                }
                                ToolTip.visible: (hovered || activeFocus) && root.compact
                                ToolTip.text: String(navRow.row.label || "")
                            }
                        }
                    }
                }
            }
        }

        Text {
            visible: !root.compact
            text: root.model.networkProfile
            color: root.theme.textMuted
            elide: Text.ElideRight
            textFormat: Text.PlainText
            font.pixelSize: 12
            Layout.fillWidth: true
        }
    }

    function navText(row) {
        if (root.compact) {
            return String(row.token || "--")
        }
        const label = String(row.label || "")
        return label
    }

    function groupText(row) {
        if (root.compact) {
            return String(row.token || "--")
        }
        return String(row.label || "")
    }

    function synchronizeNavigationRows() {
        const rows = Array.isArray(root.requestedNavigationRows)
            ? root.requestedNavigationRows : []
        const nextKeys = ({})
        for (let index = 0; index < rows.length; ++index) {
            const key = navigationRowKey(rows[index])
            if (key.length > 0) {
                nextKeys[key] = true
            }
        }

        for (let index = navigationRowsModel.count - 1; index >= 0; --index) {
            const key = String(navigationRowsModel.get(index).rowKey || "")
            if (nextKeys[key] !== true) {
                navigationRowsModel.remove(index)
            }
        }

        for (let index = 0; index < rows.length; ++index) {
            const row = rows[index] || ({})
            const key = navigationRowKey(row)
            if (key.length === 0) {
                continue
            }
            let currentIndex = navigationRowIndex(key)
            if (currentIndex < 0) {
                navigationRowsModel.insert(index, {
                    rowKey: key,
                    row: row
                })
                continue
            }
            if (currentIndex !== index) {
                navigationRowsModel.move(currentIndex, index, 1)
                currentIndex = index
            }
            if (navigationRowsModel.get(currentIndex).row !== row) {
                navigationRowsModel.setProperty(currentIndex, "row", row)
            }
        }
    }

    function navigationRowKey(row) {
        const value = row || ({})
        const type = String(value.type || "item")
        const key = String(value.key || "")
        const channelId = String(value.channelId || "")
        return key.length > 0 ? type + ":" + key + ":" + channelId : ""
    }

    function navigationRowIndex(key) {
        const target = String(key || "")
        for (let index = 0; index < navigationRowsModel.count; ++index) {
            if (String(navigationRowsModel.get(index).rowKey || "") === target) {
                return index
            }
        }
        return -1
    }

    Component.onCompleted: synchronizeNavigationRows()
}
