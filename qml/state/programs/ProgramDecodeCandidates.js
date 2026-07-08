.pragma library

function accountDecodeCandidates(root, accountId, ownerProgramId) {
    const candidates = []
    const cached = cachedIdlEntryForAccount(root, accountId, ownerProgramId)
    if (cached && String(cached.source || "") !== "shared") {
        candidates.push({
            entry: cached,
            accountType: cachedAccountType(root, accountId, ownerProgramId),
            cached: true
        })
    }
    const ownerEntries = root.idlEntriesForProgram(ownerProgramId)
    for (let ownerIndex = 0; ownerIndex < ownerEntries.length; ++ownerIndex) {
        const ownerEntry = ownerEntries[ownerIndex]
        if (!candidateListHasEntry(candidates, ownerEntry.key)) {
            candidates.push({
                entry: ownerEntry,
                accountType: "",
                cached: false,
                ownerMatched: true
            })
        }
    }
    if (cached && String(cached.source || "") === "shared" && !candidateListHasEntry(candidates, cached.key)) {
        candidates.push({
            entry: cached,
            accountType: cachedAccountType(root, accountId, ownerProgramId),
            cached: true,
            shared: true
        })
    }
    const sharedEntries = root.sharedIdlEntriesForAccount(accountId, ownerProgramId)
    for (let sharedIndex = 0; sharedIndex < sharedEntries.length; ++sharedIndex) {
        const sharedEntry = sharedEntries[sharedIndex]
        if (!candidateListHasEntry(candidates, sharedEntry.key)) {
            candidates.push({
                entry: sharedEntry,
                accountType: String(sharedEntry.accountType || ""),
                cached: false,
                shared: true
            })
        }
    }
    return uniqueCandidates(candidates)
}

function cachedIdlEntryForAccount(root, accountId, ownerProgramId) {
    const selection = root.accountIdlSelection(accountId, ownerProgramId)
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

function cachedAccountType(root, accountId, ownerProgramId) {
    const selection = root.accountIdlSelection(accountId, ownerProgramId)
    return selection ? String(selection.accountType || "") : ""
}

function transactionDecodeCandidates(root, summary) {
    const candidates = []
    const accountIds = Array.isArray(summary.account_ids) ? summary.account_ids : []
    for (let i = 0; i < accountIds.length; ++i) {
        const cached = cachedIdlEntryForAccount(root, accountIds[i], summary.program_id_hex)
        if (cached && !candidateListHasEntry(candidates, cached.key)) {
            candidates.push({
                entry: cached,
                cached: true
            })
        }
    }

    const programEntries = root.idlEntriesForProgram(summary.program_id_hex)
    for (let j = 0; j < programEntries.length; ++j) {
        if (!candidateListHasEntry(candidates, programEntries[j].key)) {
            candidates.push({
                entry: programEntries[j],
                cached: false
            })
        }
    }

    return uniqueCandidates(candidates)
}

function uniqueCandidates(candidates) {
    const rows = []
    const seen = ({})
    const list = Array.isArray(candidates) ? candidates : []
    for (let i = 0; i < list.length; ++i) {
        const candidate = list[i] || {}
        const entry = candidate.entry || candidate
        const key = String(entry.key || "")
        if (key.length && seen[key] === true) {
            continue
        }
        if (key.length) {
            seen[key] = true
        }
        rows.push(candidate)
    }
    return rows
}

function candidateListHasEntry(candidates, key) {
    const text = String(key || "")
    const rows = Array.isArray(candidates) ? candidates : []
    for (let i = 0; i < rows.length; ++i) {
        const candidate = rows[i] || {}
        const entry = candidate.entry || candidate
        if (String(entry.key || "") === text) {
            return true
        }
    }
    return false
}

function programDecodeCandidatePayload(candidates) {
    const rows = []
    const list = Array.isArray(candidates) ? candidates : []
    for (let i = 0; i < list.length; ++i) {
        const candidate = list[i] || {}
        const entry = candidate.entry || candidate
        const json = String(entry.json || "")
        if (!json.length) {
            continue
        }
        rows.push({
            key: String(entry.key || ""),
            name: String(entry.name || ""),
            programIdHex: String(entry.programIdHex || entry.program_id_hex || ""),
            json: json,
            accountType: String(candidate.accountType || entry.accountType || entry.account_type || ""),
            source: String(entry.source || ""),
            cached: candidate.cached === true,
            shared: candidate.shared === true || String(entry.source || "") === "shared",
            ownerMatched: candidate.ownerMatched === true
        })
    }
    return rows
}

function decodeSelectionEntry(selection, candidates) {
    const evidence = selection && selection.evidence ? selection.evidence : ({})
    const key = String(evidence.key || "")
    const list = Array.isArray(candidates) ? candidates : []
    for (let i = 0; i < list.length; ++i) {
        const entry = list[i] && list[i].entry ? list[i].entry : list[i]
        if (entry && key.length > 0 && String(entry.key || "") === key) {
            return entry
        }
    }
    return {
        key: key,
        name: String(evidence.name || ""),
        programIdHex: String(evidence.programIdHex || evidence.program_id_hex || ""),
        source: String(evidence.source || "")
    }
}
