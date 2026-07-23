pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../components/common"
import "../../../theme"
import "../../lez/controls/sequencer"
import "../ZonePresentation.js" as Presentation

ColumnLayout {
    id: root

    required property Theme theme
    required property var zoneState
    property string currentTab: "inspection"
    readonly property var detail: root.zoneState.l2TransactionDetail
    readonly property var report: root.zoneState.l2TransactionDetailReport
    readonly property var transaction: root.detail ? root.detail.transaction : null
    readonly property var inspection: root.detail ? root.detail.inspection : null
    readonly property var traceValue: root.zoneState.l2TransactionTrace
    readonly property var trace: root.traceValue ? root.traceValue.trace : null
    readonly property var localSubmissionDecode: root.zoneState
        .l2SubmittedTransactionLocalDecode
    readonly property string localSubmissionDecodeWarning: String(root.zoneState
        .l2SubmittedTransactionLocalDecodeWarning || "")
    readonly property string localSubmissionDecodeError: String(root.zoneState
        .l2SubmittedTransactionLocalDecodeError || "")
    readonly property bool localSubmissionPrivateSyncPending: !!(
        root.zoneState.l2SubmittedTransactionReceiptTraceInput
        && root.zoneState.l2SubmittedTransactionReceiptTraceInput.privateSyncPending
            === true)
    readonly property var appModel: root.zoneState.appModel || null
    readonly property var entityRef: typeof root.zoneState.l2TransactionEntityRef === "function"
        ? root.zoneState.l2TransactionEntityRef(root.detail) : null
    readonly property var favoriteEntry: root.appModel && root.entityRef
        ? root.appModel.favoriteStore.l2EntityEntry(root.entityRef,
            qsTr("L2 Transaction %1").arg(String(
                root.transaction && root.transaction.hash || "").slice(0, 12)),
            String(root.transaction && root.transaction.hash || "")) : null

    signal backRequested()
    signal configureSourcesRequested()

    objectName: "zoneL2TransactionDetail"
    spacing: root.theme.gapLarge
    Layout.fillWidth: true

    ListModel {
        id: detailTabs

        ListElement { value: "inspection"; label: "Inspection" }
        ListElement { value: "trace"; label: "Trace" }
    }

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        ActionButton {
            objectName: "zoneL2TransactionBackButton"
            theme: root.theme
            iconOnly: true
            iconName: "back"
            accessibleName: qsTr("Back to L2 block")
            Layout.preferredWidth: root.theme.controlHeight
            onClicked: root.backRequested()
        }

        ColumnLayout {
            spacing: root.theme.gapTiny
            Layout.fillWidth: true

            Text {
                text: qsTr("L2 Transaction")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: root.theme.panelTitleText
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            Text {
                text: root.transaction ? String(root.transaction.hash || "")
                    : root.zoneState.l2TransactionId
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
        visible: root.zoneState.l2TransactionDetailInFlight
        theme: root.theme
        tone: "info"
        title: qsTr("Loading transaction")
        message: qsTr("Applying Active Zone source policy and verifying returned content identity.")
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.zoneState.l2TransactionDetailError.length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Transaction unavailable")
        message: root.zoneState.l2TransactionDetailError
        Layout.fillWidth: true
    }

    ActionButton {
        visible: root.zoneState.l2TransactionDetailError.length > 0
        theme: root.theme
        text: root.recoveryButtonText(root.zoneState.l2TransactionDetailErrorDetails)
        enabled: !root.zoneState.l2TransactionDetailInFlight
        Layout.preferredWidth: 150
        onClicked: root.recoverTransaction()
    }

    ColumnLayout {
        visible: root.zoneState.l2TransactionCandidates.length > 0
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        StatusMessage {
            theme: root.theme
            tone: "warning"
            title: qsTr("Transaction source is ambiguous")
            message: qsTr("Choose exact source evidence before deriving inspection or trace output.")
            Layout.fillWidth: true
        }

        DataTableFrame {
            objectName: "zoneL2TransactionCandidatesTable"
            theme: root.theme
            headerCells: [
                { text: qsTr("Source ID"), width: 250, fill: true },
                { text: qsTr("Role"), width: 110 },
                { text: qsTr("Canonical transaction"), width: 250, fill: true }
            ]
            rows: root.candidateRows()
            Layout.fillWidth: true
            onCellActivated: function (row, column, cell, rowData) {
                root.zoneState.resolveL2TransactionCandidate(rowData.candidate)
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
            title: qsTr("Transaction")
            rows: root.transactionRows()
        }

        ZoneFactSection {
            theme: root.theme
            title: qsTr("Payload")
            rows: root.payloadRows()
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
            title: qsTr("Transaction comments")
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

    function commentTopic() {
        return root.appModel && root.entityRef
            && root.appModel.social
            && typeof root.appModel.social.zoneCommentTopic === "function"
            ? root.appModel.social.zoneCommentTopic(root.entityRef) : ""
    }

    TabSwitch {
        visible: root.detail !== null
        theme: root.theme
        options: detailTabs
        current: root.currentTab
        onSelected: function (value) {
            root.currentTab = value
        }
    }

    ColumnLayout {
        visible: root.detail !== null && root.currentTab === "inspection"
        spacing: root.theme.gapLarge
        Layout.fillWidth: true

        Text {
            visible: root.inspectionSections().length === 0
            text: qsTr("No normalized inspection sections returned")
            color: root.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: root.theme.dataText
            Layout.fillWidth: true
        }

        Repeater {
            model: root.inspectionSections()

            DetailSection {
                required property var modelData

                theme: root.theme
                title: String(modelData.title || qsTr("Inspection"))
                rows: root.inspectionRows(modelData.rows)
                labelWidth: 180
                surfaceColor: root.theme.surface
            }
        }
    }

    ColumnLayout {
        visible: root.detail !== null && root.currentTab === "trace"
        spacing: root.theme.gapLarge
        Layout.fillWidth: true

        StatusMessage {
            visible: root.zoneState.l2TransactionTraceInFlight
            theme: root.theme
            tone: "info"
            title: qsTr("Deriving transaction trace")
            message: qsTr("Trace uses the exact transaction source shown above.")
            Layout.fillWidth: true
        }

        StatusMessage {
            visible: root.zoneState.l2TransactionTraceError.length > 0
            theme: root.theme
            tone: "warning"
            title: qsTr("Trace unavailable")
            message: root.zoneState.l2TransactionTraceError
            Layout.fillWidth: true
        }

        ActionButton {
            visible: root.zoneState.l2TransactionTraceError.length > 0
            theme: root.theme
            text: qsTr("Retry trace")
            enabled: !root.zoneState.l2TransactionTraceInFlight
            Layout.preferredWidth: 130
            onClicked: root.retryTrace()
        }

        StatusMessage {
            visible: root.traceSourceMismatch()
            theme: root.theme
            tone: "error"
            title: qsTr("Trace provenance mismatch")
            message: qsTr("Trace output was rejected because its source differs from transaction detail.")
            Layout.fillWidth: true
        }

        ZoneFactSection {
            visible: root.trace !== null && root.trace.decoded_instruction !== null
            theme: root.theme
            title: qsTr("Decoded Instruction")
            rows: root.decodedRows()
            Layout.fillWidth: true
        }

        StatusMessage {
            visible: root.localSubmissionDecode !== null
            theme: root.theme
            tone: "info"
            title: qsTr("Locally decoded submitted instruction")
            message: qsTr("Privacy envelope does not expose program or instruction words. Decoded automatically from frozen local submission metadata held by this Inspector session and matched to this exact-source transaction.")
            Layout.fillWidth: true
        }

        StatusMessage {
            visible: root.localSubmissionPrivateSyncPending
            theme: root.theme
            tone: "warning"
            title: qsTr("Private sync pending")
            message: qsTr("Transaction was submitted and is awaiting inclusion. After inclusion, use Read incoming in Local Wallet to update local private account state.")
            Layout.fillWidth: true
        }

        StatusMessage {
            visible: root.localSubmissionDecode !== null
                && root.localSubmissionDecodeWarning.length > 0
            theme: root.theme
            tone: "warning"
            title: qsTr("Local submission decoded partially")
            message: root.localSubmissionDecodeWarning
            Layout.fillWidth: true
        }

        StatusMessage {
            visible: root.localSubmissionDecode === null
                && root.localSubmissionDecodeError.length > 0
            theme: root.theme
            tone: "warning"
            title: qsTr("Local submission decode unavailable")
            message: root.localSubmissionDecodeError
            Layout.fillWidth: true
        }

        ZoneFactSection {
            visible: root.localSubmissionDecode !== null
            theme: root.theme
            title: qsTr("Local submission metadata")
            rows: root.localSubmissionDecodedRows()
            Layout.fillWidth: true
        }

        TraceSummary {
            objectName: "zoneL2TraceSummary"
            visible: root.trace !== null && !root.traceSourceMismatch()
            theme: root.theme
            steps: root.trace && Array.isArray(root.trace.steps) ? root.trace.steps : []
            capabilities: root.trace && Array.isArray(root.trace.capabilities)
                ? root.trace.capabilities : []
            limitations: root.trace && Array.isArray(root.trace.limitations)
                ? root.trace.limitations : []
            Layout.fillWidth: true
        }
    }

    function transactionRows() {
        const value = root.transaction || ({})
        return [{
            label: qsTr("Kind"),
            value: Presentation.text(value.kind)
        }, {
            label: qsTr("Program"),
            value: Presentation.text(value.program_id_hex),
            copyable: String(value.program_id_hex || "").length > 0,
            monospace: true
        }]
    }

    function payloadRows() {
        const value = root.transaction || ({})
        return [{
            label: qsTr("Accounts"),
            value: Presentation.numberText(Array.isArray(value.account_ids) ? value.account_ids.length : 0)
        }, {
            label: qsTr("Nonces"),
            value: Presentation.numberText(Array.isArray(value.nonces) ? value.nonces.length : 0)
        }, {
            label: qsTr("Instruction words"),
            value: Presentation.numberText(Array.isArray(value.instruction_data) ? value.instruction_data.length : 0)
        }, {
            label: qsTr("Bytecode"),
            value: value.bytecode_len === undefined || value.bytecode_len === null
                ? "-" : qsTr("%1 bytes").arg(Presentation.numberText(value.bytecode_len))
        }]
    }

    function inspectionSections() {
        return root.inspection && Array.isArray(root.inspection.sections)
            ? root.inspection.sections : []
    }

    function inspectionRows(rows) {
        const source = Array.isArray(rows) ? rows : []
        return source.map(function (row) {
            const suffix = row && row.index !== undefined && row.index !== null
                ? qsTr(" [%1]").arg(row.index) : ""
            return {
                label: String(row && row.label || qsTr("Value")) + suffix,
                value: Presentation.text(row && row.value),
                subvalue: root.inspectionSubvalue(row),
                monospace: true
            }
        })
    }

    function inspectionSubvalue(row) {
        if (!row) {
            return ""
        }
        const values = []
        if (row.decimal) {
            values.push(qsTr("decimal %1").arg(row.decimal))
        }
        if (row.hex && String(row.hex) !== String(row.value || "")) {
            values.push(qsTr("hex %1").arg(row.hex))
        }
        if (row.base58 && String(row.base58) !== String(row.value || "")) {
            values.push(qsTr("base58 %1").arg(row.base58))
        }
        return values.join(qsTr(" / "))
    }

    function decodedRows() {
        const decoded = root.trace && root.trace.decoded_instruction
            ? root.trace.decoded_instruction : ({})
        return root.instructionDecodeRows(decoded)
    }

    function localSubmissionDecodedRows() {
        return root.instructionDecodeRows(root.localSubmissionDecode || ({}))
    }

    function instructionDecodeRows(decoded) {
        const rows = [{
            label: qsTr("Instruction"),
            value: Presentation.text(decoded.instruction)
        }, {
            label: qsTr("Variant"),
            value: Presentation.text(decoded.variant_index),
            monospace: true
        }, {
            label: qsTr("Program"),
            value: Presentation.text(decoded.program_id),
            copyable: String(decoded.program_id || "").length > 0,
            monospace: true
        }, {
            label: qsTr("IDL"),
            value: Presentation.text(decoded.idl_name)
        }]
        const accounts = Array.isArray(decoded.accounts) ? decoded.accounts : []
        for (let i = 0; i < accounts.length; ++i) {
            const account = accounts[i] || ({})
            const value = Presentation.text(account.value)
            rows.push({
                label: qsTr("Account %1").arg(Presentation.text(account.path, qsTr("Value"))),
                value: value,
                copyable: value !== "-",
                monospace: true
            })
        }
        const args = Array.isArray(decoded.args) ? decoded.args : []
        for (let j = 0; j < args.length; ++j) {
            const arg = args[j] || ({})
            const value = Presentation.text(arg.value)
            rows.push({
                label: qsTr("Argument %1").arg(Presentation.text(arg.path, qsTr("Value"))),
                value: value,
                copyable: value !== "-",
                monospace: true
            })
        }
        const remainingWords = Array.isArray(decoded.remaining_words)
            ? decoded.remaining_words : []
        if (remainingWords.length > 0) {
            rows.push({
                label: qsTr("Remaining instruction words"),
                value: remainingWords.join(", "),
                monospace: true
            })
        }
        return rows
    }

    function candidateRows() {
        const rows = Array.isArray(root.zoneState.l2TransactionCandidates)
            ? root.zoneState.l2TransactionCandidates : []
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

    function traceSourceMismatch() {
        if (!root.detail || !root.traceValue) {
            return false
        }
        return String(root.detail.source && root.detail.source.source_id || "")
            !== String(root.traceValue.source && root.traceValue.source.source_id || "")
    }

    function recoveryButtonText(details) {
        const recovery = String(details && details.recovery || "retry")
        return recovery === "configure_source" || recovery === "select_source"
            ? qsTr("Open Sources") : qsTr("Retry")
    }

    function recoverTransaction() {
        const details = root.zoneState.l2TransactionDetailErrorDetails
        const recovery = String(details && details.recovery || "retry")
        if (recovery === "configure_source" || recovery === "select_source") {
            root.configureSourcesRequested()
            return
        }
        root.zoneState.openL2Transaction(root.zoneState.l2TransactionId,
            root.zoneState.l2TransactionRequestedSourceId)
    }

    function retryTrace() {
        const sourceId = String(root.detail && root.detail.source
            && root.detail.source.source_id || "")
        root.zoneState.requestL2TransactionTrace(root.zoneState.l2TransactionId,
            sourceId)
    }
}
