pragma ComponentBehavior: Bound

import QtQuick
import ".."

Panel {
    id: root

    property var rows: []

    Repeater {
        model: root.rows

        StatusRow {
            required property var modelData

            theme: root.theme
            label: String(modelData.label || "")
            stateText: String(modelData.state || "")
            evidence: String(modelData.evidence || "")
            source: String(modelData.source || "")
            freshness: String(modelData.freshness || "")
            tone: String(modelData.tone || "neutral")
        }
    }
}
