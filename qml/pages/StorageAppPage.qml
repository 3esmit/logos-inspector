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
    property string activeCid: root.model.storageCidProbe
    property string pendingStorageMethod: ""
    property string pendingStorageLabel: ""
    property var pendingStorageArgs: []
    property string terminalStorageOperationId: ""

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

    Component.onCompleted: {
        if (root.activeCid.length === 0 && root.model.storageCidProbe.length > 0) {
            root.activeCid = root.model.storageCidProbe
        }
        root.refreshManifests(false)
    }

    Connections {
        target: root.model

        function onStorageCidProbeChanged() {
            if (root.activeCid !== root.model.storageCidProbe) {
                root.activeCid = root.model.storageCidProbe
            }
        }
    }

    Timer {
        id: storageOperationPoll

        interval: 500
        repeat: true
        running: root.activeStorageOperationRunning()
        onTriggered: root.pollStorageOperation(false)
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Network / Storage")
        title: qsTr("Storage")
        layerLabel: qsTr("Network")
        subtitle: qsTr("Inspect local manifests and CID presence through the configured Storage REST source.")
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

        StatusChip {
            theme: root.theme
            label: qsTr("Active")
            value: root.activeStorageStatusText()
            tone: root.activeStorageTone()
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
                    onTextEdited: text => {
                        root.activeCid = text
                        root.model.storageCidProbe = String(text || "").trim()
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
                    enabled: !root.model.busy && cidField.text.trim().length > 0 && root.storageDataSource()
                    Layout.preferredWidth: 104
                    onClicked: root.runStorage("storageExists", [cidField.text.trim()], qsTr("Storage exists"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Fetch")
                    enabled: !root.model.busy && cidField.text.trim().length > 0 && root.storageDataSource()
                    Layout.preferredWidth: 104
                    onClicked: root.runStorage("storageDownloadManifest", [cidField.text.trim()], qsTr("Fetch manifest"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Cache")
                    enabled: !root.model.busy && cidField.text.trim().length > 0 && root.storageMutatingSource()
                    Layout.preferredWidth: 104
                    onClicked: root.confirmStorage("storageFetch", [cidField.text.trim()], qsTr("Cache CID"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Download")
                    primary: true
                    enabled: !root.model.busy && !root.activeStorageOperationRunning() && cidField.text.trim().length > 0 && cidDestination.text.trim().length > 0 && root.storageMutatingSource()
                    Layout.preferredWidth: 124
                    onClicked: root.confirmStorage("storageDownloadToUrl", [cidField.text.trim(), cidDestination.text.trim(), localOnly.checked], qsTr("Download CID"))
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
                    enabled: root.storageMutatingSource()
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
                visible: !root.storageRestSource()
                theme: root.theme
                tone: "warning"
                title: qsTr("REST source required")
                message: qsTr("Upload, download, fetch, and remove use the configured Storage REST source.")
                Layout.fillWidth: true
            }

            StatusMessage {
                visible: root.storageRestSource() && !root.model.storageMutatingDiagnosticsEnabled
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
                    sourceText: root.activeCid
                    syncSourceText: true
                    Layout.fillWidth: true
                    onTextEdited: text => {
                        root.activeCid = text
                        root.model.storageCidProbe = String(text || "").trim()
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
                    enabled: !root.model.busy && root.storageMutatingSource() && uploadPath.text.trim().length > 0
                    Layout.preferredWidth: 112
                    onClicked: root.confirmStorage("storageUploadUrl", [uploadPath.text.trim(), root.chunkSizeValue(transferChunkSize.text)], qsTr("Upload file"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Download")
                    enabled: !root.model.busy && !root.activeStorageOperationRunning() && root.storageMutatingSource() && downloadCid.text.trim().length > 0 && downloadPath.text.trim().length > 0
                    Layout.preferredWidth: 124
                    onClicked: root.confirmStorage("storageDownloadToUrl", [downloadCid.text.trim(), downloadPath.text.trim(), false], qsTr("Download file"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Remove")
                    enabled: !root.model.busy && root.storageMutatingSource() && downloadCid.text.trim().length > 0
                    Layout.preferredWidth: 112
                    onClicked: root.confirmStorage("storageRemove", [downloadCid.text.trim()], qsTr("Remove CID"))
                }

                Item {
                    Layout.fillWidth: true
                }
            }

            StatusMessage {
                visible: root.activeStorageOperationKnown()
                theme: root.theme
                tone: root.activeStorageTone()
                title: root.activeStorageStatusText()
                message: root.activeStorageDetailText()
                Layout.fillWidth: true
            }

            RowLayout {
                visible: root.activeStorageOperationRunning()
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Cancel")
                    enabled: root.activeStorageOperationRunning()
                    Layout.preferredWidth: 112
                    onClicked: root.cancelStorageOperation()
                }

                Text {
                    text: root.activeStorageProgressText()
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

    ConfirmActionPopup {
        id: storageConfirm

        theme: root.theme
        title: root.pendingStorageLabel
        message: qsTr("This will call the configured Storage REST source and may change local node state or local files.")
        confirmText: root.pendingStorageLabel
        confirmEnabled: root.pendingStorageMethod.length > 0
        onAccepted: root.runPendingStorage()
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

    function storageRestSource() {
        return String(root.model.effectiveStorageSourceMode(root.model.storageSourceMode) || "").toLowerCase() === "rest"
    }

    function storageMutatingSource() {
        return root.storageRestSource() && root.model.storageMutatingDiagnosticsEnabled === true
    }

    function storageDataSource() {
        return root.storageRestSource()
    }

    function storageArgs(extra) {
        const args = [root.model.effectiveStorageSourceMode(root.model.storageSourceMode), root.model.configuredStorageRestUrl()]
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

    function confirmStorage(method, args, label) {
        root.pendingStorageMethod = String(method || "")
        root.pendingStorageArgs = [root.model.storageMutatingDiagnosticsEnabled === true].concat(args || [])
        root.pendingStorageLabel = String(label || "")
        storageConfirm.open()
    }

    function runPendingStorage() {
        if (!root.pendingStorageMethod.length) {
            return
        }
        if (root.pendingStorageMethod === "storageDownloadToUrl") {
            root.startStorageDownload(root.pendingStorageArgs, root.pendingStorageLabel)
        } else {
            root.runStorage(root.pendingStorageMethod, root.pendingStorageArgs, root.pendingStorageLabel)
        }
        root.pendingStorageMethod = ""
        root.pendingStorageArgs = []
        root.pendingStorageLabel = ""
    }

    function startStorageDownload(args, label) {
        if (root.activeStorageOperationRunning()) {
            const blocked = {
                ok: false,
                text: "",
                error: qsTr("A storage download is already running.")
            }
            root.appendOperation(label, blocked)
            root.lastOperation = qsTr("Busy")
            return blocked
        }
        const response = root.model.callInspector("storageDownloadStart", root.storageArgs(args), label)
        root.appendOperation(label, response)
        root.lastOperation = response.ok ? qsTr("Download started") : qsTr("Error")
        if (response.ok) {
            root.terminalStorageOperationId = ""
            root.model.updateStorageActiveOperation(response.value)
            storageOperationPoll.restart()
            root.model.storageAppTab = "operations"
        }
        return response
    }

    function pollStorageOperation(showResult) {
        const operation = root.activeStorageOperation()
        const operationId = String(operation && operation.operationId ? operation.operationId : "")
        if (!operationId.length) {
            storageOperationPoll.stop()
            return
        }
        root.model.requestModuleAsync(root.model.inspectorModule, "storageOperationStatus", [operationId], qsTr("Storage operation"), showResult === true, function (response) {
            if (!response || !response.ok) {
                storageOperationPoll.stop()
                return
            }
            root.model.updateStorageActiveOperation(response.value)
            if (root.activeStorageOperationTerminal(response.value)) {
                storageOperationPoll.stop()
                root.appendTerminalStorageOperation(response.value)
            }
        })
    }

    function cancelStorageOperation() {
        const operation = root.activeStorageOperation()
        const operationId = String(operation && operation.operationId ? operation.operationId : "")
        if (!operationId.length) {
            return
        }
        const response = root.model.callInspector("storageOperationCancel", [operationId], qsTr("Cancel storage operation"))
        if (response.ok) {
            root.model.updateStorageActiveOperation(response.value)
            storageOperationPoll.restart()
        }
        root.appendOperation(qsTr("Cancel download"), response)
    }

    function appendTerminalStorageOperation(operation) {
        const operationId = String(operation && operation.operationId ? operation.operationId : "")
        if (!operationId.length || root.terminalStorageOperationId === operationId) {
            return
        }
        root.terminalStorageOperationId = operationId
        const ok = String(operation.status || "") === "completed"
        root.appendOperation(qsTr("Download file"), {
            ok: ok,
            value: operation.result || operation,
            error: String(operation.error || "")
        })
        root.lastOperation = ok ? qsTr("Download complete") : qsTr("Download stopped")
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

    function activeStorageOperation() {
        const revision = root.model.storageActiveOperationRevision
        return root.model.storageActiveOperation || null
    }

    function activeStorageOperationKnown() {
        const operation = root.activeStorageOperation()
        return operation && String(operation.operationId || "").length > 0
    }

    function activeStorageOperationRunning() {
        const operation = root.activeStorageOperation()
        const status = String(operation && operation.status ? operation.status : "")
        return status === "running" || status === "canceling"
    }

    function activeStorageOperationTerminal(operation) {
        const status = String(operation && operation.status ? operation.status : "")
        return status === "completed" || status === "failed" || status === "canceled"
    }

    function activeStorageStatusText() {
        const operation = root.activeStorageOperation()
        const status = String(operation && operation.status ? operation.status : "")
        switch (status) {
        case "running":
            return qsTr("Downloading")
        case "canceling":
            return qsTr("Canceling")
        case "completed":
            return qsTr("Complete")
        case "failed":
            return qsTr("Failed")
        case "canceled":
            return qsTr("Canceled")
        default:
            return qsTr("Idle")
        }
    }

    function activeStorageTone() {
        const operation = root.activeStorageOperation()
        const status = String(operation && operation.status ? operation.status : "")
        if (status === "completed") {
            return "success"
        }
        if (status === "failed") {
            return "error"
        }
        if (status === "running" || status === "canceling") {
            return "warning"
        }
        return "neutral"
    }

    function activeStorageDetailText() {
        const operation = root.activeStorageOperation()
        if (!operation) {
            return qsTr("No active operation.")
        }
        const detail = [
            root.shortText(operation.cid, 28),
            root.activeStorageProgressText(),
            root.shortText(operation.path, 48)
        ].filter(value => String(value || "").length > 0)
        if (operation.error) {
            detail.push(String(operation.error))
        }
        return detail.join(" / ")
    }

    function activeStorageProgressText() {
        const operation = root.activeStorageOperation()
        if (!operation) {
            return ""
        }
        const written = Number(operation.bytesWritten || 0)
        const total = Number(operation.contentLength || 0)
        if (Number.isFinite(total) && total > 0) {
            const percent = Math.min(100, Math.max(0, Math.floor((written / total) * 100)))
            return qsTr("%1 / %2 bytes (%3%)").arg(root.model.valueText(written)).arg(root.model.valueText(total)).arg(percent)
        }
        return qsTr("%1 bytes").arg(root.model.valueText(written))
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
            const metadata = row.manifest || {}
            const cid = String(row.cid || row.CID || row.id || "")
            const name = String(metadata.filename || row.filename || row.name || row.path || cid || qsTr("Untitled"))
            const size = metadata.datasetSize || row.datasetSize || row.size || row.bytes || row.totalSize || "-"
            const blockSize = metadata.blockSize || row.blockSize || row.block_size || ""
            return {
                cid: cid,
                name: name,
                detail: blockSize ? qsTr("block %1").arg(blockSize) : String(metadata.treeCid || row.treeCid || row.tree_cid || ""),
                size: String(size),
                mime: String(metadata.mimetype || row.mimetype || row.mimeType || row.contentType || "-")
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
