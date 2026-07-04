pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQml.Models
import QtQuick.Layouts
import "../components"
import "../components/common"
import "../services/BridgeHelpers.js" as BridgeHelpers
import "../state"
import "../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model
    property var manifests: []
    property string lastOperation: qsTr("None")
    property string activeCid: ""

    width: parent ? parent.width : 900
    spacing: root.theme.gapLarge

    ListModel {
        id: storageTabs

        ListElement { value: "files"; label: "My Files" }
        ListElement { value: "cid"; label: "CID" }
        ListElement { value: "transfer"; label: "Transfer" }
        ListElement { value: "operations"; label: "Operations" }
    }

    ListModel {
        id: operationLog
    }

    Component.onCompleted: root.refreshManifests(false)

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Network / Storage")
        title: qsTr("Storage")
        layerLabel: qsTr("Network")
        subtitle: qsTr("Share, find, download, and remove content through the configured Storage source.")
        Layout.fillWidth: true
    }

    SourceStrip {
        theme: root.theme
        sources: root.sourceBadges()
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
            value: root.model.storageSourceLabel()
            tone: root.storageDataSource() ? "success" : "warning"
            Layout.fillWidth: true
        }

        StatusChip {
            theme: root.theme
            label: qsTr("Files")
            value: String(root.manifests.length)
            tone: root.manifests.length > 0 ? "success" : "neutral"
            Layout.fillWidth: true
        }

        StatusChip {
            theme: root.theme
            label: qsTr("Network")
            value: root.model.storageNetworkPreset
            tone: "neutral"
            Layout.fillWidth: true
        }

        StatusChip {
            theme: root.theme
            label: qsTr("Last")
            value: root.lastOperation
            tone: root.model.resultIsError && root.model.resultOwner === root.model.currentView ? "error" : "neutral"
            Layout.fillWidth: true
        }
    }

    TabSwitch {
        theme: root.theme
        current: root.model.storageAppTab
        options: storageTabs
        Layout.fillWidth: true
        onSelected: value => root.model.storageAppTab = value
    }

    Loader {
        active: true
        sourceComponent: root.tabComponent(root.model.storageAppTab)
        Layout.fillWidth: true
    }

    Panel {
        visible: root.model.pageHasOutput("storage")
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
                    onClicked: root.refreshManifests(true)
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Settings")
                    enabled: !root.model.busy
                    Layout.preferredWidth: 104
                    onClicked: root.model.openSettings("network", "storage")
                }

                Text {
                    text: root.storageTargetText()
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
                    model: root.manifestRows()

                    delegate: ManifestRow {
                        required property var modelData

                        theme: root.theme
                        row: modelData
                        onUseCid: cid => {
                            root.activeCid = cid
                            root.model.storageAppTab = "cid"
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
                    sourceText: root.activeCid
                    syncSourceText: true
                    Layout.fillWidth: true
                    onTextEdited: text => root.activeCid = text
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
                    enabled: !root.model.busy && cidField.text.trim().length > 0 && root.storageDataSource()
                    Layout.preferredWidth: 104
                    onClicked: root.runStorage("storageExists", [cidField.text.trim()], qsTr("Storage exists"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Fetch")
                    enabled: !root.model.busy && cidField.text.trim().length > 0 && root.storageModuleSource()
                    Layout.preferredWidth: 104
                    onClicked: root.runStorage("storageDownloadManifest", [cidField.text.trim()], qsTr("Fetch manifest"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Cache")
                    enabled: !root.model.busy && cidField.text.trim().length > 0 && root.storageModuleSource()
                    Layout.preferredWidth: 104
                    onClicked: root.runStorage("storageFetch", [cidField.text.trim()], qsTr("Cache CID"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Download")
                    primary: true
                    enabled: !root.model.busy && cidField.text.trim().length > 0 && cidDestination.text.trim().length > 0 && root.storageModuleSource()
                    Layout.preferredWidth: 124
                    onClicked: root.runStorage("storageDownloadToUrl", [cidField.text.trim(), cidDestination.text.trim(), localOnly.checked, root.chunkSizeValue(chunkSize.text)], qsTr("Download CID"))
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
                    enabled: root.storageModuleSource()
                    palette.text: root.theme.text
                    Layout.preferredWidth: 132
                }

                FieldRow {
                    id: chunkSize

                    theme: root.theme
                    label: qsTr("Chunk size")
                    text: "65536"
                    Layout.fillWidth: true
                }
            }
        }
    }

    Component {
        id: transferTab

        Panel {
            theme: root.theme
            title: qsTr("Transfer")

            StatusMessage {
                visible: !root.storageModuleSource()
                theme: root.theme
                tone: "warning"
                title: qsTr("Module source required")
                message: qsTr("Select the Storage module source in Settings to run upload, download, fetch, and remove operations.")
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
                    label: qsTr("File path or URL")
                    placeholderText: qsTr("/home/user/file.bin")
                    Layout.fillWidth: true
                }

                FieldRow {
                    id: downloadCid

                    theme: root.theme
                    label: qsTr("CID")
                    placeholderText: qsTr("zDv...")
                    sourceText: root.activeCid
                    syncSourceText: true
                    Layout.fillWidth: true
                    onTextEdited: text => root.activeCid = text
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
                    label: qsTr("Chunk size")
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
                    enabled: !root.model.busy && root.storageModuleSource() && uploadPath.text.trim().length > 0
                    Layout.preferredWidth: 112
                    onClicked: root.runStorage("storageUploadUrl", [uploadPath.text.trim(), root.chunkSizeValue(transferChunkSize.text)], qsTr("Upload file"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Download")
                    enabled: !root.model.busy && root.storageModuleSource() && downloadCid.text.trim().length > 0 && downloadPath.text.trim().length > 0
                    Layout.preferredWidth: 124
                    onClicked: root.runStorage("storageDownloadToUrl", [downloadCid.text.trim(), downloadPath.text.trim(), false, root.chunkSizeValue(transferChunkSize.text)], qsTr("Download file"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Remove")
                    enabled: !root.model.busy && root.storageModuleSource() && downloadCid.text.trim().length > 0
                    Layout.preferredWidth: 112
                    onClicked: root.runStorage("storageRemove", [downloadCid.text.trim()], qsTr("Remove CID"))
                }

                Item {
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
                    model: operationLog.count > 0 ? operationLog : emptyOperationModel

                    delegate: OperationHistoryRow {
                        required property string time
                        required property string label
                        required property string status
                        required property string detail

                        theme: root.theme
                        timeText: time
                        labelText: label
                        statusText: status
                        detailText: detail
                    }
                }
            }
        }
    }

    ListModel {
        id: emptyOperationModel

        ListElement {
            time: "-"
            label: "No operations"
            status: "-"
            detail: "-"
        }
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

    function sourceBadges() {
        const sources = [qsTr("Storage"), root.model.storageSourceLabel()]
        sources.push(root.shortText(root.storageTargetText(), 42))
        sources.push(root.model.storageNetworkPreset)
        return sources
    }

    function storageTargetText() {
        return root.model.storageSourceTarget()
    }

    function shortText(value, max) {
        const text = String(value || "")
        const limit = Math.max(8, Number(max || 42))
        if (text.length <= limit) {
            return text
        }
        return text.slice(0, Math.max(3, limit - 1)) + "..."
    }

    function storageModuleSource() {
        const mode = String(root.model.effectiveStorageSourceMode(root.model.storageSourceMode) || "").toLowerCase()
        return mode === "module" || mode === "basecamp" || mode === "basecamp-module" || mode === "basecamp module"
    }

    function storageRestSource() {
        return String(root.model.effectiveStorageSourceMode(root.model.storageSourceMode) || "").toLowerCase() === "rest"
    }

    function storageDataSource() {
        return root.storageModuleSource() || root.storageRestSource()
    }

    function storageArgs(extra) {
        const args = [root.model.effectiveStorageSourceMode(root.model.storageSourceMode), root.model.storageRestUrl]
        return args.concat(extra || [])
    }

    function refreshManifests(showLog) {
        if (root.model.busy || !root.storageDataSource()) {
            return
        }
        const response = root.model.callInspector("storageManifests", root.storageArgs([]), qsTr("Storage manifests"))
        if (showLog) {
            root.appendOperation(qsTr("List files"), response)
        }
        if (response.ok) {
            root.manifests = root.manifestArray(response.value)
            root.lastOperation = qsTr("List")
        } else if (showLog) {
            root.lastOperation = qsTr("Error")
        }
    }

    function runStorage(method, args, label) {
        const response = root.model.callInspector(method, root.storageArgs(args), label)
        root.appendOperation(label, response)
        root.lastOperation = response.ok ? label : qsTr("Error")
        return response
    }

    function appendOperation(label, response) {
        operationLog.insert(0, {
            time: root.timeText(),
            label: String(label || ""),
            status: response && response.ok ? qsTr("ok") : qsTr("error"),
            detail: response && response.ok ? root.operationSummary(response.value) : String((response && response.error) || "")
        })
        while (operationLog.count > 20) {
            operationLog.remove(operationLog.count - 1)
        }
    }

    function operationPayload(value) {
        if (value && value.value && value.value.result && value.value.result.value !== undefined) {
            return value.value.result.value
        }
        if (value && value.result && value.result.value !== undefined) {
            return value.result.value
        }
        if (value && value.value !== undefined) {
            return value.value
        }
        return value
    }

    function manifestArray(value) {
        const payload = root.operationPayload(value)
        if (Array.isArray(payload)) {
            return payload
        }
        if (payload && Array.isArray(payload.content)) {
            return payload.content
        }
        if (payload && Array.isArray(payload.manifests)) {
            return payload.manifests
        }
        if (payload && Array.isArray(payload.value)) {
            return payload.value
        }
        return []
    }

    function manifestRows() {
        if (root.manifests.length === 0) {
            return [{
                cid: "",
                name: qsTr("No local manifests"),
                detail: qsTr(""),
                size: "-",
                mime: "-"
            }]
        }
        return root.manifests.map(function (manifest) {
            const row = manifest || {}
            const cid = String(row.cid || row.CID || row.id || "")
            const name = String(row.filename || row.name || row.path || cid || qsTr("Untitled"))
            const size = row.datasetSize || row.size || row.bytes || row.totalSize || "-"
            const blockSize = row.blockSize || row.block_size || ""
            return {
                cid: cid,
                name: name,
                detail: blockSize ? qsTr("block %1").arg(blockSize) : String(row.treeCid || row.tree_cid || ""),
                size: String(size),
                mime: String(row.mimetype || row.mimeType || row.contentType || "-")
            }
        })
    }

    function operationSummary(value) {
        const payload = root.operationPayload(value)
        if (payload === undefined || payload === null) {
            return qsTr("No value")
        }
        if (typeof payload === "string") {
            return payload
        }
        if (typeof payload === "boolean") {
            return payload ? qsTr("true") : qsTr("false")
        }
        return BridgeHelpers.formatValue(payload).replace(/\s+/g, " ").slice(0, 180)
    }

    function chunkSizeValue(text) {
        const parsed = Number(String(text || "").trim())
        if (!isFinite(parsed) || parsed <= 0) {
            return 65536
        }
        return Math.floor(parsed)
    }

    function timeText() {
        return Qt.formatTime(new Date(), "HH:mm:ss")
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
