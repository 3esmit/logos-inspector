pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "common"
import "../theme"

ColumnLayout {
    id: root

    required property Theme theme

    property bool toolbarVisible: true
    property int loadCount: 20
    property var loadOptions: [10, 20, 50]
    property string rangeText: ""
    property bool canGoNewer: false
    property bool canGoOlder: false
    property bool busy: false
    property string refreshText: qsTr("Latest")
    property string newerText: qsTr("Newer")
    property string olderText: qsTr("Older")
    property var headerCells: []
    property var rows: []

    default property alias preTableContent: preTableColumn.data

    signal refreshRequested()
    signal newerRequested()
    signal olderRequested()
    signal loadCountSelected(int count)
    signal cellActivated(int row, int column, var cell, var rowData)

    spacing: root.theme.gap
    Layout.fillWidth: true

    ListToolbar {
        visible: root.toolbarVisible
        theme: root.theme
        loadCount: root.loadCount
        loadOptions: root.loadOptions
        rangeText: root.rangeText
        canGoNewer: root.canGoNewer
        canGoOlder: root.canGoOlder
        busy: root.busy
        refreshText: root.refreshText
        newerText: root.newerText
        olderText: root.olderText
        Layout.fillWidth: true
        onRefresh: root.refreshRequested()
        onNewer: root.newerRequested()
        onOlder: root.olderRequested()
        onLoadCountSelected: function (count) {
            root.loadCountSelected(count)
        }
    }

    ColumnLayout {
        id: preTableColumn
        spacing: root.theme.gapSmall
        Layout.fillWidth: true
    }

    DataTableFrame {
        theme: root.theme
        Layout.fillWidth: true
        headerCells: root.headerCells
        rows: root.rows
        onCellActivated: function (row, column, cell, rowData) {
            root.cellActivated(row, column, cell, rowData)
        }
    }
}
