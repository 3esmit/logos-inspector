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
        if (!model.walletsPageRows.length) {
            model.refreshWalletsPage();
        }
    }

    RowLayout {
        spacing: 12
        Layout.fillWidth: true

        ColumnLayout {
            spacing: 6
            Layout.fillWidth: true

            Text {
                text: qsTr("Home > Wallets")
                color: root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: 12
                Layout.fillWidth: true
            }

            Text {
                text: qsTr("Wallets")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: 28
                font.weight: Font.Bold
                Layout.fillWidth: true
            }

            Text {
                text: qsTr("Ordered by total inbound transfer volume. Reconstructed from Transfer op outputs; the node API does not expose a wallet directory.")
                color: root.theme.textMuted
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
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
            onClicked: root.model.refreshWalletsPage()
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Next >")
            enabled: !root.model.busy && root.model.walletsPageNextBeforeBlock > 0
            Layout.preferredWidth: 104
            onClicked: root.model.nextWalletsPage()
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

            WalletRow {
                theme: root.theme
                header: true
                columns: [qsTr("Wallet"), qsTr("Received"), qsTr("Txs"), qsTr("Outputs"), qsTr("Last slot")]
            }

            Repeater {
                model: root.walletRows()

                WalletRow {
                    required property var modelData

                    theme: root.theme
                    columns: [modelData.wallet, modelData.received, modelData.txs, modelData.outputs, modelData.lastSlot]
                    wallet: modelData.walletRaw
                    onWalletActivated: root.model.openWallet(modelData.walletRaw)
                }
            }
        }
    }

    Text {
        visible: root.model.walletsPageError.length > 0
        text: root.model.walletsPageError
        color: root.theme.warning
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 12
        Layout.fillWidth: true
    }

    WalletDetailPane {
        value: root.model.walletDetailValue
        theme: root.theme
        model: root.model
    }

    Panel {
        visible: root.model.walletDetailValue === null
        theme: root.theme
        title: qsTr("Wallet detail")
        Layout.fillWidth: true

        Text {
            text: qsTr("Select a wallet to inspect receive-side transfer outputs, source transactions, and linked blocks.")
            color: root.theme.textMuted
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 14
            Layout.fillWidth: true
        }
    }

    function walletRows() {
        const wallets = root.model.walletsPageRows || [];
        if (!wallets.length) {
            return [{
                wallet: qsTr("No wallets in loaded range"),
                walletRaw: "",
                received: "-",
                txs: "-",
                outputs: "-",
                lastSlot: "-"
            }];
        }
        return wallets.map(function (wallet) {
            return {
                wallet: root.shortWallet(wallet.wallet),
                walletRaw: String(wallet.wallet || ""),
                received: root.receivedText(wallet),
                txs: root.numberText(wallet.txs),
                outputs: root.numberText(wallet.outputs),
                lastSlot: root.numberText(wallet.last_slot)
            };
        });
    }

    function receivedText(wallet) {
        if (wallet.received === undefined || wallet.received === null || wallet.received === "") {
            return "-";
        }
        return root.coinText(wallet.received);
    }

    function coinText(value) {
        const numeric = Number(value);
        if (Number.isFinite(numeric)) {
            return (numeric / 100).toLocaleString(Qt.locale(), "f", 2) + "E";
        }
        return String(value);
    }

    function numberText(value) {
        if (value === undefined || value === null || value === "") {
            return "-";
        }
        const numeric = Number(value);
        if (Number.isFinite(numeric)) {
            return numeric.toLocaleString(Qt.locale());
        }
        return String(value);
    }

    function shortWallet(value) {
        const text = String(value || "");
        if (text.length <= 18) {
            return text.length ? text : "-";
        }
        return text.slice(0, 12) + "..." + text.slice(-8);
    }

    component WalletRow: Item {
        id: rowRoot

        required property Theme theme
        property var columns: []
        property string wallet: ""
        property bool header: false
        signal walletActivated()

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
                        onClicked: rowRoot.walletActivated()
                    }
                }
            }
        }

        function linkFor(index) {
            return !rowRoot.header && index === 0 && rowRoot.wallet.length > 0;
        }

        function textColor(index) {
            if (rowRoot.linkFor(index)) {
                return rowRoot.theme.accent;
            }
            return rowRoot.header ? rowRoot.theme.textMuted : rowRoot.theme.text;
        }

        function columnWidth(index) {
            if (index === 0) {
                return 240;
            }
            if (index === 1) {
                return 120;
            }
            return 82;
        }
    }
}
