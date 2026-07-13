pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

TestCase {
    id: testRoot

    name: "AppShellNavigation"
    when: windowShown
    width: 1180
    height: 820

    ApplicationWindow {
        id: testWindow

        visible: true
        width: 1180
        height: 820

        AppShell {
            id: shell

            anchors.fill: parent
        }
    }

    function test_visible_nav_buttons_replace_loaded_page() {
        const model = findChild(shell, "appModel")
        const loader = findChild(shell, "pageLoader")
        verify(model !== null)
        verify(loader !== null)
        tryVerify(function () { return loader.sourceComponent !== null && loader.item !== null })
        compare(model.shell.currentView, "overview")
        model.chainPages.dashboardNode = null
        compare(model.chainPages.sourceEmptyText("blockchain", "", "No blocks"), "No blocks")

        verifyButtonNavigation(model, loader, "blocks")

        const views = [
            "transactions",
            "zones",
            "blockchain",
            "storage",
            "messaging",
            "favorites",
            "programs",
            "localWallet",
            "localNodes",
            "overview"
        ]
        for (let i = 0; i < views.length; ++i) {
            verifyStateNavigation(model, loader, views[i])
        }
    }

    function verifyButtonNavigation(model, loader, view) {
        const previousSource = loader.sourceComponent
        let button = null
        tryVerify(function () {
            button = findChild(shell, "navButton_" + view)
            return button !== null && button.visible
        })

        mouseClick(button, button.width / 2, button.height / 2)

        verifyLoadedView(model, loader, view, previousSource)
    }

    function verifyStateNavigation(model, loader, view) {
        const previousSource = loader.sourceComponent
        model.shell.selectView(view)
        verifyLoadedView(model, loader, view, previousSource)
    }

    function verifyLoadedView(model, loader, view, previousSource) {
        tryCompare(model.shell, "currentView", view)
        tryVerify(function () { return loader.sourceComponent !== previousSource })
        tryVerify(function () { return loader.item !== null })
        tryVerify(function () { return loader.item.implicitHeight > 0 && loader.item.height > 0 })
    }
}
