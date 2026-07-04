pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../state"
import "../theme"

Pane {
    id: root

    required property Theme theme
    required property AppModel model
    property bool compact: false

    padding: 18

    background: Rectangle {
        color: root.theme.sidebar
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
                    text: root.model.statusText
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
                    model: root.model.navRows()

                    delegate: Component {
                        RowLayout {
                            id: navRow

                            required property int index
                            required property var modelData

                            readonly property bool isGroup: String(modelData.type || "") === "group"
                            readonly property int depth: Number(modelData.depth || 0)

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
                                    const key = String(navRow.modelData.key || "")
                                    Qt.callLater(function () {
                                        root.model.toggleNavGroup(key)
                                    })
                                }

                                contentItem: RowLayout {
                                    spacing: root.theme.gapSmall

                                    Text {
                                        visible: !root.compact
                                        text: navRow.modelData.expanded === true ? "v" : ">"
                                        color: navRow.modelData.active === true ? root.theme.accent : root.theme.textDim
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
                                        text: root.groupText(navRow.modelData)
                                        color: navRow.modelData.active === true ? root.theme.text : root.theme.textMuted
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
                                ToolTip.text: String(navRow.modelData.label || "")
                                Accessible.role: Accessible.Button
                                Accessible.name: qsTr("%1 navigation group").arg(String(navRow.modelData.label || ""))
                            }

                            ActionButton {
                                id: navButton

                                visible: !navRow.isGroup
                                theme: root.theme
                                text: root.navText(navRow.modelData)
                                accessibleName: String(navRow.modelData.label || "")
                                selected: navRow.modelData.active === true
                                Layout.fillWidth: true
                                onClicked: {
                                    const view = String(navRow.modelData.view || "")
                                    Qt.callLater(function () {
                                        root.model.selectView(view)
                                    })
                                }
                                ToolTip.visible: (hovered || activeFocus) && root.compact
                                ToolTip.text: String(navRow.modelData.label || "")
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
}
