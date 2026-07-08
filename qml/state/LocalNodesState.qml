import QtQml
import "ConfirmationPolicy.js" as ConfirmationPolicy

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
        return gateway.request("localDevnetList", [networkProfile], qsTr("Local networks"), false, function (response) {
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
            status: statusText === "failed" ? "failed" : "completed",
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
            return qsTr("New network")
        case "load_network":
            return qsTr("Load network")
        case "delete_network":
            return qsTr("Delete network")
        case "reset_network":
            return qsTr("Reset network")
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
        if (networkProfile !== "local" || busy) {
            return false
        }
        const reportValue = report || {}
        const hasActive = String(reportValue.active_devnet || "").length > 0
        const key = String(action || "")
        if (key === "new_network" || key === "load_network") {
            return true
        }
        return hasActive && (key === "reset_network" || key === "delete_network")
    }

    function modeLabel() {
        return networkProfile === "local" ? qsTr("Localnet") : qsTr("Public/Testnet")
    }

    function summaryText() {
        const summary = report && report.summary ? report.summary : null
        if (!summary) {
            return qsTr("Not loaded")
        }
        return qsTr("%1/%2 running").arg(Number(summary.running || 0)).arg(Number(summary.total || 0))
    }

    function toolProblem() {
        const tools = report && report.tools ? report.tools : null
        if (!tools) {
            return ""
        }
        const logoscore = tools.logoscore && tools.logoscore.available === true
        const sequencer = nodeByKind("sequencer")
        if (!logoscore) {
            return qsTr("logoscore not found. Module-backed node actions will report needs_configuration.")
        }
        if (networkProfile === "local" && sequencer && String(sequencer.install_state || "") === "needs_configuration") {
            return qsTr("sequencer_service not found. Local sequencer start requires a configured binary.")
        }
        return ""
    }
}
