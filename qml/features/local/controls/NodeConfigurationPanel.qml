pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../components"
import "../../../state"

Panel {
    id: root

    required property LocalNodesState model
    property string activeNode: ""
    property string currentTab: "common"
    property string baselineText: ""
    property string baselineRevision: ""
    property string draftText: ""
    property string tabGuardMessage: ""
    property string localSyntaxError: ""
    property string appliedSnapshotRevision: ""
    property bool applyingSnapshot: false

    readonly property var snapshot: root.model.nodeConfigSnapshot || null
    readonly property bool loading: root.model.nodeConfigLoading
    readonly property bool saving: root.model.nodeConfigSaving
    readonly property bool dirty: root.draftText !== root.baselineText
    readonly property bool editable: root.snapshot && root.snapshot.editable === true
    readonly property string blockedReason: String(root.snapshot && root.snapshot.blocked_reason || "")
    readonly property string validationError: root.localSyntaxError.length > 0
        ? root.localSyntaxError
        : String(root.model.nodeConfigValidation && root.model.nodeConfigValidation.error || "")
    readonly property bool validationMatchesDraft: String(root.model.nodeConfigValidationText || "")
        === root.draftText
    readonly property bool validDraft: root.editable
        && root.localSyntaxError.length === 0
        && !root.model.nodeConfigValidationLoading
        && root.validationMatchesDraft
        && root.model.nodeConfigValidation
        && root.model.nodeConfigValidation.valid === true

    title: root.activeNode.length
        ? qsTr("Configure %1").arg(root.nodeLabel())
        : qsTr("Node Configuration")
    visible: root.activeNode.length > 0
    objectName: "nodeConfigurationPanel"

    ListModel {
        id: editorTabs

        ListElement {
            value: "common"
            label: "Common settings"
        }

        ListElement {
            value: "raw"
            label: "Raw configuration"
        }
    }

    Timer {
        id: validationTimer

        interval: 180
        repeat: false
        onTriggered: {
            if (root.editable && root.localSyntaxError.length === 0) {
                root.model.validateNodeConfig(root.draftText)
            }
        }
    }

    Connections {
        target: root.model

        function onNetworkProfileChanged() {
            root.resetSelection()
        }
    }

    ColumnLayout {
        spacing: root.theme.gap
        Layout.fillWidth: true

        StatusMessage {
            visible: root.loading
            theme: root.theme
            tone: "info"
            title: qsTr("Loading configuration")
            message: qsTr("Reading the managed node configuration.")
            Layout.fillWidth: true
        }

        StatusMessage {
            visible: !root.loading && root.model.nodeConfigError.length > 0
            theme: root.theme
            tone: "error"
            title: qsTr("Configuration unavailable")
            message: root.model.nodeConfigError
            Layout.fillWidth: true
        }

        ColumnLayout {
            visible: !root.loading && root.snapshot !== null
            spacing: root.theme.gap
            Layout.fillWidth: true

            Text {
                text: qsTr("%1 · %2 · %3")
                    .arg(String(root.snapshot && root.snapshot.config_role || qsTr("Configuration")))
                    .arg(String(root.snapshot && root.snapshot.format || "JSON").toUpperCase())
                    .arg(String(root.snapshot && root.snapshot.config_path || ""))
                color: root.theme.textDim
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
                font.family: "monospace"
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }

            StatusMessage {
                visible: root.blockedReason.length > 0
                theme: root.theme
                tone: "warning"
                title: qsTr("Configuration is read-only")
                message: root.blockedReason
                Layout.fillWidth: true
            }

            StatusMessage {
                visible: root.protectedFieldsText().length > 0
                theme: root.theme
                tone: "info"
                title: qsTr("Protected values")
                message: root.protectedFieldsText()
                Layout.fillWidth: true
            }

            Text {
                text: qsTr("Validation: %1.")
                    .arg(String(root.snapshot && root.snapshot.validation_scope || ""))
                color: root.theme.textMuted
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }

            TabSwitch {
                objectName: "nodeConfigurationTabs"
                theme: root.theme
                options: editorTabs
                current: root.currentTab
                onSelected: function (value) {
                    root.requestTab(value)
                }
            }

            StatusMessage {
                visible: root.tabGuardMessage.length > 0
                theme: root.theme
                tone: "warning"
                title: qsTr("Save or undo required")
                message: root.tabGuardMessage
                Layout.fillWidth: true
            }

            GridLayout {
                visible: root.currentTab === "common"
                columns: width < 760 ? 1 : 2
                columnSpacing: root.theme.gap
                rowSpacing: root.theme.gap
                Layout.fillWidth: true

                Repeater {
                    model: root.commonFields()

                    delegate: ColumnLayout {
                        id: fieldEditor

                        required property int index
                        required property var modelData
                        readonly property var field: fieldEditor.modelData

                        spacing: 6
                        Layout.fillWidth: true

                        Text {
                            text: qsTr("%1 · %2")
                                .arg(String(fieldEditor.field.section || ""))
                                .arg(String(fieldEditor.field.label || ""))
                            color: root.editable ? root.theme.textMuted : root.theme.textDim
                            textFormat: Text.PlainText
                            font.pixelSize: root.theme.secondaryText
                            font.weight: Font.Medium
                            Layout.fillWidth: true
                        }

                        CheckBox {
                            visible: String(fieldEditor.field.kind || "") === "boolean"
                            checked: root.booleanValue(fieldEditor.field)
                            enabled: root.editable && !root.saving
                            text: root.booleanValue(fieldEditor.field)
                                ? qsTr("Enabled") : qsTr("Disabled")
                            onToggled: root.updateCommonValue(fieldEditor.field, checked)
                            Layout.fillWidth: true
                            Accessible.name: String(fieldEditor.field.label || "")
                        }

                        TextArea {
                            visible: String(fieldEditor.field.kind || "") === "string_list"
                            objectName: "nodeConfigListField" + fieldEditor.index
                            text: root.listText(fieldEditor.field)
                            readOnly: !root.editable || root.saving
                            wrapMode: TextArea.Wrap
                            color: enabled ? root.theme.text : root.theme.textDim
                            selectionColor: root.theme.accent
                            selectedTextColor: root.theme.selectedText
                            font.family: "monospace"
                            font.pixelSize: root.theme.dataText
                            leftPadding: 12
                            rightPadding: 12
                            topPadding: 8
                            bottomPadding: 8
                            Layout.fillWidth: true
                            Layout.preferredHeight: 96
                            onTextEdited: root.updateCommonValue(fieldEditor.field, root.stringList(text))

                            background: Rectangle {
                                radius: root.theme.radius
                                color: parent.hovered || parent.activeFocus
                                    ? root.theme.surfaceRaised : root.theme.field
                                border.width: parent.activeFocus ? 2 : 1
                                border.color: parent.activeFocus
                                    ? root.theme.accent : root.theme.outlineMuted
                            }

                            Accessible.name: String(fieldEditor.field.label || "")
                        }

                        TextField {
                            id: fieldInput

                            visible: String(fieldEditor.field.kind || "") !== "boolean"
                                && String(fieldEditor.field.kind || "") !== "string_list"
                            objectName: "nodeConfigCommonField" + fieldEditor.index
                            text: root.fieldText(fieldEditor.field)
                            readOnly: !root.editable || root.saving
                            color: enabled ? root.theme.text : root.theme.textDim
                            selectionColor: root.theme.accent
                            selectedTextColor: root.theme.selectedText
                            font.family: String(fieldEditor.field.kind || "") === "path"
                                ? "monospace" : ""
                            font.pixelSize: root.theme.primaryText
                            leftPadding: 12
                            rightPadding: 12
                            hoverEnabled: true
                            Layout.fillWidth: true
                            Layout.preferredHeight: root.theme.controlHeight
                            onTextEdited: root.updateCommonText(fieldEditor.field, text)

                            background: Rectangle {
                                radius: root.theme.radius
                                color: fieldInput.hovered || fieldInput.activeFocus
                                    ? root.theme.surfaceRaised : root.theme.field
                                border.width: fieldInput.activeFocus ? 2 : 1
                                border.color: fieldInput.activeFocus
                                    ? root.theme.accent : root.theme.outlineMuted
                            }

                            Accessible.name: String(fieldEditor.field.label || "")
                        }
                    }
                }
            }

            NodeConfigRawEditor {
                visible: root.currentTab === "raw"
                theme: root.theme
                text: root.draftText
                errorMessage: root.validationError
                editable: root.editable && !root.saving
                Layout.fillWidth: true
                onTextEdited: function (text) {
                    root.setDraftText(text)
                }
            }

            RowLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                Text {
                    text: root.dirty ? qsTr("Unsaved changes") : qsTr("No unsaved changes")
                    color: root.dirty ? root.theme.warning : root.theme.textDim
                    textFormat: Text.PlainText
                    font.pixelSize: root.theme.dataText
                    Layout.fillWidth: true
                }

                ActionButton {
                    objectName: "nodeConfigUndoButton"
                    theme: root.theme
                    text: qsTr("Undo")
                    enabled: root.dirty && !root.saving
                    onClicked: root.undoDraft()
                }

                ActionButton {
                    objectName: "nodeConfigSaveButton"
                    theme: root.theme
                    text: root.saving ? qsTr("Saving…") : qsTr("Save")
                    primary: true
                    enabled: root.dirty && root.validDraft && !root.saving
                    onClicked: root.saveDraft()
                }
            }
        }
    }

    onSnapshotChanged: root.applySnapshot()
    onActiveNodeChanged: {
        if (!root.activeNode.length) {
            return
        }
        if (!root.dirty) {
            root.model.loadNodeConfig(root.activeNode)
        }
    }

    function selectNode(node) {
        const nodeKey = String(node || "").trim()
        if (!nodeKey.length) {
            return false
        }
        if (root.dirty) {
            root.tabGuardMessage = qsTr("Save or undo the current draft before changing node configuration.")
            return false
        }
        if (nodeKey === root.activeNode) {
            root.model.loadNodeConfig(nodeKey)
            return true
        }
        root.activeNode = nodeKey
        return true
    }

    function resetSelection() {
        validationTimer.stop()
        root.activeNode = ""
        root.currentTab = "common"
        root.baselineText = ""
        root.baselineRevision = ""
        root.draftText = ""
        root.tabGuardMessage = ""
        root.localSyntaxError = ""
        root.appliedSnapshotRevision = ""
        root.applyingSnapshot = false
    }

    function applySnapshot() {
        const value = root.snapshot
        if (!value || String(value.node || "") !== root.activeNode) {
            return
        }
        const revision = String(value.revision || "")
        const rawText = String(value.raw_text || "")
        if (!revision.length) {
            return
        }
        if (revision === root.appliedSnapshotRevision && rawText === root.baselineText) {
            if (value.editable === true
                    && (!root.model.nodeConfigValidation
                        || root.model.nodeConfigValidationText !== root.draftText)) {
                root.model.validateNodeConfig(root.draftText)
            }
            return
        }
        root.applyingSnapshot = true
        root.baselineText = rawText
        root.baselineRevision = revision
        root.draftText = root.baselineText
        root.currentTab = "common"
        root.tabGuardMessage = ""
        root.localSyntaxError = ""
        root.appliedSnapshotRevision = revision
        root.applyingSnapshot = false
        if (value.editable === true) {
            root.model.validateNodeConfig(root.draftText)
        }
    }

    function requestTab(value) {
        const next = String(value || "")
        if (!next.length || next === root.currentTab) {
            return
        }
        if (root.dirty) {
            root.tabGuardMessage = qsTr("Save or undo this draft before changing editor tabs.")
            return
        }
        root.currentTab = next
        root.tabGuardMessage = ""
    }

    function setDraftText(text) {
        root.draftText = String(text || "")
        root.localSyntaxError = root.jsonSyntaxError(root.draftText)
        root.tabGuardMessage = ""
        if (root.editable && root.localSyntaxError.length === 0) {
            validationTimer.restart()
        }
    }

    function undoDraft() {
        root.draftText = root.baselineText
        root.localSyntaxError = ""
        root.tabGuardMessage = ""
        if (root.editable) {
            root.model.validateNodeConfig(root.draftText)
        }
    }

    function saveDraft() {
        if (!root.validDraft) {
            return
        }
        root.model.saveNodeConfig(root.draftText, root.baselineRevision)
    }

    function commonFields() {
        const source = root.snapshot && Array.isArray(root.snapshot.common_fields)
            ? root.snapshot.common_fields : []
        const value = root.jsonValue()
        return source.map(function (field) {
            const next = Object.assign({}, field)
            next.value = root.valueAtPath(value, String(field.path || ""))
            return next
        })
    }

    function jsonValue() {
        try {
            return JSON.parse(root.draftText)
        } catch (error) {
            return null
        }
    }

    function jsonSyntaxError(text) {
        try {
            JSON.parse(String(text || ""))
            return ""
        } catch (error) {
            return String(error && error.message || error)
        }
    }

    function pathTokens(path) {
        const source = String(path || "")
        if (!source.length || source === "/") {
            return []
        }
        return source.slice(1).split("/").map(function (token) {
            return token.replace(/~1/g, "/").replace(/~0/g, "~")
        })
    }

    function valueAtPath(value, path) {
        let current = value
        const tokens = root.pathTokens(path)
        for (let index = 0; index < tokens.length; ++index) {
            if (!current || typeof current !== "object"
                    || !(tokens[index] in current)) {
                return null
            }
            current = current[tokens[index]]
        }
        return current
    }

    function setValueAtPath(value, path, replacement) {
        const tokens = root.pathTokens(path)
        if (!tokens.length || !value || typeof value !== "object") {
            return false
        }
        let current = value
        for (let index = 0; index + 1 < tokens.length; ++index) {
            const token = tokens[index]
            if (!current[token] || typeof current[token] !== "object") {
                current[token] = {}
            }
            current = current[token]
        }
        current[tokens[tokens.length - 1]] = replacement
        return true
    }

    function updateCommonValue(field, replacement) {
        const value = root.jsonValue()
        if (!root.setValueAtPath(value, String(field && field.path || ""), replacement)) {
            return
        }
        root.setDraftText(JSON.stringify(value, null, 2))
    }

    function updateCommonText(field, text) {
        const kind = String(field && field.kind || "")
        const input = String(text || "")
        if (kind === "port") {
            if (!/^[0-9]+$/.test(input)) {
                root.localSyntaxError = qsTr("%1 must be a whole port number.")
                    .arg(String(field && field.label || qsTr("Port")))
                return
            }
            root.updateCommonValue(field, Number(input))
            return
        }
        root.updateCommonValue(field, input)
    }

    function fieldText(field) {
        const value = root.valueAtPath(root.jsonValue(), String(field && field.path || ""))
        return value === null || value === undefined ? "" : String(value)
    }

    function booleanValue(field) {
        return root.valueAtPath(root.jsonValue(), String(field && field.path || "")) === true
    }

    function listText(field) {
        const value = root.valueAtPath(root.jsonValue(), String(field && field.path || ""))
        return Array.isArray(value) ? value.join("\n") : ""
    }

    function stringList(text) {
        return String(text || "").split(/\r?\n/).map(function (entry) {
            return entry.trim()
        }).filter(function (entry) {
            return entry.length > 0
        })
    }

    function nodeLabel() {
        return String(root.snapshot && root.snapshot.node_label || root.activeNode)
    }

    function protectedFieldsText() {
        const values = root.snapshot && Array.isArray(root.snapshot.protected_fields)
            ? root.snapshot.protected_fields : []
        if (!values.length) {
            return ""
        }
        return qsTr("Inspector keeps %1 out of this editor.").arg(values.join(", "))
    }
}
