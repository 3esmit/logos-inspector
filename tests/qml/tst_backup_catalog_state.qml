import QtQuick
import QtTest
import "../../qml/state/domains" as Domains
import "fixtures"

TestCase {
    id: testRoot

    name: "BackupCatalogState"

    StateGatewayFixture {
        id: gateway
    }

    Domains.BackupCatalogState {
        id: catalog

        gateway: gateway
    }

    function init() {
        gateway.reset()
        catalog.entries = []
        catalog.loaded = false
        catalog.error = ""
        catalog.revision = 0
    }

    function test_load_reads_catalog_entries() {
        gateway.callResponses = ({
            loadBackupCatalog: {
                ok: true,
                value: {
                    version: 1,
                    entries: [{ backup_catalog_id: "backup-1" }]
                },
                text: "OK",
                error: ""
            }
        })

        verify(catalog.load())

        compare(gateway.lastMethod, "loadBackupCatalog")
        verify(catalog.loaded)
        compare(catalog.rows().length, 1)
        compare(catalog.rows()[0].backup_catalog_id, "backup-1")
    }

    function test_create_local_upserts_entry() {
        gateway.callResponses = ({
            createLocalSettingsBackup: {
                ok: true,
                value: {
                    backup_catalog_id: "backup-1",
                    payload_id: "sha256:abc",
                    backup_version_label: "Manual"
                },
                text: "OK",
                error: ""
            }
        })

        const entry = catalog.createLocal("Manual", false, { wallet_home: "/tmp/wallet" })

        verify(entry !== null)
        compare(gateway.lastArgs[0], "Manual")
        compare(gateway.lastArgs[1], false)
        compare(gateway.lastArgs[2].wallet_home, "/tmp/wallet")
        verify(gateway.lastArgs[3] !== undefined)
        compare(catalog.rows().length, 1)
    }

    function test_attach_remote_replaces_existing_entry_without_duplicate() {
        catalog.entries = [{
            backup_catalog_id: "backup-1",
            payload_id: "sha256:abc"
        }]
        gateway.callResponses = ({
            attachBackupRemote: {
                ok: true,
                value: {
                    backup_catalog_id: "backup-1",
                    payload_id: "sha256:abc",
                    remote: { cid: "z-cid" }
                },
                text: "OK",
                error: ""
            }
        })

        const entry = catalog.attachRemote("backup-1", "z-cid", "logos_storage")

        verify(entry !== null)
        compare(catalog.rows().length, 1)
        compare(catalog.rows()[0].remote.cid, "z-cid")
    }

    function test_restore_local_calls_catalog_restore_method() {
        gateway.callResponses = ({
            restoreLocalSettingsBackup: {
                ok: true,
                value: {
                    restored: true,
                    backup_catalog_id: "backup-1",
                    favorites: 2,
                    idl_count: 1
                },
                text: "OK",
                error: ""
            }
        })

        const summary = catalog.restoreLocal("backup-1", { wallet_home: "/tmp/wallet" }, { favorites: "merge" })

        verify(summary !== null)
        compare(gateway.lastMethod, "restoreLocalSettingsBackup")
        compare(gateway.lastArgs[0], "backup-1")
        compare(gateway.lastArgs[1].wallet_home, "/tmp/wallet")
        compare(gateway.lastArgs[2].favorites, "merge")
        compare(summary.favorites, 2)
    }

    function test_upload_local_appends_catalog_id_to_storage_args() {
        gateway.callResponses = ({
            storageUploadBackupCatalogEntry: {
                ok: true,
                value: {
                    cid: "z-cid",
                    catalog_entry: {
                        backup_catalog_id: "backup-1",
                        remote: { cid: "z-cid" }
                    }
                },
                text: "OK",
                error: ""
            }
        })

        const upload = catalog.uploadLocal("backup-1", ["rest", "http://storage", true])

        verify(upload !== null)
        compare(gateway.lastMethod, "storageUploadBackupCatalogEntry")
        compare(gateway.lastArgs[0], "rest")
        compare(gateway.lastArgs[3], "backup-1")
        compare(gateway.lastArgs[4], 65536)
        compare(catalog.rows()[0].remote.cid, "z-cid")
    }
}
