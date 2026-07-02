import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../state"
import "../theme"

Pane {
    id: root

    required property Theme theme
    required property AppModel model
    property bool compact: false

    padding: 12

    background: Rectangle {
        color: root.theme.background
    }

    contentItem: GridLayout {
        columns: root.width < 760 ? 1 : 3
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gapSmall

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter

            Rectangle {
                color: root.model.busy ? root.theme.warning : root.resultColor()
                radius: 4
                Layout.preferredWidth: 8
                Layout.preferredHeight: 8
                Layout.alignment: Qt.AlignVCenter
                Accessible.ignored: true
            }

            ColumnLayout {
                spacing: 1
                Layout.fillWidth: true

                Text {
                    text: root.model.viewTitle()
                    color: root.theme.text
                    textFormat: Text.PlainText
                    font.pixelSize: root.theme.secondaryText
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }

                Text {
                    text: root.statusLine()
                    color: root.theme.textMuted
                    textFormat: Text.PlainText
                    font.pixelSize: root.theme.dataText
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }
        }

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: root.width < 760
            Layout.preferredWidth: root.width < 760 ? 0 : 430
            Layout.minimumWidth: root.width < 760 ? 0 : 320
            Layout.alignment: Qt.AlignVCenter

            TextField {
                id: lookupField

                color: root.theme.text
                placeholderText: qsTr("Open hash, block, account, or page")
                placeholderTextColor: root.theme.textDim
                selectionColor: root.theme.accent
                selectedTextColor: root.theme.selectedText
                font.pixelSize: root.theme.secondaryText
                leftPadding: 12
                rightPadding: 12
                hoverEnabled: true
                enabled: !root.model.busy
                Layout.fillWidth: true
                Layout.preferredHeight: root.theme.controlHeight
                onAccepted: root.openLookup()

                background: Rectangle {
                    radius: root.theme.radius
                    color: lookupField.hovered || lookupField.activeFocus ? root.theme.surfaceRaised : root.theme.field
                    border.width: lookupField.activeFocus ? 2 : 1
                    border.color: lookupField.activeFocus ? root.theme.accent : root.theme.outlineMuted
                }

                Accessible.role: Accessible.EditableText
                Accessible.name: qsTr("Global reference lookup")
            }

            Rectangle {
                visible: root.width >= 1040
                color: root.theme.field
                radius: root.theme.radius
                border.width: 1
                border.color: root.theme.outlineMuted
                Layout.preferredWidth: 132
                Layout.preferredHeight: root.theme.controlHeight
                Layout.alignment: Qt.AlignVCenter

                Text {
                    anchors.fill: parent
                    anchors.leftMargin: 10
                    anchors.rightMargin: 10
                    text: root.lookupKind(lookupField.text)
                    color: lookupField.text.trim().length > 0 ? root.theme.info : root.theme.textDim
                    textFormat: Text.PlainText
                    font.pixelSize: root.theme.dataText
                    font.capitalization: Font.AllUppercase
                    verticalAlignment: Text.AlignVCenter
                    horizontalAlignment: Text.AlignHCenter
                    elide: Text.ElideRight
                }
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Open")
                primary: true
                enabled: !root.model.busy && lookupField.text.trim().length > 0
                Layout.preferredWidth: 82
                accessibleName: qsTr("Open reference")
                onClicked: root.openLookup()
            }
        }

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.alignment: root.width < 760 ? Qt.AlignLeft : Qt.AlignRight
            Layout.fillWidth: root.width < 760

            BusyIndicator {
                running: root.model.busy
                visible: root.model.busy
                Layout.preferredWidth: 30
                Layout.preferredHeight: 30
            }

            ActionButton {
                theme: root.theme
                text: root.model.busy ? qsTr("Working") : root.resultText()
                enabled: false
                Layout.preferredWidth: 104
            }
        }
    }

    function focusLookup() {
        lookupField.forceActiveFocus()
        lookupField.selectAll()
    }

    function openLookup() {
        const value = lookupField.text.trim()
        if (!value.length || root.model.busy) {
            return
        }
        root.model.routeSearch(value)
        lookupField.text = ""
    }

    function lookupKind(query) {
        const value = String(query || "").trim()
        if (!value.length) {
            return qsTr("All refs")
        }
        if (root.model.viewKeyForQuery(value).length > 0) {
            return qsTr("Page")
        }
        if (/^[0-9]+$/.test(value)) {
            return qsTr("Block")
        }
        if (/^(0x)?[0-9a-fA-F]{64}$/.test(value)) {
            return qsTr("Hash")
        }
        if (/^(0x)?[0-9a-fA-F]{40}$/.test(value)) {
            return qsTr("Address")
        }
        return qsTr("Account")
    }

    function statusLine() {
        if (root.model.busy) {
            return root.model.statusText
        }
        return qsTr("%1 profile / %2").arg(root.model.networkProfile).arg(root.model.statusText)
    }

    function resultText() {
        if (root.currentPageHasError()) {
            return qsTr("Error")
        }
        return qsTr("Ready")
    }

    function resultColor() {
        if (root.currentPageHasError()) {
            return root.theme.warning
        }
        return root.theme.success
    }

    function currentPageHasError() {
        return root.model.pageHasOutput(root.model.currentView) && root.model.resultIsError
    }
}
