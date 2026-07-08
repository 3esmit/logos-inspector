pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../state"
import "../../../theme"
import "../../../utils/UiFormat.js" as UiFormat

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model

    width: parent ? parent.width : 900
    spacing: 16

    Component.onCompleted: {
        if (!model.channelsPageRows.length) {
            model.chainPages.refreshChannelsPage();
        }
    }

    PagedInspectionTable {
        theme: root.theme
        loadCount: root.model.channelsPageLimit
        rangeText: root.slotRangeText()
        canGoNewer: root.canLoadNewer()
        canGoOlder: root.model.channelsPageSlotFrom > 0
        busy: root.model.busy
        Layout.fillWidth: true
        headerCells: [
            { text: qsTr("Channel"), width: 210, fill: true },
            { text: qsTr("Label"), width: 120 },
            { text: qsTr("Last L1 slot"), width: 86 },
            { text: qsTr("Balance"), width: 86 },
            { text: qsTr("Keys"), width: 86 }
        ]
        rows: root.channelRows()
        onRefreshRequested: root.model.chainPages.refreshChannelsPage()
        onNewerRequested: root.model.chainPages.newerChannelsPage()
        onOlderRequested: root.model.chainPages.olderChannelsPage()
        onLoadCountSelected: function (count) {
            root.model.chainPages.setChannelsPageLimit(count)
        }
        onCellActivated: function (row, column, cell, rowData) {
            if (column === 0 && rowData.channelRaw.length > 0) {
                root.model.entityNavigation.openChannel(rowData.raw)
            }
        }
    }

    StatusMessage {
        visible: root.model.channelsPageError.length > 0
        theme: root.theme
        tone: "warning"
        title: root.model.chainPages.sourceProblemTitle("blockchain", root.model.channelsPageError, qsTr("Channels unavailable"))
        message: root.model.channelsPageError
        Layout.fillWidth: true
    }

    ChannelDetailPane {
        value: root.model.channelDetailValue
        theme: root.theme
        model: root.model
    }

    StatusMessage {
        visible: root.model.channelDetailValue === null && root.model.channelDetailError.length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Channel lookup failed")
        message: root.model.channelDetailError
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.model.channelDetailValue === null && root.model.channelDetailError.length === 0
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
                channelRaw: "",
                cells: [
                    { text: root.model.chainPages.sourceEmptyText("blockchain", root.model.channelsPageError, qsTr("No channels in loaded range")), width: 210, fill: true, monospace: false },
                    { text: "-", width: 120 },
                    { text: "-", width: 86 },
                    { text: "-", width: 86 },
                    { text: "-", width: 86 }
                ],
                raw: {},
                selected: false
            }];
        }
        return channels.map(function (channel) {
            const channelId = String(channel.channel || "")
            return {
                channelRaw: channelId,
                cells: [
                    { text: UiFormat.shortId(channelId), width: 210, fill: true, link: channelId.length > 0, copyText: channelId },
                    { text: root.blankText(channel.label), width: 120, monospace: false },
                    { text: root.numberText(channel.last_slot), width: 86 },
                    { text: root.blankText(channel.balance), width: 86 },
                    { text: root.numberText(channel.keys), width: 86 }
                ],
                raw: channel,
                selected: root.isSelectedChannel(channelId)
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
        return UiFormat.numberText(value);
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
        const info = root.model.chainPages.blockchainInfo()
        if (!info || info[field] === undefined || info[field] === null) {
            return 0
        }
        return Number(info[field] || 0)
    }

    function isSelectedChannel(channel) {
        const detail = root.model.channelDetailValue;
        return detail !== null && String(detail.channel || "") === String(channel || "");
    }

}
