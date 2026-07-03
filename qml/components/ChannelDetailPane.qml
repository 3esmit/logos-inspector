pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../state"
import "../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model
    property var value: null
    readonly property var detail: normalize(value)

    visible: detail !== null
    spacing: 14
    Layout.fillWidth: true

    ColumnLayout {
        visible: root.detail !== null
        spacing: 6
        Layout.fillWidth: true

        Text {
            text: qsTr("Channel")
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: 22
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        Text {
            text: root.detail ? root.detail.channel : ""
            color: root.theme.textMuted
            textFormat: Text.PlainText
            wrapMode: Text.WrapAnywhere
            font.family: "monospace"
            font.pixelSize: 12
            Layout.fillWidth: true
        }

        SourceStrip {
            theme: root.theme
            sources: [qsTr("L1 Bedrock"), qsTr("Channel evidence")]
        }
    }

    GridLayout {
        visible: root.detail !== null
        columns: root.width < 760 ? 2 : 4
        columnSpacing: 12
        rowSpacing: 12
        Layout.fillWidth: true

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Last L1 slot")
            value: root.detail ? root.numberText(root.detail.last_slot) : "-"
            delta: root.detail && root.detail.last_tx_hash.length ? root.shortId(root.detail.last_tx_hash) : qsTr("No transaction")
            deltaColor: root.detail && root.detail.last_tx_hash.length ? root.theme.accent : root.theme.textMuted
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Operations")
            value: root.detail ? root.numberText(root.detail.operations) : "-"
            delta: qsTr("Detected")
            deltaColor: root.detail && Number(root.detail.operations || 0) > 0 ? root.theme.success : root.theme.textMuted
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Balance")
            value: root.detail ? root.valueText(root.detail.balance) : "-"
            delta: qsTr("Latest snapshot")
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Keys")
            value: root.detail ? root.numberText(root.detail.keys) : "-"
            delta: root.detail && root.detail.key_values.length ? qsTr("Stored values") : qsTr("Count only")
        }
    }

    SectionBlock {
        theme: root.theme
        title: qsTr("Activity")
        rows: root.activityRows()
    }

    SectionBlock {
        theme: root.theme
        title: qsTr("Evidence")
        rows: root.evidenceRows()
    }

    SectionBlock {
        theme: root.theme
        title: qsTr("Live snapshot")
        rows: root.snapshotRows()
    }

    ColumnLayout {
        visible: root.detail !== null
        spacing: 8
        Layout.fillWidth: true

        Text {
            text: qsTr("Keys")
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: 14
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        Frame {
            padding: 0
            Layout.fillWidth: true

            background: Rectangle {
                color: root.theme.surface
                radius: root.theme.radius
                border.width: 1
                border.color: root.theme.outlineMuted
            }

            contentItem: ColumnLayout {
                spacing: 0

                Repeater {
                    model: root.keyRows()

                    DetailRow {
                        required property var modelData

                        theme: root.theme
                        label: String(modelData.label || "")
                        value: String(modelData.value || "-")
                        linkKind: String(modelData.linkKind || "")
                        linkValue: String(modelData.linkValue || "")
                        monospace: true
                        onActivated: root.model.openReference(modelData.linkKind, modelData.linkValue)
                    }
                }
            }
        }
    }

    function normalize(value) {
        if (!value || typeof value !== "object" || Array.isArray(value) || value.type !== "channel") {
            return null
        }
        return {
            channel: String(value.channel || value.channel_id || ""),
            channel_id: String(value.channel_id || value.channel || ""),
            operation_type: String(value.operation_type || ""),
            l1_slot: value.l1_slot,
            header: String(value.header || value.l1_header_hash || ""),
            tx_hash: String(value.tx_hash || value.transaction_hash || ""),
            parent: String(value.parent || ""),
            signer: String(value.signer || ""),
            source_confidence: String(value.source_confidence || ""),
            label: String(value.label || ""),
            first_slot: value.first_slot,
            first_tx_hash: String(value.first_tx_hash || ""),
            first_block_hash: String(value.first_block_hash || ""),
            last_slot: value.last_slot,
            last_tx_hash: String(value.last_tx_hash || ""),
            last_block_hash: String(value.last_block_hash || ""),
            tip: String(value.tip || ""),
            balance: value.balance,
            withdraw_threshold: value.withdraw_threshold,
            keys: value.keys,
            key_values: Array.isArray(value.key_values) ? value.key_values : [],
            operations: value.operations,
            raw_json: value.raw_json || value.raw || null,
            raw: value.raw || null
        }
    }

    function activityRows() {
        if (!root.detail) {
            return []
        }
        return [
            { label: qsTr("First L1 slot"), value: root.numberText(root.detail.first_slot), linkKind: root.hasValue(root.detail.first_slot) ? "block" : "", linkValue: root.numberText(root.detail.first_slot), monospace: true },
            { label: qsTr("First Mantle tx"), value: root.valueText(root.detail.first_tx_hash), linkKind: root.detail.first_tx_hash.length ? "mantleTransaction" : "", linkValue: root.detail.first_tx_hash, monospace: true },
            { label: qsTr("First header"), value: root.valueText(root.detail.first_block_hash), linkKind: root.detail.first_block_hash.length ? "block" : "", linkValue: root.detail.first_block_hash, monospace: true },
            { label: qsTr("Last L1 slot"), value: root.numberText(root.detail.last_slot), linkKind: root.hasValue(root.detail.last_slot) ? "block" : "", linkValue: root.numberText(root.detail.last_slot), monospace: true },
            { label: qsTr("Last Mantle tx"), value: root.valueText(root.detail.last_tx_hash), linkKind: root.detail.last_tx_hash.length ? "mantleTransaction" : "", linkValue: root.detail.last_tx_hash, monospace: true },
            { label: qsTr("Last header"), value: root.valueText(root.detail.last_block_hash), linkKind: root.detail.last_block_hash.length ? "block" : "", linkValue: root.detail.last_block_hash, monospace: true }
        ]
    }

    function evidenceRows() {
        if (!root.detail) {
            return []
        }
        return [
            { label: qsTr("Channel ID"), value: root.valueText(root.detail.channel_id), monospace: true },
            { label: qsTr("Operation type"), value: root.valueText(root.detail.operation_type), monospace: false },
            { label: qsTr("Evidence L1 slot"), value: root.numberText(root.detail.l1_slot), linkKind: root.hasValue(root.detail.l1_slot) ? "block" : "", linkValue: root.numberText(root.detail.l1_slot), monospace: true },
            { label: qsTr("Header"), value: root.valueText(root.detail.header), linkKind: root.detail.header.length ? "block" : "", linkValue: root.detail.header, monospace: true },
            { label: qsTr("Mantle tx"), value: root.valueText(root.detail.tx_hash), linkKind: root.detail.tx_hash.length ? "mantleTransaction" : "", linkValue: root.detail.tx_hash, monospace: true },
            { label: qsTr("Parent"), value: root.valueText(root.detail.parent), linkKind: root.detail.parent.length ? "block" : "", linkValue: root.detail.parent, monospace: true },
            { label: qsTr("Signer"), value: root.valueText(root.detail.signer), linkKind: root.detail.signer.length ? "account" : "", linkValue: root.detail.signer, monospace: true },
            { label: qsTr("Source confidence"), value: root.valueText(root.detail.source_confidence), monospace: false }
        ]
    }

    function snapshotRows() {
        if (!root.detail) {
            return []
        }
        return [
            { label: qsTr("Label"), value: root.valueText(root.detail.label), monospace: false },
            { label: qsTr("Tip"), value: root.valueText(root.detail.tip), monospace: true },
            { label: qsTr("Balance"), value: root.valueText(root.detail.balance), monospace: true },
            { label: qsTr("Withdraw threshold"), value: root.valueText(root.detail.withdraw_threshold), monospace: true },
            { label: qsTr("Operations"), value: root.numberText(root.detail.operations), monospace: true }
        ]
    }

    function keyRows() {
        const keys = root.detail ? root.detail.key_values : []
        if (!keys.length) {
            return [{
                label: qsTr("Keys"),
                value: root.detail && root.detail.keys !== undefined && root.detail.keys !== null ? qsTr("%1 key(s)").arg(root.detail.keys) : "-"
            }]
        }
        return keys.map(function (key, index) {
            return {
                label: qsTr("Key %1").arg(index + 1),
                value: String(key || "-"),
                linkKind: root.isLikelyAccount(key) ? "account" : "",
                linkValue: String(key || "")
            }
        })
    }

    function hasValue(value) {
        return value !== undefined && value !== null && value !== ""
    }

    function valueText(value) {
        if (value === undefined || value === null || value === "") {
            return "-"
        }
        return String(value)
    }

    function numberText(value) {
        if (value === undefined || value === null || value === "") {
            return "-"
        }
        const numeric = Number(value)
        if (Number.isFinite(numeric)) {
            return numeric % 1 === 0 ? numeric.toLocaleString(Qt.locale(), "f", 0) : String(value)
        }
        return String(value)
    }

    function shortId(value) {
        const text = String(value || "")
        if (text.length <= 16) {
            return text.length ? text : "-"
        }
        return text.slice(0, 8) + "..." + text.slice(-6)
    }

    function isLikelyAccount(value) {
        const text = String(value || "")
        return text.length >= 32 && text.length <= 128 && /^[1-9A-HJ-NP-Za-km-z]+$/.test(text)
    }

    component SectionBlock: ColumnLayout {
        id: sectionRoot

        required property Theme theme
        property string title: ""
        property var rows: []

        visible: rows.length > 0
        spacing: 6
        Layout.fillWidth: true

        Text {
            visible: sectionRoot.title.length > 0
            text: sectionRoot.title
            color: sectionRoot.theme.text
            textFormat: Text.PlainText
            font.pixelSize: 14
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        Frame {
            padding: 0
            Layout.fillWidth: true

            background: Rectangle {
                color: sectionRoot.theme.surface
                radius: sectionRoot.theme.radius
                border.width: 1
                border.color: sectionRoot.theme.outlineMuted
            }

            contentItem: ColumnLayout {
                spacing: 0

                Repeater {
                    model: sectionRoot.rows

                    DetailRow {
                        required property var modelData

                        theme: sectionRoot.theme
                        label: String(modelData.label || "")
                        value: String(modelData.value || "-")
                        linkKind: String(modelData.linkKind || "")
                        linkValue: String(modelData.linkValue || "")
                        monospace: modelData.monospace !== undefined ? modelData.monospace : true
                        onActivated: root.model.openReference(modelData.linkKind, modelData.linkValue)
                    }
                }
            }
        }
    }

    component DetailRow: Item {
        id: rowRoot

        required property Theme theme
        property string label: ""
        property string value: ""
        property string linkKind: ""
        property string linkValue: ""
        property bool monospace: true
        signal activated()

        Layout.fillWidth: true
        implicitHeight: Math.max(42, rowGrid.implicitHeight + 18)

        GridLayout {
            id: rowGrid

            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            anchors.topMargin: 8
            anchors.bottomMargin: 8
            columns: 2
            columnSpacing: 14
            rowSpacing: 3

            Text {
                text: rowRoot.label
                color: rowRoot.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: 11
                font.weight: Font.DemiBold
                font.capitalization: Font.AllUppercase
                Layout.preferredWidth: 128
                Layout.alignment: Qt.AlignTop
            }

            LinkCell {
                text: rowRoot.value
                theme: rowRoot.theme
                link: rowRoot.linkKind.length > 0
                copyText: rowRoot.linkValue.length > 0 ? rowRoot.linkValue : rowRoot.value
                monospace: rowRoot.monospace
                wrap: true
                Layout.fillWidth: true
                onActivated: rowRoot.activated()
            }
        }
    }
}
