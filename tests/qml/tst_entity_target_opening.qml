import QtQuick
import QtTest
import "../../qml/state/chain/EntityTargetOpening.js" as EntityTargetOpening

TestCase {
    id: testRoot

    name: "EntityTargetOpening"

    QtObject {
        id: model

        property QtObject metrics: QtObject {
            function valueToString(value) {
                return value === undefined || value === null ? "" : String(value)
            }
        }
    }

    QtObject {
        id: session

        property var model: model
    }

    function test_l2_references_become_typed_searches() {
        compare(EntityTargetOpening.referenceTarget(session, "tx", "abc").command, "search")
        compare(EntityTargetOpening.referenceTarget(session, "tx", "abc").target, "tx:abc")
        compare(EntityTargetOpening.referenceTarget(session, "signer", "acct").target, "account:acct")
        compare(EntityTargetOpening.referenceTarget(session, "program", "prog").target, "program:prog")
        compare(EntityTargetOpening.referenceTarget(session, "channel", "chan").target, "channel:chan")
        compare(EntityTargetOpening.referenceTarget(session, "indexerBlock", "hash").target, "l2:hash")
    }

    function test_local_and_l1_references_keep_direct_commands() {
        compare(EntityTargetOpening.referenceTarget(session, "note", "wallet").command, "localWallet")
        compare(EntityTargetOpening.referenceTarget(session, "note", "wallet").tab, "bedrockNotes")
        compare(EntityTargetOpening.referenceTarget(session, "private", "acct").command, "privateAccount")

        const payload = { hash: "h" }
        const block = EntityTargetOpening.referenceTarget(session, "block", "h", payload)
        compare(block.command, "blockchainBlock")
        compare(block.payload.hash, "h")
    }

    function test_blank_without_payload_is_noop() {
        compare(EntityTargetOpening.referenceTarget(session, "account", "").command, "")
    }
}
