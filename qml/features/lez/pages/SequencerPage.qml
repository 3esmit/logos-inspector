pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQml.Models
import QtQuick.Layouts
import "../../../components"
import "../controls/sequencer"
import "../../../state"
import "../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model
    readonly property bool hasResponse: root.model.pageHasOutput("sequencer")
    readonly property var responseValue: root.hasResponse ? root.model.resultValue : null

    width: parent ? parent.width : 900
    spacing: 16

    ListModel {
        id: sequencerTabs

        ListElement { value: "blocks"; label: "Blocks" }
        ListElement { value: "transactions"; label: "Transactions" }
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / L2 LEZ")
        title: qsTr("L2 LEZ Blocks / Transactions")
        layerLabel: qsTr("L2 LEZ")
        subtitle: qsTr("Query LEZ blocks, transaction summaries, decoded instructions, and traces from the selected sequencer endpoint.")
        Layout.fillWidth: true

        ActionButton {
            theme: root.theme
            text: qsTr("Head")
            primary: true
            enabled: !root.model.busy
            Layout.preferredWidth: 96
            accessibleName: qsTr("Fetch sequencer head")
            onClicked: root.model.callInspector("head", root.model.executionArgs([]), qsTr("Sequencer head"))
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Programs")
            enabled: !root.model.busy
            Layout.preferredWidth: 112
            accessibleName: qsTr("Fetch sequencer programs")
            onClicked: root.model.callInspector("programs", root.model.executionRpcArgs([]), qsTr("Sequencer programs"))
        }
    }

    GridLayout {
        columns: root.width < 760 ? 2 : 4
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gap
        Layout.fillWidth: true

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Endpoint")
            value: root.endpointLabel(root.model.sequencerUrl)
            delta: root.shortEndpoint(root.model.sequencerUrl)
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Head")
            value: root.sequencerHeadText()
            delta: qsTr("Latest sequencer block")
            deltaColor: root.sequencerHeadText() !== "-" ? root.theme.success : root.theme.textMuted
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Tool")
            value: root.activeTabLabel()
            delta: root.activeTabDelta()
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Last result")
            value: root.lastResultText()
            delta: root.lastResultDelta()
            deltaColor: root.lastResultColor()
        }
    }

    Panel {
        theme: root.theme
        title: qsTr("%1 lookup").arg(root.activeTabLabel())

        TabSwitch {
            theme: root.theme
            current: root.model.sequencerTab
            options: sequencerTabs
            onSelected: value => root.model.sequencerTab = value
        }

        StatusMessage {
            theme: root.theme
            tone: "info"
            title: root.activeTabLabel()
            message: root.activeTabMessage()
            Layout.fillWidth: true
        }

        Loader {
            active: true
            sourceComponent: root.model.sequencerTab === "blocks" ? blocksForm : transactionsForm
            Layout.fillWidth: true
        }
    }

    Panel {
        visible: root.hasResponse
        theme: root.theme
        title: root.model.resultIsError ? qsTr("Sequencer error") : qsTr("Sequencer response")

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Text {
                text: root.model.resultTitle
                color: root.model.resultIsError ? root.theme.error : root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
                font.weight: Font.Medium
                elide: Text.ElideRight
                Layout.fillWidth: true
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Clear")
                enabled: root.model.resultText.length > 0 || root.model.resultValue !== null
                Layout.preferredWidth: 84
                onClicked: root.model.clearResult()
            }
        }

        StatusMessage {
            visible: root.model.resultIsError
            theme: root.theme
            tone: "warning"
            title: qsTr("Call failed")
            message: root.model.resultText
            Layout.fillWidth: true
        }

        StatusMessage {
            visible: !root.model.resultIsError && root.responseValue === null
            theme: root.theme
            tone: "warning"
            title: qsTr("No data")
            message: qsTr("The sequencer returned no object for this lookup. Check the endpoint, block ID, or transaction hash.")
            Layout.fillWidth: true
        }

        GridLayout {
            visible: !root.model.resultIsError && root.responseValue !== null
            columns: root.width < 760 ? 2 : 4
            columnSpacing: root.theme.gap
            rowSpacing: root.theme.gap
            Layout.fillWidth: true

            MetricCard {
                theme: root.theme
                compact: true
                label: qsTr("Status")
                value: qsTr("OK")
                delta: root.model.resultTitle
                deltaColor: root.theme.success
            }

            MetricCard {
                theme: root.theme
                compact: true
                label: qsTr("Payload")
                value: root.responsePayloadText()
                delta: root.responseKindText()
            }

            MetricCard {
                theme: root.theme
                compact: true
                label: qsTr("Block")
                value: root.responseBlockText()
                delta: root.responseBlockDelta()
            }

            MetricCard {
                theme: root.theme
                compact: true
                label: qsTr("Transaction")
                value: root.responseTransactionText()
                delta: root.responseTransactionDelta()
            }
        }

        SequencerBlockSummary {
            visible: root.isSequencerBlock(root.responseValue)
            theme: root.theme
            block: root.responseValue
            modelRef: root.model
        }

        SequencerProgramList {
            visible: root.programRows().length > 0
            theme: root.theme
            rows: root.programRows()
            modelRef: root.model
        }

        TransactionDetailPane {
            visible: root.isTransactionResponse(root.responseValue)
            theme: root.theme
            model: root.model
            value: root.responseValue
        }

        TraceSummary {
            visible: root.traceSteps().length > 0
            theme: root.theme
            steps: root.traceSteps()
            capabilities: root.traceCapabilities()
            limitations: root.traceLimitations()
            modelRef: root.model
        }

        TextArea {
            visible: root.shouldShowRawResponse()
            readOnly: true
            text: root.model.resultText.length ? root.model.resultText : qsTr("No response body.")
            wrapMode: TextArea.Wrap
            color: root.model.resultText.length ? root.theme.text : root.theme.textMuted
            selectedTextColor: root.theme.selectedText
            selectionColor: root.theme.accent
            textFormat: Text.PlainText
            font.family: "monospace"
            font.pixelSize: root.theme.secondaryText
            leftPadding: 12
            rightPadding: 12
            topPadding: 10
            bottomPadding: 10
            Layout.fillWidth: true
            Layout.preferredHeight: root.model.resultIsError ? 120 : 220

            background: Rectangle {
                color: root.model.resultIsError ? root.theme.errorMuted : root.theme.field
                radius: root.theme.radius
                border.width: 1
                border.color: root.model.resultIsError ? root.theme.error : root.theme.outline
            }
        }
    }

    Component {
        id: blocksForm

        ColumnLayout {
            spacing: 12

            FieldRow {
                id: blockId
                theme: root.theme
                label: qsTr("Sequencer block ID")
                placeholderText: qsTr("2051")
            }

            GridLayout {
                columns: root.width < 680 ? 1 : 3
                columnSpacing: root.theme.gapSmall
                rowSpacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Fetch block")
                    primary: true
                    enabled: !root.model.busy && root.isUnsignedInteger(blockId.text)
                    Layout.fillWidth: true
                    accessibleName: qsTr("Fetch sequencer block")
                    onClicked: root.model.callInspector("block", root.model.executionArgs([blockId.text]), qsTr("Sequencer block"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Open L2")
                    enabled: !root.model.busy && blockId.text.trim().length > 0
                    Layout.fillWidth: true
                    accessibleName: qsTr("Open LEZ block")
                    onClicked: root.model.openLezBlock(blockId.text)
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Head")
                    enabled: !root.model.busy
                    Layout.fillWidth: true
                    accessibleName: qsTr("Fetch sequencer head")
                    onClicked: root.model.callInspector("head", root.model.executionArgs([]), qsTr("Sequencer head"))
                }
            }
        }
    }

    Component {
        id: transactionsForm

        ColumnLayout {
            spacing: 12

            FieldRow {
                id: txHash
                theme: root.theme
                label: qsTr("Transaction hash")
                placeholderText: qsTr("62f6fe119469303654061239dee295fd92a2ef3ed9558f33d2a76d9aded11cbd")
            }

            TextAreaField {
                id: txIdl
                theme: root.theme
                label: qsTr("IDL JSON")
                placeholderText: qsTr("Optional IDL override")
                rows: 5
            }

            GridLayout {
                columns: root.width < 680 ? 1 : 4
                columnSpacing: root.theme.gapSmall
                rowSpacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Summary")
                    primary: true
                    enabled: !root.model.busy && txHash.text.trim().length > 0
                    Layout.fillWidth: true
                    accessibleName: qsTr("Fetch transaction summary")
                    onClicked: root.model.callInspector("transaction", root.model.executionArgs([txHash.text]), qsTr("Transaction summary"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Decode")
                    enabled: !root.model.busy && txHash.text.trim().length > 0
                    Layout.fillWidth: true
                    accessibleName: qsTr("Decode transaction")
                    onClicked: root.model.callInspector("inspectTransaction", root.transactionArgs(txHash.text, txIdl.text), qsTr("Transaction inspection"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Trace")
                    enabled: !root.model.busy && txHash.text.trim().length > 0
                    Layout.fillWidth: true
                    accessibleName: qsTr("Trace transaction")
                    onClicked: root.model.callInspector("traceTransaction", root.transactionArgs(txHash.text, txIdl.text), qsTr("Transaction trace"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Open")
                    enabled: !root.model.busy && txHash.text.trim().length > 0
                    Layout.fillWidth: true
                    accessibleName: qsTr("Open transaction screen")
                    onClicked: root.model.openReference("transaction", txHash.text)
                }
            }
        }
    }

    function activeTabLabel() {
        return root.model.sequencerTab === "transactions" ? qsTr("Transactions") : qsTr("Blocks")
    }

    function activeTabDelta() {
        return root.model.sequencerTab === "transactions" ? qsTr("Hash, decode, trace") : qsTr("Numeric block ID")
    }

    function activeTabMessage() {
        if (root.model.sequencerTab === "transactions") {
            return qsTr("Summary, decode, and trace calls stay on this page. Open follows the hash to the Transactions screen.")
        }
        return qsTr("Sequencer block fetch requires a numeric LEZ block ID. Use l1:<slot> for Bedrock slots.")
    }

    function transactionArgs(hash, idl) {
        const trimmedIdl = String(idl || "").trim()
        const args = root.model.executionArgs([String(hash || "").trim()])
        if (trimmedIdl.length > 0) {
            args.push(trimmedIdl)
        }
        return args
    }

    function activeSequencerProbe() {
        const value = root.responseValue
        if (value && typeof value === "object" && !Array.isArray(value) && value.sequencer !== undefined) {
            return value.sequencer
        }
        const overview = root.model.dashboardOverview
        if (overview && overview.sequencer !== undefined) {
            return overview.sequencer
        }
        return null
    }

    function sequencerHeadText() {
        const probe = root.activeSequencerProbe()
        if (probe && probe.head) {
            return root.valueText(probe.head.value)
        }
        if (root.hasResponse && root.model.resultTitle === qsTr("Sequencer head")) {
            return root.valueText(root.responseValue)
        }
        if (root.isSequencerBlock(root.responseValue)) {
            return root.valueText(root.responseValue.block_id)
        }
        return "-"
    }

    function lastResultText() {
        if (!root.hasResponse) {
            return qsTr("Idle")
        }
        return root.model.resultIsError ? qsTr("Error") : qsTr("OK")
    }

    function lastResultDelta() {
        if (!root.hasResponse) {
            return qsTr("No page output")
        }
        if (root.model.resultIsError) {
            return root.model.resultText
        }
        return root.model.resultTitle
    }

    function lastResultColor() {
        if (!root.hasResponse) {
            return root.theme.textMuted
        }
        return root.model.resultIsError ? root.theme.warning : root.theme.success
    }

    function responsePayloadText() {
        const value = root.responseValue
        if (value === null || value === undefined) {
            return "-"
        }
        if (Array.isArray(value)) {
            return root.numberText(value.length)
        }
        if (typeof value === "object") {
            if (root.isSequencerBlock(value)) {
                return root.numberText(value.tx_count)
            }
            if (value.steps && Array.isArray(value.steps)) {
                return root.numberText(value.steps.length)
            }
            return root.numberText(Object.keys(value).length)
        }
        return root.valueText(value)
    }

    function responseKindText() {
        const value = root.responseValue
        if (root.isSequencerBlock(value)) {
            return qsTr("Transactions")
        }
        if (root.isTransactionResponse(value)) {
            return value && value.steps ? qsTr("Trace steps") : qsTr("Transaction fields")
        }
        if (Array.isArray(value)) {
            return qsTr("Array items")
        }
        if (value && typeof value === "object") {
            return qsTr("Object fields")
        }
        return qsTr("Scalar value")
    }

    function responseBlockText() {
        const value = root.responseValue
        if (root.isSequencerBlock(value)) {
            return root.valueText(value.block_id)
        }
        if (root.model.resultTitle === qsTr("Sequencer head")) {
            return root.valueText(value)
        }
        return "-"
    }

    function responseBlockDelta() {
        const value = root.responseValue
        if (root.isSequencerBlock(value)) {
            return root.shortHash(value.header_hash)
        }
        if (root.model.resultTitle === qsTr("Sequencer head")) {
            return qsTr("Current head")
        }
        return qsTr("No block context")
    }

    function responseTransactionText() {
        const summary = root.transactionSummary(root.responseValue)
        if (summary) {
            return root.shortHash(summary.hash)
        }
        if (root.isSequencerBlock(root.responseValue)) {
            return root.numberText(root.responseValue.tx_count)
        }
        return "-"
    }

    function responseTransactionDelta() {
        const summary = root.transactionSummary(root.responseValue)
        if (summary) {
            return root.valueText(summary.kind)
        }
        if (root.isSequencerBlock(root.responseValue)) {
            return qsTr("In fetched block")
        }
        return qsTr("No transaction")
    }

    function isSequencerBlock(value) {
        return value && typeof value === "object" && !Array.isArray(value) && value.block_id !== undefined && value.header_hash !== undefined && value.transactions !== undefined
    }

    function isTransactionResponse(value) {
        return root.transactionSummary(value) !== null
    }

    function transactionSummary(value) {
        if (!value || typeof value !== "object" || Array.isArray(value)) {
            return null
        }
        if (value.raw_summary) {
            return value.raw_summary
        }
        if (value.inspection && value.inspection.raw_summary) {
            return value.inspection.raw_summary
        }
        if (value.hash && value.kind) {
            return value
        }
        return null
    }

    function traceSteps() {
        const value = root.responseValue
        return value && Array.isArray(value.steps) ? value.steps : []
    }

    function traceCapabilities() {
        const value = root.responseValue
        return value && Array.isArray(value.capabilities) ? value.capabilities : []
    }

    function traceLimitations() {
        const value = root.responseValue
        return value && Array.isArray(value.limitations) ? value.limitations : []
    }

    function shouldShowRawResponse() {
        if (!root.hasResponse || root.model.resultIsError || root.responseValue === null || root.responseValue === undefined) {
            return false
        }
        if (root.isSequencerBlock(root.responseValue) || root.isTransactionResponse(root.responseValue) || root.programRows().length > 0) {
            return false
        }
        if (root.model.resultTitle === qsTr("Sequencer head")) {
            return false
        }
        return root.model.resultText.length > 0
    }

    function programRows() {
        const value = root.responseValue
        if (!Array.isArray(value)) {
            return []
        }
        return value.filter(function (row) {
            return row && typeof row === "object" && (row.hex !== undefined || row.base58 !== undefined || row.label !== undefined)
        })
    }

    function isUnsignedInteger(value) {
        return /^[0-9]+$/.test(String(value || "").trim())
    }

    function numberText(value) {
        if (value === undefined || value === null || value === "") {
            return "-"
        }
        const number = Number(value)
        if (!Number.isNaN(number) && Number.isFinite(number)) {
            return number.toLocaleString(Qt.locale(), "f", 0)
        }
        return String(value)
    }

    function valueText(value) {
        if (value === undefined || value === null || value === "") {
            return "-"
        }
        if (typeof value === "number") {
            return value % 1 === 0 ? value.toLocaleString(Qt.locale(), "f", 0) : String(value)
        }
        return String(value)
    }

    function shortHash(value) {
        const text = String(value || "")
        if (text.length <= 16) {
            return text.length ? text : "-"
        }
        return text.slice(0, 8) + "..." + text.slice(-6)
    }

    function endpointLabel(value) {
        const text = String(value || "")
        if (!text.length) {
            return "-"
        }
        if (text.indexOf("127.0.0.1") >= 0 || text.indexOf("localhost") >= 0) {
            return qsTr("Local")
        }
        if (text.indexOf("testnet") >= 0) {
            return qsTr("Testnet")
        }
        return qsTr("Custom")
    }

    function shortEndpoint(value) {
        const text = String(value || "")
        if (!text.length) {
            return qsTr("No endpoint")
        }
        return text.replace(/^https?:\/\//, "").replace(/\/$/, "")
    }

}
