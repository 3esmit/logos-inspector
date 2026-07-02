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

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Accounts")
        title: qsTr("Accounts")
        subtitle: qsTr("Account lookup and IDL-backed data decode. Linked decoded values open their referenced inspector view.")
        Layout.fillWidth: true
    }

    GridLayout {
        columns: root.width < 760 ? 1 : 3
        columnSpacing: 12
        rowSpacing: 12
        Layout.fillWidth: true

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Sequencer")
            value: root.endpointLabel(root.model.sequencerUrl)
            delta: root.shortEndpoint(root.model.sequencerUrl)
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Indexer")
            value: root.endpointLabel(root.model.indexerUrl)
            delta: root.shortEndpoint(root.model.indexerUrl)
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Local IDLs")
            value: root.model.registeredIdls.count.toLocaleString(Qt.locale(), "f", 0)
            delta: root.model.registeredIdls.count === 1 ? qsTr("Registered schema") : qsTr("Registered schemas")
            deltaColor: root.model.registeredIdls.count > 0 ? root.theme.success : root.theme.textMuted
        }
    }

    Panel {
        theme: root.theme
        title: root.model.accountTab === "lookup" ? qsTr("Lookup account") : qsTr("Decode account data")

        TabSwitch {
            theme: root.theme
            current: root.model.accountTab
            options: accountTabs
            onSelected: value => root.model.accountTab = value
        }

        Loader {
            active: true
            sourceComponent: root.model.accountTab === "lookup" ? lookupForm : decodeForm
            Layout.fillWidth: true
        }
    }

    AccountDetailPane {
        value: root.model.accountDetailValue
        theme: root.theme
        model: root.model
    }

    StatusMessage {
        visible: root.model.accountDetailValue === null && root.model.pageHasOutput("accounts")
        theme: root.theme
        tone: root.model.resultIsError ? "warning" : "info"
        title: root.model.resultTitle
        message: root.model.resultText
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.model.accountDetailValue === null && !root.model.pageHasOutput("accounts")
        theme: root.theme
        tone: "info"
        title: qsTr("Account detail")
        message: qsTr("Lookup an account to inspect raw state, related transactions, decoded fields, and linked account, program, channel, or transaction references.")
        Layout.fillWidth: true
    }

    Component {
        id: lookupForm

        ColumnLayout {
            spacing: 12

            FieldRow {
                id: accountId
                theme: root.theme
                label: qsTr("Account address")
                placeholderText: qsTr("Base58 account address")
            }

            TextAreaField {
                id: accountIdl
                theme: root.theme
                label: qsTr("Account IDL JSON")
                placeholderText: qsTr("Optional JSON schema for decoded rows")
                rows: 4
            }

            GridLayout {
                columns: root.width < 760 ? 1 : 2
                columnSpacing: 12
                rowSpacing: 12
                Layout.fillWidth: true

                FieldRow {
                    id: accountType
                    theme: root.theme
                    label: qsTr("Definition type")
                    placeholderText: qsTr("Auto-detect from IDL")
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Lookup account")
                    primary: true
                    enabled: !root.model.busy && accountId.text.trim().length > 0
                    Layout.preferredWidth: 152
                    Layout.alignment: Qt.AlignLeft | Qt.AlignBottom
                    accessibleName: qsTr("Lookup account")
                    onClicked: root.model.callInspector("account", [root.model.sequencerUrl, root.model.indexerUrl, accountId.text, accountIdl.text, accountType.text], qsTr("Account lookup"))
                }
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
                placeholderText: qsTr("Hex bytes from an account value")
                rows: 3
            }

            TextAreaField {
                id: decodeIdl
                theme: root.theme
                label: qsTr("Account IDL JSON")
                placeholderText: qsTr("JSON schema used for decode")
                rows: 5
            }

            GridLayout {
                columns: root.width < 760 ? 1 : 2
                columnSpacing: 12
                rowSpacing: 12
                Layout.fillWidth: true

                FieldRow {
                    id: decodeType
                    theme: root.theme
                    label: qsTr("Definition type")
                    placeholderText: qsTr("Auto-detect from IDL")
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Decode data")
                    primary: true
                    enabled: !root.model.busy && dataHex.text.trim().length > 0 && decodeIdl.text.trim().length > 0
                    Layout.preferredWidth: 132
                    Layout.alignment: Qt.AlignLeft | Qt.AlignBottom
                    accessibleName: qsTr("Decode account data")
                    onClicked: root.model.callInspector("decodeAccount", [dataHex.text, decodeIdl.text, decodeType.text], qsTr("Account decode"))
                }
            }
        }
    }

    function endpointLabel(value) {
        const text = String(value || "")
        if (!text.length) {
            return "-"
        }
        if (text.indexOf("127.0.0.1") >= 0 || text.indexOf("localhost") >= 0) {
            return qsTr("Local")
        }
        if (text.indexOf("testnet") >= 0) {
            return qsTr("Testnet")
        }
        return qsTr("Custom")
    }

    function shortEndpoint(value) {
        const text = String(value || "")
        if (!text.length) {
            return qsTr("Not configured")
        }
        return text.replace(/^https?:\/\//, "").replace(/\/$/, "")
    }
}
