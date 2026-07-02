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
    property bool showBreadcrumb: false
    property bool showSubtitle: false
    default property alias actions: actionSlot.data
    readonly property bool stacked: width < 680
    readonly property bool hasText: (showBreadcrumb && breadcrumb.length > 0)
        || (showTitle && title.length > 0)
        || (showSubtitle && subtitle.length > 0)

    columns: root.stacked || !root.hasText ? 1 : 2
    columnSpacing: theme.gap
    rowSpacing: theme.gapSmall
    Layout.fillWidth: true

    ColumnLayout {
        visible: root.hasText
        spacing: root.theme.gapTiny
        Layout.fillWidth: true
        Layout.column: 0
        Layout.row: 0

        Text {
            visible: root.showBreadcrumb && root.breadcrumb.length > 0
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
            visible: root.showSubtitle && root.subtitle.length > 0
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
        Layout.column: root.stacked || !root.hasText ? 0 : 1
        Layout.row: root.stacked && root.hasText ? 1 : 0
        Layout.fillWidth: root.stacked || !root.hasText
        Layout.alignment: root.stacked ? Qt.AlignLeft : (Qt.AlignTop | Qt.AlignRight)

        Item {
            visible: !root.stacked && !root.hasText
            Layout.fillWidth: true
        }
    }

    Accessible.role: Accessible.StaticText
    Accessible.name: root.title.length > 0 ? root.title : root.breadcrumb
}
