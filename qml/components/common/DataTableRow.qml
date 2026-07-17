pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import ".."
import "../../theme"
import "../../utils/UiTone.js" as UiTone

Item {
    id: root

    required property Theme theme
    property var cells: []
    property bool header: false
    property bool selected: false
    property bool showSeparator: true
    property int headerHeight: 36
    property int rowHeight: 42
    property int horizontalPadding: 14
    property int columnSpacing: 10
    property color selectedColor: theme.accentMuted
    readonly property int columnCount: Array.isArray(root.cells) ? root.cells.length : 0
    signal cellActivated(int column, var cell)

    Layout.fillWidth: true
    Layout.preferredHeight: root.header ? root.headerHeight : root.rowHeight

    Rectangle {
        anchors.fill: parent
        color: root.header ? root.theme.field : (root.selected ? root.selectedColor : "transparent")
        border.width: 0
    }

    Rectangle {
        visible: root.showSeparator
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 1
        color: root.theme.outlineMuted
        Accessible.ignored: true
    }

    GridLayout {
        anchors.fill: parent
        anchors.leftMargin: root.horizontalPadding
        anchors.rightMargin: root.horizontalPadding
        columns: Math.max(1, root.columnCount)
        columnSpacing: root.columnSpacing

        Repeater {
            model: root.columnCount

            LinkCell {
                required property int index
                readonly property var cell: root.cellAt(index)

                theme: root.theme
                text: root.cellText(cell)
                header: root.header
                link: root.cellLink(cell)
                copyable: root.cellCopyable(cell)
                copyText: root.cellCopyText(cell)
                monospace: root.cellMonospace(cell)
                textColor: root.cellTextColor(cell)
                accessibleName: root.cellAccessibleName(cell)
                accessibleDescription: root.cellAccessibleDescription(cell)
                copyAccessibleName: root.cellCopyAccessibleName(cell)
                copyAccessibleDescription: root.cellCopyAccessibleDescription(cell)
                Layout.preferredWidth: root.cellWidth(cell)
                Layout.fillWidth: root.cellFill(cell)
                onActivated: root.cellActivated(index, cell)
            }
        }
    }

    function cellAt(index) {
        return Array.isArray(root.cells) && index >= 0 && index < root.cells.length ? root.cells[index] : ({})
    }

    function cellText(cell) {
        return String(cell && cell.text !== undefined ? cell.text : "-")
    }

    function cellLink(cell) {
        return !root.header && Boolean(cell && (cell.link === true || String(cell.linkKind || "").length > 0 || String(cell.linkValue || "").length > 0))
    }

    function cellCopyable(cell) {
        if (root.header) {
            return false
        }
        if (cell && cell.copyable !== undefined) {
            return Boolean(cell.copyable)
        }
        return root.cellLink(cell)
    }

    function cellCopyText(cell) {
        if (!cell) {
            return ""
        }
        if (cell.copyText !== undefined) {
            return String(cell.copyText || "")
        }
        if (cell.linkValue !== undefined) {
            return String(cell.linkValue || "")
        }
        return root.cellText(cell)
    }

    function cellMonospace(cell) {
        return cell && cell.monospace !== undefined ? Boolean(cell.monospace) : !root.header
    }

    function cellTextColor(cell) {
        if (root.header) {
            return root.theme.textMuted
        }
        if (cell && cell.color !== undefined) {
            return cell.color
        }
        if (cell && String(cell.tone || "").length > 0) {
            return UiTone.toneColor(root.theme, String(cell.tone))
        }
        if (root.cellLink(cell)) {
            return root.theme.accent
        }
        return root.theme.text
    }

    function cellAccessibleName(cell) {
        const configured = cell && cell.accessibleName !== undefined
            ? String(cell.accessibleName || "") : ""
        return configured.length > 0 ? configured : root.cellText(cell)
    }

    function cellAccessibleDescription(cell) {
        return cell && cell.accessibleDescription !== undefined
            ? String(cell.accessibleDescription || "") : ""
    }

    function cellCopyAccessibleName(cell) {
        const configured = cell && cell.copyAccessibleName !== undefined
            ? String(cell.copyAccessibleName || "") : ""
        return configured.length > 0
            ? configured : qsTr("Copy %1").arg(root.cellText(cell))
    }

    function cellCopyAccessibleDescription(cell) {
        return cell && cell.copyAccessibleDescription !== undefined
            ? String(cell.copyAccessibleDescription || "") : ""
    }

    function cellWidth(cell) {
        return cell && cell.width !== undefined ? Number(cell.width) : 120
    }

    function cellFill(cell) {
        return cell && cell.fill !== undefined ? Boolean(cell.fill) : false
    }
}
