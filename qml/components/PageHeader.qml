import QtQuick
import QtQuick.Layouts
import "../theme"

GridLayout {
    id: root

    required property Theme theme
    property string breadcrumb: ""
    property string title: ""
    property string subtitle: ""
    property bool showTitle: false
    default property alias actions: actionSlot.data
    readonly property bool stacked: width < 680

    columns: root.stacked ? 1 : 2
    columnSpacing: theme.gap
    rowSpacing: theme.gapSmall
    Layout.fillWidth: true

    ColumnLayout {
        spacing: root.theme.gapTiny
        Layout.fillWidth: true
        Layout.column: 0
        Layout.row: 0

        Text {
            visible: root.breadcrumb.length > 0
            text: root.breadcrumb
            color: root.theme.textDim
            textFormat: Text.PlainText
            elide: Text.ElideRight
            font.pixelSize: root.theme.labelText
            font.weight: Font.DemiBold
            font.capitalization: Font.AllUppercase
            Layout.fillWidth: true
        }

        Text {
            visible: root.showTitle && root.title.length > 0
            text: root.title
            color: root.theme.text
            textFormat: Text.PlainText
            elide: Text.ElideRight
            font.pixelSize: root.theme.titleText
            font.weight: Font.Bold
            Layout.fillWidth: true
        }

        Text {
            visible: root.subtitle.length > 0
            text: root.subtitle
            color: root.theme.textMuted
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: root.theme.primaryText
            lineHeight: 1.25
            Layout.fillWidth: true
        }
    }

    RowLayout {
        id: actionSlot

        spacing: root.theme.gapSmall
        Layout.column: root.stacked ? 0 : 1
        Layout.row: root.stacked ? 1 : 0
        Layout.fillWidth: root.stacked
        Layout.alignment: root.stacked ? Qt.AlignLeft : (Qt.AlignTop | Qt.AlignRight)
    }

    Accessible.role: Accessible.StaticText
    Accessible.name: root.title.length > 0 ? root.title : root.breadcrumb
}
