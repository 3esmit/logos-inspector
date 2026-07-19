pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "components"
import "components/common"
import "features/bedrock/pages" as BedrockPages
import "features/chain/pages" as ChainPages
import "features/dashboard/pages" as DashboardPages
import "features/delivery/pages" as DeliveryPages
import "features/local/pages" as LocalPages
import "features/modules/pages" as ModulePages
import "features/programs/pages" as ProgramPages
import "features/settings/pages" as SettingsPages
import "features/storage/pages" as StoragePages
import "features/wallet/pages" as WalletPages
import "features/zones/pages" as ZonePages
import "services"
import "state"
import "theme"

Item {
    id: root

    property QtObject bridgeHost: null
    readonly property bool compact: width < 940
    property int pageLoadSerial: 0
    property var pendingNavigationRequest: null
    readonly property ZonePages.ZonesPage loadedZonesPage:
        pageLoader.item as ZonePages.ZonesPage

    Theme {
        id: theme
    }

    BridgeClient {
        id: bridge
        host: root.bridgeHost
    }

    AppModel {
        id: appModel
        objectName: "appModel"
        bridge: bridge
        navigationGuard: function (kind, payload) {
            return root.guardNavigation(kind, payload)
        }
    }

    ConfirmActionPopup {
        id: pageNavigationGuard

        objectName: "pageNavigationGuard"
        theme: theme
        title: qsTr("Discard source draft")
        message: qsTr("Discard unsaved Channel source changes before leaving this page?")
        confirmText: qsTr("Discard")
        onAccepted: root.acceptGuardedNavigation()
    }

    ModuleEventIntake {
        id: moduleEventIntake
        bridge: bridge
        model: appModel
    }

    ListenerScheduler {
        id: listenerScheduler
        model: appModel
    }

    Component.onCompleted: {
        root.schedulePageLoaderUpdate()
        const initialReference = root.initialReferenceFromArguments()
        Qt.callLater(function () {
            appModel.sourceRouting.loadSourcePolicy()
            appModel.loadSettingsState()
            appModel.refreshLocalNodes(false)
            appModel.startZoneInspection()
            appModel.loadCapabilityRegistry()
            appModel.loadBackupCatalog()
            root.schedulePageLoaderUpdate()
            if (appModel.shell.currentView === "overview"
                    && appModel.metrics.dashboardRefreshInterval() > 0
                    && appModel.bridgeSupportsAsync()) {
                Qt.callLater(function () {
                    appModel.metrics.refreshDashboard()
                })
            }
            Qt.callLater(function () {
                appModel.loadIdlState()
                appModel.loadWalletState()
                appModel.checkLocalWalletProfile(false)
                appModel.loadCapabilityRegistry()
                moduleEventIntake.install()
                if (initialReference.length > 0) {
                    Qt.callLater(function () {
                        appModel.entityNavigation.routeSearch(initialReference)
                    })
                }
            })
        })
    }

    Component.onDestruction: appModel.stopZoneInspection()

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
                onNavigationRequested: function (view) {
                    appModel.selectView(view)
                }
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
                        objectName: "pageLoader"
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
        target: appModel.shell

        function onCurrentViewChanged() {
            root.schedulePageLoaderUpdate()
            if (pageScroll.contentItem) {
                pageScroll.contentItem.contentY = 0
            }
        }
    }

    Shortcut {
        sequence: "Alt+Left"
        enabled: appModel.canNavigateBack()
        onActivated: appModel.navigateBack()
    }

    Shortcut {
        sequence: "Alt+Right"
        enabled: appModel.canNavigateForward()
        onActivated: appModel.navigateForward()
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
        case "blockchain":
            return blockchainPage
        case "zones":
            return zonesPage
        case "sequencerDashboard":
            return sequencerDashboardPage
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
        case "favorites":
            return favoritesPage
        case "programs":
            return programsPage
        case "localWallet":
            return localWalletPage
        case "localNodes":
            return localNodesPage
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
            pageLoader.sourceComponent = root.pageFor(appModel.shell.currentView)
        })
    }

    function guardNavigation(kind, payload) {
        if (!root.currentPageHasDirtySourceDraft()) {
            return false
        }
        if (kind === "select_view"
                && appModel.normalizedNavigationView(payload && payload.view)
                    === appModel.shell.currentView) {
            return false
        }
        root.pendingNavigationRequest = {
            kind: String(kind || ""),
            payload: payload
        }
        pageNavigationGuard.open()
        return true
    }

    function currentPageHasDirtySourceDraft() {
        return appModel.shell.currentView === "zones"
            && root.loadedZonesPage !== null
            && root.loadedZonesPage.hasDirtyDraft
    }

    function acceptGuardedNavigation() {
        const request = root.pendingNavigationRequest
        root.pendingNavigationRequest = null
        if (!request) {
            return false
        }
        if (root.loadedZonesPage) {
            root.loadedZonesPage.discardSourceDraft()
        }
        return root.executeGuardedNavigation(request)
    }

    function executeGuardedNavigation(request) {
        const payload = request && request.payload ? request.payload : ({})
        switch (String(request && request.kind || "")) {
        case "back":
            return appModel.shell.navigateBack()
        case "forward":
            return appModel.shell.navigateForward()
        case "select_view":
            return appModel.shell.selectView(payload.view, payload.recordHistory)
        case "open_settings":
            return appModel.shell.openSettings(
                payload.section, payload.subsection, payload.recordHistory)
        default:
            return false
        }
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
        DashboardPages.OverviewPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: blocksPage
        BedrockPages.BlocksPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: blockDetailPage
        ChainPages.BlockDetailPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: transactionsPage
        BedrockPages.TransactionsPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: transactionDetailPage
        ChainPages.TransactionDetailPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: blockchainPage
        ModulePages.ModulePage {
            theme: theme
            model: appModel
            moduleKind: "blockchain"
            title: qsTr("Bedrock Node Diagnostics")
            subtitle: qsTr("Inspect Bedrock node state and L1 block windows through direct node HTTP.")
        }
    }

    Component {
        id: zonesPage
        ZonePages.ZonesPage {
            theme: theme
            model: appModel
            initialDetailTab: appModel.zoneInspection.requestedDetailTab
        }
    }

    Component {
        id: sequencerDashboardPage
        ZonePages.SequencerDashboardPage {
            theme: theme
            model: appModel
            initialTab: appModel.zoneInspection.requestedDetailTab
        }
    }

    Component {
        id: storagePage
        StoragePages.StorageAppPage {
            theme: theme
            model: appModel.storageApp
        }
    }

    Component {
        id: messagingPage
        DeliveryPages.DeliveryAppPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: storageDiagnosticsPage
        StoragePages.StoragePage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: deliveryDiagnosticsPage
        DeliveryPages.DeliveryPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: capabilitiesPage
        ModulePages.ModulePage {
            theme: theme
            model: appModel
            moduleKind: "capabilities"
            title: qsTr("Capabilities Diagnostics")
            subtitle: qsTr("Review capability inventory and module availability.")
        }
    }

    Component {
        id: favoritesPage
        LocalPages.FavoritesPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: programsPage
        ProgramPages.ProgramsPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: localWalletPage
        WalletPages.LocalWalletPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: localNodesPage
        LocalPages.LocalNodesPage {
            theme: theme
            model: appModel.localNodes
        }
    }

    Component {
        id: settingsPage
        SettingsPages.SettingsPage {
            theme: theme
            model: appModel
        }
    }
}
