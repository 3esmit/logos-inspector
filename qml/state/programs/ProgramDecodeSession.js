.import "../../services/BridgeHelpers.js" as BridgeHelpers

function autoDecodeAccountData(root, dataHex, accountId, ownerProgramId, callback) {
    with (root) {
        const serial = accountAutoDecodeSerial + 1
        accountAutoDecodeSerial = serial
        const candidates = accountDecodeCandidates(root, accountId, ownerProgramId)
        if (!String(dataHex || "").length || candidates.length === 0) {
            callback({ ok: false, error: "", value: null, entry: null })
            return serial
        }

        root.resolveAccountDecodeSessionAsync(String(dataHex || ""), accountId, programDecodeCandidatePayload(root, candidates), function (response) {
            if (serial !== accountAutoDecodeSerial) {
                return
            }
            const session = response && response.ok === true && response.value ? response.value : null
            const selected = session && session.selected ? session.selected : null
            if (selected && selected.report) {
                callback({
                    ok: true,
                    error: "",
                    value: selected.report,
                    entry: decodeSelectionEntry(root, selected, candidates),
                    accountType: selected.report.account_type || (selected.evidence ? selected.evidence.accountType : "")
                })
                return
            }
            callback({
                ok: false,
                error: session && session.firstError ? String(session.firstError) : String(response && response.error ? response.error : ""),
                value: null,
                entry: null
            })
        })
        return serial
    }
}

function accountDecodeCandidates(root, accountId, ownerProgramId) {
    with (root) {
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
            if (!candidateListHasEntry(root, candidates, ownerEntry.key)) {
                candidates.push({
                    entry: ownerEntry,
                    accountType: "",
                    cached: false,
                    ownerMatched: true
                })
            }
        }
        if (cached && String(cached.source || "") === "shared" && !candidateListHasEntry(root, candidates, cached.key)) {
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
            if (!candidateListHasEntry(root, candidates, sharedEntry.key)) {
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
}

function cachedIdlEntryForAccount(root, accountId, ownerProgramId) {
    with (root) {
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
}

function cachedAccountType(root, accountId, ownerProgramId) {
    with (root) {
        const selection = root.accountIdlSelection(accountId, ownerProgramId)
        return selection ? String(selection.accountType || "") : ""
    }
}

function accountDecodeFullyConsumed(root, value) {
    if (!value) {
        return false
    }
    const consumed = Number(value.consumed_bytes)
    const total = Number(value.total_bytes)
    const remaining = Number(value.remaining_bytes || 0)
    return Number.isFinite(consumed) && Number.isFinite(total) && consumed === total && remaining === 0
}

function tryAccountDecodeCandidate(root, serial, dataHex, candidates, index, firstError, callback) {
    with (root) {
        if (serial !== accountAutoDecodeSerial) {
            return
        }
        const remaining = Array.isArray(candidates) ? candidates.slice(Math.max(0, Number(index || 0))) : []
        if (remaining.length === 0) {
            callback({ ok: false, error: firstError, value: null, entry: null })
            return
        }
        root.resolveAccountDecodeSessionAsync(String(dataHex || ""), "", programDecodeCandidatePayload(root, remaining), function (response) {
            if (serial !== accountAutoDecodeSerial) {
                return
            }
            const session = response && response.ok === true && response.value ? response.value : null
            const selected = session && session.selected ? session.selected : null
            if (selected && selected.report) {
                callback({
                    ok: true,
                    error: "",
                    value: selected.report,
                    entry: decodeSelectionEntry(root, selected, remaining),
                    accountType: selected.report.account_type || (selected.evidence ? selected.evidence.accountType : "")
                })
                return
            }
            callback({
                ok: false,
                error: String(firstError || "") || (session && session.firstError ? String(session.firstError) : String(response && response.error ? response.error : "")),
                value: null,
                entry: null
            })
        })
    }
}

function autoDecodeTransactionDetail(root, detail) {
    with (root) {
        const summary = transactionSummaryFromDetail(root, detail)
        if (!summary || String(summary.kind || "") !== "Public" || !Array.isArray(summary.instruction_data) || summary.instruction_data.length === 0) {
            return
        }

        const serial = transactionAutoDecodeSerial + 1
        transactionAutoDecodeSerial = serial
        const candidates = transactionDecodeCandidates(root, summary)
        if (candidates.length === 0) {
            return
        }

        root.resolveTransactionDecodeSessionAsync(summary, programDecodeCandidatePayload(root, candidates), function (response) {
            if (serial !== transactionAutoDecodeSerial) {
                return
            }
            const report = transactionDecodeSessionReport(root, response)
            if (report) {
                transactionDetailValue = report
                lezTransactionsPageError = ""
                setResult(qsTr("Transaction"), BridgeHelpers.formatValue(report), false, report, "l2TransactionDetail")
            }
        })
    }
}

function transactionDecodeCandidates(root, summary) {
    with (root) {
        const candidates = []
        const accountIds = Array.isArray(summary.account_ids) ? summary.account_ids : []
        for (let i = 0; i < accountIds.length; ++i) {
            const cached = cachedIdlEntryForAccount(root, accountIds[i], summary.program_id_hex)
            if (cached && !candidateListHasEntry(root, candidates, cached.key)) {
                candidates.push({
                    entry: cached,
                    cached: true
                })
            }
        }

        const programEntries = root.idlEntriesForProgram(summary.program_id_hex)
        for (let j = 0; j < programEntries.length; ++j) {
            if (!candidateListHasEntry(root, candidates, programEntries[j].key)) {
                candidates.push({
                    entry: programEntries[j],
                    cached: false
                })
            }
        }

        return uniqueCandidates(candidates)
    }
}

function transactionDecodeFullyConsumed(root, value) {
    const decoded = transactionDecodedInstruction(root, value)
    return decoded !== null && !decoded.decode_error && Array.isArray(decoded.remaining_words) && decoded.remaining_words.length === 0
}

function transactionDecodedInstruction(root, value) {
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

function transactionSummaryFromDetail(root, value) {
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

function transactionDecodeSessionReport(root, response) {
    const session = response && response.ok === true && response.value ? response.value : null
    const selection = session && session.selected ? session.selected : (session && session.partial ? session.partial : null)
    return selection && selection.report ? selection.report : null
}

function transactionDecodeSessionInstruction(root, response) {
    const report = transactionDecodeSessionReport(root, response)
    return report ? transactionDecodedInstruction(root, report) : null
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

function candidateListHasEntry(root, candidates, key) {
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

function tryTransactionDecodeCandidate(root, serial, summary, candidates, index, partialValue) {
    with (root) {
        if (serial !== transactionAutoDecodeSerial) {
            return
        }
        const remaining = Array.isArray(candidates) ? candidates.slice(Math.max(0, Number(index || 0))) : []
        if (remaining.length === 0) {
            if (partialValue) {
                transactionDetailValue = partialValue
                lezTransactionsPageError = ""
                setResult(qsTr("Transaction"), BridgeHelpers.formatValue(partialValue), false, partialValue, "l2TransactionDetail")
            }
            return
        }

        root.resolveTransactionDecodeSessionAsync(summary, programDecodeCandidatePayload(root, remaining), function (response) {
            if (serial !== transactionAutoDecodeSerial) {
                return
            }
            const report = transactionDecodeSessionReport(root, response)
            if (report) {
                transactionDetailValue = report
                lezTransactionsPageError = ""
                setResult(qsTr("Transaction"), BridgeHelpers.formatValue(report), false, report, "l2TransactionDetail")
                return
            }
            if (partialValue) {
                transactionDetailValue = partialValue
                lezTransactionsPageError = ""
                setResult(qsTr("Transaction"), BridgeHelpers.formatValue(partialValue), false, partialValue, "l2TransactionDetail")
            }
        })
    }
}

function programDecodeCandidatePayload(root, candidates) {
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
            source: String(entry.source || "")
        })
    }
    return rows
}

function decodeSelectionEntry(root, selection, candidates) {
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
