import QtQuick
import QtTest
import "../../qml/state/capabilities/CapabilityGateIntents.js" as CapabilityGateIntents

TestCase {
    name: "CapabilityGateIntents"

    function test_storage_backup_actions_require_transport_and_content() {
        const upload = CapabilityGateIntents.storageDependency("backup_upload")
        const read = CapabilityGateIntents.storageDependency("backup_read_by_cid")

        compare(upload.all_of.length, 2)
        compare(upload.all_of[0], "storage.content.upload")
        compare(upload.all_of[1], "storage.backup.sync_upload")
        compare(read.all_of[1], "storage.backup.sync_read_by_cid")
    }

    function test_social_shared_idls_require_delivery_and_storage_sync() {
        const read = CapabilityGateIntents.socialDependency("shared_idl.read")
        const write = CapabilityGateIntents.socialDependency("shared_idl.write")

        compare(read.all_of[0], "delivery.store.query")
        compare(read.all_of[1], "storage.content.read_by_cid")
        compare(read.all_of[2], "storage.shared_idl.sync_read")
        compare(write.all_of[3], "social.identity.local")
    }

    function test_wallet_instruction_intents_include_static_decode() {
        const preview = CapabilityGateIntents.walletDependency("l2.preview")
        const submit = CapabilityGateIntents.walletDependency("l2.submit")

        compare(preview.all_of[0], "program_decode.static")
        compare(preview.all_of[1], "wallet.l2.instruction.preview")
        compare(submit.all_of[1], "wallet.l2.instruction.submit")
        compare(CapabilityGateIntents.programDecodeDependency(), "program_decode.static")
    }
}
