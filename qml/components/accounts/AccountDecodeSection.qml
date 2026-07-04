pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQml.Models
import QtQuick.Layouts
import ".."
import "../../state"
import "../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property AppModel modelRef
    property var detail: null
    property string dataView: "decoded"
    property var idlTypeLabels: []
    property int selectedIdlTypeIndex: -1
    property var activeDecode: null
    property string activeDecodeError: ""
    property string activeIdlTypeLabelText: "-"
    property string decodeStatusMessage: ""
    property var decodedRows: []

    signal dataViewSelected(string value)
    signal idlTypeSelected(int index)
    signal typedIdlTypeSelected(string text)
    signal rowActivated(string linkKind, var linkValue)

    visible: root.detail !== null && !root.detail.private_reference
    spacing: 8
    Layout.fillWidth: true

    ListModel {
        id: dataTabs

        ListElement { value: "decoded"; label: "Decoded" }
        ListElement { value: "raw"; label: "Raw" }
    }

    RowLayout {
        spacing: root.theme.gap
        Layout.fillWidth: true

        Text {
            text: qsTr("Data [%1]").arg(root.detail ? root.numberText(root.dataBytes(root.detail.data_hex)) : "-")
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: 14
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        TabSwitch {
            visible: root.detail && root.dataBytes(root.detail.data_hex) > 0
            theme: root.theme
            current: root.dataView
            options: dataTabs
            Layout.preferredWidth: 206
            onSelected: value => root.dataViewSelected(value)
        }
    }

    Frame {
        visible: root.detail && root.dataBytes(root.detail.data_hex) > 0
        padding: 0
        Layout.fillWidth: true

        background: Rectangle {
            color: root.theme.surface
            radius: root.theme.radius
            border.width: 1
            border.color: root.theme.outlineMuted
        }

        contentItem: ColumnLayout {
            spacing: 0

            ColumnLayout {
                visible: root.dataView === "decoded"
                spacing: root.theme.gap
                Layout.fillWidth: true

                RowLayout {
                    visible: root.idlTypeLabels.length > 0
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true
                    Layout.leftMargin: 12
                    Layout.rightMargin: 12
                    Layout.topMargin: 12

                    Text {
                        text: qsTr("IDL Type")
                        color: root.theme.textMuted
                        textFormat: Text.PlainText
                        font.pixelSize: 11
                        font.weight: Font.DemiBold
                        font.capitalization: Font.AllUppercase
                        Layout.preferredWidth: 92
                        Layout.alignment: Qt.AlignVCenter
                    }

                    ComboBox {
                        id: idlTypeCombo

                        editable: true
                        model: root.idlTypeLabels
                        currentIndex: root.selectedIdlTypeIndex
                        font.pixelSize: root.theme.secondaryText
                        Layout.fillWidth: true
                        Layout.preferredHeight: root.theme.controlHeight
                        onActivated: index => root.idlTypeSelected(index)
                        onAccepted: root.typedIdlTypeSelected(editText)

                        contentItem: TextField {
                            text: idlTypeCombo.editText
                            color: root.theme.text
                            placeholderText: qsTr("Search IDL type")
                            placeholderTextColor: root.theme.textDim
                            selectionColor: root.theme.accent
                            selectedTextColor: root.theme.selectedText
                            font: idlTypeCombo.font
                            leftPadding: 10
                            rightPadding: 10
                            readOnly: !idlTypeCombo.editable
                            background: null
                        }

                        background: Rectangle {
                            radius: root.theme.radius
                            color: idlTypeCombo.hovered || idlTypeCombo.activeFocus ? root.theme.surfaceRaised : root.theme.field
                            border.width: idlTypeCombo.activeFocus ? 2 : 1
                            border.color: idlTypeCombo.activeFocus ? root.theme.accent : root.theme.outlineMuted
                        }

                        Accessible.role: Accessible.ComboBox
                        Accessible.name: qsTr("IDL type")
                    }
                }

                Text {
                    visible: root.activeDecode !== null
                    text: qsTr("IDL Type: %1").arg(root.activeIdlTypeLabelText)
                    color: root.theme.textMuted
                    textFormat: Text.PlainText
                    wrapMode: Text.WrapAnywhere
                    font.pixelSize: root.theme.dataText
                    Layout.fillWidth: true
                    Layout.leftMargin: 12
                    Layout.rightMargin: 12
                }

                StatusMessage {
                    visible: root.activeDecode === null
                    theme: root.theme
                    tone: root.activeDecodeError.length > 0 ? "warning" : "info"
                    title: root.activeDecodeError.length > 0 ? qsTr("Decode unavailable") : qsTr("No decoded data")
                    message: root.decodeStatusMessage
                    Layout.fillWidth: true
                    Layout.leftMargin: 12
                    Layout.rightMargin: 12
                    Layout.bottomMargin: 12
                }

                Repeater {
                    model: root.decodedRows

                    AccountDetailRow {
                        required property var modelData

                        theme: root.theme
                        label: String(modelData.label || "")
                        value: String(modelData.value || "-")
                        subvalue: String(modelData.subvalue || "")
                        subvalueCopyText: String(modelData.subvalueCopyText || "")
                        linkKind: String(modelData.linkKind || "")
                        linkValue: root.modelRef ? root.modelRef.valueToString(modelData.linkValue) : String(modelData.linkValue || "")
                        tooltipText: String(modelData.tooltipText || "")
                        monospace: modelData.monospace !== undefined ? modelData.monospace : true
                        onActivated: root.rowActivated(modelData.linkKind, modelData.linkValue)
                    }
                }
            }

            TextArea {
                visible: root.dataView === "raw"
                readOnly: true
                text: root.detail ? root.detail.data_hex : ""
                wrapMode: TextArea.Wrap
                color: root.detail && root.detail.data_hex.length ? root.theme.text : root.theme.textMuted
                selectedTextColor: root.theme.selectedText
                selectionColor: root.theme.accent
                textFormat: Text.PlainText
                font.family: "monospace"
                font.pixelSize: root.theme.dataText
                leftPadding: 12
                rightPadding: 12
                topPadding: 10
                bottomPadding: 10
                Layout.fillWidth: true
                Layout.preferredHeight: 150

                background: Rectangle {
                    color: root.theme.field
                    radius: root.theme.radius
                    border.width: 0
                }
            }
        }
    }

    function dataBytes(hex) {
        const text = String(hex || "").replace(/^0x/, "")
        return Math.floor(text.length / 2)
    }

    function numberText(value) {
        if (value === undefined || value === null || value === "") {
            return "-"
        }
        const numeric = Number(value)
        if (Number.isFinite(numeric)) {
            return numeric % 1 === 0 ? numeric.toLocaleString(Qt.locale(), "f", 0) : String(value)
        }
        return String(value)
    }
}
