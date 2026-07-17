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
        height: 720
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

            ListToolbar {
                id: listToolbar

                theme: theme
                loadCount: 20
                Layout.fillWidth: true
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

            SourceSettingsPanel {
                id: sourceSettingsPanel

                theme: theme
                title: qsTr("Bedrock Blockchain")
                statusText: qsTr("Checking")
                statusDetail: ""
                Layout.fillWidth: true
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
                        {
                            text: "row-visible-value",
                            width: 180,
                            fill: true,
                            link: true,
                            copyText: "complete-row-value",
                            accessibleName: "Open exact row value",
                            accessibleDescription: "Opens the complete row target",
                            copyAccessibleName: "Copy complete row value",
                            copyAccessibleDescription: "Copies the complete row target"
                        }
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
        listToolbar.loadCount = 20
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

    function test_data_table_forwards_link_and_copy_accessibility() {
        const link = findAccessibleByName(dataTableFrame, "Open exact row value")
        const copy = findAccessibleByName(dataTableFrame, "Copy complete row value")
        verify(link !== null)
        verify(copy !== null)
        verify(link.visible)
        verify(link.enabled)
        verify(copy.visible)
        verify(copy.enabled)
        compare(link.Accessible.role, Accessible.Link)
        compare(link.Accessible.description, "Opens the complete row target")
        compare(link.copyText, "complete-row-value")
        compare(copy.Accessible.role, Accessible.Button)
        compare(copy.Accessible.description, "Copies the complete row target")
    }

    function test_loaded_row_count_exposes_current_value() {
        const combo = findAccessibleByName(listToolbar, "Loaded row count")
        verify(combo !== null)
        compare(combo.Accessible.description, "20")

        listToolbar.loadCount = 50

        tryCompare(combo.Accessible, "description", "50")
    }

    function test_source_status_exposes_context_and_detail() {
        const status = findAccessibleByName(
            sourceSettingsPanel, "Bedrock Blockchain status: Checking")
        verify(status !== null)
        compare(status.Accessible.role, Accessible.StaticText)

        sourceSettingsPanel.statusText = qsTr("OK")
        sourceSettingsPanel.statusDetail = qsTr("slot 77 at now")

        tryCompare(status.Accessible, "name", "Bedrock Blockchain status: OK")
        tryCompare(status.Accessible, "description", "slot 77 at now")
    }

    function test_confirm_popup_accept_action() {
        confirmPopup.open()
        tryCompare(confirmPopup, "opened", true)

        const messageText = findChild(confirmPopup.contentItem, "messageText")
        verify(messageText !== null)
        compare(messageText.wrapMode, Text.Wrap)

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

    function findAccessibleByName(item, expected) {
        if (!item) {
            return null
        }
        if (item.Accessible && String(item.Accessible.name) === expected) {
            return item
        }
        const children = item.children || []
        for (let i = 0; i < children.length; ++i) {
            const match = findAccessibleByName(children[i], expected)
            if (match) {
                return match
            }
        }
        return null
    }
}
