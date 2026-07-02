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

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Dashboard")
        title: qsTr("Dashboard")
        subtitle: qsTr("%1 profile across %2 network. Open blocks, transactions, wallets, channels, and accounts from live chain references.").arg(root.profileLabel(root.model.networkProfile)).arg(root.chainLabel())
        Layout.fillWidth: true

        ActionButton {
            theme: root.theme
            text: qsTr("Refresh")
            primary: true
            enabled: !root.model.busy
            Layout.preferredWidth: 116
            onClicked: root.model.refreshDashboard()
        }
    }

    Panel {
        theme: root.theme
        title: qsTr("Network status")

        GridLayout {
            columns: root.width < 760 ? 1 : 3
            columnSpacing: root.theme.gapSmall
            rowSpacing: root.theme.gapSmall
            Layout.fillWidth: true

            ServiceChip {
                theme: root.theme
                label: qsTr("Sequencer")
                status: root.serviceStatus("sequencer", "health")
                ok: root.serviceOk("sequencer", "health")
                targetView: "sequencer"
                onActivated: root.model.selectView("sequencer")
                Layout.fillWidth: true
            }

            ServiceChip {
                theme: root.theme
                label: qsTr("Indexer")
                status: root.serviceStatus("indexer", "health")
                ok: root.serviceOk("indexer", "health")
                targetView: "indexer"
                onActivated: root.model.selectView("indexer")
                Layout.fillWidth: true
            }

            ServiceChip {
                theme: root.theme
                label: qsTr("Blockchain")
                status: root.serviceStatus("node", "consensus")
                ok: root.serviceOk("node", "consensus")
                targetView: "blockchain"
                onActivated: root.model.selectView("blockchain")
                Layout.fillWidth: true
            }
        }

        StatusMessage {
            theme: root.theme
            tone: "info"
            title: qsTr("Global lookup")
            message: qsTr("Use the top lookup field to open hashes, block numbers, accounts, channels, wallets, or app pages from anywhere.")
            Layout.fillWidth: true
        }
    }

    GridLayout {
        columns: root.width < 720 ? 2 : (root.width < 1180 ? 3 : 6)
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gap
        Layout.fillWidth: true

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Tip slot")
            value: root.numberText(root.cryptarchiaValue("slot"))
            delta: qsTr("Latest")
            deltaColor: root.theme.accent
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Tip height")
            value: root.numberText(root.cryptarchiaValue("height"))
            delta: root.libDeltaText()
            deltaColor: root.theme.success
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("LIB slot")
            value: root.numberText(root.cryptarchiaValue("lib_slot"))
            delta: qsTr("Finalized")
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Mode")
            value: root.modeText()
            delta: qsTr("Cryptarchia")
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Peers")
            value: root.numberText(root.networkValue("n_peers"))
            delta: qsTr("%1 connections").arg(root.numberText(root.networkValue("n_connections")))
            deltaColor: root.theme.accent
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Mantle pending")
            value: root.numberText(root.mantleValue("pending_items"))
            delta: qsTr("Mempool")
        }
    }

    GridLayout {
        columns: root.width < 1040 ? 1 : 2
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gap
        Layout.fillWidth: true

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
                    action: qsTr("View all")
                    onActivated: root.model.selectView("blocks")
                }

                DashboardRow {
                    theme: root.theme
                    header: true
                    columns: [qsTr("Slot"), qsTr("Header"), qsTr("Tx"), qsTr("Status")]
                    columnWidths: [86, -1, 58, 86]
                }

                Repeater {
                    model: root.blockRows()

                    DashboardRow {
                        required property var modelData

                        theme: root.theme
                        columns: [modelData.slot, modelData.header, modelData.tx, modelData.status]
                        columnWidths: [86, -1, 58, 86]
                        linkKinds: ["indexerBlock", "indexerBlock", "", ""]
                        linkValues: [modelData.blockHash, modelData.blockHash, "", ""]
                        onCellActivated: function (column) {
                            root.model.openReference(column === 0 || column === 1 ? "indexerBlock" : "", modelData.blockHash)
                        }
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
                    title: qsTr("Latest Transactions")
                    action: qsTr("View all")
                    onActivated: root.model.selectView("transactions")
                }

                DashboardRow {
                    theme: root.theme
                    header: true
                    columns: [qsTr("Slot"), qsTr("Tx hash"), qsTr("Block"), qsTr("Ops")]
                    columnWidths: [86, -1, -1, 58]
                }

                Repeater {
                    model: root.transactionRows()

                    DashboardRow {
                        required property var modelData

                        theme: root.theme
                        columns: [modelData.slot, modelData.hash, modelData.block, modelData.ops]
                        columnWidths: [86, -1, -1, 58]
                        linkKinds: ["", "transaction", "indexerBlock", ""]
                        linkValues: ["", modelData.txHash, modelData.blockHash, ""]
                        onCellActivated: function (column) {
                            if (column === 1) {
                                root.model.openReference("transaction", modelData.txHash)
                            } else if (column === 2) {
                                root.model.openReference("indexerBlock", modelData.blockHash)
                            }
                        }
                    }
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
                action: qsTr("Settings")
                onActivated: root.model.selectView("settings")
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

    StatusMessage {
        visible: root.model.dashboardError.length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Dashboard refresh failed")
        message: root.model.dashboardError
        Layout.fillWidth: true
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
        if (model.networkProfile === "local" || model.networkProfile === "local-node") {
            return qsTr("Local");
        }
        if (model.networkProfile === "custom") {
            return qsTr("Custom");
        }
        return qsTr("Testnet");
    }

    function profileLabel(value) {
        if (value === "local") {
            return qsTr("Local");
        }
        if (value === "local-node") {
            return qsTr("Local node");
        }
        if (value === "testnet-indexer-local") {
            return qsTr("Mixed");
        }
        if (value === "custom") {
            return qsTr("Custom");
        }
        return qsTr("Testnet");
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
            return value.toLocaleString(Qt.locale(), "f", 0);
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
            return blocks.slice(0, 5).map(function (block) {
                return {
                    slot: root.numberText(block.block_id),
                    slotRaw: String(block.block_id || ""),
                    header: root.shortHash(block.header_hash),
                    tx: root.numberText(block.tx_count),
                    status: block.bedrock_status || "-",
                    blockHash: String(block.header_hash || "")
                };
            });
        }
        return [
            {
                slot: root.numberText(root.cryptarchiaValue("slot")),
                header: root.shortHash(root.cryptarchiaValue("tip")),
                tx: "-",
                status: qsTr("Tip"),
                blockHash: String(root.cryptarchiaValue("tip") || "")
            },
            {
                slot: root.numberText(root.cryptarchiaValue("lib_slot")),
                header: root.shortHash(root.cryptarchiaValue("lib")),
                tx: "-",
                status: qsTr("LIB"),
                blockHash: String(root.cryptarchiaValue("lib") || "")
            }
        ];
    }

    function transactionRows() {
        const rows = [];
        const blocks = model.dashboardBlocks || [];
        for (let i = 0; i < blocks.length && rows.length < 5; ++i) {
            const block = blocks[i];
            const transactions = block.transactions || [];
            for (let j = 0; j < transactions.length && rows.length < 5; ++j) {
                const tx = transactions[j];
                rows.push({
                    slot: root.numberText(block.block_id),
                    hash: root.shortHash(tx.hash),
                    block: root.shortHash(block.header_hash),
                    ops: root.numberText((tx.instruction_data || []).length),
                    txHash: String(tx.hash || ""),
                    blockHash: String(block.header_hash || "")
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
                ops: "-",
                txHash: "",
                blockHash: ""
            }
        ];
    }

    component ServiceChip: Rectangle {
        id: chipRoot

        required property Theme theme
        property string label: ""
        property string status: ""
        property bool ok: false
        property string targetView: ""
        signal activated()

        color: chipRoot.ok ? chipRoot.theme.successMuted : chipRoot.theme.warningMuted
        radius: chipRoot.theme.radius
        border.width: chipRoot.activeFocus ? 2 : 1
        border.color: chipRoot.activeFocus ? chipRoot.theme.accent : (chipRoot.ok ? chipRoot.theme.success : chipRoot.theme.warning)
        activeFocusOnTab: chipRoot.targetView.length > 0
        Layout.preferredHeight: 34

        Keys.onPressed: function (event) {
            if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                chipRoot.activated();
                event.accepted = true;
            }
        }

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: chipRoot.theme.gapSmall
            anchors.rightMargin: chipRoot.theme.gapSmall
            spacing: chipRoot.theme.gapSmall

            Rectangle {
                color: chipRoot.ok ? chipRoot.theme.success : chipRoot.theme.warning
                radius: 4
                Layout.preferredWidth: 8
                Layout.preferredHeight: 8
                Layout.alignment: Qt.AlignVCenter
                Accessible.ignored: true
            }

            Text {
                text: chipRoot.label
                color: chipRoot.theme.text
                textFormat: Text.PlainText
                font.pixelSize: chipRoot.theme.secondaryText
                font.weight: Font.DemiBold
                elide: Text.ElideRight
                Layout.fillWidth: true
            }

            Text {
                text: chipRoot.status
                color: chipRoot.ok ? chipRoot.theme.success : chipRoot.theme.warning
                textFormat: Text.PlainText
                font.pixelSize: chipRoot.theme.labelText
                font.weight: Font.DemiBold
                font.capitalization: Font.AllUppercase
            }
        }

        MouseArea {
            anchors.fill: parent
            enabled: chipRoot.targetView.length > 0
            cursorShape: Qt.PointingHandCursor
            onClicked: chipRoot.activated()
        }

        Accessible.role: chipRoot.targetView.length > 0 ? Accessible.Button : Accessible.StaticText
        Accessible.name: qsTr("%1 status %2").arg(chipRoot.label).arg(chipRoot.status)
    }

    component DashboardHeader: Item {
        id: headerRoot

        required property Theme theme
        property string title: ""
        property string action: ""
        signal activated()

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

            ActionButton {
                visible: headerRoot.action.length > 0
                theme: headerRoot.theme
                text: headerRoot.action
                Layout.preferredWidth: Math.max(96, headerRoot.action.length * 8 + 28)
                onClicked: headerRoot.activated()
            }
        }
    }

    component DashboardRow: Item {
        id: rowRoot

        required property Theme theme
        property var columns: []
        property var columnWidths: [-1, -1, -1, -1]
        property var linkKinds: ["", "", "", ""]
        property var linkValues: ["", "", "", ""]
        property bool header: false
        signal cellActivated(int column)

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

                LinkCell {
                    required property int index

                    theme: rowRoot.theme
                    text: String(rowRoot.columns[index] || "-")
                    header: rowRoot.header
                    link: rowRoot.linkFor(index)
                    monospace: !rowRoot.header
                    Layout.preferredWidth: rowRoot.columnWidth(index)
                    Layout.fillWidth: rowRoot.columnFills(index)
                    onActivated: rowRoot.cellActivated(index)
                }
            }
        }

        function linkFor(index) {
            return !rowRoot.header
                && index >= 0
                && index < rowRoot.linkKinds.length
                && String(rowRoot.linkKinds[index] || "").length > 0
                && String(rowRoot.linkValues[index] || "").length > 0
        }

        function columnWidth(index) {
            const value = Number(rowRoot.columnWidths[index] || -1)
            return value > 0 ? value : 120
        }

        function columnFills(index) {
            return Number(rowRoot.columnWidths[index] || -1) <= 0
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
