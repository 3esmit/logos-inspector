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
        id: programTabs

        ListElement { value: "idls"; label: "IDLs" }
        ListElement { value: "binaries"; label: "Binaries" }
        ListElement { value: "events"; label: "Events" }
    }

    Panel {
        theme: root.theme
        title: qsTr("SPEL")

        TabSwitch {
            theme: root.theme
            current: root.model.programTab
            options: programTabs
            onSelected: value => root.model.programTab = value
        }

        Loader {
            active: true
            sourceComponent: root.formFor(root.model.programTab)
            Layout.fillWidth: true
        }
    }

    ResultPane {
        visible: root.model.pageHasOutput("programs")
        theme: root.theme
        model: root.model
    }

    function formFor(tab) {
        switch (tab) {
        case "binaries":
            return binaryForm
        case "events":
            return eventForm
        default:
            return idlForm
        }
    }

    Component {
        id: idlForm

        ColumnLayout {
            spacing: 12

            RowLayout {
                spacing: 10
                Layout.fillWidth: true

                FieldRow {
                    id: programId
                    theme: root.theme
                    label: qsTr("Program ID or label")
                    placeholderText: qsTr("Optional")
                    Layout.fillWidth: true
                }

                FieldRow {
                    id: idlName
                    theme: root.theme
                    label: qsTr("IDL name")
                    placeholderText: qsTr("Auto from JSON")
                    Layout.fillWidth: true
                }
            }

            TextAreaField {
                id: idlJson
                theme: root.theme
                label: qsTr("IDL JSON")
                rows: 8
            }

            RowLayout {
                spacing: 10
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Save IDL")
                    primary: true
                    enabled: idlJson.text.trim().length > 0
                    Layout.preferredWidth: 116
                    onClicked: root.model.registerIdl(idlName.text, programId.text, idlJson.text)
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Summarize")
                    enabled: !root.model.busy && idlJson.text.trim().length > 0
                    Layout.preferredWidth: 120
                    onClicked: root.model.callInspector("spelIdl", [idlJson.text], qsTr("SPEL IDL"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Load programs")
                    enabled: !root.model.busy
                    Layout.preferredWidth: 140
                    onClicked: root.model.callInspector("programs", [root.model.sequencerUrl], qsTr("Program IDs"))
                }
            }

            Text {
                text: root.model.registeredIdls.count
                    ? qsTr("Registered IDLs: %1").arg(root.model.registeredIdls.count)
                    : qsTr("No IDLs registered")
                color: root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: 13
                Layout.fillWidth: true
            }
        }
    }

    Component {
        id: binaryForm

        ColumnLayout {
            spacing: 12

            FieldRow {
                id: programPath
                theme: root.theme
                label: qsTr("Path")
                placeholderText: qsTr("program.bin")
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Inspect")
                primary: true
                enabled: !root.model.busy && programPath.text.trim().length > 0
                Layout.preferredWidth: 116
                onClicked: root.model.callInspector("programFile", [programPath.text], qsTr("Program file"))
            }
        }
    }

    Component {
        id: eventForm

        ColumnLayout {
            spacing: 12

            FieldRow {
                id: eventName
                theme: root.theme
                label: qsTr("Event")
                placeholderText: qsTr("Optional event name")
            }

            TextAreaField {
                id: eventData
                theme: root.theme
                label: qsTr("Event data hex")
                rows: 4
            }

            TextAreaField {
                id: eventIdl
                theme: root.theme
                label: qsTr("IDL JSON")
                rows: 7
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Decode event")
                primary: true
                enabled: !root.model.busy && eventData.text.trim().length > 0 && eventIdl.text.trim().length > 0
                Layout.preferredWidth: 132
                onClicked: root.model.callInspector("decodeEvent", [eventData.text, eventIdl.text, eventName.text], qsTr("Event decode"))
            }
        }
    }
}
