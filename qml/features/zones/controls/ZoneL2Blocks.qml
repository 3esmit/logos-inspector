pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../theme"
import "../ZonePresentation.js" as Presentation

ColumnLayout {
    id: root

    required property Theme theme
    required property var zoneState
    property string transactionQuery: ""

    signal blockRequested(var summary, string exactSourceId)
    signal transactionRequested(string transactionId, string exactSourceId)
    signal configureSourcesRequested()

    objectName: "zoneL2Blocks"
    spacing: root.theme.gapLarge
    Layout.fillWidth: true

    RowLayout {
        spacing: root.theme.gap
        Layout.fillWidth: true

        ColumnLayout {
            spacing: root.theme.gapTiny
            Layout.fillWidth: true

            Text {
                text: qsTr("L2 Blocks")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: root.theme.panelTitleText
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            Text {
                text: root.routeSummary()
                color: root.theme.textMuted
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }
        }

        ZoneKindChip {
            visible: root.zoneState.l2BlocksLoaded
            theme: root.theme
            label: root.conflictCount() > 0
                ? root.conflictLabel()
                : Presentation.words(root.zoneState.l2BlocksRouteCompleteness)
            tone: root.conflictCount() > 0 ? "warning"
                : (root.zoneState.l2BlocksRouteCompleteness === "degraded" ? "warning" : "success")
        }
    }

    StatusMessage {
        visible: root.conflictCount() > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Conflicting block observations")
        message: qsTr("Rows sharing a block ID retain separate hashes and source evidence.")
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.zoneState.l2BlocksWarnings.length > 0
            || root.zoneState.l2BlocksRouteCompleteness === "degraded"
        theme: root.theme
        tone: "warning"
        title: qsTr("Partial source route")
        message: root.warningText()
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.zoneState.l2BlocksError.length > 0
        theme: root.theme
        tone: root.zoneState.l2BlocksErrorDetails
            && root.zoneState.l2BlocksErrorDetails.recovery === "none" ? "error" : "warning"
        title: qsTr("L2 blocks unavailable")
        message: root.zoneState.l2BlocksError
        Layout.fillWidth: true
    }

    ActionButton {
        visible: root.zoneState.l2BlocksError.length > 0
        theme: root.theme
        text: root.recoveryButtonText(root.zoneState.l2BlocksErrorDetails)
        enabled: !root.zoneState.l2BlocksInFlight
        Layout.preferredWidth: 150
        onClicked: root.recover(root.zoneState.l2BlocksErrorDetails)
    }

    RowLayout {
        visible: root.zoneState.l2ReadEnabled
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        FieldRow {
            id: transactionField

            objectName: "zoneL2TransactionField"
            theme: root.theme
            label: qsTr("Transaction hash")
            placeholderText: qsTr("Enter exact L2 transaction hash")
            Layout.fillWidth: true
            onTextEdited: function (value) {
                root.transactionQuery = String(value || "").trim()
            }
        }

        ActionButton {
            objectName: "zoneL2TransactionInspectButton"
            theme: root.theme
            text: qsTr("Inspect")
            primary: true
            enabled: root.transactionQuery.length > 0
                && !root.zoneState.l2TransactionDetailInFlight
            Layout.preferredWidth: 104
            Layout.alignment: Qt.AlignBottom
            onClicked: root.transactionRequested(root.transactionQuery, "")
        }
    }

    PagedInspectionTable {
        objectName: "zoneL2BlocksTable"
        visible: root.zoneState.l2ReadEnabled
        theme: root.theme
        loadCount: root.zoneState.l2BlocksLimit
        loadOptions: [10, 25, 50]
        rangeText: root.rangeText()
        canGoNewer: false
        canGoOlder: root.zoneState.l2BlocksHasMore
        busy: root.zoneState.l2BlocksInFlight
        headerCells: [
            { text: qsTr("Block ID"), width: 96 },
            { text: qsTr("Block hash"), width: 220, fill: true },
            { text: qsTr("Tx"), width: 64 },
            { text: qsTr("Finality"), width: 112 },
            { text: qsTr("Sources"), width: 150 }
        ]
        rows: root.blockRows()
        Layout.fillWidth: true
        onRefreshRequested: root.zoneState.refreshL2Blocks()
        onOlderRequested: root.zoneState.loadMoreL2Blocks()
        onLoadCountSelected: function (count) {
            root.zoneState.setL2BlocksLimit(count)
        }
        onCellActivated: function (row, column, cell, rowData) {
            if (rowData.summary && (column === 0 || column === 1 || column === 4)) {
                root.blockRequested(rowData.summary, rowData.exactSourceId)
            }
        }
    }

    Text {
        visible: root.zoneState.l2ReadEnabled && root.zoneState.l2BlocksLoaded
            && root.zoneState.l2BlockRows.length === 0
            && root.zoneState.l2BlocksError.length === 0
        text: qsTr("No L2 blocks returned for this Zone")
        color: root.theme.textMuted
        textFormat: Text.PlainText
        font.pixelSize: root.theme.dataText
        Layout.fillWidth: true
    }

    function blockRows() {
        const rows = Array.isArray(root.zoneState.l2BlockRows)
            ? root.zoneState.l2BlockRows : []
        const conflicts = root.conflictMap()
        return rows.map(function (row) {
            const summary = row && row.summary ? row.summary : ({})
            const observations = row && Array.isArray(row.observations)
                ? row.observations : []
            const hash = String(summary.block_hash || "")
            const conflict = conflicts[String(summary.block_id)] === true
            return {
                cells: [
                    { text: Presentation.numberText(summary.block_id), width: 96, link: true, tone: conflict ? "warning" : "neutral" },
                    { text: root.shortHash(hash), width: 220, fill: true, link: hash.length > 0, copyText: hash, tone: conflict ? "warning" : "neutral" },
                    { text: Presentation.numberText(summary.transaction_count), width: 64 },
                    { text: root.finalityText(observations), width: 112, monospace: false, tone: root.finalityTone(observations) },
                    { text: root.sourceText(observations), width: 150, link: observations.length > 0, monospace: false }
                ],
                summary: summary,
                exactSourceId: observations.length === 1
                    ? String(observations[0].source_id || "") : ""
            }
        })
    }

    function conflictMap() {
        const rows = Array.isArray(root.zoneState.l2BlockRows)
            ? root.zoneState.l2BlockRows : []
        const hashes = ({})
        const conflicts = ({})
        for (let i = 0; i < rows.length; ++i) {
            const summary = rows[i] && rows[i].summary ? rows[i].summary : ({})
            const key = String(summary.block_id)
            const hash = String(summary.block_hash || "")
            if (hashes[key] !== undefined && hashes[key] !== hash) {
                conflicts[key] = true
            } else if (hashes[key] === undefined) {
                hashes[key] = hash
            }
        }
        return conflicts
    }

    function conflictCount() {
        return Object.keys(root.conflictMap()).length
    }

    function conflictLabel() {
        const count = root.conflictCount()
        return count === 1 ? qsTr("1 conflict ID")
            : qsTr("%1 conflict IDs").arg(count)
    }

    function sourceText(observations) {
        if (!observations.length) {
            return "-"
        }
        const roles = []
        for (let i = 0; i < observations.length; ++i) {
            const role = Presentation.words(observations[i] && observations[i].source_role)
            if (roles.indexOf(role) < 0) {
                roles.push(role)
            }
        }
        return roles.join(qsTr(" + "))
    }

    function finalityText(observations) {
        let finalized = false
        let provisional = false
        for (let i = 0; i < observations.length; ++i) {
            finalized = finalized || observations[i].finality === "finalized"
            provisional = provisional || observations[i].finality === "provisional"
        }
        if (finalized && provisional) {
            return qsTr("Final + provisional")
        }
        return finalized ? qsTr("Finalized") : (provisional ? qsTr("Provisional") : "-")
    }

    function finalityTone(observations) {
        for (let i = 0; i < observations.length; ++i) {
            if (observations[i].finality === "finalized") {
                return "success"
            }
        }
        return observations.length > 0 ? "warning" : "neutral"
    }

    function routeSummary() {
        const route = root.zoneState.l2BlocksRoute || ({})
        if (!root.zoneState.l2BlocksLoaded && !root.zoneState.l2BlocksInFlight) {
            return qsTr("Active Zone source policy")
        }
        if (root.zoneState.l2BlocksInFlight) {
            return qsTr("Loading current source observations...")
        }
        return qsTr("%1 / %2 source heads")
            .arg(Presentation.words(route.policy))
            .arg(root.zoneState.l2BlocksSourceHeads.length)
    }

    function rangeText() {
        return qsTr("%1 block IDs / %2 evidence rows")
            .arg(Presentation.numberText(root.zoneState.l2BlocksDistinctCount))
            .arg(Presentation.numberText(root.zoneState.l2BlockRows.length))
    }

    function warningText() {
        const warnings = Array.isArray(root.zoneState.l2BlocksWarnings)
            ? root.zoneState.l2BlocksWarnings : []
        if (!warnings.length) {
            return qsTr("One configured source did not contribute to this page.")
        }
        return warnings.map(function (warning) {
            return String(warning && warning.message || "")
        }).filter(function (message) {
            return message.length > 0
        }).join("\n")
    }

    function recoveryButtonText(details) {
        const recovery = String(details && details.recovery || "retry")
        if (recovery === "configure_source" || recovery === "select_source") {
            return qsTr("Open Sources")
        }
        return qsTr("Retry")
    }

    function recover(details) {
        const recovery = String(details && details.recovery || "retry")
        if (recovery === "configure_source" || recovery === "select_source") {
            root.configureSourcesRequested()
        } else {
            root.zoneState.refreshL2Blocks()
        }
    }

    function shortHash(value) {
        const text = String(value || "")
        return text.length > 20 ? text.slice(0, 10) + "..." + text.slice(-8) : Presentation.text(text)
    }
}
