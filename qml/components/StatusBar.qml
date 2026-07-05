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

            ActionButton {
                theme: root.theme
                text: ""
                iconOnly: true
                iconName: "back"
                enabled: root.model.canNavigateBack()
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                accessibleName: qsTr("Back")
                ToolTip.visible: hovered && root.model.canNavigateBack()
                ToolTip.delay: 350
                ToolTip.text: root.model.navigationBackLabel().length
                    ? qsTr("Back to %1").arg(root.model.navigationBackLabel())
                    : qsTr("Back")
                onClicked: root.model.navigateBack()
            }

            ActionButton {
                theme: root.theme
                text: ""
                iconOnly: true
                iconName: "forward"
                enabled: root.model.canNavigateForward()
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                accessibleName: qsTr("Forward")
                ToolTip.visible: hovered && root.model.canNavigateForward()
                ToolTip.delay: 350
                ToolTip.text: root.model.navigationForwardLabel().length
                    ? qsTr("Forward to %1").arg(root.model.navigationForwardLabel())
                    : qsTr("Forward")
                onClicked: root.model.navigateForward()
            }

            Rectangle {
                color: root.model.busy ? root.theme.warning : root.resultColor()
                radius: 4
                Layout.preferredWidth: 8
                Layout.preferredHeight: 8
                Layout.alignment: Qt.AlignVCenter
                Accessible.ignored: true
            }

            Text {
                text: root.model.viewTitle()
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
                font.weight: Font.DemiBold
                elide: Text.ElideRight
                Layout.fillWidth: true
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
                placeholderText: qsTr("Open block, transaction, or account")
                placeholderTextColor: root.theme.textDim
                selectionColor: root.theme.accent
                selectedTextColor: root.theme.selectedText
                font.pixelSize: root.theme.secondaryText
                leftPadding: 12
                rightPadding: 60
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

                Rectangle {
                    anchors.right: parent.right
                    anchors.rightMargin: 8
                    anchors.verticalCenter: parent.verticalCenter
                    width: 42
                    height: 24
                    radius: root.theme.radius
                    color: root.lookupBadgeFill(root.lookupCode(lookupField.text))
                    border.width: 1
                    border.color: root.lookupBadgeStroke(root.lookupCode(lookupField.text))

                    Text {
                        anchors.centerIn: parent
                        text: root.lookupLabel(root.lookupCode(lookupField.text))
                        color: root.lookupBadgeText(root.lookupCode(lookupField.text))
                        textFormat: Text.PlainText
                        font.pixelSize: root.theme.labelText
                        font.family: "monospace"
                        font.weight: Font.DemiBold
                    }
                }
            }

            ActionButton {
                theme: root.theme
                text: ""
                iconOnly: true
                iconName: "search"
                primary: true
                enabled: !root.model.busy && root.lookupCanOpen(lookupField.text)
                Layout.preferredWidth: root.theme.controlHeight
                accessibleName: qsTr("Search")
                ToolTip.visible: hovered
                ToolTip.delay: 350
                ToolTip.text: qsTr("Search")
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

            Text {
                visible: root.model.busy
                text: qsTr("Working")
                color: root.theme.warning
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
                font.weight: Font.DemiBold
                Layout.alignment: Qt.AlignVCenter
            }

            ActionButton {
                visible: root.currentPageHasError()
                theme: root.theme
                text: root.resultText()
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
        if (!value.length || root.model.busy || !root.lookupCanOpen(value)) {
            return
        }
        lookupField.clear()
        root.model.routeSearch(value)
        Qt.callLater(function () {
            lookupField.clear()
        })
    }

    function lookupCode(query) {
        const value = String(query || "").trim()
        if (!value.length) {
            return "any"
        }
        if (/^[0-9]+$/.test(value)) {
            return "block"
        }
        if (/^(0x)?[0-9a-fA-F]{64}$/.test(value)) {
            return "any"
        }
        if (/^(0x)?[0-9a-fA-F]{40}$/.test(value) || /^[1-9A-HJ-NP-Za-km-z]{32,64}$/.test(value)) {
            return "account"
        }
        return "invalid"
    }

    function lookupLabel(code) {
        if (code === "block") {
            return qsTr("BLK")
        }
        if (code === "account") {
            return qsTr("ACC")
        }
        if (code === "invalid") {
            return qsTr("N/A")
        }
        return qsTr("ANY")
    }

    function lookupCanOpen(query) {
        const value = String(query || "").trim()
        const code = root.lookupCode(query)
        return code !== "invalid" && (code !== "any" || value.length > 0)
    }

    function lookupBadgeFill(code) {
        if (code === "invalid") {
            return root.theme.errorMuted
        }
        if (code === "any") {
            return root.theme.field
        }
        return root.theme.infoMuted
    }

    function lookupBadgeStroke(code) {
        if (code === "invalid") {
            return root.theme.error
        }
        if (code === "any") {
            return root.theme.outlineMuted
        }
        return root.theme.info
    }

    function lookupBadgeText(code) {
        if (code === "invalid") {
            return root.theme.error
        }
        if (code === "any") {
            return root.theme.textDim
        }
        return root.theme.info
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
