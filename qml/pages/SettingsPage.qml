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
            color: root.theme.textMuted
            wrapMode: Text.Wrap
            textFormat: Text.PlainText
            font.pixelSize: 14
            Layout.fillWidth: true
        }

        ComboBox {
            id: profile
            model: [qsTr("Default"), qsTr("Testnet with local indexer"), qsTr("Local Logos node"), qsTr("Local")]
            currentIndex: root.model.profileIndex()
            Layout.fillWidth: true
            Layout.preferredHeight: root.theme.controlHeight
            onActivated: index => root.model.applyProfile(index)
        }

        FieldRow {
            theme: root.theme
            label: qsTr("Sequencer URL")
            text: root.model.sequencerUrl
            onTextChanged: root.model.sequencerUrl = text
        }

        FieldRow {
            theme: root.theme
            label: qsTr("Indexer URL")
            text: root.model.indexerUrl
            onTextChanged: root.model.indexerUrl = text
        }

        FieldRow {
            theme: root.theme
            label: qsTr("Blockchain node URL")
            text: root.model.nodeUrl
            onTextChanged: root.model.nodeUrl = text
        }

        RowLayout {
            spacing: 10
            Layout.fillWidth: true

            ActionButton {
                theme: root.theme
                text: qsTr("Reconnect")
                primary: true
                enabled: !root.model.busy
                Layout.preferredWidth: 128
                onClicked: root.model.callInspector("overview", [root.model.sequencerUrl, root.model.indexerUrl, root.model.nodeUrl], qsTr("Reconnect"))
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Clear result")
                Layout.preferredWidth: 118
                onClicked: root.model.clearResult()
            }
        }
    }

    ResultPane {
        theme: root.theme
        model: root.model
    }
}
