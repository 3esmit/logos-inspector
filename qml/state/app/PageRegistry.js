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
                    { key: "channels", view: "channels", label: qsTr("Channels"), token: "L1C", layer: "l1" },
                    { key: "blockchain", view: "blockchain", label: qsTr("Node / Module"), token: "L1N", layer: "l1" }
                ]
            },
            {
                type: "group",
                key: "l2",
                label: qsTr("L2 LEZ"),
                token: "L2",
                layer: "l2",
                children: [
                    { key: "l2Blocks", view: "l2Blocks", label: qsTr("Blocks"), token: "L2B", layer: "l2" },
                    { key: "l2Transactions", view: "l2Transactions", label: qsTr("Transactions"), token: "L2T", layer: "l2" },
                    { key: "accounts", view: "accounts", label: qsTr("Accounts"), token: "ACC", layer: "l2" },
                    { key: "transferActivity", view: "transferActivity", label: qsTr("Transfer Activity"), token: "XFR", layer: "l2" },
                    { key: "programs", view: "programs", label: qsTr("Programs"), token: "PRG", layer: "l2" }
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
                    { key: "indexer", view: "indexer", label: qsTr("LEZ Indexer"), token: "IDX", layer: "system" },
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
    if (target === "blockDetail" || target === "transactionDetail" || target === "zones") {
        return "l1"
    }
    if (target === "l2BlockDetail" || target === "l2TransactionDetail" || target === "sequencer") {
        return "l2"
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
        if (target === "l2BlockDetail") {
            return { key: "l2BlockDetail", view: "l2BlockDetail", label: qsTr("LEZ Block"), token: "L2B", layer: "l2" }
        }
        if (target === "l2TransactionDetail") {
            return { key: "l2TransactionDetail", view: "l2TransactionDetail", label: qsTr("LEZ Transaction"), token: "L2T", layer: "l2" }
        }
        if (target === "zones") {
            return { key: "zones", view: "zones", label: qsTr("Zones"), token: "ZON", layer: "l1" }
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
    const requested = String(requestedView || "")
    return requested === "sequencer" ? "l2Blocks" : requested
}
