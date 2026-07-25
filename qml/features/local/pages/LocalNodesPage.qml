pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts
import "../../../components"
import "../../../components/common"
import "../../../state"
import "../../../theme"
import "../../../utils/UiFormat.js" as UiFormat
import "../controls"

ColumnLayout {
    id: root

    required property Theme theme
    required property LocalNodesState model

    property string newNetworkId: ""
    property string loadWorkspace: ""
    property string runtimeModulesDir: root.model.runtimeModulesDir()
    property string runtimeBinaryPath: ""
    property var selectedIndexerPackage: root.model.defaultPackageSelection()
    property var selectedModuleRepository: root.model.defaultModuleRepositorySelection()
    property string selectedModulePackageName: ""
    property var selectedModuleRelease: ({ version: "", root_hash: "" })
    property string localModulePackagePath: ""
    property bool confirmationAccepted: false
    property int confirmationGeneration: 0
    property var pageScroller: null
    property string pendingConfigurationReveal: ""
    property bool configurationResponseReady: false
    property bool configurationLayoutReady: false

    width: parent ? parent.width : 900
    spacing: 16

    Component.onCompleted: {
        root.model.refresh(false, !root.model.basecampHost);
        if (!root.model.basecampHost) {
            root.model.refreshDevnets();
        }
    }

    Connections {
        target: root.model

        function onPackageCatalogChanged() {
            const selected = root.selectedIndexerPackage || {}
            if (!root.model.packageRelease(selected.version, selected.root_hash)) {
                root.selectedIndexerPackage = root.model.defaultPackageSelection()
            }
            root.syncIndexerPackageVersionIndex()
        }

        function onModuleCatalogChanged() {
            root.syncModuleSelections()
        }

        function onNodeConfigSnapshotChanged() {
            root.markConfigurationResponseReady()
        }

        function onNodeConfigErrorChanged() {
            root.markConfigurationResponseReady()
        }

        function onNodeConfigLoadingChanged() {
            if (root.model.nodeConfigLoading) {
                root.configurationResponseReady = false
                root.configurationLayoutReady = false
            }
        }

        function onNetworkProfileChanged() {
            root.clearConfigurationReveal()
        }
    }

    Connections {
        target: root.pageScroller

        function onContentHeightChanged() {
            root.revealNodeConfiguration()
        }
    }

    FileDialog {
        id: localModulePackageDialog

        title: qsTr("Select Logos module package")
        fileMode: FileDialog.OpenFile
        nameFilters: [qsTr("Logos module packages (*.lgx)"), qsTr("All files (*)")]
        onAccepted: {
            const path = root.localPathFromFileUrl(selectedFile)
            if (path.length > 0) {
                root.localModulePackagePath = path
            }
        }
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / System / Local Nodes")
        title: qsTr("Local Nodes")
        layerLabel: qsTr("System")
        subtitle: root.model.basecampHost
            ? qsTr("Bedrock, Delivery, and Storage modules running inside Basecamp.")
            : qsTr("Local Bedrock, Channel Indexer package, Delivery, and Storage connected to Logos Testnet.")
        Layout.fillWidth: true
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
            columns: root.width < 1060 ? 2 : 5
            columnSpacing: root.theme.gapSmall
            rowSpacing: root.theme.gapSmall

            StatusChip {
                theme: root.theme
                label: qsTr("Mode")
                value: root.model.modeLabel()
                tone: root.model.report ? "success" : "neutral"
                compact: true
                showIndicator: true
                Layout.fillWidth: true
            }

            StatusChip {
                theme: root.theme
                label: qsTr("Active Topology")
                value: root.shortText(root.activeNetworkId(), 24)
                detail: root.activeNetworkId()
                tone: root.activeNetworkId().length ? "success" : "warning"
                compact: true
                showIndicator: true
                Layout.fillWidth: true
            }

            StatusChip {
                theme: root.theme
                label: qsTr("Workspace")
                value: root.shortText(root.workspaceLabel(), 28)
                detail: root.workspaceLabel()
                tone: "neutral"
                compact: true
                Layout.fillWidth: true
            }

            StatusChip {
                theme: root.theme
                label: qsTr("Status")
                value: root.model.summaryText()
                tone: root.model.summaryTone()
                compact: true
                showIndicator: true
                Layout.fillWidth: true
            }

            StatusChip {
                theme: root.theme
                label: qsTr("Runtime")
                value: root.stateLabel(root.model.runtimeState())
                detail: root.runtimeDetail()
                tone: root.runtimeTone()
                compact: true
                showIndicator: true
                Layout.fillWidth: true
            }
        }
    }

    StatusMessage {
        visible: root.model.error.length > 0
        theme: root.theme
        tone: "error"
        title: qsTr("Local node status failed")
        message: root.model.error
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.model.error.length === 0 && root.model.toolProblem().length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Configuration required")
        message: root.model.toolProblem()
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.model.error.length === 0
            && !root.model.localMode()
            && !root.model.basecampHost
        theme: root.theme
        tone: "info"
        title: qsTr("Logos Testnet topology")
        message: qsTr("Local Bedrock feeds the UI. Each Channel Zone uses its configured Channel Indexer history with its configured Testnet Sequencer.")
        Layout.fillWidth: true
    }

    Panel {
        objectName: "localDevnetConfiguration"
        visible: !root.model.basecampHost
        theme: root.theme
        title: qsTr("Local Devnet")

        ColumnLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            RowLayout {
                visible: !root.model.localMode()
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                StatusMessage {
                    theme: root.theme
                    tone: "info"
                    title: qsTr("Local profile required")
                    message: qsTr("Activate Local node profile to configure and control a Local Devnet.")
                    Layout.fillWidth: true
                }

                ActionButton {
                    objectName: "activateLocalProfileButton"
                    theme: root.theme
                    text: qsTr("Use Local profile")
                    primary: true
                    enabled: !root.model.busy
                    Layout.preferredWidth: 176
                    onClicked: root.model.activateLocalProfile()
                }
            }

            GridLayout {
                visible: root.model.localMode()
                columns: root.width < 840 ? 1 : 4
                columnSpacing: root.theme.gapSmall
                rowSpacing: root.theme.gapSmall
                Layout.fillWidth: true

                FieldRow {
                    theme: root.theme
                    label: qsTr("Devnet ID")
                    sourceText: root.newNetworkId
                    syncSourceText: true
                    placeholderText: qsTr("devnet")
                    Layout.fillWidth: true
                    onTextEdited: text => root.newNetworkId = text
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("New")
                    primary: true
                    enabled: root.model.networkActionEnabled("new_network")
                    Layout.preferredWidth: 96
                    Layout.fillWidth: root.width < 840
                    onClicked: root.openNetworkConfirm("new_network")
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Reset")
                    enabled: root.model.networkActionEnabled("reset_network")
                    Layout.preferredWidth: 96
                    Layout.fillWidth: root.width < 840
                    onClicked: root.openNetworkConfirm("reset_network")
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Delete")
                    enabled: root.model.networkActionEnabled("delete_network")
                    Layout.preferredWidth: 96
                    Layout.fillWidth: root.width < 840
                    onClicked: root.openNetworkConfirm("delete_network")
                }
            }

            GridLayout {
                visible: root.model.localMode()
                columns: root.width < 840 ? 1 : 3
                columnSpacing: root.theme.gapSmall
                rowSpacing: root.theme.gapSmall
                Layout.fillWidth: true

                FieldRow {
                    theme: root.theme
                    label: qsTr("Workspace")
                    sourceText: root.loadWorkspace
                    syncSourceText: true
                    placeholderText: qsTr("/path/to/local-network")
                    Layout.columnSpan: root.width < 840 ? 1 : 2
                    Layout.fillWidth: true
                    onTextEdited: text => root.loadWorkspace = text
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Load")
                    enabled: root.model.networkActionEnabled("load_network") && root.loadWorkspace.trim().length > 0
                    Layout.preferredWidth: 96
                    Layout.fillWidth: root.width < 840
                    onClicked: root.openNetworkConfirm("load_network")
                }
            }
        }
    }

    Panel {
        objectName: "logoscoreRuntimeConfiguration"
        visible: !root.model.basecampHost
        theme: root.theme
        title: qsTr("LogosCore Runtime")

        GridLayout {
            columns: root.width < 840 ? 1 : 4
            columnSpacing: root.theme.gapSmall
            rowSpacing: root.theme.gapSmall
            Layout.fillWidth: true

            FieldRow {
                objectName: "runtimeModulesDirectory"
                theme: root.theme
                label: qsTr("Modules directory")
                sourceText: root.runtimeModulesDir
                syncSourceText: true
                placeholderText: qsTr("/path/to/modules")
                Layout.columnSpan: root.width < 840 ? 1 : 2
                Layout.fillWidth: true
                onTextEdited: text => root.runtimeModulesDir = text
            }

            FieldRow {
                theme: root.theme
                label: qsTr("Binary path")
                sourceText: root.runtimeBinaryPath.length ? root.runtimeBinaryPath : root.configuredRuntimeBinaryPath()
                syncSourceText: true
                placeholderText: qsTr("logoscore on PATH")
                Layout.fillWidth: true
                onTextEdited: text => root.runtimeBinaryPath = text
            }

            RowLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    objectName: "runtimeStartButton"
                    theme: root.theme
                    text: root.model.localAttachedRuntime() ? qsTr("Start service") : qsTr("Start")
                    accessibleName: root.model.localAttachedRuntime()
                        ? qsTr("Start local service") : qsTr("Start Local Runtime")
                    primary: true
                    enabled: root.model.runtimeActionEnabled("start_runtime")
                    Layout.minimumWidth: implicitWidth
                    Layout.preferredWidth: Math.max(96, implicitWidth)
                    onClicked: root.openRuntimeConfirm("start_runtime")
                }

                ActionButton {
                    objectName: "runtimeStopButton"
                    theme: root.theme
                    text: root.model.localAttachedRuntime() ? qsTr("Stop service") : qsTr("Stop")
                    accessibleName: root.model.localAttachedRuntime()
                        ? qsTr("Stop local service") : qsTr("Stop Local Runtime")
                    enabled: root.model.runtimeActionEnabled("stop_runtime")
                    Layout.minimumWidth: implicitWidth
                    Layout.preferredWidth: Math.max(96, implicitWidth)
                    onClicked: root.openRuntimeConfirm("stop_runtime")
                }
            }
        }
    }

    Panel {
        objectName: "modulePackageConfiguration"
        visible: !root.model.basecampHost
        theme: root.theme
        title: qsTr("Core module packages")

        ColumnLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            StatusMessage {
                objectName: "modulePackageStatus"
                theme: root.theme
                tone: root.moduleCatalogTone()
                title: root.moduleCatalogTitle()
                message: root.moduleCatalogMessage()
                Layout.fillWidth: true
            }

            GridLayout {
                columns: root.width < 840 ? 1 : 3
                columnSpacing: root.theme.gapSmall
                rowSpacing: root.theme.gapSmall
                Layout.fillWidth: true

                ColumnLayout {
                    spacing: 6
                    Layout.fillWidth: true

                    Text {
                        text: qsTr("Repository")
                        color: root.theme.textMuted
                        textFormat: Text.PlainText
                        font.pixelSize: root.theme.secondaryText
                        font.weight: Font.Medium
                        Layout.fillWidth: true
                    }

                    ModulePackageSelector {
                        id: moduleRepositorySelector

                        objectName: "moduleRepositorySelector"
                        options: root.moduleRepositoryOptions()
                        emptyText: qsTr("No repositories")
                        accessibleLabel: qsTr("Module package repository")
                        enabled: !root.model.moduleCatalogLoading && count > 0 && !root.model.busy
                        Layout.fillWidth: true
                        onOptionSelected: option => root.selectModuleRepository(option)
                    }
                }

                ColumnLayout {
                    spacing: 6
                    Layout.fillWidth: true

                    Text {
                        text: qsTr("Core module")
                        color: root.theme.textMuted
                        textFormat: Text.PlainText
                        font.pixelSize: root.theme.secondaryText
                        font.weight: Font.Medium
                        Layout.fillWidth: true
                    }

                    ModulePackageSelector {
                        id: modulePackageSelector

                        objectName: "modulePackageSelector"
                        options: root.modulePackageOptions()
                        emptyText: qsTr("No core modules")
                        accessibleLabel: qsTr("Core module package")
                        enabled: !root.model.moduleCatalogLoading && count > 0 && !root.model.busy
                        Layout.fillWidth: true
                        onOptionSelected: option => root.selectModulePackage(option)
                    }
                }

                ColumnLayout {
                    spacing: 6
                    Layout.fillWidth: true

                    Text {
                        text: qsTr("Exact release")
                        color: root.theme.textMuted
                        textFormat: Text.PlainText
                        font.pixelSize: root.theme.secondaryText
                        font.weight: Font.Medium
                        Layout.fillWidth: true
                    }

                    ModulePackageSelector {
                        id: moduleReleaseSelector

                        objectName: "modulePackageReleaseSelector"
                        options: root.moduleReleaseOptions()
                        emptyText: qsTr("No releases")
                        accessibleLabel: qsTr("Core module exact release")
                        enabled: !root.model.moduleCatalogLoading && count > 0 && !root.model.busy
                        monospace: true
                        Layout.fillWidth: true
                        onOptionSelected: option => root.selectModuleRelease(option)
                    }
                }
            }

            RowLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    objectName: "modulePackageReloadButton"
                    theme: root.theme
                    text: qsTr("Reload catalog")
                    accessibleName: qsTr("Reload module package catalog")
                    enabled: !root.model.moduleCatalogLoading && !root.model.busy
                    Layout.preferredWidth: 144
                    onClicked: root.reloadModuleCatalog()
                }

                ActionButton {
                    objectName: "modulePackageInstallButton"
                    theme: root.theme
                    text: qsTr("Install release")
                    accessibleName: qsTr("Install selected core module release")
                    primary: true
                    enabled: root.moduleRepositoryInstallReady()
                    Layout.preferredWidth: 132
                    onClicked: root.openModuleRepositoryConfirm()
                }

                Text {
                    text: root.selectedModuleReleaseDetail()
                    color: root.theme.textDim
                    textFormat: Text.PlainText
                    wrapMode: Text.WrapAnywhere
                    font.pixelSize: root.theme.dataText
                    Layout.fillWidth: true
                    Accessible.role: Accessible.StaticText
                    Accessible.name: text
                }
            }

            Rectangle {
                color: root.theme.outlineMuted
                Layout.preferredHeight: 1
                Layout.fillWidth: true
            }

            GridLayout {
                columns: root.width < 840 ? 1 : 3
                columnSpacing: root.theme.gapSmall
                rowSpacing: root.theme.gapSmall
                Layout.fillWidth: true

                FieldRow {
                    objectName: "modulePackageFilePath"
                    theme: root.theme
                    label: qsTr("Local .lgx package")
                    sourceText: root.localModulePackagePath
                    syncSourceText: true
                    placeholderText: qsTr("/path/to/module.lgx")
                    Layout.columnSpan: root.width < 840 ? 1 : 2
                    Layout.fillWidth: true
                    onTextEdited: text => root.localModulePackagePath = text
                }

                RowLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    ActionButton {
                        objectName: "modulePackageBrowseButton"
                        theme: root.theme
                        text: qsTr("Browse")
                        enabled: !root.model.busy
                        Layout.preferredWidth: 96
                        onClicked: localModulePackageDialog.open()
                    }

                    ActionButton {
                        objectName: "modulePackageInstallFileButton"
                        theme: root.theme
                        text: qsTr("Install local")
                        primary: true
                        enabled: root.moduleFileInstallReady()
                        Layout.preferredWidth: 124
                        onClicked: root.openModuleFileConfirm()
                    }
                }
            }

            Text {
                text: root.moduleTargetDetail()
                color: root.theme.textDim
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }

            DataTableFrame {
                objectName: "installedModulePackages"
                theme: root.theme
                headerCells: [
                    { text: qsTr("Installed core module"), width: 190 },
                    { text: qsTr("Version"), width: 120 },
                    { text: qsTr("Category"), width: 130 },
                    { text: qsTr("Root hash"), width: 180 },
                    { text: qsTr("Location"), width: 260, fill: true }
                ]
                rows: root.installedModuleRows()
                Layout.fillWidth: true
            }
        }
    }

    Panel {
        objectName: "indexerPackageConfiguration"
        visible: !root.model.basecampHost
        theme: root.theme
        title: qsTr("Indexer package")

        StatusMessage {
            objectName: "indexerPackageStatus"
            theme: root.theme
            tone: root.packageStatusTone()
            title: root.packageStatusTitle()
            message: root.packageStatusMessage()
            Layout.fillWidth: true
        }

        GridLayout {
            columns: root.width < 840 ? 1 : 4
            columnSpacing: root.theme.gapSmall
            rowSpacing: root.theme.gapSmall
            Layout.fillWidth: true

            ColumnLayout {
                spacing: 6
                Layout.columnSpan: root.width < 840 ? 1 : 2
                Layout.fillWidth: true

                Text {
                    text: qsTr("Exact release")
                    color: root.theme.textMuted
                    textFormat: Text.PlainText
                    font.pixelSize: root.theme.secondaryText
                    font.weight: Font.Medium
                    Layout.fillWidth: true
                }

                ComboBox {
                    id: indexerPackageVersion

                    objectName: "indexerPackageVersionSelector"
                    model: root.packageReleaseOptions()
                    textRole: "label"
                    currentIndex: -1
                    displayText: currentIndex >= 0
                        ? String(model[currentIndex].label || "") : qsTr("No releases")
                    hoverEnabled: true
                    enabled: !root.model.packageCatalogLoading && count > 0 && !root.model.busy
                    Layout.fillWidth: true
                    Layout.preferredHeight: root.theme.controlHeight
                    onModelChanged: root.syncIndexerPackageVersionIndex()
                    onActivated: index => root.selectIndexerPackage(model[index])

                    delegate: ItemDelegate {
                        id: versionDelegate

                        required property int index
                        required property var modelData

                        width: indexerPackageVersion.width
                        text: String(modelData && modelData.label || "")
                        hoverEnabled: true
                        highlighted: indexerPackageVersion.highlightedIndex === index

                        contentItem: Text {
                            text: versionDelegate.text
                            color: versionDelegate.highlighted ? root.theme.selectedText : root.theme.text
                            textFormat: Text.PlainText
                            verticalAlignment: Text.AlignVCenter
                            font.family: "monospace"
                            font.pixelSize: root.theme.secondaryText
                        }

                        background: Rectangle {
                            color: versionDelegate.highlighted
                                ? root.theme.accent
                                : (versionDelegate.hovered ? root.theme.hover : root.theme.surfaceRaised)
                        }
                    }

                    contentItem: Text {
                        text: indexerPackageVersion.displayText
                        color: indexerPackageVersion.enabled ? root.theme.text : root.theme.textDim
                        textFormat: Text.PlainText
                        verticalAlignment: Text.AlignVCenter
                        leftPadding: 12
                        rightPadding: 36
                        font.family: "monospace"
                        font.pixelSize: root.theme.primaryText
                        font.weight: Font.Medium
                    }

                    indicator: Text {
                        x: indexerPackageVersion.width - width - 14
                        y: (indexerPackageVersion.height - height) / 2
                        text: "\u25be"
                        color: indexerPackageVersion.enabled ? root.theme.textMuted : root.theme.textDim
                        textFormat: Text.PlainText
                        font.pixelSize: root.theme.secondaryText
                    }

                    background: Rectangle {
                        radius: root.theme.radius
                        color: indexerPackageVersion.hovered || indexerPackageVersion.activeFocus
                            ? root.theme.surfaceRaised : root.theme.field
                        border.width: indexerPackageVersion.activeFocus ? 2 : 1
                        border.color: indexerPackageVersion.activeFocus
                            ? root.theme.accent : root.theme.outlineMuted
                    }

                    popup: Popup {
                        y: indexerPackageVersion.height + root.theme.gapTiny
                        width: indexerPackageVersion.width
                        implicitHeight: Math.min(contentItem.implicitHeight + 2, 260)
                        padding: 1

                        contentItem: ListView {
                            clip: true
                            implicitHeight: contentHeight
                            model: indexerPackageVersion.popup.visible
                                ? indexerPackageVersion.delegateModel : null
                            currentIndex: indexerPackageVersion.highlightedIndex
                        }

                        background: Rectangle {
                            radius: root.theme.radius
                            color: root.theme.surfaceRaised
                            border.width: 1
                            border.color: root.theme.outline
                        }
                    }

                    Accessible.role: Accessible.ComboBox
                    Accessible.name: qsTr("Indexer package exact release")
                    Accessible.description: root.selectedPackageReleaseDetail()
                }
            }

            ActionButton {
                objectName: "indexerPackageReloadButton"
                theme: root.theme
                text: qsTr("Reload releases")
                accessibleName: qsTr("Reload official Indexer releases")
                enabled: !root.model.packageCatalogLoading && !root.model.busy
                Layout.preferredWidth: 144
                Layout.fillWidth: root.width < 840
                onClicked: root.reloadPackageCatalog()
            }

            ActionButton {
                objectName: "indexerPackageInstallButton"
                theme: root.theme
                text: qsTr("Install release")
                accessibleName: qsTr("Install selected Indexer release")
                primary: true
                enabled: root.packageInstallReady()
                Layout.preferredWidth: 132
                Layout.fillWidth: root.width < 840
                onClicked: root.openIndexerPackageConfirm()
            }

            Text {
                text: root.selectedPackageReleaseDetail()
                color: root.theme.textDim
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
                font.pixelSize: root.theme.dataText
                Layout.columnSpan: root.width < 840 ? 1 : 4
                Layout.fillWidth: true
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }
        }
    }

    Panel {
        theme: root.theme
        title: root.model.basecampHost
            ? qsTr("Basecamp Module Status")
            : qsTr("System and Channel Status")

        DataTableFrame {
            theme: root.theme
            headerCells: [
                {
                    text: qsTr("Node"),
                    width: 150
                },
                {
                    text: root.model.localMode() ? qsTr("Install") : qsTr("Control"),
                    width: 130
                },
                {
                    text: root.model.localMode() ? qsTr("Run") : qsTr("Status"),
                    width: 110
                },
                {
                    text: qsTr("Endpoint"),
                    width: 230,
                    fill: true
                },
                {
                    text: qsTr("Data"),
                    width: 190
                },
                {
                    text: qsTr("Last"),
                    width: 180
                }
            ]
            rows: root.nodeTableRows()
            Layout.fillWidth: true
        }
    }

    Panel {
        theme: root.theme
        title: qsTr("Actions")

        ColumnLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Repeater {
                model: root.actionRows()

                RowLayout {
                    id: actionRow

                    required property var modelData

                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    Text {
                        text: actionRow.modelData.label
                        color: root.theme.text
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        font.pixelSize: root.theme.secondaryText
                        font.weight: Font.DemiBold
                        Layout.preferredWidth: 150
                    }

                    ActionButton {
                        theme: root.theme
                        visible: actionRow.modelData.setupAction.length > 0
                            && actionRow.modelData.key !== "indexer"
                        text: root.model.actionLabel(actionRow.modelData.setupAction)
                        enabled: root.model.actionEnabled(actionRow.modelData.key, actionRow.modelData.setupAction)
                        accessibleName: qsTr("%1 %2")
                            .arg(root.model.actionLabel(actionRow.modelData.setupAction))
                            .arg(actionRow.modelData.label)
                        Layout.preferredWidth: 92
                        onClicked: root.openNodeConfirm(actionRow.modelData.setupAction, actionRow.modelData.key)
                    }

                    ActionButton {
                        objectName: "nodeConfigure" + actionRow.modelData.key
                        theme: root.theme
                        text: qsTr("Configure")
                        enabled: root.model.configurationActionEnabled(actionRow.modelData.key)
                        accessibleName: qsTr("Configure %1").arg(actionRow.modelData.label)
                        Layout.preferredWidth: 108
                        onClicked: root.openNodeConfiguration(actionRow.modelData.key)
                    }

                    ActionButton {
                        theme: root.theme
                        visible: actionRow.modelData.key !== "indexer"
                            && root.model.actionAvailable(actionRow.modelData.key, "start")
                        text: qsTr("Start")
                        primary: true
                        enabled: root.model.actionEnabled(actionRow.modelData.key, "start")
                        accessibleName: qsTr("Start %1").arg(actionRow.modelData.label)
                        Layout.preferredWidth: 84
                        onClicked: root.openNodeConfirm("start", actionRow.modelData.key)
                    }

                    ActionButton {
                        theme: root.theme
                        visible: actionRow.modelData.key !== "indexer"
                            && root.model.actionAvailable(actionRow.modelData.key, "stop")
                        text: qsTr("Stop")
                        enabled: root.model.actionEnabled(actionRow.modelData.key, "stop")
                        accessibleName: qsTr("Stop %1").arg(actionRow.modelData.label)
                        Layout.preferredWidth: 84
                        onClicked: root.openNodeConfirm("stop", actionRow.modelData.key)
                    }

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Purge")
                        enabled: root.model.actionEnabled(actionRow.modelData.key, "purge")
                        Layout.preferredWidth: 84
                        onClicked: root.openNodeConfirm("purge", actionRow.modelData.key)
                    }

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Uninstall")
                        enabled: root.model.actionEnabled(actionRow.modelData.key, "uninstall")
                        Layout.preferredWidth: 112
                        onClicked: root.openNodeConfirm("uninstall", actionRow.modelData.key)
                    }

                    Item {
                        Layout.fillWidth: true
                    }
                }
            }
        }
    }

    NodeConfigurationPanel {
        id: nodeConfigurationPanel

        theme: root.theme
        model: root.model
        Layout.fillWidth: true
        onHeightChanged: root.noteConfigurationLayout()
        onImplicitHeightChanged: root.noteConfigurationLayout()
    }

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

    ConfirmActionPopup {
        id: confirmPopup

        objectName: "localNodesConfirmPopup"
        theme: root.theme
        title: root.confirmTitle()
        message: root.confirmMessage()
        confirmText: root.confirmActionText()
        confirmEnabled: !root.model.busy && root.model.pendingOperation.length > 0
        onAccepted: {
            root.confirmationAccepted = true
            root.acceptPendingAction()
        }
        onClosed: {
            const generation = root.confirmationGeneration
            Qt.callLater(function () {
                if (generation !== root.confirmationGeneration) {
                    return
                }
                if (!root.confirmationAccepted) {
                    root.model.clearActionDraft()
                }
                root.confirmationAccepted = false
            })
        }
    }

    function activeNetworkId() {
        const report = root.model.report || null;
        return String(report && report.active_devnet ? report.active_devnet : "");
    }

    function workspaceLabel() {
        const report = root.model.report || null;
        return String(report && report.workspace_root ? report.workspace_root : "");
    }

    function runtimeDetail() {
        const runtime = root.model.runtimeInfo();
        return String(runtime && runtime.detail ? runtime.detail : "");
    }

    function configuredRuntimeBinaryPath() {
        const runtime = root.model.runtimeInfo()
        return String(runtime && runtime.binary_path ? runtime.binary_path : "")
    }

    function runtimeTone() {
        const state = root.model.runtimeState();
        if (state === "running") {
            return "success";
        }
        if (state === "starting" || state === "stopping") {
            return "warning";
        }
        return "neutral";
    }

    function packageReleaseOptions() {
        return root.model.packageReleases().map(function (release) {
            const version = String(release && release.version || "")
            const rootHash = String(release && release.root_hash || "")
            return {
                version: version,
                root_hash: rootHash,
                released_at: String(release && release.released_at || ""),
                label: root.packageReleaseLabel(release)
            }
        }).filter(function (option) {
            return option.version.length > 0 && option.root_hash.length > 0
        })
    }

    function packageReleaseIndex(selection) {
        const selected = selection || {}
        const selectedVersion = String(selected.version || "")
        const selectedRootHash = String(selected.root_hash || "")
        const options = root.packageReleaseOptions()
        for (let i = 0; i < options.length; ++i) {
            if (options[i].version === selectedVersion
                    && options[i].root_hash === selectedRootHash) {
                return i
            }
        }
        return -1
    }

    function packageReleaseLabel(release) {
        const version = String(release && release.version || qsTr("unknown version"))
        const rootHash = root.shortPackageRootHash(release && release.root_hash)
        const releasedAt = String(release && release.released_at || "")
        const releaseDate = releasedAt.length >= 10
            ? releasedAt.slice(0, 10) : qsTr("date unavailable")
        return qsTr("%1 · %2 · %3").arg(version).arg(rootHash).arg(releaseDate)
    }

    function shortPackageRootHash(value) {
        const rootHash = String(value || "")
        if (!rootHash.length) {
            return qsTr("hash unavailable")
        }
        if (rootHash.length <= 14) {
            return rootHash
        }
        return qsTr("%1…%2").arg(rootHash.slice(0, 6)).arg(rootHash.slice(-6))
    }

    function selectIndexerPackage(option) {
        const candidate = option || {}
        const release = root.model.packageRelease(candidate.version, candidate.root_hash)
        if (!release) {
            return
        }
        root.selectedIndexerPackage = {
            version: String(release.version || ""),
            root_hash: String(release.root_hash || "")
        }
        root.syncIndexerPackageVersionIndex()
    }

    function syncIndexerPackageVersionIndex() {
        const selectedIndex = root.packageReleaseIndex(root.selectedIndexerPackage)
        if (indexerPackageVersion.currentIndex !== selectedIndex) {
            indexerPackageVersion.currentIndex = selectedIndex
        }
    }

    function selectedPackageRelease() {
        const selected = root.selectedIndexerPackage || {}
        return root.model.packageRelease(selected.version, selected.root_hash)
    }

    function selectedPackageReleaseDetail() {
        const release = root.selectedPackageRelease()
        if (!release) {
            return root.model.packageCatalogLoading
                ? qsTr("Loading exact releases…") : qsTr("No exact release selected.")
        }
        const releasedAt = String(release.released_at || qsTr("date unavailable"))
        const rootHash = String(release.root_hash || qsTr("root hash unavailable"))
        return qsTr("Released %1. Root hash %2.").arg(releasedAt).arg(rootHash)
    }

    function packageInstallReady() {
        const release = root.selectedPackageRelease()
        return !root.model.packageCatalogLoading
            && root.packageInstallRuntimeReady()
            && root.indexerPackageTargetProblem().length === 0
            && release !== null
            && String(release.version || "").length > 0
            && String(release.root_hash || "").length > 0
            && root.model.actionEnabled("indexer", "install")
    }

    function packageInstallRuntimeReady() {
        const state = root.model.runtimeState()
        return state !== "running" && state !== "starting" && state !== "stopping"
    }

    function configuredRuntimeModulesDir() {
        const runtime = root.model.runtimeInfo()
        return String(runtime && runtime.modules_dir || "").trim()
    }

    function runtimeModulesTargetProblem() {
        const target = root.runtimeModulesDir.trim()
        if (!target.length) {
            return qsTr("Modules directory is required.")
        }
        const configured = root.configuredRuntimeModulesDir()
        if (configured.length && target !== configured) {
            return qsTr("The configured LogosCore Runtime uses %1. Reconfigure the runtime before installing packages into %2.")
                .arg(configured)
                .arg(target)
        }
        return ""
    }

    function packageCatalogTargetProblem(catalogModulesDir, catalogLabel) {
        const runtimeProblem = root.runtimeModulesTargetProblem()
        if (runtimeProblem.length) {
            return runtimeProblem
        }
        const catalogTarget = String(catalogModulesDir || "").trim()
        const target = root.runtimeModulesDir.trim()
        if (catalogTarget.length && catalogTarget !== target) {
            return qsTr("Reload %1 for %2 before installing.")
                .arg(catalogLabel)
                .arg(target)
        }
        return ""
    }

    function indexerPackageTargetProblem() {
        return root.packageCatalogTargetProblem(
            root.model.packageCatalogModulesDir(),
            qsTr("the Indexer package catalog"))
    }

    function modulePackageTargetProblem() {
        return root.packageCatalogTargetProblem(
            root.model.moduleCatalogModulesDir(),
            qsTr("the module package catalog"))
    }

    function packageStatusTone() {
        if (root.model.packageCatalogError.length > 0) {
            return "error"
        }
        if (root.indexerPackageTargetProblem().length > 0) {
            return "warning"
        }
        if (root.model.packageCatalogLoading) {
            return "info"
        }
        if (root.model.installedPackage()) {
            return "success"
        }
        return root.model.packageReleases().length > 0
            && root.packageInstallRuntimeReady() ? "info" : "warning"
    }

    function packageStatusTitle() {
        if (root.model.packageCatalogLoading) {
            return qsTr("Loading official Indexer releases")
        }
        if (root.model.packageCatalogError.length > 0) {
            return qsTr("Indexer package catalog unavailable")
        }
        if (root.indexerPackageTargetProblem().length > 0) {
            return qsTr("Indexer package target needs attention")
        }
        const installed = root.model.installedPackage()
        if (installed) {
            return qsTr("%1 installed").arg(root.model.packageName())
        }
        return qsTr("Official Indexer package")
    }

    function packageStatusMessage() {
        if (root.model.packageCatalogLoading) {
            return qsTr("Querying exact releases for %1.").arg(root.runtimeModulesDir)
        }
        if (root.model.packageCatalogError.length > 0) {
            return root.model.packageCatalogError
        }
        const targetProblem = root.indexerPackageTargetProblem()
        if (targetProblem.length > 0) {
            return targetProblem
        }
        const installed = root.model.installedPackage()
        if (installed) {
            return qsTr("Version %1 is installed in %2. Stop LogosCore Runtime before changing it. Channel Indexer start and stop are in Zone Sources.")
                .arg(String(installed.version || qsTr("unknown")))
                .arg(root.model.packageCatalogModulesDir())
        }
        return qsTr("Select an exact official lez_indexer_module release. Install downloads, verifies, and installs it into %1 while LogosCore Runtime is stopped. Start the runtime to load the package; Channel Indexer start and stop are in Zone Sources.")
            .arg(root.runtimeModulesDir)
    }

    function reloadPackageCatalog() {
        root.model.refreshPackageCatalog(root.runtimeModulesDir.trim())
    }

    function openIndexerPackageConfirm() {
        const release = root.selectedPackageRelease()
        if (!release) {
            return
        }
        root.model.beginNodeAction(
            "install",
            "indexer",
            String(release.version || ""),
            String(release.root_hash || ""),
            root.runtimeModulesDir.trim())
        root.showConfirmation()
    }

    function moduleRepositoryOptions() {
        return root.model.moduleRepositories().map(function (repositoryValue) {
            const repository = repositoryValue || {}
            const name = String(repository.name || "")
            const url = String(repository.url || "")
            const displayName = String(repository.display_name || name)
            return {
                name: name,
                url: url,
                label: displayName + " · " + name
            }
        }).filter(function (option) {
            return option.name.length > 0 && option.url.length > 0
        })
    }

    function modulePackageOptions() {
        const repository = root.selectedModuleRepository || {}
        return root.model.modulePackages(repository.name, repository.url).map(function (packageValue) {
            const package = packageValue || {}
            const name = String(package.name || "")
            const category = String(package.category || qsTr("uncategorized"))
            return {
                name: name,
                label: name + " · " + category
            }
        }).filter(function (option) {
            return option.name.length > 0
        })
    }

    function moduleReleaseOptions() {
        const repository = root.selectedModuleRepository || {}
        return root.model.moduleReleases(
            repository.name,
            repository.url,
            root.selectedModulePackageName).map(function (release) {
            const value = release || {}
            return {
                version: String(value.version || ""),
                root_hash: String(value.root_hash || ""),
                label: root.packageReleaseLabel(value)
            }
        }).filter(function (option) {
            return option.version.length > 0 && option.root_hash.length > 0
        })
    }

    function moduleRepositoryIndex(selection) {
        const selected = selection || {}
        const name = String(selected.name || "")
        const url = String(selected.url || "")
        const options = root.moduleRepositoryOptions()
        for (let index = 0; index < options.length; ++index) {
            if (options[index].name === name && options[index].url === url) {
                return index
            }
        }
        return -1
    }

    function modulePackageIndex(name) {
        const selectedName = String(name || "")
        const options = root.modulePackageOptions()
        for (let index = 0; index < options.length; ++index) {
            if (options[index].name === selectedName) {
                return index
            }
        }
        return -1
    }

    function moduleReleaseIndex(selection) {
        const selected = selection || {}
        const version = String(selected.version || "")
        const rootHash = String(selected.root_hash || "")
        const options = root.moduleReleaseOptions()
        for (let index = 0; index < options.length; ++index) {
            if (options[index].version === version && options[index].root_hash === rootHash) {
                return index
            }
        }
        return -1
    }

    function syncModuleSelections() {
        const repositoryOptions = root.moduleRepositoryOptions()
        let repositoryIndex = root.moduleRepositoryIndex(root.selectedModuleRepository)
        if (repositoryIndex < 0 && repositoryOptions.length > 0) {
            root.selectedModuleRepository = {
                name: repositoryOptions[0].name,
                url: repositoryOptions[0].url
            }
            repositoryIndex = 0
        } else if (repositoryIndex < 0) {
            root.selectedModuleRepository = { name: "", url: "" }
        }
        if (moduleRepositorySelector.currentIndex !== repositoryIndex) {
            moduleRepositorySelector.currentIndex = repositoryIndex
        }

        const packageOptions = root.modulePackageOptions()
        let packageIndex = root.modulePackageIndex(root.selectedModulePackageName)
        if (packageIndex < 0 && packageOptions.length > 0) {
            root.selectedModulePackageName = packageOptions[0].name
            packageIndex = 0
        } else if (packageIndex < 0) {
            root.selectedModulePackageName = ""
        }
        if (modulePackageSelector.currentIndex !== packageIndex) {
            modulePackageSelector.currentIndex = packageIndex
        }

        const releaseOptions = root.moduleReleaseOptions()
        let releaseIndex = root.moduleReleaseIndex(root.selectedModuleRelease)
        if (releaseIndex < 0 && releaseOptions.length > 0) {
            root.selectedModuleRelease = {
                version: releaseOptions[0].version,
                root_hash: releaseOptions[0].root_hash
            }
            releaseIndex = 0
        } else if (releaseIndex < 0) {
            root.selectedModuleRelease = ({ version: "", root_hash: "" })
        }
        if (moduleReleaseSelector.currentIndex !== releaseIndex) {
            moduleReleaseSelector.currentIndex = releaseIndex
        }
    }

    function selectModuleRepository(option) {
        const value = option || {}
        const name = String(value.name || "")
        const url = String(value.url || "")
        if (!name.length || !url.length) {
            return
        }
        root.selectedModuleRepository = { name: name, url: url }
        root.selectedModulePackageName = ""
        root.selectedModuleRelease = ({ version: "", root_hash: "" })
        root.syncModuleSelections()
    }

    function selectModulePackage(option) {
        const name = String(option && option.name || "")
        if (!name.length) {
            return
        }
        root.selectedModulePackageName = name
        root.selectedModuleRelease = ({ version: "", root_hash: "" })
        root.syncModuleSelections()
    }

    function selectModuleRelease(option) {
        const value = option || {}
        const repository = root.selectedModuleRepository || {}
        const release = root.model.moduleRelease(
            repository.name,
            repository.url,
            root.selectedModulePackageName,
            value.version,
            value.root_hash)
        if (!release) {
            return
        }
        root.selectedModuleRelease = {
            version: String(release.version || ""),
            root_hash: String(release.root_hash || "")
        }
        root.syncModuleSelections()
    }

    function selectedModuleReleaseValue() {
        const repository = root.selectedModuleRepository || {}
        const selected = root.selectedModuleRelease || {}
        return root.model.moduleRelease(
            repository.name,
            repository.url,
            root.selectedModulePackageName,
            selected.version,
            selected.root_hash)
    }

    function selectedModuleReleaseDetail() {
        const release = root.selectedModuleReleaseValue()
        if (!release) {
            return root.model.moduleCatalogLoading
                ? qsTr("Loading exact releases…") : qsTr("No exact release selected.")
        }
        return qsTr("Released %1. Root hash %2.")
            .arg(String(release.released_at || qsTr("date unavailable")))
            .arg(String(release.root_hash || qsTr("root hash unavailable")))
    }

    function moduleCatalogTone() {
        if (root.model.moduleCatalogError.length > 0) {
            return "error"
        }
        if (root.modulePackageTargetProblem().length > 0) {
            return "warning"
        }
        if (root.model.moduleCatalogLoading) {
            return "info"
        }
        const repository = root.selectedModuleRepository || {}
        return root.model.modulePackages(
            repository.name,
            repository.url).length > 0 ? "info" : "warning"
    }

    function moduleCatalogTitle() {
        if (root.model.moduleCatalogLoading) {
            return qsTr("Loading configured module repositories")
        }
        if (root.model.moduleCatalogError.length > 0) {
            return qsTr("Module package catalog unavailable")
        }
        if (root.modulePackageTargetProblem().length > 0) {
            return qsTr("Module package target needs attention")
        }
        return qsTr("Configured core module packages")
    }

    function moduleCatalogMessage() {
        if (root.model.moduleCatalogLoading) {
            return qsTr("Querying configured repositories and installed core modules for %1.")
                .arg(root.runtimeModulesDir)
        }
        if (root.model.moduleCatalogError.length > 0) {
            return root.model.moduleCatalogError
        }
        const targetProblem = root.modulePackageTargetProblem()
        if (targetProblem.length > 0) {
            return targetProblem
        }
        const message = qsTr("Select a configured repository, core module, and exact release. Channel Indexer uses its dedicated package panel below. UI plugins belong to their UI host and are not installed into LogosCore's core-module directory.")
        const warnings = root.model.moduleCatalogWarnings()
        return warnings.length ? message + " " + warnings.join(" ") : message
    }

    function moduleRepositoryInstallReady() {
        const repository = root.selectedModuleRepository || {}
        const release = root.selectedModuleReleaseValue()
        return !root.model.moduleCatalogLoading
            && root.packageInstallRuntimeReady()
            && !root.model.busy
            && root.modulePackageTargetProblem().length === 0
            && String(repository.name || "").length > 0
            && String(repository.url || "").length > 0
            && root.selectedModulePackageName.length > 0
            && release !== null
            && String(release.version || "").length > 0
            && String(release.root_hash || "").length > 0
    }

    function moduleFileInstallReady() {
        return root.packageInstallRuntimeReady()
            && !root.model.busy
            && root.localModulePackagePath.trim().length > 0
            && root.runtimeModulesTargetProblem().length === 0
    }

    function moduleTargetDetail() {
        return qsTr("Target core-module directory: %1. Stop LogosCore Runtime before installation, then start it to load newly installed core modules.")
            .arg(root.runtimeModulesDir)
    }

    function reloadModuleCatalog() {
        root.model.refreshModuleCatalog(root.runtimeModulesDir.trim())
    }

    function openModuleRepositoryConfirm() {
        const repository = root.selectedModuleRepository || {}
        const release = root.selectedModuleReleaseValue()
        if (!release) {
            return
        }
        root.model.beginModuleRepositoryInstall(
            String(repository.name || ""),
            String(repository.url || ""),
            root.selectedModulePackageName,
            String(release.version || ""),
            String(release.root_hash || ""),
            root.runtimeModulesDir.trim())
        root.showConfirmation()
    }

    function openModuleFileConfirm() {
        const path = root.localModulePackagePath.trim()
        if (!path.length) {
            return
        }
        root.model.beginModuleFileInstall(path, root.runtimeModulesDir.trim())
        root.showConfirmation()
    }

    function installedModuleRows() {
        const modules = root.model.installedModules()
        if (!modules.length) {
            return [{
                cells: [{ text: qsTr("No installed core modules"), width: 190, monospace: false },
                    { text: "-", width: 120 }, { text: "-", width: 130 },
                    { text: "-", width: 180 }, { text: "-", width: 260, fill: true }]
            }]
        }
        return modules.map(function (moduleValue) {
            const module = moduleValue || {}
            const rootHash = String(module.root_hash || "")
            const location = String(module.install_dir || "")
            return {
                cells: [
                    { text: String(module.name || "-"), width: 190, monospace: false },
                    { text: String(module.version || "-"), width: 120 },
                    { text: String(module.category || "-"), width: 130, monospace: false },
                    { text: root.shortPackageRootHash(rootHash), width: 180, copyText: rootHash },
                    { text: root.shortText(location, 46), width: 260, fill: true, copyText: location }
                ]
            }
        })
    }

    function localPathFromFileUrl(fileUrl) {
        const text = String(fileUrl || "")
        if (!text.length) {
            return ""
        }
        if (text.indexOf("file://") === 0) {
            let path = decodeURIComponent(text.slice(7))
            if (/^\/[A-Za-z]:\//.test(path)) {
                path = path.slice(1)
            }
            return path
        }
        return text
    }

    function nodeTableRows() {
        const report = root.model.report || null;
        const nodes = report && Array.isArray(report.nodes) ? report.nodes : [];
        if (!nodes.length) {
            return [
                {
                    cells: [
                        {
                            text: qsTr("No node status loaded"),
                            width: 150,
                            monospace: false
                        },
                        {
                            text: "-",
                            width: 130
                        },
                        {
                            text: "-",
                            width: 110
                        },
                        {
                            text: "-",
                            width: 230,
                            fill: true
                        },
                        {
                            text: "-",
                            width: 190
                        },
                        {
                            text: "-",
                            width: 180
                        }
                    ]
                }
            ];
        }
        return nodes.map(function (node) {
            const nodeKey = String(node.key || node.kind || "")
            const controlState = root.model.controlState(node)
            const runState = root.model.basecampHost
                ? String(node.run_state || "unknown")
                : root.model.publicTestnetMode()
                ? root.model.observedRunState(nodeKey)
                : String(node.run_state || "unknown")
            const observation = root.model.observedNode(nodeKey)
            const observationDetail = root.model.basecampHost
                ? String(node.detail || "")
                : String(observation && observation.detail || "")
            const channelIndexers = nodeKey === "indexer" && observation
                && Array.isArray(observation.channels) ? observation.channels : []
            const multiChannelIndexer = channelIndexers.length > 0
            return {
                key: nodeKey,
                cells: [
                    {
                        text: multiChannelIndexer ? qsTr("Channel Indexers")
                            : String(node.label || node.kind || "-"),
                        width: 150,
                        monospace: false
                    },
                    {
                        text: root.stateLabel(controlState),
                        width: 130,
                        tone: root.installTone(controlState),
                        monospace: false
                    },
                    {
                        text: root.stateLabel(runState),
                        width: 110,
                        tone: root.runTone(runState),
                        monospace: false
                    },
                    {
                        text: multiChannelIndexer
                            ? qsTr("%1 configured Channels").arg(channelIndexers.length)
                            : String(node.endpoint || "-"),
                        width: 230,
                        fill: true,
                        copyText: multiChannelIndexer ? "" : String(node.endpoint || "")
                    },
                    {
                        text: multiChannelIndexer
                            ? root.shortText(root.channelIndexerHeads(channelIndexers), 32)
                            : root.shortText(node.data_dir || "-", 32),
                        width: 190,
                        copyText: multiChannelIndexer ? root.channelIndexerHeads(channelIndexers)
                            : String(node.data_dir || "")
                    },
                    {
                        text: observationDetail.length > 0
                            ? observationDetail : root.lastActionText(node.last_action),
                        width: 180,
                        monospace: false
                    }
                ]
            };
        });
    }

    function channelIndexerHeads(channels) {
        const rows = Array.isArray(channels) ? channels : []
        return rows.map(function (row) {
            const value = row || ({})
            const channel = String(value.short_channel_id || value.channel_id || qsTr("Channel"))
            const head = value.head === null || value.head === undefined
                ? String(value.status || qsTr("unknown")) : String(value.head)
            return channel + " " + head
        }).join(" · ")
    }

    function actionRows() {
        const report = root.model.report || null;
        const nodes = report && Array.isArray(report.nodes) ? report.nodes : [];
        return nodes.filter(function (node) {
            return String(node.key || node.kind || "") !== "indexer"
        }).map(function (node) {
            const actions = Array.isArray(node.available_actions) ? node.available_actions : [];
            const setupAction = actions.indexOf("initialize") >= 0 ? "initialize"
                              : (actions.indexOf("install") >= 0 ? "install" : "");
            return {
                key: String(node.key || node.kind || ""),
                label: String(node.label || node.kind || "-"),
                setupAction: setupAction
            };
        });
    }

    function operationRows() {
        const rows = Array.isArray(root.model.operations) ? root.model.operations.slice() : [];
        if (!rows.length) {
            return [
                {
                    time: "-",
                    label: qsTr("No operations"),
                    status: "-",
                    detail: "-"
                }
            ];
        }
        rows.reverse();
        return rows.map(function (row) {
            return {
                time: root.operationTime(row),
                label: root.operationLabel(row),
                status: String(row.status || "-"),
                detail: String(row.detail || "-")
            };
        });
    }

    function operationTime(row) {
        const millis = Number(row.timestamp_millis || row.time || 0);
        if (millis > 0) {
            return new Date(millis).toLocaleTimeString(Qt.locale(), "hh:mm:ss");
        }
        return String(row.time || "-");
    }

    function operationLabel(row) {
        const node = String(row.node || "");
        const action = root.model.actionLabel(row.action);
        return node.length ? qsTr("%1 %2").arg(action).arg(root.nodeLabel(node)) : action;
    }

    function lastActionText(operation) {
        if (!operation) {
            return "-";
        }
        return qsTr("%1 %2").arg(root.model.actionLabel(operation.action)).arg(String(operation.status || ""));
    }

    function openNodeConfirm(action, node) {
        root.model.beginNodeAction(action, node);
        root.showConfirmation();
    }

    function openNodeConfiguration(node) {
        const requestedNode = String(node || "").trim()
        if (!requestedNode.length) {
            return
        }
        root.pendingConfigurationReveal = requestedNode
        root.configurationResponseReady = false
        root.configurationLayoutReady = false
        if (!nodeConfigurationPanel.selectNode(requestedNode)) {
            root.clearConfigurationReveal()
        }
    }

    function revealNodeConfiguration() {
        if (!root.pendingConfigurationReveal.length
                || !root.configurationResponseReady
                || !root.configurationLayoutReady
                || root.model.nodeConfigLoading
                || String(nodeConfigurationPanel.activeNode || "")
                    !== root.pendingConfigurationReveal) {
            return
        }
        const scroller = root.pageScroller
        if (!scroller || !nodeConfigurationPanel.visible) {
            return
        }
        scroller.positionViewAtChild(nodeConfigurationPanel, Flickable.AlignTop)
        root.clearConfigurationReveal()
    }

    function markConfigurationResponseReady() {
        if (!root.pendingConfigurationReveal.length || root.model.nodeConfigLoading) {
            return
        }
        if (root.model.nodeConfigSnapshot === null
                && !String(root.model.nodeConfigError || "").length) {
            return
        }
        root.configurationResponseReady = true
    }

    function noteConfigurationLayout() {
        if (!root.pendingConfigurationReveal.length
                || !root.configurationResponseReady
                || root.model.nodeConfigLoading
                || !nodeConfigurationPanel.visible) {
            return
        }
        root.configurationLayoutReady = true
        Qt.callLater(function () {
            root.revealNodeConfiguration()
        })
    }

    function clearConfigurationReveal() {
        root.pendingConfigurationReveal = ""
        root.configurationResponseReady = false
        root.configurationLayoutReady = false
    }

    function openNetworkConfirm(action) {
        const actionKey = String(action || "");
        root.model.beginNetworkAction(actionKey, actionKey === "new_network" ? root.newNetworkId.trim() : root.activeNetworkId(), actionKey === "load_network" ? root.loadWorkspace.trim() : "");
        root.showConfirmation();
    }

    function openRuntimeConfirm(action) {
        root.model.beginRuntimeAction(action, root.runtimeModulesDir.trim(), root.runtimeBinaryPath.trim());
        root.showConfirmation();
    }

    function showConfirmation() {
        root.confirmationGeneration += 1
        root.confirmationAccepted = false
        confirmPopup.open()
    }

    function acceptPendingAction() {
        root.model.runPendingAction();
    }

    function confirmTitle() {
        return root.model.actionDraftTitle();
    }

    function confirmMessage() {
        return root.model.actionDraftMessage();
    }

    function confirmActionText() {
        if (root.model.pendingOperation === "module_repository"
                || root.model.pendingOperation === "module_file") {
            return qsTr("Install");
        }
        return root.model.actionLabel(root.model.pendingAction);
    }

    function stateLabel(value) {
        const text = String(value || "unknown").replace(/_/g, " ");
        return text.length ? text[0].toUpperCase() + text.slice(1) : qsTr("Unknown");
    }

    function installTone(value) {
        const text = String(value || "");
        if (text === "installed" || text === "managed") {
            return "success";
        }
        if (text === "needs_configuration") {
            return "warning";
        }
        return "neutral";
    }

    function runTone(value) {
        const text = String(value || "");
        if (text === "running" || text === "online") {
            return "success";
        }
        if (text === "initializing" || text === "starting" || text === "stopping" || text === "stale_pid"
                || text === "syncing") {
            return "warning";
        }
        if (text === "failed" || text === "unavailable") {
            return "error";
        }
        return "neutral";
    }

    function nodeLabel(kind) {
        return root.model.nodeLabel(kind);
    }

    function shortText(value, limit) {
        return UiFormat.shortText(value, {
            emptyText: "-",
            limit: limit || 24,
            minimum: 8,
            tailLength: 6
        });
    }

    component ModulePackageSelector: ComboBox {
        id: selector

        property var options: []
        property string emptyText: qsTr("No options")
        property string accessibleLabel: ""
        property bool monospace: false

        signal optionSelected(var option)

        model: selector.options
        textRole: "label"
        currentIndex: -1
        displayText: currentIndex >= 0 && currentIndex < count
            ? String(selector.options[currentIndex].label || "") : emptyText
        hoverEnabled: true
        Layout.preferredHeight: root.theme.controlHeight
        onActivated: index => {
            if (index >= 0 && index < selector.options.length) {
                selector.optionSelected(selector.options[index])
            }
        }

        delegate: ItemDelegate {
            id: optionDelegate

            required property int index
            required property var modelData

            width: selector.width
            text: String(modelData && modelData.label || "")
            hoverEnabled: true
            highlighted: selector.highlightedIndex === index

            contentItem: Text {
                text: optionDelegate.text
                color: optionDelegate.highlighted ? root.theme.selectedText : root.theme.text
                textFormat: Text.PlainText
                verticalAlignment: Text.AlignVCenter
                font.family: selector.monospace ? "monospace" : ""
                font.pixelSize: root.theme.secondaryText
            }

            background: Rectangle {
                color: optionDelegate.highlighted
                    ? root.theme.accent
                    : (optionDelegate.hovered ? root.theme.hover : root.theme.surfaceRaised)
            }
        }

        contentItem: Text {
            text: selector.displayText
            color: selector.enabled ? root.theme.text : root.theme.textDim
            textFormat: Text.PlainText
            verticalAlignment: Text.AlignVCenter
            leftPadding: 12
            rightPadding: 36
            font.family: selector.monospace ? "monospace" : ""
            font.pixelSize: root.theme.primaryText
            font.weight: Font.Medium
        }

        indicator: Text {
            x: selector.width - width - 14
            y: (selector.height - height) / 2
            text: "\u25be"
            color: selector.enabled ? root.theme.textMuted : root.theme.textDim
            textFormat: Text.PlainText
            font.pixelSize: root.theme.secondaryText
        }

        background: Rectangle {
            radius: root.theme.radius
            color: selector.hovered || selector.activeFocus
                ? root.theme.surfaceRaised : root.theme.field
            border.width: selector.activeFocus ? 2 : 1
            border.color: selector.activeFocus
                ? root.theme.accent : root.theme.outlineMuted
        }

        popup: Popup {
            y: selector.height + root.theme.gapTiny
            width: selector.width
            implicitHeight: Math.min(contentItem.implicitHeight + 2, 260)
            padding: 1

            contentItem: ListView {
                clip: true
                implicitHeight: contentHeight
                model: selector.popup.visible ? selector.delegateModel : null
                currentIndex: selector.highlightedIndex
            }

            background: Rectangle {
                radius: root.theme.radius
                color: root.theme.surfaceRaised
                border.width: 1
                border.color: root.theme.outline
            }
        }

        Accessible.role: Accessible.ComboBox
        Accessible.name: selector.accessibleLabel
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
                return rowRoot.theme.textMuted;
            }
            if (index === 2) {
                if (rowRoot.status === "started" || rowRoot.status === "installed" || rowRoot.status === "initialized" || rowRoot.status === "created" || rowRoot.status === "loaded" || rowRoot.status === "stopped" || rowRoot.status === "purged" || rowRoot.status === "reset" || rowRoot.status === "deleted") {
                    return rowRoot.theme.success;
                }
                if (rowRoot.status === "starting" || rowRoot.status === "stopping") {
                    return rowRoot.theme.warning;
                }
                if (rowRoot.status === "failed") {
                    return rowRoot.theme.error;
                }
                if (rowRoot.status === "needs_configuration") {
                    return rowRoot.theme.warning;
                }
            }
            return rowRoot.theme.text;
        }

        function columnWidth(index) {
            if (index === 0) {
                return 88;
            }
            if (index === 1) {
                return 170;
            }
            if (index === 2) {
                return 120;
            }
            return 280;
        }
    }
}
