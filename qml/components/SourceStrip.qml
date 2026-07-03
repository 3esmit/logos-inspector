pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../theme"

RowLayout {
    id: root

    required property Theme theme
    property var sources: []

    spacing: root.theme.gapSmall

    Repeater {
        model: root.sources

        LayerBadge {
            required property string modelData

            theme: root.theme
            text: modelData
        }
    }
}
