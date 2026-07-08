pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../state"
import "../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model
    property bool l2: false

    width: parent ? parent.width : 900
    spacing: 16

    PageHeader {
        theme: root.theme
        layerLabel: root.l2 ? qsTr("L2 LEZ") : qsTr("L1 BEDROCK")
        title: root.l2 ? qsTr("LEZ Block") : qsTr("Bedrock Block")
        subtitle: root.l2 ? qsTr("Block detail from the execution zone indexer or sequencer.") : qsTr("Block detail from the Bedrock node.")
        Layout.fillWidth: true

        ActionButton {
            theme: root.theme
            text: qsTr("Blocks")
            onClicked: root.model.selectView(root.l2 ? "l2Blocks" : "blocks")
        }
    }

    StatusMessage {
        visible: root.model.blockDetailValue === null && root.model.blockDetailError.length > 0
        theme: root.theme
        tone: "warning"
        title: root.l2 ? qsTr("LEZ block lookup failed") : qsTr("Block lookup failed")
        message: root.model.blockDetailError
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.model.blockDetailValue === null && root.model.blockDetailError.length === 0
        theme: root.theme
        tone: "info"
        title: root.l2 ? qsTr("LEZ block detail") : qsTr("Block detail")
        message: root.l2 ? qsTr("Select a recent L2 block or search by block id or hash.") : qsTr("Select a recent L1 block or search by slot or hash.")
        Layout.fillWidth: true
    }

    BlockDetailPane {
        value: root.model.blockDetailValue
        theme: root.theme
        model: root.model
    }
}
