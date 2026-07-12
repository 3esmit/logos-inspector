pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../theme"

Pane {
    id: root

    required property Theme theme
    property int loadCount: 20
    property var loadOptions: [10, 20, 50]
    property string rangeText: ""
    property bool canGoNewer: false
    property bool canGoOlder: false
    property bool busy: false
    property string refreshText: qsTr("Latest")
    property string newerText: qsTr("Newer")
    property string olderText: qsTr("Older")

    signal refresh()
    signal newer()
    signal older()
    signal loadCountSelected(int count)

    padding: 0
    Layout.fillWidth: true

    background: Item {}

    contentItem: GridLayout {
        id: toolbarGrid

        columns: root.width < 680 ? 1 : 2
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gapSmall

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignLeft | Qt.AlignVCenter

            Text {
                text: qsTr("Loaded")
                color: root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: root.theme.labelText
                font.weight: Font.DemiBold
                font.capitalization: Font.AllUppercase
                Layout.alignment: Qt.AlignVCenter
            }

            ComboBox {
                id: loadCombo

                model: root.loadOptions
                currentIndex: root.optionIndex(root.loadCount)
                hoverEnabled: true
                enabled: !root.busy
                Layout.preferredWidth: 86
                Layout.preferredHeight: root.theme.controlHeight
                onActivated: function (index) {
                    root.loadCountSelected(Number(root.loadOptions[index]))
                }

                delegate: ItemDelegate {
                    id: delegateRoot

                    required property int index
                    required property var modelData

                    width: loadCombo.width
                    text: String(modelData)
                    hoverEnabled: true
                    highlighted: loadCombo.highlightedIndex === index
                    font.pixelSize: root.theme.secondaryText

                    contentItem: Text {
                        text: delegateRoot.text
                        color: delegateRoot.highlighted ? root.theme.selectedText : root.theme.text
                        textFormat: Text.PlainText
                        verticalAlignment: Text.AlignVCenter
                        horizontalAlignment: Text.AlignHCenter
                        font: delegateRoot.font
                    }

                    background: Rectangle {
                        color: delegateRoot.highlighted ? root.theme.accent : (delegateRoot.hovered ? root.theme.hover : root.theme.surfaceRaised)
                    }
                }

                contentItem: Text {
                    text: String(loadCombo.currentValue || root.loadCount)
                    color: root.theme.text
                    textFormat: Text.PlainText
                    verticalAlignment: Text.AlignVCenter
                    horizontalAlignment: Text.AlignHCenter
                    font.pixelSize: root.theme.secondaryText
                    font.family: "monospace"
                    font.weight: Font.Medium
                }

                indicator: Text {
                    x: loadCombo.width - width - 8
                    y: (loadCombo.height - height) / 2
                    text: "\u25be"
                    color: root.theme.textMuted
                    textFormat: Text.PlainText
                    font.pixelSize: root.theme.labelText
                }

                background: Rectangle {
                    radius: root.theme.radius
                    color: loadCombo.hovered || loadCombo.activeFocus ? root.theme.surfaceRaised : root.theme.field
                    border.width: loadCombo.activeFocus ? 2 : 1
                    border.color: loadCombo.activeFocus ? root.theme.accent : root.theme.outlineMuted
                }

                popup: Popup {
                    y: loadCombo.height + 2
                    width: loadCombo.width
                    implicitHeight: contentItem.implicitHeight
                    padding: 1

                    contentItem: ListView {
                        clip: true
                        implicitHeight: contentHeight
                        model: loadCombo.popup.visible ? loadCombo.delegateModel : null
                        currentIndex: loadCombo.highlightedIndex
                    }

                    background: Rectangle {
                        radius: root.theme.radius
                        color: root.theme.surfaceRaised
                        border.width: 1
                        border.color: root.theme.outline
                    }
                }

                Accessible.role: Accessible.ComboBox
                Accessible.name: qsTr("Loaded row count")
            }

            Text {
                visible: root.rangeText.length > 0
                text: root.rangeText
                color: root.theme.textDim
                textFormat: Text.PlainText
                font.pixelSize: root.theme.dataText
                elide: Text.ElideRight
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter
            }
        }

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: root.width < 680
            Layout.alignment: root.width < 680 ? Qt.AlignLeft : Qt.AlignRight

            ActionButton {
                theme: root.theme
                text: root.refreshText
                primary: true
                enabled: !root.busy
                Layout.preferredWidth: 90
                onClicked: root.refresh()
            }

            ActionButton {
                theme: root.theme
                text: root.newerText
                enabled: !root.busy && root.canGoNewer
                Layout.preferredWidth: 90
                onClicked: root.newer()
            }

            ActionButton {
                theme: root.theme
                text: root.olderText
                enabled: !root.busy && root.canGoOlder
                Layout.preferredWidth: 90
                onClicked: root.older()
            }
        }
    }

    function optionIndex(count) {
        const wanted = Number(count)
        for (let i = 0; i < root.loadOptions.length; ++i) {
            if (Number(root.loadOptions[i]) === wanted) {
                return i
            }
        }
        return 0
    }
}
