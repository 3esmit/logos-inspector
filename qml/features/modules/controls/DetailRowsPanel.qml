pragma ComponentBehavior: Bound

import QtQuick
import "../../../components"

Panel {
    id: root

    property var rows: []

    Repeater {
        model: root.rows

        DetailRow {
            required property var modelData

            theme: root.theme
            label: String(modelData.label || "")
            value: String(modelData.value || "")
            copyText: String(modelData.copyText || "")
            source: String(modelData.source || "")
        }
    }
}
