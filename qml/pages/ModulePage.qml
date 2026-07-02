pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../components"
import "../state"
import "../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model
    property string moduleKind: "blockchain"
    property string title: ""
    property string subtitle: ""

    width: parent ? parent.width : 900
    spacing: 16

    Panel {
        theme: root.theme
        title: root.title

        Text {
            text: root.subtitle
            color: root.theme.textMuted
            wrapMode: Text.Wrap
            textFormat: Text.PlainText
            font.pixelSize: 14
            Layout.fillWidth: true
        }

        Loader {
            active: true
            sourceComponent: root.controlsFor(root.moduleKind)
            Layout.fillWidth: true
        }
    }

    ResultPane {
        visible: root.model.pageHasOutput(root.moduleKind)
        theme: root.theme
        model: root.model
    }

    function controlsFor(kind) {
        switch (kind) {
        case "channels":
            return channelsControls
        case "storage":
            return storageControls
        case "messaging":
            return messagingControls
        case "capabilities":
            return capabilitiesControls
        default:
            return blockchainControls
        }
    }

    Component {
        id: blockchainControls

        ColumnLayout {
            spacing: 12

            RowLayout {
                spacing: 10
                Layout.fillWidth: true

                FieldRow {
                    id: slotFrom
                    theme: root.theme
                    label: qsTr("Slot from")
                    placeholderText: qsTr("49600")
                    Layout.fillWidth: true
                }

                FieldRow {
                    id: slotTo
                    theme: root.theme
                    label: qsTr("Slot to")
                    placeholderText: qsTr("49620")
                    Layout.fillWidth: true
                }
            }

            FieldRow {
                id: address
                theme: root.theme
                label: qsTr("Wallet address")
                placeholderText: qsTr("Optional module wallet address")
            }

            RowLayout {
                spacing: 10
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Refresh node")
                    primary: true
                    enabled: !root.model.busy
                    Layout.preferredWidth: 140
                    onClicked: root.model.callInspector("blockchainNode", [root.model.nodeUrl], qsTr("Blockchain node"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Load blocks")
                    enabled: !root.model.busy && slotFrom.text.trim().length > 0 && slotTo.text.trim().length > 0
                    Layout.preferredWidth: 126
                    onClicked: root.model.callInspector("blockchainBlocks", [root.model.nodeUrl, slotFrom.text, slotTo.text], qsTr("Blockchain blocks"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Module")
                    enabled: !root.model.busy
                    Layout.preferredWidth: 110
                    onClicked: root.model.callModule(root.model.blockchainModule, "moduleVersion", [], qsTr("Blockchain module"))
                }
            }
        }
    }

    Component {
        id: channelsControls

        ColumnLayout {
            spacing: 12

            RowLayout {
                spacing: 10
                Layout.fillWidth: true

                FieldRow {
                    id: channelFrom
                    theme: root.theme
                    label: qsTr("Slot from")
                    placeholderText: qsTr("49600")
                    Layout.fillWidth: true
                }

                FieldRow {
                    id: channelTo
                    theme: root.theme
                    label: qsTr("Slot to")
                    placeholderText: qsTr("49620")
                    Layout.fillWidth: true
                }
            }

            RowLayout {
                spacing: 10
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Scan")
                    primary: true
                    enabled: !root.model.busy && channelFrom.text.trim().length > 0 && channelTo.text.trim().length > 0
                    Layout.preferredWidth: 112
                    onClicked: root.model.callInspector("channelScan", [root.model.nodeUrl, channelFrom.text, channelTo.text], qsTr("Channel scan"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Blockchain methods")
                    enabled: !root.model.busy
                    Layout.preferredWidth: 180
                    onClicked: root.model.callModule(root.model.blockchainModule, "moduleVersion", [], qsTr("Blockchain channel methods"))
                }
            }
        }
    }

    Component {
        id: storageControls

        ColumnLayout {
            spacing: 12

            FieldRow {
                id: cid
                theme: root.theme
                label: qsTr("CID")
                placeholderText: qsTr("Optional CID for exists lookup")
            }

            RowLayout {
                spacing: 10
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Module version")
                    primary: true
                    enabled: !root.model.busy
                    Layout.preferredWidth: 152
                    onClicked: root.model.callModule(root.model.storageModule, "moduleVersion", [], qsTr("Storage module"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("CID exists")
                    enabled: !root.model.busy && cid.text.trim().length > 0
                    Layout.preferredWidth: 120
                    onClicked: root.model.callModule(root.model.storageModule, "exists", [cid.text], qsTr("Storage CID"))
                }
            }
        }
    }

    Component {
        id: messagingControls

        ColumnLayout {
            spacing: 12

            FieldRow {
                id: infoId
                theme: root.theme
                label: qsTr("Info id")
                placeholderText: qsTr("Optional getNodeInfo id")
            }

            RowLayout {
                spacing: 10
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Module version")
                    primary: true
                    enabled: !root.model.busy
                    Layout.preferredWidth: 152
                    onClicked: root.model.callModule(root.model.deliveryModule, "moduleVersion", [], qsTr("Messaging module"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Node info")
                    enabled: !root.model.busy
                    Layout.preferredWidth: 116
                    onClicked: root.model.callModule(root.model.deliveryModule, "getNodeInfo", infoId.text.trim().length ? [infoId.text] : [], qsTr("Messaging node info"))
                }
            }
        }
    }

    Component {
        id: capabilitiesControls

        RowLayout {
            spacing: 10
            Layout.fillWidth: true

            ActionButton {
                theme: root.theme
                text: qsTr("Refresh capabilities")
                primary: true
                enabled: !root.model.busy
                Layout.preferredWidth: 184
                onClicked: root.model.callModule(root.model.capabilityModule, "listCapabilities", [], qsTr("Capabilities"))
            }

            ActionButton {
                theme: root.theme
                text: qsTr("All modules")
                enabled: !root.model.busy
                Layout.preferredWidth: 124
                onClicked: root.model.callInspector("modules", [], qsTr("Modules"))
            }
        }
    }
}
