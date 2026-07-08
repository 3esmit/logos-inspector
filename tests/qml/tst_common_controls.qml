import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtTest
import "../../qml/theme"
import "../../qml/components"
import "../../qml/components/common"
import "../../qml/features/settings/controls"

TestCase {
    id: testRoot

    name: "CommonControls"
    when: windowShown
    width: 640
    height: 480

    property int actionClicks: 0
    property int acceptedCount: 0

    Theme {
        id: theme
    }

    ApplicationWindow {
        id: testWindow

        visible: true
        width: 640
        height: 480
        color: theme.background

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: theme.gap
            spacing: theme.gap

            ActionButton {
                id: actionButton

                theme: theme
                text: qsTr("Run")
                primary: true
                onClicked: testRoot.actionClicks += 1
            }

            SummarySection {
                id: summarySection

                theme: theme
                title: qsTr("Program")
                rows: [
                    { "title": "Program ID", "detail": "0x1234" },
                    { "title": "Owner", "detail": "Account" }
                ]
            }

            InfoField {
                id: infoField

                theme: theme
                label: qsTr("Endpoint")
                value: "http://127.0.0.1:3040/"
            }

            DataTableFrame {
                id: dataTableFrame

                theme: theme
                headerCells: [
                    { text: "Kind", width: 120 },
                    { text: "Value", width: 180, fill: true }
                ]
                rows: [
                    { cells: [
                        { text: "block", width: 120 },
                        { text: "row-visible-value", width: 180, fill: true }
                    ] }
                ]
                Layout.fillWidth: true
            }
        }

        ConfirmActionPopup {
            id: confirmPopup

            theme: theme
            title: qsTr("Deploy")
            message: qsTr("Deploy selected binary")
            confirmText: qsTr("Deploy")
            onAccepted: testRoot.acceptedCount += 1
        }
    }

    function init() {
        actionClicks = 0
        acceptedCount = 0
        confirmPopup.close()
    }

    function test_action_button_click() {
        verify(actionButton.visible)
        verify(actionButton.width > 0)
        verify(actionButton.height > 0)

        mouseClick(actionButton, actionButton.width / 2, actionButton.height / 2)

        compare(actionClicks, 1)
    }

    function test_summary_and_info_render() {
        verify(summarySection.visible)
        compare(summarySection.rows.length, 2)
        verify(summarySection.implicitHeight > 0)
        compare(infoField.value, "http://127.0.0.1:3040/")
        verify(infoField.implicitHeight > 0)
    }

    function test_data_table_frame_renders_row_cell_text() {
        verify(dataTableFrame.visible)
        verify(dataTableFrame.width > 0)
        verify(hasVisibleText(dataTableFrame, "row-visible-value"))
    }

    function test_confirm_popup_accept_action() {
        confirmPopup.open()
        tryCompare(confirmPopup, "opened", true)

        const confirmButton = findChild(confirmPopup.contentItem, "confirmButton")
        verify(confirmButton !== null)
        mouseClick(confirmButton, confirmButton.width / 2, confirmButton.height / 2)

        tryCompare(confirmPopup, "opened", false)
        compare(acceptedCount, 1)
    }

    function hasVisibleText(item, expected) {
        if (!item) {
            return false
        }
        if (item.text !== undefined && String(item.text) === expected && item.visible && item.width > 0 && item.height > 0) {
            return true
        }
        if (item.contentItem && hasVisibleText(item.contentItem, expected)) {
            return true
        }
        const children = item.children || []
        for (let i = 0; i < children.length; ++i) {
            if (hasVisibleText(children[i], expected)) {
                return true
            }
        }
        return false
    }
}
