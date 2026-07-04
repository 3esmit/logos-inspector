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

    contentItem: Flickable {
        id: tableScroller

        implicitHeight: tableColumn.implicitHeight
        boundsBehavior: Flickable.StopAtBounds
        clip: true
        contentWidth: Math.max(width, root.tableMinimumWidth())
        contentHeight: tableColumn.implicitHeight
        flickableDirection: Flickable.HorizontalFlick
        interactive: contentWidth > width

        ColumnLayout {
            id: tableColumn

            width: tableScroller.contentWidth
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

        ScrollBar.horizontal: ScrollBar {
            policy: tableScroller.contentWidth > tableScroller.width ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff
        }
    }

    function rowAt(index) {
        return Array.isArray(root.rows) && index >= 0 && index < root.rows.length ? root.rows[index] : ({})
    }

    function tableMinimumWidth() {
        let cells = Array.isArray(root.headerCells) ? root.headerCells : []
        if (!cells.length && root.rowCount > 0) {
            const row = root.rowAt(0)
            cells = Array.isArray(row.cells) ? row.cells : []
        }
        if (!cells.length) {
            return root.width
        }
        let total = 28
        for (let i = 0; i < cells.length; ++i) {
            total += cells[i] && cells[i].width !== undefined ? Number(cells[i].width) : 120
        }
        total += Math.max(0, cells.length - 1) * 10
        return total
    }
}
