pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../components/common"
import "../../../theme"
import "../ZonePresentation.js" as Presentation

ColumnLayout {
    id: root

    required property Theme theme
    required property var zoneState
    property var selectedRecipient: null

    signal configureSourcesRequested()
    signal transactionRequested(string transactionId, string exactSourceId)

    objectName: "zoneL2Transfers"
    spacing: root.theme.gapLarge
    Layout.fillWidth: true

    Component.onCompleted: root.ensureLoaded()

    Connections {
        target: root.zoneState

        function onActiveZoneContextChanged() {
            root.selectedRecipient = null
            Qt.callLater(root.ensureLoaded)
        }

        function onL2TransferRecipientsChanged() {
            root.selectedRecipient = null
        }
    }

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        ColumnLayout {
            spacing: root.theme.gapTiny
            Layout.fillWidth: true

            Text {
                text: qsTr("L2 Transfers")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: root.theme.panelTitleText
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            Text {
                text: root.windowLabel()
                color: root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }
        }

        ZoneKindChip {
            visible: root.zoneState.l2TransfersLoaded
            theme: root.theme
            label: root.zoneState.l2TransfersFinalized
                ? qsTr("Finalized Window") : qsTr("Unverified")
            tone: root.zoneState.l2TransfersFinalized ? "success" : "warning"
        }
    }

    StatusMessage {
        visible: !root.zoneState.l2IndexerReadEnabled
        theme: root.theme
        tone: root.zoneState.l2Applicable ? "warning" : "info"
        title: root.zoneState.l2Applicable
            ? qsTr("Indexer source required") : qsTr("L2 not applicable")
        message: root.zoneState.l2Applicable
            ? qsTr("Configure an Indexer for finalized transfer windows.")
            : root.zoneState.l2AvailabilityMessage()
        Layout.fillWidth: true
    }

    ActionButton {
        visible: root.zoneState.l2Applicable && !root.zoneState.l2IndexerReadEnabled
        theme: root.theme
        text: qsTr("Open Sources")
        Layout.preferredWidth: 150
        onClicked: root.configureSourcesRequested()
    }

    StatusMessage {
        visible: root.zoneState.l2TransfersError.length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Transfer window unavailable")
        message: root.zoneState.l2TransfersError
        Layout.fillWidth: true
    }

    GridLayout {
        visible: root.zoneState.l2TransfersLoaded
        columns: width < 620 ? 1 : 2
        columnSpacing: root.theme.gapXLarge
        rowSpacing: root.theme.gapLarge
        Layout.fillWidth: true

        ZoneFactSection {
            theme: root.theme
            title: qsTr("Block Window")
            rows: root.windowRows()
        }

        ZoneFactSection {
            theme: root.theme
            title: qsTr("Window Summary")
            rows: root.summaryRows()
        }
    }

    PagedInspectionTable {
        objectName: "zoneL2TransfersTable"
        visible: root.zoneState.l2IndexerReadEnabled
        theme: root.theme
        loadCount: root.zoneState.l2TransfersLimit
        loadOptions: [10, 25, 50]
        rangeText: root.windowLabel()
        canGoNewer: root.zoneState.l2TransfersHistory.length > 0
        canGoOlder: root.zoneState.l2TransfersHasMore
        busy: root.zoneState.l2TransfersInFlight
        headerCells: [
            { text: qsTr("Recipient"), width: 300, fill: true },
            { text: qsTr("Received"), width: 110 },
            { text: qsTr("Tx"), width: 64 },
            { text: qsTr("Outputs"), width: 76 },
            { text: qsTr("Account refs"), width: 100 },
            { text: qsTr("Evidence"), width: 210 }
        ]
        rows: root.recipientRows()
        Layout.fillWidth: true
        onRefreshRequested: root.zoneState.refreshL2Transfers()
        onNewerRequested: root.zoneState.loadNewerL2Transfers()
        onOlderRequested: root.zoneState.loadOlderL2Transfers()
        onLoadCountSelected: function (count) {
            root.zoneState.setL2TransfersLimit(count)
        }
        onCellActivated: function (row, column, cell, rowData) {
            root.selectedRecipient = rowData.recipient
        }
    }

    Text {
        visible: root.zoneState.l2TransfersLoaded
            && root.zoneState.l2TransferRecipients.length === 0
            && root.zoneState.l2TransfersError.length === 0
        text: qsTr("No recipient evidence in this finalized block window")
        color: root.theme.textMuted
        textFormat: Text.PlainText
        font.pixelSize: root.theme.dataText
        Layout.fillWidth: true
    }

    ColumnLayout {
        visible: root.selectedRecipient !== null
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Text {
                text: qsTr("Recipient Evidence")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Close")
                Layout.preferredWidth: 90
                onClicked: root.selectedRecipient = null
            }
        }

        ZoneFactSection {
            theme: root.theme
            title: ""
            rows: root.selectedRecipientRows()
        }

        DataTableFrame {
            objectName: "zoneL2TransferEvidenceTable"
            theme: root.theme
            headerCells: [
                { text: qsTr("Block"), width: 90 },
                { text: qsTr("Transaction"), width: 280, fill: true },
                { text: qsTr("Evidence"), width: 150 },
                { text: qsTr("Value"), width: 120 }
            ]
            rows: root.transferEvidenceRows()
            Layout.fillWidth: true
            onCellActivated: function (row, column, cell, rowData) {
                if (column === 1 && rowData.transactionId.length > 0) {
                    root.transactionRequested(rowData.transactionId,
                        root.zoneState.l2IndexerSourceId())
                }
            }
        }
    }

    ZoneL2Provenance {
        visible: root.zoneState.l2TransfersReport !== null
        theme: root.theme
        source: null
        route: root.zoneState.l2TransfersReport
            ? root.zoneState.l2TransfersReport.route : null
        routeCompleteness: root.zoneState.l2TransfersReport
            ? String(root.zoneState.l2TransfersReport.route_completeness || "") : ""
        Layout.fillWidth: true
    }

    function ensureLoaded() {
        if (root.zoneState.l2IndexerReadEnabled
                && !root.zoneState.l2TransfersLoaded
                && !root.zoneState.l2TransfersInFlight) {
            root.zoneState.refreshL2Transfers()
        }
    }

    function windowRows() {
        return [{
            label: qsTr("Newest block"),
            value: Presentation.numberText(root.zoneState.l2TransfersNewestBlock)
        }, {
            label: qsTr("Oldest block"),
            value: Presentation.numberText(root.zoneState.l2TransfersOldestBlock)
        }, {
            label: qsTr("Scanned blocks"),
            value: Presentation.numberText(root.zoneState.l2TransfersScannedBlocks)
        }, {
            label: qsTr("Finality"),
            value: root.zoneState.l2TransfersFinalized ? qsTr("Finalized") : qsTr("Unknown"),
            tone: root.zoneState.l2TransfersFinalized ? "success" : "warning"
        }]
    }

    function summaryRows() {
        const rows = Array.isArray(root.zoneState.l2TransferRecipients)
            ? root.zoneState.l2TransferRecipients : []
        let transactions = 0
        let outputs = 0
        let references = 0
        for (let i = 0; i < rows.length; ++i) {
            transactions += Number(rows[i] && rows[i].txs || 0)
            outputs += Number(rows[i] && rows[i].outputs || 0)
            references += Number(rows[i] && rows[i].references || 0)
        }
        return [{
            label: qsTr("Recipients"),
            value: Presentation.numberText(rows.length)
        }, {
            label: qsTr("Recipient tx counts"),
            value: Presentation.numberText(transactions)
        }, {
            label: qsTr("Transfer outputs"),
            value: Presentation.numberText(outputs)
        }, {
            label: qsTr("Account references"),
            value: Presentation.numberText(references)
        }]
    }

    function recipientRows() {
        const rows = Array.isArray(root.zoneState.l2TransferRecipients)
            ? root.zoneState.l2TransferRecipients : []
        return rows.map(function (recipient) {
            const recipientId = String(recipient && recipient.recipient || "")
            return {
                cells: [
                    { text: recipientId, width: 300, fill: true, link: true, copyText: recipientId },
                    { text: Presentation.text(recipient && recipient.received), width: 110 },
                    { text: Presentation.numberText(recipient && recipient.txs), width: 64 },
                    { text: Presentation.numberText(recipient && recipient.outputs), width: 76 },
                    { text: Presentation.numberText(recipient && recipient.references), width: 100 },
                    { text: Presentation.words(recipient && recipient.source), width: 210, monospace: false }
                ],
                recipient: recipient
            }
        })
    }

    function selectedRecipientRows() {
        const recipient = root.selectedRecipient || ({})
        return [{
            label: qsTr("Recipient"),
            value: Presentation.text(recipient.recipient),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Account ref"),
            value: Presentation.text(recipient.account_ref),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Window received"),
            value: Presentation.text(recipient.received)
        }, {
            label: qsTr("Transactions"),
            value: Presentation.numberText(recipient.txs)
        }, {
            label: qsTr("Outputs"),
            value: Presentation.numberText(recipient.outputs)
        }, {
            label: qsTr("Account refs"),
            value: Presentation.numberText(recipient.references)
        }, {
            label: qsTr("Evidence"),
            value: Presentation.words(recipient.source)
        }]
    }

    function transferEvidenceRows() {
        const recipient = root.selectedRecipient || ({})
        const rows = Array.isArray(recipient.transfers) ? recipient.transfers : []
        return rows.map(function (transfer) {
            const transactionId = String(transfer && transfer.tx_hash || "")
            const hasValue = transfer && transfer.value !== undefined
                && transfer.value !== null
            return {
                cells: [
                    { text: Presentation.numberText(transfer && transfer.slot), width: 90 },
                    { text: transactionId, width: 280, fill: true, link: transactionId.length > 0, copyText: transactionId },
                    { text: hasValue ? qsTr("Transfer output") : qsTr("Account reference"), width: 150, monospace: false },
                    { text: hasValue ? String(transfer.value) : "-", width: 120 }
                ],
                transactionId: transactionId
            }
        })
    }

    function windowLabel() {
        if (!root.zoneState.l2TransfersLoaded) {
            return qsTr("Finalized Indexer window")
        }
        return qsTr("Blocks %1 to %2 / %3 scanned")
            .arg(Presentation.numberText(root.zoneState.l2TransfersNewestBlock))
            .arg(Presentation.numberText(root.zoneState.l2TransfersOldestBlock))
            .arg(Presentation.numberText(root.zoneState.l2TransfersScannedBlocks))
    }
}
