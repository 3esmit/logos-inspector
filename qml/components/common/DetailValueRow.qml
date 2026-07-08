pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import ".."

LabeledValueRowBase {
    id: root

    property string value: "-"
    property string subvalue: ""
    property string linkKind: ""
    property string linkValue: ""
    property string copyText: linkValue.length > 0 ? linkValue : value
    property bool monospace: true
    property bool copyable: linkKind.length > 0
    property int valuePixelSize: root.theme.dataText
    signal activated(string kind, string value)

    LinkCell {
        text: root.value
        theme: root.theme
        link: root.linkKind.length > 0
        copyable: root.copyable
        copyText: root.copyText.length > 0 ? root.copyText : root.value
        monospace: root.monospace
        wrap: true
        textPixelSize: root.valuePixelSize
        Layout.fillWidth: true
        onActivated: root.activated(root.linkKind, root.linkValue)
    }

    Text {
        visible: root.subvalue.length > 0
        text: root.subvalue
        color: root.theme.textDim
        textFormat: Text.PlainText
        wrapMode: Text.WrapAnywhere
        font.family: "monospace"
        font.pixelSize: root.theme.dataText
        Layout.fillWidth: true
    }
}
