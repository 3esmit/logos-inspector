pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../components"
import "../../../components/common"
import "../../../state/domains/ZoneInspectionContract.js" as ZoneInspectionContract
import "../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property var appModel: null
    property var zoneState: null
    property var zoneDetail: null
    readonly property var execution: root.appModel
        && root.appModel.programExecution !== undefined
        ? root.appModel.programExecution : null
    readonly property var plan: root.execution
        ? root.execution.idlInstructionPlanValue : null
    readonly property var preview: root.execution
        ? root.execution.idlInstructionPreviewValue : null
    readonly property var receipt: root.execution
        ? root.execution.idlInstructionReceipt : null

    property string selectedIdlKey: ""
    property string selectedInstruction: ""
    property var registeredIdlOptions: []
    property var instructionNames: []
    property var renderedPlan: null
    property var accountValues: ({})
    property var argumentValues: ({})
    property bool confirmationAccepted: false

    signal configureIdlsRequested()
    signal transactionRequested(string transactionId, string exactSourceId)

    objectName: "zoneL2ProgramInteraction"
    spacing: root.theme.gapLarge
    Layout.fillWidth: true

    Component.onCompleted: {
        root.refreshRegisteredIdlOptions()
        root.ensureRegisteredIdlSelection()
    }
    onZoneDetailChanged: root.invalidateForCurrentContext()

    Connections {
        target: root.execution
        enabled: root.execution !== null
        ignoreUnknownSignals: true

        function onIdlInstructionPlanValueChanged() {
            root.acceptInstructionPlan()
        }
    }

    Connections {
        target: root.appModel && root.appModel.registeredIdls !== undefined
            ? root.appModel.registeredIdls : null
        enabled: target !== null
        ignoreUnknownSignals: true

        function onCountChanged() {
            root.refreshRegisteredIdlOptions()
            root.ensureRegisteredIdlSelection()
        }
    }

    Connections {
        target: root.zoneState
        enabled: root.zoneState !== null
        ignoreUnknownSignals: true

        function onActiveZoneContextChanged() {
            root.invalidateForCurrentContext()
        }
    }

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true
        Layout.minimumWidth: 0

        ColumnLayout {
            spacing: root.theme.gapTiny
            Layout.fillWidth: true
            Layout.minimumWidth: 0

            Text {
                text: qsTr("Interact with a registered program")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: root.theme.panelTitleText
                font.weight: Font.DemiBold
                Layout.fillWidth: true
                Layout.minimumWidth: 0
            }

            Text {
                text: qsTr("Build from a user-registered IDL and submit only to this Zone's selected Sequencer.")
                color: root.theme.textMuted
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
                Layout.minimumWidth: 0
            }
        }

        ZoneKindChip {
            theme: root.theme
            label: root.privateDraft() ? qsTr("Private transaction") : qsTr("Public transaction")
            tone: root.privateDraft() ? "warning" : "info"
        }
    }

    StatusMessage {
        visible: root.execution === null
        theme: root.theme
        tone: "warning"
        title: qsTr("Interaction unavailable")
        message: qsTr("Program execution state is not available on this screen.")
        Layout.fillWidth: true
    }

    ColumnLayout {
        visible: root.execution !== null && root.registeredIdlCount() === 0
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        StatusMessage {
            theme: root.theme
            tone: "info"
            title: qsTr("Register an IDL first")
            message: qsTr("Registered IDLs define instruction names, account roles, and typed arguments. Known Programs does not replace an IDL.")
            Layout.fillWidth: true
        }

        ActionButton {
            objectName: "zoneProgramOpenIdlsButton"
            theme: root.theme
            text: qsTr("Open IDL registry")
            Layout.preferredWidth: 160
            onClicked: root.configureIdlsRequested()
        }
    }

    GridLayout {
        objectName: "zoneProgramSelectorGrid"
        visible: root.execution !== null && root.registeredIdlCount() > 0
        columns: root.width < 720 ? 1 : 2
        columnSpacing: root.theme.gapSmall
        rowSpacing: root.theme.gapSmall
        Layout.fillWidth: true
        Layout.minimumWidth: 0

        ColumnLayout {
            spacing: root.theme.gapTiny
            Layout.fillWidth: true
            Layout.minimumWidth: 0

            Text {
                text: qsTr("Registered IDL")
                color: root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
                font.weight: Font.Medium
                Layout.fillWidth: true
                Layout.minimumWidth: 0
            }

            ComboBox {
                id: idlSelector

                objectName: "zoneProgramIdlSelector"
                model: root.registeredIdlOptions
                enabled: !root.submitPending()
                hoverEnabled: true
                Accessible.name: qsTr("Registered IDL")
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                Layout.preferredHeight: root.theme.controlHeight
                onActivated: index => root.selectRegisteredIdl(index)

                contentItem: Text {
                    text: idlSelector.displayText
                    color: idlSelector.enabled ? root.theme.text : root.theme.textDim
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    verticalAlignment: Text.AlignVCenter
                    leftPadding: 12
                    rightPadding: 32
                    font.pixelSize: root.theme.primaryText
                }

                background: Rectangle {
                    radius: root.theme.radius
                    color: idlSelector.hovered || idlSelector.activeFocus
                        ? root.theme.surfaceRaised : root.theme.field
                    border.width: idlSelector.activeFocus ? 2 : 1
                    border.color: idlSelector.activeFocus
                        ? root.theme.accent : root.theme.outlineMuted
                }
            }
        }

        ColumnLayout {
            spacing: root.theme.gapTiny
            Layout.fillWidth: true
            Layout.minimumWidth: 0

            Text {
                text: qsTr("Instruction")
                color: root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
                font.weight: Font.Medium
                Layout.fillWidth: true
                Layout.minimumWidth: 0
            }

            ComboBox {
                id: instructionSelector

                objectName: "zoneProgramInstructionSelector"
                model: root.instructionNames
                currentIndex: root.instructionNames.indexOf(root.selectedInstruction)
                enabled: root.instructionNames.length > 0
                    && !root.planPending() && !root.submitPending()
                hoverEnabled: true
                Accessible.name: qsTr("IDL instruction")
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                Layout.preferredHeight: root.theme.controlHeight
                onActivated: index => root.selectInstruction(index)

                contentItem: Text {
                    text: instructionSelector.displayText
                    color: instructionSelector.enabled ? root.theme.text : root.theme.textDim
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    verticalAlignment: Text.AlignVCenter
                    leftPadding: 12
                    rightPadding: 32
                    font.pixelSize: root.theme.primaryText
                }

                background: Rectangle {
                    radius: root.theme.radius
                    color: instructionSelector.hovered || instructionSelector.activeFocus
                        ? root.theme.surfaceRaised : root.theme.field
                    border.width: instructionSelector.activeFocus ? 2 : 1
                    border.color: instructionSelector.activeFocus
                        ? root.theme.accent : root.theme.outlineMuted
                }
            }
        }
    }

    StatusMessage {
        visible: root.planPending()
        theme: root.theme
        tone: "info"
        title: qsTr("Reading IDL instruction")
        message: qsTr("Deriving account roles and typed arguments from the selected IDL.")
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.execution !== null
            && String(root.execution.idlInstructionPlanError || "").length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Instruction plan unavailable")
        message: root.execution
            ? String(root.execution.idlInstructionPlanError || "") : ""
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.execution !== null && root.registeredIdlCount() > 0
            && !root.targetDisplayReady()
        theme: root.theme
        tone: "warning"
        title: qsTr("Sequencer target refreshing")
        message: qsTr("Wait for this Zone's current source revision and exact RPC endpoint before previewing or sending.")
        Layout.fillWidth: true
    }

    GridLayout {
        objectName: "zoneProgramFieldsGrid"
        visible: root.renderedPlan !== null && root.selectedInstruction.length > 0
        columns: root.width < 720 ? 1 : 2
        columnSpacing: root.theme.gapSmall
        rowSpacing: root.theme.gapSmall
        Layout.fillWidth: true

        Repeater {
            model: root.renderedPlan && Array.isArray(root.renderedPlan.accounts)
                ? root.renderedPlan.accounts : []

            delegate: FieldRow {
                required property var modelData

                objectName: "zoneProgramAccount_" + root.objectPart(modelData.name)
                theme: root.theme
                label: String(modelData.label || modelData.name || qsTr("Account"))
                placeholderText: String(modelData.placeholder || "Public/<id>")
                sourceText: root.fieldValue("accounts", modelData.name)
                syncSourceText: true
                enabled: !root.submitPending()
                Layout.fillWidth: true
                onTextEdited: function (value) {
                    root.setFieldValue("accounts", String(modelData.name || ""), value)
                }
            }
        }

        Repeater {
            model: root.renderedPlan && Array.isArray(root.renderedPlan.args)
                ? root.renderedPlan.args : []

            delegate: FieldRow {
                required property var modelData

                objectName: "zoneProgramArgument_" + root.objectPart(modelData.name)
                theme: root.theme
                label: String(modelData.label || modelData.name || qsTr("Argument"))
                placeholderText: String(modelData.placeholder || qsTr("Value"))
                sourceText: root.fieldValue("args", modelData.name)
                syncSourceText: true
                enabled: !root.submitPending()
                Layout.fillWidth: true
                onTextEdited: function (value) {
                    root.setFieldValue("args", String(modelData.name || ""), value)
                }
            }
        }
    }

    RowLayout {
        visible: root.renderedPlan !== null && root.selectedInstruction.length > 0
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        ActionButton {
            objectName: "zoneProgramPreviewButton"
            theme: root.theme
            text: qsTr("Preview")
            primary: !root.previewCurrent()
            enabled: root.planReady() && root.targetDisplayReady()
                && !root.previewPending() && !root.submitPending()
            Layout.preferredWidth: 116
            onClicked: root.previewInstruction()
        }

        ActionButton {
            objectName: "zoneProgramSendButton"
            theme: root.theme
            text: qsTr("Review & Send")
            primary: root.previewCurrent()
            enabled: root.previewCurrent() && !root.submitPending()
            Layout.preferredWidth: 148
            onClicked: root.openConfirmation()
        }

        Item {
            Layout.fillWidth: true
        }
    }

    StatusMessage {
        visible: root.previewPending()
        theme: root.theme
        tone: "info"
        title: qsTr("Building preview")
        message: qsTr("Resolving the exact instruction bytes, account roles, and transaction mode without submitting.")
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.execution !== null
            && String(root.execution.idlInstructionError || "").length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Instruction unavailable")
        message: root.execution
            ? String(root.execution.idlInstructionError || "") : ""
        Layout.fillWidth: true
    }

    GridLayout {
        visible: root.previewCurrent()
        columns: root.width < 720 ? 1 : 2
        columnSpacing: root.theme.gapXLarge
        rowSpacing: root.theme.gapLarge
        Layout.fillWidth: true

        ZoneFactSection {
            theme: root.theme
            title: qsTr("Frozen preview")
            rows: root.previewFactRows()
        }

        ZoneFactSection {
            theme: root.theme
            title: qsTr("Exact target")
            rows: root.previewTargetRows()
        }
    }

    DataTableFrame {
        visible: root.previewCurrent() && root.previewAccountRows().length > 0
        objectName: "zoneProgramPreviewAccountsTable"
        theme: root.theme
        headerCells: [
            { text: qsTr("Account"), width: 150 },
            { text: qsTr("ID"), width: 300, fill: true },
            { text: qsTr("Role"), width: 120, monospace: false }
        ]
        rows: root.previewAccountRows()
        Layout.fillWidth: true
    }

    DataTableFrame {
        visible: root.previewCurrent() && root.previewArgumentRows().length > 0
        objectName: "zoneProgramPreviewArgumentsTable"
        theme: root.theme
        headerCells: [
            { text: qsTr("Argument"), width: 170 },
            { text: qsTr("Type"), width: 120, monospace: false },
            { text: qsTr("Value"), width: 260, fill: true }
        ]
        rows: root.previewArgumentRows()
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.submitPending()
        theme: root.theme
        tone: "info"
        title: qsTr("Submitting transaction")
        message: qsTr("The frozen preview is being submitted to its verified Sequencer target.")
        Layout.fillWidth: true
    }

    ColumnLayout {
        visible: root.receipt !== null
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        StatusMessage {
            theme: root.theme
            tone: root.receiptMatchesCurrentTarget() ? "success" : "warning"
            title: qsTr("Transaction submitted")
            message: root.receiptMatchesCurrentTarget()
                ? qsTr("Transaction %1 was accepted by source %2. Exact-source readback requested.")
                    .arg(root.shortValue(root.receipt && root.receipt.tx_hash))
                    .arg(root.shortValue(root.receiptSourceId()))
                : qsTr("Transaction %1 was accepted by source %2, but the active Zone or Sequencer changed. Return to that exact source to inspect it.")
                    .arg(root.shortValue(root.receipt && root.receipt.tx_hash))
                    .arg(root.shortValue(root.receiptSourceId()))
            Layout.fillWidth: true
        }

        ActionButton {
            objectName: "zoneProgramOpenReceiptButton"
            theme: root.theme
            text: qsTr("Open transaction")
            enabled: root.receiptMatchesCurrentTarget()
            Layout.preferredWidth: 152
            onClicked: root.emitReceiptTransaction()
        }
    }

    ConfirmActionPopup {
        id: sendConfirmation

        objectName: "zoneProgramSendConfirmation"
        theme: root.theme
        title: qsTr("Send program transaction")
        message: root.confirmationMessage()
        confirmText: qsTr("Send transaction")
        confirmEnabled: root.execution !== null
            && root.execution.idlInstructionConfirmation !== null
            && !root.submitPending()
        onAccepted: {
            root.confirmationAccepted = true
            root.confirmInstruction()
        }
        onClosed: Qt.callLater(function () {
            if (!root.confirmationAccepted && root.execution) {
                root.execution.cancelIdlInstructionConfirmation()
            }
            root.confirmationAccepted = false
        })
    }

    function registeredIdlCount() {
        return root.appModel && root.appModel.registeredIdls !== undefined
            ? Number(root.appModel.registeredIdls.count || 0) : 0
    }

    function registeredIdlLabels() {
        const rows = []
        for (let index = 0; index < root.registeredIdlCount(); ++index) {
            const entry = root.idlEntryAt(index)
            const name = String(entry && entry.name || qsTr("IDL %1").arg(index + 1))
            const program = String(entry && entry.programIdHex || "")
            rows.push(program.length > 0
                ? qsTr("%1 · %2").arg(name).arg(root.shortValue(program)) : name)
        }
        return rows
    }

    function refreshRegisteredIdlOptions() {
        root.registeredIdlOptions = root.registeredIdlLabels()
        idlSelector.currentIndex = root.registeredIdlIndex(root.selectedIdlKey)
    }

    function idlEntryAt(index) {
        return root.appModel && typeof root.appModel.idlEntryAt === "function"
            ? root.appModel.idlEntryAt(Number(index)) : null
    }

    function idlEntryForKey(key) {
        if (!root.appModel) {
            return null
        }
        if (typeof root.appModel.idlEntryForKey === "function") {
            return root.appModel.idlEntryForKey(String(key || ""))
        }
        for (let index = 0; index < root.registeredIdlCount(); ++index) {
            const entry = root.idlEntryAt(index)
            if (String(entry && entry.key || "") === String(key || "")) {
                return entry
            }
        }
        return null
    }

    function registeredIdlIndex(key) {
        for (let index = 0; index < root.registeredIdlCount(); ++index) {
            if (String(root.idlEntryAt(index) && root.idlEntryAt(index).key || "")
                    === String(key || "")) {
                return index
            }
        }
        return root.registeredIdlCount() > 0 ? 0 : -1
    }

    function ensureRegisteredIdlSelection() {
        if (!root.execution) {
            return
        }
        if (root.registeredIdlCount() === 0) {
            root.selectedIdlKey = ""
            root.selectedInstruction = ""
            root.instructionNames = []
            root.renderedPlan = null
            root.accountValues = ({})
            root.argumentValues = ({})
            root.execution.reviseIdlInstructionDraft(null, {}, root.currentTargetDisplay())
            return
        }
        if (root.idlEntryForKey(root.selectedIdlKey)) {
            return
        }
        root.selectRegisteredIdl(0)
    }

    function selectRegisteredIdl(index) {
        const entry = root.idlEntryAt(index)
        if (!entry || !String(entry.key || "").length) {
            return false
        }
        root.cancelOpenConfirmation()
        idlSelector.currentIndex = index
        root.selectedIdlKey = String(entry.key)
        root.selectedInstruction = ""
        root.instructionNames = []
        root.renderedPlan = null
        root.accountValues = ({})
        root.argumentValues = ({})
        root.reviseAndPlan()
        return true
    }

    function selectInstruction(index) {
        if (index < 0 || index >= root.instructionNames.length) {
            return false
        }
        root.cancelOpenConfirmation()
        root.selectedInstruction = String(root.instructionNames[index] || "")
        root.renderedPlan = null
        root.accountValues = ({})
        root.argumentValues = ({})
        root.reviseAndPlan()
        return true
    }

    function acceptInstructionPlan() {
        if (!root.plan || !Array.isArray(root.plan.instructions)) {
            return
        }
        if ((root.renderedPlan === null && root.selectedInstruction.length > 0)
                || root.planSchemaKey(root.renderedPlan)
                !== root.planSchemaKey(root.plan)) {
            root.renderedPlan = root.plan
        }
        root.instructionNames = root.plan.instructions.map(function (name) {
            return String(name || "")
        }).filter(function (name) {
            return name.length > 0
        })
        if (!root.selectedInstruction.length && root.instructionNames.length > 0) {
            root.selectedInstruction = root.instructionNames[0]
            root.accountValues = ({})
            root.argumentValues = ({})
            root.reviseAndPlan()
        }
    }

    function draftRequest() {
        const entry = root.idlEntryForKey(root.selectedIdlKey) || ({})
        return {
            idlJson: String(entry.json || ""),
            programIdHex: String(entry.programIdHex || ""),
            programBinary: String(entry.programBinary || ""),
            dependencyBinaries: [],
            instruction: root.selectedInstruction,
            accounts: root.copyMap(root.accountValues),
            args: root.copyMap(root.argumentValues)
        }
    }

    function reviseAndPlan() {
        if (!root.execution) {
            return null
        }
        const entry = root.idlEntryForKey(root.selectedIdlKey)
        root.execution.reviseIdlInstructionDraft(
            entry,
            root.draftRequest(),
            root.currentTargetDisplay())
        return root.execution.planIdlInstruction()
    }

    function setFieldValue(kind, name, value) {
        if (!String(name || "").length || root.submitPending()) {
            return false
        }
        root.cancelOpenConfirmation()
        const values = root.copyMap(kind === "accounts"
            ? root.accountValues : root.argumentValues)
        values[String(name)] = String(value || "")
        if (kind === "accounts") {
            root.accountValues = values
        } else {
            root.argumentValues = values
        }
        root.reviseAndPlan()
        return true
    }

    function fieldValue(kind, name) {
        const values = kind === "accounts" ? root.accountValues : root.argumentValues
        return String(values && values[String(name || "")] || "")
    }

    function copyMap(value) {
        const result = {}
        const source = value || {}
        for (const key in source) {
            result[key] = String(source[key] || "")
        }
        return result
    }

    function planSchemaKey(value) {
        const planValue = value || ({})
        function fields(rows) {
            return (Array.isArray(rows) ? rows : []).map(function (field) {
                const row = field || ({})
                return {
                    name: String(row.name || ""),
                    label: String(row.label || ""),
                    placeholder: String(row.placeholder || ""),
                    required: row.required === true,
                    rest: row.rest === true,
                    kind: String(row.kind || ""),
                    typeLabel: String(row.type_label || "")
                }
            })
        }
        return JSON.stringify({
            accounts: fields(planValue.accounts),
            args: fields(planValue.args)
        })
    }

    function currentTargetDisplay() {
        const context = root.zoneState && root.zoneState.activeZoneContext
            ? root.zoneState.activeZoneContext : null
        const sourceId = String(context
            && context.selected_sequencer_source_id || "")
        const config = root.zoneDetail && root.zoneDetail.channel_source_config
            ? root.zoneDetail.channel_source_config : ({})
        const sources = Array.isArray(config.sequencer_sources)
            ? config.sequencer_sources : []
        let endpoint = ""
        let label = ""
        let targetKind = ""
        for (let index = 0; index < sources.length; ++index) {
            const source = sources[index] || {}
            if (String(source.source_id || "") !== sourceId) {
                continue
            }
            endpoint = String(source.target && source.target.endpoint || "")
            targetKind = String(source.target && source.target.kind || "")
            label = String(source.label || "")
            break
        }
        const sourceConfigRevision = Number(context
            && context.source_config_revision || 0)
        const ready = context !== null
            && sourceId.length > 0
            && Number(config.config_revision || 0) === sourceConfigRevision
            && String(config.selected_sequencer_source_id || "") === sourceId
            && targetKind === "rpc" && endpoint.length > 0
        return {
            channelId: String(context && context.channel_id || ""),
            sourceId: sourceId,
            sourceLabel: label,
            endpoint: endpoint,
            targetKind: targetKind,
            sourceConfigRevision: sourceConfigRevision,
            contextRevision: Number(context && context.context_revision || 0),
            ready: ready
        }
    }

    function invalidateForCurrentContext() {
        if (root.execution) {
            const previousRevision = root.execution.idlInstructionDraftRevision
            const usable = root.execution.syncIdlInstructionContext(
                root.currentTargetDisplay())
            const changed = root.execution.idlInstructionDraftRevision
                !== previousRevision
            if (!changed) {
                return
            }
            root.cancelOpenConfirmation()
            if (usable && root.idlEntryForKey(root.selectedIdlKey)) {
                root.execution.planIdlInstruction()
            }
        }
    }

    function cancelOpenConfirmation() {
        if (!root.execution) {
            return
        }
        root.execution.cancelIdlInstructionConfirmation()
        if (sendConfirmation.visible) {
            sendConfirmation.close()
        }
    }

    function planPending() {
        return root.execution
            ? root.execution.idlInstructionPlanPending === true : false
    }

    function previewPending() {
        return root.execution
            ? root.execution.idlInstructionPreviewPending === true : false
    }

    function submitPending() {
        return root.execution
            ? root.execution.idlInstructionSubmitPending === true : false
    }

    function planReady() {
        return root.plan !== null && root.plan.inputs_complete === true
    }

    function targetDisplayReady() {
        return root.currentTargetDisplay().ready === true
    }

    function privateDraft() {
        if (root.plan && root.plan.private_mode === true) {
            return true
        }
        const values = root.accountValues || {}
        for (const key in values) {
            const references = String(values[key] || "").split(",")
            for (let index = 0; index < references.length; ++index) {
                if (references[index].trim().toLowerCase().indexOf("private/") === 0) {
                    return true
                }
            }
        }
        return false
    }

    function previewInstruction() {
        if (!root.execution || !root.planReady()) {
            return null
        }
        return root.execution.previewIdlInstructionDraft()
    }

    function previewCurrent() {
        return root.execution
            && typeof root.execution.idlInstructionPreviewCurrent === "function"
            && root.execution.idlInstructionPreviewCurrent()
    }

    function openConfirmation() {
        if (!root.execution || !root.targetDisplayReady()
                || !root.execution.beginIdlInstructionConfirmation()) {
            return false
        }
        root.confirmationAccepted = false
        sendConfirmation.open()
        return true
    }

    function confirmInstruction() {
        if (!root.execution) {
            return null
        }
        return root.execution.confirmIdlInstruction()
    }

    function backendTargetMatchesCurrent(target) {
        const context = root.zoneState && root.zoneState.activeZoneContext
            ? root.zoneState.activeZoneContext : null
        return context !== null
            && ZoneInspectionContract.scopeKey(target && target.network_scope)
                === ZoneInspectionContract.scopeKey(context.network_scope)
            && String(target && target.channel_id || "")
                === String(context.channel_id || "")
            && String(target && target.source_id || "")
                === String(context.selected_sequencer_source_id || "")
            && Number(target && target.source_config_revision || 0)
                === Number(context.source_config_revision || 0)
    }

    function receiptSourceId() {
        return String(root.execution && root.execution.idlInstructionReceiptTarget
            && root.execution.idlInstructionReceiptTarget.source_id || "")
    }

    function receiptMatchesCurrentTarget() {
        return root.receipt !== null && root.execution
            && root.backendTargetMatchesCurrent(root.execution.idlInstructionReceiptTarget)
    }

    function emitReceiptTransaction() {
        const transactionId = String(root.receipt && root.receipt.tx_hash || "")
        const sourceId = root.receiptSourceId()
        if (!transactionId.length || !sourceId.length
                || !root.receiptMatchesCurrentTarget()) {
            return false
        }
        root.transactionRequested(transactionId, sourceId)
        return true
    }

    function previewFactRows() {
        const value = root.preview || ({})
        const artifact = root.execution
            ? root.execution.idlInstructionFrozenArtifact : null
        return [{
            label: qsTr("IDL"),
            value: String(artifact && artifact.entry && artifact.entry.name || "-")
        }, {
            label: qsTr("Instruction"),
            value: String(value.instruction || "-")
        }, {
            label: qsTr("Mode"),
            value: String(value.mode || "-")
        }, {
            label: qsTr("Program"),
            value: String(value.program_id_hex || artifact
                && artifact.entry && artifact.entry.programIdHex || "-"),
            copyable: true,
            monospace: true
        }]
    }

    function previewTargetRows() {
        const artifact = root.execution
            ? root.execution.idlInstructionFrozenArtifact : null
        const target = artifact && artifact.targetDisplay
            ? artifact.targetDisplay : ({})
        return [{
            label: qsTr("Channel"),
            value: String(target.channelId || "-"),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Source"),
            value: String(target.sourceId || "-"),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Endpoint"),
            value: String(target.endpoint || "-")
        }]
    }

    function previewAccountRows() {
        const rows = root.preview && Array.isArray(root.preview.accounts)
            ? root.preview.accounts : []
        return rows.map(function (account) {
            const accountId = String(account && account.account_id || "")
            const roles = [account && account.signer === true
                ? qsTr("Signer") : qsTr("Non-signer")]
            if (account && account.rest === true) {
                roles.push(qsTr("Rest"))
            }
            if (account && account.pda === true) {
                roles.push(qsTr("PDA"))
            }
            return {
                cells: [
                    { text: String(account && account.name || "-"), width: 150, monospace: false },
                    { text: accountId, width: 300, fill: true, copyable: accountId.length > 0, copyText: accountId },
                    { text: roles.join(" · "), width: 120, monospace: false }
                ]
            }
        })
    }

    function previewArgumentRows() {
        const rows = root.preview && Array.isArray(root.preview.args)
            ? root.preview.args : []
        return rows.map(function (argument) {
            return {
                cells: [
                    { text: String(argument && argument.name || "-"), width: 170, monospace: false },
                    { text: String(argument && argument.type_label || "-"), width: 120, monospace: false },
                    { text: String(argument && argument.value || "-"), width: 260, fill: true }
                ]
            }
        })
    }

    function confirmationMessage() {
        const confirmation = root.execution
            ? root.execution.idlInstructionConfirmation : null
        if (!confirmation) {
            return ""
        }
        const entry = confirmation.entry || ({})
        const previewValue = confirmation.preview || ({})
        const target = confirmation.targetDisplay || ({})
        const lines = [
            qsTr("IDL: %1").arg(String(entry.name || "-")),
            qsTr("Program: %1").arg(String(entry.programIdHex || "-")),
            qsTr("Instruction: %1").arg(String(previewValue.instruction || "-")),
            qsTr("Mode: %1").arg(String(previewValue.mode || "-")),
            qsTr("Channel: %1").arg(String(target.channelId || "-")),
            qsTr("Source: %1").arg(String(target.sourceId || "-")),
            qsTr("Endpoint: %1").arg(String(target.endpoint || "-"))
        ]
        const accounts = Array.isArray(previewValue.accounts)
            ? previewValue.accounts : []
        if (accounts.length > 0) {
            lines.push(qsTr("Accounts:"))
            for (let index = 0; index < accounts.length; ++index) {
                const account = accounts[index] || {}
                lines.push(qsTr("- %1: %2%3")
                    .arg(String(account.name || "-"))
                    .arg(String(account.account_id || "-"))
                    .arg(account.signer === true ? qsTr(" (signer)") : ""))
            }
        }
        const args = Array.isArray(previewValue.args) ? previewValue.args : []
        if (args.length > 0) {
            lines.push(qsTr("Arguments:"))
            for (let index = 0; index < args.length; ++index) {
                const argument = args[index] || {}
                lines.push(qsTr("- %1: %2")
                    .arg(String(argument.name || "-"))
                    .arg(String(argument.value || "-")))
            }
        }
        lines.push(qsTr("Only this frozen preview will be submitted."))
        return lines.join("\n")
    }

    function objectPart(value) {
        return String(value || "field").replace(/[^A-Za-z0-9_]/g, "_")
    }

    function shortValue(value) {
        const text = String(value || "")
        return text.length > 18
            ? text.slice(0, 10) + "..." + text.slice(-6) : (text || "-")
    }
}
