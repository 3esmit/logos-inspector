import QtQuick
import LogosInspectorStandalone

Window {
    id: root

    width: 1180
    height: 820
    visible: true
    title: qsTr("Logos Inspector")

    LogosBridge {
        id: logosBridge
    }

    AppShell {
        anchors.fill: parent
        bridgeHost: logosBridge
    }
}
