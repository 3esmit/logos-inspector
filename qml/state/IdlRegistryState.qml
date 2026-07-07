import QtQml
import QtQml.Models
import "../services/BridgeHelpers.js" as BridgeHelpers

QtObject {
    id: root

    required property var gateway
    property ListModel registeredIdls: ListModel {}
    property bool loaded: false

    function load(value) {
        loaded = true
        registeredIdls.clear()
        const idls = value && Array.isArray(value.idls) ? value.idls : []
        for (let i = 0; i < idls.length; ++i) {
            const entry = normalizedEntry(idls[i], registeredIdls.count)
            if (entry !== null && entry.json.length) {
                registeredIdls.append(entry)
            }
        }
    }

    function entries() {
        const rows = []
        for (let i = 0; i < registeredIdls.count; ++i) {
            rows.push(entryAt(i))
        }
        return rows
    }

    function normalizedEntry(entry, fallbackIndex) {
        const row = entry || {}
        const json = String(row.json || "")
        const name = String(row.name || nameFromJson(json) || qsTr("IDL %1").arg(Number(fallbackIndex || 0) + 1))
        const programId = String(row.programId || row.program_id || "")
        const programIdHex = String(row.programIdHex || row.program_id_hex || gateway.canonicalProgramIdHex(programId))
        return {
            key: String(row.key || key(name, programIdHex, json)),
            name: name,
            programId: programId,
            programIdHex: programIdHex,
            programBinary: String(row.programBinary || row.program_binary || ""),
            json: json,
            source: String(row.source || ""),
            sharedTopic: String(row.sharedTopic || row.shared_topic || ""),
            sharedIdentity: row.sharedIdentity || row.shared_identity || ({}),
            sharedAccountId: String(row.sharedAccountId || row.shared_account_id || "")
        }
    }

    function entryAt(index) {
        if (index < 0 || index >= registeredIdls.count) {
            return { key: "", name: "", programId: "", programIdHex: "", programBinary: "", json: "" }
        }
        const row = registeredIdls.get(index)
        return normalizedEntry(row, index)
    }

    function nameFromJson(json) {
        const parsed = BridgeHelpers.parseJson(String(json || ""))
        return parsed.ok && parsed.value && parsed.value.name ? String(parsed.value.name) : ""
    }

    function key(name, programId, json) {
        const text = String(name || "") + "\n" + String(programId || "") + "\n" + String(json || "")
        let hash = 2166136261
        for (let i = 0; i < text.length; ++i) {
            hash ^= text.charCodeAt(i)
            hash = Math.imul(hash, 16777619)
        }
        return (hash >>> 0).toString(16)
    }

    function entryForKey(entryKey) {
        const text = String(entryKey || "")
        if (!text.length) {
            return null
        }
        for (let i = 0; i < registeredIdls.count; ++i) {
            const entry = entryAt(i)
            if (entry.key === text) {
                return entry
            }
        }
        return null
    }

    function entriesForProgram(programId) {
        const normalizedProgram = gateway.canonicalProgramIdHex(programId) || gateway.normalizedHexText(programId)
        if (!normalizedProgram.length) {
            return []
        }
        const rows = []
        for (let i = 0; i < registeredIdls.count; ++i) {
            const entry = entryAt(i)
            const entryProgram = String(entry.programIdHex || "") || gateway.canonicalProgramIdHex(entry.programId) || gateway.normalizedHexText(entry.programId)
            if (entryProgram === normalizedProgram) {
                rows.push(entry)
            }
        }
        rows.sort(function (left, right) {
            const leftShared = String(left.source || "") === "shared"
            const rightShared = String(right.source || "") === "shared"
            if (leftShared === rightShared) {
                return String(left.name || "").localeCompare(String(right.name || ""))
            }
            return leftShared ? -1 : 1
        })
        return rows
    }
}
