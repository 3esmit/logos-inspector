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
                    contentWidth: availableWidth
                    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    Loader {
                        id: pageLoader
                        active: true
                        asynchronous: true
                        width: pageScroll.availableWidth
                        sourceComponent: root.pageFor(appModel.currentView)
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
        case "transactions":
            return transactionsPage
        case "wallets":
            return walletsPage
        case "blockchain":
            return blockchainPage
        case "channels":
            return channelsPage
        case "storage":
            return storagePage
        case "messaging":
            return messagingPage
        case "capabilities":
            return capabilitiesPage
        case "sequencer":
            return sequencerPage
        case "accounts":
            return accountsPage
        case "programs":
            return programsPage
        case "indexer":
            return indexerPage
        case "settings":
            return settingsPage
        default:
            return overviewPage
        }
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
        id: transactionsPage
        TransactionsPage {
            theme: theme
            model: appModel
        }
    }

    Component {
        id: walletsPage
        WalletsPage {
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
            title: qsTr("Blockchain")
            subtitle: qsTr("Inspect node state, block windows, and blockchain module calls.")
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
        ModulePage {
            theme: theme
            model: appModel
            moduleKind: "storage"
            title: qsTr("Storage")
            subtitle: qsTr("Query storage module metadata and optional CID state.")
        }
    }

    Component {
        id: messagingPage
        ModulePage {
            theme: theme
            model: appModel
            moduleKind: "messaging"
            title: qsTr("Messaging")
            subtitle: qsTr("Inspect delivery module metadata and node info.")
        }
    }

    Component {
        id: capabilitiesPage
        ModulePage {
            theme: theme
            model: appModel
            moduleKind: "capabilities"
            title: qsTr("Capabilities")
            subtitle: qsTr("Review capability inventory and module availability.")
        }
    }

    Component {
        id: sequencerPage
        SequencerPage {
            theme: theme
            model: appModel
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
