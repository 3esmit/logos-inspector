pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQml.Models
import QtQuick.Layouts
import "../../../components"
import "../../../components/common"
import "../../lez/controls/programs"
import "../../../state"
import "../../../theme"
import "../../../state/programs/ProgramResultPresentation.js" as ProgramResultPresentation

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
        ListElement { value: "sharing"; label: "Sharing" }
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
            onLinkActivated: (kind, value) => root.model.entityNavigation.openReference(kind, value)
        }

        LinkedDetailSection {
            visible: root.isEventDecodeReport(root.responseValue)
            theme: root.theme
            title: qsTr("Event decode")
            rows: root.eventDecodeRows()
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
        case "sharing":
            return sharingForm
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
                    onClicked: root.model.entityNavigation.openLocalWallet("", "profiles")
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
        id: sharingForm

        ColumnLayout {
            spacing: 12

            StatusMessage {
                theme: root.theme
                tone: root.model.sharedIdlPolicy === "disabled" ? "warning" : "info"
                title: qsTr("Shared IDLs")
                message: root.sharedPolicyText()
                Layout.fillWidth: true
            }

            GridLayout {
                columns: root.width < 760 ? 1 : 4
                columnSpacing: root.theme.gapSmall
                rowSpacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Suggest")
                    selected: root.model.sharedIdlPolicy === "suggestion"
                    Layout.fillWidth: true
                    onClicked: root.model.setSharedIdlPolicy("suggestion")
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Session")
                    selected: root.model.sharedIdlPolicy === "sessionOnly"
                    Layout.fillWidth: true
                    onClicked: root.model.setSharedIdlPolicy("sessionOnly")
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Auto-register")
                    selected: root.model.sharedIdlPolicy === "autoRegister"
                    Layout.fillWidth: true
                    onClicked: root.model.setSharedIdlPolicy("autoRegister")
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Disabled")
                    selected: root.model.sharedIdlPolicy === "disabled"
                    Layout.fillWidth: true
                    onClicked: root.model.setSharedIdlPolicy("disabled")
                }
            }

            CheckBox {
                id: autoShare

                text: qsTr("Auto-share verified local IDLs")
                checked: root.model.sharedIdlAutoShare
                palette.text: root.theme.text
                onToggled: root.model.setSharedIdlAutoShare(checked)
                Layout.fillWidth: true
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
        return ProgramResultPresentation.activeTabLabel(root)
    }

    function activeTabDelta() {
        return ProgramResultPresentation.activeTabDelta(root)
    }

    function activeTabMessage() {
        return ProgramResultPresentation.activeTabMessage(root)
    }

    function sharedPolicyText() {
        return ProgramResultPresentation.sharedPolicyText(root)
    }

    function validProgramId(value) {
        return ProgramResultPresentation.validProgramId(root, value)
    }

    function lastResultText() {
        return ProgramResultPresentation.lastResultText(root)
    }

    function lastResultDelta() {
        return ProgramResultPresentation.lastResultDelta(root)
    }

    function lastResultColor() {
        return ProgramResultPresentation.lastResultColor(root)
    }

    function responsePayloadText() {
        return ProgramResultPresentation.responsePayloadText(root)
    }

    function responseKindText() {
        return ProgramResultPresentation.responseKindText(root)
    }

    function responseIdlName() {
        return ProgramResultPresentation.responseIdlName(root)
    }

    function responseProgramText() {
        return ProgramResultPresentation.responseProgramText(root)
    }

    function responseProgramDelta() {
        return ProgramResultPresentation.responseProgramDelta(root)
    }

    function programRows() {
        return ProgramResultPresentation.programRows(root)
    }

    function programTableRows() {
        return ProgramResultPresentation.programTableRows(root)
    }

    function isProgramContext(value) {
        return ProgramResultPresentation.isProgramContext(value)
    }

    function programContextRows() {
        return ProgramResultPresentation.programContextRows(root)
    }

    function programVerificationText(value) {
        return ProgramResultPresentation.programVerificationText(value)
    }

    function programHexText(value) {
        return ProgramResultPresentation.programHexText(value)
    }

    function programContextIdlRows() {
        return ProgramResultPresentation.programContextIdlRows(root)
    }

    function programContextTransactionRows() {
        return ProgramResultPresentation.programContextTransactionRows(root)
    }

    function programContextAccount() {
        return ProgramResultPresentation.programContextAccount(root)
    }

    function knownIdlText(programId) {
        return ProgramResultPresentation.knownIdlText(root, programId)
    }

    function isIdlReport(value) {
        return ProgramResultPresentation.isIdlReport(value)
    }

    function isProgramFile(value) {
        return ProgramResultPresentation.isProgramFile(value)
    }

    function isEventDecodeReport(value) {
        return ProgramResultPresentation.isEventDecodeReport(value)
    }

    function idlCount(key) {
        return ProgramResultPresentation.idlCount(root, key)
    }

    function idlInstructionRows() {
        return ProgramResultPresentation.idlInstructionRows(root)
    }

    function idlAccountRows() {
        return ProgramResultPresentation.idlAccountRows(root)
    }

    function idlWarningRows() {
        return ProgramResultPresentation.idlWarningRows(root)
    }

    function programFileRows() {
        return ProgramResultPresentation.programFileRows(root)
    }

    function eventDecodeRows() {
        return ProgramResultPresentation.eventDecodeRows(root)
    }

    function idlFieldCount(json) {
        return ProgramResultPresentation.idlFieldCount(json)
    }

    function endpointLabel(value) {
        return ProgramResultPresentation.endpointLabel(value)
    }

    function shortEndpoint(value) {
        return ProgramResultPresentation.shortEndpoint(value)
    }

    function shortHash(value) {
        return ProgramResultPresentation.shortHash(value)
    }

    function shortPath(value) {
        return ProgramResultPresentation.shortPath(value)
    }

    function localPathFromFileUrl(fileUrl) {
        return ProgramResultPresentation.localPathFromFileUrl(fileUrl)
    }

    function valueText(value) {
        return ProgramResultPresentation.valueText(value)
    }

    function numberText(value) {
        return ProgramResultPresentation.numberText(value)
    }
}
