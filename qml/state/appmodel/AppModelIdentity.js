.import "../../services/BridgeHelpers.js" as BridgeHelpers
.import "../../utils/UiFormat.js" as UiFormat

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
        favorites = root.normalizedFavoriteEntries(value.favorites)
        favoritesRevision += 1
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
        const social = root.socialSettingsPayload()
        return Object.assign({
            version: 1,
            network_profile: resolvedProfile,
            sequencer_url: String(sequencerUrl || ""),
            indexer_url: String(indexerUrl || ""),
            node_url: String(nodeUrl || ""),
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
            favorites: root.normalizedFavoriteEntries(favorites)
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
        loadSettingsState()
        loadIdlState()
        loadWalletState()
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

function createWalletAccount(root) {
    with (root) {
        if (busy) {
            setResult(qsTr("Wallet account"), qsTr("Another inspection is already running."), true)
            return null
        }
        if (!walletProfileConfigured()) {
            openLocalWallet("", "profiles")
            setResult(qsTr("Wallet account"), qsTr("Configure wallet binary and wallet home before creating an account."), true)
            return null
        }
        const privacy = String(walletCreatePrivacy || "public").toLowerCase() === "private" ? "private" : "public"
        const label = String(walletCreateLabel || "").trim()

        busy = true
        statusText = qsTr("Wallet account")
        return requestModuleAsync(inspectorModule, "localWalletCreateAccount", [walletProfile(), privacy, label, "confirm-create-account"], qsTr("Wallet account"), true, function (response) {
            busy = false
            if (response.ok) {
                appendLocalWalletOperation(qsTr("Create account"), "created", root.walletCommandOperationDetail(response.value))
                walletCreateLabel = ""
            } else {
                appendLocalWalletOperation(qsTr("Create account"), "down", response.error || qsTr("Account creation failed."))
            }
        })
    }
}

function sendWalletTransaction(root) {
    with (root) {
        if (busy) {
            setResult(qsTr("Wallet send"), qsTr("Another inspection is already running."), true)
            return null
        }
        if (!walletProfileConfigured()) {
            openLocalWallet("", "profiles")
            setResult(qsTr("Wallet send"), qsTr("Configure wallet binary and wallet home before sending a transaction."), true)
            return null
        }
        const request = {
            from: String(walletSendFrom || "").trim(),
            to: String(walletSendTo || "").trim(),
            to_keys: String(walletSendToKeys || "").trim(),
            to_npk: String(walletSendToNpk || "").trim(),
            to_vpk: String(walletSendToVpk || "").trim(),
            to_identifier: String(walletSendToIdentifier || "").trim(),
            amount: String(walletSendAmount || "").trim()
        }
        if (!request.from.length || !request.amount.length) {
            setResult(qsTr("Wallet send"), qsTr("Sender and amount are required."), true)
            return null
        }
        if (!request.to.length && !request.to_keys.length && (!request.to_npk.length || !request.to_vpk.length)) {
            setResult(qsTr("Wallet send"), qsTr("Recipient account, keys file, or NPK/VPK pair is required."), true)
            return null
        }

        busy = true
        statusText = qsTr("Wallet send")
        return requestModuleAsync(inspectorModule, "localWalletSendTransaction", [walletProfile(), request, "confirm-send-transaction"], qsTr("Wallet send"), true, function (response) {
            busy = false
            if (response.ok) {
                appendLocalWalletOperation(qsTr("Send transaction"), "submitted", root.walletCommandOperationDetail(response.value))
            } else {
                appendLocalWalletOperation(qsTr("Send transaction"), "down", response.error || qsTr("Wallet send failed."))
            }
        })
    }
}

function readIncomingWalletTransactions(root) {
    with (root) {
        if (busy) {
            setResult(qsTr("Read incoming"), qsTr("Another inspection is already running."), true)
            return null
        }
        if (!walletProfileConfigured()) {
            openLocalWallet("", "profiles")
            setResult(qsTr("Read incoming"), qsTr("Configure wallet binary and wallet home before reading incoming transactions."), true)
            return null
        }

        busy = true
        statusText = qsTr("Read incoming")
        return requestModuleAsync(inspectorModule, "localWalletSyncPrivate", [walletProfile(), "confirm-sync-private"], qsTr("Read incoming"), true, function (response) {
            busy = false
            if (response.ok) {
                appendLocalWalletOperation(qsTr("Read incoming"), "submitted", root.privateSyncOperationDetail(response.value))
            } else {
                appendLocalWalletOperation(qsTr("Read incoming"), "down", response.error || qsTr("Incoming transaction read failed."))
            }
        })
    }
}

function runWalletCommand(root, commandArgs) {
    with (root) {
        const args = Array.isArray(commandArgs) ? commandArgs : []
        if (busy) {
            setResult(qsTr("Wallet command"), qsTr("Another inspection is already running."), true)
            return null
        }
        if (!walletProfileConfigured()) {
            openLocalWallet("", "profiles")
            setResult(qsTr("Wallet command"), qsTr("Configure wallet binary and wallet home before running wallet commands."), true)
            return null
        }
        if (!args.length) {
            setResult(qsTr("Wallet command"), qsTr("Wallet command arguments are required."), true)
            return null
        }

        busy = true
        statusText = qsTr("Wallet command")
        return requestModuleAsync(inspectorModule, "localWalletCommand", [walletProfile(), args, "confirm-wallet-command"], qsTr("Wallet command"), true, function (response) {
            busy = false
            if (response.ok) {
                appendLocalWalletOperation(qsTr("Wallet command"), "completed", root.walletCommandOperationDetail(response.value))
            } else {
                appendLocalWalletOperation(qsTr("Wallet command"), "down", response.error || qsTr("Wallet command failed."))
            }
        })
    }
}

function walletCommandOperationDetail(root, value) {
    with (root) {
        const report = value || {}
        const tx = String(report.tx_hash || report.txHash || "")
        if (tx.length) {
            return qsTr("tx %1").arg(UiFormat.shortHash(tx))
        }
        const account = String(report.account_id || report.accountId || "")
        if (account.length) {
            return UiFormat.shortId(account)
        }
        const command = String(report.command || "")
        if (command.length) {
            return command
        }
        return String(report.status || qsTr("completed"))
    }
}

function deployProgramBinary(root, programPath) {
    with (root) {
        const path = String(programPath || "").trim()
        if (busy) {
            setResult(qsTr("Program deploy"), qsTr("Another inspection is already running."), true)
            return null
        }
        if (!path.length) {
            setResult(qsTr("Program deploy"), qsTr("Program binary path is required."), true)
            return null
        }
        if (!walletProfileConfigured()) {
            openLocalWallet("", "profiles")
            setResult(qsTr("Program deploy"), qsTr("Configure wallet binary and wallet home before deploying a program."), true)
            return null
        }

        busy = true
        statusText = qsTr("Program deploy")
        return requestModuleAsync(inspectorModule, "localWalletDeployProgram", [walletProfile(), path, "confirm-deploy-program"], qsTr("Program deploy"), true, function (response) {
            busy = false
            if (response.ok) {
                appendLocalWalletOperation(qsTr("Deploy program"), "submitted", root.deployProgramOperationDetail(response.value))
            } else {
                appendLocalWalletOperation(qsTr("Deploy program"), "down", response.error || qsTr("Program deployment failed."))
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
            return qsTr("%1, tx %2").arg(UiFormat.shortHash(program)).arg(UiFormat.shortHash(tx))
        }
        if (tx.length > 0) {
            return qsTr("tx %1").arg(UiFormat.shortHash(tx))
        }
        return qsTr("submitted")
    }
}

function syncPrivateWallet(root) {
    with (root) {
        if (busy) {
            setResult(qsTr("Private sync"), qsTr("Another inspection is already running."), true)
            return null
        }
        if (!walletProfileConfigured()) {
            openLocalWallet("", "profiles")
            return null
        }

        busy = true
        statusText = qsTr("Private sync")
        return requestModuleAsync(inspectorModule, "localWalletSyncPrivate", [walletProfile(), "confirm-sync-private"], qsTr("Private sync"), true, function (response) {
            busy = false
            if (response.ok) {
                appendLocalWalletOperation(qsTr("Private sync"), "submitted", root.privateSyncOperationDetail(response.value))
            } else {
                appendLocalWalletOperation(qsTr("Private sync"), "down", response.error || qsTr("Private sync failed."))
            }
        })
    }
}

function queryLocalWalletAccounts(root, showResult) {
    with (root) {
        if (busy) {
            setResult(qsTr("Wallet accounts"), qsTr("Another inspection is already running."), true)
            return null
        }
        if (!walletProfileConfigured()) {
            localWalletAccountsError = qsTr("Configure wallet binary and wallet home, or check a profile that resolves $LEE_WALLET_HOME_DIR.")
            setResult(qsTr("Wallet accounts"), localWalletAccountsError, true)
            return null
        }

        busy = true
        statusText = qsTr("Wallet accounts")
        return requestModuleAsync(inspectorModule, "localWalletAccounts", [walletProfile()], qsTr("Wallet accounts"), showResult === true, function (response) {
            busy = false
            if (response.ok) {
                localWalletAccountsValue = response.value || null
                localWalletAccountsError = ""
                const count = response.value && Array.isArray(response.value.accounts) ? response.value.accounts.length : 0
                appendLocalWalletOperation(qsTr("Wallet accounts"), "loaded", qsTr("%1 accounts").arg(count))
            } else {
                localWalletAccountsValue = null
                localWalletAccountsError = response.error || qsTr("Wallet account list failed.")
                appendLocalWalletOperation(qsTr("Wallet accounts"), "down", localWalletAccountsError)
            }
        })
    }
}

function privateSyncOperationDetail(root, value) {
    with (root) {
        const report = value || {}
        const status = String(report.status || "submitted")
        const home = String(report.wallet_home_source || "")
        return home.length ? qsTr("%1, home %2").arg(status).arg(home) : status
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
        const labelText = String(label || "")
        const statusText = String(status || "")
        const detailText = String(detail || "")
        const rows = Array.isArray(localWalletOperations) ? localWalletOperations.slice(-49) : []
        rows.push({
            label: labelText,
            status: statusText,
            detail: detailText,
            time: new Date().toLocaleTimeString(Qt.locale(), "hh:mm:ss")
        })
        localWalletOperations = rows
        appendNodeOperationHistory({
            domain: "wallet",
            method: labelText,
            status: walletOperationStatus(statusText),
            label: labelText,
            result: {
                status: statusText,
                detail: detailText
            },
            error: walletOperationStatus(statusText) === "failed" ? detailText : ""
        }, detailText)
        saveWalletState()
    }
}

function walletOperationStatus(status) {
    const value = String(status || "").toLowerCase()
    if (value === "down" || value === "failed" || value === "error") {
        return "failed"
    }
    return "completed"
}

function previewIdlInstruction(root, request) {
    with (root) {
        if (busy) {
            setResult(qsTr("IDL instruction"), qsTr("Another inspection is already running."), true)
            return null
        }

        busy = true
        statusText = qsTr("IDL instruction")
        idlInstructionError = ""
        return requestModuleAsync(inspectorModule, "localWalletInstructionPreview", [request || {}], qsTr("IDL instruction"), false, function (response) {
            busy = false
            if (response.ok) {
                idlInstructionPreviewValue = response.value || null
                idlInstructionError = ""
            } else {
                idlInstructionPreviewValue = null
                idlInstructionError = response.error || qsTr("Instruction preview failed.")
            }
        })
    }
}

function sendIdlInstruction(root, request) {
    with (root) {
        if (busy) {
            setResult(qsTr("IDL instruction"), qsTr("Another inspection is already running."), true)
            return null
        }
        if (!walletHomeConfigured()) {
            openLocalWallet("", "profiles")
            setResult(qsTr("IDL instruction"), qsTr("Configure wallet home before sending an IDL instruction."), true)
            return null
        }

        busy = true
        statusText = qsTr("IDL instruction")
        idlInstructionError = ""
        return requestModuleAsync(inspectorModule, "localWalletInstructionSubmit", [walletProfile(), request || {}, "confirm-idl-instruction"], qsTr("IDL instruction"), true, function (response) {
            busy = false
            if (response.ok) {
                idlInstructionPreviewValue = response.value || null
                idlInstructionError = ""
                appendLocalWalletOperation(qsTr("IDL instruction"), "submitted", root.idlInstructionOperationDetail(response.value))
            } else {
                idlInstructionError = response.error || qsTr("Instruction send failed.")
                appendLocalWalletOperation(qsTr("IDL instruction"), "down", idlInstructionError)
            }
        })
    }
}

function idlInstructionOperationDetail(root, value) {
    with (root) {
        const report = value || {}
        const tx = String(report.tx_hash || report.txHash || "")
        if (tx.length > 0) {
            return qsTr("%1 %2, tx %3")
                .arg(String(report.mode || "tx"))
                .arg(String(report.instruction || "instruction"))
                .arg(UiFormat.shortHash(tx))
        }
        const words = Array.isArray(report.instruction_words) ? report.instruction_words.length : 0
        return qsTr("%1 %2, %3 word(s)")
            .arg(String(report.mode || "preview"))
            .arg(String(report.instruction || "instruction"))
            .arg(words)
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
            programBinary: String(row.programBinary || row.program_binary || ""),
            json: json,
            source: String(row.source || ""),
            sharedTopic: String(row.sharedTopic || row.shared_topic || ""),
            sharedIdentity: row.sharedIdentity || row.shared_identity || ({}),
            sharedAccountId: String(row.sharedAccountId || row.shared_account_id || "")
        }
    }
}

function idlEntryAt(root, index) {
    with (root) {
        if (index < 0 || index >= registeredIdls.count) {
            return { key: "", name: "", programId: "", programIdHex: "", programBinary: "", json: "" }
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
        entries.sort(function (left, right) {
            const leftShared = String(left.source || "") === "shared"
            const rightShared = String(right.source || "") === "shared"
            if (leftShared === rightShared) {
                return 0
            }
            return leftShared ? 1 : -1
        })
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
        let entry = selection ? root.idlEntryForKey(selection.idlKey) : null
        if (!entry && selection) {
            const sharedRows = root.sharedIdlEntriesForAccount(accountId, ownerProgramId)
            for (let i = 0; i < sharedRows.length; ++i) {
                if (String(sharedRows[i].key || "") === String(selection.idlKey || "")) {
                    entry = sharedRows[i]
                    break
                }
            }
        }
        if (!entry || String(entry.programIdHex || "").length === 0) {
            return null
        }
        const owner = root.accountOwnerCacheKey(ownerProgramId)
        if (owner.length > 0 && String(entry.programIdHex || "") !== owner) {
            return null
        }
        return entry
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
        if (cached && String(cached.source || "") !== "shared") {
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
        if (cached && String(cached.source || "") === "shared" && !root.candidateListHasEntry(candidates, cached.key)) {
            candidates.push({
                entry: cached,
                accountType: root.cachedAccountType(accountId, ownerProgramId),
                cached: true,
                shared: true
            })
        }
        const sharedEntries = root.sharedIdlEntriesForAccount(accountId, ownerProgramId)
        for (let sharedIndex = 0; sharedIndex < sharedEntries.length; ++sharedIndex) {
            const sharedEntry = sharedEntries[sharedIndex]
            if (!root.candidateListHasEntry(candidates, sharedEntry.key)) {
                candidates.push({
                    entry: sharedEntry,
                    accountType: String(sharedEntry.accountType || ""),
                    cached: false,
                    shared: true
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
                lezTransactionsPageError = ""
                setResult(qsTr("Transaction"), BridgeHelpers.formatValue(partialValue), false, partialValue, "l2TransactionDetail")
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
                lezTransactionsPageError = ""
                setResult(qsTr("Transaction"), response.text, false, response.value, "l2TransactionDetail")
                return
            }
            const nextPartial = partialValue || (response.ok && response.value && root.transactionDecodedInstruction(response.value) ? response.value : null)
            root.tryTransactionDecodeCandidate(serial, summary, candidates, index + 1, nextPartial)
        })
    }
}
