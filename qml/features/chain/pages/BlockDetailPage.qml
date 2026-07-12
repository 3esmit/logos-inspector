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
    width: parent ? parent.width : 900
    spacing: 16

    PageHeader {
        theme: root.theme
        layerLabel: qsTr("L1 BEDROCK")
        title: qsTr("Bedrock Block")
        subtitle: qsTr("Block detail from the Bedrock node.")
        Layout.fillWidth: true

        ActionButton {
            theme: root.theme
            text: qsTr("Blocks")
            onClicked: root.model.selectView("blocks")
        }
    }

    StatusMessage {
        visible: root.model.blockDetailValue === null && root.model.blockDetailError.length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Block lookup failed")
        message: root.model.blockDetailError
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.model.blockDetailValue === null && root.model.blockDetailError.length === 0
        theme: root.theme
        tone: "info"
        title: qsTr("Block detail")
        message: qsTr("Select a recent L1 block or search by slot or hash.")
        Layout.fillWidth: true
    }

    BlockDetailPane {
        value: root.model.blockDetailValue
        theme: root.theme
        model: root.model
    }
}
