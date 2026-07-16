pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../components"
import "../../../components/common"
import "../../../state"
import "../../../theme"
import "../../../utils/UiFormat.js" as UiFormat

ColumnLayout {
    id: root

    required property Theme theme
    required property LocalNodesState model

    property string newNetworkId: ""
    property string loadWorkspace: ""
    property string runtimeModulesDir: ""
    property string runtimeBinaryPath: ""

    width: parent ? parent.width : 900
    spacing: 16

    Component.onCompleted: {
        root.model.refresh(false);
        root.model.refreshDevnets();
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / System / Local Nodes")
        title: qsTr("Local Nodes")
        layerLabel: qsTr("System")
        subtitle: qsTr("Local Bedrock, Indexer, Delivery, and Storage connected to Logos Testnet.")
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
        message: qsTr("Local Bedrock feeds the UI and Local Indexer. Channel Zones use Local Indexer history with the remote Testnet Sequencer.")
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
                theme: root.theme
                label: qsTr("Modules directory")
                sourceText: root.runtimeModulesDir.length ? root.runtimeModulesDir : root.model.runtimeModulesDir()
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
                    theme: root.theme
                    text: qsTr("Start")
                    primary: true
                    enabled: root.model.runtimeActionEnabled("start_runtime") && (root.runtimeModulesDir.trim().length > 0 || root.model.runtimeModulesDir().length > 0)
                    Layout.preferredWidth: 96
                    onClicked: root.openRuntimeConfirm("start_runtime")
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Stop")
                    enabled: root.model.runtimeActionEnabled("stop_runtime")
                    Layout.preferredWidth: 96
                    onClicked: root.openRuntimeConfirm("stop_runtime")
                }
            }
        }
    }

    Panel {
        theme: root.theme
        title: qsTr("Node Status")

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
                    text: root.model.localMode() ? qsTr("Run") : qsTr("Health"),
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
                        text: root.model.actionLabel(actionRow.modelData.setupAction)
                        enabled: root.model.actionEnabled(actionRow.modelData.key, actionRow.modelData.setupAction)
                        Layout.preferredWidth: 92
                        onClicked: root.openNodeConfirm(actionRow.modelData.setupAction, actionRow.modelData.key)
                    }

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Start")
                        primary: true
                        enabled: root.model.actionEnabled(actionRow.modelData.key, "start")
                        Layout.preferredWidth: 84
                        onClicked: root.openNodeConfirm("start", actionRow.modelData.key)
                    }

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Stop")
                        enabled: root.model.actionEnabled(actionRow.modelData.key, "stop")
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

        theme: root.theme
        title: root.confirmTitle()
        message: root.confirmMessage()
        confirmText: root.model.actionLabel(root.model.pendingAction)
        confirmEnabled: !root.model.busy && root.model.pendingAction.length > 0
        onAccepted: root.acceptPendingAction()
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
            const controlState = root.model.controlState(node)
            const runState = root.model.publicTestnetMode()
                ? root.model.observedRunState(node.key || node.kind)
                : String(node.run_state || "unknown")
            const observation = root.model.observedNode(node.key || node.kind)
            const observationDetail = String(observation && observation.detail || "")
            return {
                key: String(node.key || node.kind || ""),
                cells: [
                    {
                        text: String(node.label || node.kind || "-"),
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
                        text: String(node.endpoint || "-"),
                        width: 230,
                        fill: true,
                        copyText: String(node.endpoint || "")
                    },
                    {
                        text: root.shortText(node.data_dir || "-", 32),
                        width: 190,
                        copyText: String(node.data_dir || "")
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

    function actionRows() {
        const report = root.model.report || null;
        const nodes = report && Array.isArray(report.nodes) ? report.nodes : [];
        return nodes.map(function (node) {
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
        confirmPopup.open();
    }

    function openNetworkConfirm(action) {
        const actionKey = String(action || "");
        root.model.beginNetworkAction(actionKey, actionKey === "new_network" ? root.newNetworkId.trim() : root.activeNetworkId(), actionKey === "load_network" ? root.loadWorkspace.trim() : "");
        confirmPopup.open();
    }

    function openRuntimeConfirm(action) {
        root.model.beginRuntimeAction(action, root.runtimeModulesDir.trim(), root.runtimeBinaryPath.trim());
        confirmPopup.open();
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
        if (text === "starting" || text === "stopping" || text === "stale_pid"
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
