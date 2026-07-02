import QtQuick
import QtQml.Models
import "../services/BridgeHelpers.js" as BridgeHelpers
import "../services"

QtObject {
    id: root

    required property BridgeClient bridge

    readonly property string inspectorModule: "logos_inspector"
    readonly property string blockchainModule: "blockchain_module"
    readonly property string storageModule: "storage_module"
    readonly property string deliveryModule: "delivery_module"
    readonly property string capabilityModule: "capability_module"

    property string currentView: "overview"
    property string statusText: qsTr("Ready")
    property bool busy: false
    property string resultTitle: qsTr("Output")
    property string resultText: ""
    property var resultValue: null
    property bool resultIsError: false
    property string resultOwner: ""
    property var dashboardOverview: null
    property var dashboardNode: null
    property var dashboardBlocks: []
    property string dashboardError: ""
    property var blockDetailValue: null
    property var transactionDetailValue: null
    property var walletDetailValue: null
    property var channelDetailValue: null
    property var blocksPageRows: []
    property int blocksPageSlotFrom: 0
    property int blocksPageSlotTo: 0
    property int blocksPageWindow: 2000
    property int blocksPageLimit: 20
    property string blocksPageError: ""
    property var transactionsPageRows: []
    property int transactionsPageBeforeBlock: 0
    property int transactionsPageNextBeforeBlock: 0
    property int transactionsPageBlockBatch: 1000
    property int transactionsPageLimit: 20
    property string transactionsPageError: ""
    property var walletsPageRows: []
    property int walletsPageBeforeBlock: 0
    property int walletsPageNextBeforeBlock: 0
    property int walletsPageBlockBatch: 50
    property int walletsPageLimit: 20
    property string walletsPageError: ""
    property var channelsPageRows: []
    property int channelsPageSlotFrom: 0
    property int channelsPageSlotTo: 0
    property int channelsPageWindow: 4000
    property int channelsPageLimit: 20
    property string channelsPageError: ""

    property string networkProfile: "default"
    property string sequencerUrl: "https://testnet.lez.logos.co/"
    property string indexerUrl: "http://127.0.0.1:8779/"
    property string nodeUrl: "http://127.0.0.1:8080/"

    property string sequencerTab: "blocks"
    property string accountTab: "lookup"
    property string programTab: "idls"
    property string indexerTab: "status"

    property ListModel registeredIdls: ListModel {}

    property ListModel navItems: ListModel {
        ListElement { key: "overview"; label: "Dashboard" }
        ListElement { key: "blocks"; label: "Blocks" }
        ListElement { key: "transactions"; label: "Transactions" }
        ListElement { key: "wallets"; label: "Wallets" }
        ListElement { key: "blockchain"; label: "Blockchain" }
        ListElement { key: "channels"; label: "Channels" }
        ListElement { key: "storage"; label: "Storage" }
        ListElement { key: "messaging"; label: "Messaging" }
        ListElement { key: "capabilities"; label: "Capabilities" }
        ListElement { key: "sequencer"; label: "Sequencer" }
        ListElement { key: "accounts"; label: "Accounts" }
        ListElement { key: "programs"; label: "SPEL" }
        ListElement { key: "indexer"; label: "Indexer" }
        ListElement { key: "settings"; label: "Settings" }
    }

    function viewTitle() {
        for (let i = 0; i < navItems.count; ++i) {
            const item = navItems.get(i)
            if (item.key === currentView) {
                return item.label
            }
        }
        return qsTr("Dashboard")
    }

    function selectView(view) {
        currentView = view
        statusText = qsTr("Ready")
    }

    function clearResult() {
        resultTitle = qsTr("Output")
        resultText = ""
        resultValue = null
        resultIsError = false
        resultOwner = ""
    }

    function setResult(title, text, isError, value) {
        resultTitle = title
        resultText = text
        resultValue = value === undefined ? null : value
        resultIsError = isError
        resultOwner = currentView
        statusText = isError ? qsTr("Error") : qsTr("Ready")
    }

    function pageHasOutput(view) {
        return resultOwner === view && (resultText.length > 0 || resultValue !== null)
    }

    function callInspector(method, args, label) {
        return callModule(inspectorModule, method, args, label)
    }

    function callModule(moduleName, method, args, label) {
        return requestModule(moduleName, method, args, label, true)
    }

    function requestModule(moduleName, method, args, label, showResult) {
        if (busy) {
            return {
                ok: false,
                text: "",
                error: qsTr("Another inspection is already running.")
            }
        }

        const targetModule = moduleName === inspectorModule ? moduleName : inspectorModule
        const targetMethod = moduleName === inspectorModule ? method : "callModule"
        const targetArgs = moduleName === inspectorModule ? args : [moduleName, method, args || []]

        busy = true
        statusText = label
        const response = bridge.callModule(targetModule, targetMethod, targetArgs)
        busy = false

        if (response.ok) {
            updateDashboardCache(method, response.value)
            if (showResult) {
                setResult(label, response.text, false, response.value)
            }
        } else if (showResult) {
            setResult(label, response.error, true, null)
        }
        return response
    }

    function refreshBlocksPage(anchorSlot) {
        const node = requestModule(inspectorModule, "blockchainNode", [nodeUrl], qsTr("Blocks node state"), false)
        if (!node.ok) {
            blocksPageError = node.error
            setResult(qsTr("Blocks"), blocksPageError, true)
            return
        }

        const infoProbe = node.value ? node.value.cryptarchia_info : null
        const info = infoProbe && infoProbe.value ? infoProbe.value.cryptarchia_info : null
        const fallbackSlot = info ? (info.lib_slot || info.slot || 0) : 0
        const slotTo = Math.max(0, Number(anchorSlot === undefined || anchorSlot === null ? fallbackSlot : anchorSlot))
        const slotFrom = Math.max(0, slotTo - blocksPageWindow)
        const blocks = requestModule(inspectorModule, "blockchainBlocks", [nodeUrl, slotFrom, slotTo], qsTr("Blocks"), false)
        if (!blocks.ok) {
            blocksPageError = blocks.error
            setResult(qsTr("Blocks"), blocksPageError, true)
            return
        }

        blocksPageSlotFrom = slotFrom
        blocksPageSlotTo = slotTo
        blocksPageRows = sortedBlocks(blocks.value || []).slice(0, blocksPageLimit)
        blocksPageError = ""
        setResult(qsTr("Blocks"), BridgeHelpers.formatValue(blocksPageRows), false, blocksPageRows)
    }

    function olderBlocksPage() {
        refreshBlocksPage(Math.max(0, blocksPageSlotFrom - 1))
    }

    function sortedBlocks(blocks) {
        const copy = Array.isArray(blocks) ? blocks.slice(0) : []
        copy.sort(function (left, right) {
            return blockSlot(right) - blockSlot(left)
        })
        return copy
    }

    function blockSlot(block) {
        return Number(block && block.header ? (block.header.slot || 0) : 0)
    }

    function blockHash(block) {
        const raw = block || {}
        const header = raw.header || {}
        return String(header.id || header.hash || raw.header_hash || raw.hash || "")
    }

    function blockParent(block) {
        const raw = block || {}
        const header = raw.header || {}
        return String(header.parent_block || header.parent_hash || header.parent || raw.parent_hash || raw.parent || "")
    }

    function blockProof(block) {
        const raw = block || {}
        const header = raw.header || {}
        return header.proof_of_leadership || raw.proof_of_leadership || {}
    }

    function blockRoot(block) {
        const raw = block || {}
        const header = raw.header || {}
        return String(header.block_root || raw.block_root || "")
    }

    function blockHeight(block) {
        const raw = block || {}
        const header = raw.header || {}
        return raw.height !== undefined ? raw.height : header.height
    }

    function blockVersion(block) {
        const raw = block || {}
        const header = raw.header || {}
        return raw.version !== undefined ? raw.version : header.version
    }

    function blockSignature(block) {
        const raw = block || {}
        const header = raw.header || {}
        return String(raw.signature_hex || raw.signature || header.signature_hex || header.signature || "")
    }

    function blockStatus(block) {
        const raw = block || {}
        const explicitStatus = String(raw.bedrock_status || raw.status || "")
        if (explicitStatus.length) {
            return explicitStatus
        }

        const slot = blockSlot(block)
        const info = blockchainInfo()
        if (!slot || !info) {
            return "-"
        }
        if (info.lib_slot !== undefined && slot <= Number(info.lib_slot)) {
            return qsTr("finalized")
        }
        if (info.slot !== undefined && slot <= Number(info.slot)) {
            return qsTr("pending")
        }
        return "-"
    }

    function blockchainInfo() {
        const report = dashboardNode
        const probe = report ? report.cryptarchia_info : null
        return probe && probe.value ? probe.value.cryptarchia_info : null
    }

    function blockTransactions(block) {
        const raw = block || {}
        const transactions = Array.isArray(raw.transactions) ? raw.transactions : []
        const rows = []
        for (let i = 0; i < transactions.length; ++i) {
            const tx = transactions[i]
            const ops = transactionOps(tx)
            rows.push({
                index: i,
                hash: transactionHash(tx),
                ops: ops.length,
                operations: ops.map(function (op, index) {
                    return operationSummary(op, tx, index)
                }),
                raw: tx
            })
        }
        return rows
    }

    function blockchainBlockDetail(block) {
        const proof = blockProof(block)
        return {
            type: "blockchain_block",
            hash: blockHash(block),
            parent: blockParent(block),
            slot: blockSlot(block),
            height: blockHeight(block),
            status: blockStatus(block),
            version: blockVersion(block),
            block_root: blockRoot(block),
            voucher_cm: String(proof.voucher_cm || ""),
            entropy: String(proof.entropy_contribution || proof.entropy || ""),
            signature: blockSignature(block),
            leader_key: String(proof.leader_key || ""),
            transactions: blockTransactions(block),
            raw: block
        }
    }

    function blockchainBlockDetailById(value) {
        const wanted = normalizedHashOrValue(value)
        if (!wanted.length) {
            return null
        }
        const rows = blocksPageRows || []
        for (let i = 0; i < rows.length; ++i) {
            const block = rows[i]
            const hash = blockHash(block)
            const slot = String(blockSlot(block))
            if (normalizedHashOrValue(hash) === wanted || slot === wanted) {
                return blockchainBlockDetail(block)
            }
        }
        return null
    }

    function normalizedHashOrValue(value) {
        let text = String(value || "").trim().toLowerCase()
        if (text.startsWith("0x") && text.length === 66) {
            text = text.slice(2)
        }
        return text
    }

    function refreshTransactionsPage(beforeBlock) {
        const node = requestModule(inspectorModule, "blockchainNode", [nodeUrl], qsTr("Transactions node state"), false)
        if (!node.ok) {
            transactionsPageError = node.error
            setResult(qsTr("Transactions"), transactionsPageError, true)
            return
        }

        const infoProbe = node.value ? node.value.cryptarchia_info : null
        const info = infoProbe && infoProbe.value ? infoProbe.value.cryptarchia_info : null
        const fallbackSlot = info ? (info.lib_slot || info.slot || 0) : 0
        const slotTo = Math.max(0, Number(beforeBlock === undefined || beforeBlock === null ? fallbackSlot : beforeBlock))
        const slotFrom = Math.max(0, slotTo - transactionsPageBlockBatch)
        const blocks = requestModule(inspectorModule, "blockchainBlocks", [nodeUrl, slotFrom, slotTo], qsTr("Transactions"), false)
        if (!blocks.ok) {
            transactionsPageError = blocks.error
            setResult(qsTr("Transactions"), transactionsPageError, true)
            return
        }

        transactionsPageBeforeBlock = slotTo
        transactionsPageRows = transactionRowsFromBlocks(blocks.value || []).slice(0, transactionsPageLimit)
        transactionsPageNextBeforeBlock = slotFrom > 0 ? slotFrom - 1 : 0
        transactionsPageError = ""
        setResult(qsTr("Transactions"), BridgeHelpers.formatValue(transactionsPageRows), false, transactionsPageRows)
    }

    function olderTransactionsPage() {
        refreshTransactionsPage(transactionsPageNextBeforeBlock)
    }

    function transactionRowsFromBlocks(blocks) {
        const rows = []
        const sorted = sortedBlockchainBlocks(blocks)
        for (let i = 0; i < sorted.length; ++i) {
            const block = sorted[i]
            const header = block.header || {}
            const transactions = Array.isArray(block.transactions) ? block.transactions : []
            for (let j = 0; j < transactions.length; ++j) {
                const tx = transactions[j]
                const ops = transactionOps(tx)
                rows.push({
                    slot: header.slot || 0,
                    hash: transactionHash(tx),
                    block: header.id || header.hash || "",
                    index: j,
                    ops: ops.length,
                    operations: ops.map(function (op, index) {
                        return operationSummary(op, tx, index)
                    }),
                    raw: tx
                })
            }
        }
        return rows
    }

    function sortedBlockchainBlocks(blocks) {
        const copy = Array.isArray(blocks) ? blocks.slice(0) : []
        copy.sort(function (left, right) {
            return Number(right.header ? (right.header.slot || 0) : 0) - Number(left.header ? (left.header.slot || 0) : 0)
        })
        return copy
    }

    function transactionHash(tx) {
        const mantle = tx && tx.mantle_tx ? tx.mantle_tx : tx
        return String((mantle && mantle.hash) || (tx && tx.hash) || "")
    }

    function transactionOps(tx) {
        const mantle = tx && tx.mantle_tx ? tx.mantle_tx : tx
        return mantle && Array.isArray(mantle.ops) ? mantle.ops : []
    }

    function operationSummary(op, tx, index) {
        const opcode = Number(op && op.opcode !== undefined ? op.opcode : -1)
        const payload = op && op.payload ? op.payload : {}
        const proofs = tx && tx.ops_proofs ? tx.ops_proofs : []
        return {
            index: index,
            opcode: opcode,
            opcode_hex: byteHex(opcode),
            opcode_name: operationName(opcode),
            channel: String(payload.channel_id || payload.channelId || payload.channel || ""),
            signer: String(payload.signer || ""),
            parent: String(payload.parent || payload.parent_id || payload.parentId || ""),
            payload: payload,
            proof: Array.isArray(proofs) && proofs.length > index ? proofs[index] : null
        }
    }

    function byteHex(value) {
        const number = Number(value)
        if (number < 0 || !Number.isFinite(number)) {
            return "-"
        }
        const hex = number.toString(16)
        return "0x" + (hex.length < 2 ? "0" + hex : hex)
    }

    function operationName(opcode) {
        if (opcode === 0 || opcode === 17) {
            return "ChannelInscribe"
        }
        if (opcode === 2) {
            return "ChannelSetKeys"
        }
        if (opcode === 32) {
            return "SDPDeclare"
        }
        if (opcode === 33) {
            return "SDPWithdraw"
        }
        if (opcode === 34) {
            return "SDPActive"
        }
        if (opcode === 48) {
            return "LeaderClaim"
        }
        return qsTr("Unknown")
    }

    function refreshWalletsPage(beforeBlock) {
        const before = beforeBlock === undefined || beforeBlock === null ? null : beforeBlock
        const wallets = requestModule(inspectorModule, "indexerWallets", [indexerUrl, before, walletsPageBlockBatch], qsTr("Wallets"), false)
        if (!wallets.ok) {
            walletsPageError = wallets.error
            setResult(qsTr("Wallets"), walletsPageError, true)
            return
        }

        walletsPageBeforeBlock = before || 0
        walletsPageRows = (wallets.value || []).slice(0, walletsPageLimit)
        walletsPageNextBeforeBlock = nextWalletPageBlock(walletsPageRows)
        walletsPageError = ""
        setResult(qsTr("Wallets"), BridgeHelpers.formatValue(walletsPageRows), false, walletsPageRows)
    }

    function nextWalletsPage() {
        refreshWalletsPage(walletsPageNextBeforeBlock)
    }

    function nextWalletPageBlock(wallets) {
        const rows = Array.isArray(wallets) ? wallets : []
        let next = 0
        for (let i = 0; i < rows.length; ++i) {
            const slot = Number(rows[i].last_slot || 0)
            if (slot > 0 && (next === 0 || slot < next)) {
                next = slot
            }
        }
        return next
    }

    function walletDetail(row) {
        const wallet = row || {}
        return {
            type: "wallet",
            address: String(wallet.wallet || wallet.address || ""),
            total_received: wallet.received,
            txs: wallet.txs || 0,
            outputs: wallet.outputs || 0,
            last_slot: wallet.last_slot,
            source: String(wallet.source || ""),
            transfers: Array.isArray(wallet.transfers) ? wallet.transfers : [],
            raw: wallet
        }
    }

    function walletDetailById(value) {
        const wanted = normalizedHashOrValue(value)
        if (!wanted.length) {
            return null
        }
        const rows = walletsPageRows || []
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i]
            if (normalizedHashOrValue(row.wallet || row.address) === wanted) {
                return walletDetail(row)
            }
        }
        return null
    }

    function refreshChannelsPage(anchorSlot) {
        const node = requestModule(inspectorModule, "blockchainNode", [nodeUrl], qsTr("Channels node state"), false)
        if (!node.ok) {
            channelsPageError = node.error
            setResult(qsTr("Channels"), channelsPageError, true)
            return
        }

        const infoProbe = node.value ? node.value.cryptarchia_info : null
        const info = infoProbe && infoProbe.value ? infoProbe.value.cryptarchia_info : null
        const fallbackSlot = info ? (info.slot || info.lib_slot || 0) : 0
        const slotTo = Math.max(0, Number(anchorSlot === undefined || anchorSlot === null ? fallbackSlot : anchorSlot))
        const slotFrom = Math.max(0, slotTo - channelsPageWindow)
        const report = requestModule(inspectorModule, "channelScan", [nodeUrl, slotFrom, slotTo], qsTr("Channels"), false)
        if (!report.ok) {
            channelsPageError = report.error
            setResult(qsTr("Channels"), channelsPageError, true)
            return
        }

        channelsPageSlotFrom = slotFrom
        channelsPageSlotTo = slotTo
        channelsPageRows = ((report.value || {}).summaries || []).slice(0, channelsPageLimit)
        channelsPageError = ""
        setResult(qsTr("Channels"), BridgeHelpers.formatValue(report.value || {}), false, report.value || {})
    }

    function olderChannelsPage() {
        refreshChannelsPage(Math.max(0, channelsPageSlotFrom - 1))
    }

    function channelDetail(row) {
        const channel = row || {}
        return {
            type: "channel",
            channel: String(channel.channel || channel.channel_id || ""),
            label: channel.label,
            first_slot: channel.first_slot,
            first_tx_hash: channel.first_tx_hash,
            first_block_hash: channel.first_block_hash,
            last_slot: channel.last_slot,
            last_tx_hash: channel.last_tx_hash,
            last_block_hash: channel.last_block_hash,
            tip: channel.tip,
            balance: channel.balance,
            withdraw_threshold: channel.withdraw_threshold,
            keys: channel.keys,
            key_values: Array.isArray(channel.key_values) ? channel.key_values : [],
            operations: channel.operations || 0,
            raw: channel
        }
    }

    function channelDetailById(value) {
        const wanted = normalizedHashOrValue(value)
        if (!wanted.length) {
            return null
        }
        const rows = channelsPageRows || []
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i]
            if (normalizedHashOrValue(row.channel || row.channel_id) === wanted) {
                return channelDetail(row)
            }
        }
        return null
    }

    function refreshDashboard() {
        const overview = requestModule(inspectorModule, "overview", [sequencerUrl, indexerUrl, nodeUrl], qsTr("Dashboard overview"), false)
        const node = requestModule(inspectorModule, "blockchainNode", [nodeUrl], qsTr("Blockchain node"), false)
        const blocks = requestModule(inspectorModule, "indexerBlocks", [indexerUrl, null, 10], qsTr("Latest blocks"), false)
        const errors = []

        if (!overview.ok) {
            errors.push(overview.error)
        }
        if (!node.ok) {
            errors.push(node.error)
        }
        if (!blocks.ok) {
            errors.push(blocks.error)
        }

        dashboardError = errors.join("\n")

        if (overview.ok || node.ok || blocks.ok) {
            setResult(qsTr("Dashboard"), BridgeHelpers.formatValue({
                overview: overview.value || null,
                node: node.value || null,
                blocks: blocks.value || []
            }), false)
            return
        }

        setResult(qsTr("Dashboard"), dashboardError, true)
    }

    function updateDashboardCache(method, value) {
        if (method === "overview") {
            dashboardOverview = value
        } else if (method === "blockchainNode") {
            dashboardNode = value
        } else if (method === "indexerBlocks") {
            dashboardBlocks = value || []
        }
    }

    function routeSearch(query) {
        const value = query.trim()
        if (!value.length) {
            return
        }

        if (/^[0-9]+$/.test(value)) {
            const detail = blockchainBlockDetailById(value)
            if (detail) {
                openBlockchainBlock(value)
                return
            }
            openBlockchainBlock(value)
            return
        }

        if (/^(0x)?[0-9a-fA-F]{64}$/.test(value)) {
            const detail = blockchainBlockDetailById(value)
            if (detail) {
                openBlockchainBlock(value)
                return
            }
            const wallet = walletDetailById(value)
            if (wallet) {
                openWallet(value)
                return
            }
            const channel = channelDetailById(value)
            if (channel) {
                openChannel(value)
                return
            }
            openTransaction(value)
            return
        }

        const wallet = walletDetailById(value)
        if (wallet) {
            openWallet(value)
            return
        }

        const channel = channelDetailById(value)
        if (channel) {
            openChannel(value)
            return
        }

        currentView = "accounts"
        accountTab = "lookup"
        callInspector("account", [sequencerUrl, indexerUrl, value], qsTr("Account lookup"))
    }

    function openReference(kind, value, payload) {
        const target = String(value || "").trim()
        if (!target.length && payload === undefined) {
            return
        }

        switch (kind) {
        case "block":
        case "blockHash":
        case "blockNumber":
        case "slot":
            openBlockchainBlock(payload === undefined ? target : payload)
            return
        case "indexerBlock":
            openIndexerBlock(target)
            return
        case "transaction":
        case "transactionHash":
        case "tx":
            openTransaction(target)
            return
        case "wallet":
            openWallet(target)
            return
        case "channel":
            openChannel(payload === undefined ? target : payload)
            return
        case "account":
        case "program":
        case "signer":
            openAccount(target)
            return
        default:
            routeSearch(target)
        }
    }

    function openAccount(account) {
        const value = String(account || "").trim()
        if (!value.length) {
            return
        }
        currentView = "accounts"
        accountTab = "lookup"
        callInspector("account", [sequencerUrl, indexerUrl, value], qsTr("Account lookup"))
    }

    function openTransaction(hash) {
        const value = String(hash || "").trim()
        if (!value.length) {
            return
        }

        currentView = "transactions"
        const detail = transactionDetail(value)
        if (detail) {
            transactionDetailValue = detail
            setResult(qsTr("Transaction"), BridgeHelpers.formatValue(detail), false, detail)
            return
        }

        inspectTransaction(value, "")
    }

    function inspectTransaction(hash, idl) {
        const value = String(hash || "").trim()
        if (!value.length) {
            return
        }

        currentView = "transactions"
        const trimmedIdl = String(idl || "").trim()
        const args = trimmedIdl.length ? [sequencerUrl, value, trimmedIdl] : [sequencerUrl, value]
        const response = requestModule(inspectorModule, "inspectTransaction", args, qsTr("Transaction inspection"), false)
        if (response.ok) {
            transactionDetailValue = response.value
            transactionsPageError = ""
            setResult(qsTr("Transaction"), response.text, false, response.value)
        } else {
            transactionsPageError = response.error
            setResult(qsTr("Transaction"), response.error, true)
        }
    }

    function openBlockchainBlock(blockOrId) {
        let detail = null
        if (blockOrId && typeof blockOrId === "object") {
            detail = blockchainBlockDetail(blockOrId)
        } else {
            detail = blockchainBlockDetailById(blockOrId)
        }
        if (!detail) {
            const fallback = blockOrId && typeof blockOrId === "object" ? blockHash(blockOrId) : blockOrId
            if (/^[0-9]+$/.test(String(fallback || "").trim())) {
                loadBlockchainBlockBySlot(Number(fallback))
                return
            }
            openIndexerBlock(fallback)
            return
        }

        currentView = "blocks"
        blockDetailValue = detail
        setResult(qsTr("Block"), BridgeHelpers.formatValue(detail), false, detail)
    }

    function loadBlockchainBlockBySlot(slot) {
        const value = Math.max(0, Number(slot || 0))
        currentView = "blocks"
        const response = requestModule(inspectorModule, "blockchainBlocks", [nodeUrl, value, value], qsTr("Block lookup"), false)
        if (response.ok) {
            const blocks = Array.isArray(response.value) ? response.value : []
            if (blocks.length > 0) {
                blockDetailValue = blockchainBlockDetail(blocks[0])
                setResult(qsTr("Block"), BridgeHelpers.formatValue(blockDetailValue), false, blockDetailValue)
                return
            }
            blocksPageError = qsTr("No block found at slot %1.").arg(value)
            setResult(qsTr("Block"), blocksPageError, true)
        } else {
            blocksPageError = response.error
            setResult(qsTr("Block"), response.error, true)
        }
    }

    function openBlockchainTransaction(transaction, block) {
        const tx = transaction || {}
        const parentBlock = block || {}
        const detail = {
            type: "blockchain_transaction",
            hash: String(tx.hash || ""),
            block: String(parentBlock.hash || ""),
            slot: parentBlock.slot,
            index: tx.index,
            ops: Array.isArray(tx.operations) ? tx.operations : [],
            raw: tx.raw || null
        }
        currentView = "transactions"
        transactionDetailValue = detail
        setResult(qsTr("Transaction"), BridgeHelpers.formatValue(detail), false, detail)
    }

    function transactionDetail(hash) {
        const normalized = normalizedHashOrValue(hash)
        const rows = transactionsPageRows || []
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i]
            if (normalizedHashOrValue(row.hash) === normalized) {
                return {
                    type: "blockchain_transaction",
                    hash: row.hash,
                    block: row.block,
                    slot: row.slot,
                    index: row.index,
                    ops: row.operations || [],
                    raw: row.raw
                }
            }
        }
        return null
    }

    function openIndexerBlock(headerHash) {
        const value = String(headerHash || "").trim()
        if (!value.length) {
            return
        }

        currentView = "blocks"
        const cached = blockchainBlockDetailById(value)
        if (cached) {
            blockDetailValue = cached
            setResult(qsTr("Block"), BridgeHelpers.formatValue(cached), false, cached)
            return
        }

        const response = requestModule(inspectorModule, "indexerBlockByHash", [indexerUrl, value], qsTr("Block lookup"), false)
        if (response.ok) {
            if (response.value === null || response.value === undefined) {
                blocksPageError = qsTr("No block found for %1.").arg(value)
                setResult(qsTr("Block"), blocksPageError, true)
                return
            }
            blockDetailValue = indexerBlockDetail(response.value)
            blocksPageError = ""
            setResult(qsTr("Block"), response.text, false, blockDetailValue)
        } else {
            blocksPageError = response.error
            setResult(qsTr("Block"), response.error, true)
        }
    }

    function indexerBlockDetail(value) {
        const block = value || {}
        const transactions = Array.isArray(block.transactions) ? block.transactions : []
        return {
            type: "indexer_block",
            hash: String(block.header_hash || ""),
            parent: String(block.parent_hash || ""),
            slot: block.block_id,
            height: block.block_id,
            status: String(block.bedrock_status || ""),
            version: "",
            block_root: "",
            voucher_cm: "",
            entropy: "",
            signature: "",
            leader_key: "",
            transactions: transactions.map(function (tx, index) {
                return {
                    index: tx.index !== undefined ? tx.index : index,
                    hash: String(tx.hash || ""),
                    ops: Array.isArray(tx.instruction_data) ? tx.instruction_data.length : 0,
                    operations: [],
                    raw: tx.raw || tx
                }
            }),
            raw: block.raw || block
        }
    }

    function openWallet(wallet) {
        const value = String(wallet || "").trim()
        if (!value.length) {
            return
        }

        const detail = walletDetailById(value)
        if (detail) {
            currentView = "wallets"
            walletDetailValue = detail
            setResult(qsTr("Wallet"), BridgeHelpers.formatValue(detail), false, detail)
            return
        }

        openAccount(value)
    }

    function openChannel(channel) {
        const detail = typeof channel === "object" ? channelDetail(channel) : channelDetailById(channel)
        if (detail) {
            currentView = "channels"
            channelDetailValue = detail
            setResult(qsTr("Channel"), BridgeHelpers.formatValue(detail), false, detail)
            return
        }

        const value = { type: "channel", channel: String(channel || "") }
        currentView = "channels"
        channelDetailValue = value
        setResult(qsTr("Channel"), BridgeHelpers.formatValue(value), false, value)
    }

    function registerIdl(name, programId, json) {
        if (!json.trim().length) {
            setResult(qsTr("IDL registry"), qsTr("IDL JSON is required."), true)
            return
        }

        const parsed = BridgeHelpers.parseJson(json)
        if (!parsed.ok) {
            setResult(qsTr("IDL registry"), qsTr("Invalid IDL JSON: %1").arg(parsed.error), true)
            return
        }

        const idl = parsed.value
        const resolvedName = name.trim().length ? name.trim() : (idl.name || qsTr("IDL %1").arg(registeredIdls.count + 1))
        registeredIdls.append({
            name: resolvedName,
            programId: programId.trim(),
            json: json
        })
        setResult(qsTr("IDL registry"), qsTr("Saved %1.").arg(resolvedName), false)
    }

    function removeIdl(index) {
        registeredIdls.remove(index)
    }

    function profileIndex() {
        if (networkProfile === "testnet-indexer-local") {
            return 1
        }
        if (networkProfile === "local-node") {
            return 2
        }
        if (networkProfile === "local") {
            return 3
        }
        return 0
    }

    function applyProfile(index) {
        if (index === 3) {
            networkProfile = "local"
            sequencerUrl = "http://127.0.0.1:3040/"
            indexerUrl = "http://127.0.0.1:8779/"
            nodeUrl = "http://127.0.0.1:8080/"
            return
        }

        if (index === 2) {
            networkProfile = "local-node"
        } else {
            networkProfile = index === 1 ? "testnet-indexer-local" : "default"
        }
        sequencerUrl = "https://testnet.lez.logos.co/"
        indexerUrl = "http://127.0.0.1:8779/"
        nodeUrl = "http://127.0.0.1:8080/"
    }
}
