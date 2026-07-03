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

    ListToolbar {
        theme: root.theme
        loadCount: root.model.channelsPageLimit
        rangeText: root.slotRangeText()
        canGoNewer: root.canLoadNewer()
        canGoOlder: root.model.channelsPageSlotFrom > 0
        busy: root.model.busy
        Layout.fillWidth: true
        onRefresh: root.model.refreshChannelsPage()
        onNewer: root.model.newerChannelsPage()
        onOlder: root.model.olderChannelsPage()
        onLoadCountSelected: function (count) {
            root.model.setChannelsPageLimit(count)
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
                columns: [qsTr("Channel"), qsTr("Label"), qsTr("Last L1 slot"), qsTr("Balance"), qsTr("Keys")]
            }

            Repeater {
                model: root.channelRows()

                ChannelRow {
                    required property var modelData

                    theme: root.theme
                    columns: [modelData.channel, modelData.label, modelData.lastSlot, modelData.balance, modelData.keys]
                    channel: modelData.channelRaw
                    selected: root.isSelectedChannel(modelData.channelRaw)
                    onChannelActivated: root.model.openChannel(modelData.raw)
                }
            }
        }
    }

    StatusMessage {
        visible: root.model.channelsPageError.length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Channels unavailable")
        message: root.model.channelsPageError
        Layout.fillWidth: true
    }

    ChannelDetailPane {
        value: root.model.channelDetailValue
        theme: root.theme
        model: root.model
    }

    StatusMessage {
        visible: root.model.channelDetailValue === null
        theme: root.theme
        tone: "info"
        title: qsTr("Channel detail")
        message: qsTr("Select a channel to inspect first and last activity, live state, balances, and stored keys.")
        Layout.fillWidth: true
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
            return value.toLocaleString(Qt.locale(), "f", 0);
        }
        const numeric = Number(value);
        if (Number.isFinite(numeric)) {
            return numeric.toLocaleString(Qt.locale(), "f", 0);
        }
        return String(value);
    }

    function slotRangeText() {
        if (root.model.channelsPageSlotTo <= 0) {
            return qsTr("No range loaded");
        }
        return qsTr("L1 slots %1-%2").arg(root.numberText(root.model.channelsPageSlotFrom)).arg(root.numberText(root.model.channelsPageSlotTo));
    }

    function canLoadNewer() {
        const current = root.chainSlot("slot")
        return root.model.channelsPageSlotTo > 0 && current > 0 && root.model.channelsPageSlotTo < current
    }

    function chainSlot(field) {
        const info = root.model.blockchainInfo()
        if (!info || info[field] === undefined || info[field] === null) {
            return 0
        }
        return Number(info[field] || 0)
    }

    function isSelectedChannel(channel) {
        const detail = root.model.channelDetailValue;
        return detail !== null && String(detail.channel || "") === String(channel || "");
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
        property bool selected: false
        signal channelActivated()

        Layout.fillWidth: true
        Layout.preferredHeight: rowRoot.header ? 36 : 42

        Rectangle {
            anchors.fill: parent
            color: rowRoot.header ? rowRoot.theme.field : (rowRoot.selected ? rowRoot.theme.accentMuted : "transparent")
            border.width: 0
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 1
            color: rowRoot.theme.outlineMuted
        }

        GridLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            columns: 5
            columnSpacing: 10

            Repeater {
                model: 5

                LinkCell {
                    required property int index

                    theme: rowRoot.theme
                    text: String(rowRoot.columns[index] || "-")
                    header: rowRoot.header
                    link: rowRoot.linkFor(index)
                    copyText: rowRoot.channel.length > 0 ? rowRoot.channel : String(rowRoot.columns[index] || "")
                    monospace: !rowRoot.header
                    textColor: rowRoot.textColor(index)
                    Layout.preferredWidth: rowRoot.columnWidth(index)
                    Layout.fillWidth: index === 0
                    onActivated: rowRoot.channelActivated()
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
