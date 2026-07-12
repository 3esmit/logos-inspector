pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../theme"
import "../ZonePresentation.js" as Presentation

Rectangle {
    id: root

    required property Theme theme
    required property var evidence
    property bool selected: false
    signal activated()

    objectName: "zoneEvidenceRow_" + String(root.evidence && root.evidence.reference
        && root.evidence.reference.evidence_id || "")
    implicitHeight: 76
    radius: root.theme.radius
    color: root.selected ? root.theme.accentMuted
        : (evidenceMouse.containsMouse ? root.theme.hover : root.theme.field)
    border.width: 1
    border.color: root.selected ? root.theme.accent : root.theme.outlineMuted
    activeFocusOnTab: true

    Keys.onEnterPressed: root.activated()
    Keys.onReturnPressed: root.activated()
    Keys.onSpacePressed: root.activated()

    MouseArea {
        id: evidenceMouse

        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.activated()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: root.theme.gapSmall
        spacing: root.theme.gapTiny

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Text {
                text: Presentation.evidenceKindLabel(root.evidence && root.evidence.reference
                    && root.evidence.reference.evidence_kind)
                color: root.theme.text
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font.pixelSize: root.theme.dataText
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            Text {
                text: qsTr("L1 %1 / op %2")
                    .arg(Presentation.numberText(root.evidence && root.evidence.reference
                        && root.evidence.reference.l1_slot))
                    .arg(Presentation.numberText(root.evidence && root.evidence.reference
                        && root.evidence.reference.operation_index))
                color: root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: root.theme.dataText
            }
        }

        LinkCell {
            theme: root.theme
            text: Presentation.text(root.evidence && root.evidence.reference
                && root.evidence.reference.transaction_hash)
            copyText: text === "-" ? "" : text
            copyable: copyText.length > 0
            copyInline: false
            monospace: true
            fitSingleLine: true
            minimumPixelSize: 8
            textPixelSize: root.theme.dataText
            textColor: root.theme.textMuted
            Layout.fillWidth: true
        }

        Text {
            text: qsTr("%1 / segment %2")
                .arg(Presentation.words(root.evidence && root.evidence.finality))
                .arg(Presentation.text(root.evidence && root.evidence.segment
                    && root.evidence.segment.segment_id))
            color: root.theme.textDim
            textFormat: Text.PlainText
            elide: Text.ElideRight
            font.pixelSize: root.theme.labelText
            Layout.fillWidth: true
        }
    }

    Accessible.role: Accessible.Button
    Accessible.name: qsTr("Open %1 evidence").arg(Presentation.evidenceKindLabel(
        root.evidence && root.evidence.reference && root.evidence.reference.evidence_kind
    ))
}
