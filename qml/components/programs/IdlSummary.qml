pragma ComponentBehavior: Bound

import QtQuick.Layouts
import "../common"
import "../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property var instructions: []
    property var accounts: []
    property var warnings: []

    spacing: root.theme.gap
    Layout.fillWidth: true

    SummarySection {
        theme: root.theme
        title: qsTr("Instructions")
        rows: root.instructions
    }

    SummarySection {
        theme: root.theme
        title: qsTr("Account schemas")
        rows: root.accounts
    }

    SummarySection {
        visible: root.warnings.length > 0
        theme: root.theme
        title: qsTr("Warnings")
        rows: root.warnings
    }
}
