pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import ".."

Panel {
    id: root

    property real pageWidth: 900
    property bool busy: false
    property string kind: ""
    property string subtitle: ""
    property string connectionType: ""
    property string endpointLabel: qsTr("URL")
    property string endpoint: ""
    property string moduleName: ""
    property bool primaryFieldVisible: true
    property bool moduleFieldVisible: false
    property bool auxiliaryFieldVisible: false
    property string auxiliaryLabel: ""
    property string auxiliaryText: ""
    property string auxiliaryPlaceholder: ""
    property int refreshRate: 30
    property string statusText: qsTr("Unknown")
    property string statusDetail: ""
    property color statusColor: theme.textMuted

    signal endpointEdited(string value)
    signal auxiliaryEdited(string value)
    signal refreshRateEdited(int value)
    signal queryClicked()

    RowLayout {
        spacing: root.theme.gap
        Layout.fillWidth: true

        Text {
            text: root.subtitle
            color: root.theme.textMuted
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: root.theme.secondaryText
            Layout.fillWidth: true
        }

        StatusPill {
            theme: root.theme
            text: root.statusText
            colorToken: root.statusColor
        }
    }

    GridLayout {
        columns: root.pageWidth < 760 ? 1 : 2
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gap
        Layout.fillWidth: true

        InfoField {
            theme: root.theme
            label: qsTr("Connection")
            value: root.connectionType
        }

        RefreshRateField {
            theme: root.theme
            value: root.refreshRate
            onRateEdited: value => root.refreshRateEdited(value)
        }

        FieldRow {
            visible: root.primaryFieldVisible
            theme: root.theme
            label: root.endpointLabel
            sourceText: root.endpoint
            syncSourceText: true
            placeholderText: qsTr("Endpoint URL")
            onTextEdited: text => root.endpointEdited(text)
        }

        InfoField {
            visible: root.moduleFieldVisible
            theme: root.theme
            label: qsTr("Module bridge")
            value: root.moduleName
        }

        FieldRow {
            visible: root.auxiliaryFieldVisible
            theme: root.theme
            label: root.auxiliaryLabel
            sourceText: root.auxiliaryText
            syncSourceText: true
            placeholderText: root.auxiliaryPlaceholder
            onTextEdited: text => root.auxiliaryEdited(text)
        }
    }

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        ActionButton {
            theme: root.theme
            text: qsTr("Query status")
            primary: true
            enabled: !root.busy
            Layout.preferredWidth: 132
            accessibleName: qsTr("Query %1 status").arg(root.title)
            onClicked: root.queryClicked()
        }

        Text {
            text: root.statusDetail
            color: root.theme.textMuted
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: root.theme.dataText
            Layout.fillWidth: true
        }
    }
}
