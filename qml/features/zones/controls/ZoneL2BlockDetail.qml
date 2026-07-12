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
    readonly property var detail: root.zoneState.l2BlockDetail
    readonly property var report: root.zoneState.l2BlockDetailReport
    readonly property var appModel: root.zoneState.appModel || null
    readonly property var entityRef: typeof root.zoneState.l2BlockEntityRef === "function"
        ? root.zoneState.l2BlockEntityRef(root.detail) : null
    readonly property var favoriteEntry: root.appModel && root.entityRef
        ? root.appModel.favoriteStore.l2EntityEntry(root.entityRef,
            qsTr("L2 Block %1").arg(Presentation.numberText(
                root.detail && root.detail.summary.block_id)),
            String(root.detail && root.detail.summary.block_hash || "")) : null

    signal backRequested()
    signal transactionRequested(string transactionId, string exactSourceId)
    signal configureSourcesRequested()

    objectName: "zoneL2BlockDetail"
    spacing: root.theme.gapLarge
    Layout.fillWidth: true

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        ActionButton {
            objectName: "zoneL2BlockBackButton"
            theme: root.theme
            iconOnly: true
            iconName: "back"
            accessibleName: qsTr("Back to L2 blocks")
            Layout.preferredWidth: root.theme.controlHeight
            onClicked: root.backRequested()
        }

        ColumnLayout {
            spacing: root.theme.gapTiny
            Layout.fillWidth: true

            Text {
                text: root.detail
                    ? qsTr("L2 Block %1").arg(Presentation.numberText(root.detail.summary.block_id))
                    : qsTr("L2 Block")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: root.theme.panelTitleText
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            Text {
                visible: root.detail === null
                text: root.targetText()
                color: root.theme.textMuted
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
                font.family: "monospace"
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }
        }

        ZoneKindChip {
            visible: root.detail !== null
            theme: root.theme
            label: Presentation.words(root.detail && root.detail.source.finality)
            tone: root.detail && root.detail.source.finality === "finalized" ? "success" : "warning"
        }

        ActionButton {
            visible: root.favoriteEntry !== null
            theme: root.theme
            text: root.favoriteEntry && root.appModel.favoriteStore.isFavoriteEntry(
                root.favoriteEntry) ? qsTr("Favorited") : qsTr("Favorite")
            selected: root.favoriteEntry && root.appModel.favoriteStore.isFavoriteEntry(
                root.favoriteEntry)
            Layout.preferredWidth: 112
            onClicked: root.appModel.favoriteStore.toggle(root.favoriteEntry)
        }
    }

    StatusMessage {
        visible: root.zoneState.l2BlockDetailInFlight
        theme: root.theme
        tone: "info"
        title: qsTr("Loading block evidence")
        message: qsTr("Resolving this block against the captured Active Zone context.")
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.zoneState.l2BlockDetailError.length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Block detail unavailable")
        message: root.zoneState.l2BlockDetailError
        Layout.fillWidth: true
    }

    RowLayout {
        visible: root.zoneState.l2BlockDetailError.length > 0
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        ActionButton {
            theme: root.theme
            text: root.recoveryButtonText()
            enabled: !root.zoneState.l2BlockDetailInFlight
            Layout.preferredWidth: 150
            onClicked: root.recover()
        }
    }

    ColumnLayout {
        visible: root.zoneState.l2BlockCandidates.length > 0
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        StatusMessage {
            theme: root.theme
            tone: "warning"
            title: qsTr("Block ID is ambiguous")
            message: qsTr("Choose exact source evidence. No source-order preference was applied.")
            Layout.fillWidth: true
        }

        DataTableFrame {
            objectName: "zoneL2BlockCandidatesTable"
            theme: root.theme
            headerCells: [
                { text: qsTr("Source ID"), width: 250, fill: true },
                { text: qsTr("Role"), width: 110 },
                { text: qsTr("Canonical block"), width: 250, fill: true }
            ]
            rows: root.candidateRows()
            Layout.fillWidth: true
            onCellActivated: function (row, column, cell, rowData) {
                root.zoneState.resolveL2BlockCandidate(rowData.candidate)
            }
        }
    }

    GridLayout {
        visible: root.detail !== null
        columns: width < 620 ? 1 : 2
        columnSpacing: root.theme.gapXLarge
        rowSpacing: root.theme.gapLarge
        Layout.fillWidth: true

        ZoneFactSection {
            theme: root.theme
            title: qsTr("Block Identity")
            rows: root.identityRows()
        }

        ZoneFactSection {
            theme: root.theme
            title: qsTr("Block State")
            rows: root.stateRows()
        }
    }

    ZoneL2Provenance {
        visible: root.detail !== null
        theme: root.theme
        source: root.detail ? root.detail.source : null
        route: root.report ? root.report.route : null
        routeCompleteness: root.report ? String(root.report.route_completeness || "") : ""
        Layout.fillWidth: true
    }

    Loader {
        active: root.appModel !== null && root.detail !== null
            && root.commentTopic().length > 0
        asynchronous: false
        Layout.fillWidth: true
        sourceComponent: SocialPanel {
            theme: root.theme
            model: root.appModel
            topic: root.commentTopic()
            entityRef: root.entityRef
            title: qsTr("Block comments")
        }
    }

    StatusMessage {
        visible: root.detail !== null && root.commentTopic().length === 0
            && root.entityRef !== null
        theme: root.theme
        tone: "info"
        title: qsTr("Collaboration unavailable")
        message: qsTr("Zone collaboration requires a verified genesis network identity.")
        Layout.fillWidth: true
    }

    ColumnLayout {
        visible: root.detail !== null
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        Text {
            text: qsTr("Transactions (%1)").arg(root.transactionRows().length)
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: root.theme.secondaryText
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        DataTableFrame {
            objectName: "zoneL2BlockTransactionsTable"
            theme: root.theme
            headerCells: [
                { text: qsTr("Index"), width: 68 },
                { text: qsTr("Transaction hash"), width: 220, fill: true },
                { text: qsTr("Kind"), width: 150 },
                { text: qsTr("Program"), width: 200, fill: true }
            ]
            rows: root.transactionTableRows()
            Layout.fillWidth: true
            onCellActivated: function (row, column, cell, rowData) {
                if (rowData.transactionId.length > 0 && (column === 1 || column === 2)) {
                    root.transactionRequested(rowData.transactionId, root.sourceId())
                }
            }
        }
    }

    function identityRows() {
        const summary = root.detail ? root.detail.summary : ({})
        return [{
            label: qsTr("Block hash"),
            value: Presentation.text(summary.block_hash),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Parent hash"),
            value: Presentation.text(summary.parent_hash),
            copyable: true,
            monospace: true
        }]
    }

    function stateRows() {
        const summary = root.detail ? root.detail.summary : ({})
        return [{
            label: qsTr("Timestamp"),
            value: Presentation.numberText(summary.timestamp)
        }, {
            label: qsTr("Bedrock status"),
            value: Presentation.text(summary.bedrock_status)
        }, {
            label: qsTr("Transactions"),
            value: Presentation.numberText(summary.transaction_count)
        }]
    }

    function candidateRows() {
        const rows = Array.isArray(root.zoneState.l2BlockCandidates)
            ? root.zoneState.l2BlockCandidates : []
        return rows.map(function (candidate) {
            const sourceId = String(candidate && candidate.source_id || "")
            const key = String(candidate && candidate.canonical_key || "")
            return {
                cells: [
                    { text: sourceId, width: 250, fill: true, link: sourceId.length > 0, copyText: sourceId },
                    { text: Presentation.words(candidate && candidate.source_role), width: 110, monospace: false },
                    { text: key, width: 250, fill: true, link: sourceId.length > 0, copyText: key }
                ],
                candidate: candidate
            }
        })
    }

    function transactionRows() {
        return root.detail && Array.isArray(root.detail.transactions)
            ? root.detail.transactions : []
    }

    function commentTopic() {
        return root.appModel && root.entityRef
            && typeof root.appModel.socialZoneCommentTopic === "function"
            ? root.appModel.socialZoneCommentTopic(root.entityRef) : ""
    }

    function transactionTableRows() {
        const rows = root.transactionRows()
        if (!rows.length) {
            return [{
                cells: [
                    { text: "-", width: 68 },
                    { text: qsTr("No transactions"), width: 220, fill: true, monospace: false },
                    { text: "-", width: 150 },
                    { text: "-", width: 200, fill: true }
                ],
                transactionId: ""
            }]
        }
        return rows.map(function (transaction, index) {
            const hash = String(transaction && transaction.hash || "")
            const program = String(transaction && transaction.program_id_hex || "")
            return {
                cells: [
                    { text: Presentation.numberText(index), width: 68 },
                    { text: root.shortHash(hash), width: 220, fill: true, link: hash.length > 0, copyText: hash },
                    { text: Presentation.text(transaction && transaction.kind), width: 150, link: hash.length > 0, monospace: false },
                    { text: root.shortHash(program), width: 200, fill: true, copyable: program.length > 0, copyText: program }
                ],
                transactionId: hash
            }
        })
    }

    function sourceId() {
        return String(root.detail && root.detail.source && root.detail.source.source_id || "")
    }

    function targetText() {
        const target = root.zoneState.l2BlockTarget || ({})
        return String(target.block_hash || target.block_id || "")
    }

    function recoveryButtonText() {
        const recovery = String(root.zoneState.l2BlockDetailErrorDetails
            && root.zoneState.l2BlockDetailErrorDetails.recovery || "retry")
        return recovery === "configure_source" || recovery === "select_source"
            ? qsTr("Open Sources") : qsTr("Retry")
    }

    function recover() {
        const recovery = String(root.zoneState.l2BlockDetailErrorDetails
            && root.zoneState.l2BlockDetailErrorDetails.recovery || "retry")
        if (recovery === "configure_source" || recovery === "select_source") {
            root.configureSourcesRequested()
            return
        }
        root.zoneState.openL2Block(root.zoneState.l2BlockTarget,
            root.zoneState.l2BlockRequestedSourceId)
    }

    function shortHash(value) {
        const text = String(value || "")
        return text.length > 20 ? text.slice(0, 10) + "..." + text.slice(-8)
            : Presentation.text(text)
    }
}
