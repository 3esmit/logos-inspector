pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../components/common"
import "../../../theme"
import "../ZonePresentation.js" as Presentation

ColumnLayout {
    id: root

    required property Theme theme
    required property var zoneState
    required property var detail
    property bool editorOpen: false
    property string draftRole: "sequencer"
    property var draftSource: null
    property string pendingRemoveRole: ""
    property var pendingRemoveSource: null
    readonly property var config: root.detail && root.detail.channel_source_config
        ? root.detail.channel_source_config : ({})
    readonly property var observations: root.detail && Array.isArray(root.detail.source_observations)
        ? root.detail.source_observations : []
    readonly property bool hasDirtyDraft: editorLoader.editor !== null
        && editorLoader.editor.dirty

    objectName: "channelSourcesSection"
    spacing: root.theme.gapLarge
    Layout.fillWidth: true

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        Text {
            text: qsTr("Sequencer sources")
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: root.theme.secondaryText
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        Text {
            text: qsTr("Revision %1").arg(Presentation.numberText(root.config.config_revision))
            color: root.theme.textDim
            textFormat: Text.PlainText
            font.pixelSize: root.theme.dataText
        }

        ToolButton {
            id: addSequencerButton

            objectName: "addSequencerSourceButton"
            enabled: !root.hasDirtyDraft && !root.zoneState.sourceMutationInFlight
                && root.zoneState.verification === "verified"
            text: "+"
            hoverEnabled: true
            focusPolicy: Qt.TabFocus
            padding: 0
            Layout.preferredWidth: 30
            Layout.preferredHeight: 30
            onClicked: root.beginEditor("sequencer", null)

            ToolTip.visible: hovered
            ToolTip.delay: 500
            ToolTip.text: qsTr("Add Sequencer source")

            background: Rectangle {
                radius: root.theme.radius
                color: addSequencerButton.down ? root.theme.accentMuted
                    : (addSequencerButton.hovered || addSequencerButton.activeFocus
                        ? root.theme.hover : root.theme.surfaceRaised)
                border.width: 1
                border.color: addSequencerButton.activeFocus ? root.theme.accent : root.theme.outline
            }

            contentItem: Text {
                text: addSequencerButton.text
                color: addSequencerButton.enabled ? root.theme.text : root.theme.textDim
                textFormat: Text.PlainText
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                font.pixelSize: 20
            }

            Accessible.name: qsTr("Add Sequencer source")
        }
    }

    Text {
        visible: !Array.isArray(root.config.sequencer_sources)
            || root.config.sequencer_sources.length === 0
        text: qsTr("No Sequencer source configured")
        color: root.theme.textMuted
        textFormat: Text.PlainText
        font.pixelSize: root.theme.dataText
        Layout.fillWidth: true
    }

    Repeater {
        model: Array.isArray(root.config.sequencer_sources)
            ? root.config.sequencer_sources : []

        ChannelSourceRow {
            required property var modelData

            theme: root.theme
            source: modelData
            observation: Presentation.observationFor(root.observations, modelData.source_id)
            role: "sequencer"
            selected: String(root.config.selected_sequencer_source_id || "")
                === String(modelData.source_id || "")
            actionsEnabled: !root.hasDirtyDraft && !root.zoneState.sourceMutationInFlight
                && root.zoneState.verification === "verified"
            Layout.fillWidth: true
            onSelectRequested: root.selectSequencer(modelData)
            onEditRequested: root.beginEditor("sequencer", modelData)
            onRemoveRequested: root.confirmRemove("sequencer", modelData)
            onRetryRequested: root.retryAttestation(modelData)
        }
    }

    Rectangle {
        color: root.theme.outlineMuted
        Layout.fillWidth: true
        Layout.preferredHeight: 1
    }

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        Text {
            text: qsTr("Channel Indexer")
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: root.theme.secondaryText
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        ToolButton {
            id: configureIndexerButton

            objectName: "configureIndexerSourceButton"
            visible: !root.config.indexer_source
            enabled: !root.hasDirtyDraft && !root.zoneState.sourceMutationInFlight
                && root.zoneState.verification === "verified"
            text: "+"
            hoverEnabled: true
            focusPolicy: Qt.TabFocus
            padding: 0
            Layout.preferredWidth: 30
            Layout.preferredHeight: 30
            onClicked: root.beginEditor("indexer", root.config.indexer_source || null)

            ToolTip.visible: hovered
            ToolTip.delay: 500
            ToolTip.text: qsTr("Configure Indexer")

            background: Rectangle {
                radius: root.theme.radius
                color: configureIndexerButton.down ? root.theme.accentMuted
                    : (configureIndexerButton.hovered || configureIndexerButton.activeFocus
                        ? root.theme.hover : root.theme.surfaceRaised)
                border.width: 1
                border.color: configureIndexerButton.activeFocus ? root.theme.accent : root.theme.outline
            }

            contentItem: Text {
                text: configureIndexerButton.text
                color: configureIndexerButton.enabled ? root.theme.text : root.theme.textDim
                textFormat: Text.PlainText
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                font.pixelSize: 18
            }

            Accessible.name: qsTr("Configure Indexer")
        }
    }

    Text {
        visible: !root.config.indexer_source
        text: qsTr("Indexer not configured")
        color: root.theme.textMuted
        textFormat: Text.PlainText
        font.pixelSize: root.theme.dataText
        Layout.fillWidth: true
    }

    ChannelSourceRow {
        visible: root.config.indexer_source !== null && root.config.indexer_source !== undefined
        theme: root.theme
        source: root.config.indexer_source || ({})
        observation: Presentation.observationFor(
            root.observations,
            root.config.indexer_source && root.config.indexer_source.source_id
        )
        role: "indexer"
        actionsEnabled: !root.hasDirtyDraft && !root.zoneState.sourceMutationInFlight
            && root.zoneState.verification === "verified"
        Layout.fillWidth: true
        onEditRequested: root.beginEditor("indexer", root.config.indexer_source)
        onRemoveRequested: root.confirmRemove("indexer", root.config.indexer_source)
    }

    Loader {
        id: editorLoader

        readonly property ChannelSourceEditor editor: item as ChannelSourceEditor

        active: root.editorOpen
        asynchronous: false
        Layout.fillWidth: true
        sourceComponent: ChannelSourceEditor {
            theme: root.theme
            zoneState: root.zoneState
            onSaved: root.discardDraft()
            onCancelled: root.discardDraft()
            onReloadRequested: root.reloadDraft()
        }
        onLoaded: root.initializeEditor()
    }

    Text {
        visible: root.zoneState.sourceMutationWarning !== null
        text: String(root.zoneState.sourceMutationWarning
            && root.zoneState.sourceMutationWarning.message || "")
        color: root.theme.warning
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: root.theme.dataText
        Layout.fillWidth: true
    }

    Text {
        visible: root.zoneState.sourceMutationError.length > 0 && !root.editorOpen
        text: root.zoneState.sourceMutationError
        color: root.theme.error
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: root.theme.dataText
        Layout.fillWidth: true
    }

    ConfirmActionPopup {
        id: removePopup

        theme: root.theme
        title: root.pendingRemoveRole === "indexer"
            ? qsTr("Remove Indexer") : qsTr("Remove Sequencer source")
        message: qsTr("Remove %1 from this Channel?").arg(
            Presentation.text(root.pendingRemoveSource && root.pendingRemoveSource.label,
                Presentation.targetText(root.pendingRemoveSource && root.pendingRemoveSource.target))
        )
        confirmText: qsTr("Remove")
        onAccepted: root.removePendingSource()
    }

    function beginEditor(role, source) {
        draftRole = String(role || "sequencer")
        draftSource = source || null
        editorOpen = true
        Qt.callLater(root.initializeEditor)
    }

    function initializeEditor() {
        if (!editorLoader.editor) {
            return false
        }
        editorLoader.editor.begin(draftRole, draftSource, Number(config.config_revision || 0))
        return true
    }

    function discardDraft() {
        editorOpen = false
        draftSource = null
    }

    function reloadDraft() {
        const sourceId = String(draftSource && draftSource.source_id || "")
        let currentSource = null
        if (draftRole === "indexer") {
            currentSource = config.indexer_source || null
        } else {
            const sources = Array.isArray(config.sequencer_sources) ? config.sequencer_sources : []
            for (let i = 0; i < sources.length; ++i) {
                if (String(sources[i] && sources[i].source_id || "") === sourceId) {
                    currentSource = sources[i]
                    break
                }
            }
        }
        draftSource = currentSource
        initializeEditor()
    }

    function selectSequencer(source) {
        if (!source || root.hasDirtyDraft) {
            return false
        }
        const selectedId = String(config.selected_sequencer_source_id || "")
        const sourceId = String(source.source_id || "")
        zoneState.applyChannelSourceConfig({
            expected_config_revision: Number(config.config_revision || 0),
            mutation: {
                kind: "select_sequencer",
                source_id: selectedId === sourceId ? null : sourceId
            }
        })
        return true
    }

    function retryAttestation(source) {
        if (!source || root.hasDirtyDraft) {
            return false
        }
        zoneState.applyChannelSourceConfig({
            expected_config_revision: Number(config.config_revision || 0),
            mutation: {
                kind: "retry_attestation",
                source_id: String(source.source_id || "")
            }
        })
        return true
    }

    function confirmRemove(role, source) {
        if (!source || root.hasDirtyDraft) {
            return false
        }
        pendingRemoveRole = role
        pendingRemoveSource = source
        removePopup.open()
        return true
    }

    function removePendingSource() {
        if (!pendingRemoveSource) {
            return false
        }
        const mutation = pendingRemoveRole === "indexer"
            ? { kind: "remove_indexer" }
            : {
                kind: "remove_sequencer",
                source_id: String(pendingRemoveSource.source_id || "")
            }
        pendingRemoveSource = null
        zoneState.applyChannelSourceConfig({
            expected_config_revision: Number(config.config_revision || 0),
            mutation: mutation
        })
        return true
    }
}
