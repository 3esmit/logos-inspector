function saveIdlState(root) {
    with (root) {
        if (!idlStateLoaded) {
            return
        }
        bridge.callModule(inspectorModule, "saveIdlState", [root.idlStatePayload()])
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
        root.loadNetworkProfileSettings(value)
        blockchainSourceMode = root.normalizedCoreSourceMode(root.stringSetting(value, "blockchain_source_mode", blockchainSourceMode))
        indexerSourceMode = root.normalizedCoreSourceMode(root.stringSetting(value, "indexer_source_mode", indexerSourceMode))
        executionSourceMode = "rpc"
        messagingSourceMode = root.normalizedMessagingSourceMode(root.stringSetting(value, "messaging_source_mode", messagingSourceMode))
        messagingRestUrl = root.stringSetting(value, "messaging_rest_url", messagingRestUrl)
        messagingMetricsUrl = root.stringSetting(value, "messaging_metrics_url", messagingMetricsUrl)
        messagingNetworkPreset = root.normalizedMessagingNetworkPreset(root.stringSetting(value, "messaging_network_preset", messagingNetworkPreset))
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
        settingsBackupCid = root.stringSetting(value, "settings_backup_cid", settingsBackupCid)
        settingsRestoreCid = settingsBackupCid
        settingsBackupEncrypted = root.boolSetting(value, "settings_backup_encrypted", settingsBackupEncrypted)
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
        root.loadSocialSettings(value)
        root.favoriteStore.load(value.favorites)
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
        const social = root.socialSettingsPayload()
        const network = root.networkProfileSettingsPayload()
        return Object.assign({
            version: 1,
            network_profile: network.network_profile,
            sequencer_url: network.sequencer_url,
            indexer_url: network.indexer_url,
            node_url: network.node_url,
            blockchain_source_mode: root.normalizedCoreSourceMode(blockchainSourceMode),
            indexer_source_mode: root.normalizedCoreSourceMode(indexerSourceMode),
            execution_source_mode: "rpc",
            messaging_source_mode: root.normalizedMessagingSourceMode(messagingSourceMode),
            messaging_rest_url: String(messagingRestUrl || ""),
            messaging_metrics_url: String(messagingMetricsUrl || ""),
            messaging_network_preset: root.normalizedMessagingNetworkPreset(messagingNetworkPreset),
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
            settings_backup_cid: String(settingsBackupCid || ""),
            settings_backup_encrypted: settingsBackupEncrypted === true,
            blockchain_refresh_rate: root.canonicalRefreshRate(blockchainRefreshRate),
            indexer_refresh_rate: root.canonicalRefreshRate(indexerRefreshRate),
            execution_refresh_rate: root.canonicalRefreshRate(executionRefreshRate),
            messaging_refresh_rate: root.canonicalRefreshRate(messagingRefreshRate),
            storage_refresh_rate: root.canonicalRefreshRate(storageRefreshRate),
            footer_fields: footerFieldSelections || {},
            dashboard_graphs: dashboardGraphSelections || {},
            favorites: root.favoriteStore.payload()
        }, social)
    }
}

function backupSettingsToStorage(root, encrypted) {
    with (root) {
        if (!root.settingsBackupAvailable()) {
            settingsBackupStatus = qsTr("Storage REST with mutating diagnostics is required.")
            return false
        }
        settingsBackupEncrypted = encrypted === true
        saveSettingsState()
        saveIdlState()
        saveWalletState()
        const response = root.callInspector("storageBackupSettings", [
            root.effectiveStorageSourceMode(storageSourceMode),
            root.configuredStorageRestUrl(),
            storageMutatingDiagnosticsEnabled === true,
            settingsBackupEncrypted,
            walletProfile(),
            65536
        ], qsTr("Settings backup"))
        if (!response.ok) {
            settingsBackupStatus = response.error || qsTr("Settings backup failed.")
            return false
        }
        const cid = String(response.value && response.value.cid ? response.value.cid : "")
        settingsBackupCid = cid
        settingsRestoreCid = cid
        settingsBackupStatus = settingsBackupEncrypted
            ? qsTr("Encrypted backup stored as %1.").arg(cid)
            : qsTr("Backup stored as %1.").arg(cid)
        saveSettingsState()
        return true
    }
}

function restoreSettingsFromStorage(root, cid, useWallet) {
    with (root) {
        const backupCid = String(cid || "").trim()
        if (backupCid.length === 0) {
            settingsBackupStatus = qsTr("Backup CID is required.")
            return false
        }
        if (!root.settingsBackupAvailable()) {
            settingsBackupStatus = qsTr("Storage REST with mutating diagnostics is required.")
            return false
        }
        const response = root.callInspector("storageRestoreSettings", [
            root.effectiveStorageSourceMode(storageSourceMode),
            root.configuredStorageRestUrl(),
            storageMutatingDiagnosticsEnabled === true,
            backupCid,
            useWallet === true ? walletProfile() : ({}),
            false
        ], qsTr("Settings restore"))
        if (!response.ok) {
            settingsBackupStatus = response.error || qsTr("Settings restore failed.")
            return false
        }
        root.loadSettingsState()
        root.loadIdlState()
        root.loadWalletState()
        settingsBackupCid = backupCid
        settingsRestoreCid = backupCid
        settingsBackupEncrypted = response.value && response.value.encrypted === true
        settingsBackupStatus = qsTr("Restored %1 IDLs and %2 favorites from %3.")
            .arg(Number(response.value && response.value.idl_count ? response.value.idl_count : 0))
            .arg(Number(response.value && response.value.favorites ? response.value.favorites : 0))
            .arg(backupCid)
        saveSettingsState()
        return true
    }
}

function settingsBackupAvailable(root) {
    with (root) {
        return root.effectiveStorageSourceMode(storageSourceMode) === "rest"
            && storageMutatingDiagnosticsEnabled === true
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
            && root.walletHomeConfigured()
    }
}

function walletHomeConfigured(root) {
    with (root) {
        if (String(walletHome || "").trim().length > 0) {
            return true
        }
        const source = String(localWalletStatus && localWalletStatus.home_source ? localWalletStatus.home_source : "")
        return source.length > 0 && source !== "none"
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

function refreshBedrockWalletModule(root, address) {
    with (root) {
        const target = String(address === undefined || address === null ? walletPublicKeyProbe : address).trim()
        bedrockWalletModuleError = ""
        statusText = qsTr("Bedrock wallet")
        blockchainModuleReport = null
        return requestModuleAsync(inspectorModule, "blockchainModuleReport", [target], qsTr("Bedrock wallet"), false, function (response) {
            if (response.ok) {
                blockchainModuleReport = response.value || null
                bedrockWalletModuleError = root.moduleLastError("blockchain")
                appendLocalWalletOperation(qsTr("Bedrock wallet module"), bedrockWalletModuleError.length ? "degraded" : "ok", target.length ? target : qsTr("module report"))
            } else {
                blockchainModuleReport = null
                bedrockWalletModuleError = response.error || qsTr("Bedrock wallet module query failed.")
                appendLocalWalletOperation(qsTr("Bedrock wallet module"), "down", bedrockWalletModuleError)
            }
        })
    }
}

function bedrockWalletModuleKnownAddressRows(root) {
    with (root) {
        const items = walletPayloadList(root, "wallet_get_known_addresses", ["addresses", "known_addresses", "knownAddresses", "wallets", "public_keys", "publicKeys"])
        if (items === null) {
            return []
        }
        const rows = []
        for (let i = 0; i < items.length; ++i) {
            const item = items[i]
            const address = walletScalarText(walletField(item, ["address", "account", "account_id", "accountId", "public_key", "publicKey", "id"], item))
            if (!address.length) {
                continue
            }
            rows.push({
                address: address,
                label: walletScalarText(walletField(item, ["label", "name", "kind", "type"], "")),
                raw: item
            })
        }
        return rows
    }
}

function bedrockWalletModuleNoteRows(root) {
    with (root) {
        const items = walletPayloadList(root, "wallet_get_notes", ["notes", "wallet_notes", "walletNotes", "entries"])
        if (items === null) {
            return []
        }
        const rows = []
        for (let i = 0; i < items.length; ++i) {
            const item = items[i]
            rows.push({
                id: walletScalarText(walletField(item, ["note_id", "noteId", "id", "commitment", "note_commitment", "noteCommitment"], "")),
                value: walletScalarText(walletField(item, ["value", "amount", "balance"], "")),
                commitment: walletScalarText(walletField(item, ["commitment", "note_commitment", "noteCommitment", "cm"], "")),
                nullifier: walletScalarText(walletField(item, ["nullifier", "nullifier_hash", "nullifierHash"], "")),
                tip: walletScalarText(walletField(item, ["tip", "header", "header_id", "headerId", "block_id", "blockId"], "")),
                raw: item
            })
        }
        return rows
    }
}

function bedrockWalletModuleVoucherRows(root) {
    with (root) {
        const items = walletPayloadList(root, "wallet_get_claimable_vouchers", ["vouchers", "claimable_vouchers", "claimableVouchers", "entries"])
        if (items === null) {
            return []
        }
        const rows = []
        for (let i = 0; i < items.length; ++i) {
            const item = items[i]
            rows.push({
                commitment: walletScalarText(walletField(item, ["commitment", "voucher_commitment", "voucherCommitment", "voucher_cm", "voucherCm", "cm"], item)),
                nullifier: walletScalarText(walletField(item, ["nullifier", "nullifier_hash", "nullifierHash"], "")),
                value: walletScalarText(walletField(item, ["value", "amount", "balance"], "")),
                tip: walletScalarText(walletField(item, ["tip", "header", "header_id", "headerId", "block_id", "blockId"], "")),
                raw: item
            })
        }
        return rows
    }
}

function bedrockWalletModuleBalance(root) {
    with (root) {
        return walletProbePayload(root, "wallet_get_balance")
    }
}

function bedrockWalletModuleBalanceSummary(root) {
    with (root) {
        const balance = root.bedrockWalletModuleBalance()
        if (balance === null) {
            return ""
        }
        const scalar = root.scalarValue(balance)
        if (scalar !== null) {
            return root.valueText(scalar)
        }
        const keys = ["balance", "available", "spendable", "confirmed", "pending"]
        const parts = []
        for (let i = 0; i < keys.length; ++i) {
            const value = walletField(balance, [keys[i]], "")
            const text = walletScalarText(value)
            if (text.length) {
                parts.push(qsTr("%1 %2").arg(keys[i]).arg(text))
            }
        }
        return parts.length ? parts.join(", ") : qsTr("loaded")
    }
}

function bedrockWalletModuleRawText(root, method) {
    with (root) {
        const probe = root.moduleProbe("blockchain", method)
        if (!probe || probe.value === undefined || probe.value === null) {
            return ""
        }
        return walletJsonText(probe.value)
    }
}

function bedrockWalletModuleListKnown(root, method) {
    with (root) {
        return walletPayloadList(root, method, walletListKeys(method)) !== null
    }
}

function bedrockWalletModuleReadOnlyMethods(root) {
    with (root) {
        return [
            "wallet_get_known_addresses",
            "wallet_get_claimable_vouchers",
            "wallet_get_balance",
            "wallet_get_notes"
        ]
    }
}

function walletListKeys(method) {
    switch (String(method || "")) {
    case "wallet_get_known_addresses":
        return ["addresses", "known_addresses", "knownAddresses", "wallets", "public_keys", "publicKeys"]
    case "wallet_get_notes":
        return ["notes", "wallet_notes", "walletNotes", "entries"]
    case "wallet_get_claimable_vouchers":
        return ["vouchers", "claimable_vouchers", "claimableVouchers", "entries"]
    default:
        return []
    }
}

function walletPayloadList(root, method, keys) {
    const payload = walletProbePayload(root, method)
    if (Array.isArray(payload)) {
        return payload
    }
    if (payload && typeof payload === "object") {
        for (let i = 0; i < keys.length; ++i) {
            const value = payload[keys[i]]
            if (Array.isArray(value)) {
                return value
            }
        }
    }
    return null
}

function walletProbePayload(root, method) {
    const value = root.moduleProbeValue("blockchain", method)
    return unwrapLogoscoreCallValue(value)
}

function unwrapLogoscoreCallValue(value) {
    let current = value
    if (current && typeof current === "object" && !Array.isArray(current)
            && current.runner !== undefined && current.value !== undefined) {
        current = current.value
    }
    if (current && typeof current === "object" && !Array.isArray(current)
            && current.result !== undefined) {
        const result = current.result
        if (result && typeof result === "object" && !Array.isArray(result)
                && result.value !== undefined) {
            return result.value
        }
        return result
    }
    return current === undefined ? null : current
}

function walletField(item, keys, fallback) {
    if (!item || typeof item !== "object" || Array.isArray(item)) {
        return item === undefined || item === null ? fallback : item
    }
    for (let i = 0; i < keys.length; ++i) {
        const value = item[keys[i]]
        if (value !== undefined && value !== null && String(value).length > 0) {
            return value
        }
    }
    return fallback
}

function walletScalarText(value) {
    if (value === undefined || value === null) {
        return ""
    }
    if (typeof value === "object") {
        return walletJsonText(value)
    }
    return String(value)
}

function walletJsonText(value) {
    try {
        return JSON.stringify(value, null, 2)
    } catch (error) {
        return String(value || "")
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
    return root.networkProfileCacheScope()
}

function accountOwnerCacheKey(root, ownerProgramId) {
    with (root) {
        return root.canonicalProgramIdHex(ownerProgramId) || root.normalizedHexText(ownerProgramId)
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
