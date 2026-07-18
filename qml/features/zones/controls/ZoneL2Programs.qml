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
    property var appModel: null
    property var zoneDetail: null
    property string currentTool: "programs"
    property string commitmentQuery: ""

    signal configureSourcesRequested()
    signal configureIdlsRequested()
    signal transactionRequested(string transactionId, string exactSourceId)

    objectName: "zoneL2Programs"
    spacing: root.theme.gapLarge
    Layout.fillWidth: true

    Component.onCompleted: root.ensureProgramsLoaded()

    Connections {
        target: root.zoneState

        function onActiveZoneContextChanged() {
            Qt.callLater(root.ensureProgramsLoaded)
        }
    }

    ListModel {
        id: tools

        ListElement { value: "programs"; label: "Known Programs" }
        ListElement { value: "interact"; label: "Interact" }
        ListElement { value: "proof"; label: "Commitment Proof" }
        ListElement { value: "nonces"; label: "Account Nonces" }
    }

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        ColumnLayout {
            spacing: root.theme.gapTiny
            Layout.fillWidth: true

            Text {
                text: qsTr("L2 Programs")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: root.theme.panelTitleText
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            Text {
                text: root.zoneState.l2SequencerSourceId()
                color: root.theme.textMuted
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
                font.family: "monospace"
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }
        }

        ZoneKindChip {
            visible: root.zoneState.l2SequencerReadEnabled
            theme: root.theme
            label: qsTr("Selected Sequencer")
            tone: "info"
        }
    }

    StatusMessage {
        visible: !root.zoneState.l2SequencerReadEnabled
        theme: root.theme
        tone: root.zoneState.l2Applicable ? "warning" : "info"
        title: root.zoneState.l2Applicable
            ? qsTr("Sequencer source required") : qsTr("L2 not applicable")
        message: root.zoneState.l2Applicable
            ? qsTr("Select a Sequencer source for this Zone.")
            : root.zoneState.l2AvailabilityMessage()
        Layout.fillWidth: true
    }

    ActionButton {
        visible: root.zoneState.l2Applicable && !root.zoneState.l2SequencerReadEnabled
        theme: root.theme
        text: qsTr("Open Sources")
        Layout.preferredWidth: 150
        onClicked: root.configureSourcesRequested()
    }

    TabSwitch {
        visible: root.zoneState.l2SequencerReadEnabled
        theme: root.theme
        options: tools
        current: root.currentTool
        onSelected: function (value) {
            root.currentTool = value
            if (value === "programs") {
                root.ensureProgramsLoaded()
            }
        }
    }

    Loader {
        id: interactionLoader

        active: root.zoneState.l2SequencerReadEnabled
            && root.currentTool === "interact"
        visible: active
        Layout.fillWidth: true

        sourceComponent: ZoneL2ProgramInteraction {
            theme: root.theme
            appModel: root.appModel
            zoneState: root.zoneState
            zoneDetail: root.zoneDetail
            width: interactionLoader.width
            onConfigureIdlsRequested: root.configureIdlsRequested()
            onTransactionRequested: function (transactionId, exactSourceId) {
                root.transactionRequested(transactionId, exactSourceId)
            }
        }
    }

    ColumnLayout {
        visible: root.zoneState.l2SequencerReadEnabled
            && root.currentTool === "programs"
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Text {
                text: qsTr("Known Programs (%1)")
                    .arg(Presentation.numberText(root.zoneState.l2Programs.length))
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            ActionButton {
                objectName: "zoneL2ProgramsRefreshButton"
                theme: root.theme
                text: qsTr("Refresh")
                enabled: !root.zoneState.l2ProgramsInFlight
                Layout.preferredWidth: 100
                onClicked: root.zoneState.refreshL2Programs()
            }
        }

        StatusMessage {
            visible: root.zoneState.l2ProgramsError.length > 0
            theme: root.theme
            tone: "warning"
            title: qsTr("Programs unavailable")
            message: root.zoneState.l2ProgramsError
            Layout.fillWidth: true
        }

        DataTableFrame {
            objectName: "zoneL2ProgramsTable"
            theme: root.theme
            headerCells: [
                { text: qsTr("Label"), width: 150 },
                { text: qsTr("Base58"), width: 260, fill: true },
                { text: qsTr("Hex"), width: 260, fill: true },
                { text: qsTr("Saved"), width: 92, monospace: false }
            ]
            rows: root.programRows()
            Layout.fillWidth: true
            onCellActivated: function (row, column, cell, rowData) {
                if (column === 3 && rowData.favoriteEntry) {
                    root.zoneState.appModel.favoriteStore.toggle(rowData.favoriteEntry)
                }
            }
        }

        Text {
            visible: root.zoneState.l2ProgramsLoaded
                && root.zoneState.l2Programs.length === 0
                && root.zoneState.l2ProgramsError.length === 0
            text: qsTr("Selected Sequencer returned no known programs")
            color: root.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: root.theme.dataText
            Layout.fillWidth: true
        }

        ZoneL2Provenance {
            visible: root.zoneState.l2ProgramsReport !== null
            theme: root.theme
            source: root.programSource()
            route: root.zoneState.l2ProgramsReport
                ? root.zoneState.l2ProgramsReport.route : null
            routeCompleteness: root.zoneState.l2ProgramsReport
                ? String(root.zoneState.l2ProgramsReport.route_completeness || "") : ""
            Layout.fillWidth: true
        }
    }

    ColumnLayout {
        visible: root.zoneState.l2SequencerReadEnabled
            && root.currentTool === "proof"
        spacing: root.theme.gapLarge
        Layout.fillWidth: true

        GridLayout {
            columns: width < 620 ? 1 : 2
            columnSpacing: root.theme.gapSmall
            rowSpacing: root.theme.gapSmall
            Layout.fillWidth: true

            FieldRow {
                objectName: "zoneL2CommitmentField"
                theme: root.theme
                label: qsTr("Commitment hash")
                placeholderText: qsTr("Exact commitment hex")
                Layout.fillWidth: true
                onTextEdited: function (value) {
                    root.commitmentQuery = String(value || "").trim()
                }
            }

            ActionButton {
                objectName: "zoneL2CommitmentInspectButton"
                theme: root.theme
                text: qsTr("Inspect proof")
                primary: true
                enabled: root.commitmentQuery.length > 0
                    && !root.zoneState.l2CommitmentProofInFlight
                Layout.preferredWidth: 130
                Layout.alignment: Qt.AlignBottom | Qt.AlignLeft
                onClicked: root.zoneState.requestL2CommitmentProof(root.commitmentQuery)
            }
        }

        StatusMessage {
            visible: root.zoneState.l2CommitmentProofError.length > 0
            theme: root.theme
            tone: "warning"
            title: qsTr("Proof unavailable")
            message: root.zoneState.l2CommitmentProofError
            Layout.fillWidth: true
        }

        Text {
            visible: root.zoneState.l2CommitmentProofLoaded
                && root.zoneState.l2CommitmentProof === null
                && root.zoneState.l2CommitmentProofError.length === 0
            text: qsTr("Commitment proof not found")
            color: root.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: root.theme.dataText
            Layout.fillWidth: true
        }

        GridLayout {
            visible: root.zoneState.l2CommitmentProof !== null
            columns: width < 620 ? 1 : 2
            columnSpacing: root.theme.gapXLarge
            rowSpacing: root.theme.gapLarge
            Layout.fillWidth: true

            ZoneFactSection {
                theme: root.theme
                title: qsTr("Proof Identity")
                rows: root.proofRows()
            }

            ZoneFactSection {
                theme: root.theme
                title: qsTr("Proof Source")
                rows: root.sourceRows(root.proofSource())
            }
        }

        DataTableFrame {
            visible: root.zoneState.l2CommitmentProof !== null
            objectName: "zoneL2CommitmentSiblingsTable"
            theme: root.theme
            headerCells: [
                { text: qsTr("Level"), width: 72 },
                { text: qsTr("Sibling hash"), width: 300, fill: true }
            ]
            rows: root.siblingRows()
            Layout.fillWidth: true
        }
    }

    ColumnLayout {
        visible: root.zoneState.l2SequencerReadEnabled
            && root.currentTool === "nonces"
        spacing: root.theme.gapLarge
        Layout.fillWidth: true

        TextAreaField {
            id: nonceAccountsField

            objectName: "zoneL2NonceAccountsField"
            theme: root.theme
            label: qsTr("Account IDs")
            placeholderText: qsTr("One Base58 or hex account ID per line")
            rows: 4
        }

        ActionButton {
            objectName: "zoneL2NoncesInspectButton"
            theme: root.theme
            text: qsTr("Load nonces")
            primary: true
            enabled: nonceAccountsField.text.trim().length > 0
                && !root.zoneState.l2AccountNoncesInFlight
            Layout.preferredWidth: 130
            onClicked: root.zoneState.requestL2AccountNonces(
                root.accountIds(nonceAccountsField.text))
        }

        StatusMessage {
            visible: root.zoneState.l2AccountNoncesError.length > 0
            theme: root.theme
            tone: "warning"
            title: qsTr("Nonces unavailable")
            message: root.zoneState.l2AccountNoncesError
            Layout.fillWidth: true
        }

        DataTableFrame {
            visible: root.zoneState.l2AccountNoncesLoaded
            objectName: "zoneL2AccountNoncesTable"
            theme: root.theme
            headerCells: [
                { text: qsTr("Account ID"), width: 300, fill: true },
                { text: qsTr("Nonce"), width: 120 }
            ]
            rows: root.nonceRows()
            Layout.fillWidth: true
        }

        ZoneL2Provenance {
            visible: root.zoneState.l2AccountNoncesReport !== null
            theme: root.theme
            source: root.nonceSource()
            route: root.zoneState.l2AccountNoncesReport
                ? root.zoneState.l2AccountNoncesReport.route : null
            routeCompleteness: root.zoneState.l2AccountNoncesReport
                ? String(root.zoneState.l2AccountNoncesReport.route_completeness || "") : ""
            Layout.fillWidth: true
        }
    }

    function ensureProgramsLoaded() {
        if (root.zoneState.l2SequencerReadEnabled
                && !root.zoneState.l2ProgramsLoaded
                && !root.zoneState.l2ProgramsInFlight) {
            root.zoneState.refreshL2Programs()
        }
    }

    function programRows() {
        const rows = Array.isArray(root.zoneState.l2Programs)
            ? root.zoneState.l2Programs : []
        return rows.map(function (row) {
            const base58 = String(row && row.base58 || "")
            const hex = String(row && row.hex || "")
            const entityRef = typeof root.zoneState.l2ProgramEntityRef === "function"
                ? root.zoneState.l2ProgramEntityRef(row) : null
            const favorite = root.zoneState.appModel && entityRef
                ? root.zoneState.appModel.favoriteStore.l2EntityEntry(entityRef,
                    qsTr("Program %1").arg(String(row && row.label || hex).slice(0, 20)),
                    String(entityRef.channel_id || "")) : null
            const saved = favorite && root.zoneState.appModel.favoriteStore.isFavoriteEntry(favorite)
            return {
                cells: [
                    { text: Presentation.text(row && row.label), width: 150, monospace: false },
                    { text: base58, width: 260, fill: true, copyable: base58.length > 0, copyText: base58 },
                    { text: hex, width: 260, fill: true, copyable: hex.length > 0, copyText: hex },
                    { text: saved ? qsTr("Yes") : qsTr("Add"), width: 92, link: favorite !== null, monospace: false }
                ],
                favoriteEntry: favorite
            }
        })
    }

    function proofRows() {
        const proof = root.zoneState.l2CommitmentProof || ({})
        const siblings = Array.isArray(proof.sibling_hashes)
            ? proof.sibling_hashes : []
        return [{
            label: qsTr("Commitment"),
            value: Presentation.text(proof.commitment_hex),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Leaf index"),
            value: Presentation.numberText(proof.leaf_index)
        }, {
            label: qsTr("Sibling hashes"),
            value: Presentation.numberText(siblings.length)
        }]
    }

    function siblingRows() {
        const proof = root.zoneState.l2CommitmentProof || ({})
        const rows = Array.isArray(proof.sibling_hashes)
            ? proof.sibling_hashes : []
        return rows.map(function (hash, index) {
            return {
                cells: [
                    { text: Presentation.numberText(index), width: 72 },
                    { text: String(hash || ""), width: 300, fill: true, copyable: true, copyText: String(hash || "") }
                ]
            }
        })
    }

    function nonceRows() {
        const rows = Array.isArray(root.zoneState.l2AccountNonces)
            ? root.zoneState.l2AccountNonces : []
        return rows.map(function (row) {
            const accountId = String(row && row.account_id || "")
            return {
                cells: [
                    { text: accountId, width: 300, fill: true, copyable: accountId.length > 0, copyText: accountId },
                    { text: Presentation.text(row && row.nonce), width: 120 }
                ]
            }
        })
    }

    function accountIds(value) {
        return String(value || "").split(/[\s,]+/).filter(function (item) {
            return item.length > 0
        })
    }

    function programSource() {
        const report = root.zoneState.l2ProgramsReport
        const data = report && report.data && report.data.value
            ? report.data.value : null
        return data ? data.source : null
    }

    function proofSource() {
        return root.zoneState.l2CommitmentProof
            ? root.zoneState.l2CommitmentProof.source : null
    }

    function nonceSource() {
        const report = root.zoneState.l2AccountNoncesReport
        const data = report && report.data && report.data.value
            ? report.data.value : null
        return data ? data.source : null
    }

    function sourceRows(source) {
        const value = source || ({})
        return [{
            label: qsTr("Source ID"),
            value: Presentation.text(value.source_id),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Role"),
            value: Presentation.words(value.source_role)
        }, {
            label: qsTr("Finality"),
            value: Presentation.words(value.finality),
            tone: "warning"
        }, {
            label: qsTr("Retrieval"),
            value: Presentation.words(value.retrieval)
        }]
    }
}
