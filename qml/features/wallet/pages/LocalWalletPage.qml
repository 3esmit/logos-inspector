pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQml.Models
import QtQuick.Layouts
import "../../../components"
import "../../../components/common"
import "../../../state"
import "../../../state/wallet/LocalWalletCommandWorkspace.js" as LocalWalletCommandWorkspace
import "../../../theme"
import "../../../utils/UiFormat.js" as UiFormat

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model

    width: parent ? parent.width : 900
    spacing: 16

    ListModel {
        id: walletTabs

        ListElement { value: "profiles"; label: "Profiles" }
        ListElement { value: "controls"; label: "Controls" }
        ListElement { value: "lezAccounts"; label: "LEZ Accounts" }
        ListElement { value: "privateSync"; label: "Private Sync" }
        ListElement { value: "bedrockNotes"; label: "Bedrock Wallet" }
        ListElement { value: "operations"; label: "Operations" }
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Local / Wallet")
        title: qsTr("Local Wallet")
        layerLabel: qsTr("Local")
        subtitle: qsTr("Explicit local wallet profile, wallet accounts, private sync status, and Bedrock wallet balance probes.")
        Layout.fillWidth: true
    }

    TabSwitch {
        theme: root.theme
        current: root.model.localWalletTab
        options: walletTabs
        Layout.fillWidth: true
        onSelected: value => root.model.localWalletTab = value
    }

    Frame {
        id: walletStateShelf

        padding: root.theme.gap
        Layout.fillWidth: true

        background: Rectangle {
            color: root.theme.surface
            radius: root.theme.radius
            border.width: 1
            border.color: root.theme.outlineMuted
        }

        contentItem: ColumnLayout {
            spacing: root.theme.gapSmall

            GridLayout {
                columns: root.width < 760 ? 1 : 2
                columnSpacing: root.theme.gap
                rowSpacing: root.theme.gapSmall
                Layout.fillWidth: true

                SourceStrip {
                    theme: root.theme
                    sources: root.headerSources()
                    Layout.fillWidth: true
                }

                LinkCell {
                    visible: root.model.localWalletLookupTarget.length > 0
                    theme: root.theme
                    text: root.model.localWalletLookupTarget
                    copyable: true
                    copyText: root.model.localWalletLookupTarget
                    monospace: true
                    textColor: root.model.localWalletTab === "bedrockNotes" && root.model.bedrockWalletSourceConfigured() ? root.theme.text : (root.model.walletProfileUsable() ? root.theme.text : root.theme.warning)
                    Layout.fillWidth: true
                    Layout.alignment: root.width < 760 ? Qt.AlignLeft : Qt.AlignRight
                }
            }

            GridLayout {
                columns: root.width < 720 ? 2 : 4
                columnSpacing: root.theme.gapSmall
                rowSpacing: root.theme.gapSmall
                Layout.fillWidth: true

                StatusChip {
                    theme: root.theme
                    label: root.model.localWalletTab === "bedrockNotes" ? qsTr("Source") : qsTr("Profile")
                    value: root.model.walletProfileUsable() ? root.shortText(root.model.walletProfileLabel, 22) : (root.model.localWalletTab === "bedrockNotes" && root.model.bedrockWalletSourceConfigured() ? qsTr("L1 Bedrock") : qsTr("Required"))
                    tone: root.model.walletProfileUsable() || (root.model.localWalletTab === "bedrockNotes" && root.model.bedrockWalletSourceConfigured()) ? "success" : "warning"
                    compact: true
                    showIndicator: true
                    Layout.fillWidth: true
                }

                StatusChip {
                    theme: root.theme
                    label: qsTr("Check")
                    value: root.localStatusText()
                    detail: root.localStatusDetail()
                    tone: root.localStatusTone()
                    compact: true
                    showIndicator: true
                    Layout.fillWidth: true
                }

                StatusChip {
                    theme: root.theme
                    label: root.model.localWalletTab === "bedrockNotes" ? qsTr("Endpoint") : qsTr("Home")
                    value: root.model.localWalletTab === "bedrockNotes" ? root.shortText(root.model.nodeUrl, 22) : root.shortText(root.model.walletHomeDisplayLabel(), 22)
                    detail: root.model.localWalletTab === "bedrockNotes" ? root.model.nodeUrl : root.model.walletHomeDisplayLabel()
                    tone: "neutral"
                    compact: true
                    showIndicator: true
                    Layout.fillWidth: true
                }

                StatusChip {
                    theme: root.theme
                    label: qsTr("Bedrock")
                    value: root.model.bedrockWalletBalanceValue !== null ? qsTr("Loaded") : qsTr("Idle")
                    detail: root.shortText(root.model.nodeUrl, 42)
                    tone: root.model.bedrockWalletBalanceValue !== null ? "success" : "neutral"
                    compact: true
                    showIndicator: true
                    Layout.fillWidth: true
                }
            }
        }
    }

    StatusMessage {
        visible: root.model.localWalletTab !== "bedrockNotes" && !root.model.walletProfileUsable()
        theme: root.theme
        tone: "warning"
        title: qsTr("Local wallet profile required")
        message: qsTr("wallet:<id> uses explicit local wallet state. Indexer-derived transfers are under recipient:<id>.")
        Layout.fillWidth: true
    }

    Loader {
        active: true
        asynchronous: true
        sourceComponent: root.tabComponent(root.model.localWalletTab)
        Layout.fillWidth: true
    }

    Component {
        id: profilesTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Panel {
                theme: root.theme
                title: qsTr("Profile")

                ColumnLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Label")
                        value: root.model.walletProfileLabel
                        copyText: ""
                    }

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Wallet binary")
                        value: root.model.walletBinary.length ? root.model.walletBinaryDisplayLabel() : qsTr("Not set")
                        copyText: ""
                    }

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Wallet home")
                        value: root.model.walletHomeDisplayLabel()
                        copyText: ""
                    }

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Version")
                        value: root.model.localWalletStatus && root.model.localWalletStatus.version ? String(root.model.localWalletStatus.version) : "-"
                        copyText: root.model.localWalletStatus && root.model.localWalletStatus.version ? String(root.model.localWalletStatus.version) : ""
                    }

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Bedrock node")
                        value: root.endpointLabel(root.model.nodeUrl)
                        copyText: root.model.nodeUrl
                    }
                }

                RowLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Open Settings")
                        primary: true
                        onClicked: root.model.openSettings("wallet", "")
                    }

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Autodetect")
                        onClicked: {
                            root.model.detectWalletProfile(true)
                            root.model.checkLocalWalletProfile(false)
                        }
                    }

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Check")
                        onClicked: root.model.checkLocalWalletProfile(false)
                    }

                    Item {
                        Layout.fillWidth: true
                    }
                }
            }

            StatusMessage {
                visible: root.model.localWalletStatusError.length > 0
                theme: root.theme
                tone: "error"
                title: qsTr("Profile check failed")
                message: root.model.localWalletStatusError
                Layout.fillWidth: true
            }
        }
    }

    Component {
        id: controlsTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Panel {
                theme: root.theme
                title: qsTr("Create Account")

                ColumnLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    GridLayout {
                        columns: root.width < 760 ? 1 : 4
                        columnSpacing: root.theme.gapSmall
                        rowSpacing: root.theme.gapSmall
                        Layout.fillWidth: true

                        ActionButton {
                            theme: root.theme
                            text: qsTr("Public")
                            selected: root.model.walletCreatePrivacy === "public"
                            Layout.preferredWidth: 104
                            Layout.fillWidth: root.width < 760
                            onClicked: root.model.walletCreatePrivacy = "public"
                        }

                        ActionButton {
                            theme: root.theme
                            text: qsTr("Private")
                            selected: root.model.walletCreatePrivacy === "private"
                            Layout.preferredWidth: 104
                            Layout.fillWidth: root.width < 760
                            onClicked: root.model.walletCreatePrivacy = "private"
                        }

                        FieldRow {
                            theme: root.theme
                            label: qsTr("Label")
                            sourceText: root.model.walletCreateLabel
                            syncSourceText: true
                            placeholderText: qsTr("Optional label")
                            Layout.fillWidth: true
                            onTextEdited: text => { if (root.model.walletCreateLabel !== text) root.model.walletCreateLabel = text }
                        }

                        ActionButton {
                            objectName: "createAccountButton"
                            theme: root.theme
                            text: qsTr("Create")
                            primary: true
                            enabled: !root.model.shell.busy && root.model.walletProfileConfigured()
                            Layout.preferredWidth: 112
                            Layout.fillWidth: root.width < 760
                            onClicked: createAccountConfirm.open()
                        }
                    }
                }
            }

            Panel {
                theme: root.theme
                title: qsTr("Send Native")

                ColumnLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    GridLayout {
                        columns: root.width < 820 ? 1 : 3
                        columnSpacing: root.theme.gap
                        rowSpacing: root.theme.gapSmall
                        Layout.fillWidth: true

                        FieldRow {
                            theme: root.theme
                            label: qsTr("From")
                            sourceText: root.model.walletSendFrom
                            syncSourceText: true
                            placeholderText: qsTr("Public/... or label")
                            Layout.fillWidth: true
                            onTextEdited: text => { if (root.model.walletSendFrom !== text) root.model.walletSendFrom = text }
                        }

                        FieldRow {
                            theme: root.theme
                            label: qsTr("To")
                            sourceText: root.model.walletSendTo
                            syncSourceText: true
                            placeholderText: qsTr("Public/..., Private/..., or label")
                            Layout.fillWidth: true
                            onTextEdited: text => { if (root.model.walletSendTo !== text) root.model.walletSendTo = text }
                        }

                        FieldRow {
                            theme: root.theme
                            label: qsTr("Amount")
                            sourceText: root.model.walletSendAmount
                            syncSourceText: true
                            placeholderText: qsTr("0")
                            Layout.fillWidth: true
                            onTextEdited: text => { if (root.model.walletSendAmount !== text) root.model.walletSendAmount = text }
                        }

                        FieldRow {
                            theme: root.theme
                            label: qsTr("Keys file")
                            sourceText: root.model.walletSendToKeys
                            syncSourceText: true
                            placeholderText: qsTr("Optional recipient.keys")
                            Layout.fillWidth: true
                            onTextEdited: text => { if (root.model.walletSendToKeys !== text) root.model.walletSendToKeys = text }
                        }

                        FieldRow {
                            theme: root.theme
                            label: qsTr("NPK")
                            sourceText: root.model.walletSendToNpk
                            syncSourceText: true
                            placeholderText: qsTr("Optional hex")
                            Layout.fillWidth: true
                            onTextEdited: text => { if (root.model.walletSendToNpk !== text) root.model.walletSendToNpk = text }
                        }

                        FieldRow {
                            theme: root.theme
                            label: qsTr("VPK")
                            sourceText: root.model.walletSendToVpk
                            syncSourceText: true
                            placeholderText: qsTr("Optional hex")
                            Layout.fillWidth: true
                            onTextEdited: text => { if (root.model.walletSendToVpk !== text) root.model.walletSendToVpk = text }
                        }

                        FieldRow {
                            theme: root.theme
                            label: qsTr("Identifier")
                            sourceText: root.model.walletSendToIdentifier
                            syncSourceText: true
                            placeholderText: qsTr("Optional")
                            Layout.fillWidth: true
                            onTextEdited: text => { if (root.model.walletSendToIdentifier !== text) root.model.walletSendToIdentifier = text }
                        }
                    }

                    RowLayout {
                        spacing: root.theme.gapSmall
                        Layout.fillWidth: true

                        ActionButton {
                            objectName: "sendTransactionButton"
                            theme: root.theme
                            text: qsTr("Send")
                            primary: true
                            enabled: root.sendReady()
                            Layout.preferredWidth: 112
                            onClicked: sendTransactionConfirm.open()
                        }

                        ActionButton {
                            theme: root.theme
                            text: qsTr("Settings")
                            enabled: !root.model.shell.busy
                            Layout.preferredWidth: 112
                            onClicked: root.model.openSettings("wallet", "")
                        }

                        Item {
                            Layout.fillWidth: true
                        }
                    }
                }
            }

            Panel {
                theme: root.theme
                title: qsTr("Incoming")

                RowLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    ActionButton {
                        objectName: "readIncomingButton"
                        theme: root.theme
                        text: qsTr("Read incoming")
                        primary: true
                        enabled: !root.model.shell.busy && root.model.walletProfileConfigured()
                        Layout.preferredWidth: 144
                        onClicked: readIncomingConfirm.open()
                    }

                    ActionButton {
                        theme: root.theme
                        text: qsTr("List accounts")
                        enabled: !root.model.shell.busy && root.model.walletProfileConfigured()
                        Layout.preferredWidth: 132
                        onClicked: root.model.wallet.queryAccounts(true)
                    }

                    Item {
                        Layout.fillWidth: true
                    }
                }
            }

            Panel {
                theme: root.theme
                title: qsTr("Advanced Command")

                ColumnLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    TextAreaField {
                        theme: root.theme
                        label: qsTr("Arguments")
                        rows: 3
                        text: root.model.walletAdvancedCommand
                        placeholderText: qsTr("account get --account-id Public/...")
                        Layout.fillWidth: true
                        onTextChanged: {
                            if (root.model.walletAdvancedCommand !== text) {
                                root.model.walletAdvancedCommand = text
                            }
                        }
                    }

                    RowLayout {
                        spacing: root.theme.gapSmall
                        Layout.fillWidth: true

                        ActionButton {
                            objectName: "advancedWalletButton"
                            theme: root.theme
                            text: qsTr("Run")
                            primary: true
                            enabled: !root.model.shell.busy && root.model.walletProfileConfigured() && root.walletCommandArgs().length > 0
                            Layout.preferredWidth: 96
                            onClicked: root.openAdvancedWalletConfirm()
                        }

                        Item {
                            Layout.fillWidth: true
                        }
                    }
                }
            }
        }
    }

    Component {
        id: lezAccountsTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            StatusMessage {
                theme: root.theme
                tone: "info"
                title: qsTr("LEZ accounts")
                message: qsTr("Lists accounts known by the configured local wallet. Public chain state remains in Accounts.")
                Layout.fillWidth: true
            }

            Panel {
                theme: root.theme
                title: qsTr("Wallet Accounts")

                ColumnLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    RowLayout {
                        spacing: root.theme.gapSmall
                        Layout.fillWidth: true

                        ActionButton {
                            theme: root.theme
                            text: qsTr("List accounts")
                            primary: true
                            enabled: !root.model.shell.busy && root.model.walletProfileConfigured()
                            Layout.preferredWidth: 132
                            onClicked: root.model.wallet.queryAccounts(false)
                        }

                        ActionButton {
                            theme: root.theme
                            text: qsTr("Settings")
                            enabled: !root.model.shell.busy
                            Layout.preferredWidth: 112
                            onClicked: root.model.openSettings("wallet", "")
                        }

                        Text {
                            text: root.model.localWalletAccountsError.length ? root.model.localWalletAccountsError : root.walletAccountSummary()
                            color: root.model.localWalletAccountsError.length ? root.theme.warning : root.theme.textMuted
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: root.theme.secondaryText
                            Layout.fillWidth: true
                        }
                    }

                    DataTableFrame {
                        theme: root.theme
                        Layout.fillWidth: true
                        headerCells: [
                            { text: qsTr("Account"), width: 260, fill: true },
                            { text: qsTr("Privacy"), width: 88 },
                            { text: qsTr("State"), width: 112 },
                            { text: qsTr("Label"), width: 140 }
                        ]
                        rows: root.walletAccountRows()
                        onCellActivated: function (row, column, cell, rowData) {
                            if (rowData.typedId.length > 0) {
                                root.model.entityNavigation.routeSearch(
                                    "account:" + rowData.typedId)
                            }
                        }
                    }
                }
            }
        }
    }

    Component {
        id: privateSyncTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            StatusMessage {
                theme: root.theme
                tone: "warning"
                title: qsTr("Manual sync")
                message: qsTr("Private wallet sync is not run automatically. Configure the profile before launching external sync commands.")
                Layout.fillWidth: true
            }

            Panel {
                theme: root.theme
                title: qsTr("State")

                ColumnLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Private context")
                        value: root.model.localWalletLookupTarget.length ? root.model.localWalletLookupTarget : "-"
                        copyText: root.model.localWalletLookupTarget
                    }

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Wallet home")
                        value: root.model.walletHomeDisplayLabel()
                        copyText: ""
                    }

                    RowLayout {
                        spacing: root.theme.gapSmall
                        Layout.fillWidth: true

                        ActionButton {
                            objectName: "privateSyncButton"
                            theme: root.theme
                            text: qsTr("Sync private")
                            primary: true
                            enabled: !root.model.shell.busy && root.model.walletProfileConfigured()
                            Layout.preferredWidth: 132
                            onClicked: privateSyncConfirm.open()
                        }

                        ActionButton {
                            theme: root.theme
                            text: qsTr("Settings")
                            enabled: !root.model.shell.busy
                            Layout.preferredWidth: 112
                            onClicked: root.model.openSettings("wallet", "")
                        }

                        Item {
                            Layout.fillWidth: true
                        }
                    }
                }
            }
        }
    }

    Component {
        id: bedrockNotesTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Panel {
                theme: root.theme
                title: qsTr("Bedrock Wallet")

                GridLayout {
                    columns: root.width < 760 ? 1 : 3
                    columnSpacing: root.theme.gap
                    rowSpacing: root.theme.gap
                    Layout.fillWidth: true

                    FieldRow {
                        theme: root.theme
                        label: qsTr("Address / public key")
                        sourceText: root.model.walletPublicKeyProbe
                        syncSourceText: true
                        placeholderText: qsTr("Wallet address or 64-hex public key")
                        Layout.columnSpan: root.width < 760 ? 1 : 2
                        onTextEdited: text => { if (root.model.walletPublicKeyProbe !== text) root.model.walletPublicKeyProbe = text }
                    }

                    FieldRow {
                        theme: root.theme
                        label: qsTr("Tip")
                        sourceText: root.model.bedrockWalletBalanceTip
                        syncSourceText: true
                        placeholderText: qsTr("Optional 64-hex header id")
                        onTextEdited: text => { if (root.model.bedrockWalletBalanceTip !== text) root.model.bedrockWalletBalanceTip = text }
                    }
                }

                RowLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Save")
                        onClicked: root.model.saveWalletState()
                    }

                    ActionButton {
                        theme: root.theme
                        text: qsTr("REST Balance")
                        primary: true
                        onClicked: {
                            root.model.saveWalletState()
                            root.model.wallet.queryBedrockBalance()
                        }
                    }

                    Item {
                        Layout.fillWidth: true
                    }
                }
            }

            StatusMessage {
                visible: root.model.bedrockWalletBalanceError.length > 0
                theme: root.theme
                tone: "error"
                title: qsTr("REST balance query failed")
                message: root.model.bedrockWalletBalanceError
                Layout.fillWidth: true
            }

            Panel {
                visible: root.model.bedrockWalletBalanceValue !== null
                theme: root.theme
                title: qsTr("REST Balance")

                ColumnLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Public key")
                        value: root.model.walletPublicKeyProbe
                        copyText: root.model.walletPublicKeyProbe
                    }

                    Text {
                        objectName: "bedrockBalanceJson"
                        text: root.balanceJson()
                        color: root.theme.text
                        textFormat: Text.PlainText
                        wrapMode: Text.WrapAnywhere
                        font.family: "monospace"
                        font.pixelSize: root.theme.dataText
                        Layout.fillWidth: true

                        Accessible.role: Accessible.StaticText
                        Accessible.name: qsTr("Bedrock REST balance response: %1").arg(text)
                    }
                }
            }
        }
    }

    ConfirmActionPopup {
        id: privateSyncConfirm
        objectName: "privateSyncConfirm"

        theme: root.theme
        title: qsTr("Sync private wallet")
        message: qsTr("This runs the configured local wallet sync-private command and may update local wallet state.")
        confirmText: qsTr("Sync private")
        confirmEnabled: !root.model.shell.busy && root.model.walletProfileConfigured()
        onAccepted: root.model.wallet.syncPrivate()
    }

    ConfirmActionPopup {
        id: createAccountConfirm
        objectName: "createAccountConfirm"

        theme: root.theme
        title: qsTr("Create account")
        message: qsTr("This runs wallet account new %1.").arg(root.model.walletCreatePrivacy === "private" ? qsTr("private") : qsTr("public"))
        confirmText: qsTr("Create")
        confirmEnabled: !root.model.shell.busy && root.model.walletProfileConfigured()
        onAccepted: root.model.wallet.createAccount()
    }

    ConfirmActionPopup {
        id: sendTransactionConfirm
        objectName: "sendTransactionConfirm"

        theme: root.theme
        title: qsTr("Send transaction")
        message: qsTr("This runs wallet auth-transfer send from %1.").arg(root.shortText(root.model.walletSendFrom, 32))
        confirmText: qsTr("Send")
        confirmEnabled: root.sendReady()
        onAccepted: root.model.wallet.sendTransaction()
    }

    ConfirmActionPopup {
        id: readIncomingConfirm
        objectName: "readIncomingConfirm"

        theme: root.theme
        title: qsTr("Read incoming")
        message: qsTr("This runs wallet account sync-private and updates local wallet state.")
        confirmText: qsTr("Read")
        confirmEnabled: !root.model.shell.busy && root.model.walletProfileConfigured()
        onAccepted: root.model.wallet.readIncomingTransactions()
    }

    ConfirmActionPopup {
        id: advancedWalletConfirm
        objectName: "advancedWalletConfirm"

        theme: root.theme
        title: qsTr("Run wallet command")
        message: qsTr("This runs wallet %1.").arg(root.shortText(root.walletCommandArgs().join(" "), 54))
        confirmText: qsTr("Run")
        confirmEnabled: !root.model.shell.busy && root.model.walletProfileConfigured() && root.walletCommandArgs().length > 0
        onAccepted: root.acceptAdvancedWalletCommand()
    }

    Component {
        id: operationsTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Panel {
                theme: root.theme
                title: qsTr("Recent Operations")

                ColumnLayout {
                    spacing: 0
                    Layout.fillWidth: true

                    OperationRow {
                        theme: root.theme
                        header: true
                        columns: [qsTr("Time"), qsTr("Operation"), qsTr("Status"), qsTr("Detail")]
                    }

                    Repeater {
                        model: root.operationRows()

                        OperationRow {
                            required property var modelData

                            theme: root.theme
                            columns: [modelData.time, modelData.label, modelData.status, modelData.detail]
                            status: modelData.status
                        }
                    }
                }
            }
        }
    }

    function tabComponent(tab) {
        switch (String(tab || "")) {
        case "controls":
            return controlsTab
        case "lezAccounts":
            return lezAccountsTab
        case "privateSync":
            return privateSyncTab
        case "bedrockNotes":
            return bedrockNotesTab
        case "operations":
            return operationsTab
        default:
            return profilesTab
        }
    }

    function headerSources() {
        if (root.model.localWalletTab === "bedrockNotes") {
            return [qsTr("L1 Bedrock"), qsTr("Wallet balance"), root.shortText(root.model.nodeUrl, 42)]
        }
        if (root.model.localWalletTab === "controls") {
            return [qsTr("Local Wallet"), qsTr("Controls"), root.model.walletHomeSourceLabel()]
        }
        if (root.model.localWalletTab === "lezAccounts") {
            return [qsTr("Local Wallet"), qsTr("LEZ Accounts"), root.model.walletHomeSourceLabel()]
        }
        return [qsTr("Local Wallet"), qsTr("Explicit Profile"), root.model.walletHomeSourceLabel()]
    }

    function walletAccountSummary() {
        const report = root.model.localWalletAccountsValue || null
        const accounts = report && Array.isArray(report.accounts) ? report.accounts : []
        return accounts.length ? qsTr("%1 accounts loaded").arg(accounts.length) : qsTr("No wallet accounts loaded")
    }

    function walletAccountRows() {
        const report = root.model.localWalletAccountsValue || null
        const accounts = report && Array.isArray(report.accounts) ? report.accounts : []
        if (!accounts.length) {
            return [{
                typedId: "",
                cells: [
                    { text: qsTr("No wallet accounts loaded"), width: 260, fill: true, monospace: false },
                    { text: "-", width: 88 },
                    { text: "-", width: 112 },
                    { text: "-", width: 140 }
                ]
            }]
        }
        return accounts.map(function (account) {
            const typedId = String(account.typed_id || account.typedId || "")
            return {
                typedId: typedId,
                cells: [
                    { text: root.shortText(typedId, 42), width: 260, fill: true, link: typedId.length > 0, copyText: typedId },
                    { text: String(account.privacy || "-"), width: 88, monospace: false },
                    { text: String(account.state || "-"), width: 112, monospace: false },
                    { text: String(account.label || "-"), width: 140, monospace: false }
                ]
            }
        })
    }

    function localStatusText() {
        const status = root.model.localWalletStatus || null
        if (!status) {
            return root.model.localWalletStatusError.length ? qsTr("Down") : qsTr("Unknown")
        }
        const value = String(status.status || "unknown")
        return value.length ? value[0].toUpperCase() + value.slice(1) : qsTr("Unknown")
    }

    function localStatusDetail() {
        const status = root.model.localWalletStatus || null
        if (root.model.localWalletStatusError.length) {
            return root.shortText(root.model.localWalletStatusError, 36)
        }
        if (status && status.detail) {
            return root.shortText(status.detail, 36)
        }
        return qsTr("Not checked")
    }

    function localStatusTone() {
        const status = root.model.localWalletStatus || null
        const value = status && status.status ? String(status.status) : ""
        if (root.model.localWalletStatusError.length || value === "down") {
            return "error"
        }
        if (!value.length || value === "degraded" || value === "unknown") {
            return "warning"
        }
        if (value === "ok") {
            return "success"
        }
        return "neutral"
    }

    function balanceJson() {
        try {
            return JSON.stringify(root.model.bedrockWalletBalanceValue, null, 2)
        } catch (error) {
            return String(root.model.bedrockWalletBalanceValue || "")
        }
    }

    function operationRows() {
        return LocalWalletCommandWorkspace.operationRows(root.model)
    }

    function shortText(value, limit) {
        return UiFormat.shortText(value, {
            emptyText: "-",
            limit: limit || 24,
            minimum: 8,
            tailLength: 6
        })
    }

    function endpointLabel(value) {
        const text = String(value || "").trim()
        if (!text.length) {
            return qsTr("Not configured")
        }
        return text.replace(/^https?:\/\//, "").replace(/\/$/, "")
    }

    function sendReady() {
        return LocalWalletCommandWorkspace.sendReady(root.model)
    }

    function walletCommandArgs() {
        return LocalWalletCommandWorkspace.walletCommandArgs(root.model)
    }

    function openAdvancedWalletConfirm() {
        const error = LocalWalletCommandWorkspace.advancedCommandError(root.model)
        if (error.length) {
            root.model.shell.setResult(qsTr("Wallet command"), error, true)
            return
        }
        advancedWalletConfirm.open()
    }

    function acceptAdvancedWalletCommand() {
        const parsed = root.walletCommandArgs()
        if (parsed !== null && parsed.length > 0) {
            root.model.wallet.runCommand(parsed)
        }
    }

    function parseWalletCommandLine(value) {
        return LocalWalletCommandWorkspace.parseWalletCommandLine(value)
    }

    component CopyRow: GridLayout {
        id: copyRoot

        required property Theme theme
        property string label: ""
        property string value: "-"
        property string copyText: value

        columns: 2
        columnSpacing: copyRoot.theme.gap
        rowSpacing: copyRoot.theme.gapTiny
        Layout.fillWidth: true

        Text {
            text: copyRoot.label
            color: copyRoot.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: copyRoot.theme.secondaryText
            font.weight: Font.Medium
            Layout.preferredWidth: 132
        }

        LinkCell {
            theme: copyRoot.theme
            text: copyRoot.value
            copyable: copyRoot.copyText.length > 0
            copyText: copyRoot.copyText
            monospace: true
            wrap: true
            Layout.fillWidth: true
        }
    }

    component OperationRow: Item {
        id: rowRoot

        required property Theme theme
        property var columns: []
        property string status: ""
        property bool header: false

        Layout.fillWidth: true
        Layout.preferredHeight: rowRoot.header ? 34 : 40

        Rectangle {
            anchors.fill: parent
            color: rowRoot.header ? rowRoot.theme.field : "transparent"
            border.width: 0
        }

        GridLayout {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            columns: 4
            columnSpacing: 10

            Repeater {
                model: 4

                Text {
                    required property int index

                    text: String(rowRoot.columns[index] || "-")
                    color: rowRoot.textColor(index)
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    font.family: rowRoot.header ? "" : "monospace"
                    font.pixelSize: rowRoot.header ? rowRoot.theme.labelText : rowRoot.theme.dataText
                    font.weight: rowRoot.header ? Font.DemiBold : Font.Normal
                    font.capitalization: rowRoot.header ? Font.AllUppercase : Font.MixedCase
                    Layout.preferredWidth: rowRoot.columnWidth(index)
                    Layout.fillWidth: index === 3
                }
            }
        }

        function textColor(index) {
            if (rowRoot.header) {
                return rowRoot.theme.textMuted
            }
            if (index === 2) {
                if (rowRoot.status === "ok") {
                    return rowRoot.theme.success
                }
                if (rowRoot.status === "down") {
                    return rowRoot.theme.error
                }
                if (rowRoot.status === "degraded" || rowRoot.status === "unknown") {
                    return rowRoot.theme.warning
                }
            }
            return rowRoot.theme.text
        }

        function columnWidth(index) {
            if (index === 0) {
                return 88
            }
            if (index === 1) {
                return 150
            }
            if (index === 2) {
                return 90
            }
            return 260
        }
    }
}
