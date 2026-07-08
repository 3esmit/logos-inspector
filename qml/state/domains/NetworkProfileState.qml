import QtQml

QtObject {
    id: root

    property var sourcePolicy: null

    function normalizedProfile(value) {
        const profile = String(value || "default").trim()
        if (profile === "local" || profile === "custom") {
            return profile
        }
        return "default"
    }

    function resolvedProfile(storedProfile, sequencer, indexer, node) {
        const inferred = inferProfile(sequencer, indexer, node)
        if (inferred !== "custom") {
            return inferred
        }
        return normalizedProfile(storedProfile) === "custom" ? "custom" : inferred
    }

    function inferProfile(sequencer, indexer, node) {
        const seq = normalizeEndpoint(sequencer)
        const idx = normalizeEndpoint(indexer)
        const nod = normalizeEndpoint(node)
        const profiles = profileRows()
        for (let i = 0; i < profiles.length; ++i) {
            const profile = profiles[i] || {}
            if (seq === normalizeEndpoint(profile.sequencer_endpoint)
                    && idx === normalizeEndpoint(profile.indexer_endpoint)
                    && nod === normalizeEndpoint(profile.node_endpoint)) {
                return String(profile.id || "custom")
            }
        }
        return "custom"
    }

    function normalizeEndpoint(value) {
        return String(value || "").trim().replace(/\/+$/, "")
    }

    function profileRows() {
        if (sourcePolicy && Array.isArray(sourcePolicy.network_profiles)) {
            return sourcePolicy.network_profiles
        }
        return fallbackProfileRows()
    }

    function optionRows() {
        const rows = []
        const profiles = profileRows()
        for (let i = 0; i < profiles.length; ++i) {
            const profile = profiles[i] || {}
            rows.push({
                key: String(profile.id || ""),
                label: String(profile.label || profile.id || ""),
                summary: profileSummary(profile.id || "")
            })
        }
        rows.push({
            key: "custom",
            label: qsTr("Custom"),
            summary: qsTr("Manual endpoint override")
        })
        return rows
    }

    function fallbackProfileRows() {
        return [
            {
                id: "default",
                label: qsTr("Testnet"),
                sequencer_endpoint: sourcePolicyDefault("sequencer_endpoint", "https://testnet.lez.logos.co/"),
                indexer_endpoint: sourcePolicyDefault("indexer_endpoint", "http://127.0.0.1:8779/"),
                node_endpoint: sourcePolicyDefault("node_endpoint", "http://127.0.0.1:8080/")
            },
            {
                id: "local",
                label: qsTr("Local sequencer"),
                sequencer_endpoint: sourcePolicyDefault("local_sequencer_endpoint", "http://127.0.0.1:3040/"),
                indexer_endpoint: sourcePolicyDefault("indexer_endpoint", "http://127.0.0.1:8779/"),
                node_endpoint: sourcePolicyDefault("node_endpoint", "http://127.0.0.1:8080/")
            }
        ]
    }

    function sourcePolicyDefault(key, fallback) {
        const defaults = sourcePolicy && sourcePolicy.defaults && typeof sourcePolicy.defaults === "object"
            ? sourcePolicy.defaults
            : ({})
        const value = defaults[String(key || "")]
        return value === undefined || value === null || String(value).length === 0 ? String(fallback || "") : String(value)
    }

    function profileAt(index) {
        const rows = optionRows()
        const row = rows[Math.max(0, Number(index || 0))] || rows[0] || { key: "default" }
        return String(row.key || "default")
    }

    function profileIndex(profile) {
        const key = normalizedProfile(profile)
        const rows = optionRows()
        for (let i = 0; i < rows.length; ++i) {
            if (String(rows[i].key || "") === key) {
                return i
            }
        }
        return 0
    }

    function applyProfile(profile) {
        const key = normalizedProfile(profile)
        if (key === "custom") {
            return null
        }
        const profiles = profileRows()
        for (let i = 0; i < profiles.length; ++i) {
            const row = profiles[i] || {}
            if (String(row.id || "") === key) {
                return {
                    profile: key,
                    sequencerUrl: String(row.sequencer_endpoint || ""),
                    indexerUrl: String(row.indexer_endpoint || ""),
                    nodeUrl: String(row.node_endpoint || "")
                }
            }
        }
        return key === "default" ? null : applyProfile("default")
    }

    function settingsFromPayload(value, currentProfile, currentSequencer, currentIndexer, currentNode) {
        const profile = normalizedProfile(stringValue(value, "network_profile", currentProfile))
        const sequencer = stringValue(value, "sequencer_url", currentSequencer)
        const indexer = stringValue(value, "indexer_url", currentIndexer)
        const node = stringValue(value, "node_url", currentNode)
        return {
            profile: resolvedProfile(profile, sequencer, indexer, node),
            sequencerUrl: sequencer,
            indexerUrl: indexer,
            nodeUrl: node
        }
    }

    function settingsPayload(profile, sequencer, indexer, node) {
        return {
            network_profile: inferProfile(sequencer, indexer, node),
            sequencer_url: String(sequencer || ""),
            indexer_url: String(indexer || ""),
            node_url: String(node || "")
        }
    }

    function cacheScope(profile, sequencer) {
        return [String(profile || ""), String(sequencer || "")].join("|")
    }

    function localMode(profile) {
        return normalizedProfile(profile) === "local"
    }

    function profileLabel(profile) {
        const key = normalizedProfile(profile)
        const rows = optionRows()
        for (let i = 0; i < rows.length; ++i) {
            if (String(rows[i].key || "") === key) {
                return String(rows[i].label || key)
            }
        }
        return qsTr("Testnet")
    }

    function profileSummary(profile) {
        const key = normalizedProfile(profile)
        if (key === "local") {
            return qsTr("All endpoints local")
        }
        if (key === "custom") {
            return qsTr("Manual endpoints")
        }
        return qsTr("Default testnet")
    }

    function profileDetail(sequencer, indexer, node) {
        return qsTr("%1 / %2 / %3")
            .arg(shortEndpoint(sequencer))
            .arg(shortEndpoint(indexer))
            .arg(shortEndpoint(node))
    }

    function shortEndpoint(value) {
        const text = String(value || "")
        if (text.length <= 36) {
            return text
        }
        return text.slice(0, 18) + "..." + text.slice(text.length - 15)
    }

    function stringValue(value, key, fallback) {
        const raw = value ? value[key] : undefined
        return raw === undefined || raw === null ? String(fallback || "") : String(raw)
    }
}
