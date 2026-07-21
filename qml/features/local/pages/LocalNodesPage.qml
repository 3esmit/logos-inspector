pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
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
    property bool confirmationAccepted: false
    property int confirmationGeneration: 0
    property var pageScroller: null
    property string pendingConfigurationReveal: ""
    property bool configurationResponseReady: false
    property bool configurationLayoutReady: false

    width: parent ? parent.width : 900
    spacing: 16

    Component.onCompleted: {
        root.model.refresh(false, true);
        root.model.refreshDevnets();
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

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / System / Local Nodes")
        title: qsTr("Local Nodes")
        layerLabel: qsTr("System")
        subtitle: qsTr("Local Bedrock, Channel Indexer package, Delivery, and Storage connected to Logos Testnet.")
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
        visible: root.model.error.length === 0 && !root.model.localMode()
        theme: root.theme
        tone: "info"
        title: qsTr("Logos Testnet topology")
        message: qsTr("Local Bedrock feeds the UI. Each Channel Zone uses its configured Channel Indexer history with its configured Testnet Sequencer.")
        Layout.fillWidth: true
    }

    Panel {
        objectName: "localDevnetConfiguration"
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
        objectName: "indexerPackageConfiguration"
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
        title: qsTr("System and Channel Status")

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
        confirmText: root.model.actionLabel(root.model.pendingAction)
        confirmEnabled: !root.model.busy && root.model.pendingAction.length > 0
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
            && release !== null
            && String(release.version || "").length > 0
            && String(release.root_hash || "").length > 0
            && root.model.actionEnabled("indexer", "install")
    }

    function packageInstallRuntimeReady() {
        const state = root.model.runtimeState()
        return state !== "running" && state !== "starting" && state !== "stopping"
    }

    function packageStatusTone() {
        if (root.model.packageCatalogError.length > 0) {
            return "error"
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
            const runState = root.model.publicTestnetMode()
                ? root.model.observedRunState(nodeKey)
                : String(node.run_state || "unknown")
            const observation = root.model.observedNode(nodeKey)
            const observationDetail = String(observation && observation.detail || "")
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
