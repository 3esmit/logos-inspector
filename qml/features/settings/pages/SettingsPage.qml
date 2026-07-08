pragma ComponentBehavior: Bound

import QtQuick
import QtQml.Models
import QtQuick.Layouts
import "../../../components"
import "../../../components/common"
import "../controls"
import "../../../state"
import "../../../theme"

ColumnLayout {
    id: settingsRoot

    required property Theme theme
    required property AppModel model
    property string pendingSettingsRestoreCid: ""

    width: parent ? parent.width : 900
    spacing: 16

    ListModel {
        id: settingsSections

        ListElement { value: "general"; label: "General" }
        ListElement { value: "network"; label: "Network" }
        ListElement { value: "wallet"; label: "Wallet" }
        ListElement { value: "ui"; label: "User Interface" }
    }

    ListModel {
        id: networkSections

        ListElement { value: "blockchain"; label: "Blockchain" }
        ListElement { value: "indexer"; label: "Indexer" }
        ListElement { value: "execution"; label: "Execution Zone" }
        ListElement { value: "messaging"; label: "Messaging / Delivery" }
        ListElement { value: "storage"; label: "Storage" }
    }

    ListModel {
        id: uiSections

        ListElement { value: "footer"; label: "Footer" }
        ListElement { value: "dashboard"; label: "Dashboard" }
    }

    ListModel {
        id: profileOptions

        ListElement {
            key: "default"
            label: "Testnet"
            summary: "Public LEZ, local indexer and node defaults"
        }
        ListElement {
            key: "local"
            label: "Local sequencer"
            summary: "Local sequencer, indexer, and node"
        }
        ListElement {
            key: "custom"
            label: "Custom"
            summary: "Manual endpoint override"
        }
    }

    ListModel {
        id: coreSourceOptions
    }

    ListModel {
        id: deliverySourceOptions
    }

    ListModel {
        id: storageSourceOptions
    }

    Component.onCompleted: settingsRoot.refreshSourceOptions()

    Connections {
        target: settingsRoot.model

        function onSourcePolicyChanged() {
            settingsRoot.refreshSourceOptions()
        }
    }

    PageHeader {
        theme: settingsRoot.theme
        breadcrumb: qsTr("Home / Settings")
        title: qsTr("Settings")
        layerLabel: qsTr("System")
        subtitle: qsTr("Configure profiles, network connections, wallet sources, footer status fields, and dashboard graphs.")
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: settingsRoot.model.settingsStateError.length > 0
        theme: settingsRoot.theme
        tone: "error"
        title: qsTr("Settings load failed")
        message: settingsRoot.model.settingsStateError
        Layout.fillWidth: true
    }

    TabSwitch {
        theme: settingsRoot.theme
        current: settingsRoot.model.settingsSection
        options: settingsSections
        Layout.fillWidth: true
        onSelected: value => settingsRoot.model.settingsSection = value
    }

    Loader {
        active: true
        asynchronous: true
        sourceComponent: settingsRoot.sectionComponent(settingsRoot.model.settingsSection)
        Layout.fillWidth: true
    }

    Component {
        id: generalSection

        ColumnLayout {
            spacing: settingsRoot.theme.gap
            Layout.fillWidth: true

            GridLayout {
                columns: settingsRoot.width < 760 ? 2 : 4
                columnSpacing: settingsRoot.theme.gap
                rowSpacing: settingsRoot.theme.gap
                Layout.fillWidth: true

                MetricCard {
                    theme: settingsRoot.theme
                    compact: true
                    label: qsTr("Profile")
                    value: settingsRoot.profileLabel(settingsRoot.model.networkProfile)
                    delta: settingsRoot.profileSummary(settingsRoot.model.networkProfile)
                    deltaColor: settingsRoot.model.networkProfile === "custom" ? settingsRoot.theme.warning : settingsRoot.theme.textMuted
                }

                MetricCard {
                    theme: settingsRoot.theme
                    compact: true
                    label: qsTr("Blockchain")
                    value: settingsRoot.connectionStatusText("blockchain")
                    delta: settingsRoot.shortEndpoint(settingsRoot.model.nodeUrl)
                    deltaColor: settingsRoot.connectionStatusColor("blockchain")
                }

                MetricCard {
                    theme: settingsRoot.theme
                    compact: true
                    label: qsTr("Execution Zone")
                    value: settingsRoot.connectionStatusText("execution")
                    delta: settingsRoot.shortEndpoint(settingsRoot.model.sequencerUrl)
                    deltaColor: settingsRoot.connectionStatusColor("execution")
                }

                MetricCard {
                    theme: settingsRoot.theme
                    compact: true
                    label: qsTr("Indexer")
                    value: settingsRoot.connectionStatusText("indexer")
                    delta: settingsRoot.shortEndpoint(settingsRoot.model.indexerUrl)
                    deltaColor: settingsRoot.connectionStatusColor("indexer")
                }
            }

            Panel {
                theme: settingsRoot.theme
                title: qsTr("General")

                ColumnLayout {
                    spacing: settingsRoot.theme.gapSmall
                    Layout.fillWidth: true

                    Text {
                        text: qsTr("Network profile")
                        color: settingsRoot.theme.textMuted
                        textFormat: Text.PlainText
                        font.pixelSize: settingsRoot.theme.secondaryText
                        font.weight: Font.Medium
                        Layout.fillWidth: true
                    }

                    ProfileComboBox {
                        theme: settingsRoot.theme
                        options: profileOptions
                        currentIndex: settingsRoot.profileIndexFor(settingsRoot.model.networkProfile)
                        Layout.fillWidth: true
                        onProfileActivated: index => settingsRoot.applyProfileIndex(index)
                    }

                    StatusMessage {
                        theme: settingsRoot.theme
                        tone: settingsRoot.model.networkProfile === "custom" ? "warning" : "info"
                        title: settingsRoot.profileLabel(settingsRoot.model.networkProfile)
                        message: settingsRoot.profileDetail()
                        Layout.fillWidth: true
                    }
                }
            }

            Panel {
                theme: settingsRoot.theme
                title: qsTr("Backups")

                RowLayout {
                    spacing: settingsRoot.theme.gap
                    Layout.fillWidth: true

                    Text {
                        text: qsTr("Back up local settings, registered IDLs, wallet profile, and favorites to Logos Storage.")
                        color: settingsRoot.theme.textMuted
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: settingsRoot.theme.secondaryText
                        Layout.fillWidth: true
                    }

                    StatusPill {
                        theme: settingsRoot.theme
                        text: settingsRoot.model.settingsBackupAvailable() ? qsTr("Ready") : qsTr("Blocked")
                        colorToken: settingsRoot.model.settingsBackupAvailable() ? settingsRoot.theme.success : settingsRoot.theme.warning
                    }
                }

                StatusMessage {
                    visible: !settingsRoot.model.settingsBackupAvailable()
                    theme: settingsRoot.theme
                    tone: "warning"
                    title: qsTr("Storage backup unavailable")
                    message: qsTr("Select Standalone REST storage and enable mutating diagnostics in Network / Storage.")
                    Layout.fillWidth: true
                }

                GridLayout {
                    columns: settingsRoot.width < 760 ? 1 : 2
                    columnSpacing: settingsRoot.theme.gap
                    rowSpacing: settingsRoot.theme.gap
                    Layout.fillWidth: true

                    InfoField {
                        theme: settingsRoot.theme
                        label: qsTr("Storage REST")
                        value: settingsRoot.model.configuredStorageRestUrl()
                    }

                    FieldRow {
                        id: settingsBackupCidField

                        theme: settingsRoot.theme
                        label: qsTr("Backup CID")
                        placeholderText: qsTr("zDv...")
                        sourceText: settingsRoot.model.settingsRestoreCid.length ? settingsRoot.model.settingsRestoreCid : settingsRoot.model.settingsBackupCid
                        syncSourceText: true
                        onTextEdited: text => settingsRoot.model.settingsRestoreCid = String(text || "").trim()
                    }
                }

                RowLayout {
                    spacing: settingsRoot.theme.gapSmall
                    Layout.fillWidth: true

                    SafetyToggle {
                        theme: settingsRoot.theme
                        text: qsTr("Encrypt with wallet")
                        checked: settingsRoot.model.settingsBackupEncrypted
                        enabled: settingsRoot.model.walletHomeConfigured()
                        detail: qsTr("Uses the configured wallet home. Restore requires the same wallet config.")
                        Layout.preferredWidth: 220
                        onToggled: settingsRoot.model.settingsBackupEncrypted = checked
                    }

                    Text {
                        text: settingsRoot.walletBackupHint()
                        color: settingsRoot.model.settingsBackupEncrypted && !settingsRoot.model.walletHomeConfigured() ? settingsRoot.theme.warning : settingsRoot.theme.textMuted
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        font.pixelSize: settingsRoot.theme.secondaryText
                        Layout.fillWidth: true
                    }
                }

                RowLayout {
                    spacing: settingsRoot.theme.gapSmall
                    Layout.fillWidth: true

                    ActionButton {
                        theme: settingsRoot.theme
                        text: qsTr("Back Up")
                        primary: true
                        enabled: !settingsRoot.model.busy
                            && settingsRoot.model.settingsBackupAvailable()
                            && (!settingsRoot.model.settingsBackupEncrypted || settingsRoot.model.walletHomeConfigured())
                        Layout.preferredWidth: 112
                        onClicked: settingsRoot.model.backupSettingsToStorage(settingsRoot.model.settingsBackupEncrypted)
                    }

                    ActionButton {
                        theme: settingsRoot.theme
                        text: qsTr("Restore")
                        enabled: !settingsRoot.model.busy
                            && settingsRoot.model.settingsBackupAvailable()
                            && settingsBackupCidField.text.trim().length > 0
                        Layout.preferredWidth: 112
                        onClicked: {
                            settingsRoot.pendingSettingsRestoreCid = settingsBackupCidField.text.trim()
                            settingsRestoreConfirm.open()
                        }
                    }

                    ActionButton {
                        theme: settingsRoot.theme
                        text: qsTr("Storage")
                        enabled: !settingsRoot.model.busy
                        Layout.preferredWidth: 104
                        onClicked: settingsRoot.model.openSettings("network", "storage")
                    }

                    Item {
                        Layout.fillWidth: true
                    }
                }

                StatusMessage {
                    visible: settingsRoot.model.settingsBackupStatus.length > 0
                    theme: settingsRoot.theme
                    tone: settingsRoot.model.resultIsError && settingsRoot.model.resultOwner === settingsRoot.model.currentView ? "error" : "info"
                    title: qsTr("Backup status")
                    message: settingsRoot.model.settingsBackupStatus
                    Layout.fillWidth: true
                }
            }
        }
    }

    ConfirmActionPopup {
        id: settingsRestoreConfirm

        theme: settingsRoot.theme
        title: qsTr("Restore settings")
        message: qsTr("This replaces local settings, registered IDLs, wallet profile, and favorites with the selected backup.")
        confirmText: qsTr("Restore")
        confirmEnabled: settingsRoot.pendingSettingsRestoreCid.length > 0
        onAccepted: settingsRoot.model.restoreSettingsFromStorage(settingsRoot.pendingSettingsRestoreCid, settingsRoot.model.settingsBackupEncrypted)
    }

    Component {
        id: networkSection

        ColumnLayout {
            spacing: settingsRoot.theme.gap
            Layout.fillWidth: true

            TabSwitch {
                theme: settingsRoot.theme
                current: settingsRoot.model.settingsNetworkSection
                options: networkSections
                Layout.fillWidth: true
                onSelected: value => settingsRoot.model.settingsNetworkSection = value
            }

            Loader {
                active: true
                asynchronous: true
                sourceComponent: settingsRoot.networkComponent(settingsRoot.model.settingsNetworkSection)
                Layout.fillWidth: true
            }
        }
    }

    Component {
        id: walletSection

        ColumnLayout {
            spacing: settingsRoot.theme.gap
            Layout.fillWidth: true

            GridLayout {
                columns: settingsRoot.width < 760 ? 2 : 4
                columnSpacing: settingsRoot.theme.gap
                rowSpacing: settingsRoot.theme.gap
                Layout.fillWidth: true

                MetricCard {
                    theme: settingsRoot.theme
                    compact: true
                    label: qsTr("Profile")
                    value: settingsRoot.model.walletProfileLabel
                    delta: settingsRoot.walletSourceStatusDetail()
                    deltaColor: settingsRoot.walletSourceStatusColor()
                }

                MetricCard {
                    theme: settingsRoot.theme
                    compact: true
                    label: qsTr("Wallet binary")
                    value: settingsRoot.model.walletBinary.length ? settingsRoot.model.walletBinaryDisplayLabel() : qsTr("Not set")
                    delta: settingsRoot.model.localWalletStatus && settingsRoot.model.localWalletStatus.version ? String(settingsRoot.model.localWalletStatus.version) : qsTr("Version unknown")
                    deltaColor: settingsRoot.model.walletBinary.length ? settingsRoot.theme.textMuted : settingsRoot.theme.warning
                }

                MetricCard {
                    theme: settingsRoot.theme
                    compact: true
                    label: qsTr("Wallet home")
                    value: settingsRoot.model.walletHomeDisplayLabel()
                    delta: settingsRoot.model.walletHomeSourceLabel()
                    deltaColor: settingsRoot.model.walletHome.length ? settingsRoot.theme.textMuted : settingsRoot.theme.warning
                }

                MetricCard {
                    theme: settingsRoot.theme
                    compact: true
                    label: qsTr("Network")
                    value: settingsRoot.profileLabel(settingsRoot.model.networkProfile)
                    delta: settingsRoot.profileDetail()
                    deltaColor: settingsRoot.theme.textMuted
                }
            }

            Panel {
                theme: settingsRoot.theme
                title: qsTr("Local Wallet")

                RowLayout {
                    spacing: settingsRoot.theme.gap
                    Layout.fillWidth: true

                    Text {
                        text: qsTr("Wallet source uses active Network endpoints.")
                        color: settingsRoot.theme.textMuted
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: settingsRoot.theme.secondaryText
                        Layout.fillWidth: true
                    }

                    StatusPill {
                        theme: settingsRoot.theme
                        text: settingsRoot.walletSourceStatusText()
                        colorToken: settingsRoot.walletSourceStatusColor()
                    }
                }

                GridLayout {
                    columns: settingsRoot.width < 760 ? 1 : 2
                    columnSpacing: settingsRoot.theme.gap
                    rowSpacing: settingsRoot.theme.gap
                    Layout.fillWidth: true

                    FieldRow {
                        theme: settingsRoot.theme
                        label: qsTr("Label")
                        sourceText: settingsRoot.model.walletProfileLabel
                        syncSourceText: true
                        onTextEdited: text => { if (settingsRoot.model.walletProfileLabel !== text) settingsRoot.model.walletProfileLabel = text }
                    }

                    FieldRow {
                        theme: settingsRoot.theme
                        label: qsTr("Wallet binary")
                        placeholderText: qsTr("/path/to/wallet")
                        sourceText: settingsRoot.model.walletBinary
                        syncSourceText: true
                        onTextEdited: text => {
                            if (settingsRoot.model.walletBinary !== text) {
                                settingsRoot.model.walletBinary = text
                                settingsRoot.model.clearLocalWalletStatus()
                            }
                        }
                    }

                    FieldRow {
                        theme: settingsRoot.theme
                        label: qsTr("Wallet home")
                        placeholderText: qsTr("$LEE_WALLET_HOME_DIR")
                        sourceText: settingsRoot.model.walletHome
                        syncSourceText: true
                        onTextEdited: text => {
                            if (settingsRoot.model.walletHome !== text) {
                                settingsRoot.model.walletHome = text
                                settingsRoot.model.clearLocalWalletStatus()
                            }
                        }
                    }

                    InfoField {
                        theme: settingsRoot.theme
                        label: qsTr("Sequencer RPC")
                        value: settingsRoot.shortEndpoint(settingsRoot.model.sequencerUrl)
                    }

                    InfoField {
                        theme: settingsRoot.theme
                        label: qsTr("Indexer RPC")
                        value: settingsRoot.shortEndpoint(settingsRoot.model.indexerUrl)
                    }

                    InfoField {
                        theme: settingsRoot.theme
                        label: qsTr("Bedrock node")
                        value: settingsRoot.shortEndpoint(settingsRoot.model.nodeUrl)
                    }
                }

                RowLayout {
                    spacing: settingsRoot.theme.gapSmall
                    Layout.fillWidth: true

                    ActionButton {
                        theme: settingsRoot.theme
                        text: qsTr("Save")
                        primary: true
                        onClicked: settingsRoot.model.saveWalletState()
                    }

                    ActionButton {
                        theme: settingsRoot.theme
                        text: qsTr("Autodetect")
                        onClicked: {
                            settingsRoot.model.detectWalletProfile(true)
                            settingsRoot.model.checkLocalWalletProfile(false)
                        }
                    }

                    ActionButton {
                        theme: settingsRoot.theme
                        text: qsTr("Check")
                        onClicked: settingsRoot.model.checkLocalWalletProfile(false)
                    }

                    Item {
                        Layout.fillWidth: true
                    }
                }
            }

            StatusMessage {
                visible: settingsRoot.model.localWalletStatusError.length > 0
                theme: settingsRoot.theme
                tone: "error"
                title: qsTr("Wallet check failed")
                message: settingsRoot.model.localWalletStatusError
                Layout.fillWidth: true
            }
        }
    }

    Component {
        id: blockchainNetwork

        NetworkConnectionPanel {
            theme: settingsRoot.theme
            title: qsTr("Bedrock Blockchain")
            subtitle: qsTr("Source used for node health, consensus, blocks, and Bedrock RPC inspection.")
            kind: "blockchain"
            pageWidth: settingsRoot.width
            busy: settingsRoot.model.busy
            connectionType: settingsRoot.model.blockchainSourceLabel()
            endpointLabel: qsTr("RPC URL")
            endpoint: settingsRoot.model.nodeUrl
            primaryFieldVisible: true
            moduleName: settingsRoot.model.blockchainModule
            moduleFieldVisible: false
            sourceSelectorVisible: true
            sourceOptions: coreSourceOptions
            sourceIndex: settingsRoot.coreSourceIndexFor(settingsRoot.model.blockchainSourceMode)
            refreshRate: settingsRoot.model.blockchainRefreshRate
            statusText: settingsRoot.connectionStatusText("blockchain")
            statusDetail: settingsRoot.connectionStatusDetail("blockchain")
            statusColor: settingsRoot.connectionStatusColor("blockchain")
            onSourceActivated: index => settingsRoot.model.blockchainSourceMode = settingsRoot.coreSourceModeAt(index)
            onEndpointEdited: value => settingsRoot.updateNodeUrl(value)
            onRefreshRateEdited: value => settingsRoot.model.setNetworkConnectionRate("blockchain", value)
            onQueryClicked: settingsRoot.model.queryNetworkConnection("blockchain", true)
        }
    }

    Component {
        id: indexerNetwork

        NetworkConnectionPanel {
            theme: settingsRoot.theme
            title: qsTr("Indexer")
            subtitle: qsTr("Source used for finalized head, block lookup, transfer activity, and transaction history over RPC.")
            kind: "indexer"
            pageWidth: settingsRoot.width
            busy: settingsRoot.model.busy
            connectionType: settingsRoot.model.indexerSourceLabel()
            endpointLabel: qsTr("RPC URL")
            endpoint: settingsRoot.model.indexerUrl
            primaryFieldVisible: true
            moduleName: settingsRoot.model.indexerModule
            moduleFieldVisible: false
            sourceSelectorVisible: true
            sourceOptions: coreSourceOptions
            sourceIndex: settingsRoot.coreSourceIndexFor(settingsRoot.model.indexerSourceMode)
            refreshRate: settingsRoot.model.indexerRefreshRate
            statusText: settingsRoot.connectionStatusText("indexer")
            statusDetail: settingsRoot.connectionStatusDetail("indexer")
            statusColor: settingsRoot.connectionStatusColor("indexer")
            onSourceActivated: index => settingsRoot.model.indexerSourceMode = settingsRoot.coreSourceModeAt(index)
            onEndpointEdited: value => settingsRoot.updateIndexerUrl(value)
            onRefreshRateEdited: value => settingsRoot.model.setNetworkConnectionRate("indexer", value)
            onQueryClicked: settingsRoot.model.queryNetworkConnection("indexer", true)
        }
    }

    Component {
        id: executionNetwork

        NetworkConnectionPanel {
            theme: settingsRoot.theme
            title: qsTr("Logos Execution Zone")
            subtitle: qsTr("Source used for LEZ head checks. Sequencer blocks, accounts, transactions, and SPEL inspection require RPC.")
            kind: "execution"
            pageWidth: settingsRoot.width
            busy: settingsRoot.model.busy
            connectionType: settingsRoot.model.executionSourceLabel()
            endpointLabel: qsTr("RPC URL")
            endpoint: settingsRoot.model.sequencerUrl
            primaryFieldVisible: true
            moduleFieldVisible: false
            sourceSelectorVisible: false
            refreshRate: settingsRoot.model.executionRefreshRate
            statusText: settingsRoot.connectionStatusText("execution")
            statusDetail: settingsRoot.connectionStatusDetail("execution")
            statusColor: settingsRoot.connectionStatusColor("execution")
            onEndpointEdited: value => settingsRoot.updateSequencerUrl(value)
            onRefreshRateEdited: value => settingsRoot.model.setNetworkConnectionRate("execution", value)
            onQueryClicked: settingsRoot.model.queryNetworkConnection("execution", true)
        }
    }

    Component {
        id: messagingNetwork

        DeliveryConnectionPanel {
            theme: settingsRoot.theme
            title: qsTr("Messaging / Delivery")
            subtitle: qsTr("Configure the Delivery inspection source. Probes here are read-only status checks.")
            pageWidth: settingsRoot.width
            modelRef: settingsRoot.model
            statusText: settingsRoot.connectionStatusText("messaging")
            statusDetail: settingsRoot.connectionStatusDetail("messaging")
            statusColor: settingsRoot.connectionStatusColor("messaging")
            sourceOptions: deliverySourceOptions
            onQueryClicked: settingsRoot.model.queryNetworkConnection("messaging", true)
        }
    }

    Component {
        id: storageNetwork

        StorageConnectionPanel {
            theme: settingsRoot.theme
            title: qsTr("Storage")
            subtitle: qsTr("Configure the Storage inspection source. Safe checks only query identity, space, local manifests, metrics, and optional local exists.")
            pageWidth: settingsRoot.width
            modelRef: settingsRoot.model
            statusText: settingsRoot.connectionStatusText("storage")
            statusDetail: settingsRoot.connectionStatusDetail("storage")
            statusColor: settingsRoot.connectionStatusColor("storage")
            sourceOptions: storageSourceOptions
            onQueryClicked: settingsRoot.model.queryNetworkConnection("storage", true)
        }
    }

    Component {
        id: uiSection

        ColumnLayout {
            spacing: settingsRoot.theme.gap
            Layout.fillWidth: true

            TabSwitch {
                theme: settingsRoot.theme
                current: settingsRoot.model.settingsUiSection
                options: uiSections
                Layout.fillWidth: true
                onSelected: value => settingsRoot.model.settingsUiSection = value
            }

            Loader {
                active: true
                asynchronous: true
                sourceComponent: settingsRoot.uiComponent(settingsRoot.model.settingsUiSection)
                Layout.fillWidth: true
            }
        }
    }

    Component {
        id: footerSettings

        FieldSelector {
            theme: settingsRoot.theme
            title: qsTr("Footer fields")
            description: qsTr("Choose concise status fields for the persistent footer. The footer groups network context on the left and health/action fields on the right.")
            groups: settingsRoot.footerFieldGroups()
            mode: "footer"
            modelRef: settingsRoot.model
        }
    }

    Component {
        id: dashboardSettings

        FieldSelector {
            theme: settingsRoot.theme
            title: qsTr("Dashboard graphs")
            description: qsTr("Choose the live graph tiles shown above dashboard tables.")
            groups: settingsRoot.dashboardGraphGroups()
            mode: "dashboard"
            modelRef: settingsRoot.model
        }
    }

    function sectionComponent(section) {
        switch (section) {
        case "network":
            return networkSection
        case "wallet":
            return walletSection
        case "ui":
            return uiSection
        default:
            return generalSection
        }
    }

    function networkComponent(section) {
        switch (section) {
        case "indexer":
            return indexerNetwork
        case "execution":
            return executionNetwork
        case "messaging":
            return messagingNetwork
        case "storage":
            return storageNetwork
        default:
            return blockchainNetwork
        }
    }

    function uiComponent(section) {
        return section === "dashboard" ? dashboardSettings : footerSettings
    }

    function connectionStatus(kind) {
        return settingsRoot.model.networkConnectionState(kind)
    }

    function connectionStatusText(kind) {
        const status = settingsRoot.connectionStatus(kind)
        if (!status.known) {
            return qsTr("Unknown")
        }
        return status.ok ? qsTr("OK") : qsTr("Error")
    }

    function connectionStatusDetail(kind) {
        const status = settingsRoot.connectionStatus(kind)
        if (!status.known) {
            const rate = settingsRoot.model.networkConnectionRate(kind)
            return rate > 0
                ? qsTr("Not queried. Auto refresh runs every %1 seconds.").arg(rate)
                : qsTr("Not queried. Auto refresh is off.")
        }
        const checked = status.checkedAt && status.checkedAt.length ? qsTr(" at %1").arg(status.checkedAt) : ""
        return qsTr("%1%2").arg(status.detail || "").arg(checked)
    }

    function connectionStatusColor(kind) {
        const status = settingsRoot.connectionStatus(kind)
        if (!status.known) {
            return settingsRoot.theme.textMuted
        }
        return status.ok ? settingsRoot.theme.success : settingsRoot.theme.warning
    }

    function walletSourceStatusText() {
        const status = settingsRoot.model.localWalletStatus || null
        if (!status) {
            return settingsRoot.model.localWalletStatusError.length ? qsTr("Down") : qsTr("Unknown")
        }
        const value = String(status.status || "unknown")
        return value.length ? value[0].toUpperCase() + value.slice(1) : qsTr("Unknown")
    }

    function walletSourceStatusDetail() {
        const status = settingsRoot.model.localWalletStatus || null
        if (settingsRoot.model.localWalletStatusError.length) {
            return settingsRoot.model.localWalletStatusError
        }
        if (status && status.detail) {
            return String(status.detail)
        }
        return qsTr("Not checked")
    }

    function walletSourceStatusColor() {
        const status = settingsRoot.model.localWalletStatus || null
        const value = status && status.status ? String(status.status) : ""
        if (settingsRoot.model.localWalletStatusError.length || value === "down") {
            return settingsRoot.theme.error
        }
        if (!value.length || value === "degraded" || value === "unknown") {
            return settingsRoot.theme.warning
        }
        if (value === "ok") {
            return settingsRoot.theme.success
        }
        return settingsRoot.theme.textMuted
    }

    function walletBackupHint() {
        if (!settingsRoot.model.settingsBackupEncrypted) {
            return qsTr("Plain backup. Use wallet encryption for private or portable profiles.")
        }
        if (!settingsRoot.model.walletHomeConfigured()) {
            return qsTr("Configure Wallet home before encrypted backup or restore.")
        }
        return qsTr("Encrypted restore requires the same wallet config.")
    }

    function updateSequencerUrl(value) {
        settingsRoot.model.sequencerUrl = String(value || "").trim()
        settingsRoot.syncProfileFromEndpoints()
    }

    function updateIndexerUrl(value) {
        settingsRoot.model.indexerUrl = String(value || "").trim()
        settingsRoot.syncProfileFromEndpoints()
    }

    function updateNodeUrl(value) {
        settingsRoot.model.nodeUrl = String(value || "").trim()
        settingsRoot.syncProfileFromEndpoints()
    }

    function syncProfileFromEndpoints() {
        settingsRoot.model.networkProfile = settingsRoot.inferProfile(settingsRoot.model.sequencerUrl, settingsRoot.model.indexerUrl, settingsRoot.model.nodeUrl)
    }

    function applyProfileIndex(index) {
        if (index === 2) {
            settingsRoot.syncProfileFromEndpoints()
            return
        }
        settingsRoot.model.applyProfile(index)
    }

    function deliverySourceIndexFor(value) {
        return settingsRoot.model.sourceModeIndexFor("delivery", value, deliverySourceOptions)
    }

    function deliverySourceModeAt(index) {
        return settingsRoot.model.sourceModeAt(index, deliverySourceOptions)
    }

    function storageSourceIndexFor(value) {
        return settingsRoot.model.sourceModeIndexFor("storage", value, storageSourceOptions)
    }

    function storageSourceModeAt(index) {
        return settingsRoot.model.sourceModeAt(index, storageSourceOptions)
    }

    function coreSourceIndexFor(value) {
        return settingsRoot.model.sourceModeIndexFor("core", value, coreSourceOptions)
    }

    function coreSourceModeAt(index) {
        return settingsRoot.model.sourceModeAt(index, coreSourceOptions)
    }

    function refreshSourceOptions() {
        settingsRoot.populateSourceOptions(coreSourceOptions, "core")
        settingsRoot.populateSourceOptions(deliverySourceOptions, "delivery")
        settingsRoot.populateSourceOptions(storageSourceOptions, "storage")
    }

    function populateSourceOptions(targetModel, family) {
        targetModel.clear()
        const options = settingsRoot.model.sourceModeOptions(family)
        for (let i = 0; i < options.length; ++i) {
            targetModel.append(options[i])
        }
    }

    function profileIndexFor(value) {
        if (value === "local") {
            return 1
        }
        if (value === "custom") {
            return 2
        }
        return 0
    }

    function inferProfile(sequencer, indexer, node) {
        return settingsRoot.model.inferNetworkProfileFromEndpoints(sequencer, indexer, node)
    }

    function profileLabel(value) {
        if (value === "local") {
            return qsTr("Local")
        }
        if (value === "custom") {
            return qsTr("Custom")
        }
        return qsTr("Testnet")
    }

    function profileSummary(value) {
        if (value === "local") {
            return qsTr("All endpoints local")
        }
        if (value === "custom") {
            return qsTr("Manual endpoints")
        }
        return qsTr("Default testnet")
    }

    function profileDetail() {
        return qsTr("%1 / %2 / %3")
            .arg(settingsRoot.shortEndpoint(settingsRoot.model.sequencerUrl))
            .arg(settingsRoot.shortEndpoint(settingsRoot.model.indexerUrl))
            .arg(settingsRoot.shortEndpoint(settingsRoot.model.nodeUrl))
    }

    function normalizeEndpoint(value) {
        return String(value || "").trim().replace(/\/+$/, "")
    }

    function shortEndpoint(value) {
        const text = String(value || "")
        if (!text.length) {
            return qsTr("Not configured")
        }
        return text.replace(/^https?:\/\//, "").replace(/\/$/, "")
    }

    function footerFieldGroups() {
        return [
            { title: qsTr("Network"), fields: [
                { key: "network.network", label: qsTr("network"), detail: qsTr("testnet, mainnet, local, or custom") },
                { key: "network.chain_id", label: qsTr("chain_id"), detail: qsTr("Bedrock chain identifier") },
                { key: "network.zone_id", label: qsTr("zone_id"), detail: qsTr("Execution zone identifier") },
                { key: "network.channel_id", label: qsTr("channel_id"), detail: qsTr("Active delivery channel identifier") },
                { key: "network.report_time", label: qsTr("report_time"), detail: qsTr("Last local report timestamp") }
            ] },
            { title: qsTr("Bedrock Blockchain"), fields: [
                { key: "bedrock.node_health", label: qsTr("node_health"), detail: qsTr("ok, degraded, or down") },
                { key: "bedrock.peer_count", label: qsTr("peer_count"), detail: qsTr("Connected Bedrock peers") },
                { key: "bedrock.sync_state", label: qsTr("sync_state"), detail: qsTr("synced, syncing, or stalled") },
                { key: "bedrock.tip_height", label: qsTr("tip_height"), detail: qsTr("Current tip height") },
                { key: "bedrock.tip_hash", label: qsTr("tip_hash"), detail: qsTr("Current tip hash") },
                { key: "bedrock.lib_height", label: qsTr("lib_height"), detail: qsTr("Last irreversible block height") },
                { key: "bedrock.lib_hash", label: qsTr("lib_hash"), detail: qsTr("Last irreversible block hash") },
                { key: "bedrock.tip_minus_lib", label: qsTr("tip_minus_lib"), detail: qsTr("Distance from tip to LIB") },
                { key: "bedrock.last_tip_time", label: qsTr("last_tip_time"), detail: qsTr("Last tip observation time") },
                { key: "bedrock.last_lib_time", label: qsTr("last_lib_time"), detail: qsTr("Last LIB observation time") },
                { key: "bedrock.finality_lag_seconds", label: qsTr("finality_lag_seconds"), detail: qsTr("Approximate finality lag") }
            ] },
            { title: qsTr("LEZ Sequencer"), fields: [
                { key: "lez.rpc_health", label: qsTr("rpc_health"), detail: qsTr("Sequencer RPC availability") },
                { key: "lez.sequencer_version", label: qsTr("sequencer_version"), detail: qsTr("Sequencer version") },
                { key: "lez.last_lez_block_id", label: qsTr("last_lez_block_id"), detail: qsTr("Latest LEZ block id") },
                { key: "lez.last_lez_block_hash", label: qsTr("last_lez_block_hash"), detail: qsTr("Latest LEZ block hash") },
                { key: "lez.last_lez_block_time", label: qsTr("last_lez_block_time"), detail: qsTr("Latest LEZ block time") },
                { key: "lez.pending_tx_count", label: qsTr("pending_tx_count"), detail: qsTr("Pending sequencer transactions") },
                { key: "lez.mempool_tx_count", label: qsTr("mempool_tx_count"), detail: qsTr("Mempool transaction count") },
                { key: "lez.rejected_tx_count_recent", label: qsTr("rejected_tx_count_recent"), detail: qsTr("Recent rejected transactions") },
                { key: "lez.blocks_produced_recent", label: qsTr("blocks_produced_recent"), detail: qsTr("Recent LEZ blocks produced") },
                { key: "lez.publish_to_bedrock_status", label: qsTr("publish_to_bedrock_status"), detail: qsTr("Bedrock publish state") },
                { key: "lez.last_published_channel_update", label: qsTr("last_published_channel_update"), detail: qsTr("Last channel update publication") },
                { key: "lez.last_finalized_callback_height", label: qsTr("last_finalized_callback_height"), detail: qsTr("Last finalized callback height") },
                { key: "lez.pending_blocks_count", label: qsTr("pending_blocks_count"), detail: qsTr("Pending LEZ blocks") }
            ] },
            { title: qsTr("Indexer"), fields: [
                { key: "indexer.rpc_health", label: qsTr("rpc_health"), detail: qsTr("Indexer RPC availability") },
                { key: "indexer.indexer_version", label: qsTr("indexer_version"), detail: qsTr("Indexer version") },
                { key: "indexer.indexed_finalized_height", label: qsTr("indexed_finalized_height"), detail: qsTr("Indexed finalized height") },
                { key: "indexer.indexed_finalized_hash", label: qsTr("indexed_finalized_hash"), detail: qsTr("Indexed finalized hash") },
                { key: "indexer.indexed_channel_message", label: qsTr("indexed_channel_message"), detail: qsTr("Indexed channel message") },
                { key: "indexer.indexer_lag_vs_sequencer_head", label: qsTr("indexer_lag_vs_sequencer_head"), detail: qsTr("Indexer lag versus sequencer") },
                { key: "indexer.last_indexed_time", label: qsTr("last_indexed_time"), detail: qsTr("Last indexed timestamp") },
                { key: "indexer.db_health", label: qsTr("db_health"), detail: qsTr("Database health") },
                { key: "indexer.ingestion_status", label: qsTr("ingestion_status"), detail: qsTr("running, stalled, or backfilling") }
            ] },
            { title: qsTr("Storage"), fields: [
                { key: "storage.module", label: qsTr("source"), detail: qsTr("REST or metrics source status") },
                { key: "storage.network", label: qsTr("network"), detail: qsTr("Storage preset or network name") },
                { key: "storage.node_reachable", label: qsTr("node_reachable"), detail: qsTr("Storage node reachability") },
                { key: "storage.nat_mode", label: qsTr("nat_mode"), detail: qsTr("upnp, port-forward, or manual") },
                { key: "storage.udp_discovery_port", label: qsTr("udp_discovery_port"), detail: qsTr("UDP discovery port state") },
                { key: "storage.tcp_transfer_port", label: qsTr("tcp_transfer_port"), detail: qsTr("TCP transfer port state") },
                { key: "storage.peer_count", label: qsTr("peer_count"), detail: qsTr("Storage peers") },
                { key: "storage.dht_connected", label: qsTr("dht_connected"), detail: qsTr("DHT connectivity") },
                { key: "storage.shared_files_count", label: qsTr("shared_files_count"), detail: qsTr("Shared files") },
                { key: "storage.manifest_count", label: qsTr("manifest_count"), detail: qsTr("Manifest count") },
                { key: "storage.local_storage_used", label: qsTr("local_storage_used"), detail: qsTr("Local storage usage") },
                { key: "storage.active_uploads", label: qsTr("upload_requests_total"), detail: qsTr("Upload request counter total") },
                { key: "storage.active_downloads", label: qsTr("download_requests_total"), detail: qsTr("Download request counter total") },
                { key: "storage.failed_transfers_recent", label: qsTr("failed_transfers_recent"), detail: qsTr("Recent transfer failures") },
                { key: "storage.cid_fetch_test", label: qsTr("cid_fetch_test"), detail: qsTr("CID fetch probe result") },
                { key: "storage.last_error", label: qsTr("last_error"), detail: qsTr("Latest storage error") }
            ] },
            { title: qsTr("Messaging / Delivery"), fields: [
                { key: "messaging.module", label: qsTr("source"), detail: qsTr("REST or metrics source status") },
                { key: "messaging.connection_state", label: qsTr("connection_state"), detail: qsTr("connected, disconnected, or connecting") },
                { key: "messaging.peer_count", label: qsTr("peer_count"), detail: qsTr("Delivery peers") },
                { key: "messaging.active_subscriptions", label: qsTr("active_subscriptions"), detail: qsTr("Not exposed by current Delivery metrics") },
                { key: "messaging.content_topics", label: qsTr("content_topics"), detail: qsTr("Subscribed content topics") },
                { key: "messaging.outbound_queue", label: qsTr("outbound_queue"), detail: qsTr("Outbound message queue") },
                { key: "messaging.message_sent_events_recent", label: qsTr("message_sent_events_recent"), detail: qsTr("Not exposed by current Delivery metrics") },
                { key: "messaging.message_propagated_events_recent", label: qsTr("message_propagated_events_recent"), detail: qsTr("Not exposed by current Delivery metrics") },
                { key: "messaging.message_received_events_recent", label: qsTr("waku_node_messages_total"), detail: qsTr("Delivery message counter total") },
                { key: "messaging.message_error_events_recent", label: qsTr("waku_node_errors_total"), detail: qsTr("Delivery error counter total") },
                { key: "messaging.publish_latency_ms", label: qsTr("publish_latency_ms"), detail: qsTr("Not exposed by current Delivery metrics") },
                { key: "messaging.receive_latency_ms", label: qsTr("receive_latency_ms"), detail: qsTr("Not exposed by current Delivery metrics") },
                { key: "messaging.last_error", label: qsTr("last_error"), detail: qsTr("Latest Delivery error") }
            ] },
            { title: qsTr("Overall"), fields: [
                { key: "overall.status", label: qsTr("status"), detail: qsTr("healthy, degraded, or down") },
                { key: "overall.main_risk", label: qsTr("main_risk"), detail: qsTr("Most important current risk") },
                { key: "overall.operator_action", label: qsTr("operator_action"), detail: qsTr("Suggested operator action") }
            ] }
        ]
    }

    function dashboardGraphGroups() {
        return [
            { title: qsTr("Bedrock Blockchain"), fields: [
                { key: "bedrock.peer_count", label: qsTr("peer_count"), detail: qsTr("Connected Bedrock peers") },
                { key: "bedrock.tip_minus_lib", label: qsTr("tip_minus_lib"), detail: qsTr("Tip to LIB distance") },
                { key: "bedrock.finality_lag_seconds", label: qsTr("finality_lag_seconds"), detail: qsTr("Finality lag in seconds") }
            ] },
            { title: qsTr("LEZ Sequencer"), fields: [
                { key: "lez.pending_tx_count", label: qsTr("pending_tx_count"), detail: qsTr("Pending sequencer transactions") },
                { key: "lez.mempool_tx_count", label: qsTr("mempool_tx_count"), detail: qsTr("Mempool transaction count") },
                { key: "lez.rejected_tx_count_recent", label: qsTr("rejected_tx_count_recent"), detail: qsTr("Recent rejected transactions") },
                { key: "lez.blocks_produced_recent", label: qsTr("blocks_produced_recent"), detail: qsTr("Recent produced blocks") },
                { key: "lez.pending_blocks_count", label: qsTr("pending_blocks_count"), detail: qsTr("Pending LEZ blocks") }
            ] },
            { title: qsTr("Indexer"), fields: [
                { key: "indexer.indexer_lag_vs_sequencer_head", label: qsTr("indexer_lag_vs_sequencer_head"), detail: qsTr("Indexer lag versus sequencer head") }
            ] },
            { title: qsTr("Storage"), fields: [
                { key: "storage.peer_count", label: qsTr("peer_count"), detail: qsTr("Storage peers") },
                { key: "storage.shared_files_count", label: qsTr("shared_files_count"), detail: qsTr("Shared files") },
                { key: "storage.manifest_count", label: qsTr("manifest_count"), detail: qsTr("Manifests") },
                { key: "storage.local_storage_used", label: qsTr("local_storage_used"), detail: qsTr("Local storage usage") },
                { key: "storage.active_uploads", label: qsTr("upload_requests_total"), detail: qsTr("Upload request counter total") },
                { key: "storage.active_downloads", label: qsTr("download_requests_total"), detail: qsTr("Download request counter total") },
                { key: "storage.failed_transfers_total", label: qsTr("transfer_failures_total"), detail: qsTr("Historical transfer failure counter total") }
            ] },
            { title: qsTr("Messaging / Delivery"), fields: [
                { key: "messaging.peer_count", label: qsTr("peer_count"), detail: qsTr("Delivery peers") },
                { key: "messaging.active_subscriptions", label: qsTr("active_subscriptions"), detail: qsTr("Not exposed by current Delivery metrics") },
                { key: "messaging.content_topics", label: qsTr("content_topics"), detail: qsTr("Content topics") },
                { key: "messaging.outbound_queue", label: qsTr("outbound_queue"), detail: qsTr("Outbound queue") },
                { key: "messaging.message_sent_events_recent", label: qsTr("message_sent_events_recent"), detail: qsTr("Not exposed by current Delivery metrics") },
                { key: "messaging.message_propagated_events_recent", label: qsTr("message_propagated_events_recent"), detail: qsTr("Not exposed by current Delivery metrics") },
                { key: "messaging.message_received_events_recent", label: qsTr("waku_node_messages_total"), detail: qsTr("Delivery message counter total") },
                { key: "messaging.message_error_events_recent", label: qsTr("waku_node_errors_total"), detail: qsTr("Delivery error counter total") },
                { key: "messaging.publish_latency_ms", label: qsTr("publish_latency_ms"), detail: qsTr("Not exposed by current Delivery metrics") },
                { key: "messaging.receive_latency_ms", label: qsTr("receive_latency_ms"), detail: qsTr("Not exposed by current Delivery metrics") }
            ] }
        ]
    }

}
