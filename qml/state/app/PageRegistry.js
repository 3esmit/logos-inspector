function navTreeItems(root) {
    with (root) {
        const configuredZones = configuredZoneNavigationItems(root)
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
                    { key: "blockchain", view: "blockchain", label: qsTr("Node / Module"), token: "L1N", layer: "l1" }
                ]
            },
            {
                type: "group",
                key: "zones",
                label: qsTr("Zones"),
                token: "ZON",
                layer: "l2",
                children: [{
                    key: "zonesCatalog",
                    view: "zones",
                    label: qsTr("Zone Catalog"),
                    token: "CAT",
                    layer: "l2"
                }].concat(configuredZones)
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

function configuredZoneNavigationItems(root) {
    return configuredZoneMenuCandidates(root).filter(function (item) {
        return root.zoneMenuEnabled(String(item.menuKey || ""))
    })
}

function configuredZoneMenuCandidates(root) {
    const state = root && root.zoneInspection ? root.zoneInspection : null
    if (!state || String(state.verification || "") !== "verified"
            || state.summaryStale === true) {
        return []
    }
    const rows = Array.isArray(state.zoneSummaries) ? state.zoneSummaries : []
    const items = []
    const seen = {}
    for (let i = 0; i < rows.length; ++i) {
        const zone = rows[i] || ({})
        const channelId = String(zone.channel_id || "")
        const fields = zone.active_zone_context_fields || ({})
        const link = zone.settlement_link || ({})
        const sequencerSourceId = String(fields.selected_sequencer_source_id
            || link.selected_sequencer_source_id || "")
        const indexerSourceId = String(fields.indexer_source_id
            || link.indexer_source_id || "")
        const menuKey = configuredZoneMenuKey(state, channelId)
        if (String(zone.kind || "") !== "sequencer_zone" || !channelId.length
                || !menuKey.length || seen[channelId]
                || (!sequencerSourceId.length && !indexerSourceId.length)) {
            continue
        }
        seen[channelId] = true
        const label = configuredZoneNavigationLabel(zone)
        items.push({
            key: "zone." + channelId,
            view: sequencerSourceId.length > 0 ? "sequencerDashboard" : "zones",
            channelId: channelId,
            menuKey: menuKey,
            label: label,
            token: "ZON",
            layer: "l2",
            accessibleName: qsTr("Open Zone dashboard for %1").arg(
                configuredZoneNavigationIdentity(zone, label))
        })
    }
    items.sort(function (left, right) {
        return String(left.channelId || "").localeCompare(String(right.channelId || ""))
    })
    return items
}

function configuredZoneMenuKey(state, channelId) {
    const scope = String(state && state.networkScopeKey || "")
    const channel = String(channelId || "").toLowerCase()
    return scope.length > 0 && /^[0-9a-f]{64}$/.test(channel)
        ? "zone:" + scope + ":" + channel : ""
}

function zoneMenuSelectorGroups(root) {
    const items = configuredZoneMenuCandidates(root)
    if (!items.length) {
        return []
    }
    return [{
        title: qsTr("Configured Zones"),
        fields: items.map(function (item) {
            return {
                key: String(item.menuKey || ""),
                label: zoneMenuSelectorLabel(item),
                detail: qsTr("Add this Zone dashboard to the navigation menu. Channel: %1")
                    .arg(String(item.channelId || ""))
            }
        })
    }]
}

function zoneMenuSelectorLabel(item) {
    const label = String(item && item.label || "")
    const channelId = String(item && item.channelId || "")
    const shortId = channelId.length > 12
        ? channelId.slice(0, 6) + "…" + channelId.slice(-6) : channelId
    return label.length > 0 && shortId.length > 0 && label !== shortId
        ? qsTr("%1 · %2").arg(label).arg(shortId) : label
}

function configuredZoneNavigationLabel(zone) {
    const value = zone || ({})
    const display = value.display || ({})
    const alias = String(display.alias || "")
    if (alias.length > 0) {
        return alias
    }
    const title = String(display.title || "")
    if (title.length > 0) {
        return title
    }
    const shortId = String(display.short_channel_id || "")
    if (shortId.length > 0) {
        return shortId
    }
    const channelId = String(value.channel_id || "")
    return channelId.length > 12
        ? channelId.slice(0, 6) + "…" + channelId.slice(-6) : channelId
}

function configuredZoneNavigationIdentity(zone, label) {
    const channelId = String(zone && zone.channel_id || "")
    const name = String(label || "")
    return channelId.length > 0 && name.length > 0 && name !== channelId
        ? qsTr("%1 (%2)").arg(name).arg(channelId) : name
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
        const activeChannelId = String(zoneInspection && zoneInspection.activeZoneId || "")
        const activeItem = navItemForViewAndChannel(navTreeItems(root), target,
            activeChannelId)
        if (activeItem) {
            return activeItem
        }
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

function navItemForViewAndChannel(items, view, activeChannelId) {
    const values = Array.isArray(items) ? items : []
    const target = String(view || "")
    const channelId = String(activeChannelId || "")
    for (let i = 0; i < values.length; ++i) {
        const item = values[i] || ({})
        if (String(item.view || "") === target
                && String(item.channelId || "") === channelId
                && channelId.length > 0) {
            return item
        }
        const nested = navItemForViewAndChannel(item.children || [], target,
            channelId)
        if (nested) {
            return nested
        }
    }
    return null
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

function navItemContainsView(item, view, activeChannelId) {
    if (!item) {
        return false
    }
    if (navItemIsActive(item, view, activeChannelId)) {
        return true
    }
    const children = item.children || []
    for (let i = 0; i < children.length; ++i) {
        if (navItemContainsView(children[i], view, activeChannelId)) {
            return true
        }
    }
    return false
}

function navItemIsActive(item, view, activeChannelId) {
    const value = item || ({})
    if (String(value.view || "") !== String(view || "")) {
        return false
    }
    const itemChannelId = String(value.channelId || "")
    return !itemChannelId.length || itemChannelId === String(activeChannelId || "")
}
