pragma ComponentBehavior: Bound

import QtQuick
import "../../../state"
import "../../../theme"

Item {
    id: root

    required property Theme theme
    required property AppModel model

    readonly property Item loadedPage: pageLoader.item as Item

    implicitHeight: root.loadedPage ? root.loadedPage.implicitHeight : 0

    Loader {
        id: pageLoader

        anchors.fill: parent
        active: true
        asynchronous: true
        sourceComponent: root.model.prefersBasecampModules()
            ? basecampWalletPage : localWalletPage
    }

    Component {
        id: localWalletPage

        LocalWalletPage {
            theme: root.theme
            model: root.model
        }
    }

    Component {
        id: basecampWalletPage

        BasecampWalletPage {
            theme: root.theme
            model: root.model
        }
    }
}
