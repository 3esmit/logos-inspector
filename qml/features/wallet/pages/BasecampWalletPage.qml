pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../components"
import "../../../components/common"
import "../../../state"
import "../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model

    readonly property var wallet: root.model.basecampWallet
    readonly property string selectedTab: root.model.basecampWalletTab
    property string transferFrom: ""
    property string transferTo: ""
    property string transferAmount: ""

    width: parent ? parent.width : 900
    spacing: root.theme.gapLarge

    ListModel {
        id: tabs

        ListElement { value: "provider"; label: "Provider" }
        ListElement { value: "accounts"; label: "Accounts" }
        ListElement { value: "transfer"; label: "Transfer" }
        ListElement { value: "capabilities"; label: "Capabilities" }
        ListElement { value: "operations"; label: "Operations" }
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Wallet")
        title: qsTr("Basecamp Wallet")
        layerLabel: qsTr("Basecamp")
        subtitle: qsTr("Connect a Basecamp wallet provider. Keys remain in the wallet and every connection or transfer is approved there.")
        Layout.fillWidth: true
    }

    TabSwitch {
        theme: root.theme
        current: root.selectedTab
        options: tabs
        Layout.fillWidth: true
        onSelected: value => root.model.basecampWalletTab = value
    }

    Frame {
        padding: root.theme.gap
        Layout.fillWidth: true

        background: Rectangle {
            color: root.theme.surface
            radius: root.theme.radius
            border.width: 1
            border.color: root.theme.outlineMuted
        }

        contentItem: GridLayout {
            columns: root.width < 720 ? 1 : 3
            columnSpacing: root.theme.gapSmall
            rowSpacing: root.theme.gapSmall

            StatusChip {
                theme: root.theme
                label: qsTr("Provider")
                value: root.wallet.availabilityLabel()
                detail: root.wallet.availabilityDetail
                tone: root.wallet.availabilityTone()
                compact: true
                showIndicator: true
                Layout.fillWidth: true
            }

            StatusChip {
                theme: root.theme
                label: qsTr("Session")
                value: root.wallet.sessionLabel()
                tone: root.wallet.sessionTone()
                compact: true
                showIndicator: true
                Layout.fillWidth: true
            }

            StatusChip {
                theme: root.theme
                label: qsTr("Approval")
                value: root.wallet.approvalLabel()
                tone: root.wallet.approvalTone()
                compact: true
                showIndicator: true
                Layout.fillWidth: true
            }
        }
    }

    StatusMessage {
        visible: root.wallet.notice.length > 0
        theme: root.theme
        tone: root.wallet.awaitingApproval || root.wallet.trackingTransfer ? "warning" : "info"
        title: root.wallet.awaitingApproval ? qsTr("Wallet approval needed") : qsTr("Wallet status")
        message: root.wallet.notice
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.wallet.error.length > 0
        theme: root.theme
        tone: "error"
        title: qsTr("Wallet provider error")
        message: root.wallet.error
        Layout.fillWidth: true
    }

    Loader {
        active: true
        asynchronous: true
        sourceComponent: root.tabComponent(root.selectedTab)
        Layout.fillWidth: true
    }

    Component {
        id: providerTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Panel {
                theme: root.theme
                title: qsTr("Wallet provider")

                ColumnLayout {
                    spacing: root.theme.gap
                    Layout.fillWidth: true

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Provider")
                        value: root.wallet.providerLabel
                        copyText: ""
                    }

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Module")
                        value: root.wallet.providerModule
                    }

                    RowLayout {
                        spacing: root.theme.gapSmall
                        Layout.fillWidth: true

                        ActionButton {
                            objectName: "basecampWalletCheckProviderButton"
                            theme: root.theme
                            text: qsTr("Check provider")
                            enabled: !root.wallet.busy && !root.wallet.awaitingApproval
                            Layout.preferredWidth: 148
                            onClicked: root.wallet.checkAvailability()
                        }

                        ActionButton {
                            objectName: "basecampWalletConnectButton"
                            theme: root.theme
                            text: qsTr("Connect accounts")
                            primary: true
                            enabled: !root.wallet.busy && !root.wallet.awaitingApproval
                            Layout.preferredWidth: 164
                            onClicked: root.wallet.connectAccounts()
                        }

                        ActionButton {
                            objectName: "basecampWalletDisconnectButton"
                            theme: root.theme
                            text: root.wallet.awaitingApproval ? qsTr("Stop polling") : qsTr("Disconnect")
                            enabled: root.wallet.connected || root.wallet.awaitingApproval
                            Layout.preferredWidth: 132
                            onClicked: {
                                if (root.wallet.awaitingApproval) {
                                    root.wallet.forgetPendingRequest()
                                } else {
                                    root.wallet.disconnect()
                                }
                            }
                        }

                        Item {
                            Layout.fillWidth: true
                        }
                    }
                }
            }

            StatusMessage {
                theme: root.theme
                tone: "info"
                title: qsTr("Approval boundary")
                message: qsTr("Inspector asks the provider for account access. Select accounts and approve or reject the request in the wallet UI; Inspector never receives a password, mnemonic, or private key.")
                Layout.fillWidth: true
            }
        }
    }

    Component {
        id: accountsTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            StatusMessage {
                visible: !root.wallet.connected
                theme: root.theme
                tone: "warning"
                title: qsTr("Connect a wallet first")
                message: qsTr("Authorize account access in the Basecamp wallet to see the accounts it chooses to share.")
                Layout.fillWidth: true
            }

            Panel {
                theme: root.theme
                title: qsTr("Authorized accounts")

                ColumnLayout {
                    spacing: root.theme.gap
                    Layout.fillWidth: true

                    RowLayout {
                        spacing: root.theme.gapSmall
                        Layout.fillWidth: true

                        ActionButton {
                            objectName: "basecampWalletAccountsConnectButton"
                            theme: root.theme
                            text: root.wallet.connected ? qsTr("Refresh session") : qsTr("Connect accounts")
                            primary: true
                            enabled: !root.wallet.busy && !root.wallet.awaitingApproval
                            Layout.preferredWidth: 164
                            onClicked: {
                                if (root.wallet.connected) {
                                    root.wallet.loadSession()
                                } else {
                                    root.wallet.connectAccounts()
                                }
                            }
                        }

                        Text {
                            text: root.wallet.connected
                                ? qsTr("Only wallet-authorized account IDs are shown.")
                                : qsTr("No wallet session is connected.")
                            color: root.theme.textMuted
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: root.theme.secondaryText
                            Layout.fillWidth: true
                        }
                    }

                    DataTableFrame {
                        theme: root.theme
                        headerCells: [
                            { text: qsTr("Account"), width: 320, fill: true },
                            { text: qsTr("Access"), width: 160 }
                        ]
                        rows: root.wallet.accountRows()
                        Layout.fillWidth: true
                        onCellActivated: function (row, column, cell, rowData) {
                            if (rowData.accountId.length > 0) {
                                root.model.entityNavigation.routeSearch("account:" + rowData.accountId)
                            }
                        }
                    }
                }
            }
        }
    }

    Component {
        id: transferTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            StatusMessage {
                theme: root.theme
                tone: "warning"
                title: qsTr("Wallet-approved transfer")
                message: qsTr("Inspector creates a native transfer request only. The wallet validates it and requires a separate approval before signing or sending.")
                Layout.fillWidth: true
            }

            Panel {
                theme: root.theme
                title: qsTr("Native transfer")

                ColumnLayout {
                    spacing: root.theme.gap
                    Layout.fillWidth: true

                    FieldRow {
                        objectName: "basecampWalletTransferFrom"
                        theme: root.theme
                        label: qsTr("From")
                        sourceText: root.transferFrom
                        syncSourceText: true
                        placeholderText: qsTr("Authorized account ID")
                        Layout.fillWidth: true
                        onTextEdited: text => root.transferFrom = text
                    }

                    FieldRow {
                        objectName: "basecampWalletTransferTo"
                        theme: root.theme
                        label: qsTr("To")
                        sourceText: root.transferTo
                        syncSourceText: true
                        placeholderText: qsTr("Recipient account ID")
                        Layout.fillWidth: true
                        onTextEdited: text => root.transferTo = text
                    }

                    FieldRow {
                        objectName: "basecampWalletTransferAmount"
                        theme: root.theme
                        label: qsTr("Amount")
                        sourceText: root.transferAmount
                        syncSourceText: true
                        placeholderText: qsTr("Whole native units")
                        Layout.fillWidth: true
                        onTextEdited: text => root.transferAmount = text
                    }

                    RowLayout {
                        spacing: root.theme.gapSmall
                        Layout.fillWidth: true

                        ActionButton {
                            objectName: "basecampWalletTransferButton"
                            theme: root.theme
                            text: root.wallet.hasPermission("send")
                                ? qsTr("Request transfer") : qsTr("Connect and request")
                            primary: true
                            enabled: !root.wallet.busy && root.transferFieldsComplete()
                            Layout.preferredWidth: 192
                            onClicked: transferConfirm.open()
                        }

                        Text {
                            text: root.wallet.hasPermission("send")
                                ? qsTr("A fresh approval is still required for this transfer.")
                                : qsTr("This first asks the wallet to grant account and transfer access.")
                            color: root.theme.textMuted
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: root.theme.secondaryText
                            Layout.fillWidth: true
                        }
                    }
                }
            }
        }
    }

    Component {
        id: capabilitiesTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Panel {
                theme: root.theme
                title: qsTr("Provider capabilities")

                ColumnLayout {
                    spacing: root.theme.gap
                    Layout.fillWidth: true

                    StatusMessage {
                        theme: root.theme
                        tone: "success"
                        title: qsTr("Supported")
                        message: qsTr("Wallet-authorized account access and native transfer requests. The wallet validates and approves every transfer itself.")
                        Layout.fillWidth: true
                    }

                    StatusMessage {
                        theme: root.theme
                        tone: "warning"
                        title: qsTr("Not exposed by this provider")
                        message: qsTr("Generic program deployment and arbitrary IDL instruction signing. Inspector keeps decoded previews available, but does not fall back to a local wallet runtime in Basecamp.")
                        Layout.fillWidth: true
                    }
                }
            }
        }
    }

    Component {
        id: operationsTab

        Panel {
            theme: root.theme
            title: qsTr("Provider operations")

            DataTableFrame {
                theme: root.theme
                headerCells: [
                    { text: qsTr("Time"), width: 100 },
                    { text: qsTr("Operation"), width: 180, fill: true },
                    { text: qsTr("Status"), width: 120 },
                    { text: qsTr("Detail"), width: 260, fill: true }
                ]
                rows: root.wallet.operationRows()
                Layout.fillWidth: true
            }
        }
    }

    ConfirmActionPopup {
        id: transferConfirm

        objectName: "basecampWalletTransferConfirm"
        theme: root.theme
        title: qsTr("Request wallet transfer")
        message: qsTr("Ask the Basecamp wallet to approve a native transfer of %1 from %2 to %3. The wallet retains the signing key and may reject the request.")
            .arg(root.transferAmount).arg(root.transferFrom).arg(root.transferTo)
        confirmText: qsTr("Request")
        confirmEnabled: !root.wallet.busy && root.transferFieldsComplete()
        onAccepted: root.wallet.startNativeTransfer(root.transferFrom, root.transferTo, root.transferAmount)
    }

    function tabComponent(tab) {
        switch (String(tab || "")) {
        case "accounts":
            return accountsTab
        case "transfer":
            return transferTab
        case "capabilities":
            return capabilitiesTab
        case "operations":
            return operationsTab
        default:
            return providerTab
        }
    }

    function transferFieldsComplete() {
        return String(root.transferFrom || "").trim().length > 0
            && String(root.transferTo || "").trim().length > 0
            && /^[0-9]+$/.test(String(root.transferAmount || "").trim())
    }

    function shortText(value, limit) {
        const text = String(value || "")
        const maximum = Number(limit || 0)
        if (maximum <= 0 || text.length <= maximum) {
            return text
        }
        return text.slice(0, Math.max(1, maximum - 1)) + "…"
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
}
