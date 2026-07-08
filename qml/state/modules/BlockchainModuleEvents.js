.import "ModuleEventUtils.js" as ModuleEventUtils

function handleNewBlock(root, args) {
    with (root) {
        const block = ModuleEventUtils.parsedPayload(ModuleEventUtils.firstEventValue(args))
        if (!block || typeof block !== "object") {
            return false
        }
        const merged = root.mergeLiveBlocks([block], blocksPageRows, blocksPageLimit)
        blocksPageRows = merged
        blocksPageSlotTo = Math.max(Number(blocksPageSlotTo || 0), Number(root.blockSlot(block) || 0))
        blocksPageSlotFrom = merged.length ? Math.max(0, Math.min.apply(Math, merged.map(function (row) {
            return Number(root.blockSlot(row) || blocksPageSlotFrom || 0)
        }))) : blocksPageSlotFrom
        blocksLiveSource = "module_event"
        blocksLiveCheckedAt = ModuleEventUtils.eventTimeText("")
        blocksLiveError = ""
        blockchainLastEventText = qsTr("New block %1").arg(root.valueText(root.blockSlot(block)))
        blockchainModuleEventRevision += 1
        queryNetworkConnection("blockchain", false)

        const wallet = String(walletPublicKeyProbe || "").trim()
        if (wallet.length > 0 && ModuleEventUtils.valueContainsText(block, wallet)) {
            refreshBedrockWalletModule(wallet)
            queryBedrockWalletBalance()
        }
        return true
    }
}
