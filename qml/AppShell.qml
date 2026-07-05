pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "components"
import "pages"
import "services"
import "state"
import "theme"

Item {
    id: root

    property QtObject bridgeHost: null
    readonly property bool compact: width < 940
    property int pageLoadSerial: 0

    Theme {
        id: theme
    }

    BridgeClient {
        id: bridge
        host: root.bridgeHost
    }

    AppModel {
        id: appModel
        bridge: bridge
    }

    Component.onCompleted: {
        root.schedulePageLoaderUpdate()
        const initialReference = root.initialReferenceFromArguments()
        Qt.callLater(function () {
            appModel.loadSettingsState()
            root.schedulePageLoaderUpdate()
            if (appModel.currentView === "overview" && appModel.dashboardRefreshInterval() > 0 && appModel.bridgeSupportsAsync()) {
                Qt.callLater(function () {
                    appModel.refreshDashboard()
                })
            }
            Qt.callLater(function () {
                appModel.loadIdlState()
                appModel.loadWalletState()
                if (initialReference.length > 0) {
                    Qt.callLater(function () {
                        appModel.routeSearch(initialReference)
                    })
                }
            })
        })
    }

    Timer {
        interval: appModel.refreshInterval(appModel.blockchainRefreshRate)
        repeat: true
        running: appModel.blockchainRefreshRate > 0
        onTriggered: appModel.queryNetworkConnection("blockchain", false)
    }

    Timer {
        interval: appModel.refreshInterval(appModel.indexerRefreshRate)
        repeat: true
        running: appModel.indexerRefreshRate > 0
        onTriggered: appModel.queryNetworkConnection("indexer", false)
    }

    Timer {
        interval: appModel.refreshInterval(appModel.executionRefreshRate)
        repeat: true
        running: appModel.executionRefreshRate > 0
        onTriggered: appModel.queryNetworkConnection("execution", false)
    }

    Timer {
        interval: appModel.refreshInterval(appModel.messagingRefreshRate)
        repeat: true
        running: appModel.messagingRefreshRate > 0
        onTriggered: appModel.queryNetworkConnection("messaging", false)
    }

    Timer {
        interval: appModel.refreshInterval(appModel.storageRefreshRate)
        repeat: true
        running: appModel.storageRefreshRate > 0
        onTriggered: appModel.queryNetworkConnection("storage", false)
    }

    Timer {
        interval: appModel.dashboardRefreshInterval()
        repeat: true
        running: appModel.currentView === "overview" && appModel.dashboardRefreshInterval() > 0
        onTriggered: appModel.refreshDashboard()
    }

    Rectangle {
        anchors.fill: parent
        color: theme.background
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        RowLayout {
            spacing: 0
            Layout.fillWidth: true
            Layout.fillHeight: true

            NavRail {
                theme: theme
                model: appModel
                compact: root.compact
                Layout.preferredWidth: compact ? 96 : 228
                Layout.fillHeight: true
            }

            Rectangle {
                color: theme.outline
                Layout.preferredWidth: 1
                Layout.fillHeight: true
            }

            ColumnLayout {
                spacing: 0
                Layout.fillWidth: true
                Layout.fillHeight: true

                StatusBar {
                    id: statusBar

                    theme: theme
                    model: appModel
                    compact: root.compact
                    Layout.fillWidth: true
                }

                Rectangle {
                    color: theme.outlineMuted
                    Layout.fillWidth: true
                    Layout.preferredHeight: 1
                }

                ScrollView {
                    id: pageScroll
                    leftPadding: root.compact ? theme.gap : theme.pageMargin
                    rightPadding: root.compact ? theme.gap : theme.pageMargin
                    topPadding: theme.gapLarge
                    bottomPadding: theme.gapLarge
                    contentWidth: availableWidth
                    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    Loader {
                        id: pageLoader
                        active: true
                        asynchronous: true
                        width: pageScroll.availableWidth
                    }
                }
            }
        }

        Rectangle {
            color: theme.outlineMuted
            Layout.fillWidth: true
            Layout.preferredHeight: 1
        }

        StatusFooter {
            theme: theme
            model: appModel
            Layout.fillWidth: true
        }
    }

    Connections {
        target: appModel

        function onCurrentViewChanged() {
            root.schedulePageLoaderUpdate()
            if (pageScroll.contentItem) {
                pageScroll.contentItem.contentY = 0
            }
        }
    }

    Shortcut {
        sequence: "Ctrl+L"
        onActivated: statusBar.focusLookup()
    }

    Shortcut {
        sequence: "Ctrl+K"
        onActivated: statusBar.focusLookup()
    }

    function pageFor(view) {
        switch (view) {
        case "blocks":
            return blocksPage
        case "blockDetail":
            return blockDetailPage
        case "transactions":
            return transactionsPage
        case "transactionDetail":
            return transactionDetailPage
        case "transferActivity":
            return transferActivityPage
        case "blockchain":
            return blockchainPage
        case "channels":
            return channelsPage
        case "storage":
            return storagePage
        case "messaging":
            return messagingPage
        case "diagnosticsStorage":
            return storageDiagnosticsPage
        case "diagnosticsDelivery":
            return deliveryDiagnosticsPage
        case "capabilities":
            return capabilitiesPage
        case "l2Blocks":
        case "sequencer":
            return lezBlocksPage
        case "l2Transactions":
            return lezTransactionsPage
        case "l2BlockDetail":
            return lezBlockDetailPage
        case "l2TransactionDetail":
            return lezTransactionDetailPage
        case "accounts":
            return accountsPage
        case "programs":
            return programsPage
        case "localWallet":
            return localWalletPage
        case "indexer":
            return indexerPage
        case "settings":
            return settingsPage
        default:
            return overviewPage
        }
    }

    function schedulePageLoaderUpdate() {
        root.pageLoadSerial += 1
        const serial = root.pageLoadSerial
        Qt.callLater(function () {
            if (serial !== root.pageLoadSerial) {
                return
            }
            pageLoader.sourceComponent = root.pageFor(appModel.currentView)
        })
    }

    function initialReferenceFromArguments() {
        const args = Qt.application.arguments || []
        for (let i = 0; i < args.length - 1; i += 1) {
            if (args[i] === "--open-ref") {
                return String(args[i + 1] || "").trim()
            }
        }
        return ""
    }

    Component {
        id: overviewPage
        OverviewPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: blocksPage
        BlocksPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: blockDetailPage
        BlockDetailPage {
            theme: theme
            model: appModel
            l2: false
        }
    }

    Component {
        id: transactionsPage
        TransactionsPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: transactionDetailPage
        TransactionDetailPage {
            theme: theme
            model: appModel
            l2: false
        }
    }

    Component {
        id: transferActivityPage
        TransferActivityPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: blockchainPage
        ModulePage {
            theme: theme
            model: appModel
            moduleKind: "blockchain"
            title: qsTr("Bedrock Node Diagnostics")
            subtitle: qsTr("Inspect Bedrock node state, L1 block windows, and blockchain module calls.")
        }
    }

    Component {
        id: channelsPage
        ChannelsPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: storagePage
        StorageAppPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: messagingPage
        DeliveryAppPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: storageDiagnosticsPage
        StoragePage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: deliveryDiagnosticsPage
        DeliveryPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: capabilitiesPage
        ModulePage {
            theme: theme
            model: appModel
            moduleKind: "capabilities"
            title: qsTr("Capabilities Diagnostics")
            subtitle: qsTr("Review capability inventory and module availability.")
        }
    }

    Component {
        id: lezBlocksPage
        LezBlocksPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: lezTransactionsPage
        LezTransactionsPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: lezBlockDetailPage
        BlockDetailPage {
            theme: theme
            model: appModel
            l2: true
        }
    }

    Component {
        id: lezTransactionDetailPage
        TransactionDetailPage {
            theme: theme
            model: appModel
            l2: true
        }
    }

    Component {
        id: accountsPage
        AccountsPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: programsPage
        ProgramsPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: localWalletPage
        LocalWalletPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: indexerPage
        IndexerPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: settingsPage
        SettingsPage {
            theme: theme
            model: appModel
        }
    }
}
