import QtQuick
import LogosInspectorStandalone

Window {
    id: root

    width: 1180
    height: root.integerArgument("--window-height", 820)
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

    function integerArgument(name, fallback) {
        const args = Qt.application.arguments || []
        for (let i = 0; i < args.length - 1; i += 1) {
            if (args[i] === name) {
                const parsed = Number.parseInt(args[i + 1], 10)
                if (Number.isFinite(parsed) && parsed > 0) {
                    return parsed
                }
            }
        }
        return fallback
    }
}
