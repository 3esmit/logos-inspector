pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
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

    Panel {
        theme: root.theme
        title: qsTr("Network")

        Text {
            text: qsTr("Endpoint edits stay in the QML state and are passed to inspection actions.")
            color: theme.textMuted
            wrapMode: Text.Wrap
            textFormat: Text.PlainText
            font.pixelSize: 14
            Layout.fillWidth: true
        }

        ComboBox {
            id: profile
            model: [qsTr("Default"), qsTr("Testnet with local indexer"), qsTr("Local")]
            currentIndex: model.networkProfile === "local" ? 2 : (model.networkProfile === "testnet-indexer-local" ? 1 : 0)
            Layout.fillWidth: true
            Layout.preferredHeight: theme.controlHeight
            onActivated: index => root.model.applyProfile(index)
        }

        FieldRow {
            theme: root.theme
            label: qsTr("Sequencer URL")
            text: model.sequencerUrl
            onTextChanged: model.sequencerUrl = text
        }

        FieldRow {
            theme: root.theme
            label: qsTr("Indexer URL")
            text: model.indexerUrl
            onTextChanged: model.indexerUrl = text
        }

        FieldRow {
            theme: root.theme
            label: qsTr("Blockchain node URL")
            text: model.nodeUrl
            onTextChanged: model.nodeUrl = text
        }

        RowLayout {
            spacing: 10
            Layout.fillWidth: true

            ActionButton {
                theme: root.theme
                text: qsTr("Reconnect")
                primary: true
                enabled: !model.busy
                Layout.preferredWidth: 128
                onClicked: model.callInspector("overview", [model.sequencerUrl, model.indexerUrl, model.nodeUrl], qsTr("Reconnect"))
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Clear result")
                Layout.preferredWidth: 118
                onClicked: model.clearResult()
            }
        }
    }

    ResultPane {
        theme: root.theme
        model: root.model
    }
}
