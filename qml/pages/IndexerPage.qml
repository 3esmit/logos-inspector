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
        id: indexerTabs

        ListElement { value: "status"; label: "Dashboard" }
        ListElement { value: "rpc"; label: "RPC" }
    }

    Panel {
        theme: root.theme
        title: qsTr("Indexer")

        TabSwitch {
            theme: root.theme
            current: model.indexerTab
            options: indexerTabs
            onSelected: value => model.indexerTab = value
        }

        Loader {
            active: true
            sourceComponent: model.indexerTab === "status" ? statusForm : rpcForm
            Layout.fillWidth: true
        }
    }

    ResultPane {
        theme: root.theme
        model: root.model
    }

    Component {
        id: statusForm

        RowLayout {
            spacing: 10
            Layout.fillWidth: true

            ActionButton {
                theme: root.theme
                text: qsTr("Deep health")
                primary: true
                enabled: !root.model.busy
                Layout.preferredWidth: 132
                onClicked: root.model.callInspector("indexerHealth", [root.model.indexerUrl], qsTr("Indexer health"))
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Finalized head")
                enabled: !root.model.busy
                Layout.preferredWidth: 148
                onClicked: root.model.callInspector("indexerFinalizedHead", [root.model.indexerUrl], qsTr("Indexer head"))
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Overview")
                enabled: !root.model.busy
                Layout.preferredWidth: 112
                onClicked: root.model.callInspector("overview", [root.model.sequencerUrl, root.model.indexerUrl, root.model.nodeUrl], qsTr("Indexer dashboard"))
            }
        }
    }

    Component {
        id: rpcForm

        ColumnLayout {
            spacing: 12

            FieldRow {
                id: method
                theme: root.theme
                label: qsTr("Method")
                text: "getLastFinalizedBlockId"
            }

            TextAreaField {
                id: params
                theme: root.theme
                label: qsTr("Params JSON")
                text: "[]"
                rows: 4
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Call indexer")
                primary: true
                enabled: !root.model.busy && method.text.trim().length > 0 && params.text.trim().length > 0
                Layout.preferredWidth: 132
                onClicked: root.model.callInspector("rawRpc", [root.model.indexerUrl, method.text, params.text], qsTr("Indexer RPC"))
            }
        }
    }
}
