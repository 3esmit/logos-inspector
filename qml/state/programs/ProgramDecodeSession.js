.import "../../services/BridgeHelpers.js" as BridgeHelpers
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

function autoDecodeTransactionDetail(root, detail) {
    with (root) {
        const summary = transactionSummaryFromDetail(root, detail)
        if (!summary || String(summary.kind || "") !== "Public" || !Array.isArray(summary.instruction_data) || summary.instruction_data.length === 0) {
            return
        }

        const serial = transactionAutoDecodeSerial + 1
        transactionAutoDecodeSerial = serial
        const candidates = ProgramDecodeCandidates.transactionDecodeCandidates(root, summary)
        if (candidates.length === 0) {
            return
        }

        root.resolveTransactionDecodeSessionAsync(summary, ProgramDecodeCandidates.programDecodeCandidatePayload(candidates), function (response) {
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
    return ProgramDecodeCandidates.transactionDecodeCandidates(root, summary)
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

function candidateListHasEntry(root, candidates, key) {
    return ProgramDecodeCandidates.candidateListHasEntry(candidates, key)
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

        root.resolveTransactionDecodeSessionAsync(summary, ProgramDecodeCandidates.programDecodeCandidatePayload(remaining), function (response) {
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
    return ProgramDecodeCandidates.programDecodeCandidatePayload(candidates)
}

function decodeSelectionEntry(root, selection, candidates) {
    return ProgramDecodeCandidates.decodeSelectionEntry(selection, candidates)
}
