pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../state"

Panel {
    id: root

    required property AppModel model
    readonly property bool structured: !root.model.resultIsError && root.hasStructuredDetail(root.model.resultValue)

    title: root.model.resultTitle.length ? root.model.resultTitle : qsTr("Output")

    RowLayout {
        spacing: 8
        Layout.fillWidth: true

        Text {
            text: root.model.resultIsError ? qsTr("Error") : qsTr("Output")
            color: root.model.resultIsError ? root.theme.error : root.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: 13
            font.weight: Font.Medium
            Layout.fillWidth: true
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Clear")
            enabled: root.model.resultText.length > 0 || root.model.resultValue !== null
            Layout.preferredWidth: 84
            onClicked: root.model.clearResult()
        }
    }

    Loader {
        active: root.structured
        sourceComponent: root.detailComponent(root.model.resultValue)
        Layout.fillWidth: true
    }

    TextArea {
        visible: !root.structured || root.model.resultIsError
        readOnly: true
        text: root.model.resultText.length ? root.model.resultText : qsTr("Run an inspection to see structured output.")
        wrapMode: TextArea.Wrap
        color: root.model.resultText.length ? root.theme.text : root.theme.textMuted
        selectedTextColor: "#21160F"
        selectionColor: root.theme.accent
        textFormat: Text.PlainText
        font.family: "monospace"
        font.pixelSize: 13
        leftPadding: 12
        rightPadding: 12
        topPadding: 10
        bottomPadding: 10
        Layout.fillWidth: true
        Layout.preferredHeight: 260

        background: Rectangle {
            color: root.model.resultIsError ? "#2D1917" : root.theme.field
            radius: root.theme.radius
            border.width: 1
            border.color: root.model.resultIsError ? root.theme.error : root.theme.outline
        }
    }

    Component {
        id: transactionDetail

        TransactionDetailPane {
            theme: root.theme
            model: root.model
            value: root.model.resultValue
        }
    }

    Component {
        id: blockDetail

        BlockDetailPane {
            theme: root.theme
            model: root.model
            value: root.model.resultValue
        }
    }

    Component {
        id: walletDetail

        WalletDetailPane {
            theme: root.theme
            model: root.model
            value: root.model.resultValue
        }
    }

    Component {
        id: channelDetail

        ChannelDetailPane {
            theme: root.theme
            model: root.model
            value: root.model.resultValue
        }
    }

    Component {
        id: accountDetail

        AccountDetailPane {
            theme: root.theme
            model: root.model
            value: root.model.resultValue
        }
    }

    function hasStructuredDetail(value) {
        return hasBlockDetail(value) || hasTransactionDetail(value) || hasWalletDetail(value) || hasChannelDetail(value) || hasAccountDetail(value)
    }

    function detailComponent(value) {
        if (hasBlockDetail(value)) {
            return blockDetail
        }
        if (hasWalletDetail(value)) {
            return walletDetail
        }
        if (hasChannelDetail(value)) {
            return channelDetail
        }
        if (hasAccountDetail(value)) {
            return accountDetail
        }
        return transactionDetail
    }

    function hasBlockDetail(value) {
        if (!value || typeof value !== "object" || Array.isArray(value)) {
            return false
        }
        return value.type === "blockchain_block" || value.type === "indexer_block"
    }

    function hasTransactionDetail(value) {
        if (!value || typeof value !== "object" || Array.isArray(value)) {
            return false
        }
        return value.type === "blockchain_transaction"
            || value.raw_summary !== undefined
            || value.inspection !== undefined
            || (value.hash !== undefined && value.kind !== undefined)
    }

    function hasWalletDetail(value) {
        if (!value || typeof value !== "object" || Array.isArray(value)) {
            return false
        }
        return value.type === "wallet"
    }

    function hasChannelDetail(value) {
        if (!value || typeof value !== "object" || Array.isArray(value)) {
            return false
        }
        return value.type === "channel"
    }

    function hasAccountDetail(value) {
        if (!value || typeof value !== "object" || Array.isArray(value)) {
            return false
        }
        return (value.account_id !== undefined && value.account !== undefined && value.data_hex !== undefined)
            || (value.account !== undefined && value.account.account_id !== undefined)
            || (value.account_type !== undefined && value.rows !== undefined && value.decoded !== undefined)
    }
}
