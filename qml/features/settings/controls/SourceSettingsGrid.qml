pragma ComponentBehavior: Bound

import QtQuick.Layouts
import "../../../theme"

GridLayout {
    id: root

    required property Theme theme
    property real pageWidth: 900

    columns: root.pageWidth < 760 ? 1 : 2
    columnSpacing: root.theme.gap
    rowSpacing: root.theme.gap
    Layout.fillWidth: true
}
