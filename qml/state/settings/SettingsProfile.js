function defaultBackupContents() {
    return {
        settings: true,
        favorites: true,
        idl_registry: true,
        wallet_profile: true
    }
}

function normalizedBackupContents(contents) {
    const value = contents && typeof contents === "object" ? contents : defaultBackupContents()
    return {
        settings: value.settings === true,
        favorites: value.favorites === true,
        idl_registry: value.idl_registry === true || value.idls === true || value.idl === true,
        wallet_profile: value.wallet_profile === true || value.wallet === true
    }
}

function backupContentsSelected(contents) {
    const value = normalizedBackupContents(contents)
    return value.settings || value.favorites || value.idl_registry || value.wallet_profile
}

function updatedBackupContents(contents, area, enabled) {
    const next = normalizedBackupContents(contents)
    const key = String(area || "")
    if (key === "settings" || key === "favorites" || key === "idl_registry" || key === "wallet_profile") {
        next[key] = enabled === true
    }
    return next
}

function applySettingsState(root, value) {
    with (root) {
        settingsStateError = ""
        root.loadNetworkProfileSettings(value)
        root.loadNetworkConnectorConfig(value)
        messagingRestUrl = root.stringSetting(value, "messaging_rest_url", messagingRestUrl)
        messagingMetricsUrl = root.stringSetting(value, "messaging_metrics_url", messagingMetricsUrl)
        messagingNetworkPreset = root.sourceRouting.normalizedMessagingNetworkPreset(
            root.stringSetting(value, "messaging_network_preset", messagingNetworkPreset))
        messagingRollingWindow = root.numberSetting(value, "messaging_rolling_window", messagingRollingWindow)
        messagingAdminRestEnabled = root.boolSetting(value, "messaging_admin_rest_enabled", messagingAdminRestEnabled)
        storageRestUrl = root.stringSetting(value, "storage_rest_url", storageRestUrl)
        storageMetricsUrl = root.stringSetting(value, "storage_metrics_url", storageMetricsUrl)
        storageNetworkPreset = root.stringSetting(value, "storage_network_preset", storageNetworkPreset)
        storageCidProbe = root.stringSetting(value, "storage_cid_probe", storageCidProbe)
        storageRollingWindow = root.numberSetting(value, "storage_rolling_window", storageRollingWindow)
        storageLocalDiagnosticsEnabled = root.boolSetting(value, "storage_local_diagnostics_enabled", storageLocalDiagnosticsEnabled)
        storagePrivilegedDebugEnabled = root.boolSetting(value, "storage_privileged_debug_enabled", storagePrivilegedDebugEnabled)
        localNodesEnabled = root.boolSetting(value, "local_nodes_enabled", localNodesEnabled)
        localDevnetEnabled = localNodesEnabled && root.boolSetting(value, "local_devnet_enabled", localDevnetEnabled)
        settingsBackupEncrypted = root.boolSetting(value, "settings_backup_encrypted", settingsBackupEncrypted)
        root.metrics.blockchainRefreshRate = root.metrics.canonicalRefreshRate(
            root.numberSetting(value, "blockchain_refresh_rate", root.metrics.blockchainRefreshRate))
        root.metrics.messagingRefreshRate = root.metrics.canonicalRefreshRate(
            root.numberSetting(value, "messaging_refresh_rate", root.metrics.messagingRefreshRate))
        root.metrics.storageRefreshRate = root.metrics.canonicalRefreshRate(
            root.numberSetting(value, "storage_refresh_rate", root.metrics.storageRefreshRate))
        if (value.footer_fields && typeof value.footer_fields === "object" && !Array.isArray(value.footer_fields)) {
            root.metrics.footerFieldSelections = root.mergeMap(
                root.metrics.defaultFooterFieldSelections(), value.footer_fields)
            root.metrics.footerFieldRevision += 1
        }
        if (value.dashboard_graphs && typeof value.dashboard_graphs === "object" && !Array.isArray(value.dashboard_graphs)) {
            root.metrics.dashboardGraphSelections = root.mergeMap(
                root.metrics.defaultDashboardGraphSelections(), value.dashboard_graphs)
            root.metrics.dashboardGraphRevision += 1
        }
        root.social.loadSettings(value)
        root.favoriteStore.load(value.favorites)
        settingsStateLoaded = true
    }
}

function settingsStatePayload(root) {
    with (root) {
        const social = root.social.settingsPayload()
        const network = root.networkProfileSettingsPayload()
        return Object.assign({
            version: 2,
            network_profile: network.network_profile,
            node_url: network.node_url,
            network_connector_config: root.networkConnectorConfigPayload(),
            messaging_rest_url: String(messagingRestUrl || ""),
            messaging_metrics_url: String(messagingMetricsUrl || ""),
            messaging_network_preset: root.sourceRouting.normalizedMessagingNetworkPreset(messagingNetworkPreset),
            messaging_rolling_window: Number(messagingRollingWindow || 0),
            messaging_admin_rest_enabled: messagingAdminRestEnabled === true,
            storage_rest_url: String(storageRestUrl || ""),
            storage_metrics_url: String(storageMetricsUrl || ""),
            storage_network_preset: String(storageNetworkPreset || ""),
            storage_cid_probe: String(storageCidProbe || ""),
            storage_rolling_window: Number(storageRollingWindow || 0),
            storage_local_diagnostics_enabled: storageLocalDiagnosticsEnabled === true,
            storage_privileged_debug_enabled: storagePrivilegedDebugEnabled === true,
            local_nodes_enabled: localNodesEnabled === true,
            local_devnet_enabled: localNodesEnabled === true && localDevnetEnabled === true,
            settings_backup_encrypted: settingsBackupEncrypted === true,
            blockchain_refresh_rate: root.metrics.canonicalRefreshRate(root.metrics.blockchainRefreshRate),
            messaging_refresh_rate: root.metrics.canonicalRefreshRate(root.metrics.messagingRefreshRate),
            storage_refresh_rate: root.metrics.canonicalRefreshRate(root.metrics.storageRefreshRate),
            footer_fields: root.metrics.footerFieldSelections || {},
            dashboard_graphs: root.metrics.dashboardGraphSelections || {},
            favorites: root.favoriteStore.payload()
        }, social)
    }
}

function saveSelectedBackupContents(root, selectedContents) {
    with (root) {
        if (selectedContents.settings || selectedContents.favorites) {
            saveSettingsState()
        }
        if (selectedContents.idl_registry) {
            saveIdlState()
        }
        if (selectedContents.wallet_profile) {
            saveWalletState()
        }
    }
}
