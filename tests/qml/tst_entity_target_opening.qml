import QtQuick
import QtTest
import "../../qml/state/chain/EntityTargetOpening.js" as EntityTargetOpening

TestCase {
    id: testRoot

    name: "EntityTargetOpening"

    QtObject {
        id: model

        function valueToString(value) { return value === undefined || value === null ? "" : String(value) }
    }

    QtObject {
        id: session

        property var model: model
    }

    function test_reference_target_classifies_aliases() {
        compare(EntityTargetOpening.referenceTarget(session, "tx", "abc").command, "transaction")
        compare(EntityTargetOpening.referenceTarget(session, "signer", "acct").command, "account")
        compare(EntityTargetOpening.referenceTarget(session, "note", "wallet").command, "localWallet")
        compare(EntityTargetOpening.referenceTarget(session, "note", "wallet").tab, "bedrockNotes")
        compare(EntityTargetOpening.referenceTarget(session, "program", "prog").command, "program")
    }

    function test_reference_target_preserves_payload_backed_block_and_channel() {
        const blockPayload = { hash: "h" }
        const block = EntityTargetOpening.referenceTarget(session, "block", "h", blockPayload)
        const channel = EntityTargetOpening.referenceTarget(session, "channel", "c", { channel_id: "c" })

        compare(block.command, "blockchainBlock")
        compare(block.payload.hash, "h")
        compare(channel.command, "channel")
        compare(channel.payload.channel_id, "c")
    }

    function test_blank_without_payload_is_noop() {
        compare(EntityTargetOpening.referenceTarget(session, "account", "").command, "")
        compare(EntityTargetOpening.referenceTarget(session, "channel", "", { channel_id: "c" }).command, "channel")
    }
}
