pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQml.Models
import QtQuick.Layouts
import "../components"
import "../components/common"
import "../state"
import "../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property StorageAppState model

    width: parent ? parent.width : 900
    spacing: root.theme.gapLarge

    ListModel {
        id: storageTabs

        ListElement { value: "files"; label: "My Files" }
        ListElement { value: "cid"; label: "CID" }
        ListElement { value: "transfer"; label: "Transfer" }
        ListElement { value: "operations"; label: "Operations" }
    }

    Component.onCompleted: {
        root.model.refreshManifests(false)
    }

    Timer {
        id: storageOperationPoll

        interval: 500
        repeat: true
        running: root.model.activeStorageOperationRunning()
        onTriggered: root.model.pollStorageOperation(false)
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Network / Storage")
        title: qsTr("Storage")
        layerLabel: qsTr("Network")
        subtitle: qsTr("Inspect manifests, CID presence, and transfers through the configured Storage source.")
        Layout.fillWidth: true
    }

    SourceStrip {
        theme: root.theme
        sources: root.model.sourceBadges()
        Layout.fillWidth: true
    }

    GridLayout {
        columns: root.width < 760 ? 2 : 4
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gap
        Layout.fillWidth: true

        StatusChip {
            theme: root.theme
            label: qsTr("Source")
            value: root.model.sourceLabel
            tone: root.model.storageDataSource() ? "success" : "warning"
            Layout.fillWidth: true
        }

        StatusChip {
            theme: root.theme
            label: qsTr("Files")
            value: String(root.model.manifests.length)
            tone: root.model.manifests.length > 0 ? "success" : "neutral"
            Layout.fillWidth: true
        }

        StatusChip {
            theme: root.theme
            label: qsTr("Network")
            value: root.model.networkPreset
            tone: "neutral"
            Layout.fillWidth: true
        }

        StatusChip {
            theme: root.theme
            label: qsTr("Space")
            value: root.model.storageSpaceSummary()
            tone: root.model.storageSpaceTone()
            Layout.fillWidth: true
        }

        StatusChip {
            theme: root.theme
            label: qsTr("Last")
            value: root.model.lastOperation
            tone: root.model.resultIsError && root.model.resultOwner === root.model.currentView ? "error" : "neutral"
            Layout.fillWidth: true
        }

        StatusChip {
            theme: root.theme
            label: qsTr("Active")
            value: root.model.activeStorageStatusText()
            tone: root.model.activeStorageTone()
            Layout.fillWidth: true
        }
    }

    TabSwitch {
        theme: root.theme
        current: root.model.currentTab
        options: storageTabs
        Layout.fillWidth: true
        onSelected: value => root.model.currentTab = value
    }

    Loader {
        active: true
        sourceComponent: root.tabComponent(root.model.currentTab)
        Layout.fillWidth: true
    }

    Panel {
        visible: root.model.resultVisible()
        theme: root.theme
        title: root.model.resultIsError ? qsTr("Operation error") : qsTr("Operation result")

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Text {
                text: root.model.resultTitle
                color: root.model.resultIsError ? root.theme.error : root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
                font.weight: Font.Medium
                elide: Text.ElideRight
                Layout.fillWidth: true
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Clear")
                Layout.preferredWidth: 84
                onClicked: root.model.clearResult()
            }
        }

        TextArea {
            readOnly: true
            text: root.model.resultText.length ? root.model.resultText : qsTr("No response body.")
            wrapMode: TextArea.Wrap
            color: root.model.resultIsError ? root.theme.warning : root.theme.text
            selectedTextColor: root.theme.selectedText
            selectionColor: root.theme.accent
            textFormat: Text.PlainText
            font.family: "monospace"
            font.pixelSize: root.theme.secondaryText
            leftPadding: 12
            rightPadding: 12
            topPadding: 10
            bottomPadding: 10
            Layout.fillWidth: true
            Layout.preferredHeight: 220

            background: Rectangle {
                color: root.model.resultIsError ? root.theme.errorMuted : root.theme.field
                radius: root.theme.radius
                border.width: 1
                border.color: root.model.resultIsError ? root.theme.error : root.theme.outline
            }
        }
    }

    Component {
        id: filesTab

        Panel {
            theme: root.theme
            title: qsTr("My Files")

            RowLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("List")
                    primary: true
                    enabled: !root.model.busy
                    Layout.preferredWidth: 96
                    onClicked: root.model.refreshManifests(true)
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Settings")
                    enabled: !root.model.busy
                    Layout.preferredWidth: 104
                    onClicked: root.model.openStorageSettings()
                }

                Text {
                    text: root.model.sourceTarget
                    color: root.theme.textMuted
                    textFormat: Text.PlainText
                    elide: Text.ElideMiddle
                    font.pixelSize: root.theme.secondaryText
                    Layout.fillWidth: true
                }
            }

            ColumnLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                Repeater {
                    model: root.model.manifestRows()

                    delegate: ManifestRow {
                        required property var modelData

                        theme: root.theme
                        row: modelData
                        onUseCid: cid => {
                            root.model.activeCid = cid
                            root.model.currentTab = "cid"
                        }
                    }
                }
            }
        }
    }

    Component {
        id: cidTab

        Panel {
            theme: root.theme
            title: qsTr("CID")

            GridLayout {
                columns: root.width < 760 ? 1 : 2
                columnSpacing: root.theme.gap
                rowSpacing: root.theme.gap
                Layout.fillWidth: true

                FieldRow {
                    id: cidField

                    theme: root.theme
                    label: qsTr("CID")
                    placeholderText: qsTr("zDv...")
                    sourceText: root.model.activeCid
                    syncSourceText: true
                    Layout.fillWidth: true
                    onTextEdited: text => {
                        root.model.activeCid = text
                        root.model.setCidProbe(text)
                    }
                }

                FieldRow {
                    id: cidDestination

                    theme: root.theme
                    label: qsTr("Save path")
                    placeholderText: qsTr("/tmp/file.bin")
                    Layout.fillWidth: true
                }
            }

            RowLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Check")
                    enabled: !root.model.busy && cidField.text.trim().length > 0 && root.model.storageDataSource()
                    Layout.preferredWidth: 104
                    onClicked: root.model.runStorage("storageExists", [cidField.text.trim()], qsTr("Storage exists"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Fetch")
                    enabled: !root.model.busy && cidField.text.trim().length > 0 && root.model.storageDataSource()
                    Layout.preferredWidth: 104
                    onClicked: root.model.runStorage("storageDownloadManifest", [cidField.text.trim()], qsTr("Fetch manifest"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Cache")
                    enabled: !root.model.busy && !root.model.storageOperationBusy() && cidField.text.trim().length > 0 && root.model.storageMutatingSource()
                    Layout.preferredWidth: 104
                    onClicked: {
                        root.model.confirmStorage("storageFetch", [cidField.text.trim()], qsTr("Cache CID"))
                        storageConfirm.open()
                    }
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Download")
                    primary: true
                    enabled: !root.model.busy && !root.model.storageOperationBusy() && cidField.text.trim().length > 0 && cidDestination.text.trim().length > 0 && root.model.storageMutatingSource()
                    Layout.preferredWidth: 124
                    onClicked: {
                        root.model.confirmStorage("storageDownloadToUrl", [cidField.text.trim(), cidDestination.text.trim(), localOnly.checked], qsTr("Download CID"))
                        storageConfirm.open()
                    }
                }

                Item {
                    Layout.fillWidth: true
                }
            }

            RowLayout {
                spacing: root.theme.gap
                Layout.fillWidth: true

                CheckBox {
                    id: localOnly

                    text: qsTr("Local only")
                    checked: false
                    enabled: root.model.storageMutatingSource()
                    palette.text: root.theme.text
                    Layout.preferredWidth: 132
                }

                Item { Layout.fillWidth: true }
            }
        }
    }

    Component {
        id: transferTab

        Panel {
            theme: root.theme
            title: qsTr("Transfer")

            StatusMessage {
                visible: !root.model.storageDataSource()
                theme: root.theme
                tone: "warning"
                title: qsTr("Storage source required")
                message: qsTr("Upload, download, fetch, and remove use the configured Storage source.")
                Layout.fillWidth: true
            }

            StatusMessage {
                visible: root.model.storageDataSource() && !root.model.mutatingDiagnosticsEnabled
                theme: root.theme
                tone: "warning"
                title: qsTr("Mutating diagnostics off")
                message: qsTr("Enable mutating diagnostics in Settings before upload, download, fetch, or remove.")
                Layout.fillWidth: true
            }

            GridLayout {
                columns: root.width < 760 ? 1 : 2
                columnSpacing: root.theme.gap
                rowSpacing: root.theme.gap
                Layout.fillWidth: true

                FieldRow {
                    id: uploadPath

                    theme: root.theme
                    label: qsTr("File path")
                    placeholderText: qsTr("/home/user/file.bin")
                    Layout.fillWidth: true
                }

                FieldRow {
                    id: downloadCid

                    theme: root.theme
                    label: qsTr("CID")
                    placeholderText: qsTr("zDv...")
                    sourceText: root.model.activeCid
                    syncSourceText: true
                    Layout.fillWidth: true
                    onTextEdited: text => {
                        root.model.activeCid = text
                        root.model.setCidProbe(text)
                    }
                }

                FieldRow {
                    id: downloadPath

                    theme: root.theme
                    label: qsTr("Download path")
                    placeholderText: qsTr("/tmp/file.bin")
                    Layout.fillWidth: true
                }

                FieldRow {
                    id: transferChunkSize

                    theme: root.theme
                    label: qsTr("Upload block size")
                    text: "65536"
                    Layout.fillWidth: true
                }
            }

            RowLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Upload")
                    primary: true
                    enabled: !root.model.busy && !root.model.storageOperationBusy() && root.model.storageMutatingSource() && uploadPath.text.trim().length > 0
                    Layout.preferredWidth: 112
                    onClicked: {
                        root.model.confirmStorage("storageUploadUrl", [uploadPath.text.trim(), root.model.chunkSizeValue(transferChunkSize.text)], qsTr("Upload file"))
                        storageConfirm.open()
                    }
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Download")
                    enabled: !root.model.busy && !root.model.storageOperationBusy() && root.model.storageMutatingSource() && downloadCid.text.trim().length > 0 && downloadPath.text.trim().length > 0
                    Layout.preferredWidth: 124
                    onClicked: {
                        root.model.confirmStorage("storageDownloadToUrl", [downloadCid.text.trim(), downloadPath.text.trim(), false], qsTr("Download file"))
                        storageConfirm.open()
                    }
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Remove")
                    enabled: !root.model.busy && !root.model.storageOperationBusy() && root.model.storageMutatingSource() && downloadCid.text.trim().length > 0
                    Layout.preferredWidth: 112
                    onClicked: {
                        root.model.confirmStorage("storageRemove", [downloadCid.text.trim()], qsTr("Remove CID"))
                        storageConfirm.open()
                    }
                }

                Item {
                    Layout.fillWidth: true
                }
            }

            StatusMessage {
                visible: root.model.activeStorageOperationKnown()
                theme: root.theme
                tone: root.model.activeStorageTone()
                title: root.model.activeStorageStatusText()
                message: root.model.activeStorageDetailText()
                Layout.fillWidth: true
            }

            RowLayout {
                visible: root.model.activeStorageOperationRunning()
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Cancel")
                    visible: root.model.activeStorageOperationCancelable()
                    enabled: root.model.activeStorageOperationCancelable()
                    Layout.preferredWidth: 112
                    onClicked: root.model.cancelStorageOperation()
                }

                Text {
                    text: root.model.activeStorageProgressText()
                    color: root.theme.textMuted
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    font.pixelSize: root.theme.secondaryText
                    Layout.fillWidth: true
                }
            }
        }
    }

    Component {
        id: operationsTab

        Panel {
            theme: root.theme
            title: qsTr("Operations")

            ColumnLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                Repeater {
                    model: root.model.operationRows()

                    delegate: OperationHistoryRow {
                        required property var modelData

                        theme: root.theme
                        timeText: String(modelData.time || "")
                        labelText: String(modelData.label || "")
                        statusText: String(modelData.status || "")
                        detailText: String(modelData.detail || "")
                    }
                }
            }
        }
    }

    ConfirmActionPopup {
        id: storageConfirm

        theme: root.theme
        title: root.model.pendingLabel
        message: qsTr("This will call the configured Storage source and may change local node state or local files.")
        confirmText: root.model.pendingLabel
        confirmEnabled: root.model.pendingMethod.length > 0
        onAccepted: root.model.runPendingStorage()
    }

    function tabComponent(tab) {
        switch (String(tab || "")) {
        case "cid":
            return cidTab
        case "transfer":
            return transferTab
        case "operations":
            return operationsTab
        default:
            return filesTab
        }
    }

    component ManifestRow: Rectangle {
        id: manifestRoot

        required property Theme theme
        property var row: ({})
        signal useCid(string cid)

        radius: manifestRoot.theme.radius
        color: manifestRoot.theme.field
        border.width: 1
        border.color: manifestRoot.theme.outlineMuted
        implicitHeight: 74
        Layout.fillWidth: true

        RowLayout {
            anchors.fill: parent
            anchors.margins: manifestRoot.theme.gap
            spacing: manifestRoot.theme.gap

            ColumnLayout {
                spacing: 2
                Layout.fillWidth: true

                Text {
                    text: String(manifestRoot.row.name || "-")
                    color: manifestRoot.theme.text
                    textFormat: Text.PlainText
                    font.pixelSize: manifestRoot.theme.primaryText
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }

                Text {
                    text: String(manifestRoot.row.cid || manifestRoot.row.detail || "-")
                    color: manifestRoot.theme.textMuted
                    textFormat: Text.PlainText
                    font.family: "monospace"
                    font.pixelSize: manifestRoot.theme.dataText
                    elide: Text.ElideMiddle
                    Layout.fillWidth: true
                }
            }

            Text {
                text: String(manifestRoot.row.size || "-")
                color: manifestRoot.theme.textMuted
                textFormat: Text.PlainText
                font.family: "monospace"
                font.pixelSize: manifestRoot.theme.secondaryText
                horizontalAlignment: Text.AlignRight
                Layout.preferredWidth: 96
            }

            ActionButton {
                theme: manifestRoot.theme
                text: qsTr("Use")
                enabled: String(manifestRoot.row.cid || "").length > 0
                Layout.preferredWidth: 72
                onClicked: manifestRoot.useCid(String(manifestRoot.row.cid || ""))
            }
        }
    }

}
