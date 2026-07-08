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
    property string pendingAction: ""
    property string pendingNode: ""
    property string pendingNetworkId: ""
    property string pendingWorkspace: ""

    width: parent ? parent.width : 900
    spacing: 16

    Component.onCompleted: {
        root.model.refresh(false)
        root.model.refreshDevnets()
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / System / Local Nodes")
        title: qsTr("Local Nodes")
        layerLabel: qsTr("System")
        subtitle: qsTr("Manual lifecycle controls for configured Logos nodes.")
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
            columns: root.width < 760 ? 2 : 4
            columnSpacing: root.theme.gapSmall
            rowSpacing: root.theme.gapSmall

            StatusChip {
                theme: root.theme
                label: qsTr("Mode")
                value: root.model.modeLabel()
                tone: root.model.networkProfile === "local" ? "success" : "neutral"
                compact: true
                showIndicator: true
                Layout.fillWidth: true
            }

            StatusChip {
                theme: root.theme
                label: qsTr("Active")
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
                tone: root.summaryTone()
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

    Panel {
        visible: root.model.networkProfile === "local"
        theme: root.theme
        title: qsTr("Local Network")

        ColumnLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            GridLayout {
                columns: root.width < 840 ? 1 : 4
                columnSpacing: root.theme.gapSmall
                rowSpacing: root.theme.gapSmall
                Layout.fillWidth: true

                FieldRow {
                    theme: root.theme
                    label: qsTr("Network ID")
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
        theme: root.theme
        title: qsTr("Node Status")

        DataTableFrame {
            theme: root.theme
            headerCells: [
                { text: qsTr("Node"), width: 150 },
                { text: qsTr("Install"), width: 130 },
                { text: qsTr("Run"), width: 110 },
                { text: qsTr("Endpoint"), width: 230, fill: true },
                { text: qsTr("Data"), width: 190 },
                { text: qsTr("Last"), width: 180 }
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
                        text: qsTr("Install")
                        enabled: root.model.actionEnabled(actionRow.modelData.key, "install")
                        Layout.preferredWidth: 92
                        onClicked: root.openNodeConfirm("install", actionRow.modelData.key)
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
        confirmText: root.model.actionLabel(root.pendingAction)
        confirmEnabled: !root.model.busy && root.pendingAction.length > 0
        onAccepted: root.acceptPendingAction()
    }

    function activeNetworkId() {
        const report = root.model.report || null
        return String(report && report.active_devnet ? report.active_devnet : "")
    }

    function workspaceLabel() {
        const report = root.model.report || null
        return String(report && report.workspace_root ? report.workspace_root : "")
    }

    function summaryTone() {
        const report = root.model.report || null
        const summary = report && report.summary ? report.summary : null
        if (!summary) {
            return "warning"
        }
        if (Number(summary.needs_configuration || 0) > 0) {
            return "warning"
        }
        return Number(summary.running || 0) > 0 ? "success" : "neutral"
    }

    function nodeTableRows() {
        const report = root.model.report || null
        const nodes = report && Array.isArray(report.nodes) ? report.nodes : []
        if (!nodes.length) {
            return [{
                cells: [
                    { text: qsTr("No node status loaded"), width: 150, monospace: false },
                    { text: "-", width: 130 },
                    { text: "-", width: 110 },
                    { text: "-", width: 230, fill: true },
                    { text: "-", width: 190 },
                    { text: "-", width: 180 }
                ]
            }]
        }
        return nodes.map(function (node) {
            return {
                key: String(node.key || node.kind || ""),
                cells: [
                    { text: String(node.label || node.kind || "-"), width: 150, monospace: false },
                    { text: root.stateLabel(node.install_state), width: 130, tone: root.installTone(node.install_state), monospace: false },
                    { text: root.stateLabel(node.run_state), width: 110, tone: root.runTone(node.run_state), monospace: false },
                    { text: String(node.endpoint || "-"), width: 230, fill: true, copyText: String(node.endpoint || "") },
                    { text: root.shortText(node.data_dir || "-", 32), width: 190, copyText: String(node.data_dir || "") },
                    { text: root.lastActionText(node.last_action), width: 180, monospace: false }
                ]
            }
        })
    }

    function actionRows() {
        const report = root.model.report || null
        const nodes = report && Array.isArray(report.nodes) ? report.nodes : []
        return nodes.map(function (node) {
            return {
                key: String(node.key || node.kind || ""),
                label: String(node.label || node.kind || "-")
            }
        })
    }

    function operationRows() {
        const rows = Array.isArray(root.model.operations) ? root.model.operations.slice() : []
        if (!rows.length) {
            return [{ time: "-", label: qsTr("No operations"), status: "-", detail: "-" }]
        }
        rows.reverse()
        return rows.map(function (row) {
            return {
                time: root.operationTime(row),
                label: root.operationLabel(row),
                status: String(row.status || "-"),
                detail: String(row.detail || "-")
            }
        })
    }

    function operationTime(row) {
        const millis = Number(row.timestamp_millis || row.time || 0)
        if (millis > 0) {
            return new Date(millis).toLocaleTimeString(Qt.locale(), "hh:mm:ss")
        }
        return String(row.time || "-")
    }

    function operationLabel(row) {
        const node = String(row.node || "")
        const action = root.model.actionLabel(row.action)
        return node.length ? qsTr("%1 %2").arg(action).arg(root.nodeLabel(node)) : action
    }

    function lastActionText(operation) {
        if (!operation) {
            return "-"
        }
        return qsTr("%1 %2").arg(root.model.actionLabel(operation.action)).arg(String(operation.status || ""))
    }

    function openNodeConfirm(action, node) {
        root.pendingAction = String(action || "")
        root.pendingNode = String(node || "")
        root.pendingNetworkId = ""
        root.pendingWorkspace = ""
        confirmPopup.open()
    }

    function openNetworkConfirm(action) {
        root.pendingAction = String(action || "")
        root.pendingNode = ""
        root.pendingNetworkId = root.pendingAction === "new_network" ? root.newNetworkId.trim() : root.activeNetworkId()
        root.pendingWorkspace = root.pendingAction === "load_network" ? root.loadWorkspace.trim() : ""
        confirmPopup.open()
    }

    function acceptPendingAction() {
        root.model.runAction(
            root.pendingAction,
            root.pendingNode,
            root.pendingNetworkId,
            root.pendingWorkspace,
            root.confirmTitle()
        )
    }

    function confirmTitle() {
        if (!root.pendingAction.length) {
            return qsTr("Confirm")
        }
        if (root.pendingNode.length) {
            return qsTr("%1 %2").arg(root.model.actionLabel(root.pendingAction)).arg(root.nodeLabel(root.pendingNode))
        }
        return root.model.actionLabel(root.pendingAction)
    }

    function confirmMessage() {
        const action = root.pendingAction
        if (action === "delete_network") {
            return qsTr("This stops all local nodes in %1 and removes the managed workspace plus node data.").arg(root.activeNetworkId())
        }
        if (action === "reset_network") {
            return qsTr("This stops all local nodes in %1, deletes node databases, and regenerates configs in the same workspace.").arg(root.activeNetworkId())
        }
        if (action === "new_network") {
            const target = root.pendingNetworkId.length ? root.pendingNetworkId : qsTr("a generated devnet")
            return qsTr("This creates %1 under the managed local nodes workspace and makes it active.").arg(target)
        }
        if (action === "load_network") {
            return qsTr("This loads the managed local network manifest from %1 and makes it active.").arg(root.pendingWorkspace)
        }
        const node = root.model.nodeByKind(root.pendingNode) || {}
        if (action === "purge") {
            return qsTr("This stops %1 and deletes data directory %2. Config and install record remain.").arg(root.nodeLabel(root.pendingNode)).arg(String(node.data_dir || "-"))
        }
        if (action === "uninstall") {
            return qsTr("This stops %1 and removes its install registration. Node databases remain.").arg(root.nodeLabel(root.pendingNode))
        }
        if (action === "start") {
            return qsTr("This starts %1 using config %2.").arg(root.nodeLabel(root.pendingNode)).arg(String(node.config_path || "-"))
        }
        if (action === "stop") {
            return qsTr("This stops %1 and keeps its data and config.").arg(root.nodeLabel(root.pendingNode))
        }
        if (action === "install") {
            return qsTr("This verifies %1 control tooling and records the resolved install path. It does not start the node.").arg(root.nodeLabel(root.pendingNode))
        }
        return qsTr("Run local node action.")
    }

    function stateLabel(value) {
        const text = String(value || "unknown").replace(/_/g, " ")
        return text.length ? text[0].toUpperCase() + text.slice(1) : qsTr("Unknown")
    }

    function installTone(value) {
        const text = String(value || "")
        if (text === "installed") {
            return "success"
        }
        if (text === "needs_configuration") {
            return "warning"
        }
        return "neutral"
    }

    function runTone(value) {
        const text = String(value || "")
        if (text === "running") {
            return "success"
        }
        if (text === "stale_pid") {
            return "warning"
        }
        return "neutral"
    }

    function nodeLabel(kind) {
        switch (String(kind || "")) {
        case "bedrock":
            return qsTr("Bedrock")
        case "sequencer":
            return qsTr("Local Sequencer")
        case "indexer":
            return qsTr("Indexer")
        case "storage":
            return qsTr("Storage")
        case "messaging":
            return qsTr("Messaging")
        default:
            return String(kind || "-")
        }
    }

    function shortText(value, limit) {
        return UiFormat.shortText(value, {
            emptyText: "-",
            limit: limit || 24,
            minimum: 8,
            tailLength: 6
        })
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
                return rowRoot.theme.textMuted
            }
            if (index === 2) {
                if (rowRoot.status === "started" || rowRoot.status === "installed" || rowRoot.status === "created" || rowRoot.status === "loaded" || rowRoot.status === "stopped" || rowRoot.status === "purged" || rowRoot.status === "reset" || rowRoot.status === "deleted") {
                    return rowRoot.theme.success
                }
                if (rowRoot.status === "failed") {
                    return rowRoot.theme.error
                }
                if (rowRoot.status === "needs_configuration") {
                    return rowRoot.theme.warning
                }
            }
            return rowRoot.theme.text
        }

        function columnWidth(index) {
            if (index === 0) {
                return 88
            }
            if (index === 1) {
                return 170
            }
            if (index === 2) {
                return 120
            }
            return 280
        }
    }
}
