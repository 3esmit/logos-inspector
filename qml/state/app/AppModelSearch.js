.import "../../services/BridgeHelpers.js" as BridgeHelpers

function refreshDashboard(root) {
    with (root) {
        if (dashboardRefreshing) {
            return
        }
        const refreshId = dashboardRefreshSerial + 1
        const configRevision = networkConfigurationRevision
        dashboardRefreshSerial = refreshId
        dashboardRefreshing = true
        dashboardError = ""
        projectZoneDashboard(root)
        const requests = [
            { module: inspectorModule, method: "blockchainNode", args: root.blockchainArgs([]), label: qsTr("Blockchain node") },
            { module: inspectorModule, method: "blockchainLiveBlocks", args: root.blockchainArgs([0, 9007199254740991, 5]), label: qsTr("Latest L1 blocks") },
            { module: inspectorModule, method: "storageSourceReport", args: root.sourceRouting.storageSourceReportArgs(false), label: qsTr("Storage source") },
            { module: inspectorModule, method: "deliverySourceReport", args: root.sourceRouting.deliverySourceReportArgs(), label: qsTr("Delivery source") }
        ]
        const errors = []
        let remaining = requests.length
        let okCount = 0

        for (let i = 0; i < requests.length; ++i) {
            const request = requests[i]
            requestModuleAsync(request.module, request.method, request.args, request.label, false, function (response) {
                if (refreshId !== dashboardRefreshSerial || configRevision !== networkConfigurationRevision) {
                    return
                }
                if (response.ok) {
                    okCount += 1
                } else {
                    errors.push(response.error)
                }
                if (request.method === "blockchainNode") {
                    root.updateNetworkConnectionStatus("blockchain", response)
                } else if (request.method === "storageSourceReport") {
                    root.updateNetworkConnectionStatus("storage", response)
                } else if (request.method === "deliverySourceReport") {
                    root.updateNetworkConnectionStatus("messaging", response)
                }
                remaining -= 1
                if (remaining === 0) {
                    projectZoneDashboard(root)
                    dashboardRefreshing = false
                    dashboardError = errors.join("\n")
                    root.recordDashboardSnapshot()
                    if (okCount > 0) {
                        shell.setResult(qsTr("Dashboard"), BridgeHelpers.formatValue({
                            overview: dashboardOverview || null,
                            node: dashboardNode || null,
                            l1Blocks: dashboardL1Blocks || [],
                            blocks: dashboardBlocks || [],
                            storage: storageSourceReport || null,
                            messaging: messagingSourceReport || null
                        }), false)
                    } else {
                        shell.setResult(qsTr("Dashboard"), dashboardError, true)
                    }
                }
            }, function () {
                return refreshId === dashboardRefreshSerial && configRevision === networkConfigurationRevision
            })
        }
    }
}

function projectZoneDashboard(root) {
    with (root) {
        const state = zoneInspection
        const context = state ? state.activeZoneContext : null
        if (!state || !context) {
            dashboardOverview = null
            dashboardProvisionalBlocks = []
            dashboardBlocks = []
            dashboardLezBlockRows = []
            return
        }
        const summary = zoneDashboardSummary(state)
        const l2 = summary && summary.l2_zone ? summary.l2_zone : ({})
        const sourceStatus = String(l2.source_status || "unknown")
        const latestBlock = l2.latest_block_id === undefined ? null : l2.latest_block_id
        const finalizedBlock = l2.finalized_block_id === undefined
            ? (l2.safe_block_id === undefined ? null : l2.safe_block_id)
            : l2.finalized_block_id
        const channelId = String(context.channel_id || "")
        const sequencerObservation = zoneSourceObservation(state,
            context.selected_sequencer_source_id, "sequencer")
        const indexerObservation = zoneSourceObservation(state,
            context.indexer_source_id, "indexer")
        const sequencerHealth = sourceHealthProjection(sequencerObservation, sourceStatus)
        const indexerHealth = sourceHealthProjection(indexerObservation, "unknown")
        dashboardOverview = {
            context_revision: Number(context.context_revision || 0),
            network_scope: context.network_scope,
            channel_id: channelId,
            sequencer: {
                health: sequencerHealth,
                head: { ok: latestBlock !== null, value: latestBlock },
                zone_id: { ok: true, value: channelId },
                channel_id: { ok: true, value: channelId }
            },
            indexer: {
                health: indexerHealth,
                head: { ok: finalizedBlock !== null, value: finalizedBlock }
            }
        }
        const projected = zoneDashboardRows(state)
        dashboardLezBlockRows = projected.slice(0, 5)
        dashboardProvisionalBlocks = projected.filter(function (row) {
            return Array.isArray(row.source_roles)
                && row.source_roles.indexOf("sequencer") >= 0
        })
        dashboardBlocks = projected.filter(function (row) {
            return Array.isArray(row.source_roles)
                && row.source_roles.indexOf("indexer") >= 0
        })
        if (!projected.length && latestBlock !== null) {
            const synthetic = {
                block_id: latestBlock,
                header_hash: String(l2.latest_block_hash || ""),
                timestamp: Number(summary && summary.activity_detail
                    && summary.activity_detail.last_seen_unix || 0),
                tx_count: null,
                bedrock_status: String(l2.finality_state || "unknown"),
                source: "summary",
                source_roles: ["sequencer"],
                entity_ref: String(l2.latest_block_hash || "").length > 0
                    ? state.l2.l2EntityRef("block", "block:" + String(latestBlock)
                        + ":" + String(l2.latest_block_hash), null) : null
            }
            dashboardLezBlockRows = [synthetic]
            dashboardProvisionalBlocks = [synthetic]
            if (finalizedBlock !== null && Number(finalizedBlock) === Number(latestBlock)) {
                synthetic.source_roles.push("indexer")
                dashboardBlocks = [synthetic]
            }
        }
    }
}

function zoneSourceObservation(state, sourceId, role) {
    const id = String(sourceId || "")
    const rows = state && state.zoneDetail
        && Array.isArray(state.zoneDetail.source_observations)
        ? state.zoneDetail.source_observations : []
    for (let i = 0; i < rows.length; ++i) {
        const row = rows[i] || ({})
        if (String(row.source_id || "") === id
                && String(row.role || "") === String(role || "")) {
            return row
        }
    }
    return null
}

function sourceHealthProjection(observation, fallback) {
    const health = String(observation && observation.health || fallback || "unknown")
    return { ok: health === "reachable", value: health }
}

function zoneDashboardSummary(state) {
    if (state.zoneDetail && state.zoneDetail.summary) {
        return state.zoneDetail.summary
    }
    const rows = Array.isArray(state.zoneSummaries) ? state.zoneSummaries : []
    for (let i = 0; i < rows.length; ++i) {
        if (String(rows[i] && rows[i].channel_id || "") === String(state.activeZoneId || "")) {
            return rows[i]
        }
    }
    return null
}

function zoneDashboardRows(state) {
    const blockState = state.l2 && state.l2.blocks ? state.l2.blocks : null
    const rows = blockState && Array.isArray(blockState.l2BlockRows)
        ? blockState.l2BlockRows : []
    const result = []
    for (let i = 0; i < rows.length; ++i) {
        const row = rows[i] || ({})
        const summary = row.summary || ({})
        const observations = Array.isArray(row.observations) ? row.observations : []
        const observation = observations.length > 0 ? observations[0] : null
        const sourceRoles = []
        for (let j = 0; j < observations.length; ++j) {
            const role = String(observations[j] && observations[j].source_role || "")
            if (role.length > 0 && sourceRoles.indexOf(role) < 0) {
                sourceRoles.push(role)
            }
        }
        result.push({
            block_id: summary.block_id,
            header_hash: String(summary.block_hash || ""),
            parent_hash: String(summary.parent_hash || ""),
            timestamp: summary.timestamp,
            tx_count: summary.transaction_count,
            bedrock_status: String(summary.bedrock_status || ""),
            source: String(observation && observation.source_role || ""),
            source_roles: sourceRoles,
            entity_ref: state.l2.l2EntityRef("block", "block:" + String(summary.block_id)
                + ":" + String(summary.block_hash || ""), observation)
        })
    }
    return result
}

function updateDashboardCache(root, method, value) {
    with (root) {
        if (method === "blockchainNode") {
            dashboardNode = value
        } else if (method === "blockchainLiveBlocks") {
            dashboardL1Blocks = value && Array.isArray(value.blocks) ? value.blocks : []
        } else if (method === "blockchainModuleReport") {
            blockchainModuleReport = value || null
        } else if (method === "storageReport") {
            storageModuleReport = value || null
        } else if (method === "storageSourceReport") {
            storageSourceReport = value || null
        } else if (method === "deliveryReport") {
            messagingModuleReport = value || null
        } else if (method === "deliverySourceReport") {
            messagingSourceReport = value || null
        }
    }
}

function routeSearch(root, query) {
    with (root) {
        const value = query.trim()
        if (!value.length) {
            return
        }

        const parsedPrefix = searchPrefix(value)
        if (parsedPrefix.prefix.length > 0 && isLocalSearchPrefix(parsedPrefix.prefix)) {
            routePrefixedSearch(value)
            return
        }

        const settingsTarget = settingsTargetForQuery(value)
        if (settingsTarget.section.length > 0) {
            openSettings(settingsTarget.section, settingsTarget.subsection)
            return
        }

        const view = viewKeyForQuery(value)
        if (view.length > 0) {
            selectView(view)
            return
        }

        if (root.isStorageCid(value)) {
            openStorageCid(root, value)
            return
        }

        resolveInspectionTarget(value)
    }
}

function isLocalSearchPrefix(prefix) {
    const value = String(prefix || "").toLowerCase()
    return value === "mantle" || value === "private" || value === "recipient"
        || value === "wallet" || value === "cid" || value === "storage"
        || value === "l1-wallet" || value === "note" || value === "module"
}

function resolveInspectionTarget(root, query) {
    with (root) {
        const value = String(query || "").trim()
        if (!value.length || !zoneInspection) {
            return
        }
        pushNavigationHistory()
        shell.statusText = qsTr("Search")
        zoneInspection.resolveTarget(value, function (report, error) {
            if (!report) {
                shell.setResult(qsTr("Search"), error || qsTr("Target resolution failed."), true, null)
                return
            }
            const candidates = Array.isArray(report.candidates)
                ? report.candidates.slice() : []
            if (/^(0x)?[0-9a-fA-F]{64}$/.test(value)
                    && root.programIdKnown(value) && zoneInspection.activeZoneContext) {
                const canonicalProgram = root.canonicalProgramIdHex(value)
                const localProgramRef = canonicalProgram.length > 0
                    ? zoneInspection.l2.l2EntityRef("program", canonicalProgram, null) : null
                if (localProgramRef) {
                    candidates.push({ entity_ref: Object.assign({ layer: "l2" }, localProgramRef) })
                }
            }
            report.candidates = dedupeInspectionCandidates(candidates)
            zoneInspection.targetResolutionReport = report
            zoneInspection.targetResolutionCandidates = report.candidates
            if (report.candidates.length === 1) {
                openInspectionCandidate(report.candidates[0], false)
                return
            }
            if (report.candidates.length > 1) {
                zoneInspection.targetResolutionStatus = "ambiguous"
                zoneInspection.requestedDetailTab = "overview"
                selectView("zones", false)
                shell.setResult(qsTr("Search candidates"), qsTr("Select one typed candidate."), false, report)
                return
            }
            if (String(report.status || "") === "recovery") {
                zoneInspection.requestedDetailTab = "sources"
                selectView("zones", false)
                shell.setResult(qsTr("Search"), qsTr("Select an Active Zone before resolving this L2 target."), true, report)
                return
            }
            shell.setResult(qsTr("Search"), qsTr("No matching inspection target found."), true, report)
        })
    }
}

function dedupeInspectionCandidates(candidates) {
    const result = []
    const seen = ({})
    for (let i = 0; i < candidates.length; ++i) {
        const candidate = candidates[i] || null
        const entity = candidate && candidate.entity_ref ? candidate.entity_ref : null
        if (!entity) {
            continue
        }
        const key = JSON.stringify(entity)
        if (seen[key] === true) {
            continue
        }
        seen[key] = true
        result.push(candidate)
    }
    return result
}

function openInspectionCandidate(root, candidate, recordHistory) {
    const entity = candidate && candidate.entity_ref ? candidate.entity_ref : candidate
    return openInspectionEntityRef(root, entity, recordHistory)
}

function openInspectionEntityRef(root, entity, recordHistory) {
    with (root) {
        if (!entity || typeof entity !== "object") {
            return false
        }
        if (recordHistory !== false) {
            pushNavigationHistory()
        }
        const layer = String(entity.layer || "")
        if (layer === "zone") {
            if (!inspectionEntityRefMatchesCatalog(root, entity)) {
                shell.setResult(qsTr("Open reference"), qsTr("Stored Zone reference does not match the current network catalog."), true, entity)
                selectView("zones", false)
                return false
            }
            currentInspectionEntityRef = entity
            zoneInspection.requestedDetailTab = "overview"
            selectView("zones", false)
            return zoneInspection.activateZone(String(entity.channel_id || ""))
        }
        if (layer === "l1") {
            if (!inspectionNetworkScopeMatches(root, entity.network_scope)) {
                shell.setResult(qsTr("Open reference"), qsTr("Stored L1 reference belongs to another network."), true, entity)
                return false
            }
            currentInspectionEntityRef = entity
            const target = entity.block_id !== undefined && entity.block_id !== null
                ? entity.block_id : entity.block_hash
            entityNavigation.openBlockchainBlock(target)
            return true
        }
        if (layer !== "l2" || !inspectionEntityRefMatchesCatalog(root, entity)) {
            shell.setResult(qsTr("Open reference"), qsTr("Stored reference does not match current network or Zone catalog."), true, entity)
            selectView("zones", false)
            return false
        }
        const tab = inspectionDetailTab(entity.entity_kind)
        zoneInspection.requestedDetailTab = tab
        zoneInspection.requestedL2View = String(entity.entity_kind || "") === "transaction"
            ? "transaction" : (String(entity.entity_kind || "") === "block" ? "block" : "blocks")
        selectView("zones", false)
        if (!zoneInspection.activeZoneContext
                || String(zoneInspection.activeZoneId || "") !== String(entity.channel_id || "")) {
            pendingInspectionEntityRef = entity
            if (!zoneInspection.activateZone(String(entity.channel_id || ""))) {
                pendingInspectionEntityRef = null
                return false
            }
            return true
        }
        return openInspectionEntityInActiveZone(entity)
    }
}

function resumePendingInspectionEntityRef(root) {
    with (root) {
        const entity = pendingInspectionEntityRef
        if (!entity || !zoneInspection || zoneInspection.detailInFlight
                || zoneInspection.detailStale || !zoneInspection.zoneDetail) {
            return false
        }
        if (String(zoneInspection.activeZoneId || "") !== String(entity.channel_id || "")) {
            return false
        }
        if (!inspectionEntityRefMatchesCatalog(root, entity)) {
            pendingInspectionEntityRef = null
            return false
        }
        pendingInspectionEntityRef = null
        return openInspectionEntityInActiveZone(entity)
    }
}

function inspectionEntityRefMatchesCatalog(root, entity) {
    with (root) {
        if (!zoneInspection || !inspectionNetworkScopeMatches(root, entity.network_scope)) {
            return false
        }
        const rows = Array.isArray(zoneInspection.zoneSummaries)
            ? zoneInspection.zoneSummaries : []
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i] || ({})
            if (String(row.channel_id || "") === String(entity.channel_id || "")
                    && String(row.kind || "unknown") === String(entity.zone_kind || "unknown")) {
                return true
            }
        }
        return false
    }
}

function inspectionNetworkScopeMatches(root, scope) {
    return root.zoneInspection
        && root.zoneInspection.scopeKey(scope)
            === root.zoneInspection.scopeKey(root.zoneInspection.networkScope)
}

function openInspectionEntityInActiveZone(root, entity) {
    with (root) {
        if (!inspectionEntityRefMatchesCatalog(root, entity)) {
            return false
        }
        const source = entity.source && typeof entity.source === "object"
            ? entity.source : ({ kind: "policy" })
        const sourceId = String(source.source_id || "")
        if (String(source.kind || "policy") === "exact") {
            const role = String(source.source_role || "")
            const currentId = role === "indexer" ? zoneInspection.l2.l2IndexerSourceId()
                : (role === "sequencer" ? zoneInspection.l2.l2SequencerSourceId() : "")
            if (!sourceId.length || sourceId !== currentId) {
                zoneInspection.requestedDetailTab = "sources"
                shell.setResult(qsTr("Open reference"), qsTr("Exact source is no longer configured for this Zone."), true, entity)
                return false
            }
        }
        const kind = String(entity.entity_kind || "")
        const key = String(entity.canonical_key || "")
        let opened = false
        if (kind === "block") {
            const block = inspectionBlockTarget(key)
            opened = block ? zoneInspection.l2.blocks.openL2Block(block, sourceId) !== null : false
        } else if (kind === "transaction") {
            opened = zoneInspection.l2.blocks.openL2Transaction(key, sourceId) !== null
        } else if (kind === "account") {
            opened = zoneInspection.l2.accounts.inspectL2AccountReference(key, source)
        } else if (kind === "program") {
            opened = zoneInspection.l2.tools.refreshL2Programs() !== null
        }
        if (opened) {
            currentInspectionEntityRef = entity
        }
        return opened
    }
}

function inspectionBlockTarget(canonicalKey) {
    const parts = String(canonicalKey || "").split(":")
    if (parts.length >= 3 && parts[0] === "block") {
        const blockId = Number(parts[1])
        return Number.isFinite(blockId) ? {
            block_id: blockId,
            block_hash: parts.slice(2).join(":")
        } : null
    }
    if (parts.length === 2 && parts[0] === "block") {
        const blockId = Number(parts[1])
        return Number.isFinite(blockId) ? { block_id: blockId } : null
    }
    return null
}

function inspectionDetailTab(entityKind) {
    const kind = String(entityKind || "")
    if (kind === "account") {
        return "accounts"
    }
    if (kind === "program") {
        return "programs"
    }
    return "l2"
}

function routePrefixedSearch(root, query) {
    with (root) {
        const parsed = searchPrefix(query)
        if (!parsed.prefix.length) {
            return false
        }

        const prefix = parsed.prefix
        const target = parsed.target
        if (prefix === "mantle") {
            if (target.length > 0) {
                entityNavigation.openMantleTransaction(target)
            } else {
                selectView("transactions")
            }
            return true
        }
        if (prefix === "private") {
            openPrivateAccountReference(target.length > 0 && target.indexOf("Private/") !== 0 ? "Private/" + target : target)
            return true
        }
        if (prefix === "wallet") {
            entityNavigation.openLocalWallet(target, "lezAccounts")
            return true
        }
        if (prefix === "cid" || prefix === "storage") {
            if (target.length > 0) {
                openStorageCid(root, target)
            } else {
                selectView("storage")
            }
            return true
        }
        if (prefix === "l1-wallet" || prefix === "note") {
            entityNavigation.openLocalWallet(target, "bedrockNotes")
            return true
        }
        if (prefix === "module") {
            routeModuleSearchTarget(target)
            return true
        }
        return false
    }
}

function searchPrefix(root, query) {
    with (root) {
        const text = String(query || "").trim()
        let match = text.match(/^([A-Za-z][A-Za-z0-9_-]*)\s*:\s*(.*)$/)
        if (match && isSearchPrefix(match[1])) {
            return { prefix: String(match[1]).toLowerCase(), target: String(match[2] || "").trim() }
        }
        match = text.match(/^([A-Za-z][A-Za-z0-9_-]*)\s+(.+)$/)
        if (match && isSearchPrefix(match[1])) {
            return { prefix: String(match[1]).toLowerCase(), target: String(match[2] || "").trim() }
        }
        return { prefix: "", target: "" }
    }
}

function isSearchPrefix(root, prefix) {
    with (root) {
        const value = String(prefix || "").toLowerCase()
        return value === "mantle" || value === "private" || value === "wallet"
            || value === "l1-wallet" || value === "note" || value === "module"
            || value === "cid" || value === "storage"
    }
}

function openStorageCid(root, cid) {
    with (root) {
        const value = String(cid || "").trim()
        if (!value.length) {
            selectView("storage")
            return
        }
        pushNavigationHistory()
        storageCidProbe = value
        storageAppTab = "cid"
        selectView("storage", false)
        shell.setResult(qsTr("Storage CID"), qsTr("Storage CID context: %1").arg(value), false, {
            cid: value,
            source: root.storageSourceTarget()
        })
        if (root.storageSourceTarget().length > 0) {
            root.queryNetworkConnection("storage", false, true)
        }
    }
}

function isStorageCid(root, value) {
    with (root) {
        const text = String(value || "").trim()
        if (text.length < 20 || /\s/.test(text)) {
            return false
        }
        if (/^Qm[1-9A-HJ-NP-Za-km-z]{44}$/.test(text)) {
            return true
        }
        if (/^b[a-z2-7]{20,}$/i.test(text)) {
            return true
        }
        return /^z[1-9A-HJ-NP-Za-km-z]{20,}$/.test(text)
    }
}

function routeModuleSearchTarget(root, target) {
    with (root) {
        const value = String(target || "").trim().toLowerCase()
        if (value === "storage") {
            selectView("storage")
        } else if (value === "messaging" || value === "delivery") {
            selectView("messaging")
        } else if (value === "capability" || value === "capabilities") {
            selectView("capabilities")
        } else if (value === "blockchain" || value === "bedrock" || value === "node") {
            selectView("blockchain")
        } else {
            selectView("storage")
        }
    }
}

function viewKeyForQuery(root, query) {
    with (root) {
        const normalized = String(query || "").trim().toLowerCase()
        if (!normalized.length) {
            return ""
        }
        const item = navItemForQuery(normalized)
        if (item && String(item.view || "").length > 0) {
            return item.view
        }
        if (normalized === "home" || normalized === "dashboard" || normalized === "overview") {
            return "overview"
        }
        if (normalized === "l1" || normalized === "l1 bedrock" || normalized === "bedrock" || normalized === "cryptarchia" || normalized === "block" || normalized === "latest blocks") {
            return "blocks"
        }
        if (normalized === "transaction" || normalized === "tx" || normalized === "txs" || normalized === "latest transactions") {
            return "transactions"
        }
        if (normalized === "wallet" || normalized === "local wallet" || normalized === "wallets") {
            return "localWallet"
        }
        if (normalized === "zone" || normalized === "channel" || normalized === "l2"
                || normalized === "lez" || normalized === "sequencer"
                || normalized === "indexer" || normalized === "account"
                || normalized === "public account" || normalized === "spel"
                || normalized === "program" || normalized === "programs"
                || normalized === "recipient" || normalized === "recipients"
                || normalized === "transfer" || normalized === "transfers"
                || normalized === "transfer activity" || normalized === "l2 blocks"
                || normalized === "lez blocks" || normalized === "l2 transaction"
                || normalized === "l2 transactions" || normalized === "lez transaction"
                || normalized === "lez transactions") {
            return "zones"
        }
        if (normalized === "chain" || normalized === "base chain" || normalized === "node" || normalized === "consensus" || normalized === "bedrock node" || normalized === "node diagnostics") {
            return "blockchain"
        }
        if (normalized === "storage diagnostics") {
            return "diagnosticsStorage"
        }
        if (normalized === "delivery diagnostics" || normalized === "messaging diagnostics") {
            return "diagnosticsDelivery"
        }
        if (normalized === "messages" || normalized === "messaging" || normalized === "delivery") {
            return "messaging"
        }
        if (normalized === "capability") {
            return "capabilities"
        }
        if (normalized === "config" || normalized === "profile") {
            return "settings"
        }
        return ""
    }
}

function settingsTargetForQuery(root, query) {
    with (root) {
        const normalized = String(query || "").trim().toLowerCase()
        if (!normalized.length) {
            return { section: "", subsection: "" }
        }
        if (normalized === "network") {
            return { section: "network", subsection: settingsNetworkSection }
        }
        if (normalized === "wallet settings" || normalized === "local wallet settings" || normalized === "wallet profile") {
            return { section: "wallet", subsection: "" }
        }
        if (normalized === "blockchain rpc" || normalized === "node rpc" || normalized === "chain rpc" || normalized === "base chain rpc") {
            return { section: "network", subsection: "blockchain" }
        }
        if (normalized === "messaging rpc" || normalized === "delivery rpc" || normalized === "delivery settings") {
            return { section: "network", subsection: "messaging" }
        }
        if (normalized === "storage rpc" || normalized === "storage network") {
            return { section: "network", subsection: "storage" }
        }
        if (normalized === "footer") {
            return { section: "ui", subsection: "footer" }
        }
        if (normalized === "dashboard settings") {
            return { section: "ui", subsection: "dashboard" }
        }
        if (normalized === "config" || normalized === "profile" || normalized === "settings") {
            return { section: "general", subsection: "" }
        }
        return { section: "", subsection: "" }
    }
}
