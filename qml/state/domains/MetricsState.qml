import QtQml
import "../../services/BridgeHelpers.js" as BridgeHelpers
import "../chain/BlockchainRangeValidation.js" as BlockchainRangeValidation
import "../chain/ChainPageQuery.js" as ChainPageQuery
import "../metrics/AppModelMetrics.js" as AppModelMetrics
import "../network/AppModelNetwork.js" as AppModelNetwork

QtObject {
    id: root

    required property var gateway
    required property var sourceRouting
    required property string inspectorModule
    required property string nodeUrl
    required property int storageRollingWindow
    required property int messagingRollingWindow
    required property var dashboardOverview
    required property var dashboardNode
    required property var dashboardL1Blocks
    required property int dashboardL1BlocksSlotTo
    required property var dashboardBlocks
    required property var dashboardProvisionalBlocks

    property int blockchainRefreshRate: 30
    property int messagingRefreshRate: 30
    property int storageRefreshRate: 30
    property int observationTimeoutMs: 45000
    property var networkConnectionStatus: ({})
    property int networkConnectionStatusRevision: 0
    property var networkConnectionPending: ({})
    property int networkConnectionPendingRevision: 0
    property var footerFieldSelections: AppModelMetrics.defaultFooterFieldSelections(root)
    property int footerFieldRevision: 0
    property var dashboardGraphSelections: AppModelMetrics.defaultDashboardGraphSelections(root)
    property int dashboardGraphRevision: 0
    property var dashboardMetricHistory: ({})
    property var dashboardMetricLastSeen: ({})
    property int dashboardMetricHistoryRevision: 0
    property int dashboardSnapshotRevision: 0
    property bool dashboardRefreshing: false
    property int dashboardRefreshSerial: 0
    property string dashboardError: ""
    property var blockchainSourceReport: null
    property var blockchainModuleReport: null
    property var storageModuleReport: null
    property var messagingModuleReport: null
    property var storageSourceReport: null
    property var messagingSourceReport: null

    property var observationConfigurationGenerations: ({
        blockchain: 0,
        storage: 0,
        messaging: 0
    })
    property var observationRequestSequences: ({
        blockchain: 0,
        storage: 0,
        messaging: 0
    })
    property var activeObservationLeases: ({})
    property var observationWaiters: ({})
    property var observationAttempts: ({})
    property var observationReportProvenance: ({})
    property var observationReportRequestIdentities: ({})
    property var observationReportRevisions: ({
        blockchain: 0,
        storage: 0,
        messaging: 0
    })
    property var moduleReportRevisions: ({
        blockchain: 0,
        storage: 0,
        messaging: 0
    })
    property var observationStatusRevisions: ({
        blockchain: 0,
        storage: 0,
        messaging: 0
    })
    property int observationRevision: 0

    property Timer observationTimeoutTimer: Timer {
        interval: Math.max(1, Math.min(root.observationTimeoutMs, 1000))
        repeat: true
        running: Object.keys(root.activeObservationLeases).length > 0
        onTriggered: root.expireTimedOutObservations()
    }

    function refreshInterval(seconds) {
        return AppModelNetwork.refreshInterval(root, seconds)
    }

    function dashboardRefreshInterval() {
        return AppModelNetwork.dashboardRefreshInterval(root)
    }

    function canonicalRefreshRate(seconds) {
        return AppModelNetwork.canonicalRefreshRate(root, seconds)
    }

    function networkConnectionRate(kind) {
        return AppModelNetwork.networkConnectionRate(root, kind)
    }

    function setNetworkConnectionRate(kind, seconds) {
        return AppModelNetwork.setNetworkConnectionRate(root, kind, seconds)
    }

    function knownObservationKind(kind) {
        const target = String(kind || "")
        return target === "blockchain" || target === "storage" || target === "messaging"
    }

    function familyConfigurationGeneration(kind) {
        return Number(observationConfigurationGenerations[String(kind || "")] || 0)
    }

    function nextFamilyRequestSequence(kind) {
        const target = String(kind || "")
        const next = copyMap(observationRequestSequences)
        const sequence = Number(next[target] || 0) + 1
        next[target] = sequence
        observationRequestSequences = next
        return sequence
    }

    function observationLeaseCurrent(lease) {
        if (!lease || !knownObservationKind(lease.kind)) {
            return false
        }
        const current = activeObservationLeases[lease.kind]
        return current
            && Number(current.sequence || 0) === Number(lease.sequence || 0)
            && Number(current.configurationGeneration || 0)
                === Number(lease.configurationGeneration || 0)
            && familyConfigurationGeneration(lease.kind)
                === Number(lease.configurationGeneration || 0)
    }

    function beginObservation(kind, origin, requestKey, requestBaseKey,
            sensitiveProbe, storageCid, runtimeDiagnosticsEnabled,
            runtimeDiagnosticsReduced, interactive, waiter) {
        const target = String(kind || "")
        const lease = {
            kind: target,
            configurationGeneration: familyConfigurationGeneration(target),
            sequence: nextFamilyRequestSequence(target),
            origin: String(origin || "manual"),
            requestKey: String(requestKey || ""),
            requestBaseKey: String(requestBaseKey || requestKey || ""),
            sensitiveProbe: sensitiveProbe === true,
            storageCid: target === "storage" ? String(storageCid || "").trim() : "",
            runtimeDiagnosticsEnabled: runtimeDiagnosticsEnabled === true,
            runtimeDiagnosticsReduced: runtimeDiagnosticsReduced === true,
            interactive: interactive === true,
            deadlineMs: Date.now() + Math.max(1, root.observationTimeoutMs)
        }
        const leases = copyMap(activeObservationLeases)
        leases[target] = lease
        activeObservationLeases = leases
        addObservationWaiter(target, waiter)
        setNetworkConnectionPending(target, true)
        return lease
    }

    function observationWaiter(callback, showResult, label, owner) {
        let presentation = null
        if (showResult === true && gateway
                && typeof gateway.beginObservationPresentation === "function") {
            presentation = gateway.beginObservationPresentation(
                String(label || ""), String(owner || ""))
        }
        if (typeof callback !== "function" && !presentation) {
            return null
        }
        return {
            callback: typeof callback === "function" ? callback : null,
            presentation: presentation,
            label: String(label || ""),
            owner: String(owner || "")
        }
    }

    function addObservationWaiter(kind, waiter) {
        if (!waiter) {
            return
        }
        const target = String(kind || "")
        const next = copyMap(observationWaiters)
        const waiters = Array.isArray(next[target]) ? next[target].slice(0) : []
        waiters.push(waiter)
        next[target] = waiters
        observationWaiters = next
    }

    function completeObservationPresentation(waiter, response) {
        if (!waiter || !waiter.presentation || !gateway
                || typeof gateway.completeObservationPresentation !== "function") {
            return false
        }
        const ok = response && response.ok === true
        const value = ok
            ? observationPresentationValue(waiter, response.value) : null
        return gateway.completeObservationPresentation(
            waiter.presentation,
            waiter.label,
            ok ? BridgeHelpers.formatValue(value)
               : String(response && response.error || qsTr("Source observation failed.")),
            !ok,
            value
        )
    }

    function observationPresentationValue(waiter, value) {
        return waiter && waiter.owner === "storage"
            ? storageObservationSummary(value) : value
    }

    function storageObservationSummary(value) {
        const report = value && typeof value === "object" ? value : ({})
        const health = report.health && typeof report.health === "object"
            ? report.health : ({})
        const probes = Array.isArray(report.probes) ? report.probes
            : (Array.isArray(report.probe_facts) ? report.probe_facts : [])
        let successful = 0
        for (let i = 0; i < probes.length; ++i) {
            if (probes[i] && probes[i].ok === true) {
                successful += 1
            }
        }
        return {
            source: sourceRouting.storageSourceLabel(),
            module: String(report.module || "storage"),
            status: String(health.status || health.summary
                || (health.ready === true ? "healthy" : "unknown")),
            ready: health.ready === true,
            probes: probes.length,
            successful_probes: successful,
            failed_probes: probes.length - successful
        }
    }

    function notifyObservationWaiters(kind, response, snapshot) {
        const target = String(kind || "")
        const next = copyMap(observationWaiters)
        const waiters = Array.isArray(next[target]) ? next[target].slice(0) : []
        delete next[target]
        observationWaiters = next
        for (let i = 0; i < waiters.length; ++i) {
            const waiter = waiters[i]
            completeObservationPresentation(waiter, response)
            if (waiter && typeof waiter.callback === "function") {
                waiter.callback(response, snapshot)
            }
        }
    }

    function queryNetworkConnection(kind, showResult, includeSensitiveProbe, origin) {
        return observeNetworkConnection(
            kind,
            showResult === true,
            includeSensitiveProbe === true,
            null,
            String(origin || "manual")
        )
    }

    function observeNetworkConnection(kind, showResult, includeSensitiveProbe, callback, origin) {
        const target = String(kind || "")
        const requestOrigin = String(origin || "manual")
        const request = networkConnectionRequest(
            target,
            includeSensitiveProbe === true,
            requestOrigin
        )
        if (!request) {
            return {
                ok: false,
                text: "",
                error: qsTr("Unknown connection.")
            }
        }

        const requestKey = JSON.stringify([
            request.method,
            request.args
        ])
        const requestBaseKey = observationRequestBaseKey(target, request)
        const sensitiveProbe = observationRequestIncludesSensitiveProbe(target, request)
        const storageCid = observationRequestStorageCid(target, request)
        const runtimeDiagnosticsEnabled =
            observationRequestIncludesRuntimeDiagnostics(target, request)
        const runtimeDiagnosticsReduced =
            request.runtimeDiagnosticsReduced === true
        const interactive = showResult === true
        const waiter = observationWaiter(
            callback, interactive, request.label, target)
        if (networkConnectionIsPending(target)) {
            const active = activeObservationLeases[target]
            if (observationRequestCompatible(
                    active,
                    requestKey,
                    requestBaseKey,
                    sensitiveProbe,
                    runtimeDiagnosticsEnabled)) {
                if (interactive) {
                    promoteObservationInteractive(target)
                }
                addObservationWaiter(target, waiter)
                return {
                    ok: true,
                    pending: true,
                    joined: true,
                    text: "",
                    error: ""
                }
            }
            if (active && active.runtimeDiagnosticsEnabled === true
                    && runtimeDiagnosticsEnabled !== true) {
                const response = {
                    ok: false,
                    pending: true,
                    skipped: true,
                    text: "",
                    error: qsTr("A full source observation is already pending.")
                }
                if (waiter) {
                    completeObservationPresentation(waiter, response)
                    if (typeof waiter.callback === "function") {
                        waiter.callback(response, sourceObservation(target))
                    }
                }
                return response
            } else if (observationRequestUpgrade(
                    active,
                    requestKey,
                    requestBaseKey,
                    sensitiveProbe,
                    runtimeDiagnosticsEnabled)) {
                supersedeObservation(target, true)
            } else if (active && active.interactive === true && !interactive) {
                const response = {
                    ok: false,
                    pending: true,
                    skipped: true,
                    text: "",
                    error: qsTr("An interactive source observation is already pending.")
                }
                if (waiter) {
                    completeObservationPresentation(waiter, response)
                    if (typeof waiter.callback === "function") {
                        waiter.callback(response, sourceObservation(target))
                    }
                }
                return response
            } else {
                supersedeObservation(target, false)
            }
        }

        const lease = beginObservation(
            target,
            requestOrigin,
            requestKey,
            requestBaseKey,
            sensitiveProbe,
            storageCid,
            runtimeDiagnosticsEnabled,
            runtimeDiagnosticsReduced,
            interactive,
            waiter
        )
        const complete = function (response) {
            root.completeObservation(lease, response)
            return false
        }
        let dispatch = null
        if (target === "blockchain") {
            dispatch = gateway.startBlockchainObservation(
                false,
                request,
                complete
            )
        } else {
            dispatch = gateway.requestModuleAsyncUnobserved(
                request.module,
                request.method,
                request.args,
                request.label,
                false,
                complete,
                function () { return root.observationLeaseCurrent(lease) }
            )
        }
        if (observationLeaseCurrent(lease) && (dispatch === false || dispatch === null)) {
            completeObservation(lease, {
                ok: false,
                text: "",
                error: qsTr("Connection request was not admitted.")
            })
        }
        return dispatch
    }

    function promoteObservationInteractive(kind) {
        const target = String(kind || "")
        const current = activeObservationLeases[target]
        if (!current || current.interactive === true) {
            return false
        }
        const promoted = copyMap(current)
        promoted.interactive = true
        const leases = copyMap(activeObservationLeases)
        leases[target] = promoted
        activeObservationLeases = leases
        return true
    }

    function observationRequestBaseKey(kind, request) {
        const target = String(kind || "")
        const args = request && Array.isArray(request.args)
            ? request.args.slice(0) : []
        if ((target === "storage" || target === "messaging") && args.length > 0
                && args[0] && typeof args[0] === "object") {
            const head = copyMap(args[0])
            if (target === "storage" && head.inputs
                    && typeof head.inputs === "object") {
                const inputs = copyMap(head.inputs)
                delete inputs.include_sensitive_probe
                head.inputs = inputs
            }
            if (head.options && typeof head.options === "object") {
                const options = copyMap(head.options)
                delete options.cid
                delete options.runtime_diagnostics_enabled
                head.options = options
            }
            delete head.include_sensitive_probe
            args[0] = head
        }
        return JSON.stringify([
            String(request && request.method || ""),
            args
        ])
    }

    function observationRequestIncludesSensitiveProbe(kind, request) {
        if (String(kind || "") !== "storage" || !request
                || !Array.isArray(request.args) || request.args.length === 0) {
            return false
        }
        const head = request.args[0]
        if (!head || typeof head !== "object") {
            return false
        }
        const cid = head.options && typeof head.options === "object"
            ? String(head.options.cid || "").trim() : ""
        const nestedFlag = head.inputs && typeof head.inputs === "object"
            ? head.inputs.include_sensitive_probe === true : false
        return cid.length > 0 || nestedFlag || head.include_sensitive_probe === true
    }

    function observationRequestStorageCid(kind, request) {
        if (String(kind || "") !== "storage" || !request
                || !Array.isArray(request.args) || request.args.length === 0) {
            return ""
        }
        const head = request.args[0]
        return head && typeof head === "object"
                && head.options && typeof head.options === "object"
            ? String(head.options.cid || "").trim() : ""
    }

    function observationRequestIncludesRuntimeDiagnostics(kind, request) {
        const target = String(kind || "")
        if ((target !== "storage" && target !== "messaging") || !request
                || !Array.isArray(request.args) || request.args.length === 0) {
            return false
        }
        const head = request.args[0]
        return head && typeof head === "object"
            && head.options && typeof head.options === "object"
            && head.options.runtime_diagnostics_enabled === true
    }

    function observationRequestCompatible(active, requestKey, requestBaseKey,
            sensitiveProbe, runtimeDiagnosticsEnabled) {
        if (!active) {
            return false
        }
        if (String(active.requestKey || "") === String(requestKey || "")) {
            return true
        }
        const target = String(active.kind || "")
        return (target === "storage" || target === "messaging")
            && String(active.requestBaseKey || "") === String(requestBaseKey || "")
            && sensitiveProbe !== true
            && (runtimeDiagnosticsEnabled !== true
                || active.runtimeDiagnosticsEnabled === true)
    }

    function observationRequestUpgrade(active, requestKey, requestBaseKey,
            sensitiveProbe, runtimeDiagnosticsEnabled) {
        if (!active) {
            return false
        }
        if (active.sensitiveProbe === true && sensitiveProbe === true
                && String(active.requestKey || "") !== String(requestKey || "")) {
            return false
        }
        const target = String(active.kind || "")
        const sameFamily = (target === "storage" || target === "messaging")
            && String(active.requestBaseKey || "") === String(requestBaseKey || "")
        const requestedDominates =
            (active.sensitiveProbe !== true || sensitiveProbe === true)
            && (active.runtimeDiagnosticsEnabled !== true
                || runtimeDiagnosticsEnabled === true)
        const strictlyStronger =
            (active.sensitiveProbe !== true && sensitiveProbe === true)
            || (active.runtimeDiagnosticsEnabled !== true
                && runtimeDiagnosticsEnabled === true)
        return sameFamily && requestedDominates && strictlyStronger
    }

    function supersedeObservation(kind, retainWaiters) {
        const target = String(kind || "")
        const leases = copyMap(activeObservationLeases)
        delete leases[target]
        activeObservationLeases = leases
        if (retainWaiters === true) {
            return
        }
        notifyObservationWaiters(target, {
            ok: false,
            superseded: true,
            text: "",
            error: qsTr("A newer source observation superseded this request.")
        }, sourceObservation(target))
    }

    function networkConnectionRequest(kind, includeSensitiveProbe, origin) {
        switch (String(kind || "")) {
        case "blockchain":
            return {
                module: inspectorModule,
                method: "blockchainNode",
                args: [],
                label: qsTr("Blockchain node")
            }
        case "messaging":
            return sourceNetworkConnectionRequest(
                "messaging",
                "deliverySourceReport",
                sourceRouting.deliverySourceReportArgs(),
                qsTr("Delivery source"),
                origin
            )
        case "storage":
            return sourceNetworkConnectionRequest(
                "storage",
                "storageSourceReport",
                sourceRouting.storageSourceReportArgs(
                    includeSensitiveProbe === true
                ),
                qsTr("Storage source"),
                origin
            )
        default:
            return null
        }
    }

    function sourceNetworkConnectionRequest(kind, method, args, label, origin) {
        const runtimeDiagnosticsReduced =
            sourceObservationReducesRuntimeDiagnostics(args, origin)
        return {
            module: inspectorModule,
            method: method,
            args: observationArgsWithGeneration(
                kind,
                sourceObservationArgs(args, origin)
            ),
            label: label,
            runtimeDiagnosticsReduced: runtimeDiagnosticsReduced
        }
    }

    function passiveSourceObservation(origin) {
        switch (String(origin || "")) {
        case "scheduler":
        case "dashboard":
        case "module-event":
        case "storage-refresh":
            return true
        default:
            return false
        }
    }

    function sourceObservationArgs(args, origin) {
        const values = Array.isArray(args) ? args.slice(0) : []
        if (!sourceObservationReducesRuntimeDiagnostics(values, origin)) {
            return values
        }
        const request = copyMap(values[0])
        const options = copyMap(request.options)
        options.runtime_diagnostics_enabled = false
        request.options = options
        values[0] = request
        return values
    }

    function sourceObservationReducesRuntimeDiagnostics(args, origin) {
        if (!passiveSourceObservation(origin)
                || !Array.isArray(args)
                || args.length === 0
                || !args[0]
                || typeof args[0] !== "object") {
            return false
        }
        const request = args[0]
        const sourceMode = String(request.source_mode || "")
            .trim()
            .toLowerCase()
        const moduleSource = sourceMode === "module"
            || sourceMode === "logoscore_cli"
            || sourceMode === "logoscore-cli"
        return moduleSource
            && request.options
            && typeof request.options === "object"
            && request.options.runtime_diagnostics_enabled === true
    }

    function observationArgsWithGeneration(kind, args) {
        const values = Array.isArray(args) ? args.slice(0) : []
        if (values.length === 0 || !values[0] || typeof values[0] !== "object") {
            return values
        }
        const request = copyMap(values[0])
        request.configuration_generation = familyConfigurationGeneration(kind)
        values[0] = request
        return values
    }

    function completeObservation(lease, response) {
        if (!observationLeaseCurrent(lease)) {
            return false
        }
        const target = String(lease.kind || "")
        const leases = copyMap(activeObservationLeases)
        delete leases[target]
        activeObservationLeases = leases
        setNetworkConnectionPending(target, false)
        commitObservation(target, response, lease)
        const snapshot = sourceObservation(target)
        notifyObservationWaiters(target, response, snapshot)
        return true
    }

    function expireTimedOutObservations() {
        const now = Date.now()
        const leases = activeObservationLeases || ({})
        for (const kind in leases) {
            const lease = leases[kind]
            if (!lease || Number(lease.deadlineMs || 0) > now) {
                continue
            }
            completeObservation(lease, {
                ok: false,
                text: "",
                error: qsTr("Source observation timed out.")
            })
        }
    }

    function commitObservation(kind, response, lease) {
        const target = String(kind || "")
        const preserveFullReport =
            reducedObservationPreservesFullReport(target, lease)
        const checkedAtMs = Date.now()
        const successfulTransport = response && response.ok === true
        const value = successfulTransport && response.value !== undefined
            ? response.value : null
        const storedReport = sourceReport(target)
        const statusValue = successfulTransport && preserveFullReport
            ? storedReport : value
        const healthy = successfulTransport
            && connectionValueOk(target, statusValue)
        const compatibleStoredReport = storedReport !== null
            && storedReport !== undefined
            && observationReportCompatible(target, lease)
        if (!successfulTransport && storedReport !== null
                && storedReport !== undefined && !compatibleStoredReport) {
            clearObservationReport(target)
        }
        const priorReport = compatibleStoredReport ? storedReport : null
        const attempt = {
            ok: healthy,
            transportOk: successfulTransport,
            error: successfulTransport ? "" : String(response && response.error
                ? response.error : qsTr("No response")),
            checkedAtMs: checkedAtMs,
            configurationGeneration: Number(lease.configurationGeneration || 0),
            requestGeneration: Number(lease.sequence || 0),
            origin: String(lease.origin || "manual"),
            runtimeDiagnosticsReduced:
                lease.runtimeDiagnosticsReduced === true
        }
        const attempts = copyMap(observationAttempts)
        attempts[target] = attempt
        observationAttempts = attempts

        if (successfulTransport && preserveFullReport) {
            return
        }

        if (successfulTransport && !preserveFullReport) {
            cacheObservationValue(target, value, lease, checkedAtMs)
        }

        const nextStatus = copyMap(networkConnectionStatus)
        nextStatus[target] = {
            known: true,
            ok: healthy,
            transportOk: successfulTransport,
            text: healthy ? qsTr("OK") : qsTr("Error"),
            detail: successfulTransport
                ? networkConnectionSummary(target, statusValue)
                : attempt.error,
            value: statusValue,
            checkedAt: new Date(checkedAtMs).toLocaleTimeString(Qt.locale(), "hh:mm:ss"),
            checkedAtMs: checkedAtMs,
            stale: !successfulTransport && priorReport !== null && priorReport !== undefined,
            configurationGeneration: Number(lease.configurationGeneration || 0),
            requestGeneration: Number(lease.sequence || 0),
            origin: String(lease.origin || "manual")
        }
        networkConnectionStatus = nextStatus
        networkConnectionStatusRevision += 1
        incrementStatusRevision(target)
        observationRevision += 1
        recordDashboardSnapshot()
        gateway.refreshCapabilityRegistryIfLoaded()
    }

    function reducedObservationPreservesFullReport(kind, lease) {
        const target = String(kind || "")
        if (!lease || (target !== "storage" && target !== "messaging")
                || lease.runtimeDiagnosticsReduced !== true
                || !passiveSourceObservation(lease.origin)) {
            return false
        }
        const report = sourceReport(target)
        const identity = observationReportRequestIdentities[target] || null
        return report !== null
            && report !== undefined
            && identity !== null
            && identity.runtimeDiagnosticsEnabled === true
            && identity.runtimeDiagnosticsReduced !== true
            && Number(identity.configurationGeneration || 0)
                === Number(lease.configurationGeneration || 0)
    }

    function cacheObservationValue(kind, value, lease, checkedAtMs) {
        const target = String(kind || "")
        if (target === "blockchain") {
            gateway.cacheBlockchainResult("blockchainNode", value)
        }
        setSourceReport(target, value || null, {
            configurationGeneration: Number(lease.configurationGeneration || 0),
            requestGeneration: Number(lease.sequence || 0),
            origin: String(lease.origin || "manual"),
            checkedAtMs: checkedAtMs
        }, lease)
    }

    function observationReportCompatible(kind, lease) {
        const target = String(kind || "")
        const identity = observationReportRequestIdentities[target] || null
        if (!identity || !lease) {
            return false
        }
        return Number(identity.configurationGeneration || 0)
                === Number(lease.configurationGeneration || 0)
            && observationRequestCompatible(
                identity,
                lease.requestKey,
                lease.requestBaseKey,
                lease.sensitiveProbe === true,
                lease.runtimeDiagnosticsEnabled === true
            )
    }

    function setNetworkConnectionPending(kind, pending) {
        const target = String(kind || "")
        const value = pending === true
        if (networkConnectionPending[target] === value) {
            return false
        }
        const next = copyMap(networkConnectionPending)
        if (value) {
            next[target] = true
        } else {
            delete next[target]
        }
        networkConnectionPending = next
        networkConnectionPendingRevision += 1
        observationRevision += 1
        return true
    }

    function networkConnectionIsPending(kind) {
        const revision = networkConnectionPendingRevision
        return networkConnectionPending[String(kind || "")] === true
    }

    function networkConnectionState(kind) {
        return AppModelNetwork.networkConnectionState(root, kind)
    }

    function updateNetworkConnectionStatus(kind, response) {
        const target = String(kind || "")
        if (!knownObservationKind(target) || networkConnectionIsPending(target)) {
            return false
        }
        const lease = {
            kind: target,
            configurationGeneration: familyConfigurationGeneration(target),
            sequence: nextFamilyRequestSequence(target),
            origin: "compatibility"
        }
        commitObservation(target, response, lease)
        return true
    }

    function cacheNetworkConnectionResult(kind, response) {
        if (!response || response.ok !== true) {
            return false
        }
        const target = String(kind || "")
        const lease = {
            configurationGeneration: familyConfigurationGeneration(target),
            sequence: Number(observationRequestSequences[target] || 0),
            origin: "compatibility"
        }
        cacheObservationValue(target, response.value, lease, Date.now())
        return true
    }

    function updateNetworkConnectionStatusForMethod(method, response) {
        const kind = networkConnectionKindForMethod(method)
        return kind.length > 0 ? updateNetworkConnectionStatus(kind, response) : false
    }

    function networkConnectionKindForMethod(method) {
        switch (String(method || "")) {
        case "blockchainNode":
            return "blockchain"
        case "deliverySourceReport":
            return "messaging"
        case "storageSourceReport":
            return "storage"
        default:
            return ""
        }
    }

    function invalidateConfiguration(kind, reason) {
        const target = String(kind || "")
        if (!knownObservationKind(target)) {
            return false
        }
        const generations = copyMap(observationConfigurationGenerations)
        generations[target] = Number(generations[target] || 0) + 1
        observationConfigurationGenerations = generations

        const leases = copyMap(activeObservationLeases)
        delete leases[target]
        activeObservationLeases = leases
        setNetworkConnectionPending(target, false)

        const status = copyMap(networkConnectionStatus)
        if (status[target] !== undefined) {
            delete status[target]
            networkConnectionStatus = status
            networkConnectionStatusRevision += 1
            incrementStatusRevision(target)
        }

        const attempts = copyMap(observationAttempts)
        delete attempts[target]
        observationAttempts = attempts

        clearObservationReport(target)
        if (target === "blockchain") {
            clearDashboardMetricHistoryForPrefixes([
                "bedrock.",
                "lez.",
                "indexer."
            ])
        } else if (target === "storage" || target === "messaging") {
            clearDashboardMetricHistoryForPrefixes([target + "."])
        }
        observationRevision += 1
        notifyObservationWaiters(target, {
            ok: false,
            cancelled: true,
            text: "",
            error: String(reason || qsTr("Source configuration changed."))
        }, sourceObservation(target))
        return true
    }

    function clearObservationReport(kind) {
        const target = String(kind || "")
        if (target === "blockchain") {
            gateway.clearBlockchainObservation()
        }
        setSourceReport(target, null, null, null)
    }

    function incrementStatusRevision(kind) {
        const target = String(kind || "")
        const next = copyMap(observationStatusRevisions)
        next[target] = Number(next[target] || 0) + 1
        observationStatusRevisions = next
    }

    function incrementReportRevision(kind) {
        const target = String(kind || "")
        const next = copyMap(observationReportRevisions)
        next[target] = Number(next[target] || 0) + 1
        observationReportRevisions = next
    }

    function incrementModuleReportRevision(kind) {
        const target = String(kind || "")
        const next = copyMap(moduleReportRevisions)
        next[target] = Number(next[target] || 0) + 1
        moduleReportRevisions = next
    }

    function observationTimeText(value) {
        const milliseconds = Number(value || 0)
        return milliseconds > 0
            ? new Date(milliseconds).toLocaleTimeString(Qt.locale(), "hh:mm:ss")
            : ""
    }

    function sourceObservation(kind) {
        const target = String(kind || "")
        const revision = observationRevision
        const statusRevision = networkConnectionStatusRevision
        const pendingRevision = networkConnectionPendingRevision
        const status = networkConnectionState(target)
        const attempt = observationAttempts[target] || null
        const provenance = observationReportProvenance[target] || null
        return {
            family: target,
            sourceReport: sourceReport(target),
            moduleReport: moduleReport(target),
            latestAttempt: attempt,
            status: status,
            pending: networkConnectionIsPending(target),
            familyConfigGeneration: familyConfigurationGeneration(target),
            requestGeneration: Number(observationRequestSequences[target] || 0),
            reportRevision: Number(observationReportRevisions[target] || 0),
            moduleReportRevision: Number(moduleReportRevisions[target] || 0),
            statusRevision: Number(observationStatusRevisions[target] || 0),
            checkedAt: status && status.checkedAt ? status.checkedAt : "",
            checkedAtMs: status && status.checkedAtMs ? status.checkedAtMs : 0,
            reportCheckedAt: provenance && provenance.checkedAtMs
                ? observationTimeText(provenance.checkedAtMs) : "",
            reportCheckedAtMs: provenance && provenance.checkedAtMs
                ? Number(provenance.checkedAtMs) : 0,
            stale: status && status.stale === true,
            provenance: provenance
        }
    }

    function moduleReport(kind) {
        return AppModelMetrics.moduleReport(root, kind)
    }

    function sourceReport(kind) {
        switch (String(kind || "")) {
        case "blockchain":
            return blockchainSourceReport || null
        case "storage":
            return storageSourceReport || null
        case "messaging":
        case "delivery":
            return messagingSourceReport || null
        default:
            return null
        }
    }

    function observationReport(kind) {
        return sourceReport(kind)
    }

    function observationReportStorageCid(kind) {
        const target = String(kind || "") === "delivery"
            ? "messaging" : String(kind || "")
        const identity = observationReportRequestIdentities[target] || null
        return target === "storage" && identity
            ? String(identity.storageCid || "").trim() : ""
    }

    function setModuleReport(kind, report) {
        const target = String(kind || "")
        if (target === "blockchain") {
            blockchainModuleReport = report || null
        } else if (target === "storage") {
            storageModuleReport = report || null
        } else if (target === "messaging" || target === "delivery") {
            messagingModuleReport = report || null
        } else {
            return false
        }
        incrementModuleReportRevision(target === "delivery" ? "messaging" : target)
        observationRevision += 1
        return true
    }

    function setSourceReport(kind, report, provenance, requestIdentity) {
        const target = String(kind || "") === "delivery" ? "messaging" : String(kind || "")
        if (target === "blockchain") {
            blockchainSourceReport = report || null
        } else if (target === "storage") {
            storageSourceReport = report || null
        } else if (target === "messaging") {
            messagingSourceReport = report || null
        } else {
            return false
        }
        const next = copyMap(observationReportProvenance)
        if (report) {
            next[target] = provenance || {
                configurationGeneration: familyConfigurationGeneration(target),
                requestGeneration: Number(observationRequestSequences[target] || 0),
                origin: "compatibility",
                checkedAtMs: Date.now()
            }
        } else {
            delete next[target]
        }
        observationReportProvenance = next
        const identities = copyMap(observationReportRequestIdentities)
        if (report && requestIdentity) {
            identities[target] = {
                kind: target,
                configurationGeneration: Number(
                    requestIdentity.configurationGeneration || 0),
                requestKey: String(requestIdentity.requestKey || ""),
                requestBaseKey: String(requestIdentity.requestBaseKey
                    || requestIdentity.requestKey || ""),
                sensitiveProbe: requestIdentity.sensitiveProbe === true,
                storageCid: target === "storage"
                    ? String(requestIdentity.storageCid || "").trim() : "",
                runtimeDiagnosticsEnabled:
                    requestIdentity.runtimeDiagnosticsEnabled === true,
                runtimeDiagnosticsReduced:
                    requestIdentity.runtimeDiagnosticsReduced === true
            }
        } else {
            delete identities[target]
        }
        observationReportRequestIdentities = identities
        incrementReportRevision(target)
        observationRevision += 1
        return true
    }

    function cacheResponseValue(method, value) {
        switch (String(method || "")) {
        case "blockchainNode":
        case "blockchainLiveBlocks":
            gateway.cacheBlockchainResult(method, value)
            return true
        case "blockchainModuleReport":
            return setModuleReport("blockchain", value)
        case "storageReport":
            return setModuleReport("storage", value)
        case "deliveryReport":
            return setModuleReport("messaging", value)
        case "storageSourceReport":
            return setSourceReport("storage", value, null)
        case "deliverySourceReport":
            return setSourceReport("messaging", value, null)
        default:
            return false
        }
    }

    function projectResponse(method, response, cacheResult) {
        const targetMethod = String(method || "")
        const kind = networkConnectionKindForMethod(targetMethod)
        if (kind.length > 0) {
            if (networkConnectionIsPending(kind)) {
                return false
            }
            return updateNetworkConnectionStatus(kind, response)
        }
        if (cacheResult !== false && response && response.ok === true) {
            const cached = cacheResponseValue(targetMethod, response.value)
            if (cached) {
                gateway.refreshCapabilityRegistryIfLoaded()
            }
            return cached
        }
        return false
    }

    function refreshDashboard() {
        if (dashboardRefreshing) {
            return false
        }
        const refreshId = dashboardRefreshSerial + 1
        dashboardRefreshSerial = refreshId
        dashboardRefreshing = true
        dashboardError = ""
        gateway.projectZoneDashboard()

        const liveBlocksSupported = sourceRouting.blockchainSupportsCapability(
            "l1.live_blocks.observe")
        let remaining = liveBlocksSupported ? 4 : 3
        let successful = 0
        const errors = []
        const complete = function (response) {
            if (refreshId !== root.dashboardRefreshSerial) {
                return
            }
            if (response && response.ok === true) {
                successful += 1
            } else {
                errors.push(String(response && response.error
                    ? response.error : qsTr("Dashboard request failed.")))
            }
            remaining -= 1
            if (remaining !== 0) {
                return
            }
            gateway.projectZoneDashboard()
            root.dashboardRefreshing = false
            root.dashboardError = errors.join("\n")
            root.recordDashboardSnapshot()
            const value = {
                overview: root.dashboardOverview || null,
                node: root.dashboardNode || null,
                l1Blocks: root.dashboardL1Blocks || [],
                blocks: root.dashboardBlocks || [],
                storage: root.storageSourceReport || null,
                messaging: root.messagingSourceReport || null
            }
            gateway.setDashboardResult(
                successful > 0,
                successful > 0 ? BridgeHelpers.formatValue(value) : root.dashboardError,
                value
            )
        }

        const once = function (callback) {
            let settled = false
            return function (response) {
                if (settled) {
                    return false
                }
                settled = true
                callback(response)
                return false
            }
        }

        const liveComplete = once(complete)
        const blockchainComplete = once(function (response) {
            complete(response)
            if (refreshId !== root.dashboardRefreshSerial || !root.dashboardRefreshing) {
                return
            }
            if (!liveBlocksSupported) {
                return
            }
            if (!response || response.ok !== true) {
                liveComplete({
                    ok: false,
                    text: "",
                    error: qsTr("Latest L1 blocks require current node state.")
                })
                return
            }
            const slotTo = ChainPageQuery.slotTip(response.value, false)
            if (!Number.isSafeInteger(slotTo) || slotTo <= 0) {
                liveComplete({
                    ok: false,
                    text: "",
                    error: qsTr("No L1 tip available for latest blocks.")
                })
                return
            }
            const liveWindow = ChainPageQuery.liveSlotWindow(
                slotTo, 0, BlockchainRangeValidation.maximumSlotCount() - 1)
            const liveRequest = {
                method: "blockchainLiveBlocks",
                args: [liveWindow.slotFrom, liveWindow.slotTo, 5],
                label: qsTr("Latest L1 blocks")
            }
            const liveDispatch = gateway.startDashboardBlockchainOperation(
                liveRequest, function (liveResponse) {
                    if (refreshId === root.dashboardRefreshSerial
                            && liveResponse && liveResponse.ok === true) {
                        gateway.cacheBlockchainResult(
                            liveRequest.method, liveResponse.value, slotTo)
                    }
                    liveComplete(liveResponse)
                    return false
                })
            if (liveDispatch === false || liveDispatch === null) {
                liveComplete({
                    ok: false,
                    text: "",
                    error: qsTr("Latest L1 block request was not admitted.")
                })
            }
        })
        observeNetworkConnection(
            "blockchain", false, false, blockchainComplete, "dashboard")
        observeNetworkConnection(
            "storage", false, false, once(complete), "dashboard")
        observeNetworkConnection(
            "messaging", false, false, once(complete), "dashboard")
        return true
    }

    function invalidateDashboard(reason) {
        dashboardRefreshSerial += 1
        dashboardRefreshing = false
        dashboardError = ""
        gateway.invalidateDashboardOperations(String(reason || qsTr("Dashboard configuration changed.")))
        gateway.resetDashboardProjection()
    }

    function setFooterFieldEnabled(key, enabled) {
        return AppModelNetwork.setFooterFieldEnabled(root, key, enabled)
    }

    function footerFieldEnabled(key) {
        return AppModelNetwork.footerFieldEnabled(root, key)
    }

    function setDashboardGraphEnabled(key, enabled) {
        return AppModelNetwork.setDashboardGraphEnabled(root, key, enabled)
    }

    function dashboardGraphEnabled(key) {
        return AppModelNetwork.dashboardGraphEnabled(root, key)
    }

    function copyMap(source) {
        const next = {}
        const current = source || {}
        for (const key in current) {
            next[key] = current[key]
        }
        return next
    }

    function scalarValue(value) {
        if (value === undefined || value === null || value === "") {
            return null
        }
        if (typeof value === "number" || typeof value === "string" || typeof value === "boolean") {
            return value
        }
        if (Array.isArray(value)) {
            return value.length
        }
        if (typeof value === "object") {
            if (value.result !== undefined && value.result !== null) {
                return scalarValue(value.result)
            }
            if (value.value !== undefined && value.value !== null) {
                return scalarValue(value.value)
            }
            if (value.count !== undefined && value.count !== null) {
                return scalarValue(value.count)
            }
            if (value.total !== undefined && value.total !== null) {
                return scalarValue(value.total)
            }
        }
        return null
    }

    function dashboardGate(key) {
        return gateway.dashboardGate(key)
    }

    function valueText(value) { return AppModelMetrics.valueText(root, value) }
    function valueToString(value) { return AppModelMetrics.valueToString(root, value) }
    function moduleProbe(kind, method) { return AppModelMetrics.moduleProbe(root, kind, method) }
    function moduleProbeValue(kind, method) { return AppModelMetrics.moduleProbeValue(root, kind, method) }
    function sourceProbe(kind, method) { return AppModelMetrics.sourceProbe(root, kind, method) }
    function sourceProbeValue(kind, method) { return AppModelMetrics.sourceProbeValue(root, kind, method) }
    function observationProbeValue(kind, method) { return AppModelMetrics.observationProbeValue(root, kind, method) }
    function moduleProbeError(kind, method) { return AppModelMetrics.moduleProbeError(root, kind, method) }
    function moduleLastError(kind) { return AppModelMetrics.moduleLastError(root, kind) }
    function openMetricsText(kind) { return AppModelMetrics.openMetricsText(root, kind) }
    function openMetricsTextFromValue(value) { return AppModelMetrics.openMetricsTextFromValue(root, value) }
    function openMetricValue(kind, names) { return AppModelMetrics.openMetricValue(root, kind, names) }
    function openMetricLabels(text) { return AppModelMetrics.openMetricLabels(root, text) }
    function metricJsonValue(value, names) { return AppModelMetrics.metricJsonValue(root, value, names) }
    function metricSpecName(spec) { return AppModelMetrics.metricSpecName(root, spec) }
    function metricSpecLabels(spec) { return AppModelMetrics.metricSpecLabels(root, spec) }
    function metricJsonLabels(value) { return AppModelMetrics.metricJsonLabels(root, value) }
    function metricLabelsMatch(actual, wanted) { return AppModelMetrics.metricLabelsMatch(root, actual, wanted) }
    function metricNumber(value) { return AppModelMetrics.metricNumber(root, value) }
    function overviewProbeValue(section, field) { return AppModelMetrics.overviewProbeValue(root, section, field) }
    function indexerHeadValue() { return AppModelMetrics.indexerHeadValue(root) }
    function sequencerHeadValue() { return AppModelMetrics.sequencerHeadValue(root) }
    function nodeProbeValue(name) { return AppModelMetrics.nodeProbeValue(root, name) }
    function cryptarchiaInfo() { return AppModelMetrics.cryptarchiaInfo(root) }
    function cryptarchiaValue(key) { return AppModelMetrics.cryptarchiaValue(root, key) }
    function networkInfo() { return AppModelMetrics.networkInfo(root) }
    function networkValue(key) { return AppModelMetrics.networkValue(root, key) }
    function mantleMetrics() { return AppModelMetrics.mantleMetrics(root) }
    function mantleValue(keys) { return AppModelMetrics.mantleValue(root, keys) }
    function tipMinusLib() { return AppModelMetrics.tipMinusLib(root) }
    function finalityLagSeconds() { return AppModelMetrics.finalityLagSeconds(root) }
    function indexerLag() { return AppModelMetrics.indexerLag(root) }
    function moduleMetricValue(kind, names) { return AppModelMetrics.moduleMetricValue(root, kind, names) }
    function moduleMetricSum(kind, names) { return AppModelMetrics.moduleMetricSum(root, kind, names) }
    function storageManifestCount() { return AppModelMetrics.storageManifestCount(root) }
    function dashboardMetricRawValue(key) { return AppModelMetrics.dashboardMetricRawValue(root, key) }
    function dashboardMetricValue(key) { return AppModelMetrics.dashboardMetricValue(root, key) }
    function dashboardMetricUsesWindow(key) { return AppModelMetrics.dashboardMetricUsesWindow(root, key) }
    function dashboardMetricWindowDelta(key) { return AppModelMetrics.dashboardMetricWindowDelta(root, key) }
    function dashboardMetricWindowMs(key) { return AppModelMetrics.dashboardMetricWindowMs(root, key) }
    function dashboardMetricText(key) { return AppModelMetrics.dashboardMetricText(root, key) }
    function recordDashboardSnapshot() {
        const result = AppModelMetrics.recordDashboardSnapshot(root)
        dashboardSnapshotRevision += 1
        return result
    }
    function dashboardMetricSampleUpdate(stored, lastSeen, now, value) { return AppModelMetrics.dashboardMetricSampleUpdate(root, stored, lastSeen, now, value) }
    function dashboardMetricSamples(key) { return AppModelMetrics.dashboardMetricSamples(root, key) }
    function normalizedDashboardSample(sample) { return AppModelMetrics.normalizedDashboardSample(root, sample) }
    function normalizedDashboardSamples(samples) { return AppModelMetrics.normalizedDashboardSamples(root, samples) }
    function nextDashboardSampleTimestamp(previous, now) { return AppModelMetrics.nextDashboardSampleTimestamp(root, previous, now) }
    function trimDashboardMetricSamples(samples) { return AppModelMetrics.trimDashboardMetricSamples(root, samples) }
    function dashboardMetricWindowSamples(key) { return AppModelMetrics.dashboardMetricWindowSamples(root, key) }
    function windowDeltaFromSamples(samples, timestamp, windowMs) { return AppModelMetrics.windowDeltaFromSamples(root, samples, timestamp, windowMs) }
    function defaultFooterFieldSelections() { return AppModelMetrics.defaultFooterFieldSelections(root) }
    function defaultDashboardGraphSelections() { return AppModelMetrics.defaultDashboardGraphSelections(root) }
    function clearDashboardMetricHistoryForPrefix(prefix) { return AppModelMetrics.clearDashboardMetricHistoryForPrefix(root, prefix) }
    function clearDashboardMetricHistoryForPrefixes(prefixes) { return AppModelMetrics.clearDashboardMetricHistoryForPrefixes(root, prefixes) }

    function networkConnectionSummary(kind, value) { return AppModelNetwork.networkConnectionSummary(root, kind, value) }
    function connectionValueOk(kind, value) { return AppModelNetwork.connectionValueOk(root, kind, value) }
    function storageReportReady(report) { return AppModelNetwork.storageReportReady(root, report) }
    function moduleReportReachable(report) { return AppModelNetwork.moduleReportReachable(root, report) }
    function sourceHealthReady(report) { return AppModelNetwork.sourceHealthReady(root, report) }
    function sourceCapabilityAvailable(report, key) { return AppModelNetwork.sourceCapabilityAvailable(root, report, key) }
    function sourceCapabilityEvidence(report, key) { return AppModelNetwork.sourceCapabilityEvidence(root, report, key) }
    function sourceCapabilityValue(report, key) { return AppModelNetwork.sourceCapabilityValue(root, report, key) }
    function sourceProbeFact(report, key) { return AppModelNetwork.sourceProbeFact(root, report, key) }
    function reportProbeValue(report, method) { return AppModelNetwork.reportProbeValue(root, report, method) }
    function reportProbeOk(report, method) { return AppModelNetwork.reportProbeOk(root, report, method) }
    function reportProbe(report, method) { return AppModelNetwork.reportProbe(root, report, method) }
    function deliveryReportHealthy(report) { return AppModelNetwork.deliveryReportHealthy(root, report) }
    function deliveryHealthValueOk(value, unknownOk) { return AppModelNetwork.deliveryHealthValueOk(root, value, unknownOk) }
    function moduleReportError(report) { return AppModelNetwork.moduleReportError(root, report) }
}
