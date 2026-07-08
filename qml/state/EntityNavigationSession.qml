import QtQml
import "chain/AppModelOpeners.js" as AppModelOpeners
import "app/AppModelSearch.js" as AppModelSearch

QtObject {
    id: root

    required property var model

    function refreshDashboard() { return AppModelSearch.refreshDashboard(model) }

    function updateDashboardCache(method, value) { return AppModelSearch.updateDashboardCache(model, method, value) }

    function routeSearch(query) { return AppModelSearch.routeSearch(model, query) }

    function openStorageCid(cid) { return AppModelSearch.openStorageCid(model, cid) }

    function isStorageCid(value) { return AppModelSearch.isStorageCid(model, value) }

    function numericSearchUsesLezBlock() { return AppModelSearch.numericSearchUsesLezBlock(model) }

    function routePrefixedSearch(query) { return AppModelSearch.routePrefixedSearch(model, query) }

    function searchPrefix(query) { return AppModelSearch.searchPrefix(model, query) }

    function isSearchPrefix(prefix) { return AppModelSearch.isSearchPrefix(model, prefix) }

    function routeModuleSearchTarget(target) { return AppModelSearch.routeModuleSearchTarget(model, target) }

    function resolveSearchHash(hash) { return AppModelSearch.resolveSearchHash(model, hash) }

    function applyResolvedLezTarget(response, errorTitle) { return AppModelSearch.applyResolvedLezTarget(model, response, errorTitle) }

    function resolveSearchTransaction(serial, hash, recordHistory) { return AppModelSearch.resolveSearchTransaction(model, serial, hash, recordHistory) }

    function resolveSearchAccount(serial, account, recordHistory) { return AppModelSearch.resolveSearchAccount(model, serial, account, recordHistory) }

    function viewKeyForQuery(query) { return AppModelSearch.viewKeyForQuery(model, query) }

    function settingsTargetForQuery(query) { return AppModelSearch.settingsTargetForQuery(model, query) }

    function openReference(kind, value, payload) { return AppModelOpeners.openReference(model, kind, value, payload) }

    function openMantleTransaction(hash) { return AppModelOpeners.openMantleTransaction(model, hash) }

    function openAccount(account) { return AppModelOpeners.openAccount(model, account) }

    function openPrivateAccountReference(account) { return AppModelOpeners.openPrivateAccountReference(model, account) }

    function openTransaction(hash) { return AppModelOpeners.openTransaction(model, hash) }

    function openLezSearchTarget(target) { return AppModelOpeners.openLezSearchTarget(model, target) }

    function openLezBlock(blockId) { return AppModelOpeners.openLezBlock(model, blockId) }

    function resolveLezHash(hash) { return AppModelOpeners.resolveLezHash(model, hash) }

    function openLezTransaction(hash, recordHistory) { return AppModelOpeners.openLezTransaction(model, hash, recordHistory) }

    function inspectTransaction(hash, idl, recordHistory) { return AppModelOpeners.inspectTransaction(model, hash, idl, recordHistory) }

    function openBlockchainBlock(blockOrId) { return AppModelOpeners.openBlockchainBlock(model, blockOrId) }

    function loadBlockchainBlockById(blockId) { return AppModelOpeners.loadBlockchainBlockById(model, blockId) }

    function loadBlockchainBlockBySlot(slot) { return AppModelOpeners.loadBlockchainBlockBySlot(model, slot) }

    function openBlockchainTransaction(transaction, block) { return AppModelOpeners.openBlockchainTransaction(model, transaction, block) }

    function transactionDetail(hash) { return AppModelOpeners.transactionDetail(model, hash) }

    function blockchainTransactionDetail(value, fallbackHash) { return AppModelOpeners.blockchainTransactionDetail(model, value, fallbackHash) }

    function openIndexerBlock(headerHash, payload) { return AppModelOpeners.openIndexerBlock(model, headerHash, payload) }

    function indexerBlockDetail(value, source) { return AppModelOpeners.indexerBlockDetail(model, value, source) }

    function openLocalWallet(wallet, tab) { return AppModelOpeners.openLocalWallet(model, wallet, tab) }

    function showLocalWalletRequired(wallet) { return AppModelOpeners.showLocalWalletRequired(model, wallet) }

    function openProgram(programId) { return AppModelOpeners.openProgram(model, programId) }

    function programContextDetail(programId) { return AppModelOpeners.programContextDetail(model, programId) }

    function programContextFromParts(input, normalized, knownRow, accountResponse, lookupError) { return AppModelOpeners.programContextFromParts(model, input, normalized, knownRow, accountResponse, lookupError) }

    function knownProgramRow(programId) { return AppModelOpeners.knownProgramRow(model, programId) }

    function programRecentTransactions(programId) { return AppModelOpeners.programRecentTransactions(model, programId) }

    function looksLikeHexId(value) { return AppModelOpeners.looksLikeHexId(model, value) }

    function openRecipient(recipient) { return AppModelOpeners.openRecipient(model, recipient) }

    function openChannel(channel) { return AppModelOpeners.openChannel(model, channel) }
}
