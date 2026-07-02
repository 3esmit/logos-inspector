pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../components"
import "../state"
import "../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model

    width: parent ? parent.width : 900
    spacing: 16

    Component.onCompleted: {
        if (!model.channelsPageRows.length) {
            model.refreshChannelsPage();
        }
    }

    RowLayout {
        spacing: 12
        Layout.fillWidth: true

        ColumnLayout {
            spacing: 6
            Layout.fillWidth: true

            Text {
                text: qsTr("Home > Channels")
                color: root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: 12
                Layout.fillWidth: true
            }

            Text {
                text: qsTr("Channels")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: 28
                font.weight: Font.Bold
                Layout.fillWidth: true
            }

            Text {
                text: qsTr("Sorted by last activity.")
                color: root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: 14
                Layout.fillWidth: true
            }
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Latest")
            primary: true
            enabled: !root.model.busy
            Layout.preferredWidth: 104
            onClicked: root.model.refreshChannelsPage()
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Older >")
            enabled: !root.model.busy && root.model.channelsPageSlotFrom > 0
            Layout.preferredWidth: 104
            onClicked: root.model.olderChannelsPage()
        }
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

            ChannelRow {
                theme: root.theme
                header: true
                columns: [qsTr("Channel"), qsTr("Label"), qsTr("Last slot"), qsTr("Balance"), qsTr("Keys")]
            }

            Repeater {
                model: root.channelRows()

                ChannelRow {
                    required property var modelData

                    theme: root.theme
                    columns: [modelData.channel, modelData.label, modelData.lastSlot, modelData.balance, modelData.keys]
                    channel: modelData.channelRaw
                    onChannelActivated: root.model.openChannel(modelData.raw)
                }
            }
        }
    }

    Text {
        visible: root.model.channelsPageError.length > 0
        text: root.model.channelsPageError
        color: root.theme.warning
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 12
        Layout.fillWidth: true
    }

    ResultPane {
        theme: root.theme
        model: root.model
    }

    function channelRows() {
        const channels = root.model.channelsPageRows || [];
        if (!channels.length) {
            return [{
                channel: qsTr("No channels in loaded range"),
                channelRaw: "",
                label: "-",
                lastSlot: "-",
                balance: "-",
                keys: "-",
                raw: {}
            }];
        }
        return channels.map(function (channel) {
            return {
                channel: root.shortId(channel.channel),
                channelRaw: String(channel.channel || ""),
                label: root.blankText(channel.label),
                lastSlot: root.numberText(channel.last_slot),
                balance: root.blankText(channel.balance),
                keys: root.numberText(channel.keys),
                raw: channel
            };
        });
    }

    function blankText(value) {
        if (value === undefined || value === null || value === "") {
            return "-";
        }
        return String(value);
    }

    function numberText(value) {
        if (value === undefined || value === null || value === "") {
            return "-";
        }
        if (typeof value === "number") {
            return value.toLocaleString(Qt.locale());
        }
        const numeric = Number(value);
        if (Number.isFinite(numeric)) {
            return numeric.toLocaleString(Qt.locale());
        }
        return String(value);
    }

    function shortId(value) {
        const text = String(value || "");
        if (text.length <= 16) {
            return text.length ? text : "-";
        }
        return text.slice(0, 8) + "..." + text.slice(-6);
    }

    component ChannelRow: Item {
        id: rowRoot

        required property Theme theme
        property var columns: []
        property string channel: ""
        property bool header: false
        signal channelActivated()

        Layout.fillWidth: true
        Layout.preferredHeight: rowRoot.header ? 36 : 42

        Rectangle {
            anchors.fill: parent
            color: rowRoot.header ? rowRoot.theme.field : "transparent"
            border.width: 0
        }

        GridLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            columns: 5
            columnSpacing: 10

            Repeater {
                model: 5

                Text {
                    required property int index

                    text: String(rowRoot.columns[index] || "-")
                    color: rowRoot.textColor(index)
                    textFormat: Text.PlainText
                    font.family: rowRoot.header ? "" : "monospace"
                    font.pixelSize: rowRoot.header ? 11 : 12
                    font.weight: rowRoot.header ? Font.DemiBold : Font.Normal
                    font.capitalization: rowRoot.header ? Font.AllUppercase : Font.MixedCase
                    font.underline: rowRoot.linkFor(index)
                    elide: Text.ElideRight
                    Layout.preferredWidth: rowRoot.columnWidth(index)
                    Layout.fillWidth: index === 0

                    MouseArea {
                        anchors.fill: parent
                        enabled: rowRoot.linkFor(parent.index)
                        cursorShape: Qt.PointingHandCursor
                        onClicked: rowRoot.channelActivated()
                    }
                }
            }
        }

        function linkFor(index) {
            return !rowRoot.header && index === 0 && rowRoot.channel.length > 0;
        }

        function textColor(index) {
            if (rowRoot.linkFor(index)) {
                return rowRoot.theme.accent;
            }
            return rowRoot.header ? rowRoot.theme.textMuted : rowRoot.theme.text;
        }

        function columnWidth(index) {
            if (index === 0) {
                return 210;
            }
            if (index === 1) {
                return 120;
            }
            return 86;
        }
    }
}
