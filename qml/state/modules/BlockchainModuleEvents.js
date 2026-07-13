.import "../chain/AppModelPages.js" as AppModelPages
.import "ModuleEventUtils.js" as ModuleEventUtils

function handleNewBlock(root, args) {
    with (root) {
        const report = AppModelPages.normalizedLiveBlockReport(ModuleEventUtils.firstEventValue(args), "module_event")
        const block = report.blocks.length > 0 ? report.blocks[0] : null
        if (!block) {
            return false
        }
        AppModelPages.applyLiveBlockReport(root.chainPages, report, {
            checkedAt: ModuleEventUtils.eventTimeText("")
        })
        blockchainLastEventText = qsTr("New block %1").arg(root.valueText(root.chainPages.blockSlot(block)))
        blockchainModuleEventRevision += 1
        queryNetworkConnection("blockchain", false)

        const wallet = String(walletPublicKeyProbe || "").trim()
        if (wallet.length > 0 && ModuleEventUtils.valueContainsText(block, wallet)) {
            refreshBedrockWalletModule(wallet)
            wallet.queryBedrockBalance()
        }
        return true
    }
}
