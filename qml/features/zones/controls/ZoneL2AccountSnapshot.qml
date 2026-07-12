pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../theme"
import "../ZonePresentation.js" as Presentation

ColumnLayout {
    id: root

    required property Theme theme
    property string title: ""
    property string emptyText: qsTr("Not requested")
    property var snapshot: null
    property var report: null
    property string error: ""
    property bool busy: false
    readonly property var account: root.snapshot && root.snapshot.account
        ? root.snapshot.account : ({})
    readonly property var source: root.snapshot && root.snapshot.source
        ? root.snapshot.source : ({})

    objectName: "zoneL2AccountSnapshot"
    spacing: root.theme.gapSmall
    Layout.fillWidth: true
    Layout.minimumWidth: 300
    Layout.alignment: Qt.AlignTop

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        Text {
            text: root.title
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: root.theme.secondaryText
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        ZoneKindChip {
            visible: root.snapshot !== null
            theme: root.theme
            label: Presentation.words(root.source.finality)
            tone: root.source.finality === "finalized"
                ? "success" : (root.snapshot
                    && root.snapshot.anchor_state === "moving" ? "warning" : "info")
        }
    }

    Rectangle {
        color: root.theme.outlineMuted
        Layout.fillWidth: true
        Layout.preferredHeight: 1
    }

    StatusMessage {
        visible: root.busy
        theme: root.theme
        tone: "info"
        title: qsTr("Loading snapshot")
        message: root.title
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.error.length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Snapshot unavailable")
        message: root.error
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.snapshot !== null
            && String(root.snapshot.anchor_state || "") === "moving"
        theme: root.theme
        tone: "warning"
        title: qsTr("Sequencer head moved")
        message: qsTr("Before and after anchors differ; values are provisional.")
        Layout.fillWidth: true
    }

    Text {
        visible: !root.busy && root.error.length === 0 && root.snapshot === null
        text: root.emptyText
        color: root.theme.textMuted
        textFormat: Text.PlainText
        font.pixelSize: root.theme.dataText
        Layout.fillWidth: true
    }

    ZoneFactSection {
        visible: root.snapshot !== null
        theme: root.theme
        title: qsTr("Account State")
        rows: root.accountRows()
    }

    ZoneFactSection {
        visible: root.snapshot !== null
        theme: root.theme
        title: qsTr("Block Anchor")
        rows: root.anchorRows()
    }

    ZoneFactSection {
        visible: root.snapshot !== null
        theme: root.theme
        title: qsTr("Source Evidence")
        rows: root.sourceRows()
    }

    function accountRows() {
        return [{
            label: qsTr("Account ID"),
            value: Presentation.text(root.account.account_id),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Base58"),
            value: Presentation.text(root.account.account_id_base58),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Hex"),
            value: Presentation.text(root.account.account_id_hex),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Balance"),
            value: Presentation.text(root.account.balance)
        }, {
            label: qsTr("Nonce"),
            value: Presentation.text(root.account.nonce)
        }, {
            label: qsTr("Owner program"),
            value: Presentation.text(root.account.owner_program_base58),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Owner hex"),
            value: Presentation.text(root.account.owner_program_hex),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Existence"),
            value: Presentation.words(root.account.existence),
            tone: "warning"
        }, {
            label: qsTr("Data bytes"),
            value: Presentation.numberText(root.hexBytes(root.account.data_hex))
        }, {
            label: qsTr("Data hex"),
            value: Presentation.text(root.account.data_hex),
            copyable: true,
            monospace: true
        }]
    }

    function anchorRows() {
        const before = root.snapshot && root.snapshot.anchor
            ? root.snapshot.anchor : ({})
        const after = root.snapshot && root.snapshot.after_anchor
            ? root.snapshot.after_anchor : null
        const rows = [{
            label: qsTr("Anchor state"),
            value: Presentation.words(root.snapshot && root.snapshot.anchor_state),
            tone: root.snapshot && root.snapshot.anchor_state === "moving"
                ? "warning" : "success"
        }, {
            label: qsTr("Block ID"),
            value: Presentation.numberText(before.block_id)
        }, {
            label: qsTr("Block hash"),
            value: Presentation.text(before.block_hash),
            copyable: true,
            monospace: true
        }]
        if (after) {
            rows.push({
                label: qsTr("After block"),
                value: Presentation.numberText(after.block_id)
            })
            rows.push({
                label: qsTr("After hash"),
                value: Presentation.text(after.block_hash),
                copyable: true,
                monospace: true
            })
        }
        return rows
    }

    function sourceRows() {
        const route = root.report && root.report.route ? root.report.route : ({})
        return [{
            label: qsTr("Source ID"),
            value: Presentation.text(root.source.source_id),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Role"),
            value: Presentation.words(root.source.source_role)
        }, {
            label: qsTr("Finality"),
            value: Presentation.words(root.source.finality),
            tone: root.source.finality === "finalized" ? "success" : "warning"
        }, {
            label: qsTr("Retrieval"),
            value: Presentation.words(root.source.retrieval)
        }, {
            label: qsTr("Config revision"),
            value: Presentation.numberText(root.source.source_config_revision)
        }, {
            label: qsTr("Route policy"),
            value: Presentation.words(route.policy)
        }]
    }

    function hexBytes(value) {
        const text = String(value || "")
        return Math.floor(text.length / 2)
    }
}
