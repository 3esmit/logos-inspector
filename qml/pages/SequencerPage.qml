pragma ComponentBehavior: Bound

import QtQuick
import QtQml.Models
import QtQuick.Layouts
import "../components"
import "../state"
import "../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model

    width: parent ? parent.width : 900
    spacing: 16

    ListModel {
        id: sequencerTabs

        ListElement { value: "blocks"; label: "Blocks" }
        ListElement { value: "transactions"; label: "Transactions" }
    }

    Panel {
        theme: root.theme
        title: qsTr("Sequencer")

        TabSwitch {
            theme: root.theme
            current: model.sequencerTab
            options: sequencerTabs
            onSelected: value => model.sequencerTab = value
        }

        Loader {
            active: true
            sourceComponent: model.sequencerTab === "blocks" ? blocksForm : transactionsForm
            Layout.fillWidth: true
        }
    }

    ResultPane {
        theme: root.theme
        model: root.model
    }

    Component {
        id: blocksForm

        ColumnLayout {
            spacing: 12

            FieldRow {
                id: blockId
                theme: root.theme
                label: qsTr("Block ID or header hash")
                placeholderText: qsTr("7067 or 64-byte hash")
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Inspect")
                primary: true
                enabled: !root.model.busy && blockId.text.trim().length > 0
                Layout.preferredWidth: 128
                onClicked: root.model.callInspector("block", [root.model.sequencerUrl, blockId.text], qsTr("Block detail"))
            }
        }
    }

    Component {
        id: transactionsForm

        ColumnLayout {
            spacing: 12

            FieldRow {
                id: txHash
                theme: root.theme
                label: qsTr("Hash")
                placeholderText: qsTr("Transaction hash")
            }

            TextAreaField {
                id: txIdl
                theme: root.theme
                label: qsTr("IDL JSON")
                placeholderText: qsTr("Optional IDL override")
                rows: 5
            }

            RowLayout {
                spacing: 10
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Summary")
                    primary: true
                    enabled: !root.model.busy && txHash.text.trim().length > 0
                    Layout.preferredWidth: 116
                    onClicked: root.model.callInspector("transaction", [root.model.sequencerUrl, txHash.text], qsTr("Transaction summary"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Inspect")
                    enabled: !root.model.busy && txHash.text.trim().length > 0
                    Layout.preferredWidth: 116
                    onClicked: root.model.callInspector("inspectTransaction", [root.model.sequencerUrl, txHash.text, txIdl.text], qsTr("Transaction inspection"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Trace")
                    enabled: !root.model.busy && txHash.text.trim().length > 0
                    Layout.preferredWidth: 104
                    onClicked: root.model.callInspector("traceTransaction", [root.model.sequencerUrl, txHash.text, txIdl.text], qsTr("Transaction trace"))
                }
            }
        }
    }
}
