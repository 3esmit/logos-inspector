pragma ComponentBehavior: Bound

import QtQuick
import QtQml.Models
import QtQuick.Layouts
import "../theme"

RowLayout {
    id: root

    required property Theme theme
    required property ListModel options
    property string current: ""
    signal selected(string value)

    spacing: 8
    Layout.fillWidth: true

    Repeater {
        model: root.options

        delegate: Component {
            ActionButton {
                id: optionButton

                required property string value
                required property string label

                theme: root.theme
                text: label
                selected: root.current === value
                Layout.preferredWidth: Math.max(118, implicitWidth)
                onClicked: root.selected(value)
            }
        }
    }
}
