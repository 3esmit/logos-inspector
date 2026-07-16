pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../state"
import "../theme"
import "common"
import "../utils/UiFormat.js" as UiFormat

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model
    property string topic: ""
    property string title: qsTr("Comments")
    property string expectedAccountId: ""
    property var entityRef: null
    readonly property var commentView: model.social.commentsView(topic)
    readonly property var identityView: model.social.identitiesView()
    property string composerIdentityKey: model.social.selectedSocialIdentityKey

    visible: root.topic.length > 0
    spacing: root.theme.gap
    Layout.fillWidth: true

    onTopicChanged: Qt.callLater(root.reload)
    Component.onCompleted: Qt.callLater(root.reload)

    Connections {
        target: root.model.social

        function onSocialIdentityRevisionChanged() {
            root.composerIdentityKey = root.model.social.selectedSocialIdentityKey
        }
    }

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        Text {
            text: root.title
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: 14
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        Text {
            text: root.statusText()
            color: root.statusColor()
            textFormat: Text.PlainText
            elide: Text.ElideRight
            font.pixelSize: root.theme.secondaryText
            Layout.preferredWidth: 210
        }
    }

    StatusMessage {
        visible: root.panelState().error.length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Comments")
        message: root.panelState().error
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: !root.storeAvailable() && !root.panelState().loading && !root.panelState().error.length
        theme: root.theme
        tone: "warning"
        title: qsTr("Comments unavailable")
        message: root.storeUnavailableText()
        Layout.fillWidth: true
    }

    StatusMessage {
        objectName: "commentSendError"
        visible: String(root.panelState().sendError || "").length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Comment not posted")
        message: String(root.panelState().sendError || "")
        Layout.fillWidth: true
    }

    ColumnLayout {
        visible: root.comments().length > 0
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        Repeater {
            model: root.comments()

            Frame {
                id: commentFrame

                required property var modelData

                objectName: "socialCommentCard"
                padding: root.theme.gap
                Layout.fillWidth: true

                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("%1. %2")
                    .arg(String(commentFrame.modelData.displayName
                        || qsTr("Pseudonym")))
                    .arg(String(commentFrame.modelData.body || ""))
                Accessible.description: root.shortTime(
                    commentFrame.modelData.createdAt)

                background: Rectangle {
                    color: root.theme.surface
                    radius: root.theme.radius
                    border.width: 1
                    border.color: root.theme.outlineMuted
                }

                contentItem: ColumnLayout {
                    spacing: root.theme.gapSmall

                    RowLayout {
                        spacing: root.theme.gapSmall
                        Layout.fillWidth: true

                        Text {
                            text: String(commentFrame.modelData.displayName || qsTr("Pseudonym"))
                            color: root.theme.text
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.pixelSize: root.theme.secondaryText
                            font.weight: Font.DemiBold
                            Layout.fillWidth: true
                        }

                        Text {
                            text: root.shortTime(commentFrame.modelData.createdAt)
                            color: root.theme.textDim
                            textFormat: Text.PlainText
                            font.pixelSize: root.theme.labelText
                            Layout.alignment: Qt.AlignVCenter
                        }
                    }

                    Text {
                        text: String(commentFrame.modelData.body || "")
                        color: root.theme.textMuted
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: root.theme.primaryText
                        Layout.fillWidth: true
                    }
                }
            }
        }
    }

    StatusMessage {
        visible: root.comments().length === 0 && root.storeAvailable() && !root.panelState().loading && !root.panelState().error.length
        theme: root.theme
        tone: "info"
        title: qsTr("No comments")
        message: qsTr("No public comments found for this topic.")
        Layout.fillWidth: true
    }

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        ActionButton {
            theme: root.theme
            text: qsTr("Refresh")
            enabled: !root.panelState().loading && root.storeAvailable()
            Layout.preferredWidth: 104
            onClicked: root.reload()
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Next page")
            enabled: !root.panelState().loading && !root.panelState().exhausted && root.storeAvailable()
            Layout.preferredWidth: 122
            onClicked: root.model.social.loadComments(root.topic, false, root.model.social.socialCommentPageSize, root.expectedAccountId)
        }

        Text {
            text: root.topic
            color: root.theme.textDim
            textFormat: Text.PlainText
            elide: Text.ElideMiddle
            font.family: "monospace"
            font.pixelSize: root.theme.labelText
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
        }
    }

    GridLayout {
        columns: root.width < 720 ? 1 : 2
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gapSmall
        Layout.fillWidth: true

        ColumnLayout {
            spacing: 6
            Layout.fillWidth: true

            Text {
                text: qsTr("Identity")
                color: root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
                font.weight: Font.Medium
                Layout.fillWidth: true
            }

            ComboBox {
                id: identityCombo

                objectName: "commentIdentity"
                model: root.identityLabels()
                currentIndex: root.identityIndex()
                hoverEnabled: true
                Layout.fillWidth: true
                Layout.preferredHeight: root.theme.controlHeight
                onActivated: index => root.selectIdentity(index)

                Accessible.name: qsTr("Comment identity")
                Accessible.description: identityCombo.displayText

                contentItem: TextField {
                    text: identityCombo.displayText
                    color: root.theme.text
                    verticalAlignment: Text.AlignVCenter
                    leftPadding: 12
                    rightPadding: 24
                    readOnly: true
                    background: null
                    font.pixelSize: root.theme.primaryText
                }

                background: Rectangle {
                    radius: root.theme.radius
                    color: identityCombo.hovered || identityCombo.activeFocus ? root.theme.surfaceRaised : root.theme.field
                    border.width: identityCombo.activeFocus ? 2 : 1
                    border.color: identityCombo.activeFocus ? root.theme.accent : root.theme.outlineMuted
                }
            }
        }

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignBottom

            ActionButton {
                theme: root.theme
                text: qsTr("New")
                Layout.preferredWidth: 92
                onClicked: {
                    const identity = root.model.social.createIdentity("")
                    root.composerIdentityKey = identity.key
                }
            }

            ActionButton {
                theme: root.theme
                text: root.identityView.defaultMode === "manual" ? qsTr("Manual") : qsTr("Per topic")
                selected: root.identityView.defaultMode !== "manual"
                Layout.preferredWidth: 116
                onClicked: root.model.social.setIdentityDefaultMode(root.identityView.defaultMode === "manual" ? "perConversation" : "manual")
            }

            Item {
                Layout.fillWidth: true
            }
        }
    }

    TextAreaField {
        id: commentBody
        objectName: "commentBody"

        theme: root.theme
        label: qsTr("Comment")
        rows: 3
        placeholderText: qsTr("Write a public comment")
        Layout.fillWidth: true
    }

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        Text {
            objectName: "commentSendHint"
            text: root.sendHint()
            color: String(root.panelState().sendError || "").length > 0 || !root.writeAvailable() ? root.theme.warning : root.theme.textDim
            textFormat: Text.PlainText
            elide: Text.ElideRight
            font.pixelSize: root.theme.secondaryText
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
        }

        ActionButton {
            objectName: "commentPostButton"
            theme: root.theme
            text: qsTr("Post")
            primary: true
            enabled: root.writeAvailable() && commentBody.text.trim().length > 0
            Layout.preferredWidth: 104
            onClicked: postConfirm.open()
        }
    }

    ConfirmActionPopup {
        id: postConfirm

        theme: root.theme
        title: qsTr("Post comment")
        message: qsTr("This sends a public Delivery message on %1.").arg(root.topic)
        confirmText: qsTr("Post")
        confirmEnabled: root.writeAvailable() && commentBody.text.trim().length > 0
        onAccepted: {
            const draft = commentBody.text
            root.model.social.postComment(root.topic, draft, root.composerIdentityKey, root.entityRef, function (response) {
                if (response && response.ok === true && commentBody.text === draft) {
                    commentBody.text = ""
                }
            })
        }
    }

    function reload() {
        if (!root.topic.length || !root.storeAvailable()) {
            return
        }
        root.model.social.loadComments(root.topic, true, root.model.social.socialCommentPageSize, root.expectedAccountId)
    }

    function storeGate() {
        return root.commentView.readGate
    }

    function storeAvailable() {
        return root.storeGate().enabled === true
    }

    function writeGate() {
        return root.commentView.writeGate
    }

    function writeAvailable() {
        return root.commentView.writeAvailable === true
    }

    function storeUnavailableText() {
        return root.commentView.readError
    }

    function writeUnavailableText() {
        return root.commentView.writeError
    }

    function panelState() {
        return root.commentView.state
    }

    function comments() {
        return root.commentView.rows
    }

    function identityRows() {
        return root.identityView.rows
    }

    function identityLabels() {
        const rows = root.identityRows()
        if (!rows.length) {
            return [qsTr("New pseudonym")]
        }
        return rows.map(function (row) {
            return String(row.displayName || row.localId || qsTr("Pseudonym"))
        })
    }

    function identityIndex() {
        const rows = root.identityRows()
        for (let i = 0; i < rows.length; ++i) {
            if (String(rows[i].key || "") === root.composerIdentityKey) {
                return i
            }
        }
        return rows.length > 0 ? 0 : 0
    }

    function selectIdentity(index) {
        const rows = root.identityRows()
        if (index >= 0 && index < rows.length) {
            root.composerIdentityKey = rows[index].key
            root.model.social.selectIdentity(root.composerIdentityKey)
        }
    }

    function statusText() {
        if (root.panelState().loading) {
            return qsTr("Loading")
        }
        if (!root.storeAvailable()) {
            return root.storeUnavailableText()
        }
        return qsTr("%1 comments").arg(root.comments().length)
    }

    function statusColor() {
        if (root.panelState().error.length > 0 || !root.storeAvailable()) {
            return root.theme.warning
        }
        return root.theme.textDim
    }

    function sendHint() {
        if (root.panelState().sending === true) {
            return qsTr("Posting comment")
        }
        if (String(root.panelState().sendError || "").length > 0) {
            return String(root.panelState().sendError)
        }
        if (!root.writeAvailable()) {
            return root.writeUnavailableText()
        }
        return qsTr("Public JSON message")
    }

    function shortTime(value) {
        const text = String(value || "")
        if (!text.length) {
            return ""
        }
        const parsed = Date.parse(text)
        if (!Number.isFinite(parsed)) {
            return UiFormat.shortHash(text)
        }
        return Qt.formatDateTime(new Date(parsed), "yyyy-MM-dd HH:mm")
    }
}
