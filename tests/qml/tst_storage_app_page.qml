pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/features/storage/pages"
import "../../qml/state"
import "../../qml/theme"
import "fixtures"

TestCase {
    id: testRoot

    name: "StorageAppPage"
    when: windowShown
    width: 1280
    height: 900

    Theme {
        id: theme
    }

    StateGatewayFixture {
        id: gateway
    }

    QtObject {
        id: gate

        property int revision: 0
        property bool allowActions: false

        function storageGate() {
            return {
                enabled: allowActions,
                status: allowActions ? "enabled" : "disabled",
                missing: allowActions ? [] : [{
                    dependency: "storage",
                    label: "Storage capability",
                    status: "unavailable",
                    capability: "storage",
                    provenance: "test"
                }],
                warnings: [],
                provenance: ["test"]
            }
        }
    }

    StorageAppState {
        id: storageState

        gateway: gateway
        gateFacade: gate
        currentView: "storage"
        effectiveSourceMode: "none"
    }

    ApplicationWindow {
        id: testWindow

        visible: true
        width: testRoot.width
        height: testRoot.height
        color: theme.background

        StorageAppPage {
            id: page

            theme: theme
            model: storageState
            width: testWindow.width
        }
    }

    function init() {
        gateway.reset()
        gate.allowActions = false
        gate.revision += 1
        storageState.operationSession.reset()
        storageState.currentTab = "files"
        storageState.activeCid = ""
        storageState.cidProbe = ""
        storageState.manifests = [{
            cid: "z-semantic-cid-1",
            filename: "semantic.bin",
            datasetSize: 42,
            blockSize: 65536,
            mimetype: "application/octet-stream"
        }, {
            cid: "z-semantic-cid-2",
            filename: "semantic.bin",
            datasetSize: 84,
            blockSize: 32768,
            mimetype: "application/json"
        }]
        wait(0)
    }

    function test_manifest_row_exposes_full_identity_and_contextual_use_action() {
        const rowNames = []
        collectAccessibleNames(page, "semantic.bin, CID ", rowNames)
        compare(rowNames, [
            "semantic.bin, CID z-semantic-cid-1",
            "semantic.bin, CID z-semantic-cid-2"
        ])

        const firstRow = findAccessibleByName(
            page, "semantic.bin, CID z-semantic-cid-1")
        const secondRow = findAccessibleByName(
            page, "semantic.bin, CID z-semantic-cid-2")
        verify(firstRow !== null)
        verify(secondRow !== null)
        compare(firstRow.Accessible.role, Accessible.StaticText)
        compare(secondRow.Accessible.role, Accessible.StaticText)
        compare(
            String(firstRow.Accessible.description),
            "Size 42. Type application/octet-stream. block 65536")
        compare(
            String(secondRow.Accessible.description),
            "Size 84. Type application/json. block 32768")

        const useNames = []
        collectAccessibleNames(page, "Use CID ", useNames)
        compare(useNames, [
            "Use CID z-semantic-cid-1 for semantic.bin",
            "Use CID z-semantic-cid-2 for semantic.bin"
        ])
        const secondUseButton = findAccessibleByName(
            page, "Use CID z-semantic-cid-2 for semantic.bin")
        verify(secondUseButton !== null)
        compare(secondUseButton.Accessible.role, Accessible.Button)
        verify(secondUseButton.enabled)
    }

    function test_contextual_use_action_projects_exact_cid_into_cid_tab() {
        const useButton = findAccessibleByName(
            page, "Use CID z-semantic-cid-2 for semantic.bin")
        verify(useButton !== null)

        mouseClick(useButton, useButton.width / 2, useButton.height / 2)

        tryCompare(storageState, "currentTab", "cid")
        compare(storageState.activeCid, "z-semantic-cid-2")
        tryVerify(function () {
            const cidField = findChild(page, "storageCidFieldInput")
            return cidField !== null && String(cidField.text) === "z-semantic-cid-2"
        })
    }

    function test_empty_manifest_row_has_no_fake_use_action() {
        storageState.manifests = []
        wait(0)

        const emptyRow = findAccessibleByName(page, "No local manifests")
        verify(emptyRow !== null)
        compare(emptyRow.Accessible.role, Accessible.StaticText)

        const fakeUseAction = findAccessibleByName(
            page, "Use manifest No local manifests")
        verify(fakeUseAction === null || !fakeUseAction.visible)
    }

    function test_operation_history_row_exposes_time_status_and_detail() {
        storageState.operationSession.operationLog = [{
            time: "12:34:56",
            label: "Storage manifests",
            status: "ok",
            detail: "15 manifests"
        }]
        storageState.operationSession.operationLogRevision += 1
        storageState.currentTab = "operations"

        let row = null
        tryVerify(function () {
            row = findAccessibleByName(page, "Storage manifests: ok")
            return row !== null
        })
        compare(row.Accessible.role, Accessible.StaticText)
        compare(String(row.Accessible.description), "12:34:56. 15 manifests")
    }

    function test_fetch_disables_while_storage_operation_is_busy() {
        gate.allowActions = true
        gate.revision += 1
        storageState.activeCid = "z-semantic-cid-1"
        storageState.currentTab = "cid"

        let fetchButton = null
        tryVerify(function () {
            fetchButton = findAccessibleByName(page, "Fetch", Accessible.Button)
            return fetchButton !== null && fetchButton.enabled
        })

        storageState.operationSession.startPending = true

        tryVerify(function () { return !fetchButton.enabled })
    }

    function findAccessibleByName(item, expectedName, expectedRole) {
        if (!item) {
            return null
        }
        if (item.Accessible
                && String(item.Accessible.name) === String(expectedName)
                && (expectedRole === undefined
                    || item.Accessible.role === expectedRole)) {
            return item
        }
        const children = item.children || []
        for (let i = 0; i < children.length; ++i) {
            const match = findAccessibleByName(
                children[i], expectedName, expectedRole)
            if (match) {
                return match
            }
        }
        return null
    }

    function collectAccessibleNames(item, prefix, output) {
        if (!item) {
            return
        }
        if (item.Accessible
                && String(item.Accessible.name).startsWith(String(prefix))) {
            output.push(String(item.Accessible.name))
        }
        const children = item.children || []
        for (let i = 0; i < children.length; ++i) {
            collectAccessibleNames(children[i], prefix, output)
        }
    }
}
