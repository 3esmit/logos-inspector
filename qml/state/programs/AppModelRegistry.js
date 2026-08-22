.import "../../services/BridgeHelpers.js" as BridgeHelpers

function programIdKnown(root, programId) {
    with (root) {
        const normalized = root.canonicalProgramIdHex(programId) || root.normalizedHexText(programId)
        if (!normalized.length) {
            return false
        }
        for (let i = 0; i < registeredIdls.count; ++i) {
            const entry = root.idlEntryAt(i)
            const entryProgram = String(entry.programIdHex || "") || root.canonicalProgramIdHex(entry.programId) || root.normalizedHexText(entry.programId)
            if (entryProgram === normalized) {
                return true
            }
        }
        const rows = root.knownProgramIdRows()
        for (let j = 0; j < rows.length; ++j) {
            const row = rows[j] || {}
            const rowProgram = String(row.hex || row.programIdHex || "") || root.canonicalProgramIdHex(row.base58 || row.programId || row.program_id)
            if (rowProgram === normalized) {
                return true
            }
        }
        return false
    }
}

function knownProgramCacheScope(root) {
    return root.zoneSourceScopeKey()
}

function knownProgramIdRows(root) {
    with (root) {
        const revision = knownProgramIdsRevision
        const scope = root.knownProgramCacheScope()
        if (!scope.length) {
            return []
        }
        const rows = knownProgramIds[scope]
        return Array.isArray(rows) ? rows : []
    }
}

function updateKnownProgramIds(root, value) {
    with (root) {
        if (!Array.isArray(value)) {
            return
        }
        const scope = root.knownProgramCacheScope()
        if (!scope.length) {
            return
        }
        const rows = []
        for (let i = 0; i < value.length; ++i) {
            const row = value[i] || {}
            const hex = String(row.hex || row.programIdHex || row.program_id_hex || "")
            const base58 = String(row.base58 || row.programId || row.program_id || "")
            const normalized = hex.length ? root.normalizedHexText(hex) : root.canonicalProgramIdHex(base58)
            if (!normalized.length) {
                continue
            }
            rows.push({
                hex: normalized,
                base58: base58,
                label: String(row.label || row.name || "")
            })
        }
        const next = copyMap(knownProgramIds)
        next[scope] = rows
        knownProgramIds = next
        knownProgramIdsRevision += 1
    }
}

function registeredIdlDuplicate(root, name, programIdHex, json, idl) {
    with (root) {
        const expectedName = String(name || "").trim()
        const expectedProgramId = root.normalizedHexText(programIdHex)
        const expectedVersion = idlVersion(idl)
        const expectedJson = String(json || "")
        for (let i = 0; i < registeredIdls.count; ++i) {
            const entry = root.idlEntryAt(i)
            const entryProgramId = root.normalizedHexText(entry.programIdHex)
                || root.canonicalProgramIdHex(entry.programId)
            if (entryProgramId !== expectedProgramId) {
                continue
            }
            const entryJson = String(entry.json || "")
            if (!expectedName.length) {
                if (entryJson === expectedJson) {
                    return entry
                }
                continue
            }
            if (String(entry.name || "") !== expectedName) {
                continue
            }
            const parsedEntry = BridgeHelpers.parseJson(String(entry.json || ""))
            const entryVersion = parsedEntry.ok ? idlVersion(parsedEntry.value) : ""
            if (expectedVersion.length > 0 || entryVersion.length > 0) {
                if (expectedVersion === entryVersion) {
                    return entry
                }
                continue
            }
            if (entryJson === expectedJson) {
                return entry
            }
        }
        return null
    }
}

function idlVersion(idl) {
    if (!idl || typeof idl !== "object" || Array.isArray(idl)) {
        return ""
    }
    if (idl.version !== undefined && idl.version !== null) {
        return String(idl.version).trim()
    }
    const metadata = idl.metadata
    return metadata && typeof metadata === "object" && !Array.isArray(metadata)
        && metadata.version !== undefined && metadata.version !== null
        ? String(metadata.version).trim() : ""
}

function idlLogicalName(idl) {
    if (!idl || typeof idl !== "object" || Array.isArray(idl)) {
        return ""
    }
    const name = String(idl.name || "").trim()
    if (name.length) {
        return name
    }
    const metadata = idl.metadata
    return metadata && typeof metadata === "object" && !Array.isArray(metadata)
        ? String(metadata.name || "").trim() : ""
}

function registerIdl(root, name, programId, json, programBinary) {
    with (root) {
        if (!json.trim().length) {
            shell.setResult(qsTr("IDL registry"), qsTr("IDL JSON is required."), true)
            return
        }

        const parsed = BridgeHelpers.parseJson(json)
        if (!parsed.ok) {
            shell.setResult(qsTr("IDL registry"), qsTr("Invalid IDL JSON: %1").arg(parsed.error), true)
            return
        }

        const idl = parsed.value
        const requestedName = String(name || "").trim()
        const logicalName = requestedName.length ? requestedName : idlLogicalName(idl)
        const resolvedName = logicalName.length
            ? logicalName : qsTr("IDL %1").arg(registeredIdls.count + 1)
        const resolvedProgramId = programId.trim()
        const resolvedProgramIdHex = resolvedProgramId.length ? root.canonicalProgramIdHex(resolvedProgramId) : ""
        if (!resolvedProgramId.length) {
            shell.setResult(qsTr("IDL registry"), qsTr("Program ID is required for automatic decode."), true)
            return
        }
        if (resolvedProgramId.length && !resolvedProgramIdHex.length) {
            shell.setResult(qsTr("IDL registry"), qsTr("Program ID must be hex or base58."), true)
            return
        }
        const duplicate = registeredIdlDuplicate(
            root, logicalName, resolvedProgramIdHex, json, idl)
        if (duplicate !== null) {
            const duplicateName = String(duplicate.name || resolvedName)
            const duplicateLabel = duplicateName.indexOf("IDL ") === 0
                ? duplicateName : qsTr("IDL %1").arg(duplicateName)
            shell.setResult(
                qsTr("IDL registry"),
                qsTr("%1 is already registered for this program.").arg(duplicateLabel),
                true)
            return
        }
        registeredIdls.append({
            key: idlKey(resolvedName, resolvedProgramIdHex, json),
            name: resolvedName,
            programId: resolvedProgramId,
            programIdHex: resolvedProgramIdHex,
            programBinary: String(programBinary || "").trim(),
            json: json,
            source: "local",
            sharedTopic: "",
            sharedIdentity: ({}),
            sharedAccountId: ""
        })
        saveIdlState()
        shell.setResult(qsTr("IDL registry"), qsTr("Saved %1.").arg(resolvedName), false)
    }
}

function removeIdl(root, index) {
    with (root) {
        if (index < 0 || index >= registeredIdls.count) {
            return
        }
        const entry = idlEntryAt(index)
        registeredIdls.remove(index)
        if (entry.key.length) {
            const next = {}
            const current = accountIdlSelections || {}
            for (const accountId in current) {
                if (String(current[accountId].idlKey || "") !== entry.key) {
                    next[accountId] = current[accountId]
                }
            }
            accountIdlSelections = next
            accountIdlSelectionRevision += 1
        }
        saveIdlState()
    }
}
