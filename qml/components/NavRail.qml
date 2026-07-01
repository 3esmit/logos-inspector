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
        color: theme.sidebar
    }

    contentItem: ColumnLayout {
        spacing: 14

        RowLayout {
            spacing: 10
            Layout.fillWidth: true

            Rectangle {
                radius: 8
                color: theme.accent
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34

                Text {
                    anchors.centerIn: parent
                    text: qsTr("LI")
                    color: "#21160F"
                    textFormat: Text.PlainText
                    font.pixelSize: 13
                    font.weight: Font.Bold
                }
            }

            ColumnLayout {
                visible: !root.compact
                spacing: 1
                Layout.fillWidth: true

                Text {
                    text: qsTr("Logos Inspector")
                    color: theme.text
                    textFormat: Text.PlainText
                    font.pixelSize: 16
                    font.weight: Font.DemiBold
                    Layout.fillWidth: true
                }

                Text {
                    text: root.model.statusText
                    color: theme.textMuted
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
                spacing: 6

                Repeater {
                    model: root.model.navItems

                    delegate: Component {
                        ActionButton {
                            id: navButton

                            required property int index
                            required property string key
                            required property string label

                            theme: root.theme
                            text: root.compact ? String(index + 1) : label
                            selected: root.model.currentView === key
                            Layout.fillWidth: true
                            onClicked: root.model.selectView(key)
                        }
                    }
                }
            }
        }

        Text {
            visible: !root.compact
            text: root.model.networkProfile
            color: theme.textMuted
            elide: Text.ElideRight
            textFormat: Text.PlainText
            font.pixelSize: 12
            Layout.fillWidth: true
        }
    }
}
