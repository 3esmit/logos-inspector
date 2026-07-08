pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../state"

Panel {
    id: root

    property string description: ""
    property var groups: []
    property string mode: "footer"
    property AppModel modelRef

    Text {
        text: root.description
        color: root.theme.textMuted
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: root.theme.secondaryText
        Layout.fillWidth: true
    }

    Repeater {
        model: root.groups

        ColumnLayout {
            id: fieldGroupRoot

            required property var modelData

            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Text {
                text: String(fieldGroupRoot.modelData.title || "")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            Flow {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                Repeater {
                    model: fieldGroupRoot.modelData.fields || []

                    FieldToggle {
                        required property var modelData

                        theme: root.theme
                        fieldKey: String(modelData.key || "")
                        label: String(modelData.label || "")
                        detail: String(modelData.detail || "")
                        checked: root.mode === "dashboard"
                            ? root.modelRef.dashboardGraphEnabled(fieldKey)
                            : root.modelRef.footerFieldEnabled(fieldKey)
                        onToggled: {
                            if (root.mode === "dashboard") {
                                root.modelRef.setDashboardGraphEnabled(fieldKey, checked)
                            } else {
                                root.modelRef.setFooterFieldEnabled(fieldKey, checked)
                            }
                        }
                    }
                }
            }
        }
    }
}
