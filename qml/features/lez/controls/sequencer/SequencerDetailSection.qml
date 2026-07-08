pragma ComponentBehavior: Bound

import QtQuick.Layouts
import "../../../../components/common"
import "../../../../state"
import "../../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property string title: ""
    property var rows: []
    property AppModel modelRef

    visible: root.rows.length > 0
    spacing: 0
    Layout.fillWidth: true

    DetailSection {
        theme: root.theme
        title: root.title
        rows: root.rows
        labelWidth: 128
        surfaceColor: root.theme.surface
        onLinkActivated: function (kind, value) {
            if (root.modelRef !== null) {
                root.modelRef.openReference(kind, value)
            }
        }
    }
}
