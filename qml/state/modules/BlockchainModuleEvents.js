.import "../chain/AppModelPages.js" as AppModelPages
.import "ModuleEventUtils.js" as ModuleEventUtils

function handleNewBlock(root, args) {
    if (!root || !root.sourceRouting) {
        return false
    }
    const trustedWatch = trustedLogoscoreCliWatchEvent(root, args)
    const acceptsUntagged = typeof root.sourceRouting.acceptsUntaggedBlockchainModuleEvents === "function"
        && root.sourceRouting.acceptsUntaggedBlockchainModuleEvents() === true
    if (!trustedWatch && !acceptsUntagged) {
        return false
    }
    with (root) {
        const report = AppModelPages.normalizedLiveBlockReport(
            trustedWatch ? trustedWatch.payload : newBlockEventPayload(args),
            trustedWatch ? "logoscore_cli_watch" : "module_event")
        const block = report.blocks.length > 0 ? report.blocks[0] : null
        if (!block) {
            return false
        }
        AppModelPages.applyLiveBlockReport(root.chainPages, report, {
            checkedAt: ModuleEventUtils.eventTimeText(trustedWatch ? trustedWatch.timestamp : "")
        })
        blockchainLastEventText = qsTr("New block %1").arg(
            root.metrics.valueText(root.chainPages.blockSlot(block)))
        blockchainModuleEventRevision += 1
        metrics.queryNetworkConnection("blockchain", false, false, "module-event")

        const wallet = String(walletPublicKeyProbe || "").trim()
        if (wallet.length > 0 && ModuleEventUtils.valueContainsText(block, wallet)) {
            refreshBedrockWalletModule(wallet)
            wallet.queryBedrockBalance()
        }
        return true
    }
}

function newBlockEventPayload(args) {
    const values = ModuleEventUtils.eventValues(args)
    let value = values.length === 1 ? values[0] : ModuleEventUtils.firstEventValue(args)
    // The module contract is a flat QVariantList with one newBlock argument.
    // Some Basecamp ingress paths preserve that argument vector once more,
    // either as a list or a serialized JSON list. Normalize only this
    // newBlock transport envelope; block payload arrays remain unsupported.
    for (let depth = 0; depth < 2; ++depth) {
        if (Array.isArray(value) && value.length === 1) {
            value = value[0]
            continue
        }
        const parsed = ModuleEventUtils.parsedPayload(value)
        if (Array.isArray(parsed) && parsed.length === 1) {
            value = parsed[0]
            continue
        }
        break
    }
    return value
}

function trustedLogoscoreCliWatchEvent(root, args) {
    if (!root || !root.sourceRouting
            || typeof root.sourceRouting.acceptsTrustedLogoscoreCliBlockchainEvents !== "function"
            || root.sourceRouting.acceptsTrustedLogoscoreCliBlockchainEvents() !== true) {
        return null
    }
    const values = ModuleEventUtils.eventValues(args)
    if (values.length !== 1) {
        return null
    }
    const tagged = ModuleEventUtils.parsedPayload(values[0])
    if (!tagged || typeof tagged !== "object" || Array.isArray(tagged)
            || String(tagged.source || "") !== "logoscore_cli_watch"
            || String(tagged.protocol || "") !== "logoscore.watch"
            || Number(tagged.version) !== 1
            || tagged.payload === undefined) {
        return null
    }
    return {
        payload: tagged.payload,
        timestamp: String(tagged.timestamp || "")
    }
}
