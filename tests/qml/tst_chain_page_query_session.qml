import QtQuick
import QtTest
import "../../qml/state/chain/ChainPageQuerySession.js" as ChainPageQuerySession

TestCase {
    name: "ChainPageQuerySession"

    QtObject {
        id: chainRoot

        property var lezTransactionsPageRows: []
        property var lezTransactionsPageOverflowRows: []
        property int lezTransactionsPageLimit: 2
        property int lezTransactionsPageNextBeforeBlock: 0
        property int lezTransactionsPageOverflowNextBeforeBlock: 0
        property string resultTitle: ""
        property var resultValue: null

        function setResult(title, text, isError, value) {
            resultTitle = title
            resultValue = value
        }
    }

    function init() {
        chainRoot.lezTransactionsPageRows = []
        chainRoot.lezTransactionsPageOverflowRows = []
        chainRoot.lezTransactionsPageLimit = 2
        chainRoot.lezTransactionsPageNextBeforeBlock = 0
        chainRoot.lezTransactionsPageOverflowNextBeforeBlock = 0
        chainRoot.resultTitle = ""
        chainRoot.resultValue = null
    }

    function test_response_block_array_requires_ok_array() {
        compare(ChainPageQuerySession.responseBlockArray({ ok: true, value: [1, 2] }).length, 2)
        compare(ChainPageQuerySession.responseBlockArray({ ok: true, value: ({}) }), null)
        compare(ChainPageQuerySession.responseBlockArray({ ok: false, value: [] }), null)
    }

    function test_older_lez_transactions_consumes_overflow_before_query() {
        chainRoot.lezTransactionsPageOverflowRows = [{ hash: "a" }, { hash: "b" }, { hash: "c" }]
        chainRoot.lezTransactionsPageOverflowNextBeforeBlock = 7

        ChainPageQuerySession.olderLezTransactionsPage(chainRoot)

        compare(chainRoot.lezTransactionsPageRows.length, 2)
        compare(chainRoot.lezTransactionsPageOverflowRows.length, 1)
        compare(chainRoot.lezTransactionsPageNextBeforeBlock, 0)
        compare(chainRoot.resultTitle, "L2 transactions")
    }
}
