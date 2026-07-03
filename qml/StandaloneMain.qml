import QtQuick
import LogosInspectorStandalone

Window {
    id: root

    width: 1180
    height: 820
    minimumWidth: 760
    minimumHeight: 620
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
