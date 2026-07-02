pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../theme"

Flow {
    id: root

    required property Theme theme
    required property ListModel options
    property string current: ""
    signal selected(string value)

    spacing: root.theme.gapSmall
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
                accessibleName: selected ? qsTr("%1 selected").arg(label) : label
                width: Math.min(Math.max(118, implicitWidth), root.width > 0 ? root.width : Math.max(118, implicitWidth))
                onClicked: root.selected(value)
            }
        }
    }
}
