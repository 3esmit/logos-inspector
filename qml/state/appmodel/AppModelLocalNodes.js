.import "../../services/BridgeHelpers.js" as BridgeHelpers

function refreshLocalNodes(root, showResult) {
    with (root) {
        localNodesError = ""
        return requestModuleAsync(inspectorModule, "localNodesStatus", [networkProfile], qsTr("Local nodes"), showResult === true, function (response) {
            if (response.ok) {
                localNodesReport = response.value || null
                localNodesOperations = response.value && Array.isArray(response.value.operations) ? response.value.operations : []
                localNodesError = ""
                localNodesRevision += 1
            } else {
                localNodesReport = null
                localNodesError = response.error || qsTr("Local node status failed.")
                localNodesRevision += 1
            }
        })
    }
}

function refreshLocalDevnets(root) {
    with (root) {
        return requestModuleAsync(inspectorModule, "localDevnetList", [networkProfile], qsTr("Local networks"), false, function (response) {
            if (response.ok) {
                localDevnets = response.value && Array.isArray(response.value.devnets) ? response.value.devnets : []
            }
        })
    }
}

function runLocalNodeAction(root, action, node, networkId, workspacePath, label) {
    with (root) {
        if (busy) {
            setResult(qsTr("Local nodes"), qsTr("Another inspection is already running."), true)
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

        const operationLabel = String(label || localNodeActionLabel(action))
        busy = true
        statusText = operationLabel
        return requestModuleAsync(inspectorModule, "localNodesAction", [networkProfile, request, "confirm-local-node-action"], operationLabel, true, function (response) {
            busy = false
            if (response.ok) {
                localNodesReport = response.value || null
                localNodesOperations = response.value && Array.isArray(response.value.operations) ? response.value.operations : []
                localNodesError = ""
                localNodesRevision += 1
                appendNodeOperationHistory({
                    domain: "localNodes",
                    method: "localNodesAction",
                    status: "completed",
                    label: operationLabel,
                    result: {
                        status: "completed",
                        detail: localNodeActionDetail(localNodesOperations, request)
                    }
                }, localNodeActionDetail(localNodesOperations, request))
                refreshLocalDevnets()
            } else {
                localNodesError = response.error || qsTr("Local node action failed.")
                appendLocalNodeOperation(localNodeActionLabel(action), "failed", localNodesError)
            }
        })
    }
}

function appendLocalNodeOperation(root, label, status, detail) {
    with (root) {
        const labelText = String(label || qsTr("Local nodes"))
        const statusText = String(status || "failed")
        const detailText = String(detail || "")
        const rows = Array.isArray(localNodesOperations) ? localNodesOperations.slice(0) : []
        rows.push({
            time: new Date().toLocaleTimeString(Qt.locale(), "hh:mm:ss"),
            action: labelText,
            status: statusText,
            detail: detailText
        })
        localNodesOperations = rows.slice(-50)
        localNodesRevision += 1
        appendNodeOperationHistory({
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
}

function localNodeActionDetail(operations, request) {
    const rows = Array.isArray(operations) ? operations : []
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

function localNodeActionLabel(root, action) {
    with (root) {
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
}

function localNodeByKind(root, kind) {
    with (root) {
        const revision = localNodesRevision
        const nodes = localNodesReport && Array.isArray(localNodesReport.nodes) ? localNodesReport.nodes : []
        const key = String(kind || "")
        for (let i = 0; i < nodes.length; ++i) {
            if (String(nodes[i].key || nodes[i].kind || "") === key) {
                return nodes[i]
            }
        }
        return null
    }
}

function localNodeActionEnabled(root, kind, action) {
    const node = localNodeByKind(root, kind)
    const actions = node && Array.isArray(node.available_actions) ? node.available_actions : []
    return actions.indexOf(String(action || "")) >= 0 && root.busy !== true
}

function localNodeNetworkActionEnabled(root, action) {
    with (root) {
        if (networkProfile !== "local" || busy) {
            return false
        }
        const report = localNodesReport || {}
        const hasActive = String(report.active_devnet || "").length > 0
        const key = String(action || "")
        if (key === "new_network" || key === "load_network") {
            return true
        }
        return hasActive && (key === "reset_network" || key === "delete_network")
    }
}

function localNodeModeLabel(root) {
    with (root) {
        return networkProfile === "local" ? qsTr("Localnet") : qsTr("Public/Testnet")
    }
}

function localNodeSummaryText(root) {
    with (root) {
        const summary = localNodesReport && localNodesReport.summary ? localNodesReport.summary : null
        if (!summary) {
            return qsTr("Not loaded")
        }
        return qsTr("%1/%2 running").arg(Number(summary.running || 0)).arg(Number(summary.total || 0))
    }
}

function localNodeToolProblem(root) {
    with (root) {
        const tools = localNodesReport && localNodesReport.tools ? localNodesReport.tools : null
        if (!tools) {
            return ""
        }
        const logoscore = tools.logoscore && tools.logoscore.available === true
        const sequencer = localNodeByKind(root, "sequencer")
        if (!logoscore) {
            return qsTr("logoscore not found. Module-backed node actions will report needs_configuration.")
        }
        if (networkProfile === "local" && sequencer && String(sequencer.install_state || "") === "needs_configuration") {
            return qsTr("sequencer_service not found. Local sequencer start requires a configured binary.")
        }
        return ""
    }
}
