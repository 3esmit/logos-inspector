pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../../components"
import "../../../../components/common"
import "../../../../state"
import "../../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property var rows: []
    property var idls: []
    property var transactions: []
    property var account: null
    property string rawText: ""
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
        sources: root.account !== null
            ? [qsTr("L2 LEZ"), qsTr("sequencer getProgramIds"), qsTr("sequencer getAccount"), qsTr("local IDL")]
            : [qsTr("L2 LEZ"), qsTr("sequencer getProgramIds"), qsTr("local IDL")]
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

    TextArea {
        visible: root.rawText.length > 0
        readOnly: true
        text: root.rawText
        wrapMode: TextArea.Wrap
        color: root.theme.text
        selectedTextColor: root.theme.selectedText
        selectionColor: root.theme.accent
        textFormat: Text.PlainText
        font.family: "monospace"
        font.pixelSize: root.theme.secondaryText
        leftPadding: 12
        rightPadding: 12
        topPadding: 10
        bottomPadding: 10
        Layout.fillWidth: true
        Layout.preferredHeight: 180

        background: Rectangle {
            color: root.theme.field
            radius: root.theme.radius
            border.width: 1
            border.color: root.theme.outline
        }
    }
}
