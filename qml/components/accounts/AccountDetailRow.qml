pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import ".."
import "../common"

LabeledValueRowBase {
    id: root

    property string value: ""
    property string subvalue: ""
    property string subvalueCopyText: ""
    property string linkKind: ""
    property string linkValue: ""
    property string tooltipText: ""
    property bool monospace: true

    signal activated()

    labelPixelSize: 11
    labelMaximumLineCount: 2
    labelWrapMode: Text.Wrap

    LinkCell {
        text: root.value
        theme: root.theme
        link: root.linkKind.length > 0
        copyText: root.linkValue.length > 0 ? root.linkValue : root.value
        tooltipText: root.tooltipText
        monospace: root.monospace
        wrap: true
        Layout.fillWidth: true
        onActivated: root.activated()
    }

    LinkCell {
        visible: root.subvalue.length > 0
        text: root.subvalue
        theme: root.theme
        copyable: root.subvalueCopyText.length > 0
        copyText: root.subvalueCopyText
        monospace: true
        wrap: true
        textColor: root.theme.textDim
        textPixelSize: 11
        Layout.fillWidth: true
    }
}
