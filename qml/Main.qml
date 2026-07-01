import QtQuick

AppShell {
    id: root

    bridgeHost: typeof logos === "undefined" ? null : logos
    width: parent ? parent.width : 1180
    height: parent ? parent.height : 820
}
