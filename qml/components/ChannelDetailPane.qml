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
            text: root.detail ? qsTr("Home > Channels > %1").arg(root.shortId(root.detail.channel)) : ""
            color: root.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: 12
            Layout.fillWidth: true
        }

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
    }

    SectionBlock {
        theme: root.theme
        title: qsTr("Activity")
        rows: root.activityRows()
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
                        monospace: true
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
            raw: value.raw || null
        }
    }

    function activityRows() {
        if (!root.detail) {
            return []
        }
        return [
            {
                label: qsTr("First seen"),
                value: root.txText(root.detail.first_tx_hash, root.detail.first_slot),
                link: root.detail.first_tx_hash,
                monospace: true
            },
            {
                label: qsTr("Last seen"),
                value: root.txText(root.detail.last_tx_hash, root.detail.last_slot),
                link: root.detail.last_tx_hash,
                monospace: true
            }
        ]
    }

    function snapshotRows() {
        if (!root.detail) {
            return []
        }
        return [
            { label: qsTr("Tip"), value: root.valueText(root.detail.tip), monospace: true },
            { label: qsTr("Balance"), value: root.valueText(root.detail.balance), monospace: true },
            { label: qsTr("Withdraw threshold"), value: root.valueText(root.detail.withdraw_threshold), monospace: true }
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
                label: qsTr("Key %1").arg(index),
                value: String(key || "-")
            }
        })
    }

    function txText(hash, slot) {
        const parts = []
        if (hash) {
            parts.push(root.shortId(hash))
        }
        if (slot !== undefined && slot !== null && slot !== "") {
            parts.push(qsTr("slot %1").arg(root.numberText(slot)))
        }
        return parts.length ? parts.join(" ") : "-"
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
            return numeric.toLocaleString(Qt.locale())
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
                        link: String(modelData.link || "")
                        monospace: modelData.monospace !== undefined ? modelData.monospace : true
                        onActivated: root.model.openTransaction(modelData.link)
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
        property string link: ""
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

            Text {
                text: rowRoot.value
                color: rowRoot.link.length ? rowRoot.theme.accent : rowRoot.theme.text
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
                font.family: rowRoot.monospace ? "monospace" : ""
                font.pixelSize: 12
                font.underline: rowRoot.link.length > 0
                Layout.fillWidth: true

                MouseArea {
                    anchors.fill: parent
                    enabled: rowRoot.link.length > 0
                    cursorShape: Qt.PointingHandCursor
                    onClicked: rowRoot.activated()
                }
            }
        }
    }
}
