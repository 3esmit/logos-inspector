import QtQml
import "chain/AppModelOpeners.js" as AppModelOpeners
import "chain/EntityTargetOpening.js" as EntityTargetOpening
import "app/AppModelSearch.js" as AppModelSearch

QtObject {
    id: root

    required property var model

    function projectZoneDashboard() { return AppModelSearch.projectZoneDashboard(model) }

    function openZoneDashboard(channelId, recordHistory) {
        return AppModelSearch.openZoneDashboard(model, channelId, recordHistory)
    }

    function routeSearch(query) { return AppModelSearch.routeSearch(model, query) }

    function resolveInspectionTarget(query) { return AppModelSearch.resolveInspectionTarget(model, query) }

    function openInspectionCandidate(candidate, recordHistory) { return AppModelSearch.openInspectionCandidate(model, candidate, recordHistory) }

    function openInspectionEntityRef(entity, recordHistory) { return AppModelSearch.openInspectionEntityRef(model, entity, recordHistory) }

    function resumePendingInspectionEntityRef() { return AppModelSearch.resumePendingInspectionEntityRef(model) }

    function openStorageCid(cid) { return AppModelSearch.openStorageCid(model, cid) }

    function isStorageCid(value) { return AppModelSearch.isStorageCid(model, value) }

    function routePrefixedSearch(query) { return AppModelSearch.routePrefixedSearch(model, query) }

    function searchPrefix(query) { return AppModelSearch.searchPrefix(model, query) }

    function isSearchPrefix(prefix) { return AppModelSearch.isSearchPrefix(model, prefix) }

    function routeModuleSearchTarget(target) { return AppModelSearch.routeModuleSearchTarget(model, target) }

    function viewKeyForQuery(query) { return AppModelSearch.viewKeyForQuery(model, query) }

    function settingsTargetForQuery(query) { return AppModelSearch.settingsTargetForQuery(model, query) }

    function referenceTarget(kind, value, payload) { return EntityTargetOpening.referenceTarget(root, kind, value, payload) }

    function openReference(kind, value, payload) { return EntityTargetOpening.openReference(root, kind, value, payload) }

    function openMantleTransaction(hash, navigationContext) {
        return AppModelOpeners.openMantleTransaction(model, hash,
            navigationContext)
    }

    function openPrivateAccountReference(account) { return AppModelOpeners.openPrivateAccountReference(model, account) }

    function openBlockchainBlock(blockOrId) { return AppModelOpeners.openBlockchainBlock(model, blockOrId) }

    function loadBlockchainBlockById(blockId) { return AppModelOpeners.loadBlockchainBlockById(model, blockId) }

    function loadBlockchainBlockBySlot(slot) { return AppModelOpeners.loadBlockchainBlockBySlot(model, slot) }

    function openBlockchainTransaction(transaction, block) { return AppModelOpeners.openBlockchainTransaction(model, transaction, block) }

    function transactionDetail(hash) { return AppModelOpeners.transactionDetail(model, hash) }

    function blockchainTransactionDetail(value, fallbackHash) { return AppModelOpeners.blockchainTransactionDetail(model, value, fallbackHash) }

    function openLocalWallet(wallet, tab) { return AppModelOpeners.openLocalWallet(model, wallet, tab) }

    function showLocalWalletRequired(wallet) { return AppModelOpeners.showLocalWalletRequired(model, wallet) }

}
