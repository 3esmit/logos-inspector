pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../components"
import "../../../components/common"
import "../../../theme"
import "../../zones/ZonePresentation.js" as Presentation

Frame {
    id: root

    required property Theme theme
    required property var model
    readonly property var zoneState: root.model && root.model.zoneInspection
        ? root.model.zoneInspection : null
    readonly property var allZones: root.zoneState
        && Array.isArray(root.zoneState.zoneSummaries)
        ? root.zoneState.zoneSummaries : []
    readonly property var zones: root.allZones.slice(0, 8)
    readonly property bool stale: !root.zoneState
        || String(root.zoneState.verification || "") !== "verified"
        || root.zoneState.summaryStale
    readonly property int sequencerCount: root.countKind("sequencer_zone")
    readonly property int dataCount: root.countKind("data_channel")

    objectName: "dashboardZonesPanel"
    padding: 0
    Layout.fillWidth: true

    background: Rectangle {
        color: root.theme.surface
        radius: root.theme.radius
        border.width: 1
        border.color: root.theme.outlineMuted
    }

    contentItem: ColumnLayout {
        spacing: 0

        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: 48

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 14
                anchors.rightMargin: 14
                spacing: root.theme.gap

                ColumnLayout {
                    spacing: 1
                    Layout.fillWidth: true

                    Text {
                        objectName: "dashboardZonesTitle"
                        text: qsTr("Zones")
                        color: root.theme.text
                        textFormat: Text.PlainText
                        font.pixelSize: 15
                        font.weight: Font.DemiBold
                        Layout.fillWidth: true
                    }

                    Text {
                        objectName: "dashboardZonesCount"
                        text: qsTr("%1 Sequencer / %2 data / %3 total")
                            .arg(Presentation.numberText(root.sequencerCount))
                            .arg(Presentation.numberText(root.dataCount))
                            .arg(Presentation.numberText(root.allZones.length))
                        color: root.theme.textDim
                        textFormat: Text.PlainText
                        font.pixelSize: root.theme.labelText
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                }

                ActionButton {
                    objectName: "dashboardZonesViewAll"
                    theme: root.theme
                    text: qsTr("View all")
                    accessibleName: qsTr("View all Zones")
                    Layout.preferredWidth: 96
                    onClicked: root.openZones()
                }
            }
        }

        Text {
            visible: root.stale && root.zones.length > 0
            text: qsTr("Cached catalog / verification required")
            color: root.theme.warning
            textFormat: Text.PlainText
            font.pixelSize: root.theme.dataText
            leftPadding: 14
            rightPadding: 14
            bottomPadding: root.theme.gapSmall
            Layout.fillWidth: true
        }

        DataTableRow {
            visible: root.zones.length > 0
            theme: root.theme
            header: true
            headerHeight: 34
            cells: [
                { text: qsTr("Zone"), width: 142, fill: true },
                { text: qsTr("Status"), width: 72 },
                { text: qsTr("Source"), width: 72 },
                { text: qsTr("Finality"), width: 76 }
            ]
        }

        Repeater {
            model: root.zones

            DataTableRow {
                required property var modelData

                objectName: "dashboardZoneRow_"
                    + String(modelData && modelData.channel_id || "")
                theme: root.theme
                rowHeight: 42
                cells: root.zoneCells(modelData)
                onCellActivated: function (column) {
                    if (column === 0) {
                        root.openZone(String(modelData && modelData.channel_id || ""))
                    }
                }
            }
        }

        Item {
            visible: root.zones.length === 0
            Layout.fillWidth: true
            Layout.preferredHeight: 92

            Text {
                anchors.fill: parent
                anchors.leftMargin: 14
                anchors.rightMargin: 14
                text: root.emptyText()
                color: root.zoneState && root.zoneState.summaryError.length > 0
                    ? root.theme.error : root.theme.textMuted
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                verticalAlignment: Text.AlignVCenter
                font.pixelSize: root.theme.dataText
            }
        }

        Text {
            visible: root.allZones.length > root.zones.length
            text: qsTr("%1 more Zones available")
                .arg(Presentation.numberText(root.allZones.length - root.zones.length))
            color: root.theme.textDim
            textFormat: Text.PlainText
            font.pixelSize: root.theme.labelText
            horizontalAlignment: Text.AlignRight
            leftPadding: 14
            rightPadding: 14
            topPadding: root.theme.gapSmall
            bottomPadding: root.theme.gapSmall
            Layout.fillWidth: true
        }
    }

    function countKind(kind) {
        let count = 0
        for (let index = 0; index < root.allZones.length; ++index) {
            if (String(root.allZones[index] && root.allZones[index].kind || "") === kind) {
                count += 1
            }
        }
        return count
    }

    function shortChannelId(zone) {
        const configured = String(zone && zone.display
            && zone.display.short_channel_id || "")
        if (configured.length > 0) {
            return configured
        }
        const channelId = String(zone && zone.channel_id || "")
        return channelId.length > 18
            ? channelId.slice(0, 8) + "..." + channelId.slice(-6)
            : channelId
    }

    function zoneLabel(zone) {
        const title = Presentation.title(zone)
        const channelId = root.shortChannelId(zone)
        return channelId.length > 0
            ? qsTr("%1 / %2").arg(title).arg(channelId)
            : title
    }

    function sourceText(zone) {
        if (String(zone && zone.kind || "") === "sequencer_zone") {
            return Presentation.words(zone && zone.l2_zone
                && zone.l2_zone.source_status)
        }
        return Presentation.words(zone && zone.settlement_link
            && zone.settlement_link.source)
    }

    function zoneFinalityTone(zone) {
        if (String(zone && zone.kind || "") === "sequencer_zone"
                && zone.l2_zone
                && String(zone.l2_zone.finality_state || "unknown") !== "unknown") {
            return Presentation.finalityTone(zone.l2_zone.finality_state)
        }
        if (String(zone && zone.kind || "") === "data_channel" && zone.raw_activity) {
            return Presentation.finalityTone(zone.raw_activity.finality_state)
        }
        return Presentation.finalityTone(zone && zone.l1_channel
            && zone.l1_channel.finality_state)
    }

    function zoneCells(zone) {
        const channelId = String(zone && zone.channel_id || "")
        const actionable = !root.stale && channelId.length > 0
        return [
            {
                text: root.zoneLabel(zone),
                width: 142,
                fill: true,
                link: actionable,
                copyText: channelId,
                accessibleName: root.zoneActionAccessibleName(
                    zone, actionable),
                copyAccessibleName: qsTr("Copy Zone channel ID %1").arg(channelId),
                monospace: false
            },
            {
                text: Presentation.words(zone && zone.activity_state),
                width: 72,
                tone: Presentation.stateTone(zone, root.stale),
                monospace: false
            },
            {
                text: root.sourceText(zone),
                width: 72,
                monospace: false
            },
            {
                text: Presentation.zoneFinality(zone),
                width: 76,
                tone: root.zoneFinalityTone(zone),
                monospace: false
            }
        ]
    }

    function zoneById(channelId) {
        const target = String(channelId || "")
        for (let index = 0; index < root.allZones.length; ++index) {
            const zone = root.allZones[index] || ({})
            if (String(zone.channel_id || "") === target) {
                return zone
            }
        }
        return null
    }

    function sequencerConfigured(zone) {
        const fields = zone && zone.active_zone_context_fields
            ? zone.active_zone_context_fields : ({})
        const link = zone && zone.settlement_link ? zone.settlement_link : ({})
        return String(zone && zone.kind || "") === "sequencer_zone"
            && String(fields.selected_sequencer_source_id
                || link.selected_sequencer_source_id || "").length > 0
    }

    function zoneActionAccessibleName(zone, actionable) {
        const channelId = String(zone && zone.channel_id || "")
        if (!actionable) {
            return root.zoneLabel(zone)
        }
        return root.sequencerConfigured(zone)
            ? qsTr("Open Sequencer dashboard for Zone %1").arg(channelId)
            : qsTr("Open Zone %1").arg(channelId)
    }

    function emptyText() {
        if (!root.zoneState) {
            return qsTr("Zone catalog is unavailable.")
        }
        if (root.zoneState.summaryInFlight) {
            return qsTr("Loading Zone catalog...")
        }
        if (String(root.zoneState.summaryError || "").length > 0) {
            return String(root.zoneState.summaryError)
        }
        return qsTr("No Zones discovered. Open Zones to inspect catalog status.")
    }

    function openZones() {
        if (!root.model || typeof root.model.selectView !== "function") {
            return false
        }
        root.model.selectView("zones")
        return true
    }

    function openZone(channelId) {
        const target = String(channelId || "")
        if (root.stale || target.length === 0 || !root.zoneState
                || typeof root.zoneState.activateZone !== "function"
                || !root.zoneState.activateZone(target)) {
            return false
        }
        const zone = root.zoneById(target)
        root.model.selectView(root.sequencerConfigured(zone)
            ? "sequencerDashboard" : "zones")
        return true
    }
}
