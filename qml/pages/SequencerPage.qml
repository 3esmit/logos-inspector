pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQml.Models
import QtQuick.Layouts
import "../components"
import "../state"
import "../theme"

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
            onClicked: root.model.callInspector("head", [root.model.sequencerUrl], qsTr("Sequencer head"))
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Programs")
            enabled: !root.model.busy
            Layout.preferredWidth: 112
            accessibleName: qsTr("Fetch sequencer programs")
            onClicked: root.model.callInspector("programs", [root.model.sequencerUrl], qsTr("Sequencer programs"))
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
                    onClicked: root.model.callInspector("block", [root.model.sequencerUrl, blockId.text], qsTr("Sequencer block"))
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
                    onClicked: root.model.callInspector("head", [root.model.sequencerUrl], qsTr("Sequencer head"))
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
                    onClicked: root.model.callInspector("transaction", [root.model.sequencerUrl, txHash.text], qsTr("Transaction summary"))
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
        const args = [root.model.sequencerUrl, String(hash || "").trim()]
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

    component SequencerBlockSummary: ColumnLayout {
        id: blockRoot

        required property Theme theme
        property var block: null
        property AppModel modelRef

        visible: blockRoot.block !== null
        spacing: blockRoot.theme.gap
        Layout.fillWidth: true

        SectionBlock {
            theme: blockRoot.theme
            title: qsTr("Block")
            rows: blockRoot.overviewRows()
            modelRef: blockRoot.modelRef
        }

        StatusMessage {
            visible: blockRoot.block && String(blockRoot.block.decode_warning || "").length > 0
            theme: blockRoot.theme
            tone: "warning"
            title: qsTr("Decode warning")
            message: String(blockRoot.block ? blockRoot.block.decode_warning || "" : "")
            Layout.fillWidth: true
        }

        ColumnLayout {
            visible: blockRoot.block !== null
            spacing: blockRoot.theme.gapSmall
            Layout.fillWidth: true

            Text {
                text: qsTr("Transactions (%1)").arg(blockRoot.valueText(blockRoot.block ? blockRoot.block.tx_count : 0))
                color: blockRoot.theme.text
                textFormat: Text.PlainText
                font.pixelSize: blockRoot.theme.primaryText
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            Frame {
                padding: 0
                Layout.fillWidth: true

                background: Rectangle {
                    color: blockRoot.theme.surface
                    radius: blockRoot.theme.radius
                    border.width: 1
                    border.color: blockRoot.theme.outlineMuted
                }

                contentItem: ColumnLayout {
                    spacing: 0

                    SequencerTransactionRow {
                        theme: blockRoot.theme
                        header: true
                        columns: [qsTr("Index"), qsTr("Hash"), qsTr("Kind"), qsTr("Program")]
                    }

                    Repeater {
                        model: blockRoot.transactionRows()

                        SequencerTransactionRow {
                            required property var modelData

                            theme: blockRoot.theme
                            columns: [modelData.index, modelData.hashText, modelData.kind, modelData.programText]
                            hash: modelData.hash
                            program: modelData.program
                            modelRef: blockRoot.modelRef
                        }
                    }
                }
            }
        }

        function overviewRows() {
            const value = blockRoot.block || {}
            return [
                { label: qsTr("L2 block ID"), value: blockRoot.valueText(value.block_id), monospace: true, linkKind: "lezBlock", linkValue: blockRoot.valueText(value.block_id) },
                { label: qsTr("Header hash"), value: blockRoot.valueText(value.header_hash), monospace: true, linkKind: "", linkValue: "" },
                { label: qsTr("Previous header hash"), value: blockRoot.valueText(value.parent_hash), monospace: true, linkKind: "", linkValue: "" },
                { label: qsTr("Timestamp"), value: blockRoot.valueText(value.timestamp), monospace: true },
                { label: qsTr("Bedrock status"), value: blockRoot.valueText(value.bedrock_status), monospace: false },
                { label: qsTr("Transactions"), value: blockRoot.valueText(value.tx_count), monospace: true }
            ]
        }

        function transactionRows() {
            const transactions = blockRoot.block && Array.isArray(blockRoot.block.transactions) ? blockRoot.block.transactions : []
            if (!transactions.length) {
                return [{
                    index: "-",
                    hashText: qsTr("No transactions"),
                    kind: "-",
                    programText: "-",
                    hash: "",
                    program: ""
                }]
            }
            return transactions.map(function (tx, index) {
                return {
                    index: blockRoot.valueText(index),
                    hashText: blockRoot.shortHash(tx.hash),
                    kind: blockRoot.valueText(tx.kind),
                    programText: tx.program_id_hex ? blockRoot.shortHash(tx.program_id_hex) : "-",
                    hash: String(tx.hash || ""),
                    program: String(tx.program_id_hex || "")
                }
            })
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
    }

    component SequencerProgramList: ColumnLayout {
        id: programRoot

        required property Theme theme
        property var rows: []
        property AppModel modelRef

        spacing: programRoot.theme.gapSmall
        Layout.fillWidth: true

        Text {
            text: qsTr("Sequencer programs")
            color: programRoot.theme.text
            textFormat: Text.PlainText
            font.pixelSize: programRoot.theme.primaryText
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        Frame {
            padding: 0
            Layout.fillWidth: true

            background: Rectangle {
                color: programRoot.theme.surface
                radius: programRoot.theme.radius
                border.width: 1
                border.color: programRoot.theme.outlineMuted
            }

            contentItem: ColumnLayout {
                spacing: 0

                SequencerProgramRow {
                    theme: programRoot.theme
                    header: true
                    columns: [qsTr("Label"), qsTr("Program ID"), qsTr("Base58")]
                }

                Repeater {
                    model: programRoot.rows

                    SequencerProgramRow {
                        required property var modelData

                        theme: programRoot.theme
                        columns: [
                            String(modelData.label || "-"),
                            programRoot.shortHash(modelData.hex),
                            programRoot.shortHash(modelData.base58)
                        ]
                        program: String(modelData.hex || modelData.base58 || "")
                        modelRef: programRoot.modelRef
                    }
                }
            }
        }

        function shortHash(value) {
            const text = String(value || "")
            if (text.length <= 16) {
                return text.length ? text : "-"
            }
            return text.slice(0, 8) + "..." + text.slice(-6)
        }
    }

    component SequencerProgramRow: Item {
        id: programRow

        required property Theme theme
        property var columns: []
        property string program: ""
        property bool header: false
        property AppModel modelRef

        Layout.fillWidth: true
        Layout.preferredHeight: programRow.header ? 36 : 42

        Rectangle {
            anchors.fill: parent
            color: programRow.header ? programRow.theme.field : "transparent"
            border.width: 0
        }

        GridLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            columns: 3
            columnSpacing: 10

            Repeater {
                model: 3

                LinkCell {
                    required property int index

                    theme: programRow.theme
                    text: String(programRow.columns[index] || "-")
                    header: programRow.header
                    link: !programRow.header && index > 0 && programRow.program.length > 0
                    copyText: programRow.program.length > 0 ? programRow.program : String(programRow.columns[index] || "")
                    monospace: index > 0 && !programRow.header
                    Layout.preferredWidth: index === 0 ? 160 : 180
                    Layout.fillWidth: index > 0
                    onActivated: programRow.modelRef.openReference("program", programRow.program)
                }
            }
        }
    }

    component TraceSummary: ColumnLayout {
        id: traceRoot

        required property Theme theme
        property var steps: []
        property var capabilities: []
        property var limitations: []
        property AppModel modelRef

        spacing: traceRoot.theme.gap
        Layout.fillWidth: true

        GridLayout {
            visible: traceRoot.capabilities.length > 0 || traceRoot.limitations.length > 0
            columns: root.width < 760 ? 1 : 2
            columnSpacing: traceRoot.theme.gap
            rowSpacing: traceRoot.theme.gap
            Layout.fillWidth: true

            TraceNote {
                visible: traceRoot.capabilities.length > 0
                theme: traceRoot.theme
                title: qsTr("Capabilities")
                rows: traceRoot.capabilities
                tone: "success"
            }

            TraceNote {
                visible: traceRoot.limitations.length > 0
                theme: traceRoot.theme
                title: qsTr("Limitations")
                rows: traceRoot.limitations
                tone: "warning"
            }
        }

        Text {
            text: qsTr("Trace steps")
            color: traceRoot.theme.text
            textFormat: Text.PlainText
            font.pixelSize: traceRoot.theme.primaryText
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        Repeater {
            model: traceRoot.steps

            TraceStepCard {
                required property var modelData

                theme: traceRoot.theme
                step: modelData
                modelRef: traceRoot.modelRef
            }
        }
    }

    component TraceNote: Frame {
        id: noteRoot

        required property Theme theme
        property string title: ""
        property var rows: []
        property string tone: "info"

        padding: noteRoot.theme.gap
        Layout.fillWidth: true

        background: Rectangle {
            color: noteRoot.tone === "warning" ? noteRoot.theme.warningMuted : noteRoot.theme.successMuted
            radius: noteRoot.theme.radius
            border.width: 1
            border.color: noteRoot.tone === "warning" ? noteRoot.theme.warning : noteRoot.theme.success
        }

        contentItem: ColumnLayout {
            spacing: noteRoot.theme.gapTiny

            Text {
                text: noteRoot.title
                color: noteRoot.theme.text
                textFormat: Text.PlainText
                font.pixelSize: noteRoot.theme.secondaryText
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            Repeater {
                model: noteRoot.rows

                Text {
                    required property string modelData

                    text: modelData
                    color: noteRoot.theme.textMuted
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: noteRoot.theme.dataText
                    Layout.fillWidth: true
                }
            }
        }
    }

    component TraceStepCard: Frame {
        id: stepRoot

        required property Theme theme
        property var step: null
        property AppModel modelRef

        padding: stepRoot.theme.gap
        Layout.fillWidth: true

        background: Rectangle {
            color: stepRoot.theme.surface
            radius: stepRoot.theme.radius
            border.width: 1
            border.color: stepRoot.severityColor(stepRoot.step ? stepRoot.step.severity : "")
        }

        contentItem: ColumnLayout {
            spacing: stepRoot.theme.gapSmall

            RowLayout {
                spacing: stepRoot.theme.gapSmall
                Layout.fillWidth: true

                Text {
                    text: stepRoot.step ? qsTr("%1. %2").arg(stepRoot.step.index).arg(stepRoot.step.label || "-") : "-"
                    color: stepRoot.theme.text
                    textFormat: Text.PlainText
                    font.pixelSize: stepRoot.theme.secondaryText
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }

                Text {
                    text: stepRoot.step ? stepRoot.valueText(stepRoot.step.status || stepRoot.step.phase) : "-"
                    color: stepRoot.theme.textMuted
                    textFormat: Text.PlainText
                    font.pixelSize: stepRoot.theme.dataText
                    font.family: "monospace"
                    horizontalAlignment: Text.AlignRight
                    Layout.preferredWidth: 120
                }
            }

            Repeater {
                model: stepRoot.detailRows()

                Text {
                    required property var modelData

                    text: String(modelData || "")
                    color: stepRoot.theme.textMuted
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: stepRoot.theme.dataText
                    Layout.fillWidth: true
                }
            }

            SectionBlock {
                visible: stepRoot.referenceRows().length > 0
                theme: stepRoot.theme
                title: qsTr("References")
                rows: stepRoot.referenceRows()
                modelRef: stepRoot.modelRef
            }
        }

        function referenceRows() {
            const refs = stepRoot.step ? stepRoot.step.refs : null
            if (!refs || typeof refs !== "object") {
                return []
            }
            const rows = []
            if (refs.program_id_hex) {
                rows.push({ label: qsTr("Program"), value: refs.program_id_hex, monospace: true, linkKind: "program", linkValue: refs.program_id_hex })
            }
            if (refs.program_id_base58) {
                rows.push({ label: qsTr("Program base58"), value: refs.program_id_base58, monospace: true, linkKind: "program", linkValue: refs.program_id_base58 })
            }
            if (refs.account_id) {
                rows.push({ label: qsTr("Account"), value: refs.account_id, monospace: true, linkKind: "account", linkValue: refs.account_id })
            }
            if (refs.instruction_word_index !== undefined && refs.instruction_word_index !== null) {
                rows.push({ label: qsTr("Instruction word"), value: stepRoot.valueText(refs.instruction_word_index), monospace: true })
            }
            if (refs.decode_path) {
                rows.push({ label: qsTr("Decode path"), value: refs.decode_path, monospace: true })
            }
            return rows
        }

        function detailRows() {
            const details = stepRoot.step ? stepRoot.step.details : null
            if (!details || details.length === undefined) {
                return []
            }
            const rows = []
            for (let i = 0; i < details.length; ++i) {
                rows.push(String(details[i] || ""))
            }
            return rows
        }

        function severityColor(value) {
            const severity = String(value || "")
            if (severity === "error") {
                return stepRoot.theme.error
            }
            if (severity === "warning") {
                return stepRoot.theme.warning
            }
            if (severity === "ok" || severity === "success") {
                return stepRoot.theme.success
            }
            return stepRoot.theme.outlineMuted
        }

        function valueText(value) {
            if (value === undefined || value === null || value === "") {
                return "-"
            }
            return String(value)
        }
    }

    component SequencerTransactionRow: Item {
        id: txRoot

        required property Theme theme
        property var columns: []
        property string hash: ""
        property string program: ""
        property bool header: false
        property AppModel modelRef

        Layout.fillWidth: true
        Layout.preferredHeight: txRoot.header ? 36 : 42

        Rectangle {
            anchors.fill: parent
            color: txRoot.header ? txRoot.theme.field : "transparent"
            border.width: 0
        }

        GridLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            columns: 4
            columnSpacing: 10

            Repeater {
                model: 4

                LinkCell {
                    required property int index

                    theme: txRoot.theme
                    text: String(txRoot.columns[index] || "-")
                    header: txRoot.header
                    link: txRoot.linkFor(index)
                    copyText: txRoot.copyValueFor(index)
                    monospace: !txRoot.header
                    Layout.preferredWidth: txRoot.columnWidth(index)
                    Layout.fillWidth: index === 1 || index === 3
                    onActivated: {
                        if (index === 1) {
                            txRoot.modelRef.openReference("transaction", txRoot.hash)
                        } else if (index === 3) {
                            txRoot.modelRef.openReference("program", txRoot.program)
                        }
                    }
                }
            }
        }

        function linkFor(index) {
            if (txRoot.header) {
                return false
            }
            if (index === 1) {
                return txRoot.hash.length > 0
            }
            if (index === 3) {
                return txRoot.program.length > 0
            }
            return false
        }

        function copyValueFor(index) {
            if (index === 1 && txRoot.hash.length > 0) {
                return txRoot.hash
            }
            if (index === 3 && txRoot.program.length > 0) {
                return txRoot.program
            }
            return String(txRoot.columns[index] || "")
        }

        function columnWidth(index) {
            if (index === 0) {
                return 68
            }
            if (index === 2) {
                return 96
            }
            return 180
        }
    }

    component SectionBlock: ColumnLayout {
        id: sectionRoot

        required property Theme theme
        property string title: ""
        property var rows: []
        property AppModel modelRef

        visible: rows.length > 0
        spacing: 6
        Layout.fillWidth: true

        Text {
            visible: sectionRoot.title.length > 0
            text: sectionRoot.title
            color: sectionRoot.theme.text
            textFormat: Text.PlainText
            font.pixelSize: sectionRoot.theme.primaryText
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        Frame {
            padding: 0
            Layout.fillWidth: true

            background: Rectangle {
                color: sectionRoot.theme.surface
                radius: sectionRoot.theme.radius
                border.width: 1
                border.color: sectionRoot.theme.outlineMuted
            }

            contentItem: ColumnLayout {
                spacing: 0

                Repeater {
                    model: sectionRoot.rows

                    DetailRow {
                        required property var modelData

                        theme: sectionRoot.theme
                        label: String(modelData.label || "")
                        value: String(modelData.value || "-")
                        linkKind: String(modelData.linkKind || "")
                        linkValue: root.model.valueToString(modelData.linkValue)
                        monospace: modelData.monospace !== undefined ? modelData.monospace : true
                        modelRef: sectionRoot.modelRef
                    }
                }
            }
        }
    }

    component DetailRow: Item {
        id: rowRoot

        required property Theme theme
        property string label: ""
        property string value: ""
        property string linkKind: ""
        property string linkValue: ""
        property bool monospace: true
        property AppModel modelRef

        Layout.fillWidth: true
        implicitHeight: Math.max(42, rowGrid.implicitHeight + 18)

        GridLayout {
            id: rowGrid

            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            anchors.topMargin: 8
            anchors.bottomMargin: 8
            columns: 2
            columnSpacing: 14
            rowSpacing: 3

            Text {
                text: rowRoot.label
                color: rowRoot.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: rowRoot.theme.labelText
                font.weight: Font.DemiBold
                font.capitalization: Font.AllUppercase
                Layout.preferredWidth: 128
                Layout.alignment: Qt.AlignTop
            }

            LinkCell {
                text: rowRoot.value
                theme: rowRoot.theme
                link: rowRoot.linkKind.length > 0 && rowRoot.linkValue.length > 0 && rowRoot.linkValue !== "-"
                copyText: rowRoot.linkValue.length > 0 ? rowRoot.linkValue : rowRoot.value
                monospace: rowRoot.monospace
                wrap: true
                Layout.fillWidth: true
                onActivated: rowRoot.modelRef.openReference(rowRoot.linkKind, rowRoot.linkValue)
            }
        }
    }
}
