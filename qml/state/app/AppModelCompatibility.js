function contract() {
    return [{
        key: "shell",
        facade: "AppShellState",
        status: "compatibility",
        members: [
            "currentView",
            "statusText",
            "busy",
            "resultTitle",
            "resultText",
            "resultValue",
            "setResult",
            "clearResult"
        ]
    }, {
        key: "source_routing",
        facade: "SourceRoutingState",
        status: "compatibility",
        members: [
            "deliverySourceReportArgs",
            "deliverySourceLabel",
            "storageSourceReportArgs",
            "storageSourceLabel",
            "effectiveStorageSourceMode",
            "sourceHealth",
            "sourceCapability",
            "sourceProbeValue"
        ]
    }, {
        key: "settings_backup",
        facade: "BackupImportState",
        status: "compatibility",
        members: [
            "previewLocalSettingsImportPlan",
            "restoreLocalSettingsBackup",
            "backupImportDecisionSummaryText",
            "uploadBackupCatalogEntry"
        ]
    }, {
        key: "wallet",
        facade: "LocalWalletAppState",
        status: "compatibility",
        members: [
            "createWalletAccount",
            "sendWalletTransaction",
            "readIncomingWalletTransactions",
            "runWalletCommand",
            "syncPrivateWallet",
            "queryLocalWalletAccounts",
            "queryBedrockWalletBalance"
        ]
    }, {
        key: "metrics",
        facade: "AppModelMetrics",
        status: "compatibility",
        members: [
            "dashboardMetricValue",
            "dashboardMetricText",
            "openMetricValue",
            "moduleReport",
            "moduleProbeValue",
            "defaultFooterFieldSelections"
        ]
    }, {
        key: "navigation",
        facade: "EntityNavigationSession",
        status: "compatibility",
        members: [
            "routeSearch",
            "openStorageCid",
            "openAccount",
            "openTransaction",
            "openProgram",
            "openLocalWallet"
        ]
    }]
}

function groupFor(key) {
    const groups = contract()
    const wanted = String(key || "")
    for (let i = 0; i < groups.length; ++i) {
        if (groups[i].key === wanted) {
            return groups[i]
        }
    }
    return null
}

function memberGroup(member) {
    const groups = contract()
    const wanted = String(member || "")
    for (let i = 0; i < groups.length; ++i) {
        const members = groups[i].members || []
        if (members.indexOf(wanted) >= 0) {
            return groups[i]
        }
    }
    return null
}
