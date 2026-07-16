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
    property string accountQuery: String(root.zoneState.l2AccountId || "")
    property string historicalBlockId: ""
    property string historicalBlockHash: ""
    readonly property var appModel: root.zoneState.appModel || null
    readonly property var entityRef: typeof root.zoneState.l2AccountEntityRef === "function"
        ? root.zoneState.l2AccountEntityRef(root.zoneState.l2AccountFinalized
            || root.zoneState.l2AccountProvisional) : null
    readonly property var favoriteEntry: root.appModel && root.entityRef
        ? root.appModel.favoriteStore.l2EntityEntry(root.entityRef,
            qsTr("L2 Account %1").arg(String(root.zoneState.l2AccountId).slice(0, 12)),
            String(root.entityRef.channel_id || "")) : null

    signal configureSourcesRequested()
    signal transactionRequested(string transactionId, string exactSourceId)

    objectName: "zoneL2Accounts"
    spacing: root.theme.gapLarge
    Layout.fillWidth: true

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        ColumnLayout {
            spacing: root.theme.gapTiny
            Layout.fillWidth: true

            Text {
                text: qsTr("L2 Accounts")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: root.theme.panelTitleText
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            Text {
                text: root.zoneState.l2AccountId.length > 0
                    ? root.zoneState.l2AccountId : qsTr("Finalized and provisional snapshots")
                color: root.theme.textMuted
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
                font.family: root.zoneState.l2AccountId.length > 0 ? "monospace" : "sans-serif"
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }
        }

        ActionButton {
            visible: root.zoneState.l2AccountId.length > 0
            theme: root.theme
            text: qsTr("Refresh snapshots")
            enabled: !root.zoneState.l2AccountFinalizedInFlight
                && !root.zoneState.l2AccountProvisionalInFlight
            Layout.preferredWidth: 160
            onClicked: root.zoneState.refreshL2AccountSnapshots()
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
        visible: !root.zoneState.l2ReadEnabled
        theme: root.theme
        tone: root.zoneState.l2Applicable ? "warning" : "info"
        title: root.zoneState.l2Applicable
            ? qsTr("L2 source required") : qsTr("L2 not applicable")
        message: root.zoneState.l2AvailabilityMessage()
        Layout.fillWidth: true
    }

    ActionButton {
        visible: root.zoneState.l2Applicable && !root.zoneState.l2SourceConfigured
        theme: root.theme
        text: qsTr("Open Sources")
        Layout.preferredWidth: 150
        onClicked: root.configureSourcesRequested()
    }

    GridLayout {
        visible: root.zoneState.l2ReadEnabled
        columns: width < 620 ? 1 : 2
        columnSpacing: root.theme.gapSmall
        rowSpacing: root.theme.gapSmall
        Layout.fillWidth: true

        FieldRow {
            id: accountField

            objectName: "zoneL2AccountField"
            theme: root.theme
            label: qsTr("Account ID")
            placeholderText: qsTr("Base58 or hex account ID")
            sourceText: root.accountQuery
            syncSourceText: true
            Layout.fillWidth: true
            onTextEdited: function (value) {
                root.accountQuery = String(value || "").trim()
            }
        }

        ActionButton {
            objectName: "zoneL2AccountInspectButton"
            theme: root.theme
            text: qsTr("Inspect")
            primary: true
            enabled: root.accountQuery.length > 0
                && !root.zoneState.l2AccountFinalizedInFlight
                && !root.zoneState.l2AccountProvisionalInFlight
            Layout.preferredWidth: 110
            Layout.alignment: Qt.AlignBottom | Qt.AlignLeft
            onClicked: root.zoneState.inspectL2Account(root.accountQuery)
        }
    }

    Loader {
        active: root.appModel !== null && root.entityRef !== null
            && root.commentTopic().length > 0
        asynchronous: false
        Layout.fillWidth: true
        sourceComponent: SocialPanel {
            theme: root.theme
            model: root.appModel
            topic: root.commentTopic()
            entityRef: root.entityRef
            expectedAccountId: root.zoneState.l2AccountId
            title: qsTr("Account comments")
        }
    }

    StatusMessage {
        visible: root.entityRef !== null && root.commentTopic().length === 0
        theme: root.theme
        tone: "info"
        title: qsTr("Collaboration unavailable")
        message: qsTr("Zone collaboration requires a verified genesis network identity. Local account inspection remains available.")
        Layout.fillWidth: true
    }

    GridLayout {
        visible: root.zoneState.l2AccountId.length > 0
        columns: width < 720 ? 1 : 2
        columnSpacing: root.theme.gapXLarge
        rowSpacing: root.theme.gapXLarge
        Layout.fillWidth: true

        ZoneL2AccountSnapshot {
            objectName: "zoneL2FinalizedAccountSnapshot"
            theme: root.theme
            title: qsTr("Finalized Snapshot")
            snapshot: root.zoneState.l2AccountFinalized
            report: root.zoneState.l2AccountFinalizedReport
            error: root.zoneState.l2AccountFinalizedError
            busy: root.zoneState.l2AccountFinalizedInFlight
            decode: root.zoneState.l2AccountFinalizedDecode
            decodeError: root.zoneState.l2AccountFinalizedDecodeError
            decodeInFlight: root.zoneState.l2AccountFinalizedDecodeInFlight
        }

        ZoneL2AccountSnapshot {
            objectName: "zoneL2ProvisionalAccountSnapshot"
            theme: root.theme
            title: qsTr("Provisional Snapshot")
            snapshot: root.zoneState.l2AccountProvisional
            report: root.zoneState.l2AccountProvisionalReport
            error: root.zoneState.l2AccountProvisionalError
            busy: root.zoneState.l2AccountProvisionalInFlight
            decode: root.zoneState.l2AccountProvisionalDecode
            decodeError: root.zoneState.l2AccountProvisionalDecodeError
            decodeInFlight: root.zoneState.l2AccountProvisionalDecodeInFlight
        }
    }

    ColumnLayout {
        visible: root.zoneState.l2AccountId.length > 0
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        Text {
            text: qsTr("Historical Snapshot")
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: root.theme.secondaryText
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        GridLayout {
            columns: width < 720 ? 1 : 3
            columnSpacing: root.theme.gapSmall
            rowSpacing: root.theme.gapSmall
            Layout.fillWidth: true

            FieldRow {
                objectName: "zoneL2HistoricalBlockIdField"
                theme: root.theme
                label: qsTr("Block ID")
                placeholderText: qsTr("Exact block ID")
                Layout.preferredWidth: 160
                Layout.fillWidth: true
                onTextEdited: function (value) {
                    root.historicalBlockId = String(value || "").trim()
                }
            }

            FieldRow {
                objectName: "zoneL2HistoricalBlockHashField"
                theme: root.theme
                label: qsTr("Block hash")
                placeholderText: qsTr("Exact block hash")
                Layout.fillWidth: true
                onTextEdited: function (value) {
                    root.historicalBlockHash = String(value || "").trim()
                }
            }

            ActionButton {
                objectName: "zoneL2HistoricalInspectButton"
                theme: root.theme
                text: qsTr("Load snapshot")
                enabled: root.historicalBlockId.length > 0
                    && root.historicalBlockHash.length > 0
                    && !root.zoneState.l2AccountHistoricalInFlight
                Layout.preferredWidth: 140
                Layout.alignment: Qt.AlignBottom | Qt.AlignLeft
                onClicked: root.zoneState.requestL2HistoricalAccount(
                    Number(root.historicalBlockId), root.historicalBlockHash)
            }
        }

        ZoneL2AccountSnapshot {
            visible: root.zoneState.l2AccountHistorical !== null
                || root.zoneState.l2AccountHistoricalInFlight
                || root.zoneState.l2AccountHistoricalError.length > 0
            objectName: "zoneL2HistoricalAccountSnapshot"
            theme: root.theme
            title: root.historicalTitle()
            snapshot: root.zoneState.l2AccountHistorical
            report: root.zoneState.l2AccountHistoricalReport
            error: root.zoneState.l2AccountHistoricalError
            busy: root.zoneState.l2AccountHistoricalInFlight
            decode: root.zoneState.l2AccountHistoricalDecode
            decodeError: root.zoneState.l2AccountHistoricalDecodeError
            decodeInFlight: root.zoneState.l2AccountHistoricalDecodeInFlight
        }
    }

    ColumnLayout {
        visible: root.zoneState.l2AccountId.length > 0
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        Text {
            text: qsTr("Account Activity")
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: root.theme.secondaryText
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        StatusMessage {
            visible: root.zoneState.l2AccountActivityError.length > 0
            theme: root.theme
            tone: "warning"
            title: qsTr("Activity unavailable")
            message: root.zoneState.l2AccountActivityError
            Layout.fillWidth: true
        }

        PagedInspectionTable {
            objectName: "zoneL2AccountActivityTable"
            theme: root.theme
            loadCount: root.zoneState.l2AccountActivityLimit
            loadOptions: [10, 25, 50]
            rangeText: qsTr("%1 rows / oldest first")
                .arg(Presentation.numberText(root.zoneState.l2AccountActivityRows.length))
            canGoNewer: false
            canGoOlder: root.zoneState.l2AccountActivityHasMore
            busy: root.zoneState.l2AccountActivityInFlight
            refreshText: qsTr("Oldest")
            olderText: qsTr("More")
            headerCells: [
                { text: qsTr("Index"), width: 70 },
                { text: qsTr("Transaction"), width: 250, fill: true },
                { text: qsTr("Kind"), width: 110 },
                { text: qsTr("Direction"), width: 110 },
                { text: qsTr("Program"), width: 220, fill: true }
            ]
            rows: root.activityRows()
            Layout.fillWidth: true
            onRefreshRequested: root.zoneState.refreshL2AccountActivity()
            onOlderRequested: root.zoneState.loadMoreL2AccountActivity()
            onLoadCountSelected: function (count) {
                root.zoneState.setL2AccountActivityLimit(count)
            }
            onCellActivated: function (row, column, cell, rowData) {
                if (column === 1 && rowData.transactionId.length > 0) {
                    root.transactionRequested(rowData.transactionId,
                        root.zoneState.l2IndexerSourceId())
                }
            }
        }

        Text {
            visible: root.zoneState.l2AccountActivityLoaded
                && root.zoneState.l2AccountActivityRows.length === 0
                && root.zoneState.l2AccountActivityError.length === 0
            text: qsTr("No finalized activity for this account")
            color: root.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: root.theme.dataText
            Layout.fillWidth: true
        }
    }

    function activityRows() {
        const rows = Array.isArray(root.zoneState.l2AccountActivityRows)
            ? root.zoneState.l2AccountActivityRows : []
        return rows.map(function (row) {
            const transactionId = String(row && row.transaction_id || "")
            const programId = String(row && row.program_id_hex || "")
            return {
                cells: [
                    { text: Presentation.numberText(row && row.index), width: 70 },
                    { text: transactionId, width: 250, fill: true, link: transactionId.length > 0, copyText: transactionId },
                    { text: Presentation.words(row && row.kind), width: 110, monospace: false },
                    { text: Presentation.words(row && row.direction), width: 110, monospace: false },
                    { text: programId, width: 220, fill: true, copyable: programId.length > 0, copyText: programId }
                ],
                transactionId: transactionId
            }
        })
    }

    function commentTopic() {
        return root.appModel && root.entityRef
            && root.appModel.social
            && typeof root.appModel.social.zoneCommentTopic === "function"
            ? root.appModel.social.zoneCommentTopic(root.entityRef) : ""
    }

    function historicalTitle() {
        const target = root.zoneState.l2AccountHistoricalTarget || ({})
        return target.block_id === undefined
            ? qsTr("Historical Snapshot")
            : qsTr("Historical Snapshot / Block %1")
                .arg(Presentation.numberText(target.block_id))
    }
}
