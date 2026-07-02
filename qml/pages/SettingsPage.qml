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
                text: qsTr("Refresh status")
                primary: true
                enabled: !root.model.busy
                Layout.preferredWidth: 136
                onClicked: root.model.refreshDashboard()
            }
        }
    }

    Panel {
        theme: root.theme
        title: qsTr("Status")
        Layout.fillWidth: true

        StatusLine {
            theme: root.theme
            label: qsTr("Sequencer")
            value: root.model.sequencerUrl
            ok: root.serviceOk("sequencer", "health")
        }

        StatusLine {
            theme: root.theme
            label: qsTr("Indexer")
            value: root.model.indexerUrl
            ok: root.serviceOk("indexer", "health")
        }

        StatusLine {
            theme: root.theme
            label: qsTr("Blockchain node")
            value: root.model.nodeUrl
            ok: root.serviceOk("node", "consensus")
        }
    }

    function overview() {
        return root.model.dashboardOverview || {}
    }

    function serviceOk(section, field) {
        const target = root.overview()[section]
        const probe = target ? target[field] : null
        return !!(probe && probe.ok)
    }

    component StatusLine: Item {
        id: lineRoot

        required property Theme theme
        property string label: ""
        property string value: ""
        property bool ok: false

        Layout.fillWidth: true
        Layout.preferredHeight: 34

        RowLayout {
            anchors.fill: parent
            spacing: 10

            Rectangle {
                color: lineRoot.ok ? lineRoot.theme.success : lineRoot.theme.warning
                radius: 4
                Layout.preferredWidth: 8
                Layout.preferredHeight: 8
            }

            Text {
                text: lineRoot.label
                color: lineRoot.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: 11
                font.weight: Font.DemiBold
                font.capitalization: Font.AllUppercase
                Layout.preferredWidth: 130
            }

            Text {
                text: lineRoot.value
                color: lineRoot.theme.text
                textFormat: Text.PlainText
                font.family: "monospace"
                font.pixelSize: 12
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }
    }
}
