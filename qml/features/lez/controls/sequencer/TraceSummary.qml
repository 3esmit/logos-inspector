pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../../components"
import "../../../../state"
import "../../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property var steps: []
    property var capabilities: []
    property var limitations: []
    property AppModel modelRef
    readonly property real layoutWidth: root.width > 0 ? root.width : (parent ? parent.width : 900)

    spacing: root.theme.gap
    Layout.fillWidth: true

    GridLayout {
        visible: root.capabilities.length > 0 || root.limitations.length > 0
        columns: root.layoutWidth < 760 ? 1 : 2
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gap
        Layout.fillWidth: true

        TraceNote {
            visible: root.capabilities.length > 0
            theme: root.theme
            title: qsTr("Capabilities")
            rows: root.capabilities
            tone: "success"
        }

        TraceNote {
            visible: root.limitations.length > 0
            theme: root.theme
            title: qsTr("Limitations")
            rows: root.limitations
            tone: "warning"
        }
    }

    Text {
        text: qsTr("Trace steps")
        color: root.theme.text
        textFormat: Text.PlainText
        font.pixelSize: root.theme.primaryText
        font.weight: Font.DemiBold
        Layout.fillWidth: true
    }

    Repeater {
        model: root.steps

        TraceStepCard {
            required property var modelData

            theme: root.theme
            step: modelData
            modelRef: root.modelRef
        }
    }

    component TraceNote: Frame {
        id: noteRoot

        required property Theme theme
        property string title: ""
        property var rows: []
        property string tone: "info"

        padding: noteRoot.theme.gap
        Layout.fillWidth: true

        background: Rectangle {
            color: noteRoot.tone === "warning" ? noteRoot.theme.warningMuted : noteRoot.theme.successMuted
            radius: noteRoot.theme.radius
            border.width: 1
            border.color: noteRoot.tone === "warning" ? noteRoot.theme.warning : noteRoot.theme.success
        }

        contentItem: ColumnLayout {
            spacing: noteRoot.theme.gapTiny

            Text {
                text: noteRoot.title
                color: noteRoot.theme.text
                textFormat: Text.PlainText
                font.pixelSize: noteRoot.theme.secondaryText
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            Repeater {
                model: noteRoot.rows

                Text {
                    required property string modelData

                    text: modelData
                    color: noteRoot.theme.textMuted
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: noteRoot.theme.dataText
                    Layout.fillWidth: true
                }
            }
        }
    }

    component TraceStepCard: Frame {
        id: stepRoot

        required property Theme theme
        property var step: null
        property AppModel modelRef

        padding: stepRoot.theme.gap
        Layout.fillWidth: true

        background: Rectangle {
            color: stepRoot.theme.surface
            radius: stepRoot.theme.radius
            border.width: 1
            border.color: stepRoot.severityColor(stepRoot.step ? stepRoot.step.severity : "")
        }

        contentItem: ColumnLayout {
            spacing: stepRoot.theme.gapSmall

            RowLayout {
                spacing: stepRoot.theme.gapSmall
                Layout.fillWidth: true

                Text {
                    text: stepRoot.step ? qsTr("%1. %2").arg(stepRoot.step.index).arg(stepRoot.step.label || "-") : "-"
                    color: stepRoot.theme.text
                    textFormat: Text.PlainText
                    font.pixelSize: stepRoot.theme.secondaryText
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }

                Text {
                    text: stepRoot.step ? stepRoot.valueText(stepRoot.step.status || stepRoot.step.phase) : "-"
                    color: stepRoot.theme.textMuted
                    textFormat: Text.PlainText
                    font.pixelSize: stepRoot.theme.dataText
                    font.family: "monospace"
                    horizontalAlignment: Text.AlignRight
                    Layout.preferredWidth: 120
                }
            }

            Repeater {
                model: stepRoot.detailRows()

                Text {
                    required property var modelData

                    text: String(modelData || "")
                    color: stepRoot.theme.textMuted
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: stepRoot.theme.dataText
                    Layout.fillWidth: true
                }
            }

            ColumnLayout {
                visible: stepRoot.referenceRows().length > 0
                spacing: stepRoot.theme.gapTiny
                Layout.fillWidth: true

                Text {
                    text: qsTr("References")
                    color: stepRoot.theme.text
                    textFormat: Text.PlainText
                    font.pixelSize: stepRoot.theme.secondaryText
                    font.weight: Font.DemiBold
                    Layout.fillWidth: true
                }

                Repeater {
                    model: stepRoot.referenceRows()

                    RowLayout {
                        id: referenceRow

                        required property var modelData

                        spacing: stepRoot.theme.gap
                        Layout.fillWidth: true

                        Text {
                            text: String(referenceRow.modelData.label || "")
                            color: stepRoot.theme.textDim
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.pixelSize: stepRoot.theme.labelText
                            font.capitalization: Font.AllUppercase
                            Layout.preferredWidth: 128
                        }

                        LinkCell {
                            theme: stepRoot.theme
                            text: String(referenceRow.modelData.value || "-")
                            link: stepRoot.modelRef !== null
                                && String(referenceRow.modelData.linkKind || "").length > 0
                            copyable: String(referenceRow.modelData.value || "").length > 0
                            copyText: String(referenceRow.modelData.value || "")
                            monospace: referenceRow.modelData.monospace === true
                            wrap: true
                            Layout.fillWidth: true
                            onActivated: {
                                if (stepRoot.modelRef !== null) {
                                    stepRoot.modelRef.entityNavigation.openReference(
                                        String(referenceRow.modelData.linkKind || ""),
                                        referenceRow.modelData.linkValue
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }

        function referenceRows() {
            const refs = stepRoot.step ? stepRoot.step.refs : null
            if (!refs || typeof refs !== "object") {
                return []
            }
            const rows = []
            if (refs.program_id_hex) {
                rows.push({ label: qsTr("Program"), value: refs.program_id_hex, monospace: true, linkKind: "program", linkValue: refs.program_id_hex })
            }
            if (refs.program_id_base58) {
                rows.push({ label: qsTr("Program base58"), value: refs.program_id_base58, monospace: true, linkKind: "program", linkValue: refs.program_id_base58 })
            }
            if (refs.account_id) {
                rows.push({ label: qsTr("Account"), value: refs.account_id, monospace: true, linkKind: "account", linkValue: refs.account_id })
            }
            if (refs.instruction_word_index !== undefined && refs.instruction_word_index !== null) {
                rows.push({ label: qsTr("Instruction word"), value: stepRoot.valueText(refs.instruction_word_index), monospace: true })
            }
            if (refs.decode_path) {
                rows.push({ label: qsTr("Decode path"), value: refs.decode_path, monospace: true })
            }
            return rows
        }

        function detailRows() {
            const details = stepRoot.step ? stepRoot.step.details : null
            if (!details || details.length === undefined) {
                return []
            }
            const rows = []
            for (let i = 0; i < details.length; ++i) {
                rows.push(String(details[i] || ""))
            }
            return rows
        }

        function severityColor(value) {
            const severity = String(value || "")
            if (severity === "error") {
                return stepRoot.theme.error
            }
            if (severity === "warning") {
                return stepRoot.theme.warning
            }
            if (severity === "ok" || severity === "success") {
                return stepRoot.theme.success
            }
            return stepRoot.theme.outlineMuted
        }

        function valueText(value) {
            if (value === undefined || value === null || value === "") {
                return "-"
            }
            return String(value)
        }
    }
}
