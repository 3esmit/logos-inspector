pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtTest
import "../../qml/features/modules/controls"
import "../../qml/theme"

TestCase {
    id: testRoot

    name: "ProtocolRowsPanel"
    when: windowShown
    width: 900
    height: 500

    Theme {
        id: theme
    }

    ApplicationWindow {
        id: testWindow

        visible: true
        width: testRoot.width
        height: testRoot.height
        color: theme.background

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: theme.gap
            spacing: theme.gap

            ProtocolRowsPanel {
                id: storagePanel

                theme: theme
                title: "Storage protocols"
                rows: testRoot.storageRows()
                Layout.fillWidth: true
            }

            ProtocolRowsPanel {
                id: deliveryPanel

                theme: theme
                title: "Delivery protocols"
                rows: testRoot.deliveryRows()
                Layout.fillWidth: true
            }

            Item {
                Layout.fillHeight: true
            }
        }
    }

    function init() {
        storagePanel.rows = testRoot.storageRows()
        deliveryPanel.rows = testRoot.deliveryRows()
        wait(0)
    }

    function test_storage_row_exposes_complete_semantics_and_exact_copy() {
        const row = findAccessibleByName(
                storagePanel,
                "Store / RepoStore: observed. Protocol ID: repository. 43 manifests")
        const copy = findAccessibleByName(
                storagePanel, "Copy Store / RepoStore protocol ID")

        verify(row !== null)
        compare(row.Accessible.role, Accessible.StaticText)
        compare(row.copyText, "repository")
        verify(copy !== null)
        compare(copy.Accessible.role, Accessible.Button)
        compare(copy.Accessible.description,
                "Copies exact Store / RepoStore protocol ID.")
        verify(findVisibleText(storagePanel, "Store / RepoStore").Accessible.ignored)
        verify(findVisibleText(storagePanel, "observed").Accessible.ignored)
        verify(findVisibleText(storagePanel, "43 manifests").Accessible.ignored)
    }

    function test_delivery_row_exposes_complete_semantics_and_exact_copy() {
        const row = findAccessibleByName(
                deliveryPanel,
                "Peer exchange: unknown. Protocol ID: "
                    + "/vac/waku/peer-exchange/2.0.0-alpha1. No passive evidence")
        const copy = findAccessibleByName(
                deliveryPanel, "Copy Peer exchange protocol ID")

        verify(row !== null)
        compare(row.Accessible.role, Accessible.StaticText)
        compare(row.copyText, "/vac/waku/peer-exchange/2.0.0-alpha1")
        verify(copy !== null)
        compare(copy.Accessible.role, Accessible.Button)
        compare(copy.Accessible.description,
                "Copies exact Peer exchange protocol ID.")
        verify(findVisibleText(deliveryPanel, "Peer exchange").Accessible.ignored)
        verify(findVisibleText(deliveryPanel, "unknown").Accessible.ignored)
        verify(findVisibleText(deliveryPanel, "No passive evidence").Accessible.ignored)
    }

    function test_delivery_health_row_without_protocol_id_has_no_copy() {
        const row = findAccessibleByName(
                deliveryPanel, "Relay: healthy. 6 connected peers")

        verify(row !== null)
        compare(row.Accessible.role, Accessible.StaticText)
        compare(row.copyText, "")
        compare(findAccessibleByName(
                deliveryPanel, "Copy Relay protocol ID"), null)
    }

    function test_multiline_evidence_is_normalized_and_bounded() {
        const evidence = "first line\n" + "x".repeat(300)
        const normalized = evidence.replace(/\s+/g, " ").trim()
        const bounded = normalized.slice(0, 237) + "..."
        storagePanel.rows = [{
            label: "Store / RepoStore",
            protocolId: "repository",
            state: "observed",
            evidence: evidence,
            tone: "success"
        }]
        wait(0)

        const expected = "Store / RepoStore: observed. Protocol ID: repository. "
            + bounded
        const row = findAccessibleByName(storagePanel, expected)

        verify(row !== null)
        compare(row.Accessible.name, expected)
        verify(String(row.Accessible.name).indexOf("\n") < 0)
    }

    function test_source_derived_owner_and_copy_context_are_bounded() {
        const label = "Relay label\n" + "l".repeat(180)
        const state = "healthy\n" + "s".repeat(120)
        const protocolId = "/vac/waku/custom/" + "p".repeat(240)
        const evidence = "peer evidence\n" + "e".repeat(300)
        const boundedLabel = label.replace(/\s+/g, " ").trim().slice(0, 93) + "..."
        storagePanel.rows = [{
            label: label,
            protocolId: protocolId,
            state: state,
            evidence: evidence,
            tone: "success"
        }]
        wait(0)

        const row = findOwnerByCopyText(storagePanel, protocolId)
        const copy = findAccessibleByName(
                storagePanel, "Copy " + boundedLabel + " protocol ID")

        verify(row !== null)
        compare(row.Accessible.name.length, 384)
        verify(String(row.Accessible.name).indexOf("\n") < 0)
        verify(String(row.Accessible.name).endsWith("..."))
        verify(copy !== null)
        compare(copy.Accessible.description,
                "Copies exact " + boundedLabel + " protocol ID.")
        compare(row.copyText, protocolId)
    }

    function storageRows() {
        return [{
            label: "Store / RepoStore",
            protocolId: "repository",
            state: "observed",
            evidence: "43 manifests",
            tone: "success"
        }]
    }

    function deliveryRows() {
        return [{
            label: "Peer exchange",
            protocolId: "/vac/waku/peer-exchange/2.0.0-alpha1",
            state: "unknown",
            evidence: "No passive evidence",
            tone: "neutral"
        }, {
            label: "Relay",
            protocolId: "",
            state: "healthy",
            evidence: "6 connected peers",
            tone: "success"
        }]
    }

    function findAccessibleByName(item, expectedName) {
        if (!item) {
            return null
        }
        if (item.Accessible && !item.Accessible.ignored
                && String(item.Accessible.name || "") === expectedName
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

    function findOwnerByCopyText(item, expectedCopyText) {
        if (!item) {
            return null
        }
        if (item.Accessible && !item.Accessible.ignored
                && item.Accessible.role === Accessible.StaticText
                && item.copyText !== undefined
                && String(item.copyText || "") === expectedCopyText
                && item.visible) {
            return item
        }
        const children = item.children || []
        for (let i = 0; i < children.length; ++i) {
            const match = findOwnerByCopyText(children[i], expectedCopyText)
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
