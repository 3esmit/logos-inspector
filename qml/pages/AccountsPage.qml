pragma ComponentBehavior: Bound

import QtQuick
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

    PageHeader {
        visible: root.model.accountDetailValue === null
        theme: root.theme
        breadcrumb: qsTr("Home / L2 LEZ / Accounts")
        title: qsTr("L2 Accounts")
        layerLabel: qsTr("L2 LEZ")
        subtitle: qsTr("Public account state from sequencer latest, finalized related transactions from the indexer, and local IDL decoding.")
        Layout.fillWidth: true
    }

    AccountDetailPane {
        value: root.model.accountDetailValue
        theme: root.theme
        model: root.model
        Layout.fillWidth: true
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
        message: qsTr("Use the header search field to open an account. Loaded accounts show balance, nonce, owner, decoded data, raw data, and linked transactions.")
        Layout.fillWidth: true
    }
}
