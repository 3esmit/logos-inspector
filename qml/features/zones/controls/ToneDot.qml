import QtQuick
import "../../../theme"

Rectangle {
    id: root

    required property Theme theme
    property string tone: "neutral"

    implicitWidth: 8
    implicitHeight: 8
    radius: 4
    color: root.tone === "success" ? root.theme.success
        : (root.tone === "warning" ? root.theme.warning
        : (root.tone === "error" ? root.theme.error
        : (root.tone === "info" ? root.theme.info : root.theme.outline)))

    Accessible.ignored: true
}
