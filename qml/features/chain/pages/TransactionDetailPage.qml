pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../state"
import "../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model
    width: parent ? parent.width : 900
    spacing: 16

    PageHeader {
        theme: root.theme
        layerLabel: qsTr("L1 BEDROCK")
        title: qsTr("Mantle Transaction")
        subtitle: qsTr("Transaction detail from the Bedrock node.")
        Layout.fillWidth: true

        ActionButton {
            theme: root.theme
            text: qsTr("Mantle Tx")
            onClicked: root.model.selectView("transactions")
        }
    }

    StatusMessage {
        visible: root.model.transactionDetailValue === null && root.model.transactionDetailError.length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Transaction lookup failed")
        message: root.model.transactionDetailError
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.model.transactionDetailValue === null && root.model.transactionDetailError.length === 0
        theme: root.theme
        tone: "info"
        title: qsTr("Transaction detail")
        message: qsTr("Select a recent Mantle transaction or search by transaction hash.")
        Layout.fillWidth: true
    }

    TransactionDetailPane {
        value: root.model.transactionDetailValue
        theme: root.theme
        model: root.model
    }
}
