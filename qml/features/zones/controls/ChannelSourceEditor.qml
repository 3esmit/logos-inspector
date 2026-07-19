pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../components"
import "../../../components/common"
import "../../../theme"
import "../../../state/source_routing/SourcePolicyCatalog.js" as SourcePolicyCatalog
import "../ZonePresentation.js" as Presentation

Rectangle {
    id: root

    required property Theme theme
    required property var zoneState
    property string role: "sequencer"
    property string mode: "add"
    property var source: null
    property double expectedRevision: 0
    property string targetKind: "rpc"
    property string initialLabel: ""
    property string initialTargetKind: "rpc"
    property string initialTargetValue: ""
    property bool conflict: false
    readonly property var adapterPolicy: root.selectedAdapterPolicy()
    readonly property string targetValue: root.targetKind === "module"
        ? root.moduleDefault() : endpointField.text.trim()
    readonly property bool dirty: labelField.text.trim() !== root.initialLabel
        || root.targetKind !== root.initialTargetKind
        || root.targetValue !== root.initialTargetValue
    readonly property bool validDraft: root.targetModeImplemented(root.targetKind)
        && (root.targetKind === "module"
            || (root.adapterAcceptsInput("rpc_endpoint") && root.targetValue.length > 0))
    signal saved()
    signal cancelled()
    signal reloadRequested()

    objectName: "channelSourceEditor"
    implicitHeight: editorLayout.implicitHeight + root.theme.gapLarge * 2
    radius: root.theme.radius
    color: root.theme.surface
    border.width: 1
    border.color: root.conflict ? root.theme.warning : root.theme.outline

    ListModel {
        id: targetOptions
    }

    ColumnLayout {
        id: editorLayout

        anchors.fill: parent
        anchors.margins: root.theme.gapLarge
        spacing: root.theme.gap

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Text {
                text: root.mode === "add"
                    ? (root.role === "sequencer" ? qsTr("Add Sequencer source") : qsTr("Configure Indexer"))
                    : (root.role === "sequencer" ? qsTr("Edit Sequencer source") : qsTr("Edit Indexer"))
                color: root.theme.text
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font.pixelSize: root.theme.secondaryText
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            Text {
                text: qsTr("Revision %1").arg(Presentation.numberText(root.expectedRevision))
                color: root.theme.textDim
                textFormat: Text.PlainText
                font.pixelSize: root.theme.dataText
            }
        }

        TextField {
            id: labelField

            placeholderText: qsTr("Label (optional)")
            color: root.theme.text
            placeholderTextColor: root.theme.textMuted
            selectionColor: root.theme.accent
            selectedTextColor: root.theme.selectedText
            font.pixelSize: root.theme.secondaryText
            Layout.fillWidth: true
            Layout.preferredHeight: root.theme.controlHeight

            background: Rectangle {
                radius: root.theme.radius
                color: root.theme.field
                border.width: labelField.activeFocus ? 1 : 0
                border.color: root.theme.accent
            }
        }

        TabSwitch {
            theme: root.theme
            options: targetOptions
            current: root.targetKind
            onSelected: function (value) {
                root.targetKind = value
            }
        }

        Text {
            visible: !root.targetModeImplemented(root.targetKind)
            text: qsTr("This source mode is unavailable. Select a supported mode before saving.")
            color: root.theme.warning
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: root.theme.dataText
            Layout.fillWidth: true
        }

        TextField {
            id: endpointField

            objectName: "channelSourceEndpointField"
            visible: root.adapterAcceptsInput("rpc_endpoint")
            placeholderText: qsTr("https://host:port/")
            color: root.theme.text
            placeholderTextColor: root.theme.textMuted
            selectionColor: root.theme.accent
            selectedTextColor: root.theme.selectedText
            inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoAutoUppercase
            font.family: "monospace"
            font.pixelSize: root.theme.dataText
            Layout.fillWidth: true
            Layout.preferredHeight: visible ? root.theme.controlHeight : 0

            background: Rectangle {
                radius: root.theme.radius
                color: root.theme.field
                border.width: endpointField.activeFocus ? 1 : 0
                border.color: root.theme.accent
            }
        }

        DetailValueRow {
            objectName: "channelSourceModuleInfo"
            visible: root.targetKind === "module"
            theme: root.theme
            label: qsTr("Module")
            value: root.moduleDefault()
            copyable: true
            Layout.fillWidth: true
        }

        CheckBox {
            id: insecureHttpCheck

            visible: root.targetKind === "rpc" && root.remoteInsecureHttp(endpointField.text)
            text: qsTr("Allow insecure remote HTTP")
            enabled: !root.zoneState.sourceMutationInFlight
            font.pixelSize: root.theme.dataText
            Layout.fillWidth: true

            contentItem: Text {
                leftPadding: insecureHttpCheck.indicator.width + root.theme.gapSmall
                text: insecureHttpCheck.text
                color: root.theme.warning
                textFormat: Text.PlainText
                verticalAlignment: Text.AlignVCenter
                font.pixelSize: root.theme.dataText
            }
        }

        Text {
            visible: root.zoneState.verification !== "verified"
            text: qsTr("Catalog verification changed. Draft retained; saving is disabled.")
            color: root.theme.warning
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: root.theme.dataText
            Layout.fillWidth: true
        }

        RowLayout {
            visible: root.conflict
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Text {
                text: qsTr("Source revision changed. Draft was not rebased.")
                color: root.theme.warning
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Reload")
                onClicked: root.reloadRequested()
            }
        }

        Text {
            visible: root.zoneState.sourceMutationError.length > 0 && !root.conflict
            text: root.zoneState.sourceMutationError
            color: root.theme.error
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: root.theme.dataText
            Layout.fillWidth: true
        }

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Item { Layout.fillWidth: true }

            ActionButton {
                theme: root.theme
                text: qsTr("Cancel")
                enabled: !root.zoneState.sourceMutationInFlight
                onClicked: root.cancelled()
            }

            ActionButton {
                id: saveButton

                theme: root.theme
                objectName: "channelSourceSaveButton"
                text: qsTr("Save")
                primary: true
                enabled: root.validDraft
                    && root.dirty
                    && !root.conflict
                    && root.zoneState.verification === "verified"
                    && root.zoneState.activeZoneId.length > 0
                    && !root.zoneState.sourceMutationInFlight
                onClicked: root.submit()
            }
        }
    }

    function begin(nextRole, nextSource, revision) {
        role = String(nextRole || "sequencer")
        rebuildTargetOptions()
        source = nextSource || null
        mode = source ? "edit" : "add"
        expectedRevision = Number(revision || 0)
        initialLabel = String(source && source.label || "")
        initialTargetKind = String(source && source.target && source.target.kind || "rpc")
        initialTargetValue = Presentation.targetText(source && source.target)
        if (initialTargetValue === "-") {
            initialTargetValue = initialTargetKind === "module" ? moduleDefault() : ""
        }
        labelField.text = initialLabel
        targetKind = initialTargetKind
        endpointField.text = initialTargetKind === "rpc" ? initialTargetValue : ""
        insecureHttpCheck.checked = false
        conflict = false
    }

    function submit() {
        if (!saveButton.enabled) {
            return false
        }
        const label = labelField.text.trim().length > 0 ? labelField.text.trim() : null
        const target = targetKind === "module"
            ? { kind: "module", module_id: moduleDefault() }
            : { kind: "rpc", endpoint: endpointField.text.trim() }
        let mutation
        if (role === "indexer") {
            mutation = {
                kind: "set_indexer",
                label: label,
                target: target,
                allow_insecure_http: insecureHttpCheck.visible && insecureHttpCheck.checked
            }
        } else if (mode === "edit") {
            mutation = {
                kind: "update_sequencer",
                source_id: String(source && source.source_id || ""),
                label: label,
                target: target,
                allow_insecure_http: insecureHttpCheck.visible && insecureHttpCheck.checked
            }
        } else {
            mutation = {
                kind: "add_sequencer",
                label: label,
                target: target,
                allow_insecure_http: insecureHttpCheck.visible && insecureHttpCheck.checked
            }
        }
        root.zoneState.applyChannelSourceConfig({
            expected_config_revision: expectedRevision,
            mutation: mutation
        }, function (response) {
            if (response && response.ok === true) {
                root.saved()
                return
            }
            const error = String(response && response.error || "")
            if (error.toLowerCase().indexOf("revision conflict") >= 0) {
                root.conflict = true
            }
        })
        return true
    }

    function moduleDefault() {
        return String(root.adapterPolicy.module_id || "")
    }

    function selectedAdapterPolicy() {
        const modes = sourceModes()
        for (let i = 0; i < modes.length; ++i) {
            const mode = modes[i] || {}
            if (String(mode.key || "") === targetKind) {
                return mode.adapter && typeof mode.adapter === "object" ? mode.adapter : ({})
            }
        }
        return ({})
    }

    function sourceModes() {
        const family = role === "indexer"
            ? "execution_zone_indexer" : "execution_zone_sequencer"
        return SourcePolicyCatalog.sourceModes(family)
    }

    function targetModeImplemented(value) {
        const key = String(value || "")
        const modes = sourceModes()
        for (let i = 0; i < modes.length; ++i) {
            if (String(modes[i] && modes[i].key || "") === key) {
                return modes[i].implemented === true
            }
        }
        return false
    }

    function rebuildTargetOptions() {
        targetOptions.clear()
        const modes = sourceModes()
        for (let i = 0; i < modes.length; ++i) {
            const mode = modes[i] || ({})
            if (mode.implemented !== true) {
                continue
            }
            const key = String(mode.key || "")
            if (!key.length) {
                continue
            }
            targetOptions.append({
                value: key,
                label: key === "rpc" ? qsTr("RPC")
                    : (key === "module" ? qsTr("Module")
                        : String(mode.label || key))
            })
        }
    }

    function adapterAcceptsInput(inputKey) {
        const inputs = root.adapterPolicy && Array.isArray(root.adapterPolicy.inputs)
            ? root.adapterPolicy.inputs : []
        for (let i = 0; i < inputs.length; ++i) {
            if (String(inputs[i] && inputs[i].key || "") === String(inputKey || "")) {
                return true
            }
        }
        return false
    }

    function remoteInsecureHttp(value) {
        return Presentation.remoteInsecureHttp(value)
    }
}
