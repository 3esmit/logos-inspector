import QtQml

QtObject {
    id: root

    required property var model
    property string backupId: ""
    property var options: defaultOptions()
    property var plan: null
    property string planError: ""

    function defaultOptions() {
        return {
            settings: "replace",
            favorites: "merge",
            idl_registry: "merge",
            wallet_profile: "skip"
        }
    }

    function reset() {
        options = defaultOptions()
        plan = null
        planError = ""
    }

    function copyOptions() {
        const source = options || {}
        const result = {
            settings: String(source.settings || "skip"),
            favorites: String(source.favorites || "skip"),
            idl_registry: String(source.idl_registry || "skip"),
            wallet_profile: String(source.wallet_profile || "skip")
        }
        if (source.items && typeof source.items === "object") {
            result.items = copyNestedOptionMap(source.items)
        }
        if (source.conflicts && typeof source.conflicts === "object") {
            result.conflicts = copyNestedOptionMap(source.conflicts)
        }
        return result
    }

    function copyNestedOptionMap(source) {
        const result = {}
        const value = source && typeof source === "object" ? source : ({})
        const areas = Object.keys(value)
        for (let i = 0; i < areas.length; ++i) {
            const area = areas[i]
            result[area] = copyFlatOptionMap(value[area])
        }
        return result
    }

    function copyFlatOptionMap(source) {
        const result = {}
        const value = source && typeof source === "object" ? source : ({})
        const keys = Object.keys(value)
        for (let i = 0; i < keys.length; ++i) {
            result[keys[i]] = value[keys[i]]
        }
        return result
    }

    function setMode(area, mode) {
        const next = copyOptions()
        next[String(area || "")] = String(mode || "skip")
        options = next
        preview()
    }

    function itemRows(area) {
        const areaKey = String(area || "")
        const mode = String((options || {})[areaKey] || "skip")
        if (mode !== "merge") {
            return []
        }
        const value = plan || null
        return value && value.items && Array.isArray(value.items[areaKey]) ? value.items[areaKey] : []
    }

    function itemSelected(area, key) {
        const areaKey = String(area || "")
        const itemKey = String(key || "")
        const items = options && options.items && options.items[areaKey]
        if (!items || typeof items !== "object" || !(itemKey in items)) {
            return true
        }
        return items[itemKey] === true
    }

    function setItemSelected(area, key, selected) {
        const areaKey = String(area || "")
        const itemKey = String(key || "")
        const next = copyOptions()
        if (!next.items) {
            next.items = {}
        }
        const map = {}
        const rows = itemRows(areaKey)
        for (let i = 0; i < rows.length; ++i) {
            const rowKey = String(rows[i] && rows[i].key ? rows[i].key : "")
            if (rowKey.length) {
                map[rowKey] = itemSelected(areaKey, rowKey)
            }
        }
        map[itemKey] = selected === true
        next.items[areaKey] = map
        options = next
        preview()
    }

    function conflictRows() {
        const value = plan || null
        const conflicts = value && value.conflicts && typeof value.conflicts === "object" ? value.conflicts : ({})
        const rows = []
        const areas = ["favorites", "idl_registry"]
        for (let i = 0; i < areas.length; ++i) {
            const area = areas[i]
            const areaRows = Array.isArray(conflicts[area]) ? conflicts[area] : []
            for (let j = 0; j < areaRows.length; ++j) {
                rows.push(areaRows[j])
            }
        }
        return rows
    }

    function conflictDecision(area, key) {
        const areaKey = String(area || "")
        const itemKey = String(key || "")
        const conflicts = options && options.conflicts && options.conflicts[areaKey]
        if (!conflicts || typeof conflicts !== "object") {
            return "required"
        }
        return String(conflicts[itemKey] || "required")
    }

    function conflictDecisionIndexFor(area, key, optionsModel) {
        const selected = conflictDecision(area, key)
        for (let i = 0; i < optionsModel.count; ++i) {
            if (String(optionsModel.get(i).key || "") === selected) {
                return i
            }
        }
        return 0
    }

    function setConflictDecision(area, key, decision) {
        const areaKey = String(area || "")
        const itemKey = String(key || "")
        const next = copyOptions()
        if (!next.conflicts) {
            next.conflicts = {}
        }
        const areaMap = next.conflicts[areaKey] && typeof next.conflicts[areaKey] === "object"
            ? next.conflicts[areaKey]
            : ({})
        if (String(decision || "") === "required") {
            delete areaMap[itemKey]
        } else {
            areaMap[itemKey] = String(decision || "required")
        }
        next.conflicts[areaKey] = areaMap
        options = next
        preview()
    }

    function hasRequiredConflicts() {
        const rows = conflictRows()
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i] || {}
            if (conflictDecision(String(row.area || ""), String(row.key || "")) === "required") {
                return true
            }
        }
        return false
    }

    function modeIndexFor(area, optionsModel) {
        const selected = String((options || {})[String(area || "")] || "skip")
        for (let i = 0; i < optionsModel.count; ++i) {
            if (String(optionsModel.get(i).key || "") === selected) {
                return i
            }
        }
        return 0
    }

    function modeAt(index, optionsModel) {
        const row = optionsModel.get(Math.max(0, Math.min(optionsModel.count - 1, Number(index || 0)))) || {}
        return String(row.key || "skip")
    }

    function preview() {
        if (!backupId.length) {
            plan = null
            planError = qsTr("Backup id is required.")
            return null
        }
        planError = ""
        const value = model.previewLocalSettingsImportPlan(backupId, copyOptions())
        plan = value
        if (!value) {
            planError = model.backupCatalogError.length
                ? model.backupCatalogError
                : qsTr("Import plan is unavailable.")
        }
        return value
    }

    function planText() {
        if (planError.length) {
            return planError
        }
        const value = plan || null
        if (!value) {
            return ""
        }
        const selected = selectedAreas()
        if (selected.length === 0) {
            return qsTr("No sections selected.")
        }
        const parts = []
        if (value.settings === true) {
            parts.push(qsTr("settings"))
        }
        if (Number(value.favorites || 0) > 0) {
            parts.push(qsTr("%1 favorites").arg(Number(value.favorites || 0)))
        }
        if (value.idls === true) {
            parts.push(qsTr("%1 IDLs").arg(Number(value.idl_count || 0)))
        }
        if (value.wallet === true) {
            parts.push(qsTr("wallet profile"))
        }
        const lines = []
        lines.push(parts.length
            ? qsTr("Will import %1.").arg(parts.join(", "))
            : qsTr("Selected sections have no importable data."))
        const modeDescription = modeText()
        if (modeDescription.length > 0) {
            lines.push(modeDescription)
        }
        const operationDescription = operationText(value)
        if (operationDescription.length > 0) {
            lines.push(operationDescription)
        }
        const warningDescription = warningText(value)
        if (warningDescription.length > 0) {
            lines.push(warningDescription)
        }
        if (hasRequiredConflicts()) {
            lines.push(qsTr("Resolve import conflicts before applying."))
        }
        if (value.blocked === true) {
            lines.push(qsTr("Import is blocked until affected operations finish or sections change."))
        }
        return lines.join("\n")
    }

    function confirmEnabled() {
        return backupId.length > 0
            && plan !== null
            && planError.length === 0
            && plan.blocked !== true
            && !hasRequiredConflicts()
            && selectedAreas().length > 0
    }

    function modeText() {
        const current = copyOptions()
        const rows = []
        appendMode(rows, qsTr("Settings"), current.settings)
        appendMode(rows, qsTr("Favorites"), current.favorites)
        appendMode(rows, qsTr("IDL Registry"), current.idl_registry)
        appendMode(rows, qsTr("Wallet Profile"), current.wallet_profile)
        return rows.length ? qsTr("Modes: %1.").arg(rows.join("; ")) : ""
    }

    function appendMode(rows, label, mode) {
        const value = modeLabel(mode)
        if (value.length > 0) {
            rows.push(qsTr("%1 %2").arg(label).arg(value))
        }
    }

    function modeLabel(mode) {
        switch (String(mode || "skip")) {
        case "replace":
            return qsTr("replace")
        case "merge":
            return qsTr("merge")
        default:
            return qsTr("not import")
        }
    }

    function operationText(value) {
        const decisions = value && Array.isArray(value.operation_decisions) ? value.operation_decisions : []
        if (decisions.length === 0) {
            return qsTr("Affected operations: none.")
        }
        const rows = []
        for (let i = 0; i < decisions.length; ++i) {
            if (model && typeof model.backupImportDecisionSummaryText === "function") {
                rows.push(model.backupImportDecisionSummaryText(decisions[i]))
            }
        }
        return rows.length ? qsTr("Affected operations:\n%1").arg(rows.join("\n")) : ""
    }

    function warningText(value) {
        const warnings = value && Array.isArray(value.warnings) ? value.warnings : []
        if (warnings.length === 0) {
            return ""
        }
        const rows = []
        for (let i = 0; i < warnings.length; ++i) {
            const warning = warnings[i] && typeof warnings[i] === "object" ? warnings[i] : ({})
            const message = String(warning.message || warning.detail || "")
            if (message.length > 0) {
                rows.push(message)
            }
        }
        return rows.length ? qsTr("Warnings:\n%1").arg(rows.join("\n")) : ""
    }

    function selectedAreas() {
        const current = copyOptions()
        const areas = ["settings", "favorites", "idl_registry", "wallet_profile"]
        const selected = []
        for (let i = 0; i < areas.length; ++i) {
            const area = areas[i]
            const mode = String(current[area] || "skip")
            if (mode !== "skip" && mode !== "none" && mode !== "not_import" && mode !== "not import") {
                selected.push(area)
            }
        }
        return selected
    }
}
