import QtQml
import "ConfirmationPolicy.js" as ConfirmationPolicy
import "OperationHistoryVocabulary.js" as OperationHistoryVocabulary

QtObject {
    id: root

    required property var gateway
    property string networkProfile: "default"
    property bool busy: false

    property var report: null
    property string error: ""
    property var operations: []
    property int revision: 0
    property var devnets: []
    property string pendingAction: ""
    property string pendingNode: ""
    property string pendingNetworkId: ""
    property string pendingWorkspace: ""

    function clearStatus() {
        report = null
        error = ""
        revision += 1
    }

    function refresh(showResult) {
        error = ""
        return gateway.request("localNodesStatus", [networkProfile], qsTr("Local nodes"), showResult === true, function (response) {
            if (response.ok) {
                report = response.value || null
                operations = response.value && Array.isArray(response.value.operations) ? response.value.operations : []
                error = ""
                revision += 1
            } else {
                report = null
                error = response.error || qsTr("Local node status failed.")
                revision += 1
            }
        })
    }

    function refreshDevnets() {
        return gateway.request("localDevnetList", [networkProfile], qsTr("Local Devnets"), false, function (response) {
            if (response.ok) {
                devnets = response.value && Array.isArray(response.value.devnets) ? response.value.devnets : []
            }
        })
    }

    function runAction(action, node, networkId, workspacePath, label) {
        if (busy) {
            gateway.setResult(qsTr("Local nodes"), qsTr("Another inspection is already running."), true, null)
            return null
        }
        const request = {
            action: String(action || "")
        }
        const nodeKey = String(node || "")
        if (nodeKey.length) {
            request.node = nodeKey
        }
        const targetNetwork = String(networkId || "").trim()
        if (targetNetwork.length) {
            request.network_id = targetNetwork
        }
        const workspace = String(workspacePath || "").trim()
        if (workspace.length) {
            request.workspace_path = workspace
        }

        const operationLabel = String(label || actionLabel(action))
        gateway.setBusy(true, operationLabel)
        return gateway.request("localNodesAction", [networkProfile, request, ConfirmationPolicy.token("local-node-action")], operationLabel, true, function (response) {
            gateway.setBusy(false, "")
            if (response.ok) {
                report = response.value || null
                operations = response.value && Array.isArray(response.value.operations) ? response.value.operations : []
                error = ""
                revision += 1
                const detail = actionDetail(operations, request)
                gateway.appendOperationHistory({
                    domain: "localNodes",
                    method: "localNodesAction",
                    status: "completed",
                    label: operationLabel,
                    result: {
                        status: "completed",
                        detail: detail
                    }
                }, detail)
                refreshDevnets()
            } else {
                error = response.error || qsTr("Local node action failed.")
                appendOperation(actionLabel(action), "failed", error)
            }
        })
    }

    function appendOperation(label, status, detail) {
        const labelText = String(label || qsTr("Local nodes"))
        const statusText = String(status || "failed")
        const detailText = String(detail || "")
        const rows = Array.isArray(operations) ? operations.slice(0) : []
        rows.push({
            time: new Date().toLocaleTimeString(Qt.locale(), "hh:mm:ss"),
            action: labelText,
            status: statusText,
            detail: detailText
        })
        operations = rows.slice(-50)
        revision += 1
        gateway.appendOperationHistory({
            domain: "localNodes",
            method: "localNodesAction",
            status: OperationHistoryVocabulary.syntheticHistoryStatus(statusText),
            label: labelText,
            result: {
                status: statusText,
                detail: detailText
            },
            error: statusText === "failed" ? detailText : ""
        }, detailText)
    }

    function actionDetail(operationRows, request) {
        const rows = Array.isArray(operationRows) ? operationRows : []
        if (rows.length > 0) {
            const row = rows[rows.length - 1] || {}
            const detail = String(row.detail || "")
            if (detail.length) {
                return detail
            }
            const status = String(row.status || "")
            if (status.length) {
                return status
            }
        }
        const node = request && request.node ? String(request.node) : ""
        if (node.length) {
            return node
        }
        const network = request && request.network_id ? String(request.network_id) : ""
        if (network.length) {
            return network
        }
        return ""
    }

    function actionLabel(action) {
        switch (String(action || "")) {
        case "install":
            return qsTr("Install")
        case "uninstall":
            return qsTr("Uninstall")
        case "new_network":
            return qsTr("New Local Devnet")
        case "load_network":
            return qsTr("Load Local Devnet")
        case "delete_network":
            return qsTr("Delete Local Devnet")
        case "reset_network":
            return qsTr("Reset Local Devnet")
        case "start":
            return qsTr("Start")
        case "stop":
            return qsTr("Stop")
        case "purge":
            return qsTr("Purge")
        default:
            return qsTr("Local node action")
        }
    }

    function beginNodeAction(action, node) {
        pendingAction = String(action || "")
        pendingNode = String(node || "")
        pendingNetworkId = ""
        pendingWorkspace = ""
    }

    function beginNetworkAction(action, networkId, workspacePath) {
        pendingAction = String(action || "")
        pendingNode = ""
        pendingNetworkId = String(networkId || "").trim()
        pendingWorkspace = String(workspacePath || "").trim()
    }

    function clearActionDraft() {
        pendingAction = ""
        pendingNode = ""
        pendingNetworkId = ""
        pendingWorkspace = ""
    }

    function runPendingAction() {
        if (!pendingAction.length) {
            return null
        }
        const action = pendingAction
        const node = pendingNode
        const networkId = pendingNetworkId
        const workspacePath = pendingWorkspace
        const label = actionDraftTitle()
        clearActionDraft()
        return runAction(action, node, networkId, workspacePath, label)
    }

    function actionDraftTitle() {
        if (!pendingAction.length) {
            return qsTr("Confirm")
        }
        if (pendingNode.length) {
            return qsTr("%1 %2").arg(actionLabel(pendingAction)).arg(nodeLabel(pendingNode))
        }
        return actionLabel(pendingAction)
    }

    function actionDraftMessage() {
        const action = pendingAction
        if (action === "delete_network") {
            return qsTr("This stops all local nodes in Local Devnet %1 and removes the managed workspace plus node data.").arg(pendingNetworkId.length ? pendingNetworkId : activeNetworkId())
        }
        if (action === "reset_network") {
            return qsTr("This stops all local nodes in Local Devnet %1, deletes node databases, and regenerates configs in the same workspace.").arg(pendingNetworkId.length ? pendingNetworkId : activeNetworkId())
        }
        if (action === "new_network") {
            const target = pendingNetworkId.length ? pendingNetworkId : qsTr("a generated Local Devnet")
            return qsTr("This creates %1 under the Managed Workspace Root and sets it as Active Devnet.").arg(target)
        }
        if (action === "load_network") {
            return qsTr("This loads the Local Devnet manifest from %1 and sets it as Active Devnet.").arg(pendingWorkspace)
        }
        const node = nodeByKind(pendingNode) || {}
        if (action === "purge") {
            return qsTr("This stops %1 and deletes data directory %2. Config and install record remain.").arg(nodeLabel(pendingNode)).arg(String(node.data_dir || "-"))
        }
        if (action === "uninstall") {
            return qsTr("This stops %1 and removes its install registration. Node databases remain.").arg(nodeLabel(pendingNode))
        }
        if (action === "start") {
            return qsTr("This starts %1 using config %2.").arg(nodeLabel(pendingNode)).arg(String(node.config_path || "-"))
        }
        if (action === "stop") {
            return qsTr("This stops %1 and keeps its data and config.").arg(nodeLabel(pendingNode))
        }
        if (action === "install") {
            return qsTr("This verifies %1 control tooling and records the resolved install path. It does not start the node.").arg(nodeLabel(pendingNode))
        }
        return qsTr("Run local node action.")
    }

    function activeNetworkId() {
        const reportValue = report || null
        return String(reportValue && reportValue.active_devnet ? reportValue.active_devnet : "")
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

    function nodeByKind(kind) {
        const nodes = revision >= 0 && report && Array.isArray(report.nodes) ? report.nodes : []
        const key = String(kind || "")
        for (let i = 0; i < nodes.length; ++i) {
            if (String(nodes[i].key || nodes[i].kind || "") === key) {
                return nodes[i]
            }
        }
        return null
    }

    function actionEnabled(kind, action) {
        const node = nodeByKind(kind)
        const actions = node && Array.isArray(node.available_actions) ? node.available_actions : []
        return actions.indexOf(String(action || "")) >= 0 && busy !== true
    }

    function networkActionEnabled(action) {
        if (busy) {
            return false
        }
        const key = String(action || "")
        const actions = networkActions()
        return actions.indexOf(key) >= 0
    }

    function networkActions() {
        const reportValue = report || null
        if (reportValue && Array.isArray(reportValue.available_network_actions)) {
            return reportValue.available_network_actions
        }
        if (!localMode()) {
            return []
        }
        const actions = ["new_network", "load_network"]
        if (reportValue && String(reportValue.active_devnet || "").length > 0) {
            actions.push("reset_network")
            actions.push("delete_network")
        }
        return actions
    }

    function localMode() {
        const reportValue = report || null
        if (!reportValue) {
            return false
        }
        const mode = reportValue ? String(reportValue.mode || "") : ""
        if (mode.length) {
            return mode === "localnet"
        }
        const profile = reportValue ? String(reportValue.profile || "") : ""
        if (profile.length) {
            return profile === "local"
        }
        return false
    }

    function modeLabel() {
        return localMode() ? qsTr("Local Devnet") : qsTr("External Network")
    }

    function summaryText() {
        const summary = report && report.summary ? report.summary : null
        if (!summary) {
            return qsTr("Not loaded")
        }
        return qsTr("%1/%2 running").arg(Number(summary.running || 0)).arg(Number(summary.total || 0))
    }

    function toolProblem() {
        const reportValue = report || null
        const problem = reportValue ? String(reportValue.primary_problem || "") : ""
        if (problem === "missing_logoscore") {
            return qsTr("logoscore not found. Module-backed node actions will report needs_configuration.")
        }
        if (problem === "missing_sequencer_binary") {
            return qsTr("sequencer_service not found. Local sequencer start requires a configured binary.")
        }
        return ""
    }
}
