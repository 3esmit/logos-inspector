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
    property bool statusLoading: false
    property int statusGeneration: 0
    property bool statusRefreshDeferred: false
    property bool statusRefreshShowResult: false
    property bool statusRefreshIncludePackageCatalog: false
    property var devnets: []
    property var packageCatalog: null
    property string packageCatalogError: ""
    property bool packageCatalogLoading: false
    property int packageCatalogGeneration: 0
    property var observedNodes: ({})
    readonly property string defaultRuntimeModulesDir: "/opt/logos-node/modules"
    property string pendingAction: ""
    property string pendingNode: ""
    property string pendingNetworkId: ""
    property string pendingWorkspace: ""
    property string pendingRuntimeModulesDir: ""
    property string pendingRuntimeBinaryPath: ""
    property string pendingPackageVersion: ""
    property string pendingPackageRootHash: ""
    property bool pendingAllowIdentityRotation: false

    onBusyChanged: {
        if (!busy) {
            runDeferredStatusRefresh()
        }
    }

    onNetworkProfileChanged: {
        invalidateStatusRefresh()
        statusRefreshDeferred = false
        statusRefreshShowResult = false
        statusRefreshIncludePackageCatalog = false
        report = null
        error = ""
        operations = []
        clearActionDraft()
        revision += 1
    }

    function clearStatus() {
        report = null;
        error = "";
        revision += 1;
    }

    function refresh(showResult, includePackageCatalog) {
        if (busy || statusLoading) {
            statusRefreshDeferred = true;
            statusRefreshShowResult = statusRefreshShowResult || showResult === true;
            statusRefreshIncludePackageCatalog = statusRefreshIncludePackageCatalog
                || includePackageCatalog === true;
            return null;
        }
        const generation = statusGeneration + 1;
        const requestedProfile = networkProfile;
        statusGeneration = generation;
        statusLoading = true;
        error = "";
        return gateway.request("localNodesStatus", [requestedProfile], qsTr("Local nodes"), showResult === true, function (response) {
            if (generation !== statusGeneration || requestedProfile !== networkProfile) {
                return;
            }
            statusLoading = false;
            if (response.ok) {
                report = response.value || null;
                operations = response.value && Array.isArray(response.value.operations) ? response.value.operations : [];
                error = "";
                revision += 1;
            } else {
                report = null;
                error = response.error || qsTr("Local node status failed.");
                revision += 1;
            }
            if (includePackageCatalog === true) {
                refreshPackageCatalog(root.runtimeModulesDir());
            }
            runDeferredStatusRefresh();
        });
    }

    function runDeferredStatusRefresh() {
        if (!statusRefreshDeferred || busy || statusLoading) {
            return null;
        }
        const showResult = statusRefreshShowResult;
        const includePackageCatalog = statusRefreshIncludePackageCatalog;
        statusRefreshDeferred = false;
        statusRefreshShowResult = false;
        statusRefreshIncludePackageCatalog = false;
        return refresh(showResult, includePackageCatalog);
    }

    function invalidateStatusRefresh() {
        statusGeneration += 1;
        statusLoading = false;
    }

    function refreshDevnets() {
        return gateway.request("localDevnetList", [networkProfile], qsTr("Local Devnets"), false, function (response) {
            if (response.ok) {
                devnets = response.value && Array.isArray(response.value.devnets) ? response.value.devnets : [];
            }
        });
    }

    function refreshPackageCatalog(modulesDir) {
        const requestedModulesDir = String(modulesDir || "").trim();
        const targetModulesDir = requestedModulesDir.length
            ? requestedModulesDir : runtimeModulesDir();
        const generation = packageCatalogGeneration + 1;
        packageCatalogGeneration = generation;
        packageCatalogLoading = true;
        packageCatalogError = "";
        return gateway.request("localNodePackageCatalog", [targetModulesDir], qsTr("Indexer package catalog"), false, function (response) {
            if (generation !== packageCatalogGeneration) {
                return;
            }
            packageCatalogLoading = false;
            const value = response && response.ok ? response.value || null : null;
            const packageValue = value && value.package ? value.package : null;
            if (packageValue && Array.isArray(packageValue.versions)) {
                packageCatalog = value;
                packageCatalogError = "";
                return;
            }
            packageCatalog = null;
            packageCatalogError = response && response.error
                ? response.error : qsTr("Indexer package catalog failed.");
        });
    }

    function activateLocalProfile() {
        if (!gateway.activateLocalProfile()) {
            error = qsTr("Local network profile is not available.")
            revision += 1
            return false
        }
        refresh(false)
        refreshDevnets()
        return true
    }

    function runAction(action, node, networkId, workspacePath, label, runtimeModulesDir, runtimeBinaryPath, packageVersion, packageRootHash, allowIdentityRotation) {
        if (busy) {
            gateway.setResult(qsTr("Local nodes"), qsTr("Another inspection is already running."), true, null);
            return null;
        }
        const request = {
            action: String(action || "")
        };
        const nodeKey = String(node || "");
        if (nodeKey.length) {
            request.node = nodeKey;
        }
        const targetNetwork = String(networkId || "").trim();
        if (targetNetwork.length) {
            request.network_id = targetNetwork;
        }
        const workspace = String(workspacePath || "").trim();
        if (workspace.length) {
            request.workspace_path = workspace;
        }
        const modulesDir = String(runtimeModulesDir || "").trim();
        if (modulesDir.length) {
            request.runtime_modules_dir = modulesDir;
        }
        const binaryPath = String(runtimeBinaryPath || "").trim();
        if (binaryPath.length) {
            request.runtime_binary_path = binaryPath;
        }
        const selectedPackageVersion = String(packageVersion || "").trim();
        if (selectedPackageVersion.length) {
            request.package_version = selectedPackageVersion;
        }
        const selectedPackageRootHash = String(packageRootHash || "").trim();
        if (selectedPackageRootHash.length) {
            request.package_root_hash = selectedPackageRootHash;
        }
        if (allowIdentityRotation === true) {
            request.allow_identity_rotation = true;
        }

        const operationLabel = String(label || actionLabel(action));
        invalidateStatusRefresh();
        gateway.setBusy(true, operationLabel);
        return gateway.request("localNodesAction", [networkProfile, request, ConfirmationPolicy.token("local-node-action")], operationLabel, true, function (response) {
            if (response.ok) {
                report = response.value || null;
                operations = response.value && Array.isArray(response.value.operations) ? response.value.operations : [];
                error = "";
                revision += 1;
                const operation = latestActionOperation(operations);
                const detail = actionDetail(operations, request);
                const operationStatus = String(operation.status || "completed");
                const historyStatus = actionHistoryStatus(operationStatus);
                gateway.appendOperationHistory({
                    domain: "localNodes",
                    method: "localNodesAction",
                    status: historyStatus,
                    label: operationLabel,
                    result: {
                        status: operationStatus,
                        detail: detail
                    },
                    error: historyStatus === "failed" ? detail : ""
                }, detail);
                if (historyStatus === "failed") {
                    gateway.setResult(operationLabel, detail, true, operation);
                }
                refreshDevnets();
                if (request.node === "indexer"
                        && (request.action === "install" || request.action === "uninstall")) {
                    refreshPackageCatalog(request.runtime_modules_dir || root.runtimeModulesDir());
                }
            } else {
                error = response.error || qsTr("Local node action failed.");
                appendNodeOperation(action, nodeKey, "failed", error);
            }
            gateway.setBusy(false, "");
        });
    }

    function appendOperation(label, status, detail) {
        return appendOperationRecord(label, "", status, detail, label);
    }

    function appendNodeOperation(action, node, status, detail) {
        return appendOperationRecord(action, node, status, detail, actionLabel(action));
    }

    function appendOperationRecord(action, node, status, detail, label) {
        const actionKey = String(action || "");
        const nodeKey = String(node || "");
        const labelText = String(label || qsTr("Local nodes"));
        const statusText = String(status || "failed");
        const detailText = String(detail || "");
        const rows = Array.isArray(operations) ? operations.slice(0) : [];
        rows.push({
            time: new Date().toLocaleTimeString(Qt.locale(), "hh:mm:ss"),
            action: actionKey,
            node: nodeKey,
            status: statusText,
            detail: detailText
        });
        operations = rows.slice(-50);
        revision += 1;
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
        }, detailText);
        return rows[rows.length - 1];
    }

    function actionDetail(operationRows, request) {
        const row = latestActionOperation(operationRows);
        if (Object.keys(row).length > 0) {
            const detail = String(row.detail || "");
            if (detail.length) {
                return detail;
            }
            const status = String(row.status || "");
            if (status.length) {
                return status;
            }
        }
        const node = request && request.node ? String(request.node) : "";
        if (node.length) {
            return node;
        }
        const network = request && request.network_id ? String(request.network_id) : "";
        if (network.length) {
            return network;
        }
        return "";
    }

    function latestActionOperation(operationRows) {
        const rows = Array.isArray(operationRows) ? operationRows : [];
        return rows.length > 0 ? rows[rows.length - 1] || {} : {};
    }

    function actionHistoryStatus(status) {
        switch (String(status || "").toLowerCase()) {
        case "failed":
        case "error":
        case "needs_configuration":
        case "timed_out":
        case "interrupted":
            return "failed";
        case "canceled":
        case "cancelled":
            return "canceled";
        default:
            return "completed";
        }
    }

    function actionLabel(action) {
        switch (String(action || "")) {
        case "install":
            return qsTr("Install");
        case "initialize":
            return qsTr("Initialize");
        case "start_runtime":
            return qsTr("Start Local Runtime");
        case "stop_runtime":
            return qsTr("Stop Local Runtime");
        case "uninstall":
            return qsTr("Uninstall");
        case "new_network":
            return qsTr("New Local Devnet");
        case "load_network":
            return qsTr("Load Local Devnet");
        case "delete_network":
            return qsTr("Delete Local Devnet");
        case "reset_network":
            return qsTr("Reset Local Devnet");
        case "start":
            return qsTr("Start");
        case "stop":
            return qsTr("Stop");
        case "purge":
            return qsTr("Purge");
        default:
            return qsTr("Local node action");
        }
    }

    function beginNodeAction(action, node, packageVersion, packageRootHash, modulesDir) {
        pendingAction = String(action || "");
        pendingNode = String(node || "");
        pendingNetworkId = "";
        pendingWorkspace = "";
        pendingRuntimeModulesDir = String(modulesDir || "").trim();
        pendingRuntimeBinaryPath = "";
        pendingPackageVersion = String(packageVersion || "").trim();
        pendingPackageRootHash = String(packageRootHash || "").trim();
        pendingAllowIdentityRotation = pendingAction === "stop" && pendingNode === "messaging";
    }

    function beginNetworkAction(action, networkId, workspacePath) {
        pendingAction = String(action || "");
        pendingNode = "";
        pendingNetworkId = String(networkId || "").trim();
        pendingWorkspace = String(workspacePath || "").trim();
        pendingRuntimeModulesDir = "";
        pendingRuntimeBinaryPath = "";
        pendingPackageVersion = "";
        pendingPackageRootHash = "";
        pendingAllowIdentityRotation = false;
    }

    function beginRuntimeAction(action, modulesDir, binaryPath) {
        pendingAction = String(action || "");
        pendingNode = "";
        pendingNetworkId = "";
        pendingWorkspace = "";
        const requestedModulesDir = String(modulesDir || "").trim();
        pendingRuntimeModulesDir = requestedModulesDir.length
            ? requestedModulesDir : runtimeModulesDir();
        pendingRuntimeBinaryPath = String(binaryPath || "").trim();
        pendingPackageVersion = "";
        pendingPackageRootHash = "";
        pendingAllowIdentityRotation = pendingAction === "start_runtime"
            || pendingAction === "stop_runtime";
    }

    function clearActionDraft() {
        pendingAction = "";
        pendingNode = "";
        pendingNetworkId = "";
        pendingWorkspace = "";
        pendingRuntimeModulesDir = "";
        pendingRuntimeBinaryPath = "";
        pendingPackageVersion = "";
        pendingPackageRootHash = "";
        pendingAllowIdentityRotation = false;
    }

    function runPendingAction() {
        if (!pendingAction.length) {
            return null;
        }
        const action = pendingAction;
        const node = pendingNode;
        const networkId = pendingNetworkId;
        const workspacePath = pendingWorkspace;
        const runtimeModulesDir = pendingRuntimeModulesDir;
        const runtimeBinaryPath = pendingRuntimeBinaryPath;
        const packageVersion = pendingPackageVersion;
        const packageRootHash = pendingPackageRootHash;
        const allowIdentityRotation = pendingAllowIdentityRotation;
        const label = actionDraftTitle();
        clearActionDraft();
        return runAction(action, node, networkId, workspacePath, label, runtimeModulesDir, runtimeBinaryPath, packageVersion, packageRootHash, allowIdentityRotation);
    }

    function actionDraftTitle() {
        if (!pendingAction.length) {
            return qsTr("Confirm");
        }
        if (pendingAction === "install" && pendingNode === "indexer"
                && pendingPackageVersion.length) {
            return qsTr("Install Indexer %1").arg(pendingPackageVersion);
        }
        if (pendingNode.length) {
            return qsTr("%1 %2").arg(actionLabel(pendingAction)).arg(nodeLabel(pendingNode));
        }
        return actionLabel(pendingAction);
    }

    function actionDraftMessage() {
        const action = pendingAction;
        if (action === "delete_network") {
            return qsTr("This stops all local nodes in Local Devnet %1 and removes the managed workspace plus node data.").arg(pendingNetworkId.length ? pendingNetworkId : activeNetworkId());
        }
        if (action === "reset_network") {
            return qsTr("This stops all local nodes in Local Devnet %1, deletes node databases, and regenerates configs in the same workspace.").arg(pendingNetworkId.length ? pendingNetworkId : activeNetworkId());
        }
        if (action === "new_network") {
            const target = pendingNetworkId.length ? pendingNetworkId : qsTr("a generated Local Devnet");
            return qsTr("This creates %1 under the Managed Workspace Root and sets it as Active Devnet.").arg(target);
        }
        if (action === "load_network") {
            return qsTr("This loads the Local Devnet manifest from %1 and sets it as Active Devnet.").arg(pendingWorkspace);
        }
        if (action === "start_runtime") {
            return qsTr("This starts an Inspector-managed LogosCore runtime using modules from %1. If it must replace a stopped runtime with remaining module contexts, Inspector first verifies persisted Messaging peer identity. A legacy Messaging context without one will use a new Peer ID after the next Initialize; this one-time rotation is unavoidable, and later lifecycle cycles preserve that identity.").arg(pendingRuntimeModulesDir.length ? pendingRuntimeModulesDir : runtimeModulesDir());
        }
        if (action === "stop_runtime") {
            return qsTr("This stops only the Inspector-managed LogosCore runtime and clears its module contexts. Inspector first verifies persisted Messaging peer identity. A legacy Messaging context without one will use a new Peer ID after the next Initialize; this one-time rotation is unavoidable, and later lifecycle cycles preserve that identity.");
        }
        const node = nodeByKind(pendingNode) || {};
        if (action === "purge") {
            return qsTr("This stops %1 and deletes data directory %2. Config and install record remain.").arg(nodeLabel(pendingNode)).arg(String(node.data_dir || "-"));
        }
        if (action === "uninstall") {
            return qsTr("This stops %1 and removes its install registration. Node databases remain.").arg(nodeLabel(pendingNode));
        }
        if (action === "start") {
            return qsTr("This starts %1 using config %2.").arg(nodeLabel(pendingNode)).arg(String(node.config_path || "-"));
        }
        if (action === "stop" && pendingNode === "messaging") {
            return qsTr("This stops Messaging by unloading its Delivery context. Inspector first verifies a persisted peer identity. A legacy node without one will use a new Peer ID after the next Initialize; this one-time rotation is unavoidable, and later lifecycle cycles preserve that identity. Its data and config remain, but you must initialize Messaging before starting it again.");
        }
        if (action === "stop") {
            return qsTr("This stops %1 and keeps its data and config.").arg(nodeLabel(pendingNode));
        }
        if (action === "install" && pendingNode === "indexer") {
            const version = pendingPackageVersion.length
                ? pendingPackageVersion : qsTr("selected release");
            const rootHash = pendingPackageRootHash.length
                ? pendingPackageRootHash : qsTr("catalog root hash");
            const modulesDir = pendingRuntimeModulesDir.length
                ? pendingRuntimeModulesDir : runtimeModulesDir();
            return qsTr("This downloads official lez_indexer_module %1, verifies root hash %2, and installs it into %3. LogosCore Runtime must be stopped. After installation, start the runtime, then use Zone Sources to start the selected Channel Indexer.")
                .arg(version).arg(rootHash).arg(modulesDir);
        }
        if (action === "install") {
            return qsTr("This verifies %1 control tooling and records the resolved install path. It does not start the node.").arg(nodeLabel(pendingNode));
        }
        if (action === "initialize") {
            return qsTr("This initializes %1 using config %2. It creates a module context but does not start the node.").arg(nodeLabel(pendingNode)).arg(String(node.config_path || "-"));
        }
        return qsTr("Run local node action.");
    }

    function activeNetworkId() {
        const reportValue = report || null;
        return String(reportValue && reportValue.active_devnet ? reportValue.active_devnet : "");
    }

    function nodeLabel(kind) {
        switch (String(kind || "")) {
        case "bedrock":
            return qsTr("Bedrock");
        case "sequencer":
            return qsTr("Local Sequencer");
        case "indexer":
            return qsTr("Indexer");
        case "storage":
            return qsTr("Storage");
        case "messaging":
            return qsTr("Messaging");
        default:
            return String(kind || "-");
        }
    }

    function nodeByKind(kind) {
        const nodes = revision >= 0 && report && Array.isArray(report.nodes) ? report.nodes : [];
        const key = String(kind || "");
        for (let i = 0; i < nodes.length; ++i) {
            if (String(nodes[i].key || nodes[i].kind || "") === key) {
                return nodes[i];
            }
        }
        return null;
    }

    function actionAvailable(kind, action) {
        const node = nodeByKind(kind);
        const actions = node && Array.isArray(node.available_actions) ? node.available_actions : [];
        return actions.indexOf(String(action || "")) >= 0;
    }

    function actionEnabled(kind, action) {
        return actionAvailable(kind, action) && busy !== true;
    }

    function networkActionEnabled(action) {
        if (busy) {
            return false;
        }
        const key = String(action || "");
        const actions = networkActions();
        return actions.indexOf(key) >= 0;
    }

    function networkActions() {
        const reportValue = report || null;
        if (reportValue && Array.isArray(reportValue.available_network_actions)) {
            return reportValue.available_network_actions;
        }
        if (!localMode()) {
            return [];
        }
        const actions = ["new_network", "load_network"];
        if (reportValue && String(reportValue.active_devnet || "").length > 0) {
            actions.push("reset_network");
            actions.push("delete_network");
        }
        return actions;
    }

    function runtimeActionEnabled(action) {
        if (busy) {
            return false;
        }
        const reportValue = report || null;
        const actions = reportValue && Array.isArray(reportValue.available_runtime_actions) ? reportValue.available_runtime_actions : [];
        return actions.indexOf(String(action || "")) >= 0;
    }

    function runtimeInfo() {
        const reportValue = report || null;
        return reportValue && reportValue.runtime ? reportValue.runtime : null;
    }

    function runtimeModulesDir() {
        const runtime = runtimeInfo();
        const configured = String(runtime && runtime.modules_dir ? runtime.modules_dir : "").trim();
        return configured.length ? configured : defaultRuntimeModulesDir;
    }

    function packageCatalogModulesDir() {
        const value = packageCatalog || null;
        const configured = String(value && value.modules_dir ? value.modules_dir : "").trim();
        return configured.length ? configured : runtimeModulesDir();
    }

    function packageName() {
        const packageValue = packageCatalog && packageCatalog.package
            ? packageCatalog.package : null;
        return String(packageValue && packageValue.name
            ? packageValue.name : "lez_indexer_module");
    }

    function packageReleases() {
        const packageValue = packageCatalog && packageCatalog.package
            ? packageCatalog.package : null;
        return packageValue && Array.isArray(packageValue.versions)
            ? packageValue.versions : [];
    }

    function packageRelease(version, rootHash) {
        const selectedVersion = String(version || "");
        const selectedRootHash = String(rootHash || "");
        if (!selectedVersion.length || !selectedRootHash.length) {
            return null;
        }
        const releases = packageReleases();
        for (let i = 0; i < releases.length; ++i) {
            if (String(releases[i].version || "") === selectedVersion
                    && String(releases[i].root_hash || "") === selectedRootHash) {
                return releases[i];
            }
        }
        return null;
    }

    function installedPackage() {
        const value = packageCatalog || null;
        return value && value.installed ? value.installed : null;
    }

    function defaultPackageSelection() {
        const installed = installedPackage();
        const installedVersion = String(installed && installed.version
            ? installed.version : "");
        const installedRootHash = String(installed && installed.root_hash
            ? installed.root_hash : "");
        const installedRelease = packageRelease(installedVersion, installedRootHash);
        if (installedRelease) {
            return {
                version: installedVersion,
                root_hash: installedRootHash
            };
        }
        const releases = packageReleases();
        for (let i = 0; i < releases.length; ++i) {
            const version = String(releases[i].version || "");
            const rootHash = String(releases[i].root_hash || "");
            if (version.length && rootHash.length) {
                return {
                    version: version,
                    root_hash: rootHash
                };
            }
        }
        return {
            version: "",
            root_hash: ""
        };
    }

    function runtimeState() {
        const runtime = runtimeInfo();
        return String(runtime && runtime.run_state ? runtime.run_state : "not_configured");
    }

    function runtimeDiagnosticsReady(kind) {
        const reportValue = report || null
        if (!reportValue) {
            return false
        }
        const runtime = reportValue.runtime || null
        if (!runtime || String(runtime.ownership || "") !== "inspector_managed") {
            return true
        }
        if (String(runtime.run_state || "") !== "running") {
            return false
        }
        const node = nodeByKind(kind)
        return String(node && node.run_state || "") === "running"
    }

    function localMode() {
        const reportValue = report || null;
        if (!reportValue) {
            return false;
        }
        const mode = reportValue ? String(reportValue.mode || "") : "";
        if (mode.length) {
            return mode === "localnet";
        }
        const profile = reportValue ? String(reportValue.profile || "") : "";
        if (profile.length) {
            return profile === "local";
        }
        return false;
    }

    function modeLabel() {
        const mode = String(report && report.mode || "")
        if (mode === "public_testnet") {
            return qsTr("Testnet")
        }
        return localMode() ? qsTr("Local Devnet") : qsTr("External Network");
    }

    function publicTestnetMode() {
        return String(report && report.mode || "") === "public_testnet"
    }

    function observedNode(kind) {
        const key = String(kind || "")
        const values = observedNodes && typeof observedNodes === "object"
            ? observedNodes : ({})
        return values[key] || null
    }

    function observedRunState(kind) {
        const key = String(kind || "")
        const observation = observedNode(key)
        const status = String(observation && observation.status || "unknown").toLowerCase()
        if (key === "indexer" && observation
                && Array.isArray(observation.channels) && observation.channels.length > 0) {
            return channelIndexerObservedRunState(observation.channels)
        }
        const lifecycleState = managedLifecycleRunState(key)
        if (lifecycleState.length) {
            return lifecycleState
        }
        const runtimeState = indexerRuntimeRunState(observation && observation.indexer_state)
        if (key === "indexer" && runtimeState.length) {
            return runtimeState
        }
        const reachable = status === "healthy" || status === "ready"
            || status === "reachable" || status === "online"
        if (reachable) {
            return "online"
        }
        if (status === "syncing" || status === "degraded" || status === "backfilling") {
            return "syncing"
        }
        if (status === "unavailable" || status === "unreachable" || status === "failed"
                || status === "offline") {
            return "unavailable"
        }
        return "unknown"
    }

    function channelIndexerObservedRunState(channels) {
        const rows = Array.isArray(channels) ? channels : []
        let unresolved = false
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i] || ({})
            const status = String(row.status || "unknown").toLowerCase()
            if (status === "unavailable" || status === "unreachable"
                    || status === "failed" || status === "offline") {
                return "unavailable"
            }
            if (status === "syncing" || status === "degraded"
                    || status === "stale" || status === "backfilling") {
                return "syncing"
            }
            if (status !== "healthy" && status !== "ready"
                    && status !== "reachable" && status !== "online") {
                unresolved = true
                continue
            }
            const runtimeState = indexerRuntimeRunState(row.indexer_state)
            if (runtimeState.length) {
                if (runtimeState !== "online") {
                    return runtimeState
                }
                continue
            }
        }
        return unresolved ? "unknown" : "online"
    }

    function indexerRuntimeRunState(value) {
        switch (String(value || "").toLowerCase()) {
        case "running":
        case "caught_up":
            return "online"
        case "starting":
        case "syncing":
            return "syncing"
        case "stopped":
        case "error":
        case "failed":
        case "stalled":
        case "unavailable":
        case "offline":
            return "unavailable"
        default:
            return ""
        }
    }

    function managedLifecycleRunState(kind) {
        const node = nodeByKind(kind)
        if (!node || controlState(node) !== "managed") {
            return ""
        }
        switch (String(node.run_state || "unknown")) {
        case "running":
        case "caught_up":
            return "online"
        case "initializing":
        case "starting":
        case "stopping":
            return String(node.run_state)
        case "syncing":
            return "syncing"
        case "stopped":
        case "not_initialized":
        case "failed":
        case "error":
        case "stalled":
        case "stale_pid":
            return "unavailable"
        default:
            return "unknown"
        }
    }

    function observedSummary() {
        const nodes = report && Array.isArray(report.nodes) ? report.nodes : []
        const summary = { total: 0, online: 0, syncing: 0, unavailable: 0, unknown: 0 }
        for (let i = 0; i < nodes.length; ++i) {
            const node = nodes[i] || ({})
            const key = String(node.key || node.kind || "")
            const observation = observedNode(key)
            const channels = key === "indexer" && observation
                && Array.isArray(observation.channels) ? observation.channels : []
            if (channels.length > 0) {
                for (let channelIndex = 0; channelIndex < channels.length; ++channelIndex) {
                    recordObservedSummaryState(summary,
                        channelIndexerObservedRunState([channels[channelIndex]]))
                }
            } else {
                recordObservedSummaryState(summary, observedRunState(key))
            }
        }
        return summary
    }

    function recordObservedSummaryState(summary, state) {
        summary.total += 1
        if (state === "online") {
            summary.online += 1
        } else if (state === "syncing" || state === "initializing"
                || state === "starting" || state === "stopping") {
            summary.syncing += 1
        } else if (state === "unavailable") {
            summary.unavailable += 1
        } else {
            summary.unknown += 1
        }
    }

    function controlState(node) {
        const value = node || ({})
        if (localMode()) {
            return String(value.install_state || "needs_configuration")
        }
        const ownership = String(value.ownership || "")
        if (ownership.length) {
            return ownership === "inspector_managed" ? "managed" : "external"
        }
        if (String(value.install_state || "") === "installed") {
            return "managed"
        }
        return "external"
    }

    function summaryTone() {
        if (publicTestnetMode()) {
            const observed = observedSummary()
            if (observed.total > 0 && observed.online === observed.total) {
                return "success"
            }
            return observed.unavailable > 0 ? "error" : "warning"
        }
        const summary = report && report.summary ? report.summary : null
        if (!summary || Number(summary.needs_configuration || 0) > 0) {
            return "warning"
        }
        return Number(summary.running || 0) > 0 ? "success" : "neutral"
    }

    function summaryText() {
        if (publicTestnetMode()) {
            const observed = observedSummary()
            return qsTr("%1/%2 online").arg(observed.online).arg(observed.total)
        }
        const summary = report && report.summary ? report.summary : null;
        if (!summary) {
            return qsTr("Not loaded");
        }
        return qsTr("%1/%2 running").arg(Number(summary.running || 0)).arg(Number(summary.total || 0));
    }

    function toolProblem() {
        const reportValue = report || null;
        const problem = reportValue ? String(reportValue.primary_problem || "") : "";
        if (problem === "missing_logoscore") {
            return qsTr("logoscore not found. Module-backed node actions will report needs_configuration.");
        }
        if (problem === "missing_sequencer_binary") {
            return qsTr("sequencer_service not found. Local sequencer start requires a configured binary.");
        }
        return "";
    }
}
