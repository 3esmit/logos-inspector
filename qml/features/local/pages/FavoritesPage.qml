pragma ComponentBehavior: Bound

import QtQuick
import QtQml.Models
import QtQuick.Layouts
import "../../../components"
import "../../../components/common"
import "../../../state"
import "../../../theme"
import "../../../utils/UiFormat.js" as UiFormat

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model

    width: parent ? parent.width : 900
    spacing: root.theme.gapLarge

    ListModel {
        id: favoriteTabs

        ListElement { value: "all"; label: "All" }
        ListElement { value: "account"; label: "Accounts" }
        ListElement { value: "transaction"; label: "Transactions" }
        ListElement { value: "block"; label: "Blocks" }
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Local / Favorites")
        title: qsTr("Favorites")
        layerLabel: qsTr("Local")
        subtitle: qsTr("Saved accounts, transactions, and blocks for quick return during inspection.")
        Layout.fillWidth: true
    }

    GridLayout {
        columns: root.width < 760 ? 2 : 4
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gap
        Layout.fillWidth: true

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("All")
            value: root.countText("all")
            delta: qsTr("Saved items")
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Accounts")
            value: root.countText("account")
            delta: qsTr("L2 account refs")
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Transactions")
            value: root.countText("transaction")
            delta: qsTr("L1 and L2")
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Blocks")
            value: root.countText("block")
            delta: qsTr("L1 and L2")
        }
    }

    TabSwitch {
        theme: root.theme
        current: root.model.favoriteStore.filter
        options: favoriteTabs
        Layout.fillWidth: true
        onSelected: value => root.model.favoriteStore.filter = value
    }

    StatusMessage {
        visible: root.visibleRows().length === 0
        theme: root.theme
        tone: "info"
        title: qsTr("No favorites saved")
        message: qsTr("Open an account, transaction, or block detail and use Favorite to keep it here.")
        Layout.fillWidth: true
    }

    DataTableFrame {
        visible: root.visibleRows().length > 0
        theme: root.theme
        Layout.fillWidth: true
        headerCells: [
            { text: qsTr("Type"), width: 116 },
            { text: qsTr("Name"), width: 220, fill: true },
            { text: qsTr("Reference"), width: 220, fill: true },
            { text: qsTr("Layer"), width: 72 },
            { text: qsTr("Saved"), width: 132 },
            { text: qsTr("Remove"), width: 86 }
        ]
        rows: root.tableRows()
        onCellActivated: function (row, column, cell, rowData) {
            if (!rowData.favorite) {
                return
            }
            if (column === 5) {
                root.model.favoriteStore.remove(rowData.key)
                return
            }
            root.model.favoriteStore.open(rowData.favorite)
        }
    }

    function visibleRows() {
        return root.model.favoriteStore.rows(root.model.favoriteStore.filter)
    }

    function tableRows() {
        const rows = root.visibleRows()
        return rows.map(function (entry) {
            const key = root.model.favoriteStore.favoriteKey(entry)
            return {
                cells: [
                    { text: root.model.favoriteStore.kindLabel(entry.kind), width: 116, monospace: false },
                    { text: entry.title, width: 220, fill: true, link: true, copyText: entry.value, monospace: false },
                    { text: root.referenceText(entry), width: 220, fill: true, link: true, copyText: entry.value },
                    { text: root.model.favoriteStore.layerLabel(entry.layer), width: 72, monospace: false },
                    { text: root.savedText(entry.created_at), width: 132, monospace: false },
                    { text: qsTr("Remove"), width: 86, link: true, monospace: false, copyable: false, tone: "warning" }
                ],
                favorite: entry,
                key: key
            }
        })
    }

    function referenceText(entry) {
        const text = String(entry.value || "")
        return text.length > 18 ? UiFormat.shortHash(text) : text
    }

    function savedText(value) {
        const text = String(value || "")
        if (text.length >= 10) {
            return text.slice(0, 10)
        }
        return qsTr("-")
    }

    function countText(filter) {
        return UiFormat.numberText(root.model.favoriteStore.count(filter))
    }
}
