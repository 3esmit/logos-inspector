pragma ComponentBehavior: Bound

import "../../../../components/common"
import "../../../../state"

DataTableRow {
    id: root

    property var columns: []
    property string hash: ""
    property string program: ""
    property AppModel modelRef

    cells: root.rowCells()
    onCellActivated: function (column, cell) {
        if (root.modelRef === null) {
            return
        }
        if (column === 1) {
            root.modelRef.entityNavigation.openReference("transaction", root.hash)
        } else if (column === 3) {
            root.modelRef.entityNavigation.openReference("program", root.program)
        }
    }

    function rowCells() {
        return [
            { text: String(root.columns[0] || "-"), width: 68 },
            { text: String(root.columns[1] || "-"), width: 180, fill: true, link: !root.header && root.hash.length > 0, copyText: root.hash },
            { text: String(root.columns[2] || "-"), width: 96 },
            { text: String(root.columns[3] || "-"), width: 180, fill: true, link: !root.header && root.program.length > 0, copyText: root.program }
        ]
    }
}
