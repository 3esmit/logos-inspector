pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../theme"
import "../ZonePresentation.js" as Presentation

GridLayout {
    id: root

    required property Theme theme
    required property var detail
    readonly property var zone: root.detail && root.detail.summary ? root.detail.summary : ({})
    readonly property bool singleColumn: width < 620

    columns: root.singleColumn ? 1 : 2
    columnSpacing: root.theme.gapXLarge
    rowSpacing: root.theme.gapLarge
    Layout.fillWidth: true

    ZoneFactSection {
        theme: root.theme
        title: qsTr("Channel Details")
        rows: root.channelRows()
    }

    ZoneFactSection {
        theme: root.theme
        title: qsTr("L1 State")
        rows: root.l1StateRows()
    }

    ZoneFactSection {
        theme: root.theme
        title: qsTr("Settlement Link")
        rows: root.settlementRows()
    }

    ZoneFactSection {
        theme: root.theme
        title: String(root.zone.kind || "") === "data_channel"
            ? qsTr("Raw L1 Activity") : qsTr("L2 Sequencer")
        rows: root.zoneActivityRows()
    }

    ZoneFactSection {
        theme: root.theme
        title: qsTr("Catalog Evidence")
        rows: root.classificationRows()
        Layout.columnSpan: root.singleColumn ? 1 : 2
    }

    function channelRows() {
        const l1 = root.zone.l1_channel || ({})
        const snapshot = root.detail.l1_channel_snapshot || ({})
        return [{
            label: qsTr("L1 Channel"),
            value: Presentation.text(root.zone.channel_id),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Channel tip"),
            value: Presentation.text(snapshot.channel_tip || l1.tip_hash),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Tip slot"),
            value: Presentation.numberText(l1.tip_slot),
            tone: Presentation.finalityTone(l1.finality_state)
        }, {
            label: qsTr("Observed slot"),
            value: Presentation.numberText(snapshot.observed_at_slot)
        }]
    }

    function l1StateRows() {
        const l1 = root.zone.l1_channel || ({})
        return [{
            label: qsTr("Finality"),
            value: Presentation.words(l1.finality_state),
            tone: Presentation.finalityTone(l1.finality_state)
        }, {
            label: qsTr("Balance"),
            value: Presentation.text(l1.balance)
        }, {
            label: qsTr("Keys"),
            value: Presentation.numberText(l1.key_count)
        }, {
            label: qsTr("Withdraw threshold"),
            value: Presentation.text(l1.withdraw_threshold)
        }, {
            label: qsTr("Operations"),
            value: Presentation.numberText(l1.operation_count)
        }]
    }

    function settlementRows() {
        const link = root.zone.settlement_link || ({})
        return [{
            label: qsTr("Status"),
            value: Presentation.words(link.status),
            tone: link.status === "linked" ? "success"
                : (link.status === "raw_data" ? "info" : "warning")
        }, {
            label: qsTr("Evidence source"),
            value: Presentation.words(link.source)
        }, {
            label: qsTr("Sequencer source"),
            value: Presentation.text(link.selected_sequencer_source_id),
            monospace: true
        }, {
            label: qsTr("Indexer source"),
            value: Presentation.text(link.indexer_source_id),
            monospace: true
        }, {
            label: qsTr("Lag"),
            value: link.lag_blocks === undefined || link.lag_blocks === null
                ? qsTr("%1 L1 slots").arg(Presentation.numberText(link.lag_slots))
                : qsTr("%1 L2 blocks / %2 L1 slots")
                    .arg(Presentation.numberText(link.lag_blocks))
                    .arg(Presentation.numberText(link.lag_slots))
        }]
    }

    function zoneActivityRows() {
        if (String(root.zone.kind || "") === "data_channel") {
            const raw = root.zone.raw_activity || ({})
            return [{
                label: qsTr("State"),
                value: qsTr("Raw data"),
                tone: "info"
            }, {
                label: qsTr("Raw inscriptions"),
                value: Presentation.numberText(raw.inscription_count)
            }, {
                label: qsTr("Latest L1 slot"),
                value: Presentation.numberText(raw.latest_slot)
            }, {
                label: qsTr("Latest payload"),
                value: raw.latest_payload_size === undefined || raw.latest_payload_size === null
                    ? "-" : qsTr("%1 bytes").arg(Presentation.numberText(raw.latest_payload_size))
            }, {
                label: qsTr("Finality"),
                value: Presentation.words(raw.finality_state),
                tone: Presentation.finalityTone(raw.finality_state)
            }]
        }
        const l2 = root.zone.l2_zone || ({})
        return [{
            label: qsTr("Source"),
            value: Presentation.words(l2.source_status),
            tone: Presentation.stateTone(root.zone, false)
        }, {
            label: qsTr("Head block"),
            value: Presentation.numberText(l2.latest_block_id)
        }, {
            label: qsTr("Head hash"),
            value: Presentation.text(l2.latest_block_hash),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Safe block"),
            value: Presentation.numberText(l2.safe_block_id),
            tone: "info"
        }, {
            label: qsTr("Finalized block"),
            value: Presentation.numberText(l2.finalized_block_id),
            tone: "success"
        }, {
            label: qsTr("Agreement"),
            value: Presentation.words(l2.agreement_state)
        }, {
            label: qsTr("Source error"),
            value: root.firstSourceError(),
            tone: root.firstSourceError() === "-" ? "neutral" : "error"
        }]
    }

    function classificationRows() {
        const evidence = root.detail.classification_evidence || ({})
        const counts = root.detail.activity_counts || ({})
        return [{
            label: qsTr("L1 operations"),
            value: Presentation.numberText(counts.l1_operations)
        }, {
            label: qsTr("Recognized L2 blocks"),
            value: Presentation.numberText(counts.recognized_l2_blocks)
        }, {
            label: qsTr("Raw inscriptions"),
            value: Presentation.numberText(counts.raw_inscriptions)
        }, {
            label: qsTr("Coverage proves L2 absence"),
            value: evidence.l2_absence_is_covered === true ? qsTr("Yes") : qsTr("No"),
            tone: evidence.l2_absence_is_covered === true ? "success" : "neutral"
        }, {
            label: qsTr("Conflicting evidence"),
            value: evidence.conflicting_evidence === true ? qsTr("Yes") : qsTr("No"),
            tone: evidence.conflicting_evidence === true ? "error" : "success"
        }]
    }

    function firstSourceError() {
        const observations = Array.isArray(root.detail.source_observations)
            ? root.detail.source_observations : []
        for (let i = 0; i < observations.length; ++i) {
            if (String(observations[i] && observations[i].last_error || "").length > 0) {
                return String(observations[i].last_error)
            }
        }
        return "-"
    }
}
