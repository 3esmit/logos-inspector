pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../theme"

Control {
    id: root

    required property Theme theme
    required property ListModel options
    property string current: ""
    signal selected(string value)

    readonly property int tabSpacing: root.theme.gapSmall
    readonly property int tabPadding: 24
    readonly property int minTabWidth: 56
    readonly property int baseTabWidth: 96
    readonly property int optionCount: root.options ? root.options.count : 0

    Layout.fillWidth: true
    implicitWidth: Math.min(root.naturalWidthTotal(), root.width > 0 ? root.width : 360)
    implicitHeight: 38
    padding: 0

    contentItem: Item {
        implicitWidth: root.implicitWidth
        implicitHeight: root.implicitHeight

        Flickable {
            id: tabScroller

            anchors.fill: parent
            boundsBehavior: Flickable.StopAtBounds
            clip: true
            contentWidth: tabRow.implicitWidth
            contentHeight: height
            flickableDirection: Flickable.HorizontalFlick
            interactive: contentWidth > width

            Row {
                id: tabRow

                height: tabScroller.height
                spacing: root.tabSpacing

                Repeater {
                    id: tabRepeater

                    model: root.options

                    delegate: Component {
                        TabButton {
                            id: optionTab

                            required property int index
                            required property string value
                            required property string label
                            readonly property bool active: root.current === value

                            text: qsTr(label)
                            checked: active
                            hoverEnabled: true
                            activeFocusOnTab: true
                            width: root.tabWidth(label)
                            height: parent ? parent.height : root.implicitHeight
                            padding: 0
                            onActiveFocusChanged: {
                                if (activeFocus) {
                                    root.ensureVisible(optionTab)
                                }
                            }
                            onClicked: {
                                root.ensureVisible(optionTab)
                                root.selected(value)
                            }
                            Keys.onLeftPressed: root.activateRelative(index, -1)
                            Keys.onRightPressed: root.activateRelative(index, 1)

                            contentItem: Text {
                                text: optionTab.text
                                color: optionTab.active ? root.theme.text : (optionTab.hovered ? root.theme.text : root.theme.textMuted)
                                elide: Text.ElideRight
                                textFormat: Text.PlainText
                                verticalAlignment: Text.AlignVCenter
                                horizontalAlignment: Text.AlignHCenter
                                font.pixelSize: 14
                                font.weight: optionTab.active ? Font.DemiBold : Font.Medium
                            }

                            background: Item {
                                Rectangle {
                                    anchors.left: parent.left
                                    anchors.right: parent.right
                                    anchors.bottom: parent.bottom
                                    height: 2
                                    color: optionTab.active ? root.theme.accent : (optionTab.hovered ? root.theme.outline : "transparent")
                                }

                                Rectangle {
                                    anchors.fill: parent
                                    visible: optionTab.activeFocus
                                    color: "transparent"
                                    radius: root.theme.radius
                                    border.width: 1
                                    border.color: root.theme.accent
                                }
                            }

                            Accessible.role: Accessible.PageTab
                            Accessible.name: active ? qsTr("%1 selected").arg(label) : label
                        }
                    }
                }
            }

            ScrollBar.horizontal: ScrollBar {
                policy: tabScroller.contentWidth > tabScroller.width ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff
            }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 1
            color: root.theme.outlineMuted
            z: -1
        }
    }

    Accessible.role: Accessible.PageTabList

    function activateRelative(currentIndex, delta) {
        if (root.optionCount === 0) {
            return
        }

        const nextIndex = (currentIndex + delta + root.optionCount) % root.optionCount
        const nextTab = tabRepeater.itemAt(nextIndex)
        if (nextTab) {
            nextTab.forceActiveFocus()
            root.ensureVisible(nextTab)
            root.selected(root.options.get(nextIndex).value)
        }
    }

    function ensureVisible(item) {
        if (!item || tabScroller.width <= 0 || tabScroller.contentWidth <= tabScroller.width) {
            return
        }

        const left = item.x
        const right = item.x + item.width
        if (left < tabScroller.contentX) {
            tabScroller.contentX = left
        } else if (right > tabScroller.contentX + tabScroller.width) {
            tabScroller.contentX = Math.min(right - tabScroller.width, tabScroller.contentWidth - tabScroller.width)
        }
    }

    function naturalTabWidth(label) {
        return Math.max(root.baseTabWidth, String(label).length * 8 + root.tabPadding)
    }

    function naturalWidthTotal() {
        if (root.optionCount === 0) {
            return 0
        }

        let total = root.tabSpacing * (root.optionCount - 1)
        for (let i = 0; i < root.optionCount; i += 1) {
            total += root.naturalTabWidth(root.options.get(i).label)
        }
        return total
    }

    function compressedTabWidth() {
        if (root.optionCount === 0 || root.width <= 0) {
            return root.baseTabWidth
        }

        return Math.max(root.minTabWidth, Math.floor((root.width - root.tabSpacing * (root.optionCount - 1)) / root.optionCount))
    }

    function tabWidth(label) {
        if (root.width > 0 && root.naturalWidthTotal() > root.width) {
            return root.compressedTabWidth()
        }

        return root.naturalTabWidth(label)
    }
}
