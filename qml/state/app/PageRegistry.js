function navTreeItems(root) {
    with (root) {
        const sequencerChildren = zoneInspection
            && zoneInspection.l2
            && zoneInspection.l2.l2SequencerConfigured === true
            ? [{
                key: "sequencerDashboard",
                view: "sequencerDashboard",
                label: qsTr("Sequencer"),
                token: "SEQ",
                layer: "l2"
            }] : []
        return [
            { type: "item", key: "overview", view: "overview", label: qsTr("Dashboard"), token: "DAS", layer: "system" },
            {
                type: "group",
                key: "l1",
                label: qsTr("L1 Bedrock"),
                token: "L1",
                layer: "l1",
                children: [
                    { key: "blocks", view: "blocks", label: qsTr("Blocks"), token: "L1B", layer: "l1" },
                    { key: "transactions", view: "transactions", label: qsTr("Mantle Tx"), token: "L1T", layer: "l1" },
                    {
                        key: "zones",
                        view: "zones",
                        label: qsTr("Zones"),
                        token: "ZON",
                        layer: "l1",
                        children: sequencerChildren
                    },
                    { key: "blockchain", view: "blockchain", label: qsTr("Node / Module"), token: "L1N", layer: "l1" }
                ]
            },
            {
                type: "group",
                key: "network",
                label: qsTr("Network"),
                token: "NET",
                layer: "module",
                children: [
                    { key: "storage", view: "storage", label: qsTr("Storage"), token: "STO", layer: "module" },
                    { key: "messaging", view: "messaging", label: qsTr("Delivery"), token: "DLV", layer: "module" }
                ]
            },
            {
                type: "group",
                key: "diagnostics",
                label: qsTr("Diagnostics"),
                token: "DIA",
                layer: "system",
                children: [
                    { key: "storageDiagnostics", view: "diagnosticsStorage", label: qsTr("Storage"), token: "DST", layer: "system" },
                    { key: "deliveryDiagnostics", view: "diagnosticsDelivery", label: qsTr("Delivery"), token: "DDL", layer: "system" },
                    { key: "capabilities", view: "capabilities", label: qsTr("Capabilities"), token: "CAP", layer: "system" }
                ]
            },
            {
                type: "group",
                key: "local",
                label: qsTr("Local"),
                token: "LOC",
                layer: "local",
                children: [
                    { key: "favorites", view: "favorites", label: qsTr("Favorites"), token: "FAV", layer: "local" },
                    { key: "programs", view: "programs", label: qsTr("Program / IDL"), token: "IDL", layer: "local" },
                    { key: "localWallet", view: "localWallet", label: qsTr("Wallet"), token: "WAL", layer: "local" }
                ]
            },
            {
                type: "group",
                key: "system",
                label: qsTr("System"),
                token: "SYS",
                layer: "system",
                children: [
                    { key: "localNodes", view: "localNodes", label: qsTr("Local Nodes"), token: "NOD", layer: "system" },
                    { key: "settings", view: "settings", label: qsTr("Settings"), token: "SET", layer: "system" }
                ]
            }
        ]
    }
}

function parentNavKeyForView(root, view) {
    const target = String(view || "")
    if (target === "blockDetail" || target === "transactionDetail") {
        return "l1"
    }
    const path = navPathForView(root, target)
    if (path.length < 2) {
        return ""
    }
    return String(path[0].key || "")
}

function ancestorNavKeysForView(root, view) {
    const path = navPathForView(root, view)
    const keys = []
    for (let i = 0; i < path.length - 1; ++i) {
        if (String(path[i].type || "") === "group") {
            keys.push(String(path[i].key || ""))
        }
    }
    return keys
}

function navItemForView(root, view) {
    with (root) {
        const target = String(view || "")
        const path = navPathForView(root, target)
        if (path.length > 0) {
            return path[path.length - 1]
        }
        if (target === "blockDetail") {
            return { key: "blockDetail", view: "blockDetail", label: qsTr("Block"), token: "L1B", layer: "l1" }
        }
        if (target === "transactionDetail") {
            return { key: "transactionDetail", view: "transactionDetail", label: qsTr("Mantle Tx"), token: "L1T", layer: "l1" }
        }
        return null
    }
}

function layerForView(root, view) {
    const item = navItemForView(root, view)
    return item ? String(item.layer || "") : ""
}

function navLabelForView(root, view) {
    const item = navItemForView(root, view)
    return item ? String(item.label || "") : ""
}

function navTokenForView(root, view) {
    const item = navItemForView(root, view)
    return item ? String(item.token || "") : ""
}

function navItemForQuery(root, query) {
    const normalized = String(query || "").trim().toLowerCase()
    return navItemForQueryIn(navTreeItems(root), normalized)
}

function navItemMatches(item, normalized) {
    const key = String(item.key || "").toLowerCase()
    const view = String(item.view || "").toLowerCase()
    const label = String(item.label || "").toLowerCase()
    return normalized === key || normalized === view || normalized === label
}

function viewTitle(root) {
    const item = navItemForView(root, root.shell.currentView)
    if (item) {
        return item.label
    }
    return qsTr("Dashboard")
}

function normalizedNavigationView(requestedView) {
    return String(requestedView || "")
}

function navPathForView(root, view) {
    return navPathForViewIn(navTreeItems(root), String(view || ""), [])
}

function navPathForViewIn(items, target, ancestors) {
    const values = Array.isArray(items) ? items : []
    for (let i = 0; i < values.length; ++i) {
        const item = values[i]
        const path = ancestors.concat([item])
        if (String(item.view || "") === target) {
            return path
        }
        const nested = navPathForViewIn(item.children || [], target, path)
        if (nested.length > 0) {
            return nested
        }
    }
    return []
}

function navItemForQueryIn(items, normalized) {
    const values = Array.isArray(items) ? items : []
    for (let i = 0; i < values.length; ++i) {
        const item = values[i]
        if (navItemMatches(item, normalized)) {
            return item
        }
        const nested = navItemForQueryIn(item.children || [], normalized)
        if (nested) {
            return nested
        }
    }
    return null
}

function navItemContainsView(item, view) {
    if (!item) {
        return false
    }
    const target = String(view || "")
    if (String(item.view || "") === target) {
        return true
    }
    const children = item.children || []
    for (let i = 0; i < children.length; ++i) {
        if (navItemContainsView(children[i], target)) {
            return true
        }
    }
    return false
}
