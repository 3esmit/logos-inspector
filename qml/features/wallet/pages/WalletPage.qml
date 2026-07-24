pragma ComponentBehavior: Bound

import QtQuick
import "../../../state"
import "../../../theme"

Loader {
    id: root

    required property Theme theme
    required property AppModel model

    active: true
    asynchronous: true
    width: parent ? parent.width : 900
    sourceComponent: root.model.prefersBasecampModules() ? basecampWalletPage : localWalletPage

    Component {
        id: localWalletPage

        LocalWalletPage {
            theme: root.theme
            model: root.model
        }
    }

    Component {
        id: basecampWalletPage

        BasecampLezWalletPage {
            theme: root.theme
            model: root.model
        }
    }
}
