function navTreeItems(root) {
    with (root) {
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
                    { key: "zones", view: "zones", label: qsTr("Zones"), token: "ZON", layer: "l1" },
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
    const tree = navTreeItems(root)
    for (let i = 0; i < tree.length; ++i) {
        const item = tree[i]
        const children = item.children || []
        for (let j = 0; j < children.length; ++j) {
            if (String(children[j].view || "") === target) {
                return item.key
            }
        }
    }
    return ""
}

function navItemForView(root, view) {
    with (root) {
        const target = String(view || "")
        const tree = navTreeItems(root)
        for (let i = 0; i < tree.length; ++i) {
            const item = tree[i]
            if (String(item.view || "") === target) {
                return item
            }
            const children = item.children || []
            for (let j = 0; j < children.length; ++j) {
                if (String(children[j].view || "") === target) {
                    return children[j]
                }
            }
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
    const tree = navTreeItems(root)
    for (let i = 0; i < tree.length; ++i) {
        const item = tree[i]
        if (navItemMatches(item, normalized)) {
            return item
        }
        const children = item.children || []
        for (let j = 0; j < children.length; ++j) {
            if (navItemMatches(children[j], normalized)) {
                return children[j]
            }
        }
    }
    return null
}

function navItemMatches(item, normalized) {
    const key = String(item.key || "").toLowerCase()
    const view = String(item.view || "").toLowerCase()
    const label = String(item.label || "").toLowerCase()
    return normalized === key || normalized === view || normalized === label
}

function viewTitle(root) {
    const item = navItemForView(root, root.currentView)
    if (item) {
        return item.label
    }
    return qsTr("Dashboard")
}

function normalizedNavigationView(requestedView) {
    return String(requestedView || "")
}
