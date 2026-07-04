.import "../../services/BridgeHelpers.js" as BridgeHelpers

function loadIdlState(root) {
    with (root) {
        const response = bridge.callModule(inspectorModule, "loadIdlState", [])
        idlStateLoaded = true
        if (!response.ok || !response.value || typeof response.value !== "object") {
            return
        }

        registeredIdls.clear()
        const idls = Array.isArray(response.value.idls) ? response.value.idls : []
        for (let i = 0; i < idls.length; ++i) {
            const entry = root.normalizedIdlEntry(idls[i], registeredIdls.count)
            if (entry !== null && entry.json.length) {
                registeredIdls.append(entry)
            }
        }

        accountIdlSelections = response.value.account_idl_selections && typeof response.value.account_idl_selections === "object"
            ? response.value.account_idl_selections
            : ({})
        accountIdlSelectionRevision += 1
    }
}

function saveIdlState(root) {
    with (root) {
        if (!idlStateLoaded) {
            return
        }
        bridge.callModule(inspectorModule, "saveIdlState", [idlStatePayload()])
    }
}

function idlStatePayload(root) {
    with (root) {
        return {
            version: 1,
            idls: registeredIdlEntries(),
            account_idl_selections: accountIdlSelections || {}
        }
    }
}

function loadSettingsState(root) {
    with (root) {
        const response = bridge.callModule(inspectorModule, "loadSettingsState", [])
        if (!response.ok || !response.value || typeof response.value !== "object") {
            settingsStateLoaded = true
            settingsStateError = response && response.error ? response.error : qsTr("Settings state is not readable.")
            return
        }

        settingsStateError = ""
        const value = response.value
        const storedNetworkProfile = root.normalizedNetworkProfile(root.stringSetting(value, "network_profile", networkProfile))
        sequencerUrl = root.stringSetting(value, "sequencer_url", sequencerUrl)
        indexerUrl = root.stringSetting(value, "indexer_url", indexerUrl)
        nodeUrl = root.stringSetting(value, "node_url", nodeUrl)
        networkProfile = root.resolvedNetworkProfile(storedNetworkProfile, sequencerUrl, indexerUrl, nodeUrl)
        messagingSourceMode = root.normalizedMessagingSourceMode(root.stringSetting(value, "messaging_source_mode", messagingSourceMode))
        messagingRestUrl = root.stringSetting(value, "messaging_rest_url", messagingRestUrl)
        messagingMetricsUrl = root.stringSetting(value, "messaging_metrics_url", messagingMetricsUrl)
        messagingNetworkPreset = root.normalizedMessagingNetworkPreset(root.stringSetting(value, "messaging_network_preset", messagingNetworkPreset))
        messagingNodeInfoId = root.stringSetting(value, "messaging_node_info_id", messagingNodeInfoId)
        messagingRollingWindow = root.numberSetting(value, "messaging_rolling_window", messagingRollingWindow)
        messagingAdminRestEnabled = root.boolSetting(value, "messaging_admin_rest_enabled", messagingAdminRestEnabled)
        messagingMutatingDiagnosticsEnabled = root.boolSetting(value, "messaging_mutating_diagnostics_enabled", messagingMutatingDiagnosticsEnabled)
        storageSourceMode = root.normalizedStorageSourceMode(root.stringSetting(value, "storage_source_mode", storageSourceMode))
        storageRestUrl = root.stringSetting(value, "storage_rest_url", storageRestUrl)
        storageMetricsUrl = root.stringSetting(value, "storage_metrics_url", storageMetricsUrl)
        storageNetworkPreset = root.stringSetting(value, "storage_network_preset", storageNetworkPreset)
        storageDataDir = root.stringSetting(value, "storage_data_dir", storageDataDir)
        storageCidProbe = root.stringSetting(value, "storage_cid_probe", storageCidProbe)
        storageRollingWindow = root.numberSetting(value, "storage_rolling_window", storageRollingWindow)
        storageLocalDiagnosticsEnabled = root.boolSetting(value, "storage_local_diagnostics_enabled", storageLocalDiagnosticsEnabled)
        storagePrivilegedDebugEnabled = root.boolSetting(value, "storage_privileged_debug_enabled", storagePrivilegedDebugEnabled)
        storageMutatingDiagnosticsEnabled = root.boolSetting(value, "storage_mutating_diagnostics_enabled", storageMutatingDiagnosticsEnabled)
        blockchainRefreshRate = root.canonicalRefreshRate(root.numberSetting(value, "blockchain_refresh_rate", blockchainRefreshRate))
        indexerRefreshRate = root.canonicalRefreshRate(root.numberSetting(value, "indexer_refresh_rate", indexerRefreshRate))
        executionRefreshRate = root.canonicalRefreshRate(root.numberSetting(value, "execution_refresh_rate", executionRefreshRate))
        messagingRefreshRate = root.canonicalRefreshRate(root.numberSetting(value, "messaging_refresh_rate", messagingRefreshRate))
        storageRefreshRate = root.canonicalRefreshRate(root.numberSetting(value, "storage_refresh_rate", storageRefreshRate))
        if (value.footer_fields && typeof value.footer_fields === "object" && !Array.isArray(value.footer_fields)) {
            footerFieldSelections = root.mergeMap(root.defaultFooterFieldSelections(), value.footer_fields)
            footerFieldRevision += 1
        }
        if (value.dashboard_graphs && typeof value.dashboard_graphs === "object" && !Array.isArray(value.dashboard_graphs)) {
            dashboardGraphSelections = root.mergeMap(root.defaultDashboardGraphSelections(), value.dashboard_graphs)
            dashboardGraphRevision += 1
        }
        settingsStateLoaded = true
    }
}

function saveSettingsState(root) {
    with (root) {
        if (!settingsStateLoaded) {
            return
        }
        bridge.callModule(inspectorModule, "saveSettingsState", [settingsStatePayload()])
    }
}

function settingsStatePayload(root) {
    with (root) {
        const resolvedProfile = root.inferNetworkProfileFromEndpoints(sequencerUrl, indexerUrl, nodeUrl)
        return {
            version: 1,
            network_profile: resolvedProfile,
            sequencer_url: String(sequencerUrl || ""),
            indexer_url: String(indexerUrl || ""),
            node_url: String(nodeUrl || ""),
            messaging_source_mode: root.normalizedMessagingSourceMode(messagingSourceMode),
            messaging_rest_url: String(messagingRestUrl || ""),
            messaging_metrics_url: String(messagingMetricsUrl || ""),
            messaging_network_preset: root.normalizedMessagingNetworkPreset(messagingNetworkPreset),
            messaging_node_info_id: String(messagingNodeInfoId || ""),
            messaging_rolling_window: Number(messagingRollingWindow || 0),
            messaging_admin_rest_enabled: messagingAdminRestEnabled === true,
            messaging_mutating_diagnostics_enabled: messagingMutatingDiagnosticsEnabled === true,
            storage_source_mode: root.normalizedStorageSourceMode(storageSourceMode),
            storage_rest_url: String(storageRestUrl || ""),
            storage_metrics_url: String(storageMetricsUrl || ""),
            storage_network_preset: String(storageNetworkPreset || ""),
            storage_data_dir: String(storageDataDir || ""),
            storage_cid_probe: String(storageCidProbe || ""),
            storage_rolling_window: Number(storageRollingWindow || 0),
            storage_local_diagnostics_enabled: storageLocalDiagnosticsEnabled === true,
            storage_privileged_debug_enabled: storagePrivilegedDebugEnabled === true,
            storage_mutating_diagnostics_enabled: storageMutatingDiagnosticsEnabled === true,
            blockchain_refresh_rate: root.canonicalRefreshRate(blockchainRefreshRate),
            indexer_refresh_rate: root.canonicalRefreshRate(indexerRefreshRate),
            execution_refresh_rate: root.canonicalRefreshRate(executionRefreshRate),
            messaging_refresh_rate: root.canonicalRefreshRate(messagingRefreshRate),
            storage_refresh_rate: root.canonicalRefreshRate(storageRefreshRate),
            footer_fields: footerFieldSelections || {},
            dashboard_graphs: dashboardGraphSelections || {}
        }
    }
}

function loadWalletState(root) {
    with (root) {
        const response = bridge.callModule(inspectorModule, "loadWalletState", [])
        walletStateLoaded = true
        if (!response.ok || !response.value || typeof response.value !== "object") {
            return
        }

        const profile = response.value.profile && typeof response.value.profile === "object" ? response.value.profile : response.value
        walletProfileLabel = String(profile.label || profile.name || qsTr("Local wallet"))
        walletBinary = String(profile.wallet_binary || profile.walletBinary || "")
        walletHome = String(profile.wallet_home || profile.walletHome || "")
        walletPublicKeyProbe = String(profile.public_key_probe || profile.publicKeyProbe || "")
        localWalletOperations = Array.isArray(response.value.operations) ? response.value.operations : []
    }
}

function detectWalletProfile(root, saveDetected) {
    with (root) {
        const response = bridge.callModule(inspectorModule, "detectWalletProfile", [])
        if (!response.ok || !response.value || typeof response.value !== "object") {
            localWalletStatusError = response && response.error ? response.error : qsTr("Wallet autodetect failed.")
            return false
        }

        const detectedBinary = String(response.value.wallet_binary || response.value.walletBinary || "")
        const detectedHome = String(response.value.wallet_home || response.value.walletHome || "")
        if (detectedBinary.length > 0) {
            walletBinary = detectedBinary
        }
        if (detectedHome.length > 0) {
            walletHome = detectedHome
        }
        clearLocalWalletStatus()
        if (saveDetected !== false) {
            saveWalletState()
        }
        return detectedBinary.length > 0 || detectedHome.length > 0
    }
}

function saveWalletState(root) {
    with (root) {
        if (!walletStateLoaded) {
            return
        }
        bridge.callModule(inspectorModule, "saveWalletState", [walletStatePayload()])
    }
}

function walletStatePayload(root) {
    with (root) {
        return {
            version: 1,
            profile: walletProfile(),
            operations: Array.isArray(localWalletOperations) ? localWalletOperations.slice(-50) : []
        }
    }
}

function walletProfile(root) {
    with (root) {
        return {
            label: String(walletProfileLabel || qsTr("Local wallet")),
            wallet_binary: String(walletBinary || ""),
            wallet_home: String(walletHome || ""),
            network_profile: String(networkProfile || ""),
            public_key_probe: String(walletPublicKeyProbe || "")
        }
    }
}

function walletProfileConfigured(root) {
    with (root) {
        return String(walletBinary || "").trim().length > 0
    }
}

function bedrockWalletSourceConfigured(root) {
    with (root) {
        return String(nodeUrl || "").trim().length > 0
    }
}

function walletProfileUsable(root) {
    with (root) {
        return walletProfileConfigured()
            && localWalletStatus
            && String(localWalletStatus.status || "") === "ok"
    }
}

function clearLocalWalletStatus(root) {
    with (root) {
        localWalletStatus = null
        localWalletStatusError = ""
    }
}

function walletHomeFallbackLabel(root) {
    with (root) {
        if (String(walletHome || "").trim().length > 0) {
            return root.redactedPath(walletHome)
        }
        const source = String(localWalletStatus && localWalletStatus.home_source ? localWalletStatus.home_source : "")
        if (source.length > 0 && source !== "none" && source !== "profile") {
            return qsTr("$%1").arg(source)
        }
        return qsTr("Not configured")
    }
}

function walletHomeSourceLabel(root) {
    with (root) {
        if (String(walletHome || "").trim().length > 0) {
            return qsTr("profile home")
        }
        const source = String(localWalletStatus && localWalletStatus.home_source ? localWalletStatus.home_source : "")
        if (source.length > 0 && source !== "none" && source !== "profile") {
            return qsTr("$%1").arg(source)
        }
        return qsTr("home not configured")
    }
}

function walletBinaryDisplayLabel(root) {
    with (root) {
        return root.redactedPath(walletBinary)
    }
}

function walletHomeDisplayLabel(root) {
    with (root) {
        return root.walletHomeFallbackLabel()
    }
}

function redactedPath(root, path) {
    with (root) {
        const text = String(path || "").trim()
        if (!text.length) {
            return ""
        }
        const normalized = text.replace(/\\/g, "/")
        const parts = normalized.split("/").filter(part => part.length > 0)
        const isDriveRoot = /^[A-Za-z]:\/?$/.test(normalized)
        const absolutePath = normalized.startsWith("/") || /^[A-Za-z]:\//.test(normalized)
        if (isDriveRoot) {
            return "..."
        }
        if (parts.length === 0 && absolutePath) {
            return "..."
        }
        if (parts.length === 1 && absolutePath) {
            return qsTr(".../%1").arg(parts[0])
        }
        if (parts.length <= 1) {
            return "..."
        }
        return qsTr(".../%1").arg(parts[parts.length - 1])
    }
}

function storageDisplayPath(root, path) {
    with (root) {
        return storageLocalDiagnosticsEnabled === true ? String(path || "") : root.redactedPath(path)
    }
}

function checkLocalWalletProfile(root, showResult) {
    with (root) {
        localWalletStatusError = ""
        statusText = qsTr("Local wallet")
        return requestModuleAsync(inspectorModule, "localWalletProfileStatus", [walletProfile()], qsTr("Local wallet"), showResult === true, function (response) {
            if (response.ok) {
                localWalletStatus = response.value || null
                localWalletStatusError = ""
                appendLocalWalletOperation(qsTr("Profile status"), String(response.value && response.value.status ? response.value.status : "ok"), String(response.value && response.value.detail ? response.value.detail : ""))
            } else {
                localWalletStatus = null
                localWalletStatusError = response.error || qsTr("Profile status failed.")
                appendLocalWalletOperation(qsTr("Profile status"), "down", localWalletStatusError)
            }
        })
    }
}

function checkedLocalWalletProfile(root) {
    with (root) {
        const response = requestModule(inspectorModule, "localWalletProfileStatus", [walletProfile()], qsTr("Local wallet"), false)
        if (response.ok) {
            localWalletStatus = response.value || null
            localWalletStatusError = ""
            const status = String(response.value && response.value.status ? response.value.status : "")
            return {
                ok: status === "ok",
                detail: String(response.value && response.value.detail ? response.value.detail : "")
            }
        }
        localWalletStatus = null
        localWalletStatusError = response.error || qsTr("Profile status failed.")
        return {
            ok: false,
            detail: localWalletStatusError
        }
    }
}

function deployProgramBinary(root, programPath) {
    with (root) {
        if (busy) {
            setResult(qsTr("Program deploy"), qsTr("Another inspection is already running."), true)
            return null
        }

        const path = String(programPath || "").trim()
        if (!path.length) {
            setResult(qsTr("Program deploy"), qsTr("Program binary path is required."), true)
            return null
        }
        if (!walletProfileConfigured()) {
            openLocalWallet("", "profiles")
            return null
        }

        busy = true
        statusText = qsTr("Deploy program")
        return requestModuleAsync(inspectorModule, "localWalletDeployProgram", [walletProfile(), path], qsTr("Program deploy"), true, function (response) {
            busy = false
            if (response.ok) {
                appendLocalWalletOperation(qsTr("Deploy program"), "submitted", root.deployProgramOperationDetail(response.value))
            } else {
                appendLocalWalletOperation(qsTr("Deploy program"), "down", response.error || qsTr("Deployment failed."))
            }
        })
    }
}

function deployProgramOperationDetail(root, value) {
    with (root) {
        const report = value || {}
        const program = String(report.program_id_base58 || report.program_id_hex || "")
        const tx = String(report.deployment_tx_hash || "")
        if (program.length > 0 && tx.length > 0) {
            return qsTr("%1, tx %2").arg(root.shortHash(program)).arg(root.shortHash(tx))
        }
        if (tx.length > 0) {
            return qsTr("tx %1").arg(root.shortHash(tx))
        }
        return qsTr("submitted")
    }
}

function queryBedrockWalletBalance(root) {
    with (root) {
        const publicKey = String(walletPublicKeyProbe || "").trim()
        if (!publicKey.length) {
            bedrockWalletBalanceError = qsTr("Wallet public key is required.")
            return
        }
        if (!root.isBedrockHexId(publicKey)) {
            bedrockWalletBalanceError = qsTr("Wallet public key must be 64 hex characters.")
            return
        }
        const tip = String(bedrockWalletBalanceTip || "").trim()
        if (tip.length > 0 && !root.isBedrockHexId(tip)) {
            bedrockWalletBalanceError = qsTr("Balance tip must be a 64-hex header id.")
            return
        }
        bedrockWalletBalanceError = ""
        statusText = qsTr("Bedrock wallet")
        return requestModuleAsync(inspectorModule, "bedrockWalletBalance", [String(nodeUrl || ""), publicKey, tip], qsTr("Bedrock wallet"), false, function (response) {
            if (response.ok) {
                bedrockWalletBalanceValue = response.value
                bedrockWalletBalanceError = ""
                appendLocalWalletOperation(qsTr("Bedrock balance"), "ok", publicKey)
            } else {
                bedrockWalletBalanceValue = null
                bedrockWalletBalanceError = response.error || qsTr("Balance query failed.")
                appendLocalWalletOperation(qsTr("Bedrock balance"), "down", bedrockWalletBalanceError)
            }
        })
    }
}

function isBedrockHexId(root, value) {
    with (root) {
        return /^(0x)?[0-9a-fA-F]{64}$/.test(String(value || "").trim())
    }
}

function appendLocalWalletOperation(root, label, status, detail) {
    with (root) {
        const rows = Array.isArray(localWalletOperations) ? localWalletOperations.slice(-49) : []
        rows.push({
            label: String(label || ""),
            status: String(status || ""),
            detail: String(detail || ""),
            time: new Date().toLocaleTimeString(Qt.locale(), "hh:mm:ss")
        })
        localWalletOperations = rows
        saveWalletState()
    }
}

function registeredIdlEntries(root) {
    with (root) {
        const rows = []
        for (let i = 0; i < registeredIdls.count; ++i) {
            rows.push(root.idlEntryAt(i))
        }
        return rows
    }
}

function normalizedIdlEntry(root, entry, fallbackIndex) {
    with (root) {
        const row = entry || {}
        const json = String(row.json || "")
        const name = String(row.name || root.idlNameFromJson(json) || qsTr("IDL %1").arg(Number(fallbackIndex || 0) + 1))
        const programId = String(row.programId || row.program_id || "")
        const programIdHex = String(row.programIdHex || row.program_id_hex || root.canonicalProgramIdHex(programId))
        return {
            key: String(row.key || root.idlKey(name, programIdHex, json)),
            name: name,
            programId: programId,
            programIdHex: programIdHex,
            json: json
        }
    }
}

function idlEntryAt(root, index) {
    with (root) {
        if (index < 0 || index >= registeredIdls.count) {
            return { key: "", name: "", programId: "", json: "" }
        }
        const row = registeredIdls.get(index)
        return root.normalizedIdlEntry(row, index)
    }
}

function idlNameFromJson(root, json) {
    with (root) {
        const parsed = BridgeHelpers.parseJson(String(json || ""))
        return parsed.ok && parsed.value && parsed.value.name ? String(parsed.value.name) : ""
    }
}

function idlKey(root, name, programId, json) {
    with (root) {
        const text = String(name || "") + "\n" + String(programId || "") + "\n" + String(json || "")
        let hash = 2166136261
        for (let i = 0; i < text.length; ++i) {
            hash ^= text.charCodeAt(i)
            hash = Math.imul(hash, 16777619)
        }
        return (hash >>> 0).toString(16)
    }
}

function idlEntryForKey(root, key) {
    with (root) {
        const text = String(key || "")
        if (!text.length) {
            return null
        }
        for (let i = 0; i < registeredIdls.count; ++i) {
            const entry = root.idlEntryAt(i)
            if (entry.key === text) {
                return entry
            }
        }
        return null
    }
}

function idlEntriesForProgram(root, programId) {
    with (root) {
        const normalizedProgram = root.canonicalProgramIdHex(programId) || root.normalizedHexText(programId)
        if (!normalizedProgram.length) {
            return []
        }
        const entries = []
        for (let i = 0; i < registeredIdls.count; ++i) {
            const entry = root.idlEntryAt(i)
            const entryProgram = String(entry.programIdHex || "") || root.canonicalProgramIdHex(entry.programId) || root.normalizedHexText(entry.programId)
            if (entryProgram === normalizedProgram) {
                entries.push(entry)
            }
        }
        return entries
    }
}

function cacheAccountIdlSelection(root, accountId, idlEntry, accountType, ownerProgramId) {
    with (root) {
        const key = root.accountCacheKey(accountId, ownerProgramId)
        const entry = idlEntry || {}
        const entryKey = String(entry.key || entry.idlKey || "")
        if (!key.length || !entryKey.length) {
            return
        }
        const next = copyMap(accountIdlSelections)
        next[key] = {
            idlKey: entryKey,
            accountType: String(accountType || ""),
            ownerProgram: root.accountOwnerCacheKey(ownerProgramId),
            network: root.accountNetworkCacheScope()
        }
        accountIdlSelections = next
        accountIdlSelectionRevision += 1
        saveIdlState()
    }
}

function accountIdlSelection(root, accountId, ownerProgramId) {
    with (root) {
        const revision = accountIdlSelectionRevision
        const key = root.accountCacheKey(accountId, ownerProgramId)
        return key.length ? (accountIdlSelections || {})[key] || null : null
    }
}

function cachedIdlEntryForAccount(root, accountId, ownerProgramId) {
    with (root) {
        const selection = accountIdlSelection(accountId, ownerProgramId)
        const entry = selection ? root.idlEntryForKey(selection.idlKey) : null
        return entry && String(entry.programIdHex || "").length > 0 ? entry : null
    }
}

function cachedAccountType(root, accountId, ownerProgramId) {
    with (root) {
        const selection = accountIdlSelection(accountId, ownerProgramId)
        return selection ? String(selection.accountType || "") : ""
    }
}

function accountCacheKey(root, accountId, ownerProgramId) {
    with (root) {
        const account = String(accountId || "").trim()
        if (!account.length) {
            return ""
        }
        return [root.accountNetworkCacheScope(), account, root.accountOwnerCacheKey(ownerProgramId)].join("|")
    }
}

function accountNetworkCacheScope(root) {
    with (root) {
        return [String(networkProfile || ""), String(sequencerUrl || "")].join("|")
    }
}

function accountOwnerCacheKey(root, ownerProgramId) {
    with (root) {
        return root.canonicalProgramIdHex(ownerProgramId) || root.normalizedHexText(ownerProgramId)
    }
}

function accountDecodeFullyConsumed(root, value) {
    with (root) {
        if (!value) {
            return false
        }
        const consumed = Number(value.consumed_bytes)
        const total = Number(value.total_bytes)
        const remaining = Number(value.remaining_bytes || 0)
        return Number.isFinite(consumed) && Number.isFinite(total) && consumed === total && remaining === 0
    }
}

function transactionDecodeFullyConsumed(root, value) {
    with (root) {
        const decoded = root.transactionDecodedInstruction(value)
        return decoded !== null && !decoded.decode_error && Array.isArray(decoded.remaining_words) && decoded.remaining_words.length === 0
    }
}

function transactionDecodedInstruction(root, value) {
    with (root) {
        if (!value || typeof value !== "object") {
            return null
        }
        if (value.decoded_instruction) {
            return value.decoded_instruction
        }
        if (value.decoded) {
            return value.decoded
        }
        return null
    }
}

function transactionSummaryFromDetail(root, value) {
    with (root) {
        if (!value || typeof value !== "object") {
            return null
        }
        if (value.raw_summary) {
            return value.raw_summary
        }
        if (value.inspection && value.inspection.raw_summary) {
            return value.inspection.raw_summary
        }
        if (value.summary) {
            return value.summary
        }
        return null
    }
}

function normalizedHexText(root, value) {
    with (root) {
        return String(value || "").trim().replace(/^0x/i, "").toLowerCase()
    }
}

function canonicalProgramIdHex(root, value) {
    with (root) {
        const text = String(value || "").trim()
        if (!text.length) {
            return ""
        }
        if (/^(0x)?[0-9a-fA-F]{64}$/.test(text)) {
            return root.normalizedHexText(text)
        }
        const response = bridge.callModule(inspectorModule, "normalizeProgramId", [text])
        return response.ok && response.value !== undefined && response.value !== null ? String(response.value) : ""
    }
}

function autoDecodeAccountData(root, dataHex, accountId, ownerProgramId, callback) {
    with (root) {
        const serial = accountAutoDecodeSerial + 1
        accountAutoDecodeSerial = serial
        const candidates = root.accountDecodeCandidates(accountId, ownerProgramId)
        if (!String(dataHex || "").length || candidates.length === 0) {
            callback({ ok: false, error: "", value: null, entry: null })
            return serial
        }

        root.tryAccountDecodeCandidate(serial, String(dataHex || ""), candidates, 0, "", callback)
        return serial
    }
}

function accountDecodeCandidates(root, accountId, ownerProgramId) {
    with (root) {
        const candidates = []
        const cached = root.cachedIdlEntryForAccount(accountId, ownerProgramId)
        if (cached) {
            candidates.push({
                entry: cached,
                accountType: root.cachedAccountType(accountId, ownerProgramId),
                cached: true
            })
        }
        const ownerEntries = root.idlEntriesForProgram(ownerProgramId)
        for (let ownerIndex = 0; ownerIndex < ownerEntries.length; ++ownerIndex) {
            const ownerEntry = ownerEntries[ownerIndex]
            if (!root.candidateListHasEntry(candidates, ownerEntry.key)) {
                candidates.push({
                    entry: ownerEntry,
                    accountType: "",
                    cached: false,
                    ownerMatched: true
                })
            }
        }
        for (let i = 0; i < registeredIdls.count; ++i) {
            const entry = root.idlEntryAt(i)
            if (!root.candidateListHasEntry(candidates, entry.key)) {
                candidates.push({
                    entry: entry,
                    accountType: "",
                    cached: false
                })
            }
        }
        return candidates
    }
}

function tryAccountDecodeCandidate(root, serial, dataHex, candidates, index, firstError, callback) {
    with (root) {
        if (serial !== accountAutoDecodeSerial) {
            return
        }
        if (index >= candidates.length) {
            callback({ ok: false, error: firstError, value: null, entry: null })
            return
        }

        const candidate = candidates[index]
        decodeAccountDataAsync(dataHex, candidate.entry.json, candidate.accountType, function (response) {
            if (serial !== accountAutoDecodeSerial) {
                return
            }
            const error = firstError.length ? firstError : String(response.error || "")
            if (response.ok && response.value && root.accountDecodeFullyConsumed(response.value)) {
                callback({
                    ok: true,
                    error: "",
                    value: response.value,
                    entry: candidate.entry,
                    accountType: response.value.account_type || candidate.accountType
                })
                return
            }
            root.tryAccountDecodeCandidate(serial, dataHex, candidates, index + 1, error, callback)
        })
    }
}

function autoDecodeTransactionDetail(root, detail) {
    with (root) {
        const summary = root.transactionSummaryFromDetail(detail)
        if (!summary || String(summary.kind || "") !== "Public" || !Array.isArray(summary.instruction_data) || summary.instruction_data.length === 0) {
            return
        }

        const serial = transactionAutoDecodeSerial + 1
        transactionAutoDecodeSerial = serial
        const candidates = root.transactionDecodeCandidates(summary)
        if (candidates.length === 0) {
            return
        }

        root.tryTransactionDecodeCandidate(serial, summary, candidates, 0, null)
    }
}

function transactionDecodeCandidates(root, summary) {
    with (root) {
        const candidates = []
        const accountIds = Array.isArray(summary.account_ids) ? summary.account_ids : []
        for (let i = 0; i < accountIds.length; ++i) {
            const cached = root.cachedIdlEntryForAccount(accountIds[i], summary.program_id_hex)
            if (cached && !root.candidateListHasEntry(candidates, cached.key)) {
                candidates.push({
                    entry: cached,
                    cached: true
                })
            }
        }

        const programEntries = root.idlEntriesForProgram(summary.program_id_hex)
        for (let j = 0; j < programEntries.length; ++j) {
            if (!root.candidateListHasEntry(candidates, programEntries[j].key)) {
                candidates.push({
                    entry: programEntries[j],
                    cached: false
                })
            }
        }

        for (let k = 0; k < registeredIdls.count; ++k) {
            const entry = root.idlEntryAt(k)
            if (!root.candidateListHasEntry(candidates, entry.key)) {
                candidates.push({
                    entry: entry,
                    cached: false
                })
            }
        }
        return candidates
    }
}

function candidateListHasEntry(root, candidates, key) {
    with (root) {
        const text = String(key || "")
        for (let i = 0; i < candidates.length; ++i) {
            if (String(candidates[i].entry.key || "") === text) {
                return true
            }
        }
        return false
    }
}

function tryTransactionDecodeCandidate(root, serial, summary, candidates, index, partialValue) {
    with (root) {
        if (serial !== transactionAutoDecodeSerial) {
            return
        }
        if (index >= candidates.length) {
            if (partialValue) {
                transactionDetailValue = partialValue
                if (currentView === "l2TransactionDetail") {
                    lezTransactionsPageError = ""
                } else {
                    transactionsPageError = ""
                }
                setResult(qsTr("Transaction"), BridgeHelpers.formatValue(partialValue), false, partialValue)
            }
            return
        }

        const candidate = candidates[index]
        decodeTransactionSummaryAsync(summary, candidate.entry.json, function (response) {
            if (serial !== transactionAutoDecodeSerial) {
                return
            }
            if (response.ok && response.value && root.transactionDecodeFullyConsumed(response.value)) {
                transactionDetailValue = response.value
                if (currentView === "l2TransactionDetail") {
                    lezTransactionsPageError = ""
                } else {
                    transactionsPageError = ""
                }
                setResult(qsTr("Transaction"), response.text, false, response.value)
                return
            }
            const nextPartial = partialValue || (response.ok && response.value && root.transactionDecodedInstruction(response.value) ? response.value : null)
            root.tryTransactionDecodeCandidate(serial, summary, candidates, index + 1, nextPartial)
        })
    }
}

