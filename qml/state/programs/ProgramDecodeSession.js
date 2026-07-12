.import "ProgramDecodeCandidates.js" as ProgramDecodeCandidates

function autoDecodeAccountData(root, dataHex, accountId, ownerProgramId, callback) {
    with (root) {
        const serial = accountAutoDecodeSerial + 1
        accountAutoDecodeSerial = serial
        const candidates = ProgramDecodeCandidates.accountDecodeCandidates(root, accountId, ownerProgramId)
        if (!String(dataHex || "").length || candidates.length === 0) {
            callback({ ok: false, error: "", value: null, entry: null })
            return serial
        }

        root.selectAccountDecodeSessionAsync(String(dataHex || ""), accountId, ownerProgramId, ProgramDecodeCandidates.programDecodeCandidatePayload(candidates), function (response) {
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
                    entry: ProgramDecodeCandidates.decodeSelectionEntry(selected, candidates),
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
    return ProgramDecodeCandidates.accountDecodeCandidates(root, accountId, ownerProgramId)
}

function cachedIdlEntryForAccount(root, accountId, ownerProgramId) {
    return ProgramDecodeCandidates.cachedIdlEntryForAccount(root, accountId, ownerProgramId)
}

function cachedAccountType(root, accountId, ownerProgramId) {
    return ProgramDecodeCandidates.cachedAccountType(root, accountId, ownerProgramId)
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
        root.resolveAccountDecodeSessionAsync(String(dataHex || ""), "", ProgramDecodeCandidates.programDecodeCandidatePayload(remaining), function (response) {
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
                    entry: ProgramDecodeCandidates.decodeSelectionEntry(selected, remaining),
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

function programDecodeCandidatePayload(root, candidates) {
    return ProgramDecodeCandidates.programDecodeCandidatePayload(candidates)
}

function decodeSelectionEntry(root, selection, candidates) {
    return ProgramDecodeCandidates.decodeSelectionEntry(selection, candidates)
}
