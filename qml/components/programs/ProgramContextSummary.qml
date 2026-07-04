pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import ".."
import "../common"
import "../../state"
import "../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property var rows: []
    property var idls: []
    property var transactions: []
    property var account: null
    property AppModel modelRef

    spacing: 6
    Layout.fillWidth: true

    Text {
        text: qsTr("Program")
        color: root.theme.text
        textFormat: Text.PlainText
        font.pixelSize: root.theme.primaryText
        font.weight: Font.DemiBold
        Layout.fillWidth: true
    }

    SourceStrip {
        theme: root.theme
        sources: [qsTr("L2 LEZ"), qsTr("sequencer getProgramIds"), qsTr("sequencer getAccount"), qsTr("local IDL")]
        Layout.fillWidth: true
    }

    LinkedDetailSection {
        theme: root.theme
        rows: root.rows
        onLinkActivated: (kind, value) => {
            if (root.modelRef !== null) {
                root.modelRef.openReference(kind, value)
            }
        }
    }

    SummarySection {
        theme: root.theme
        title: qsTr("Registered IDLs")
        rows: root.idls
    }

    SummarySection {
        theme: root.theme
        title: qsTr("Loaded recent transactions")
        rows: root.transactions
    }

    Text {
        visible: root.account !== null
        text: qsTr("Program account state")
        color: root.theme.text
        textFormat: Text.PlainText
        font.pixelSize: root.theme.primaryText
        font.weight: Font.DemiBold
        Layout.fillWidth: true
    }

    AccountDetailPane {
        visible: root.account !== null
        value: root.account
        theme: root.theme
        model: root.modelRef
        Layout.fillWidth: true
    }
}
