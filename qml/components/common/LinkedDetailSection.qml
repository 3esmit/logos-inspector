pragma ComponentBehavior: Bound

import QtQuick.Layouts
import "../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property string title: ""
    property var rows: []
    property int labelWidth: 132

    signal linkActivated(string kind, string value)

    visible: root.rows.length > 0
    spacing: 0
    Layout.fillWidth: true

    DetailSection {
        theme: root.theme
        title: root.title
        rows: root.normalizedRows()
        labelWidth: root.labelWidth
        surfaceColor: root.theme.field
        onLinkActivated: function (kind, value) {
            root.linkActivated(kind, value)
        }
    }

    function normalizedRows() {
        const result = []
        for (let i = 0; i < root.rows.length; ++i) {
            const row = root.rows[i] || {}
            const value = String(row.value || "-")
            result.push({
                label: String(row.label || ""),
                value: value,
                linkKind: String(row.linkKind || ""),
                linkValue: row.linkValue !== undefined ? row.linkValue : value,
                copyText: value,
                copyable: row.copyable !== undefined ? row.copyable : String(row.linkKind || "").length > 0,
                monospace: row.monospace !== undefined ? row.monospace : true
            })
        }
        return result
    }
}
