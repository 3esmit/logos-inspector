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
    readonly property int rowCount: Array.isArray(root.rows) ? root.rows.length : 0
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
            model: root.rowCount

            DataTableRow {
                required property int index
                readonly property var rowData: root.rowAt(index)

                theme: root.theme
                cells: rowData.cells || []
                selected: Boolean(rowData.selected)
                headerHeight: root.headerHeight
                rowHeight: root.rowHeight
                onCellActivated: function (column, cell) {
                    root.cellActivated(index, column, cell, rowData)
                }
            }
        }
    }

    function rowAt(index) {
        return Array.isArray(root.rows) && index >= 0 && index < root.rows.length ? root.rows[index] : ({})
    }
}
