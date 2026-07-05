pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQml.Models
import QtQuick.Layouts
import "../components"
import "../components/common"
import "../components/programs"
import "../state"
import "../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model
    readonly property bool hasResponse: root.model.pageHasOutput("programs")
    readonly property var responseValue: root.hasResponse ? root.model.resultValue : null

    width: parent ? parent.width : 900
    spacing: 16

    ListModel {
        id: programTabs

        ListElement { value: "programIds"; label: "Known IDs" }
        ListElement { value: "idls"; label: "IDLs" }
        ListElement { value: "binaries"; label: "Binaries" }
        ListElement { value: "events"; label: "Events" }
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / L2 LEZ / Programs")
        title: qsTr("Known L2 Programs")
        layerLabel: qsTr("L2 LEZ")
        subtitle: qsTr("Sequencer known program IDs with local SPEL / IDL bindings and binary inspection.")
        Layout.fillWidth: true
    }

    GridLayout {
        columns: root.width < 760 ? 2 : 4
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gap
        Layout.fillWidth: true

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Registered")
            value: root.numberText(root.model.registeredIdls.count)
            delta: qsTr("Local IDLs")
            deltaColor: root.model.registeredIdls.count > 0 ? root.theme.success : root.theme.textMuted
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
            label: qsTr("Sequencer")
            value: root.endpointLabel(root.model.sequencerUrl)
            delta: root.shortEndpoint(root.model.sequencerUrl)
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
        title: root.activeTabLabel()

        TabSwitch {
            theme: root.theme
            current: root.model.programTab
            options: programTabs
            onSelected: value => root.model.programTab = value
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
            sourceComponent: root.formFor(root.model.programTab)
            Layout.fillWidth: true
        }
    }

    Panel {
        theme: root.theme
        title: qsTr("Registered IDLs")

        StatusMessage {
            visible: root.model.registeredIdls.count === 0
            theme: root.theme
            tone: "info"
            title: qsTr("Registry empty")
            message: qsTr("Save an IDL to reuse it while inspecting transactions, accounts, and event payloads.")
            Layout.fillWidth: true
        }

        Frame {
            visible: root.model.registeredIdls.count > 0
            padding: 0
            Layout.fillWidth: true

            background: Rectangle {
                color: root.theme.field
                radius: root.theme.radius
                border.width: 1
                border.color: root.theme.outlineMuted
            }

            contentItem: ColumnLayout {
                spacing: 0

                Repeater {
                    model: root.model.registeredIdls

                    RegisteredIdlRow {
                        required property int index
                        required property string name
                        required property string programId
                        required property string json

                        theme: root.theme
                        idlName: name
                        programIdText: programId
                        fieldCount: root.idlFieldCount(json)
                        compact: root.width < 720
                        onRemoveRequested: root.model.removeIdl(index)
                    }
                }
            }
        }
    }

    Panel {
        visible: root.hasResponse
        theme: root.theme
        title: root.model.resultIsError ? qsTr("Program error") : qsTr("Program response")

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

        GridLayout {
            visible: !root.model.resultIsError && !root.isProgramContext(root.responseValue)
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
                label: qsTr("Instructions")
                value: root.numberText(root.idlCount("instructions"))
                delta: root.responseIdlName()
                deltaColor: root.idlCount("instructions") > 0 ? root.theme.success : root.theme.textMuted
            }

            MetricCard {
                theme: root.theme
                compact: true
                label: qsTr("Program")
                value: root.responseProgramText()
                delta: root.responseProgramDelta()
            }
        }

        ProgramIdList {
            visible: root.programRows().length > 0
            theme: root.theme
            rows: root.programTableRows()
            modelRef: root.model
        }

        IdlSummary {
            visible: root.isIdlReport(root.responseValue)
            theme: root.theme
            instructions: root.idlInstructionRows()
            accounts: root.idlAccountRows()
            warnings: root.idlWarningRows()
        }

        LinkedDetailSection {
            visible: root.isProgramFile(root.responseValue)
            theme: root.theme
            title: qsTr("Program file")
            rows: root.programFileRows()
            onLinkActivated: (kind, value) => root.model.openReference(kind, value)
        }

        ProgramContextSummary {
            visible: root.isProgramContext(root.responseValue)
            theme: root.theme
            rows: root.programContextRows()
            idls: root.programContextIdlRows()
            transactions: root.programContextTransactionRows()
            account: root.programContextAccount()
            rawText: root.model.resultText
            modelRef: root.model
        }

        TextArea {
            visible: !root.isProgramContext(root.responseValue)
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

    function formFor(tab) {
        switch (tab) {
        case "binaries":
            return binaryForm
        case "idls":
            return idlForm
        case "events":
            return eventForm
        default:
            return programIdsForm
        }
    }

    Component {
        id: programIdsForm

        ColumnLayout {
            spacing: 12

            SourceStrip {
                theme: root.theme
                sources: [qsTr("L2 LEZ"), qsTr("sequencer known table"), qsTr("program id")]
                Layout.fillWidth: true
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Load known IDs")
                primary: true
                enabled: !root.model.busy
                Layout.preferredWidth: 190
                onClicked: root.model.callInspector("programs", root.model.executionRpcArgs([]), qsTr("Known program IDs"))
            }
        }
    }

    Component {
        id: idlForm

        ColumnLayout {
            spacing: 12

            GridLayout {
                columns: root.width < 680 ? 1 : 2
                columnSpacing: root.theme.gap
                rowSpacing: root.theme.gap
                Layout.fillWidth: true

                FieldRow {
                    id: programId
                    theme: root.theme
                    label: qsTr("Program ID")
                    placeholderText: qsTr("Required hex or base58")
                    Layout.fillWidth: true
                }

                FieldRow {
                    id: idlName
                    theme: root.theme
                    label: qsTr("IDL name")
                    placeholderText: qsTr("Auto from JSON")
                    Layout.fillWidth: true
                }

                FieldRow {
                    id: idlProgramBinary
                    theme: root.theme
                    label: qsTr("Program binary")
                    placeholderText: qsTr("Required for private tx")
                    Layout.fillWidth: true
                }
            }

            TextAreaField {
                id: idlJson
                theme: root.theme
                label: qsTr("IDL JSON")
                rows: 8
            }

            GridLayout {
                columns: root.width < 680 ? 1 : 3
                columnSpacing: root.theme.gapSmall
                rowSpacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Save IDL")
                    primary: true
                    enabled: idlJson.text.trim().length > 0 && root.validProgramId(programId.text)
                    Layout.fillWidth: true
                    onClicked: root.model.registerIdl(idlName.text, programId.text, idlJson.text, idlProgramBinary.text)
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Summarize")
                    enabled: !root.model.busy && idlJson.text.trim().length > 0
                    Layout.fillWidth: true
                    onClicked: root.model.callInspector("spelIdl", [idlJson.text], qsTr("SPEL IDL"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Load known IDs")
                    enabled: !root.model.busy
                    Layout.fillWidth: true
                    onClicked: root.model.callInspector("programs", root.model.executionRpcArgs([]), qsTr("Known program IDs"))
                }
            }
        }
    }

    Component {
        id: binaryForm

        ColumnLayout {
            spacing: 12

            FileDialog {
                id: programFileDialog

                title: qsTr("Select program binary")
                fileMode: FileDialog.OpenFile
                nameFilters: [qsTr("Binary files (*.bin *.wasm)"), qsTr("All files (*)")]
                onAccepted: {
                    const path = root.localPathFromFileUrl(selectedFile)
                    if (path.length > 0) {
                        programPath.text = path
                    }
                }
            }

            ColumnLayout {
                spacing: 6
                Layout.fillWidth: true

                Text {
                    text: qsTr("Path")
                    color: root.theme.textMuted
                    textFormat: Text.PlainText
                    font.pixelSize: root.theme.secondaryText
                    font.weight: Font.Medium
                    Layout.fillWidth: true
                }

                RowLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    TextField {
                        id: programPath

                        color: root.theme.text
                        placeholderText: qsTr("program.bin")
                        placeholderTextColor: root.theme.textDim
                        selectionColor: root.theme.accent
                        selectedTextColor: root.theme.selectedText
                        font.pixelSize: root.theme.primaryText
                        leftPadding: 12
                        rightPadding: 12
                        hoverEnabled: true
                        Layout.fillWidth: true
                        Layout.preferredHeight: root.theme.controlHeight

                        background: Rectangle {
                            radius: root.theme.radius
                            color: programPath.hovered || programPath.activeFocus ? root.theme.surfaceRaised : root.theme.field
                            border.width: programPath.activeFocus ? 2 : 1
                            border.color: programPath.activeFocus ? root.theme.accent : root.theme.outlineMuted
                        }

                        Accessible.name: qsTr("Program binary path")
                    }

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Browse")
                        enabled: !root.model.busy
                        Layout.preferredWidth: 96
                        onClicked: programFileDialog.open()
                    }
                }
            }

            RowLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Inspect")
                    primary: true
                    enabled: !root.model.busy && programPath.text.trim().length > 0
                    Layout.preferredWidth: 124
                    onClicked: root.model.callInspector("programFile", [programPath.text], qsTr("Program file"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Deploy")
                    enabled: !root.model.busy && programPath.text.trim().length > 0 && root.model.walletProfileConfigured()
                    Layout.preferredWidth: 124
                    onClicked: deployProgramConfirm.open()
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Wallet")
                    enabled: !root.model.busy
                    Layout.preferredWidth: 104
                    onClicked: root.model.openLocalWallet("", "profiles")
                }

                Item {
                    Layout.fillWidth: true
                }
            }

            StatusMessage {
                theme: root.theme
                tone: root.model.walletProfileConfigured() ? "info" : "warning"
                title: root.model.walletProfileConfigured() ? qsTr("Deploy ready") : qsTr("Local wallet required")
                message: root.model.walletProfileConfigured()
                    ? qsTr("Deployment uses the configured local wallet and writes through wallet deploy-program.")
                    : qsTr("Configure wallet binary and wallet home before deploying program binaries.")
                Layout.fillWidth: true
            }

            ConfirmActionPopup {
                id: deployProgramConfirm

                theme: root.theme
                title: qsTr("Deploy program")
                message: qsTr("This runs the configured local wallet deploy-program command for %1.").arg(root.shortPath(programPath.text))
                confirmText: qsTr("Deploy")
                confirmEnabled: !root.model.busy && programPath.text.trim().length > 0 && root.model.walletProfileConfigured()
                onAccepted: root.model.deployProgramBinary(programPath.text)
            }
        }
    }

    Component {
        id: eventForm

        ColumnLayout {
            spacing: 12

            FieldRow {
                id: eventName
                theme: root.theme
                label: qsTr("Event")
                placeholderText: qsTr("Optional event name")
            }

            TextAreaField {
                id: eventData
                theme: root.theme
                label: qsTr("Event data hex")
                rows: 4
            }

            TextAreaField {
                id: eventIdl
                theme: root.theme
                label: qsTr("IDL JSON")
                rows: 7
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Decode event")
                primary: true
                enabled: !root.model.busy && eventData.text.trim().length > 0 && eventIdl.text.trim().length > 0
                Layout.preferredWidth: 140
                onClicked: root.model.callInspector("decodeEvent", [eventData.text, eventIdl.text, eventName.text], qsTr("Event decode"))
            }
        }
    }

    function activeTabLabel() {
        if (root.model.programTab === "programIds") {
            return qsTr("Known IDs")
        }
        if (root.model.programTab === "binaries") {
            return qsTr("Binaries")
        }
        if (root.model.programTab === "events") {
            return qsTr("Events")
        }
        return qsTr("IDLs")
    }

    function activeTabDelta() {
        if (root.model.programTab === "programIds") {
            return qsTr("Static table")
        }
        if (root.model.programTab === "binaries") {
            return qsTr("File inspection")
        }
        if (root.model.programTab === "events") {
            return qsTr("Event decode")
        }
        return qsTr("Registry")
    }

    function activeTabMessage() {
        if (root.model.programTab === "programIds") {
            return qsTr("Load the sequencer known-program table before binding local IDLs or binaries.")
        }
        if (root.model.programTab === "binaries") {
            return qsTr("Inspect compiled program bytecode, then deploy it with the configured local wallet.")
        }
        if (root.model.programTab === "events") {
            return qsTr("Decode event payloads with a user-provided IDL. Program-specific decoding stays local to the supplied IDL.")
        }
        return qsTr("Save local IDLs, summarize their instruction/account shape, or load program IDs from the sequencer.")
    }

    function validProgramId(value) {
        const text = String(value || "").trim()
        return text.length > 0 && root.model.canonicalProgramIdHex(text).length > 0
    }

    function lastResultText() {
        if (!root.hasResponse) {
            return qsTr("Idle")
        }
        return root.model.resultIsError ? qsTr("Error") : qsTr("OK")
    }

    function lastResultDelta() {
        if (!root.hasResponse) {
            return qsTr("No output")
        }
        return root.model.resultTitle.length ? root.model.resultTitle : qsTr("Program call")
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
            return root.numberText(Object.keys(value).length)
        }
        return root.valueText(value)
    }

    function responseKindText() {
        const value = root.responseValue
        if (Array.isArray(value)) {
            return qsTr("Array items")
        }
        if (value && typeof value === "object") {
            return qsTr("Object fields")
        }
        return qsTr("Scalar value")
    }

    function responseIdlName() {
        const value = root.responseValue
        if (value && typeof value === "object" && value.name !== undefined) {
            return root.valueText(value.name)
        }
        return qsTr("IDL summary")
    }

    function responseProgramText() {
        const value = root.responseValue
        if (Array.isArray(value)) {
            return root.numberText(value.length)
        }
        if (root.isProgramContext(value)) {
            return root.shortHash(value.program_id_base58 || value.program_id_hex || value.program_id)
        }
        if (root.isProgramFile(value)) {
            return root.shortHash(value.program_id_hex)
        }
        return "-"
    }

    function responseProgramDelta() {
        const value = root.responseValue
        if (Array.isArray(value)) {
            return qsTr("Known program IDs")
        }
        if (root.isProgramContext(value)) {
            return value.in_chain ? qsTr("verified in chain") : qsTr("not verified")
        }
        if (root.isProgramFile(value)) {
            return qsTr("%1 bytes").arg(root.numberText(value.bytecode_len))
        }
        return qsTr("Sequencer")
    }

    function programRows() {
        return Array.isArray(root.responseValue) ? root.responseValue : []
    }

    function programTableRows() {
        return root.programRows().map(function (row) {
            const hex = String(row.hex || "")
            const base58 = String(row.base58 || "")
            return {
                label: String(row.label || "-"),
                hex: hex,
                base58: base58,
                programIdText: base58.length ? base58 : hex,
                knownIdl: root.knownIdlText(hex)
            }
        })
    }

    function isProgramContext(value) {
        return value && typeof value === "object" && !Array.isArray(value)
            && value.type === "program"
            && value.program_id !== undefined
    }

    function programContextRows() {
        const value = root.responseValue || {}
        if (!root.isProgramContext(value)) {
            return []
        }
        const programId = root.valueText(value.program_id)
        const programHex = root.programHexText(value.program_id_hex)
        const programBase58 = root.valueText(value.program_id_base58)
        const accountLookup = programBase58 !== "-" ? programBase58 : programHex
        const verified = value.in_chain === true
        const rows = [
            { label: qsTr("Known program"), value: root.programVerificationText(value), linkKind: "" },
            { label: qsTr("Program ID"), value: programBase58 !== "-" ? programBase58 : programId, linkKind: verified ? "program" : "" },
            { label: qsTr("Program ID (0x)"), value: programHex, linkKind: verified ? "program" : "" },
            { label: qsTr("Inspect as account"), value: accountLookup, linkKind: accountLookup !== "-" ? "account" : "" },
            { label: qsTr("Sequencer label"), value: root.valueText(value.known_label), linkKind: "" }
        ]
        if (value.verification_detail !== undefined && String(value.verification_detail || "").length > 0) {
            rows.push({ label: qsTr("Verification error"), value: String(value.verification_detail || ""), linkKind: "" })
        }
        return rows
    }

    function programVerificationText(value) {
        if (!root.isProgramContext(value)) {
            return "-"
        }
        if (value.in_chain === true) {
            return qsTr("yes")
        }
        if (String(value.verification || "") === "unavailable") {
            return qsTr("verification unavailable")
        }
        return qsTr("not in getProgramIds")
    }

    function programHexText(value) {
        const text = String(value || "").replace(/^0x/i, "")
        return text.length ? "0x" + text : "-"
    }

    function programContextIdlRows() {
        const value = root.responseValue || {}
        const entries = root.isProgramContext(value) && Array.isArray(value.idls) ? value.idls : []
        return entries.map(function (entry) {
            const json = String(entry.json || "")
            return {
                title: root.valueText(entry.name || entry.programId || entry.programIdHex),
                detail: qsTr("%1 field(s), program %2").arg(root.numberText(root.idlFieldCount(json))).arg(root.shortHash(entry.programId || entry.programIdHex))
            }
        })
    }

    function programContextTransactionRows() {
        const value = root.responseValue || {}
        const rows = root.isProgramContext(value) && Array.isArray(value.recent_transactions) ? value.recent_transactions : []
        return rows.slice(0, 8).map(function (tx) {
            return {
                title: root.shortHash(tx.hash),
                detail: qsTr("block %1, %2, %3 word(s)").arg(root.valueText(tx.block_id)).arg(root.valueText(tx.kind)).arg(root.numberText(tx.ops))
            }
        })
    }

    function programContextAccount() {
        const value = root.responseValue || {}
        return root.isProgramContext(value) && value.account && typeof value.account === "object" ? value.account : null
    }

    function knownIdlText(programId) {
        const entries = root.model.idlEntriesForProgram(programId)
        if (entries.length > 0) {
            return entries[0].name || qsTr("registered")
        }
        return qsTr("none")
    }

    function isIdlReport(value) {
        return value && typeof value === "object" && !Array.isArray(value)
            && value.instructions !== undefined
            && value.accounts !== undefined
            && value.counts !== undefined
    }

    function isProgramFile(value) {
        return value && typeof value === "object" && !Array.isArray(value)
            && value.program_id_hex !== undefined
            && value.deployment_tx_hash !== undefined
    }

    function idlCount(key) {
        const value = root.responseValue
        if (root.isIdlReport(value) && value.counts && value.counts[key] !== undefined) {
            return Number(value.counts[key] || 0)
        }
        return 0
    }

    function idlInstructionRows() {
        const value = root.responseValue
        const instructions = root.isIdlReport(value) && Array.isArray(value.instructions) ? value.instructions : []
        return instructions.slice(0, 6).map(function (item) {
            const args = Array.isArray(item.args) ? item.args.length : 0
            const accounts = Array.isArray(item.accounts) ? item.accounts.length : 0
            return {
                title: root.valueText(item.name),
                detail: qsTr("%1 instruction account role(s), %2 arg(s)").arg(root.numberText(accounts)).arg(root.numberText(args))
            }
        })
    }

    function idlAccountRows() {
        const value = root.responseValue
        const accounts = root.isIdlReport(value) && Array.isArray(value.accounts) ? value.accounts : []
        return accounts.slice(0, 6).map(function (item) {
            return {
                title: root.valueText(item.name),
                detail: root.valueText(item.type_label)
            }
        })
    }

    function idlWarningRows() {
        const value = root.responseValue
        const warnings = root.isIdlReport(value) && Array.isArray(value.warnings) ? value.warnings : []
        return warnings.slice(0, 4).map(function (item) {
            return {
                title: qsTr("Warning"),
                detail: root.valueText(item)
            }
        })
    }

    function programFileRows() {
        const value = root.responseValue || {}
        if (!root.isProgramFile(value)) {
            return []
        }
        const rows = [
            { label: qsTr("Path"), value: root.valueText(value.path), linkKind: "" },
            { label: qsTr("Bytecode"), value: qsTr("%1 bytes").arg(root.numberText(value.bytecode_len)), linkKind: "" },
            { label: qsTr("Program ID (0x)"), value: root.valueText(value.program_id_hex), linkKind: "program" },
            { label: qsTr("Program ID"), value: root.valueText(value.program_id_base58), linkKind: "" },
            { label: qsTr("Deployment tx"), value: root.valueText(value.deployment_tx_hash), linkKind: "transaction" }
        ]
        if (String(value.source || "") === "local_wallet_cli") {
            rows.unshift({ label: qsTr("Deploy status"), value: root.valueText(value.status), linkKind: "" })
            rows.push({ label: qsTr("Wallet command"), value: root.valueText(value.command), linkKind: "" })
            rows.push({ label: qsTr("Wallet home"), value: root.valueText(value.wallet_home_source), linkKind: "" })
            rows.push({ label: qsTr("Submitted at"), value: root.valueText(value.submitted_at), linkKind: "" })
            rows.push({ label: qsTr("Exit status"), value: root.valueText(value.exit_status), linkKind: "" })
            if (String(value.stdout || "").length > 0) {
                rows.push({ label: qsTr("stdout"), value: String(value.stdout || ""), linkKind: "" })
            }
            if (String(value.stderr || "").length > 0) {
                rows.push({ label: qsTr("stderr"), value: String(value.stderr || ""), linkKind: "" })
            }
        }
        return rows
    }

    function idlFieldCount(json) {
        try {
            const parsed = JSON.parse(json || "{}")
            return parsed && typeof parsed === "object" ? Object.keys(parsed).length : 0
        } catch (error) {
            return 0
        }
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
            return qsTr("Not configured")
        }
        return text.replace(/^https?:\/\//, "").replace(/\/$/, "")
    }

    function shortHash(value) {
        const text = String(value || "")
        if (text.length <= 16) {
            return text.length ? text : "-"
        }
        return text.slice(0, 8) + "..." + text.slice(-6)
    }

    function shortPath(value) {
        const text = String(value || "").trim()
        if (!text.length) {
            return qsTr("the selected binary")
        }
        if (text.length <= 48) {
            return text
        }
        return "..." + text.slice(-45)
    }

    function localPathFromFileUrl(fileUrl) {
        const text = String(fileUrl || "")
        if (!text.length) {
            return ""
        }
        if (text.indexOf("file://") === 0) {
            let path = decodeURIComponent(text.slice(7))
            if (/^\/[A-Za-z]:\//.test(path)) {
                path = path.slice(1)
            }
            return path
        }
        return text
    }

    function valueText(value) {
        if (value === undefined || value === null || value === "") {
            return "-"
        }
        if (typeof value === "number") {
            return value % 1 === 0 ? value.toLocaleString(Qt.locale(), "f", 0) : String(value)
        }
        if (typeof value === "object") {
            return JSON.stringify(value)
        }
        return String(value)
    }

    function numberText(value) {
        if (value === undefined || value === null || value === "") {
            return "-"
        }
        const numeric = Number(value)
        if (Number.isFinite(numeric)) {
            return numeric % 1 === 0 ? numeric.toLocaleString(Qt.locale(), "f", 0) : String(value)
        }
        return String(value)
    }
}
