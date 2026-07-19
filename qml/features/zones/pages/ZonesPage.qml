pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../components"
import "../../../components/common"
import "../../../theme"
import "../controls"
import "../ZonePresentation.js" as Presentation

ColumnLayout {
    id: root

    required property Theme theme
    required property var model
    property string filter: "all"
    property string query: ""
    property string initialDetailTab: "overview"
    property bool sourceEditorInitiallyOpen: false
    property string pendingZoneId: ""
    property string pendingZoneView: ""
    property bool retainDetailForDraft: false
    readonly property var zoneState: root.model && root.model.zoneInspection
        ? root.model.zoneInspection : null
    readonly property bool rowsStale: root.zoneState
        && (root.zoneState.verification !== "verified" || root.zoneState.summaryStale)
    readonly property var visibleZones: Presentation.filterRows(
        root.zoneState ? root.zoneState.zoneSummaries : [],
        root.filter,
        root.query,
        root.rowsStale
    )
    readonly property bool stacked: width < 900
    readonly property bool hasDirtyDraft: detailLoader.detailItem !== null
        && detailLoader.detailItem.hasDirtyDraft

    onHasDirtyDraftChanged: {
        if (hasDirtyDraft) {
            retainDetailForDraft = true
        } else if (zoneState && zoneState.zoneDetail !== null) {
            retainDetailForDraft = false
        }
    }

    objectName: "zonesPage"
    width: parent ? parent.width : 1180
    spacing: root.theme.gapLarge

    ListModel {
        id: zoneFilters

        ListElement { value: "all"; label: "All" }
        ListElement { value: "sequencer"; label: "Sequencer" }
        ListElement { value: "data"; label: "Data" }
        ListElement { value: "attention"; label: "Needs attention" }
    }

    PageHeader {
        theme: root.theme
        layerLabel: qsTr("NETWORK")
        breadcrumb: qsTr("L1 Channels / L2 settlement")
        title: qsTr("Zones")
        subtitle: root.catalogSubtitle()

        ActionButton {
            visible: root.zoneState && (root.zoneState.currentError.length > 0
                || root.zoneState.statusError.length > 0)
            theme: root.theme
            text: qsTr("Retry catalog")
            enabled: !root.zoneState.controlInFlight
            onClicked: root.zoneState.retryCatalog()
        }
    }

    ZoneCatalogStatus {
        theme: root.theme
        zoneState: root.zoneState
        Layout.fillWidth: true
    }

    ColumnLayout {
        visible: root.zoneState && Array.isArray(root.zoneState.targetResolutionCandidates)
            && root.zoneState.targetResolutionCandidates.length > 1
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        StatusMessage {
            theme: root.theme
            tone: "warning"
            title: qsTr("Search is ambiguous")
            message: qsTr("Select a typed target. Source order is not used as a tie-breaker.")
            Layout.fillWidth: true
        }

        DataTableFrame {
            objectName: "inspectionTargetCandidatesTable"
            theme: root.theme
            headerCells: [
                { text: qsTr("Layer"), width: 72 },
                { text: qsTr("Entity"), width: 120 },
                { text: qsTr("Canonical key"), width: 320, fill: true },
                { text: qsTr("Source"), width: 220, fill: true }
            ]
            rows: root.targetCandidateRows()
            Layout.fillWidth: true
            onCellActivated: function (row, column, cell, rowData) {
                const candidate = rowData.candidate
                root.zoneState.resetTargetResolution()
                root.model.openInspectionCandidate(candidate, false)
            }
        }
    }

    RowLayout {
        spacing: root.theme.gap
        Layout.fillWidth: true

        TabSwitch {
            theme: root.theme
            options: zoneFilters
            current: root.filter
            onSelected: function (value) {
                root.filter = value
            }
        }

        TextField {
            id: zoneSearch

            objectName: "zoneSearchField"
            placeholderText: qsTr("Filter by name or Channel")
            color: root.theme.text
            placeholderTextColor: root.theme.textMuted
            selectionColor: root.theme.accent
            selectedTextColor: root.theme.selectedText
            font.pixelSize: root.theme.secondaryText
            Layout.preferredWidth: root.stacked ? 220 : 280
            Layout.minimumWidth: 160
            Layout.preferredHeight: root.theme.controlHeight
            onTextChanged: root.query = text

            background: Rectangle {
                radius: root.theme.radius
                color: root.theme.field
                border.width: zoneSearch.activeFocus ? 1 : 0
                border.color: root.theme.accent
            }
        }
    }

    GridLayout {
        columns: root.stacked ? 1 : 2
        columnSpacing: root.theme.gapXLarge
        rowSpacing: root.theme.gapXLarge
        Layout.fillWidth: true

        ColumnLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true
            Layout.preferredWidth: root.stacked ? root.width : 430
            Layout.minimumWidth: 320
            Layout.alignment: Qt.AlignTop

            RowLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                Text {
                    text: qsTr("Zone catalog")
                    color: root.theme.text
                    textFormat: Text.PlainText
                    font.pixelSize: root.theme.secondaryText
                    font.weight: Font.DemiBold
                    Layout.fillWidth: true
                }

                Text {
                    text: qsTr("%1 visible / %2 total")
                        .arg(Presentation.numberText(root.visibleZones.length))
                        .arg(Presentation.numberText(root.zoneState
                            && root.zoneState.zoneSummaries.length))
                    color: root.theme.textDim
                    textFormat: Text.PlainText
                    font.pixelSize: root.theme.dataText
                }
            }

            Text {
                visible: root.rowsStale && root.zoneState && root.zoneState.zoneSummaries.length > 0
                text: qsTr("Cached catalog rows / verification required")
                color: root.theme.warning
                textFormat: Text.PlainText
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }

            Text {
                visible: root.visibleZones.length === 0 && !root.zoneState.summaryInFlight
                text: root.zoneState.summaryError.length > 0
                    ? root.zoneState.summaryError : qsTr("No Zone facts match current filter")
                color: root.zoneState.summaryError.length > 0
                    ? root.theme.error : root.theme.textMuted
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }

            ListView {
                id: zonesList

                objectName: "zonesList"
                visible: root.visibleZones.length > 0
                model: root.visibleZones
                spacing: root.theme.gapSmall
                clip: true
                reuseItems: true
                boundsBehavior: Flickable.StopAtBounds
                implicitHeight: Math.max(120, Math.min(contentHeight, root.stacked ? 460 : 650))
                Layout.fillWidth: true
                Layout.preferredHeight: implicitHeight

                delegate: ZoneListRow {
                    required property var modelData

                    width: zonesList.width
                    theme: root.theme
                    zone: modelData
                    selected: root.zoneState.activeZoneId === String(modelData.channel_id || "")
                    stale: root.rowsStale || String(modelData.provenance
                        && modelData.provenance.verification_state || "verified") !== "verified"
                    interactive: !stale
                    onActivated: root.requestZoneActivation(String(modelData.channel_id || ""))
                    onChannelActivated: root.requestSequencerActivation(
                        String(modelData.channel_id || ""))
                }

                ScrollBar.vertical: ScrollBar {}
            }
        }

        ColumnLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true
            Layout.preferredWidth: root.stacked ? root.width : 650
            Layout.minimumWidth: 420
            Layout.alignment: Qt.AlignTop

            Text {
                visible: root.hasDirtyDraft && root.zoneState.zoneDetail === null
                text: qsTr("Network or Zone context changed. Source draft retained; saving is disabled.")
                color: root.theme.warning
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }

            Text {
                visible: root.zoneState.activeZoneId.length === 0 && !root.hasDirtyDraft
                text: qsTr("No active Zone")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: root.theme.panelTitleText
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            Text {
                visible: root.zoneState.activeZoneId.length === 0 && !root.hasDirtyDraft
                text: qsTr("%1 verified catalog rows available")
                    .arg(Presentation.numberText(root.zoneState.zoneSummaries.length))
                color: root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }

            Text {
                visible: root.zoneState.detailInFlight && root.zoneState.zoneDetail === null
                text: qsTr("Loading Zone detail...")
                color: root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }

            Text {
                visible: root.zoneState.detailError.length > 0
                text: root.zoneState.detailError
                color: root.theme.error
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }

            Loader {
                id: detailLoader

                readonly property ZoneDetail detailItem: item as ZoneDetail

                // Do not derive Loader.active from item. Keep a separate
                // latch for an unsaved draft, while avoiding a ZoneDetail
                // instance when there is no detail to render.
                active: root.zoneState.zoneDetail !== null || root.retainDetailForDraft
                asynchronous: false
                Layout.fillWidth: true
                sourceComponent: ZoneDetail {
                    theme: root.theme
                    model: root.model
                    zoneState: root.zoneState
                    initialTab: root.initialDetailTab
                    sourceEditorInitiallyOpen: root.sourceEditorInitiallyOpen
                }
            }
        }
    }

    ConfirmActionPopup {
        id: zoneGuardPopup

        objectName: "zoneNavigationGuard"
        theme: root.theme
        title: qsTr("Discard source draft")
        message: qsTr("Discard unsaved Channel source changes before changing Zone?")
        confirmText: qsTr("Discard")
        onAccepted: {
            root.discardSourceDraft()
            root.activateZoneForView(root.pendingZoneId, root.pendingZoneView)
            root.pendingZoneId = ""
            root.pendingZoneView = ""
        }
    }

    function requestZoneActivation(channelId) {
        if (channelId.length === 0) {
            return false
        }
        cancelPendingInspectionOpen()
        if (channelId === zoneState.activeZoneId) {
            return false
        }
        if (hasDirtyDraft) {
            pendingZoneId = channelId
            pendingZoneView = ""
            zoneGuardPopup.open()
            return false
        }
        return zoneState.activateZone(channelId)
    }

    function requestSequencerActivation(channelId) {
        if (channelId.length === 0) {
            return false
        }
        cancelPendingInspectionOpen()
        if (hasDirtyDraft) {
            pendingZoneId = channelId
            pendingZoneView = "sequencerDashboard"
            zoneGuardPopup.open()
            return false
        }
        return activateZoneForView(channelId, "sequencerDashboard")
    }

    function activateZoneForView(channelId, view) {
        if (!zoneState.activateZone(channelId)) {
            return false
        }
        if (String(view || "") === "sequencerDashboard") {
            Qt.callLater(function () {
                root.model.selectView("sequencerDashboard")
            })
        }
        return true
    }

    function cancelPendingInspectionOpen() {
        if (root.model
                && root.model.pendingInspectionEntityRef !== undefined) {
            root.model.pendingInspectionEntityRef = null
        }
    }

    function discardSourceDraft() {
        if (detailLoader.detailItem) {
            detailLoader.detailItem.discardSourceDraft()
        }
        retainDetailForDraft = false
    }

    function targetCandidateRows() {
        const candidates = root.zoneState && Array.isArray(
            root.zoneState.targetResolutionCandidates)
            ? root.zoneState.targetResolutionCandidates : []
        return candidates.map(function (candidate) {
            const entity = candidate && candidate.entity_ref ? candidate.entity_ref : ({})
            const source = entity.source || ({})
            const sourceText = String(source.kind || "") === "exact"
                ? String(source.source_role || "") + " / " + String(source.source_id || "")
                : String(source.kind || "-")
            const canonicalKey = String(entity.canonical_key || entity.channel_id || "")
            return {
                cells: [
                    { text: String(entity.layer || "-").toUpperCase(), width: 72, monospace: false },
                    { text: Presentation.words(entity.entity_kind || "zone"), width: 120, monospace: false },
                    { text: canonicalKey, width: 320, fill: true, link: true, copyText: canonicalKey },
                    { text: sourceText, width: 220, fill: true, monospace: false }
                ],
                candidate: candidate
            }
        })
    }

    function catalogSubtitle() {
        if (!zoneState) {
            return ""
        }
        return qsTr("Source revision %1 / catalog revision %2")
            .arg(Presentation.numberText(zoneState.sourceRevision))
            .arg(Presentation.numberText(zoneState.catalogRevision))
    }
}
