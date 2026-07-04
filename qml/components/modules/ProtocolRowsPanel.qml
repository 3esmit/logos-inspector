pragma ComponentBehavior: Bound

import QtQuick
import ".."

Panel {
    id: root

    property var rows: []
    property int labelWidth: 132

    Repeater {
        model: root.rows

        ProtocolRow {
            required property var modelData

            theme: root.theme
            label: String(modelData.label || "")
            protocolId: String(modelData.protocolId || "")
            stateText: String(modelData.state || "")
            evidence: String(modelData.evidence || "")
            tone: String(modelData.tone || "neutral")
            labelWidth: root.labelWidth
        }
    }
}
