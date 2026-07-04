pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../theme"

Flow {
    id: root

    required property Theme theme
    property var sources: []

    spacing: root.theme.gapSmall
    Layout.fillWidth: true

    Repeater {
        model: root.sources

        LayerBadge {
            required property string modelData

            theme: root.theme
            text: modelData
        }
    }
}
