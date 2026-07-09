import QtQuick
import QtTest
import "../../qml/state/backup" as Backup

TestCase {
    id: testRoot

    name: "BackupImportDialogState"

    QtObject {
        id: model

        property string backupCatalogError: ""
        property string lastBackupId: ""
        property var lastOptions: null
        property var nextPlan: null

        function previewLocalSettingsImportPlan(backupId, options) {
            lastBackupId = String(backupId || "")
            lastOptions = options
            return nextPlan
        }

        function backupImportDecisionSummaryText(decision) {
            return String(decision && decision.label ? decision.label : "")
        }
    }

    Backup.BackupImportDialogState {
        id: dialog

        model: model
    }

    ListModel {
        id: conflictOptions

        ListElement { key: "required"; label: "Choose" }
        ListElement { key: "replace_existing"; label: "Replace" }
        ListElement { key: "skip_backup_item"; label: "Skip" }
    }

    function init() {
        model.backupCatalogError = ""
        model.lastBackupId = ""
        model.lastOptions = null
        model.nextPlan = ({
            settings: true,
            favorites: 1,
            idls: true,
            idl_count: 2,
            wallet: false,
            items: {
                favorites: [{ key: "fav-a", label: "Favorite A" }]
            },
            conflicts: {
                favorites: [{ area: "favorites", key: "fav-a", label: "Favorite A" }]
            },
            operation_decisions: [{ label: "Read CID" }],
            warnings: [{ message: "Wallet path differs." }]
        })
        dialog.backupId = ""
        dialog.reset()
    }

    function test_preview_tracks_options_and_text_projection() {
        dialog.backupId = "backup-1"
        dialog.preview()

        compare(model.lastBackupId, "backup-1")
        compare(model.lastOptions.settings, "replace")
        verify(dialog.planText().indexOf("Will import settings, 1 favorites, 2 IDLs.") >= 0)
        verify(dialog.planText().indexOf("Read CID") >= 0)
        verify(dialog.planText().indexOf("Wallet path differs.") >= 0)
    }

    function test_item_and_conflict_choices_update_preview_options() {
        dialog.backupId = "backup-1"
        dialog.preview()

        compare(dialog.itemRows("favorites").length, 1)
        verify(dialog.itemSelected("favorites", "fav-a"))

        dialog.setItemSelected("favorites", "fav-a", false)
        compare(model.lastOptions.items.favorites["fav-a"], false)

        verify(dialog.hasRequiredConflicts())
        compare(dialog.conflictDecisionIndexFor("favorites", "fav-a", conflictOptions), 0)
        dialog.setConflictDecision("favorites", "fav-a", "skip_backup_item")
        compare(model.lastOptions.conflicts.favorites["fav-a"], "skip_backup_item")
        verify(!dialog.hasRequiredConflicts())
        verify(dialog.confirmEnabled())
    }
}
