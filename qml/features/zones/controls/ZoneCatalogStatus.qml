pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../components"
import "../../../theme"
import "../ZonePresentation.js" as Presentation

Rectangle {
    id: root

    required property Theme theme
    required property var zoneState
    readonly property bool compactGrid: width < 680
    readonly property string tone: Presentation.catalogTone(
        root.zoneState && root.zoneState.verification,
        root.zoneState && root.zoneState.coverage
    )
    readonly property var readiness: root.zoneState && root.zoneState.readiness || null
    readonly property bool waitingForBedrock: root.readiness
        && String(root.readiness.phase || "") === "waiting_for_bedrock"
    readonly property string statusOrConfigureError: String(root.zoneState
        && (root.zoneState.statusError || root.zoneState.configureError) || "")
    readonly property string catalogErrorText: root.statusOrConfigureError.length > 0
        ? root.statusOrConfigureError
        : String(root.zoneState && root.zoneState.currentError || "")
    readonly property var facts: root.statusFacts()

    objectName: "zoneCatalogStatus"
    implicitHeight: statusGrid.implicitHeight + root.theme.gapLarge
        + (bedrockReadiness.visible ? bedrockReadiness.implicitHeight + root.theme.gapSmall : 0)
        + (catalogError.visible ? catalogError.implicitHeight + root.theme.gapSmall : 0)
    radius: root.theme.radius
    color: root.theme.surface
    border.width: 1
    border.color: root.theme.outlineMuted

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: root.theme.gapSmall
        spacing: root.theme.gapSmall

        StatusMessage {
            id: bedrockReadiness

            objectName: "zoneCatalogBedrockReadiness"
            visible: root.waitingForBedrock
            theme: root.theme
            tone: "warning"
            title: qsTr("Bedrock synchronization in progress")
            message: root.bedrockReadinessMessage()
            Layout.fillWidth: true
        }

        GridLayout {
            id: statusGrid

            columns: root.compactGrid ? 2 : 4
            columnSpacing: 0
            rowSpacing: root.theme.gapSmall
            Layout.fillWidth: true

            Repeater {
                model: root.facts

                Item {
                    id: factItem

                    required property var modelData
                    required property int index

                    objectName: "zoneCatalogFact_" + factItem.index
                    implicitHeight: Math.max(40, factRow.implicitHeight)
                    activeFocusOnTab: factLabel.truncated || factValue.truncated
                    Layout.fillWidth: true
                    Layout.minimumWidth: 110

                    Accessible.role: Accessible.StaticText
                    Accessible.name: root.factAccessibleName(factItem.modelData)

                    Rectangle {
                        visible: factItem.index % statusGrid.columns !== 0
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.bottom: parent.bottom
                        width: 1
                        color: root.theme.outlineMuted
                        Accessible.ignored: true
                    }

                    RowLayout {
                        id: factRow

                        anchors.fill: parent
                        anchors.leftMargin: factItem.index % statusGrid.columns === 0 ? root.theme.gapTiny : root.theme.gap
                        anchors.rightMargin: root.theme.gap
                        spacing: root.theme.gapSmall

                        ToneDot {
                            theme: root.theme
                            tone: factItem.modelData.tone
                            Layout.preferredWidth: 7
                            Layout.preferredHeight: 7
                            Layout.alignment: Qt.AlignTop
                            Layout.topMargin: 5
                            Accessible.ignored: true
                        }

                        ColumnLayout {
                            spacing: 0
                            Layout.fillWidth: true

                            Text {
                                id: factLabel

                                text: factItem.modelData.label
                                color: root.theme.textDim
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                font.pixelSize: root.theme.labelText
                                Layout.fillWidth: true
                                Accessible.ignored: true
                            }

                            Text {
                                id: factValue

                                objectName: "zoneCatalogFactValue_" + factItem.index
                                text: factItem.modelData.value
                                color: root.theme.text
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                wrapMode: Text.Wrap
                                maximumLineCount: 2
                                font.pixelSize: root.theme.secondaryText
                                font.weight: Font.DemiBold
                                Layout.fillWidth: true
                                Accessible.ignored: true
                            }
                        }
                    }

                    HoverHandler {
                        id: factHover
                    }

                    ToolTip.visible: (factHover.hovered || factItem.activeFocus)
                        && (factLabel.truncated || factValue.truncated)
                    ToolTip.delay: 350
                    ToolTip.text: root.factAccessibleName(factItem.modelData)
                }
            }
        }

        Text {
            id: catalogError

            objectName: "zoneCatalogError"
            visible: text.length > 0 && (!root.waitingForBedrock
                || root.statusOrConfigureError.length > 0)
            text: root.catalogErrorText
            color: root.theme.error
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: root.theme.dataText
            Layout.fillWidth: true
            Accessible.role: Accessible.StaticText
            Accessible.name: root.errorAccessibleName(text)
        }
    }

    function statusFacts() {
        const state = root.zoneState || ({})
        const coverage = state.coverage || ({})
        const ingestion = state.ingestion || ({})
        const readiness = root.readiness || ({})
        const floor = coverage.coverage_floor === undefined || coverage.coverage_floor === null
            ? "-" : Presentation.numberText(coverage.coverage_floor)
        const scanned = coverage.scanned_through_slot === undefined || coverage.scanned_through_slot === null
            ? "-" : Presentation.numberText(coverage.scanned_through_slot)
        const facts = [{
            label: qsTr("Catalog verification"),
            value: Presentation.words(state.verification),
            tone: root.tone
        }, {
            label: qsTr("Coverage"),
            value: qsTr("%1 / prefix %2")
                .arg(Presentation.words(coverage.status))
                .arg(Presentation.words(coverage.prefix_status)),
            tone: root.tone
        }, {
            label: qsTr("Finalized range"),
            value: qsTr("%1 - %2").arg(floor).arg(scanned),
            tone: Number(coverage.gap_count || 0) > 0 ? "warning" : root.tone
        }, {
            label: qsTr("Catalog facts"),
            value: qsTr("%1 Zones / %2 gaps")
                .arg(Presentation.numberText(ingestion.discovered_zone_count))
                .arg(Presentation.numberText(coverage.gap_count)),
            tone: Number(coverage.gap_count || 0) > 0 ? "warning" : root.tone
        }]
        if (!root.waitingForBedrock) {
            return facts
        }
        const finalized = Presentation.numberText(readiness.finalized_lib_slot)
        const checkpoint = Presentation.numberText(readiness.required_checkpoint_slot)
        const remaining = root.remainingSlots(readiness)
        facts.push({
            label: qsTr("Bedrock synchronization"),
            value: qsTr("Waiting for Bedrock"),
            tone: "warning"
        }, {
            label: qsTr("Finalized LIB"),
            value: qsTr("%1 / checkpoint %2").arg(finalized).arg(checkpoint),
            tone: "warning"
        }, {
            label: qsTr("Remaining until catalog"),
            value: qsTr("%1 slots").arg(Presentation.numberText(remaining)),
            tone: "warning"
        })
        return facts
    }

    function remainingSlots(readiness) {
        if (!readiness || readiness.finalized_lib_slot === undefined
                || readiness.finalized_lib_slot === null
                || readiness.required_checkpoint_slot === undefined
                || readiness.required_checkpoint_slot === null) {
            return null
        }
        const finalized = Number(readiness && readiness.finalized_lib_slot)
        const checkpoint = Number(readiness && readiness.required_checkpoint_slot)
        if (!Number.isFinite(finalized) || !Number.isFinite(checkpoint)
                || finalized < 0 || checkpoint < 0) {
            return null
        }
        return Math.max(0, checkpoint - finalized)
    }

    function bedrockReadinessMessage() {
        const readiness = root.readiness || ({})
        const finalized = Presentation.numberText(readiness.finalized_lib_slot)
        const checkpoint = Presentation.numberText(readiness.required_checkpoint_slot)
        const remaining = Presentation.numberText(root.remainingSlots(readiness))
        return qsTr("Finalized LIB %1 of Zone Catalog checkpoint %2. %3 slots remain. Zones resume automatically when this checkpoint is reached.")
            .arg(finalized)
            .arg(checkpoint)
            .arg(remaining)
    }

    function factAccessibleName(fact) {
        return qsTr("%1: %2")
            .arg(String(fact && fact.label || ""))
            .arg(String(fact && fact.value || ""))
    }

    function errorAccessibleName(errorText) {
        const normalized = String(errorText || "").replace(/\s+/g, " ").trim()
        const limit = 240
        if (normalized.length <= limit) {
            return normalized
        }
        return normalized.slice(0, limit - 3) + "..."
    }
}
