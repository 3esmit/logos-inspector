import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/features/settings/controls"
import "../../qml/theme"

TestCase {
    id: testRoot

    name: "SettingsControls"
    when: windowShown
    width: 640
    height: 300

    Theme {
        id: theme
    }

    ApplicationWindow {
        id: testWindow

        visible: true
        width: testRoot.width
        height: testRoot.height
        color: theme.background

        Column {
            anchors.centerIn: parent
            spacing: 16

            FieldToggle {
                id: fieldToggle

                theme: theme
                label: qsTr("Configured Zone")
                detail: qsTr("Show the configured Zone dashboard in navigation.")
            }

            SafetyToggle {
                id: safetyToggle

                theme: theme
                text: qsTr("Include local paths")
                detail: qsTr("Shows local paths in the UI.")
            }

            Button {
                id: destination

                text: qsTr("Destination")
            }
        }
    }

    function init() {
        mouseMove(destination, destination.width / 2, destination.height / 2)
        wait(0)
    }

    function test_field_toggle_focus_does_not_open_input_overlay() {
        fieldToggle.forceActiveFocus()

        tryCompare(fieldToggle.ToolTip, "visible", false)
        compare(fieldToggle.Accessible.description,
                "Show the configured Zone dashboard in navigation.")
    }

    function test_safety_toggle_focus_does_not_open_input_overlay() {
        safetyToggle.forceActiveFocus()

        tryCompare(safetyToggle.ToolTip, "visible", false)
        compare(safetyToggle.Accessible.description,
                "Shows local paths in the UI.")
    }

    function test_tooltips_remain_available_on_hover() {
        mouseMove(fieldToggle, fieldToggle.width / 2, fieldToggle.height / 2)
        tryCompare(fieldToggle.ToolTip, "visible", true)

        mouseMove(destination, destination.width / 2, destination.height / 2)
        tryCompare(fieldToggle.ToolTip, "visible", false)

        mouseMove(safetyToggle, safetyToggle.width / 2, safetyToggle.height / 2)
        tryCompare(safetyToggle.ToolTip, "visible", true)
    }
}
