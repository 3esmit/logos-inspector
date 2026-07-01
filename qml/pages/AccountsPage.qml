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
        id: accountTabs

        ListElement { value: "lookup"; label: "Lookup" }
        ListElement { value: "decode"; label: "Decode data" }
    }

    Panel {
        theme: root.theme
        title: qsTr("Accounts")

        TabSwitch {
            theme: root.theme
            current: model.accountTab
            options: accountTabs
            onSelected: value => model.accountTab = value
        }

        Loader {
            active: true
            sourceComponent: model.accountTab === "lookup" ? lookupForm : decodeForm
            Layout.fillWidth: true
        }
    }

    ResultPane {
        theme: root.theme
        model: root.model
    }

    Component {
        id: lookupForm

        ColumnLayout {
            spacing: 12

            FieldRow {
                id: accountId
                theme: root.theme
                label: qsTr("Account address")
                placeholderText: qsTr("Account address")
            }

            TextAreaField {
                id: accountIdl
                theme: root.theme
                label: qsTr("IDL JSON")
                placeholderText: qsTr("Optional account IDL")
                rows: 6
            }

            FieldRow {
                id: accountType
                theme: root.theme
                label: qsTr("DefinitionType")
                placeholderText: qsTr("Auto-detect")
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Lookup account")
                primary: true
                enabled: !root.model.busy && accountId.text.trim().length > 0
                Layout.preferredWidth: 152
                onClicked: root.model.callInspector("account", [root.model.sequencerUrl, root.model.indexerUrl, accountId.text, accountIdl.text, accountType.text], qsTr("Account lookup"))
            }
        }
    }

    Component {
        id: decodeForm

        ColumnLayout {
            spacing: 12

            TextAreaField {
                id: dataHex
                theme: root.theme
                label: qsTr("Data hex")
                placeholderText: qsTr("Account data hex")
                rows: 4
            }

            TextAreaField {
                id: decodeIdl
                theme: root.theme
                label: qsTr("IDL JSON")
                placeholderText: qsTr("Account IDL")
                rows: 7
            }

            FieldRow {
                id: decodeType
                theme: root.theme
                label: qsTr("DefinitionType")
                placeholderText: qsTr("Auto-detect")
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Decode data")
                primary: true
                enabled: !root.model.busy && dataHex.text.trim().length > 0 && decodeIdl.text.trim().length > 0
                Layout.preferredWidth: 132
                onClicked: root.model.callInspector("decodeAccount", [dataHex.text, decodeIdl.text, decodeType.text], qsTr("Account decode"))
            }
        }
    }
}
