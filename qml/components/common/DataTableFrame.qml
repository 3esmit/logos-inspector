pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../theme"

Frame {
    id: root

    required property Theme theme
    property var headerCells: []
    property var rows: []
    property int headerHeight: 36
    property int rowHeight: 42
    property color surfaceColor: theme.surface
    signal cellActivated(int row, int column, var cell, var rowData)

    padding: 0
    Layout.fillWidth: true

    background: Rectangle {
        color: root.surfaceColor
        radius: root.theme.radius
        border.width: 1
        border.color: root.theme.outlineMuted
    }

    contentItem: ColumnLayout {
        spacing: 0

        DataTableRow {
            theme: root.theme
            header: true
            cells: root.headerCells
            headerHeight: root.headerHeight
            rowHeight: root.rowHeight
        }

        Repeater {
            model: root.rows

            DataTableRow {
                required property int index
                required property var modelData

                theme: root.theme
                cells: modelData.cells || []
                selected: Boolean(modelData.selected)
                headerHeight: root.headerHeight
                rowHeight: root.rowHeight
                onCellActivated: function (column, cell) {
                    root.cellActivated(index, column, cell, modelData)
                }
            }
        }
    }
}
