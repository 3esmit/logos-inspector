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
                objectName: "navigationBackButton"
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
                objectName: "navigationForwardButton"
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
                color: root.model.shell.busy ? root.theme.warning : root.resultColor()
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

                objectName: "globalReferenceLookup"
                color: root.theme.text
                placeholderText: qsTr("Open block, transaction, or account")
                placeholderTextColor: root.theme.textDim
                selectionColor: root.theme.accent
                selectedTextColor: root.theme.selectedText
                font.pixelSize: root.theme.secondaryText
                leftPadding: 12
                rightPadding: 60
                hoverEnabled: true
                enabled: !root.model.shell.busy
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
                objectName: "globalReferenceSearch"
                theme: root.theme
                text: ""
                iconOnly: true
                iconName: "search"
                primary: true
                enabled: !root.model.shell.busy && root.lookupCanOpen(lookupField.text)
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
                running: root.model.shell.busy
                visible: root.model.shell.busy
                Layout.preferredWidth: 30
                Layout.preferredHeight: 30
            }

            Text {
                visible: root.model.shell.busy
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
        if (!value.length || root.model.shell.busy || !root.lookupCanOpen(value)) {
            return
        }
        lookupField.clear()
        root.model.entityNavigation.routeSearch(value)
        Qt.callLater(function () {
            lookupField.clear()
        })
    }

    function lookupCode(query) {
        const value = String(query || "").trim()
        if (!value.length) {
            return "any"
        }
        const prefixed = value.match(/^([A-Za-z][A-Za-z0-9_-]*)(?:\s*:\s*|\s+)(.*)$/)
        if (prefixed) {
            return root.prefixedLookupCode(
                String(prefixed[1]).toLowerCase(),
                String(prefixed[2] || "").trim())
        }
        if (/^[0-9]+$/.test(value)) {
            return root.lookupNumericBlockTarget(value) ? "block"
                : (root.lookupHashTarget(value) ? "any" : "invalid")
        }
        if (root.lookupHashTarget(value)) {
            return "any"
        }
        if (root.lookupUnprefixedAccountTarget(value)) {
            return "account"
        }
        return "invalid"
    }

    function prefixedLookupCode(prefix, target) {
        if (prefix === "mantle") {
            return "transaction"
        }
        if (["private", "wallet", "cid", "storage", "l1-wallet",
                "note", "module"].indexOf(prefix) >= 0) {
            return "any"
        }
        if (!target.length) {
            return "invalid"
        }
        if (["l1", "slot", "l2", "lez", "block"].indexOf(prefix) >= 0) {
            return root.lookupBlockTarget(target) ? "block" : "invalid"
        }
        if (prefix === "tx" || prefix === "transaction") {
            return root.lookupHashTarget(target) ? "transaction" : "invalid"
        }
        if (prefix === "account") {
            return root.lookupAccountTarget(target) ? "account" : "invalid"
        }
        if (prefix === "program") {
            return root.lookupHashTarget(target) ? "program" : "invalid"
        }
        if (prefix === "zone" || prefix === "channel") {
            return root.lookupHashTarget(target) ? "zone" : "invalid"
        }
        return "invalid"
    }

    function lookupHashTarget(value) {
        return /^(0[xX])?[0-9a-fA-F]{64}$/.test(String(value || "").trim())
    }

    function lookupBlockTarget(value) {
        const target = String(value || "").trim()
        return root.lookupNumericBlockTarget(target) || root.lookupHashTarget(target)
    }

    function lookupNumericBlockTarget(value) {
        const target = String(value || "").trim()
        if (!/^[0-9]+$/.test(target)) {
            return false
        }
        const normalized = target.replace(/^0+/, "") || "0"
        const maximum = "18446744073709551615"
        return normalized.length < maximum.length
            || (normalized.length === maximum.length && normalized <= maximum)
    }

    function lookupAccountTarget(value) {
        let target = String(value || "").trim()
        if (target.indexOf("Public/") === 0 || target.indexOf("public/") === 0) {
            target = target.slice(7)
        } else if (target.indexOf("Private/") === 0
                || target.indexOf("private/") === 0) {
            return false
        }
        if (/^(0[xX])?[0-9a-fA-F]{64}$/.test(target)) {
            return true
        }
        if (/^(0[xX])?[0-9a-fA-F]{40}$/.test(target)) {
            return false
        }
        return /^[1-9A-HJ-NP-Za-km-z]{32,64}$/.test(target)
    }

    function lookupUnprefixedAccountTarget(value) {
        const target = String(value || "").trim()
        if (target.indexOf("Public/") === 0) {
            return root.lookupAccountTarget(target)
        }
        if (target.indexOf("public/") === 0 || target.indexOf("Private/") === 0
                || target.indexOf("private/") === 0) {
            return false
        }
        return false
    }

    function lookupLabel(code) {
        if (code === "block") {
            return qsTr("BLK")
        }
        if (code === "account") {
            return qsTr("ACC")
        }
        if (code === "transaction") {
            return qsTr("TX")
        }
        if (code === "program") {
            return qsTr("PRG")
        }
        if (code === "zone") {
            return qsTr("ZONE")
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
        return root.model.pageHasOutput(root.model.shell.currentView) && root.model.shell.resultIsError
    }
}
