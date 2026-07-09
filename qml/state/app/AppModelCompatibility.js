function contract() {
    return [{
        key: "shell",
        facade: "AppShellState",
        owner: "shell",
        ownerLabel: "App shell state",
        status: "compatibility",
        migration: "active",
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
        owner: "sourceRouting",
        ownerLabel: "Source routing state",
        status: "compatibility",
        migration: "active",
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
        owner: "backupImport",
        ownerLabel: "Backup import state",
        status: "compatibility",
        migration: "active",
        members: [
            "previewLocalSettingsImportPlan",
            "restoreLocalSettingsBackup",
            "backupImportDecisionSummaryText",
            "uploadBackupCatalogEntry"
        ]
    }, {
        key: "wallet",
        facade: "LocalWalletAppState",
        owner: "wallet",
        ownerLabel: "Local wallet state",
        status: "compatibility",
        migration: "active",
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
        owner: "",
        ownerLabel: "Metrics helper module",
        status: "compatibility",
        migration: "active",
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
        owner: "chainPages",
        ownerLabel: "Network inspection state",
        status: "compatibility",
        migration: "active",
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
                owner: groups[i].owner || "",
                status: groups[i].status,
                migration: groups[i].migration || ""
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
                    owner: groups[i].owner || "",
                    status: groups[i].status,
                    migration: groups[i].migration || ""
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

function ownerRecords() {
    const groups = contract()
    const rows = []
    for (let i = 0; i < groups.length; ++i) {
        const owner = String(groups[i].owner || "")
        if (owner.length) {
            rows.push({
                key: groups[i].key,
                facade: groups[i].facade,
                owner: owner,
                ownerLabel: String(groups[i].ownerLabel || owner),
                status: groups[i].status,
                migration: groups[i].migration || ""
            })
        }
    }
    return rows
}

function missingOwners(target) {
    const rows = ownerRecords()
    const missing = []
    for (let i = 0; i < rows.length; ++i) {
        const owner = rows[i].owner
        if (!target || target[owner] === undefined || target[owner] === null) {
            missing.push(rows[i])
        }
    }
    return missing
}

function migrationRows() {
    const groups = contract()
    const rows = []
    for (let i = 0; i < groups.length; ++i) {
        rows.push({
            key: groups[i].key,
            facade: groups[i].facade,
            owner: groups[i].owner || "",
            ownerLabel: groups[i].ownerLabel || "",
            status: groups[i].status,
            migration: groups[i].migration || "active",
            memberCount: groups[i].members ? groups[i].members.length : 0
        })
    }
    return rows
}

function report(target) {
    const groups = contract()
    const missing = missingMembers(target)
    const owners = ownerRecords()
    const ownerMissing = missingOwners(target)
    return {
        ok: missing.length === 0 && ownerMissing.length === 0,
        groups: groups,
        members: allMembers(),
        missing: missing,
        owners: owners,
        missingOwners: ownerMissing,
        migrations: migrationRows(),
        groupCount: groups.length,
        memberCount: allMembers().length,
        ownerCount: owners.length,
        provenance: ["app_model_compatibility_manifest"]
    }
}
