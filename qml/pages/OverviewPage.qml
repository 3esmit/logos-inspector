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
        if (!model.dashboardOverview) {
            model.refreshDashboard();
        }
    }

    RowLayout {
        spacing: 12
        Layout.fillWidth: true

        ColumnLayout {
            spacing: 6
            Layout.fillWidth: true

            RowLayout {
                spacing: 10
                Layout.fillWidth: true

                Text {
                    text: qsTr("Logos")
                    color: root.theme.text
                    textFormat: Text.PlainText
                    font.pixelSize: 28
                    font.weight: Font.Bold
                }

                Rectangle {
                    color: root.theme.surfaceRaised
                    radius: 12
                    Layout.preferredWidth: 88
                    Layout.preferredHeight: 26

                    Row {
                        anchors.centerIn: parent
                        spacing: 6

                        Rectangle {
                            width: 7
                            height: 7
                            radius: 4
                            color: root.serviceOk("sequencer", "health") ? root.theme.success : root.theme.warning
                            anchors.verticalCenter: parent.verticalCenter
                        }

                        Text {
                            text: root.chainLabel()
                            color: root.theme.textMuted
                            textFormat: Text.PlainText
                            font.pixelSize: 11
                            font.weight: Font.DemiBold
                            anchors.verticalCenter: parent.verticalCenter
                        }
                    }
                }
            }

            Text {
                text: qsTr("Explorer-style dashboard for node state, LEZ indexer data, and inspection shortcuts.")
                color: root.theme.textMuted
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 14
                Layout.fillWidth: true
            }
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Refresh")
            primary: true
            enabled: !root.model.busy
            Layout.preferredWidth: 116
            onClicked: root.model.refreshDashboard()
        }
    }

    RowLayout {
        spacing: 10
        Layout.fillWidth: true

        FieldRow {
            id: searchField
            theme: root.theme
            label: qsTr("Search")
            placeholderText: qsTr("Block id, transaction hash, or account address")
            Layout.fillWidth: true
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Open")
            primary: true
            enabled: searchField.text.trim().length > 0 && !root.model.busy
            Layout.preferredWidth: 104
            onClicked: root.model.routeSearch(searchField.text)
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

            DashboardHeader {
                theme: root.theme
                title: qsTr("Latest Blocks")
                action: qsTr("Live")
            }

            DashboardRow {
                theme: root.theme
                header: true
                columns: [qsTr("Slot"), qsTr("Header"), qsTr("Tx"), qsTr("Status")]
            }

            Repeater {
                model: root.blockRows()

                DashboardRow {
                    required property var modelData

                    theme: root.theme
                    columns: [modelData.slot, modelData.header, modelData.tx, modelData.status]
                }
            }
        }
    }

    GridLayout {
        columns: root.width < 720 ? 2 : (root.width < 1180 ? 3 : 6)
        columnSpacing: 12
        rowSpacing: 12
        Layout.fillWidth: true

        MetricCard {
            theme: root.theme
            label: qsTr("Tip slot")
            value: root.numberText(root.cryptarchiaValue("slot"))
            delta: qsTr("Latest")
            deltaColor: root.theme.accent
        }

        MetricCard {
            theme: root.theme
            label: qsTr("Tip height")
            value: root.numberText(root.cryptarchiaValue("height"))
            delta: root.libDeltaText()
            deltaColor: root.theme.success
        }

        MetricCard {
            theme: root.theme
            label: qsTr("LIB slot")
            value: root.numberText(root.cryptarchiaValue("lib_slot"))
            delta: qsTr("Finalized")
        }

        MetricCard {
            theme: root.theme
            label: qsTr("Mode")
            value: root.modeText()
            delta: qsTr("Cryptarchia")
        }

        MetricCard {
            theme: root.theme
            label: qsTr("Peers")
            value: root.numberText(root.networkValue("n_peers"))
            delta: qsTr("%1 connections").arg(root.numberText(root.networkValue("n_connections")))
            deltaColor: root.theme.accent
        }

        MetricCard {
            theme: root.theme
            label: qsTr("Mantle pending")
            value: root.numberText(root.mantleValue("pending_items"))
            delta: qsTr("Mempool")
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

            DashboardHeader {
                theme: root.theme
                title: qsTr("Latest transactions")
                action: qsTr("Indexed")
            }

            DashboardRow {
                theme: root.theme
                header: true
                columns: [qsTr("Slot"), qsTr("Tx hash"), qsTr("Block"), qsTr("Ops")]
            }

            Repeater {
                model: root.transactionRows()

                DashboardRow {
                    required property var modelData

                    theme: root.theme
                    columns: [modelData.slot, modelData.hash, modelData.block, modelData.ops]
                }
            }
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

            DashboardHeader {
                theme: root.theme
                title: qsTr("Connections")
                action: root.model.networkProfile
            }

            DashboardDetail {
                theme: root.theme
                label: qsTr("Sequencer")
                value: root.model.sequencerUrl
                status: root.serviceStatus("sequencer", "health")
            }

            DashboardDetail {
                theme: root.theme
                label: qsTr("Indexer")
                value: root.model.indexerUrl
                status: root.serviceStatus("indexer", "health")
            }

            DashboardDetail {
                theme: root.theme
                label: qsTr("Blockchain node")
                value: root.model.nodeUrl
                status: root.serviceStatus("node", "consensus")
            }
        }
    }

    Text {
        visible: root.model.dashboardError.length > 0
        text: root.model.dashboardError
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

    function overview() {
        return model.dashboardOverview || {};
    }

    function nodeReport() {
        return model.dashboardNode || {};
    }

    function serviceOk(section, field) {
        const target = overview()[section];
        const probe = target ? target[field] : null;
        return !!(probe && probe.ok);
    }

    function serviceStatus(section, field) {
        return serviceOk(section, field) ? qsTr("ok") : qsTr("error");
    }

    function chainLabel() {
        return model.networkProfile === "local-node" ? qsTr("Local") : qsTr("Testnet");
    }

    function consensusValue() {
        const node = overview().node;
        const probe = node ? node.consensus : null;
        return probe && probe.value ? probe.value : {};
    }

    function cryptarchiaInfo() {
        return consensusValue().cryptarchia_info || {};
    }

    function cryptarchiaValue(key) {
        const value = cryptarchiaInfo()[key];
        return value === undefined || value === null ? null : value;
    }

    function networkInfo() {
        return root.reportValue("network_info");
    }

    function mantleInfo() {
        return root.reportValue("mantle_metrics");
    }

    function reportValue(key) {
        const report = nodeReport()[key];
        return report && report.value ? report.value : {};
    }

    function networkValue(key) {
        const value = networkInfo()[key];
        return value === undefined || value === null ? null : value;
    }

    function mantleValue(key) {
        const value = mantleInfo()[key];
        return value === undefined || value === null ? null : value;
    }

    function modeText() {
        const mode = consensusValue().mode;
        if (typeof mode === "string") {
            return mode;
        }
        if (mode && mode.Started) {
            return mode.Started;
        }
        return "-";
    }

    function libDeltaText() {
        const slot = cryptarchiaValue("slot");
        const lib = cryptarchiaValue("lib_slot");
        if (slot === null || lib === null) {
            return qsTr("Above LIB");
        }
        return qsTr("+%1 above LIB").arg(root.numberText(slot - lib));
    }

    function numberText(value) {
        if (value === undefined || value === null || value === "") {
            return "-";
        }
        if (typeof value === "number") {
            return value.toLocaleString(Qt.locale());
        }
        return String(value);
    }

    function shortHash(value) {
        const text = String(value || "");
        if (text.length <= 16) {
            return text.length ? text : "-";
        }
        return text.slice(0, 8) + "..." + text.slice(-6);
    }

    function blockRows() {
        const blocks = model.dashboardBlocks || [];
        if (blocks.length > 0) {
            return blocks.slice(0, 10).map(function (block) {
                return {
                    slot: root.numberText(block.block_id),
                    header: root.shortHash(block.header_hash),
                    tx: root.numberText(block.tx_count),
                    status: block.bedrock_status || "-"
                };
            });
        }
        return [
            {
                slot: root.numberText(root.cryptarchiaValue("slot")),
                header: root.shortHash(root.cryptarchiaValue("tip")),
                tx: "-",
                status: qsTr("Tip")
            },
            {
                slot: root.numberText(root.cryptarchiaValue("lib_slot")),
                header: root.shortHash(root.cryptarchiaValue("lib")),
                tx: "-",
                status: qsTr("LIB")
            }
        ];
    }

    function transactionRows() {
        const rows = [];
        const blocks = model.dashboardBlocks || [];
        for (let i = 0; i < blocks.length && rows.length < 8; ++i) {
            const block = blocks[i];
            const transactions = block.transactions || [];
            for (let j = 0; j < transactions.length && rows.length < 8; ++j) {
                const tx = transactions[j];
                rows.push({
                    slot: root.numberText(block.block_id),
                    hash: root.shortHash(tx.hash),
                    block: root.shortHash(block.header_hash),
                    ops: root.numberText((tx.instruction_data || []).length)
                });
            }
        }
        if (rows.length > 0) {
            return rows;
        }
        return [
            {
                slot: "-",
                hash: qsTr("No indexed transactions"),
                block: "-",
                ops: "-"
            }
        ];
    }

    component DashboardHeader: Item {
        id: headerRoot

        required property Theme theme
        property string title: ""
        property string action: ""

        Layout.fillWidth: true
        Layout.preferredHeight: 48

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            spacing: 10

            Text {
                text: headerRoot.title
                color: headerRoot.theme.text
                textFormat: Text.PlainText
                font.pixelSize: 15
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            Text {
                text: headerRoot.action
                color: headerRoot.theme.accent
                textFormat: Text.PlainText
                font.pixelSize: 12
                font.weight: Font.Medium
            }
        }
    }

    component DashboardRow: Item {
        id: rowRoot

        required property Theme theme
        property var columns: []
        property bool header: false

        Layout.fillWidth: true
        Layout.preferredHeight: rowRoot.header ? 34 : 38

        Rectangle {
            anchors.fill: parent
            color: rowRoot.header ? rowRoot.theme.field : "transparent"
            border.width: 0
        }

        GridLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            columns: 4
            columnSpacing: 10

            Repeater {
                model: 4

                Text {
                    required property int index

                    text: String(rowRoot.columns[index] || "-")
                    color: rowRoot.header ? rowRoot.theme.textMuted : rowRoot.theme.text
                    textFormat: Text.PlainText
                    font.family: rowRoot.header ? "" : "monospace"
                    font.pixelSize: rowRoot.header ? 11 : 12
                    font.weight: rowRoot.header ? Font.DemiBold : Font.Normal
                    font.capitalization: rowRoot.header ? Font.AllUppercase : Font.MixedCase
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }
        }
    }

    component DashboardDetail: Item {
        id: detailRoot

        required property Theme theme
        property string label: ""
        property string value: ""
        property string status: ""

        Layout.fillWidth: true
        Layout.preferredHeight: 42

        GridLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            columns: 3
            columnSpacing: 12

            Text {
                text: detailRoot.label
                color: detailRoot.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: 11
                font.capitalization: Font.AllUppercase
                Layout.preferredWidth: 130
            }

            Text {
                text: detailRoot.value
                color: detailRoot.theme.text
                textFormat: Text.PlainText
                font.family: "monospace"
                font.pixelSize: 12
                elide: Text.ElideRight
                Layout.fillWidth: true
            }

            Text {
                text: detailRoot.status
                color: detailRoot.status === "ok" ? detailRoot.theme.success : detailRoot.theme.warning
                textFormat: Text.PlainText
                font.pixelSize: 12
                font.weight: Font.DemiBold
                horizontalAlignment: Text.AlignRight
                Layout.preferredWidth: 64
            }
        }
    }
}
