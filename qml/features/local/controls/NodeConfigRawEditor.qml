pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property string text: ""
    property string errorMessage: ""
    property bool editable: false
    property int rows: 16
    signal textEdited(string text)

    objectName: "nodeConfigRawEditor"
    spacing: root.theme.gapSmall
    Layout.fillWidth: true

    Text {
        text: qsTr("Raw JSON configuration")
        color: root.enabled ? root.theme.textMuted : root.theme.textDim
        textFormat: Text.PlainText
        font.pixelSize: root.theme.secondaryText
        font.weight: Font.Medium
        Layout.fillWidth: true
    }

    TextArea {
        id: editor

        objectName: "nodeConfigRawInput"
        readOnly: !root.editable
        wrapMode: TextArea.NoWrap
        color: enabled ? root.theme.text : root.theme.textDim
        selectionColor: root.theme.accent
        selectedTextColor: root.theme.selectedText
        font.family: "monospace"
        font.pixelSize: root.theme.dataText
        leftPadding: 12
        rightPadding: 12
        topPadding: 10
        bottomPadding: 10
        hoverEnabled: true
        Layout.fillWidth: true
        Layout.preferredHeight: Math.max(192, root.rows * 22)
        onTextEdited: root.textEdited(text)

        Binding {
            target: editor
            property: "text"
            value: root.text
            when: !editor.activeFocus
        }

        background: Rectangle {
            radius: root.theme.radius
            color: editor.hovered || editor.activeFocus
                ? root.theme.surfaceRaised : root.theme.field
            border.width: editor.activeFocus || root.errorMessage.length > 0 ? 2 : 1
            border.color: root.errorMessage.length > 0 ? root.theme.error
                : (editor.activeFocus ? root.theme.accent : root.theme.outlineMuted)
        }

        Accessible.name: qsTr("Raw JSON node configuration")
        Accessible.description: root.errorMessage.length > 0
            ? qsTr("Error: %1").arg(root.errorMessage)
            : (root.editable ? qsTr("Editable JSON configuration")
                : qsTr("Read-only JSON configuration"))
    }

    Text {
        visible: root.errorMessage.length > 0
        text: root.errorMessage
        color: root.theme.error
        textFormat: Text.PlainText
        wrapMode: Text.WrapAnywhere
        font.pixelSize: root.theme.dataText
        Layout.fillWidth: true
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }

    Text {
        text: qsTr("Syntax-highlighted preview")
        color: root.theme.textMuted
        textFormat: Text.PlainText
        font.pixelSize: root.theme.secondaryText
        font.weight: Font.Medium
        Layout.fillWidth: true
    }

    Frame {
        padding: 10
        Layout.fillWidth: true
        Layout.preferredHeight: Math.max(128, Math.min(300, preview.implicitHeight + 20))

        background: Rectangle {
            color: root.theme.field
            radius: root.theme.radius
            border.width: 1
            border.color: root.theme.outlineMuted
        }

        contentItem: Flickable {
            id: previewViewport

            contentWidth: Math.max(width, preview.implicitWidth)
            contentHeight: preview.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            flickableDirection: Flickable.AutoFlickIfNeeded

            Text {
                id: preview

                width: Math.max(previewViewport.width, implicitWidth)
                text: root.highlightJson(root.text)
                color: root.theme.text
                textFormat: Text.RichText
                wrapMode: Text.NoWrap
                font.family: "monospace"
                font.pixelSize: root.theme.dataText
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("Syntax-highlighted JSON preview")
            }

            ScrollBar.vertical: ScrollBar {
                policy: previewViewport.contentHeight > previewViewport.height
                    ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff
            }

            ScrollBar.horizontal: ScrollBar {
                policy: previewViewport.contentWidth > previewViewport.width
                    ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff
            }
        }
    }

    function escapeHtml(value) {
        return String(value || "")
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/\"/g, "&quot;")
    }

    function whitespaceHtml(character) {
        if (character === " ") {
            return "&nbsp;"
        }
        if (character === "\t") {
            return "&nbsp;&nbsp;&nbsp;&nbsp;"
        }
        if (character === "\n") {
            return "<br/>"
        }
        return root.escapeHtml(character)
    }

    function tokenColor(token, color) {
        return "<span style=\"color:" + color + "\">" + root.escapeHtml(token) + "</span>"
    }

    function highlightJson(value) {
        const source = String(value || "")
        let output = ""
        let index = 0
        while (index < source.length) {
            const character = source.charAt(index)
            if (character === "\"") {
                let end = index + 1
                let escaped = false
                while (end < source.length) {
                    const next = source.charAt(end)
                    if (!escaped && next === "\"") {
                        end += 1
                        break
                    }
                    escaped = !escaped && next === "\\"
                    if (next !== "\\") {
                        escaped = false
                    }
                    end += 1
                }
                const token = source.slice(index, end)
                let after = end
                while (after < source.length && /\s/.test(source.charAt(after))) {
                    after += 1
                }
                output += root.tokenColor(token, after < source.length
                    && source.charAt(after) === ":" ? "#79c0ff" : "#a5d6ff")
                index = end
                continue
            }
            if (/[0-9-]/.test(character)) {
                const numberMatch = source.slice(index).match(/^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/)
                if (numberMatch) {
                    output += root.tokenColor(numberMatch[0], "#d2a8ff")
                    index += numberMatch[0].length
                    continue
                }
            }
            const literalMatch = source.slice(index).match(/^(true|false|null)\b/)
            if (literalMatch) {
                output += root.tokenColor(literalMatch[0], "#ffab70")
                index += literalMatch[0].length
                continue
            }
            if (/[{}\[\],:]/.test(character)) {
                output += root.tokenColor(character, "#8b949e")
            } else if (/\s/.test(character)) {
                output += root.whitespaceHtml(character)
            } else {
                output += root.escapeHtml(character)
            }
            index += 1
        }
        return output.length ? output : "&nbsp;"
    }
}
