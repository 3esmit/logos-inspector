pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQml.Models
import QtQuick.Layouts
import "../../../components"
import "../../../components/common"
import "../controls"
import "../../../state"
import "../../../state/settings/SettingsProfileWorkspace.js" as SettingsProfileWorkspace
import "../../../state/backup" as Backup
import "../../../theme"

ColumnLayout {
    id: settingsRoot

    required property Theme theme
    required property AppModel model
    property string pendingSettingsDownloadCid: ""
    property alias pendingSettingsRestoreBackupId: backupRestoreDialog.backupId
    property alias pendingSettingsRestoreOptions: backupRestoreDialog.options
    property alias pendingSettingsRestorePlan: backupRestoreDialog.plan
    property alias pendingSettingsRestorePlanError: backupRestoreDialog.planError

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

        ListElement { key: "default"; label: "Testnet"; summary: "Local nodes with Logos Testnet" }
        ListElement {
            key: "custom"
            label: "Custom"
            summary: "Manual L1 endpoint"
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

    ListModel {
        id: replaceSkipImportOptions

        ListElement {
            key: "replace"
            label: "Replace"
            summary: "Use backup values"
        }
        ListElement {
            key: "skip"
            label: "Skip"
            summary: "Leave current values"
        }
    }

    ListModel {
        id: mergeImportOptions

        ListElement {
            key: "merge"
            label: "Merge"
            summary: "Keep local rows and add backup rows"
        }
        ListElement {
            key: "replace"
            label: "Replace"
            summary: "Use backup rows"
        }
        ListElement {
            key: "skip"
            label: "Skip"
            summary: "Leave current rows"
        }
    }

    ListModel {
        id: conflictDecisionOptions

        ListElement {
            key: "required"
            label: "Choose"
            summary: "Decision required"
        }
        ListElement {
            key: "replace_existing"
            label: "Replace"
            summary: "Use backup item"
        }
        ListElement {
            key: "skip_backup_item"
            label: "Skip"
            summary: "Keep current item"
        }
    }

    Component.onCompleted: {
        settingsRoot.refreshProfileOptions()
        settingsRoot.refreshSourceOptions()
    }

    Backup.BackupImportDialogState {
        id: backupRestoreDialog

        model: settingsRoot.model.backupImport
    }

    Connections {
        target: settingsRoot.model.sourceRouting

        function onSourcePolicyChanged() {
            settingsRoot.refreshProfileOptions()
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
                columns: settingsRoot.width < 760 ? 1 : 2
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

                    GridLayout {
                        columns: settingsRoot.width < 520 ? 1 : 2
                        columnSpacing: settingsRoot.theme.gapSmall
                        rowSpacing: settingsRoot.theme.gapTiny
                        Layout.fillWidth: true

                        FieldToggle {
                            theme: settingsRoot.theme
                            label: qsTr("Local Nodes")
                            detail: qsTr("Enable local node control capabilities.")
                            checked: settingsRoot.model.localNodesEnabled
                            Layout.fillWidth: true
                            onToggled: {
                                settingsRoot.model.localNodesEnabled = checked
                                if (!checked) {
                                    settingsRoot.model.localDevnetEnabled = false
                                }
                            }
                        }

                        FieldToggle {
                            theme: settingsRoot.theme
                            label: qsTr("Local Devnet")
                            detail: qsTr("Enable local devnet sequencer capabilities.")
                            enabled: settingsRoot.model.localNodesEnabled
                            checked: settingsRoot.model.localDevnetEnabled
                            Layout.fillWidth: true
                            onToggled: settingsRoot.model.localDevnetEnabled = checked
                        }
                    }

                    RowLayout {
                        spacing: settingsRoot.theme.gapSmall
                        Layout.fillWidth: true

                        ActionButton {
                            theme: settingsRoot.theme
                            text: qsTr("Restore Testnet defaults")
                            enabled: !settingsRoot.model.shell.busy
                            onClicked: testnetDefaultsConfirm.open()
                        }

                        Text {
                            text: qsTr("Resets network and UI settings. Wallet stays unchanged.")
                            color: settingsRoot.theme.textMuted
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: settingsRoot.theme.secondaryText
                            Layout.fillWidth: true
                        }
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
                        text: qsTr("Back up local settings, registered IDLs, wallet profile, and favorites to the local catalog. Upload selected catalog entries to Logos Storage when needed.")
                        color: settingsRoot.theme.textMuted
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: settingsRoot.theme.secondaryText
                        Layout.fillWidth: true
                    }

                    StatusPill {
                        theme: settingsRoot.theme
                        text: settingsRoot.model.backupCatalogLoaded ? qsTr("Catalog") : qsTr("Loading")
                        colorToken: settingsRoot.model.backupCatalogLoaded ? settingsRoot.theme.success : settingsRoot.theme.warning
                    }
                }

                StatusMessage {
                    visible: !settingsRoot.model.settingsBackupAvailable()
                    theme: settingsRoot.theme
                    tone: "warning"
                    title: qsTr("Storage upload unavailable")
                    message: qsTr("Storage backup upload capability is required before uploading catalog entries.")
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
                        value: settingsRoot.model.sourceRouting.configuredStorageRestUrl()
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

                Text {
                    text: qsTr("Backup Contents")
                    color: settingsRoot.theme.text
                    textFormat: Text.PlainText
                    font.pixelSize: settingsRoot.theme.secondaryText
                    font.weight: Font.Medium
                    Layout.fillWidth: true
                }

                GridLayout {
                    columns: settingsRoot.width < 700 ? 1 : 4
                    columnSpacing: settingsRoot.theme.gapSmall
                    rowSpacing: settingsRoot.theme.gapTiny
                    Layout.fillWidth: true

                    FieldToggle {
                        theme: settingsRoot.theme
                        label: qsTr("Settings")
                        checked: settingsRoot.model.normalizedBackupContents(settingsRoot.model.settingsBackupContents).settings
                        Layout.fillWidth: true
                        onToggled: settingsRoot.model.setSettingsBackupContent("settings", checked)
                    }

                    FieldToggle {
                        theme: settingsRoot.theme
                        label: qsTr("Favorites")
                        checked: settingsRoot.model.normalizedBackupContents(settingsRoot.model.settingsBackupContents).favorites
                        Layout.fillWidth: true
                        onToggled: settingsRoot.model.setSettingsBackupContent("favorites", checked)
                    }

                    FieldToggle {
                        theme: settingsRoot.theme
                        label: qsTr("IDL Registry")
                        checked: settingsRoot.model.normalizedBackupContents(settingsRoot.model.settingsBackupContents).idl_registry
                        Layout.fillWidth: true
                        onToggled: settingsRoot.model.setSettingsBackupContent("idl_registry", checked)
                    }

                    FieldToggle {
                        theme: settingsRoot.theme
                        label: qsTr("Wallet Profile")
                        checked: settingsRoot.model.normalizedBackupContents(settingsRoot.model.settingsBackupContents).wallet_profile
                        Layout.fillWidth: true
                        onToggled: settingsRoot.model.setSettingsBackupContent("wallet_profile", checked)
                    }
                }

                RowLayout {
                    spacing: settingsRoot.theme.gapSmall
                    Layout.fillWidth: true

                    ActionButton {
                        theme: settingsRoot.theme
                        text: qsTr("Create Local")
                        primary: true
                        enabled: !settingsRoot.model.shell.busy
                            && !settingsRoot.model.backupCatalogTransferRunning
                            && !settingsRoot.model.backupCatalogImportRunning
                            && settingsRoot.model.backupContentsSelected(settingsRoot.model.settingsBackupContents)
                            && (!settingsRoot.model.settingsBackupEncrypted || settingsRoot.model.walletHomeConfigured())
                        Layout.preferredWidth: 128
                        onClicked: {
                            const entry = settingsRoot.model.createLocalSettingsBackup(settingsRoot.model.settingsBackupEncrypted ? qsTr("Encrypted settings backup") : qsTr("Settings backup"), settingsRoot.model.settingsBackupEncrypted, settingsRoot.model.settingsBackupContents)
                            settingsRoot.model.settingsBackupStatus = entry
                                ? qsTr("Local backup %1 created.").arg(entry.backup_version_label || entry.backup_catalog_id)
                                : settingsRoot.model.backupImport.backupCatalogError
                        }
                    }

                    ActionButton {
                        theme: settingsRoot.theme
                        text: qsTr("Upload")
                        enabled: !settingsRoot.model.shell.busy
                            && !settingsRoot.model.backupCatalogTransferRunning
                            && !settingsRoot.model.backupCatalogImportRunning
                            && settingsRoot.model.settingsBackupAvailable()
                            && settingsRoot.model.backupContentsSelected(settingsRoot.model.settingsBackupContents)
                            && (!settingsRoot.model.settingsBackupEncrypted || settingsRoot.model.walletHomeConfigured())
                        Layout.preferredWidth: 112
                        onClicked: settingsRoot.model.backupSettingsToStorage(settingsRoot.model.settingsBackupEncrypted, settingsRoot.model.settingsBackupContents)
                    }

                    ActionButton {
                        theme: settingsRoot.theme
                        text: qsTr("Download")
                        enabled: !settingsRoot.model.shell.busy
                            && !settingsRoot.model.backupCatalogTransferRunning
                            && !settingsRoot.model.backupCatalogImportRunning
                            && settingsRoot.model.settingsBackupDownloadAvailable()
                            && settingsBackupCidField.text.trim().length > 0
                        Layout.preferredWidth: 116
                        onClicked: {
                            settingsRoot.pendingSettingsDownloadCid = settingsBackupCidField.text.trim()
                            settingsDownloadConfirm.open()
                        }
                    }

                    ActionButton {
                        theme: settingsRoot.theme
                        text: qsTr("Storage")
                        enabled: !settingsRoot.model.shell.busy
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
                    tone: settingsRoot.model.backupCatalogError.length > 0
                        || (settingsRoot.model.shell.resultIsError
                            && settingsRoot.model.shell.resultOwner === settingsRoot.model.shell.currentView)
                        ? "error" : "info"
                    title: qsTr("Backup status")
                    message: settingsRoot.model.settingsBackupStatus
                    Layout.fillWidth: true
                }

                ColumnLayout {
                    spacing: settingsRoot.theme.gapSmall
                    Layout.fillWidth: true

                    Text {
                        text: qsTr("Local Backup Catalog")
                        color: settingsRoot.theme.text
                        textFormat: Text.PlainText
                        font.pixelSize: settingsRoot.theme.primaryText
                        font.weight: Font.Medium
                        Layout.fillWidth: true
                    }

                    Repeater {
                        model: settingsRoot.model.backupCatalogRows()

                        delegate: RowLayout {
                            id: backupCatalogRow

                            required property var modelData

                            spacing: settingsRoot.theme.gapSmall
                            Layout.fillWidth: true

                            Text {
                                text: String(backupCatalogRow.modelData.backup_version_label || backupCatalogRow.modelData.backup_catalog_id || "")
                                color: settingsRoot.theme.text
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }

                            Text {
                                text: String(backupCatalogRow.modelData.remote && backupCatalogRow.modelData.remote.cid ? backupCatalogRow.modelData.remote.cid : backupCatalogRow.modelData.encrypted ? qsTr("Encrypted local") : qsTr("Local"))
                                color: settingsRoot.theme.textMuted
                                textFormat: Text.PlainText
                                elide: Text.ElideMiddle
                                Layout.preferredWidth: 160
                            }

                            ActionButton {
                                theme: settingsRoot.theme
                                text: qsTr("Restore")
                                enabled: !settingsRoot.model.shell.busy
                                    && !settingsRoot.model.backupCatalogTransferRunning
                                    && !settingsRoot.model.backupCatalogImportRunning
                                Layout.preferredWidth: 96
                                onClicked: {
                                    settingsRoot.pendingSettingsRestoreBackupId = String(backupCatalogRow.modelData.backup_catalog_id || "")
                                    settingsRoot.resetPendingSettingsRestoreOptions()
                                    settingsRoot.previewPendingLocalRestore()
                                    localSettingsRestoreConfirm.open()
                                }
                            }

                            ActionButton {
                                theme: settingsRoot.theme
                                text: qsTr("Upload")
                                enabled: !settingsRoot.model.shell.busy
                                    && !settingsRoot.model.backupCatalogTransferRunning
                                    && !settingsRoot.model.backupCatalogImportRunning
                                    && settingsRoot.model.settingsBackupAvailable()
                                Layout.preferredWidth: 88
                                onClicked: settingsRoot.model.backupImport.uploadBackupCatalogEntry(
                                    String(backupCatalogRow.modelData.backup_catalog_id || ""))
                            }
                        }
                    }

                    StatusMessage {
                        visible: settingsRoot.model.backupCatalogLoaded && settingsRoot.model.backupCatalogRows().length === 0
                        theme: settingsRoot.theme
                        tone: "info"
                        title: qsTr("No local backups")
                        message: qsTr("Create a local backup before uploading or restoring.")
                        Layout.fillWidth: true
                    }
                }
            }
        }
    }

    ConfirmActionPopup {
        id: testnetDefaultsConfirm

        theme: settingsRoot.theme
        title: qsTr("Restore Testnet defaults")
        message: qsTr("Restore local Testnet nodes and Logos Execution Zone sources. Wallet and registered IDLs remain unchanged.")
        confirmText: qsTr("Restore defaults")
        confirmEnabled: !settingsRoot.model.shell.busy
        onAccepted: settingsRoot.model.restoreDefaultSettings()
    }

    ConfirmActionPopup {
        id: settingsDownloadConfirm

        theme: settingsRoot.theme
        title: qsTr("Download backup")
        message: qsTr("This downloads the CID into the Local Backup Catalog. Import is done from the catalog.")
        confirmText: qsTr("Download")
        confirmEnabled: settingsRoot.pendingSettingsDownloadCid.length > 0
            && !settingsRoot.model.backupCatalogTransferRunning
            && !settingsRoot.model.backupCatalogImportRunning
        onAccepted: settingsRoot.model.downloadSettingsBackupToCatalog(
            settingsRoot.pendingSettingsDownloadCid)
    }

    Item {
        visible: false
        implicitWidth: 0
        implicitHeight: 0

        Popup {
            id: localSettingsRestoreConfirm

            parent: Overlay.overlay
            modal: true
            focus: true
            padding: settingsRoot.theme.gap
            width: Math.min(620, parent ? Math.max(0, parent.width - 24) : 620)
            x: parent ? Math.max(0, (parent.width - width) / 2) : 0
            y: 72
            closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

            background: Rectangle {
                color: settingsRoot.theme.surface
                radius: settingsRoot.theme.radius
                border.width: 1
                border.color: settingsRoot.theme.outline
            }

            contentItem: ColumnLayout {
                spacing: settingsRoot.theme.gapSmall

                Text {
                    text: qsTr("Import local backup")
                    color: settingsRoot.theme.text
                    textFormat: Text.PlainText
                    font.pixelSize: settingsRoot.theme.primaryText
                    font.weight: Font.DemiBold
                    Layout.fillWidth: true
                }

                Text {
                    text: settingsRoot.pendingSettingsRestoreBackupId
                    color: settingsRoot.theme.textMuted
                    textFormat: Text.PlainText
                    elide: Text.ElideMiddle
                    font.pixelSize: settingsRoot.theme.secondaryText
                    Layout.fillWidth: true
                }

                GridLayout {
                    columns: settingsRoot.width < 700 ? 1 : 2
                    columnSpacing: settingsRoot.theme.gap
                    rowSpacing: settingsRoot.theme.gapSmall
                    Layout.fillWidth: true

                    ColumnLayout {
                        spacing: settingsRoot.theme.gapTiny
                        Layout.fillWidth: true

                        Text {
                            text: qsTr("Settings")
                            color: settingsRoot.theme.textMuted
                            textFormat: Text.PlainText
                            font.pixelSize: settingsRoot.theme.secondaryText
                            font.weight: Font.Medium
                            Layout.fillWidth: true
                        }

                        ProfileComboBox {
                            theme: settingsRoot.theme
                            accessibleName: qsTr("Settings import mode")
                            options: replaceSkipImportOptions
                            currentIndex: settingsRoot.importModeIndexFor("settings", replaceSkipImportOptions)
                            Layout.fillWidth: true
                            onProfileActivated: index => settingsRoot.setPendingImportMode("settings", settingsRoot.importModeAt(index, replaceSkipImportOptions))
                        }
                    }

                    ColumnLayout {
                        spacing: settingsRoot.theme.gapTiny
                        Layout.fillWidth: true

                        Text {
                            text: qsTr("Favorites")
                            color: settingsRoot.theme.textMuted
                            textFormat: Text.PlainText
                            font.pixelSize: settingsRoot.theme.secondaryText
                            font.weight: Font.Medium
                            Layout.fillWidth: true
                        }

                        ProfileComboBox {
                            theme: settingsRoot.theme
                            accessibleName: qsTr("Favorites import mode")
                            options: mergeImportOptions
                            currentIndex: settingsRoot.importModeIndexFor("favorites", mergeImportOptions)
                            Layout.fillWidth: true
                            onProfileActivated: index => settingsRoot.setPendingImportMode("favorites", settingsRoot.importModeAt(index, mergeImportOptions))
                        }
                    }

                    ColumnLayout {
                        spacing: settingsRoot.theme.gapTiny
                        Layout.fillWidth: true

                        Text {
                            text: qsTr("IDL Registry")
                            color: settingsRoot.theme.textMuted
                            textFormat: Text.PlainText
                            font.pixelSize: settingsRoot.theme.secondaryText
                            font.weight: Font.Medium
                            Layout.fillWidth: true
                        }

                        ProfileComboBox {
                            theme: settingsRoot.theme
                            accessibleName: qsTr("IDL registry import mode")
                            options: mergeImportOptions
                            currentIndex: settingsRoot.importModeIndexFor("idl_registry", mergeImportOptions)
                            Layout.fillWidth: true
                            onProfileActivated: index => settingsRoot.setPendingImportMode("idl_registry", settingsRoot.importModeAt(index, mergeImportOptions))
                        }
                    }

                    ColumnLayout {
                        spacing: settingsRoot.theme.gapTiny
                        Layout.fillWidth: true

                        Text {
                            text: qsTr("Wallet Profile")
                            color: settingsRoot.theme.textMuted
                            textFormat: Text.PlainText
                            font.pixelSize: settingsRoot.theme.secondaryText
                            font.weight: Font.Medium
                            Layout.fillWidth: true
                        }

                        ProfileComboBox {
                            theme: settingsRoot.theme
                            accessibleName: qsTr("Wallet profile import mode")
                            options: replaceSkipImportOptions
                            currentIndex: settingsRoot.importModeIndexFor("wallet_profile", replaceSkipImportOptions)
                            Layout.fillWidth: true
                            onProfileActivated: index => settingsRoot.setPendingImportMode("wallet_profile", settingsRoot.importModeAt(index, replaceSkipImportOptions))
                        }
                    }
                }

                ColumnLayout {
                    visible: settingsRoot.pendingImportItemRows("favorites").length > 0
                    spacing: settingsRoot.theme.gapTiny
                    Layout.fillWidth: true

                    Text {
                        text: qsTr("Favorite Items")
                        color: settingsRoot.theme.textMuted
                        textFormat: Text.PlainText
                        font.pixelSize: settingsRoot.theme.secondaryText
                        font.weight: Font.Medium
                        Layout.fillWidth: true
                    }

                    Repeater {
                        model: settingsRoot.pendingImportItemRows("favorites")

                        delegate: FieldToggle {
                            required property var modelData

                            theme: settingsRoot.theme
                            label: String(modelData.label || modelData.key || "")
                            checked: settingsRoot.pendingImportItemSelected("favorites", String(modelData.key || ""))
                            detail: String(modelData.key || "")
                            Layout.fillWidth: true
                            onToggled: settingsRoot.setPendingImportItemSelected("favorites", String(modelData.key || ""), checked)
                        }
                    }
                }

                ColumnLayout {
                    visible: settingsRoot.pendingImportItemRows("idl_registry").length > 0
                    spacing: settingsRoot.theme.gapTiny
                    Layout.fillWidth: true

                    Text {
                        text: qsTr("IDL Items")
                        color: settingsRoot.theme.textMuted
                        textFormat: Text.PlainText
                        font.pixelSize: settingsRoot.theme.secondaryText
                        font.weight: Font.Medium
                        Layout.fillWidth: true
                    }

                    Repeater {
                        model: settingsRoot.pendingImportItemRows("idl_registry")

                        delegate: FieldToggle {
                            required property var modelData

                            theme: settingsRoot.theme
                            label: String(modelData.label || modelData.key || "")
                            checked: settingsRoot.pendingImportItemSelected("idl_registry", String(modelData.key || ""))
                            detail: String(modelData.key || "")
                            Layout.fillWidth: true
                            onToggled: settingsRoot.setPendingImportItemSelected("idl_registry", String(modelData.key || ""), checked)
                        }
                    }
                }

                ColumnLayout {
                    visible: settingsRoot.pendingImportConflictRows().length > 0
                    spacing: settingsRoot.theme.gapTiny
                    Layout.fillWidth: true

                    Text {
                        text: qsTr("Import Conflicts")
                        color: settingsRoot.theme.textMuted
                        textFormat: Text.PlainText
                        font.pixelSize: settingsRoot.theme.secondaryText
                        font.weight: Font.Medium
                        Layout.fillWidth: true
                    }

                    Repeater {
                        model: settingsRoot.pendingImportConflictRows()

                        delegate: RowLayout {
                            id: conflictDelegate

                            required property var modelData

                            spacing: settingsRoot.theme.gapSmall
                            Layout.fillWidth: true

                            Text {
                                text: String(conflictDelegate.modelData.label || conflictDelegate.modelData.key || "")
                                color: settingsRoot.theme.text
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }

                            ProfileComboBox {
                                theme: settingsRoot.theme
                                accessibleName: qsTr("Import conflict decision")
                                options: conflictDecisionOptions
                                currentIndex: settingsRoot.conflictDecisionIndexFor(String(conflictDelegate.modelData.area || ""), String(conflictDelegate.modelData.key || ""))
                                Layout.preferredWidth: 180
                                onProfileActivated: index => settingsRoot.setPendingImportConflictDecision(String(conflictDelegate.modelData.area || ""), String(conflictDelegate.modelData.key || ""), settingsRoot.importModeAt(index, conflictDecisionOptions))
                            }
                        }
                    }
                }

                StatusMessage {
                    visible: settingsRoot.pendingImportPlanText().length > 0
                    theme: settingsRoot.theme
                    tone: settingsRoot.pendingSettingsRestorePlanError.length ? "error" : "info"
                    title: settingsRoot.pendingSettingsRestorePlanError.length ? qsTr("Import plan failed") : qsTr("Import plan")
                    message: settingsRoot.pendingImportPlanText()
                    Layout.fillWidth: true
                }

                RowLayout {
                    spacing: settingsRoot.theme.gapSmall
                    Layout.fillWidth: true

                    Item {
                        Layout.fillWidth: true
                    }

                    ActionButton {
                        theme: settingsRoot.theme
                        text: qsTr("Cancel")
                        onClicked: localSettingsRestoreConfirm.close()
                    }

                    ActionButton {
                        theme: settingsRoot.theme
                        text: qsTr("Import")
                        primary: true
                        enabled: settingsRoot.pendingImportConfirmEnabled()
                            && !settingsRoot.model.backupCatalogTransferRunning
                            && !settingsRoot.model.backupCatalogImportRunning
                        onClicked: {
                            const options = settingsRoot.copyPendingSettingsRestoreOptions()
                            localSettingsRestoreConfirm.close()
                            settingsRoot.model.backupImport.restoreLocalSettingsBackup(
                                settingsRoot.pendingSettingsRestoreBackupId, options)
                        }
                    }
                }
            }
        }
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
            busy: settingsRoot.model.shell.busy
            connectionType: settingsRoot.model.sourceRouting.blockchainSourceLabel()
            endpointLabel: qsTr("RPC URL")
            endpoint: settingsRoot.model.nodeUrl
            primaryFieldVisible: settingsRoot.model.sourceRouting.sourceModeUsesInput(
                "core",
                settingsRoot.model.currentConnectorSourceMode("l1", "rpc"),
                "rpc_endpoint"
            )
            moduleName: settingsRoot.model.blockchainModule
            moduleFieldVisible: !primaryFieldVisible
            sourceSelectorVisible: true
            sourceOptions: coreSourceOptions
            sourceIndex: settingsRoot.coreSourceIndexFor(settingsRoot.model.currentConnectorSourceMode("l1", "rpc"))
            refreshRate: settingsRoot.model.metrics.blockchainRefreshRate
            statusText: settingsRoot.connectionStatusText("blockchain")
            statusDetail: settingsRoot.connectionStatusDetail("blockchain")
            statusColor: settingsRoot.connectionStatusColor("blockchain")
            onSourceActivated: index => settingsRoot.model.setNetworkConnectorMode("l1", settingsRoot.coreSourceModeAt(index))
            onEndpointEdited: value => settingsRoot.updateNodeUrl(value)
            onRefreshRateEdited: value => settingsRoot.model.metrics.setNetworkConnectionRate("blockchain", value)
            onQueryClicked: settingsRoot.model.metrics.queryNetworkConnection("blockchain", true)
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
            onQueryClicked: settingsRoot.model.metrics.queryNetworkConnection("messaging", true)
        }
    }

    Component {
        id: storageNetwork

        StorageConnectionPanel {
            theme: settingsRoot.theme
            title: qsTr("Storage")
            subtitle: qsTr("Configure the Storage inspection source. Status checks are read-only.")
            pageWidth: settingsRoot.width
            modelRef: settingsRoot.model
            statusText: settingsRoot.connectionStatusText("storage")
            statusDetail: settingsRoot.connectionStatusDetail("storage")
            statusColor: settingsRoot.connectionStatusColor("storage")
            sourceOptions: storageSourceOptions
            onQueryClicked: SettingsProfileWorkspace.queryStorageStatus(settingsRoot)
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

    function connectionStatus(kind) { return SettingsProfileWorkspace.connectionStatus(settingsRoot, kind) }
    function connectionStatusText(kind) { return SettingsProfileWorkspace.connectionStatusText(settingsRoot, kind) }
    function connectionStatusDetail(kind) { return SettingsProfileWorkspace.connectionStatusDetail(settingsRoot, kind) }
    function connectionStatusColor(kind) { return SettingsProfileWorkspace.connectionStatusColor(settingsRoot, kind) }
    function walletSourceStatusText() { return SettingsProfileWorkspace.walletSourceStatusText(settingsRoot) }
    function walletSourceStatusDetail() { return SettingsProfileWorkspace.walletSourceStatusDetail(settingsRoot) }
    function walletSourceStatusColor() { return SettingsProfileWorkspace.walletSourceStatusColor(settingsRoot) }
    function walletBackupHint() { return SettingsProfileWorkspace.walletBackupHint(settingsRoot) }
    function resetPendingSettingsRestoreOptions() { SettingsProfileWorkspace.resetPendingSettingsRestoreOptions(backupRestoreDialog) }
    function copyPendingSettingsRestoreOptions() { return SettingsProfileWorkspace.copyPendingSettingsRestoreOptions(backupRestoreDialog) }
    function copyNestedOptionMap(source) { return SettingsProfileWorkspace.copyNestedOptionMap(backupRestoreDialog, source) }
    function copyFlatOptionMap(source) { return SettingsProfileWorkspace.copyFlatOptionMap(backupRestoreDialog, source) }
    function setPendingImportMode(area, mode) { SettingsProfileWorkspace.setPendingImportMode(backupRestoreDialog, area, mode) }
    function pendingImportItemRows(area) { return SettingsProfileWorkspace.pendingImportItemRows(backupRestoreDialog, area) }
    function pendingImportItemSelected(area, key) { return SettingsProfileWorkspace.pendingImportItemSelected(backupRestoreDialog, area, key) }
    function setPendingImportItemSelected(area, key, selected) { SettingsProfileWorkspace.setPendingImportItemSelected(backupRestoreDialog, area, key, selected) }
    function pendingImportConflictRows() { return SettingsProfileWorkspace.pendingImportConflictRows(backupRestoreDialog) }
    function pendingImportConflictDecision(area, key) { return SettingsProfileWorkspace.pendingImportConflictDecision(backupRestoreDialog, area, key) }
    function conflictDecisionIndexFor(area, key) { return SettingsProfileWorkspace.conflictDecisionIndexFor(backupRestoreDialog, area, key, conflictDecisionOptions) }
    function setPendingImportConflictDecision(area, key, decision) { SettingsProfileWorkspace.setPendingImportConflictDecision(backupRestoreDialog, area, key, decision) }
    function pendingImportHasRequiredConflicts() { return SettingsProfileWorkspace.pendingImportHasRequiredConflicts(backupRestoreDialog) }
    function importModeIndexFor(area, optionsModel) { return SettingsProfileWorkspace.importModeIndexFor(backupRestoreDialog, area, optionsModel) }
    function importModeAt(index, optionsModel) { return SettingsProfileWorkspace.importModeAt(backupRestoreDialog, index, optionsModel) }
    function previewPendingLocalRestore() { return SettingsProfileWorkspace.previewPendingLocalRestore(backupRestoreDialog) }
    function pendingImportPlanText() { return SettingsProfileWorkspace.pendingImportPlanText(backupRestoreDialog) }
    function pendingImportConfirmEnabled() { return SettingsProfileWorkspace.pendingImportConfirmEnabled(backupRestoreDialog) }
    function pendingImportModeText() { return SettingsProfileWorkspace.pendingImportModeText(backupRestoreDialog) }
    function appendPendingImportMode(rows, label, mode) { SettingsProfileWorkspace.appendPendingImportMode(backupRestoreDialog, rows, label, mode) }
    function importModeLabel(mode) { return SettingsProfileWorkspace.importModeLabel(backupRestoreDialog, mode) }
    function pendingImportOperationText(plan) { return SettingsProfileWorkspace.pendingImportOperationText(backupRestoreDialog, plan) }
    function pendingImportWarningText(plan) { return SettingsProfileWorkspace.pendingImportWarningText(backupRestoreDialog, plan) }
    function pendingImportSelectedAreas() { return SettingsProfileWorkspace.pendingImportSelectedAreas(backupRestoreDialog) }
    function updateNodeUrl(value) { SettingsProfileWorkspace.updateEndpoint(settingsRoot, "nodeUrl", value) }
    function syncProfileFromEndpoints() { SettingsProfileWorkspace.syncProfileFromEndpoints(settingsRoot) }
    function applyProfileIndex(index) { SettingsProfileWorkspace.applyProfileIndex(settingsRoot, index) }
    function deliverySourceIndexFor(value) { return SettingsProfileWorkspace.sourceIndexFor(settingsRoot, "delivery", value, deliverySourceOptions) }
    function deliverySourceModeAt(index) { return SettingsProfileWorkspace.sourceModeAt(settingsRoot, index, deliverySourceOptions) }
    function storageSourceIndexFor(value) { return SettingsProfileWorkspace.sourceIndexFor(settingsRoot, "storage", value, storageSourceOptions) }
    function storageSourceModeAt(index) { return SettingsProfileWorkspace.sourceModeAt(settingsRoot, index, storageSourceOptions) }
    function coreSourceIndexFor(value) { return SettingsProfileWorkspace.sourceIndexFor(settingsRoot, "core", value, coreSourceOptions) }
    function coreSourceModeAt(index) { return SettingsProfileWorkspace.sourceModeAt(settingsRoot, index, coreSourceOptions) }
    function refreshSourceOptions() { SettingsProfileWorkspace.refreshSourceOptions(settingsRoot, coreSourceOptions, deliverySourceOptions, storageSourceOptions) }
    function refreshProfileOptions() { SettingsProfileWorkspace.refreshProfileOptions(settingsRoot, profileOptions) }
    function populateSourceOptions(targetModel, family) { SettingsProfileWorkspace.populateSourceOptions(settingsRoot, targetModel, family) }
    function profileIndexFor(value) { return SettingsProfileWorkspace.profileIndexFor(settingsRoot, value) }
    function inferProfile(node) { return SettingsProfileWorkspace.inferProfile(settingsRoot, node) }
    function profileLabel(value) { return SettingsProfileWorkspace.profileLabel(settingsRoot, value) }
    function profileSummary(value) { return SettingsProfileWorkspace.profileSummary(settingsRoot, value) }
    function profileDetail() { return SettingsProfileWorkspace.profileDetail(settingsRoot) }
    function normalizeEndpoint(value) { return SettingsProfileWorkspace.normalizeEndpoint(settingsRoot, value) }
    function shortEndpoint(value) { return SettingsProfileWorkspace.shortEndpoint(value) }
    function footerFieldGroups() { return SettingsProfileWorkspace.footerFieldGroups() }
    function dashboardGraphGroups() { return SettingsProfileWorkspace.dashboardGraphGroups() }

}
