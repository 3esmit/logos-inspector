pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../components"
import "../../../theme"
import "../ZonePresentation.js" as Presentation

ColumnLayout {
    id: root

    required property Theme theme
    required property var zoneState
    required property var detail
    property string payloadView: "text"
    readonly property var selectedDetail: root.zoneState.evidenceDetail
    readonly property var payload: root.selectedDetail && root.selectedDetail.payload
        ? root.selectedDetail.payload : ({})

    objectName: "zoneEvidenceViewer"
    spacing: root.theme.gap
    Layout.fillWidth: true

    ListModel {
        id: evidenceFilters

        ListElement { value: "all"; label: "All" }
        ListElement { value: "channel_configuration"; label: "Configuration" }
        ListElement { value: "channel_operation"; label: "Operations" }
        ListElement { value: "raw_inscription"; label: "Raw" }
    }

    TabSwitch {
        theme: root.theme
        options: evidenceFilters
        current: root.zoneState.evidenceFilter
        onSelected: function (value) {
            root.zoneState.loadEvidence(value)
        }
    }

    Text {
        visible: root.zoneState.evidenceError.length > 0
        text: root.zoneState.evidenceError
        color: root.theme.error
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: root.theme.dataText
        Layout.fillWidth: true
    }

    Text {
        visible: root.zoneState.evidenceLoaded
            && root.zoneState.evidenceRows.length === 0
            && !root.zoneState.evidenceInFlight
        text: qsTr("No L1 evidence for this filter")
        color: root.theme.textMuted
        textFormat: Text.PlainText
        font.pixelSize: root.theme.dataText
        Layout.fillWidth: true
    }

    ListView {
        id: evidenceList

        visible: root.zoneState.evidenceRows.length > 0
        model: root.zoneState.evidenceRows
        spacing: root.theme.gapSmall
        clip: true
        reuseItems: true
        boundsBehavior: Flickable.StopAtBounds
        implicitHeight: Math.min(contentHeight, 236)
        Layout.fillWidth: true
        Layout.preferredHeight: Math.min(contentHeight, 236)
        Layout.minimumHeight: visible ? Math.min(contentHeight, 76) : 0

        delegate: ZoneEvidenceRow {
            required property var modelData

            width: evidenceList.width
            theme: root.theme
            evidence: modelData
            selected: root.selectedEvidenceId() === String(modelData.reference.evidence_id || "")
            onActivated: root.zoneState.openEvidence(modelData)
        }

        ScrollBar.vertical: ScrollBar {}
    }

    RowLayout {
        visible: root.zoneState.evidenceInFlight
            || root.zoneState.evidenceNextCursor.length > 0
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        Text {
            text: root.zoneState.evidenceInFlight ? qsTr("Loading L1 evidence...") : ""
            color: root.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: root.theme.dataText
            Layout.fillWidth: true
        }

        ActionButton {
            visible: root.zoneState.evidenceNextCursor.length > 0
            theme: root.theme
            text: qsTr("Load more")
            enabled: !root.zoneState.evidenceInFlight
            onClicked: root.zoneState.loadMoreEvidence()
        }
    }

    Rectangle {
        visible: detailLoader.active
        color: root.theme.outlineMuted
        Layout.fillWidth: true
        Layout.preferredHeight: 1
    }

    Loader {
        id: detailLoader

        active: root.zoneState.evidenceDetailInFlight
            || root.zoneState.selectedEvidenceRow !== null
        asynchronous: false
        Layout.fillWidth: true
        sourceComponent: ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Text {
                visible: root.zoneState.evidenceDetailInFlight
                text: qsTr("Refetching exact L1 evidence...")
                color: root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }

            Text {
                visible: root.zoneState.evidenceDetailError.length > 0
                text: root.zoneState.evidenceDetailError
                color: root.theme.error
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }

            GridLayout {
                visible: root.selectedDetail !== null
                columns: width < 920 ? 1 : 2
                columnSpacing: root.theme.gapXLarge
                rowSpacing: root.theme.gapLarge
                Layout.fillWidth: true

                ZoneFactSection {
                    theme: root.theme
                    title: qsTr("Evidence Location")
                    rows: root.locationRows()
                }

                ZoneFactSection {
                    theme: root.theme
                    title: qsTr("Payload Integrity")
                    rows: root.payloadRows()
                }
            }

            RowLayout {
                visible: root.selectedDetail !== null
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    visible: String(root.payload.encoding || "") === "json"
                    theme: root.theme
                    text: qsTr("JSON")
                    selected: root.payloadView === "json"
                    onClicked: root.payloadView = "json"
                }

                ActionButton {
                    visible: String(root.payload.encoding || "") !== "binary"
                    theme: root.theme
                    text: qsTr("Text")
                    selected: root.payloadView === "text"
                    onClicked: root.payloadView = "text"
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Hex")
                    selected: root.payloadView === "hex"
                    onClicked: root.payloadView = "hex"
                }

                Item { Layout.fillWidth: true }

                ActionButton {
                    visible: String(root.payload.session_id || "").length > 0
                        && !root.zoneState.evidencePayloadDone
                    theme: root.theme
                    text: qsTr("Load next chunk")
                    enabled: !root.zoneState.evidencePayloadInFlight
                    onClicked: root.zoneState.loadNextEvidencePayloadChunk()
                }
            }

            TextArea {
                visible: root.selectedDetail !== null
                readOnly: true
                selectByMouse: true
                text: root.payloadBody()
                textFormat: Text.PlainText
                wrapMode: TextEdit.WrapAnywhere
                color: root.theme.text
                selectionColor: root.theme.accent
                selectedTextColor: root.theme.selectedText
                font.family: "monospace"
                font.pixelSize: root.theme.dataText
                padding: root.theme.gap
                Layout.fillWidth: true
                Layout.preferredHeight: 176

                background: Rectangle {
                    radius: root.theme.radius
                    color: root.theme.field
                    border.width: 1
                    border.color: root.theme.outlineMuted
                }
            }

            Text {
                visible: root.zoneState.evidencePayloadError.length > 0
                    || root.payload.warning !== null && root.payload.warning !== undefined
                text: root.zoneState.evidencePayloadError.length > 0
                    ? root.zoneState.evidencePayloadError
                    : String(root.payload.warning && root.payload.warning.message || "")
                color: root.theme.warning
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }
        }
    }

    Component.onCompleted: root.ensureLoaded()

    Connections {
        target: root.zoneState

        function onActiveZoneIdChanged() {
            root.ensureLoaded()
        }

        function onEvidenceDetailChanged() {
            const encoding = String(root.payload.encoding || "")
            root.payloadView = encoding === "json" ? "json"
                : (encoding === "binary" ? "hex" : "text")
        }
    }

    function ensureLoaded() {
        if (zoneState.activeZoneId.length > 0
                && zoneState.verification === "verified"
                && !zoneState.evidenceLoaded
                && !zoneState.evidenceInFlight) {
            zoneState.loadEvidence(zoneState.evidenceFilter)
        }
    }

    function selectedEvidenceId() {
        return String(zoneState.selectedEvidenceRow && zoneState.selectedEvidenceRow.reference
            && zoneState.selectedEvidenceRow.reference.evidence_id || "")
    }

    function locationRows() {
        const row = selectedDetail && selectedDetail.row ? selectedDetail.row : ({})
        const reference = row.reference || ({})
        return [{
            label: qsTr("Kind"),
            value: Presentation.evidenceKindLabel(reference.evidence_kind)
        }, {
            label: qsTr("L1 slot"),
            value: Presentation.numberText(reference.l1_slot),
            tone: "success"
        }, {
            label: qsTr("Block"),
            value: Presentation.text(reference.block_id),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Transaction"),
            value: Presentation.text(reference.transaction_hash),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Operation"),
            value: Presentation.numberText(reference.operation_index)
        }, {
            label: qsTr("Coverage segment"),
            value: Presentation.text(row.segment && row.segment.segment_id),
            monospace: true
        }]
    }

    function payloadRows() {
        const row = selectedDetail && selectedDetail.row ? selectedDetail.row : ({})
        return [{
            label: qsTr("Encoding"),
            value: Presentation.words(payload.encoding)
        }, {
            label: qsTr("Size"),
            value: qsTr("%1 bytes").arg(Presentation.numberText(payload.byte_length))
        }, {
            label: qsTr("SHA-256"),
            value: Presentation.text(payload.sha256),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Source"),
            value: Presentation.text(row.source && row.source.fingerprint),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Finality"),
            value: Presentation.words(row.finality),
            tone: "success"
        }]
    }

    function payloadBody() {
        const inlineText = payload.inline_text === null || payload.inline_text === undefined
            ? "" : String(payload.inline_text)
        const inlineBase64 = payload.inline_base64 === null || payload.inline_base64 === undefined
            ? "" : String(payload.inline_base64)
        let textChunks = ""
        let hexChunks = ""
        const chunks = Array.isArray(zoneState.evidencePayloadChunks)
            ? zoneState.evidencePayloadChunks : []
        for (let i = 0; i < chunks.length; ++i) {
            textChunks += String(chunks[i].text || "")
            if (String(chunks[i].base64 || "").length > 0) {
                hexChunks += base64ToHex(chunks[i].base64)
            } else {
                hexChunks += stringToHex(chunks[i].text)
            }
        }
        if (payloadView === "hex") {
            if (inlineBase64.length > 0) {
                return base64ToHex(inlineBase64)
            }
            if (inlineText.length > 0) {
                return stringToHex(inlineText)
            }
            return hexChunks.length > 0 ? hexChunks : Presentation.text(payload.preview)
        }
        const body = inlineText.length > 0 ? inlineText
            : (textChunks.length > 0 ? textChunks : Presentation.text(payload.preview))
        if (payloadView === "json") {
            try {
                return JSON.stringify(JSON.parse(body), null, 2)
            } catch (_error) {
                return body
            }
        }
        return body
    }

    function base64ToHex(value) {
        try {
            const bytes = Qt.atob(String(value || ""))
            let result = ""
            for (let i = 0; i < bytes.length; ++i) {
                result += ("0" + bytes.charCodeAt(i).toString(16)).slice(-2)
            }
            return result
        } catch (_error) {
            return ""
        }
    }

    function stringToHex(value) {
        const encoded = unescape(encodeURIComponent(String(value || "")))
        let result = ""
        for (let i = 0; i < encoded.length; ++i) {
            result += ("0" + encoded.charCodeAt(i).toString(16)).slice(-2)
        }
        return result
    }
}
