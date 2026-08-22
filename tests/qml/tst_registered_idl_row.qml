pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/theme"
import "../../qml/features/lez/controls/programs"

TestCase {
    id: testRoot

    name: "RegisteredIdlRow"
    when: windowShown
    width: 640
    height: 180

    Theme {
        id: theme
    }

    ApplicationWindow {
        id: testWindow

        visible: true
        width: 640
        height: 180
        color: theme.background

        RegisteredIdlRow {
            id: row

            anchors.fill: parent
            theme: theme
            idlName: "lez-token-0.1.0"
            programIdText: "token-program"
            fieldCount: 12
        }
    }

    function init() {
        row.idlName = "lez-token-0.1.0"
        row.programIdText = "token-program"
        row.fieldCount = 12
    }

    function test_registered_idl_row_exposes_identity_and_context() {
        verify(row.visible)
        compare(row.Accessible.role, Accessible.ListItem)
        compare(row.Accessible.name, "lez-token-0.1.0")
        compare(row.Accessible.description, "Program token-program. 12 field(s)")

        const removeButton = findAccessibleByName(row, "Remove")
        verify(removeButton !== null)
        compare(removeButton.Accessible.role, Accessible.Button)
        verify(removeButton.enabled)
    }

    function test_unnamed_idl_row_exposes_fallback_context() {
        row.idlName = ""
        row.programIdText = ""
        row.fieldCount = 0

        tryCompare(row.Accessible, "name", "Unnamed IDL")
        tryCompare(row.Accessible, "description", "0 field(s). No program binding")
    }

    function findAccessibleByName(item, expected) {
        if (!item) {
            return null
        }
        if (item.Accessible && String(item.Accessible.name) === expected) {
            return item
        }
        const children = item.children || []
        for (let index = 0; index < children.length; ++index) {
            const match = findAccessibleByName(children[index], expected)
            if (match) {
                return match
            }
        }
        return null
    }
}
