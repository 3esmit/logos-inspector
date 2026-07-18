pragma ComponentBehavior: Bound

import QtQuick
import QtQml.Models
import "../../../components"

SourceSettingsPanel {
    id: root

    property real pageWidth: 900
    property string kind: ""
    property string connectionType: ""
    property string endpointLabel: qsTr("URL")
    property string endpoint: ""
    property string moduleName: ""
    property bool primaryFieldVisible: true
    property bool moduleFieldVisible: false
    property bool sourceSelectorVisible: false
    property var sourceOptions
    property int sourceIndex: 0
    property bool auxiliaryFieldVisible: false
    property string auxiliaryLabel: ""
    property string auxiliaryText: ""
    property string auxiliaryPlaceholder: ""
    property int refreshRate: 30

    signal endpointEdited(string value)
    signal auxiliaryEdited(string value)
    signal refreshRateEdited(int value)
    signal sourceActivated(int index)

    ListModel {
        id: emptySourceOptions
    }

    SourceSettingsGrid {
        theme: root.theme
        pageWidth: root.pageWidth

        InfoField {
            theme: root.theme
            label: qsTr("Connection")
            value: root.connectionType
        }

        ComboField {
            visible: root.sourceSelectorVisible
            theme: root.theme
            label: qsTr("Connector")
            accessibleName: qsTr("%1 connector").arg(root.title)
            options: root.sourceOptions || emptySourceOptions
            currentIndex: root.sourceIndex
            onActivated: index => root.sourceActivated(index)
        }

        RefreshRateField {
            theme: root.theme
            accessibleName: root.title.length
                ? qsTr("%1 auto refresh").arg(root.title)
                : qsTr("Auto refresh")
            accessibleDescription: root.title.length
                ? qsTr("Automatic %1 status refresh interval in seconds. Set to 0 to turn it off.").arg(root.title)
                : qsTr("Automatic status refresh interval in seconds. Set to 0 to turn it off.")
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
}
