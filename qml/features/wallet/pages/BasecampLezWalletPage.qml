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
        ListElement { value: "operations"; label: "Operations" }
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Wallet")
        title: qsTr("LEZ Wallet")
        layerLabel: qsTr("Basecamp")
        subtitle: qsTr("Inspect accounts and submit public transfers through the official LEZ Core wallet module.")
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
                label: qsTr("LEZ Core")
                value: root.wallet.availabilityLabel()
                detail: root.wallet.availabilityDetail
                tone: root.wallet.availabilityTone()
                compact: true
                showIndicator: true
                Layout.fillWidth: true
            }

            StatusChip {
                theme: root.theme
                label: qsTr("Accounts")
                value: qsTr("%1 loaded").arg(Array.isArray(root.wallet.accounts) ? root.wallet.accounts.length : 0)
                tone: Array.isArray(root.wallet.accounts) && root.wallet.accounts.length > 0 ? "success" : "neutral"
                compact: true
                showIndicator: true
                Layout.fillWidth: true
            }

            StatusChip {
                theme: root.theme
                label: qsTr("Transfers")
                value: root.wallet.transferResult && root.wallet.transferResult.success === true ? qsTr("Submitted") : qsTr("Ready")
                tone: root.wallet.transferResult && root.wallet.transferResult.success === true ? "success" : "neutral"
                compact: true
                showIndicator: true
                Layout.fillWidth: true
            }
        }
    }

    StatusMessage {
        visible: root.wallet.notice.length > 0
        theme: root.theme
        tone: "info"
        title: qsTr("Wallet status")
        message: root.wallet.notice
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.wallet.error.length > 0
        theme: root.theme
        tone: "error"
        title: qsTr("LEZ wallet error")
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
                title: qsTr("Official wallet provider")

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
                            objectName: "basecampLezWalletRefreshButton"
                            theme: root.theme
                            text: root.wallet.busy ? qsTr("Refreshing") : qsTr("Refresh wallet")
                            primary: true
                            enabled: !root.wallet.busy
                            Layout.preferredWidth: 164
                            onClicked: root.wallet.refresh()
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
                title: qsTr("Wallet setup remains in LEZ Wallet")
                message: qsTr("Install and open the official LEZ Wallet UI to create or unlock a wallet. Inspector uses the opened LEZ Core module; it never asks for a password, recovery phrase, or private key.")
                Layout.fillWidth: true
            }
        }
    }

    Component {
        id: accountsTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Panel {
                theme: root.theme
                title: qsTr("Wallet accounts")

                ColumnLayout {
                    spacing: root.theme.gap
                    Layout.fillWidth: true

                    RowLayout {
                        spacing: root.theme.gapSmall
                        Layout.fillWidth: true

                        ActionButton {
                            objectName: "basecampLezWalletAccountsRefreshButton"
                            theme: root.theme
                            text: root.wallet.busy ? qsTr("Refreshing") : qsTr("Refresh accounts")
                            primary: true
                            enabled: !root.wallet.busy
                            Layout.preferredWidth: 168
                            onClicked: root.wallet.refresh()
                        }

                        Text {
                            text: qsTr("Balances are returned by LEZ Core in atomic units.")
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
                            { text: qsTr("Account"), width: 360, fill: true },
                            { text: qsTr("Kind"), width: 120 },
                            { text: qsTr("Balance"), width: 180 }
                        ]
                        rows: root.wallet.accountRows()
                        Layout.fillWidth: true
                        onCellActivated: function(row, column, cell, rowData) {
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
                title: qsTr("Public transfer")
                message: qsTr("Confirm the account IDs and atomic-unit amount before sending. LEZ Core signs using the opened official wallet.")
                Layout.fillWidth: true
            }

            Panel {
                theme: root.theme
                title: qsTr("Send public native assets")

                ColumnLayout {
                    spacing: root.theme.gap
                    Layout.fillWidth: true

                    FieldRow {
                        objectName: "basecampLezWalletTransferFrom"
                        theme: root.theme
                        label: qsTr("From")
                        sourceText: root.transferFrom
                        syncSourceText: true
                        placeholderText: qsTr("32-byte hexadecimal public account ID")
                        Layout.fillWidth: true
                        onTextEdited: text => root.transferFrom = text
                    }

                    FieldRow {
                        objectName: "basecampLezWalletTransferTo"
                        theme: root.theme
                        label: qsTr("To")
                        sourceText: root.transferTo
                        syncSourceText: true
                        placeholderText: qsTr("32-byte hexadecimal recipient account ID")
                        Layout.fillWidth: true
                        onTextEdited: text => root.transferTo = text
                    }

                    FieldRow {
                        objectName: "basecampLezWalletTransferAmount"
                        theme: root.theme
                        label: qsTr("Amount")
                        sourceText: root.transferAmount
                        syncSourceText: true
                        placeholderText: qsTr("Positive atomic-unit amount")
                        Layout.fillWidth: true
                        onTextEdited: text => root.transferAmount = text
                    }

                    RowLayout {
                        spacing: root.theme.gapSmall
                        Layout.fillWidth: true

                        ActionButton {
                            objectName: "basecampLezWalletTransferButton"
                            theme: root.theme
                            text: qsTr("Send public transfer")
                            primary: true
                            enabled: !root.wallet.busy && root.wallet.transferReady(root.transferFrom, root.transferTo, root.transferAmount)
                            Layout.preferredWidth: 200
                            onClicked: transferConfirm.open()
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
        id: operationsTab

        Panel {
            theme: root.theme
            title: qsTr("LEZ Core operations")

            DataTableFrame {
                theme: root.theme
                headerCells: [
                    { text: qsTr("Operation"), width: 180 },
                    { text: qsTr("Status"), width: 120 },
                    { text: qsTr("Detail"), width: 360, fill: true }
                ]
                rows: root.wallet.operationRows()
                Layout.fillWidth: true
            }
        }
    }

    ConfirmActionPopup {
        id: transferConfirm

        objectName: "basecampLezWalletTransferConfirm"
        theme: root.theme
        title: qsTr("Send public transfer")
        message: qsTr("Send %1 atomic units from %2 to %3 through LEZ Core?")
            .arg(root.transferAmount).arg(root.transferFrom).arg(root.transferTo)
        confirmText: qsTr("Send transaction")
        confirmEnabled: !root.wallet.busy && root.wallet.transferReady(root.transferFrom, root.transferTo, root.transferAmount)
        onAccepted: root.wallet.submitPublicTransfer(root.transferFrom, root.transferTo, root.transferAmount)
    }

    Component.onCompleted: root.wallet.refresh()

    function tabComponent(tab) {
        switch (String(tab || "")) {
        case "accounts":
            return accountsTab
        case "transfer":
            return transferTab
        case "operations":
            return operationsTab
        default:
            return providerTab
        }
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
