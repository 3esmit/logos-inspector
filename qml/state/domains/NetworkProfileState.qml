import QtQml

QtObject {
    id: root

    property var sourcePolicy: null

    function normalizedProfile(value) {
        const key = String(value || "default").trim()
        if (key === "custom") {
            return key
        }
        const rows = profileRows()
        for (let i = 0; i < rows.length; ++i) {
            if (String(rows[i].id || "") === key) {
                return key
            }
        }
        return "default"
    }

    function resolvedProfile(storedProfile, node) {
        const inferred = inferProfile(node)
        if (inferred !== "custom") {
            return inferred
        }
        return normalizedProfile(storedProfile) === "custom" ? "custom" : inferred
    }

    function inferProfile(node) {
        const endpoint = normalizeEndpoint(node)
        const rows = profileRows()
        for (let i = 0; i < rows.length; ++i) {
            if (endpoint === normalizeEndpoint(rows[i].node_endpoint)) {
                return String(rows[i].id || "default")
            }
        }
        return "custom"
    }

    function normalizeEndpoint(value) {
        return String(value || "").trim().replace(/\/+$/, "")
    }

    function profileRows() {
        const source = sourcePolicy && Array.isArray(sourcePolicy.network_profiles)
            ? sourcePolicy.network_profiles : fallbackProfileRows()
        const rows = []
        const seen = ({})
        for (let i = 0; i < source.length; ++i) {
            const row = source[i] || ({})
            const endpoint = String(row.node_endpoint || "")
            const key = normalizeEndpoint(endpoint)
            if (!key.length || seen[key] === true) {
                continue
            }
            seen[key] = true
            rows.push({
                id: String(row.id || (rows.length === 0 ? "default" : "profile-" + rows.length)),
                label: String(row.label || row.id || qsTr("Network")),
                node_endpoint: endpoint
            })
        }
        return rows.length > 0 ? rows : fallbackProfileRows()
    }

    function optionRows() {
        const rows = profileRows().map(function (profile) {
            return {
                key: String(profile.id || ""),
                label: String(profile.label || profile.id || ""),
                summary: qsTr("Bedrock node profile")
            }
        })
        rows.push({ key: "custom", label: qsTr("Custom"), summary: qsTr("Manual L1 endpoint") })
        return rows
    }

    function fallbackProfileRows() {
        return [{
            id: "default",
            label: qsTr("Default"),
            node_endpoint: sourcePolicyDefault("node_endpoint", "http://127.0.0.1:8080/")
        }]
    }

    function sourcePolicyDefault(key, fallback) {
        const defaults = sourcePolicy && sourcePolicy.defaults && typeof sourcePolicy.defaults === "object"
            ? sourcePolicy.defaults : ({})
        const value = defaults[String(key || "")]
        return value === undefined || value === null || String(value).length === 0
            ? String(fallback || "") : String(value)
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
        const rows = profileRows()
        for (let i = 0; i < rows.length; ++i) {
            if (String(rows[i].id || "") === key) {
                return {
                    profile: key,
                    nodeUrl: String(rows[i].node_endpoint || "")
                }
            }
        }
        return null
    }

    function settingsFromPayload(value, currentProfile, currentNode) {
        const profile = normalizedProfile(stringValue(value, "network_profile", currentProfile))
        const node = stringValue(value, "node_url", currentNode)
        return {
            profile: resolvedProfile(profile, node),
            nodeUrl: node
        }
    }

    function settingsPayload(profile, node) {
        return {
            network_profile: inferProfile(node),
            node_url: String(node || "")
        }
    }

    function cacheScope(profile, node) {
        return [String(profile || ""), normalizeEndpoint(node)].join("|")
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
        return qsTr("Default")
    }

    function profileSummary(profile) {
        return normalizedProfile(profile) === "custom"
            ? qsTr("Manual L1 endpoint") : qsTr("Bedrock node profile")
    }

    function profileDetail(node) {
        return shortEndpoint(node)
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
