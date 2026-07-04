pragma ComponentBehavior: Bound

import "../common"

DataTableRow {
    id: root

    property var columns: []
    property string txHash: ""
    property string programId: ""

    cells: root.rowCells()

    function rowCells() {
        return [
            { text: String(root.columns[0] || "-"), width: 180, fill: true, link: !root.header && root.txHash.length > 0, copyText: root.txHash },
            { text: String(root.columns[1] || "-"), width: 92 },
            { text: String(root.columns[2] || "-"), width: 160, fill: true },
            { text: String(root.columns[3] || "-"), width: 180, fill: true, link: !root.header && root.programId.length > 0, copyText: root.programId },
            { text: String(root.columns[4] || "-"), width: 92 }
        ]
    }
}
