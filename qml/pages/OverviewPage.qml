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
        title: qsTr("Dashboard")

        Text {
            text: qsTr("Inspect Logos Blockchain, Logos Execution Zone, and local module state from one QML surface.")
            color: theme.textMuted
            wrapMode: Text.Wrap
            textFormat: Text.PlainText
            font.pixelSize: 15
            Layout.fillWidth: true
        }

        RowLayout {
            spacing: 10
            Layout.fillWidth: true

            FieldRow {
                id: searchField
                theme: root.theme
                label: qsTr("Search")
                placeholderText: qsTr("Block id, transaction hash, or account address")
                Layout.fillWidth: true
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Open")
                primary: true
                enabled: searchField.text.trim().length > 0 && !model.busy
                Layout.preferredWidth: 112
                onClicked: model.routeSearch(searchField.text)
            }
        }

        RowLayout {
            spacing: 10
            Layout.fillWidth: true

            ActionButton {
                theme: root.theme
                text: qsTr("Refresh overview")
                primary: true
                enabled: !model.busy
                Layout.preferredWidth: 160
                onClicked: model.callInspector("overview", [model.sequencerUrl, model.indexerUrl, model.nodeUrl], qsTr("Overview"))
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Sequencer head")
                enabled: !model.busy
                Layout.preferredWidth: 150
                onClicked: model.callInspector("head", [model.sequencerUrl], qsTr("Sequencer head"))
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Program IDs")
                enabled: !model.busy
                Layout.preferredWidth: 132
                onClicked: model.callInspector("programs", [model.sequencerUrl], qsTr("Program IDs"))
            }
        }
    }

    Panel {
        theme: root.theme
        title: qsTr("Connection")

        Text {
            text: qsTr("Profile: %1").arg(model.networkProfile)
            color: theme.text
            textFormat: Text.PlainText
            font.pixelSize: 14
            Layout.fillWidth: true
        }

        Text {
            text: qsTr("Sequencer: %1").arg(model.sequencerUrl)
            color: theme.textMuted
            elide: Text.ElideRight
            textFormat: Text.PlainText
            font.pixelSize: 13
            Layout.fillWidth: true
        }

        Text {
            text: qsTr("Indexer: %1").arg(model.indexerUrl)
            color: theme.textMuted
            elide: Text.ElideRight
            textFormat: Text.PlainText
            font.pixelSize: 13
            Layout.fillWidth: true
        }
    }

    ResultPane {
        theme: root.theme
        model: root.model
    }
}
