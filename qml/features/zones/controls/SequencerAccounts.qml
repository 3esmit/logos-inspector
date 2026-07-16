pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property var zoneState
    property string accountQuery: String(root.zoneState.l2AccountId || "")

    signal configureSourcesRequested()

    objectName: "sequencerAccounts"
    spacing: root.theme.gapLarge
    Layout.fillWidth: true

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        ColumnLayout {
            spacing: root.theme.gapTiny
            Layout.fillWidth: true

            Text {
                text: qsTr("Sequencer Account")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: root.theme.panelTitleText
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            Text {
                text: root.zoneState.l2SequencerSourceId()
                color: root.theme.textMuted
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
                font.family: "monospace"
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }
        }

        ActionButton {
            visible: root.zoneState.l2AccountId.length > 0
            theme: root.theme
            text: qsTr("Refresh snapshot")
            enabled: !root.zoneState.l2AccountProvisionalInFlight
            Layout.preferredWidth: 156
            onClicked: root.zoneState.refreshL2SequencerAccount()
        }
    }

    StatusMessage {
        visible: !root.zoneState.l2SequencerReadEnabled
        theme: root.theme
        tone: root.zoneState.l2Applicable ? "warning" : "info"
        title: root.zoneState.l2Applicable
            ? qsTr("Sequencer source required") : qsTr("L2 not applicable")
        message: root.zoneState.l2Applicable
            ? qsTr("Select a Sequencer source for this Zone.")
            : root.zoneState.l2AvailabilityMessage()
        Layout.fillWidth: true
    }

    ActionButton {
        visible: root.zoneState.l2Applicable
            && !root.zoneState.l2SequencerReadEnabled
        theme: root.theme
        text: qsTr("Open Sources")
        Layout.preferredWidth: 150
        onClicked: root.configureSourcesRequested()
    }

    RowLayout {
        visible: root.zoneState.l2SequencerReadEnabled
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        FieldRow {
            objectName: "sequencerAccountField"
            theme: root.theme
            label: qsTr("Account ID")
            placeholderText: qsTr("Base58 or hex account ID")
            sourceText: root.accountQuery
            syncSourceText: true
            Layout.fillWidth: true
            onTextEdited: function (value) {
                root.accountQuery = String(value || "").trim()
            }
        }

        ActionButton {
            objectName: "sequencerAccountInspectButton"
            theme: root.theme
            text: qsTr("Inspect")
            primary: true
            enabled: root.accountQuery.length > 0
                && !root.zoneState.l2AccountProvisionalInFlight
            Layout.preferredWidth: 110
            Layout.alignment: Qt.AlignBottom | Qt.AlignLeft
            onClicked: root.zoneState.inspectL2SequencerAccount(
                root.accountQuery)
        }
    }

    ZoneL2AccountSnapshot {
        objectName: "sequencerAccountSnapshot"
        theme: root.theme
        title: qsTr("Provisional Snapshot")
        emptyText: qsTr("Enter an account ID to read selected Sequencer state.")
        snapshot: root.zoneState.l2AccountProvisional
        report: root.zoneState.l2AccountProvisionalReport
        error: root.zoneState.l2AccountProvisionalError
        busy: root.zoneState.l2AccountProvisionalInFlight
        decode: root.zoneState.l2AccountProvisionalDecode
        decodeError: root.zoneState.l2AccountProvisionalDecodeError
        decodeInFlight: root.zoneState.l2AccountProvisionalDecodeInFlight
        Layout.fillWidth: true
    }
}
