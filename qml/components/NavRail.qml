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
                spacing: 4

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

                            ActionButton {
                                id: navButton

                                theme: root.theme
                                text: root.navText(navRow.modelData)
                                accessibleName: String(navRow.modelData.label || "")
                                selected: navRow.modelData.active === true
                                Layout.fillWidth: true
                                onClicked: {
                                    if (navRow.isGroup) {
                                        root.model.toggleNavGroup(navRow.modelData.key)
                                    } else {
                                        root.model.selectView(navRow.modelData.view)
                                    }
                                }
                                ToolTip.visible: hovered && root.compact
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
        if (String(row.type || "") !== "group") {
            return label
        }
        return (row.expanded === true ? "- " : "+ ") + label
    }
}
