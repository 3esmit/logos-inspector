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

function memberRecord(member) {
    const groups = contract()
    const wanted = String(member || "")
    for (let i = 0; i < groups.length; ++i) {
        const members = groups[i].members || []
        if (members.indexOf(wanted) >= 0) {
            return {
                member: wanted,
                key: groups[i].key,
                facade: groups[i].facade,
                status: groups[i].status
            }
        }
    }
    return null
}

function memberGroup(member) {
    const record = memberRecord(member)
    return record ? groupFor(record.key) : null
}

function allMembers() {
    const groups = contract()
    const rows = []
    for (let i = 0; i < groups.length; ++i) {
        const members = groups[i].members || []
        for (let j = 0; j < members.length; ++j) {
            rows.push({
                member: members[j],
                key: groups[i].key,
                facade: groups[i].facade,
                status: groups[i].status
            })
        }
    }
    return rows
}

function missingMembers(target) {
    const rows = allMembers()
    const missing = []
    for (let i = 0; i < rows.length; ++i) {
        const member = rows[i].member
        if (!target || target[member] === undefined) {
            missing.push(rows[i])
        }
    }
    return missing
}

function report(target) {
    const groups = contract()
    const missing = missingMembers(target)
    return {
        ok: missing.length === 0,
        groups: groups,
        members: allMembers(),
        missing: missing,
        groupCount: groups.length,
        memberCount: allMembers().length,
        provenance: ["app_model_compatibility_manifest"]
    }
}
