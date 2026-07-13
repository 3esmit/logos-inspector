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
        compare(catalog.rows()[0].backup_version_label, "Manual")
    }

    function test_create_local_allows_backend_default_for_empty_label() {
        gateway.callResponses = ({
            createLocalSettingsBackup: {
                ok: true,
                value: {
                    backup_catalog_id: "backup-default",
                    payload_id: "sha256:def",
                    backup_version_label: "1720000000",
                    created_at: "1720000000"
                },
                text: "OK",
                error: ""
            }
        })

        const entry = catalog.createLocal("", false, {}, {})

        verify(entry !== null)
        compare(gateway.lastArgs[0], "")
        compare(catalog.rows().length, 1)
        compare(catalog.rows()[0].backup_version_label, "1720000000")
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

    function test_apply_import_calls_transaction_method() {
        gateway.callResponses = ({
            settingsBackupImportApply: {
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

        const summary = catalog.applyImport("backup-1", { wallet_home: "/tmp/wallet" }, { favorites: "merge" })

        verify(summary !== null)
        compare(gateway.lastMethod, "settingsBackupImportApply")
        compare(gateway.lastArgs[0], "backup-1")
        compare(gateway.lastArgs[1].wallet_home, "/tmp/wallet")
        compare(gateway.lastArgs[2].favorites, "merge")
        compare(summary.favorites, 2)
    }

    function test_preview_import_calls_transaction_method() {
        gateway.callResponses = ({
            settingsBackupImportPreview: {
                ok: true,
                value: {
                    import_plan: true,
                    blocked: false,
                    selectedAreas: ["settings"]
                },
                text: "OK",
                error: ""
            }
        })

        const plan = catalog.previewImport("backup-1", {}, { settings: "replace" })

        verify(plan !== null)
        compare(gateway.lastMethod, "settingsBackupImportPreview")
        compare(gateway.lastArgs[0], "backup-1")
        compare(gateway.lastArgs[2].settings, "replace")
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

        const upload = catalog.uploadLocal("backup-1", {
            adapter: {
                source_mode: "rest",
                inputs: { rest_endpoint: "http://storage" }
            },
            payload: {},
            mutating_enabled: true
        })

        verify(upload !== null)
        compare(gateway.lastMethod, "storageUploadBackupCatalogEntry")
        compare(gateway.lastArgs[0].adapter.source_mode, "rest")
        compare(gateway.lastArgs[0].adapter.inputs.rest_endpoint, "http://storage")
        compare(gateway.lastArgs[0].payload.backup_catalog_id, "backup-1")
        compare(gateway.lastArgs[0].payload.block_size, 65536)
        compare(catalog.rows()[0].remote.cid, "z-cid")
    }
}
