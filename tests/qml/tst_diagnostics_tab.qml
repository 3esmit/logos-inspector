pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/features/modules/controls"
import "../../qml/theme"

TestCase {
    id: testRoot

    name: "DiagnosticsTab"
    when: windowShown
    width: 900
    height: 700

    Theme {
        id: theme
    }

    ApplicationWindow {
        id: testWindow

        visible: true
        width: testRoot.width
        height: testRoot.height
        color: theme.background

        DiagnosticsTab {
            id: tab

            theme: theme
            readTitle: "Read"
            guardedTitle: "Actions"
            guardedActions: [{
                action: "live",
                text: "Live action",
                enabled: true
            }, {
                action: "pending",
                text: "Pending action"
            }]
            evidenceRows: [{
                label: "storage_module.dataDir",
                state: "ok",
                evidence: ".../storage-data",
                source: "LogosCore CLI (Storage)",
                freshness: "09:42:00",
                tone: "success"
            }]
            width: testWindow.width
        }
    }

    SignalSpy {
        id: actionSpy

        target: tab
        signalName: "guardedActionRequested"
    }

    function init() {
        actionSpy.clear()
        tab.pending = false
        tab.guardedActions = [{
            action: "live",
            text: "Live action",
            enabled: true
        }, {
            action: "pending",
            text: "Pending action"
        }]
        wait(0)
    }

    function test_actions_are_explicitly_enabled_and_emit_exact_key() {
        const liveAction = findAccessibleByName(tab, "Live action")
        const pendingAction = findAccessibleByName(tab, "Pending action")
        verify(liveAction !== null)
        verify(pendingAction !== null)
        verify(liveAction.enabled)
        verify(!pendingAction.enabled)
        verify(findVisibleText(tab, "Adapters pending") !== null)

        mouseClick(liveAction, liveAction.width / 2, liveAction.height / 2)

        compare(actionSpy.count, 1)
        compare(actionSpy.signalArguments[0][0], "live")

        tab.pending = true
        tryVerify(function () { return !liveAction.enabled })
    }

    function test_pending_label_hides_when_every_action_is_live() {
        tab.guardedActions = [{
            action: "live",
            text: "Live action",
            enabled: true
        }]

        tryVerify(function () {
            return findVisibleText(tab, "Adapters pending") === null
        })
    }

    function test_evidence_row_exposes_complete_accessible_semantics() {
        const row = findAccessibleByName(tab, "storage_module.dataDir: ok")

        verify(row !== null)
        compare(row.Accessible.role, Accessible.StaticText)
        compare(row.Accessible.description,
                ".../storage-data. LogosCore CLI (Storage). 09:42:00")
        verify(findVisibleText(tab, "storage_module.dataDir").Accessible.ignored)
        verify(findVisibleText(tab, "ok").Accessible.ignored)
        verify(findVisibleText(tab, ".../storage-data").Accessible.ignored)
        verify(findVisibleText(
                tab, "LogosCore CLI (Storage) / 09:42:00").Accessible.ignored)
    }

    function findAccessibleByName(item, expectedName) {
        if (!item) {
            return null
        }
        if (item.Accessible && String(item.Accessible.name || "") === expectedName
                && item.visible) {
            return item
        }
        const children = item.children || []
        for (let i = 0; i < children.length; ++i) {
            const match = findAccessibleByName(children[i], expectedName)
            if (match) {
                return match
            }
        }
        return null
    }

    function findVisibleText(item, expectedText) {
        if (!item) {
            return null
        }
        if (item.visible && item.text !== undefined
                && String(item.text || "") === expectedText) {
            return item
        }
        const children = item.children || []
        for (let i = 0; i < children.length; ++i) {
            const match = findVisibleText(children[i], expectedText)
            if (match) {
                return match
            }
        }
        return null
    }
}
