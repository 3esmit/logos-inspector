.import "../chain/AppModelPages.js" as AppModelPages
.import "ModuleEventUtils.js" as ModuleEventUtils

function handleNewBlock(root, args) {
    // The event contract has no connector identity. Only the selected host
    // module route can make an untagged event authoritative.
    if (!root || !root.sourceRouting
            || typeof root.sourceRouting.acceptsUntaggedBlockchainModuleEvents !== "function"
            || root.sourceRouting.acceptsUntaggedBlockchainModuleEvents() !== true) {
        return false
    }
    with (root) {
        const report = AppModelPages.normalizedLiveBlockReport(ModuleEventUtils.firstEventValue(args), "module_event")
        const block = report.blocks.length > 0 ? report.blocks[0] : null
        if (!block) {
            return false
        }
        AppModelPages.applyLiveBlockReport(root.chainPages, report, {
            checkedAt: ModuleEventUtils.eventTimeText("")
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
