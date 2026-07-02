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
                    model: root.model.navItems

                    delegate: Component {
                        ColumnLayout {
                            id: navItem

                            required property int index
                            required property string key
                            required property string label
                            required property string section

                            Layout.fillWidth: true
                            spacing: 4

                            Text {
                                visible: !root.compact && root.startsSection(navItem.index, navItem.section)
                                text: navItem.section
                                color: root.theme.textDim
                                textFormat: Text.PlainText
                                font.pixelSize: root.theme.labelText
                                font.weight: Font.DemiBold
                                font.capitalization: Font.AllUppercase
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                                Layout.topMargin: navItem.index === 0 ? 0 : root.theme.gapSmall
                            }

                            ActionButton {
                                id: navButton

                                theme: root.theme
                                text: root.compact ? root.navToken(navItem.key, navItem.label) : navItem.label
                                accessibleName: navItem.label
                                selected: root.model.currentView === navItem.key
                                Layout.fillWidth: true
                                onClicked: root.model.selectView(navItem.key)
                                ToolTip.visible: hovered && root.compact
                                ToolTip.text: navItem.label
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

    function startsSection(index, section) {
        if (index <= 0) {
            return true;
        }
        const previous = root.model.navItems.get(index - 1);
        return !previous || String(previous.section || "") !== String(section || "");
    }

    function navToken(key, label) {
        const lookup = {
            overview: "DAS",
            blocks: "BLK",
            transactions: "TX",
            wallets: "WLT",
            channels: "CHN",
            sequencer: "SEQ",
            accounts: "ACC",
            programs: "SPL",
            indexer: "IDX",
            blockchain: "BCH",
            storage: "STO",
            messaging: "MSG",
            capabilities: "CAP",
            settings: "SET"
        };
        if (lookup[key]) {
            return lookup[key];
        }
        const words = String(label || "").split(/\s+/).filter(function (word) { return word.length > 0; });
        if (!words.length) {
            return "--";
        }
        if (words.length > 1) {
            return String(words[0].charAt(0) + words[1].charAt(0)).toUpperCase();
        }
        const word = words[0];
        return String(word.slice(0, Math.min(3, word.length))).toUpperCase();
    }
}
