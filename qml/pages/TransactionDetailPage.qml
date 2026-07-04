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
    property bool l2: false

    width: parent ? parent.width : 900
    spacing: 16

    PageHeader {
        theme: root.theme
        layerLabel: root.l2 ? qsTr("L2 LEZ") : qsTr("L1 BEDROCK")
        title: root.l2 ? qsTr("LEZ Transaction") : qsTr("Mantle Transaction")
        subtitle: root.l2 ? qsTr("Transaction inspection from the execution zone.") : qsTr("Transaction detail from the Bedrock node.")
        Layout.fillWidth: true

        ActionButton {
            theme: root.theme
            text: root.l2 ? qsTr("Transactions") : qsTr("Mantle Tx")
            onClicked: root.model.selectView(root.l2 ? "l2Transactions" : "transactions")
        }
    }

    StatusMessage {
        visible: root.model.transactionDetailValue === null && root.model.transactionDetailError.length > 0
        theme: root.theme
        tone: "warning"
        title: root.l2 ? qsTr("LEZ transaction lookup failed") : qsTr("Transaction lookup failed")
        message: root.model.transactionDetailError
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.model.transactionDetailValue === null && root.model.transactionDetailError.length === 0
        theme: root.theme
        tone: "info"
        title: root.l2 ? qsTr("LEZ transaction detail") : qsTr("Transaction detail")
        message: root.l2 ? qsTr("Select a recent L2 transaction or search by transaction hash.") : qsTr("Select a recent Mantle transaction or search by transaction hash.")
        Layout.fillWidth: true
    }

    TransactionDetailPane {
        value: root.model.transactionDetailValue
        theme: root.theme
        model: root.model
    }
}
