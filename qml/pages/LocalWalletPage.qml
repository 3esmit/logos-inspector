pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQml.Models
import QtQuick.Layouts
import "../components"
import "../components/common"
import "../state"
import "../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model

    width: parent ? parent.width : 900
    spacing: 16

    ListModel {
        id: walletTabs

        ListElement { value: "profiles"; label: "Profiles" }
        ListElement { value: "privateSync"; label: "Private Sync" }
        ListElement { value: "bedrockNotes"; label: "Bedrock Notes" }
        ListElement { value: "operations"; label: "Operations" }
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Local / Wallet")
        title: qsTr("Local Wallet")
        layerLabel: qsTr("Local")
        subtitle: qsTr("Explicit local wallet profile, private sync status, and Bedrock wallet note probes.")
        Layout.fillWidth: true
    }

    TabSwitch {
        theme: root.theme
        current: root.model.localWalletTab
        options: walletTabs
        Layout.fillWidth: true
        onSelected: value => root.model.localWalletTab = value
    }

    Frame {
        id: walletStateShelf

        padding: root.theme.gap
        Layout.fillWidth: true

        background: Rectangle {
            color: root.theme.surface
            radius: root.theme.radius
            border.width: 1
            border.color: root.theme.outlineMuted
        }

        contentItem: ColumnLayout {
            spacing: root.theme.gapSmall

            GridLayout {
                columns: root.width < 760 ? 1 : 2
                columnSpacing: root.theme.gap
                rowSpacing: root.theme.gapSmall
                Layout.fillWidth: true

                SourceStrip {
                    theme: root.theme
                    sources: [qsTr("Local Wallet"), qsTr("Explicit Profile"), root.model.walletHomeSourceLabel()]
                    Layout.fillWidth: true
                }

                LinkCell {
                    visible: root.model.localWalletLookupTarget.length > 0
                    theme: root.theme
                    text: root.model.localWalletLookupTarget
                    copyable: true
                    copyText: root.model.localWalletLookupTarget
                    monospace: true
                    textColor: root.model.localWalletTab === "bedrockNotes" && root.model.bedrockWalletSourceConfigured() ? root.theme.text : (root.model.walletProfileUsable() ? root.theme.text : root.theme.warning)
                    Layout.fillWidth: true
                    Layout.alignment: root.width < 760 ? Qt.AlignLeft : Qt.AlignRight
                }
            }

            GridLayout {
                columns: root.width < 720 ? 2 : 4
                columnSpacing: root.theme.gapSmall
                rowSpacing: root.theme.gapSmall
                Layout.fillWidth: true

                StatusChip {
                    theme: root.theme
                    label: qsTr("Profile")
                    value: root.model.walletProfileUsable() ? root.shortText(root.model.walletProfileLabel, 22) : (root.model.localWalletTab === "bedrockNotes" && root.model.bedrockWalletSourceConfigured() ? qsTr("Bedrock") : qsTr("Required"))
                    tone: root.model.walletProfileUsable() || (root.model.localWalletTab === "bedrockNotes" && root.model.bedrockWalletSourceConfigured()) ? "success" : "warning"
                    compact: true
                    showIndicator: true
                    Layout.fillWidth: true
                }

                StatusChip {
                    theme: root.theme
                    label: qsTr("Check")
                    value: root.localStatusText()
                    detail: root.localStatusDetail()
                    tone: root.localStatusTone()
                    compact: true
                    showIndicator: true
                    Layout.fillWidth: true
                }

                StatusChip {
                    theme: root.theme
                    label: qsTr("Home")
                    value: root.shortText(root.model.walletHomeDisplayLabel(), 22)
                    detail: root.model.walletHomeDisplayLabel()
                    tone: "neutral"
                    compact: true
                    showIndicator: true
                    Layout.fillWidth: true
                }

                StatusChip {
                    theme: root.theme
                    label: qsTr("Bedrock")
                    value: root.model.bedrockWalletBalanceValue !== null ? qsTr("Loaded") : qsTr("Idle")
                    detail: root.shortText(root.model.nodeUrl, 42)
                    tone: root.model.bedrockWalletBalanceValue !== null ? "success" : "neutral"
                    compact: true
                    showIndicator: true
                    Layout.fillWidth: true
                }
            }
        }
    }

    StatusMessage {
        visible: root.model.localWalletTab !== "bedrockNotes" && !root.model.walletProfileUsable()
        theme: root.theme
        tone: "warning"
        title: qsTr("Local wallet profile required")
        message: qsTr("wallet:<id> uses explicit local wallet state. Indexer-derived transfers are under recipient:<id>.")
        Layout.fillWidth: true
    }

    Loader {
        active: true
        asynchronous: true
        sourceComponent: root.tabComponent(root.model.localWalletTab)
        Layout.fillWidth: true
    }

    Component {
        id: profilesTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Panel {
                theme: root.theme
                title: qsTr("Profile")

                ColumnLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Label")
                        value: root.model.walletProfileLabel
                        copyText: ""
                    }

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Wallet binary")
                        value: root.model.walletBinary.length ? root.model.walletBinaryDisplayLabel() : qsTr("Not set")
                        copyText: ""
                    }

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Wallet home")
                        value: root.model.walletHomeDisplayLabel()
                        copyText: ""
                    }

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Version")
                        value: root.model.localWalletStatus && root.model.localWalletStatus.version ? String(root.model.localWalletStatus.version) : "-"
                        copyText: root.model.localWalletStatus && root.model.localWalletStatus.version ? String(root.model.localWalletStatus.version) : ""
                    }

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Sequencer RPC")
                        value: root.endpointLabel(root.model.sequencerUrl)
                        copyText: root.model.sequencerUrl
                    }

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Indexer RPC")
                        value: root.endpointLabel(root.model.indexerUrl)
                        copyText: root.model.indexerUrl
                    }

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Bedrock node")
                        value: root.endpointLabel(root.model.nodeUrl)
                        copyText: root.model.nodeUrl
                    }
                }

                RowLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Open Settings")
                        primary: true
                        onClicked: root.model.openSettings("wallet", "")
                    }

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Autodetect")
                        onClicked: {
                            root.model.detectWalletProfile(true)
                            root.model.checkLocalWalletProfile(false)
                        }
                    }

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Check")
                        onClicked: root.model.checkLocalWalletProfile(false)
                    }

                    Item {
                        Layout.fillWidth: true
                    }
                }
            }

            StatusMessage {
                visible: root.model.localWalletStatusError.length > 0
                theme: root.theme
                tone: "error"
                title: qsTr("Profile check failed")
                message: root.model.localWalletStatusError
                Layout.fillWidth: true
            }
        }
    }

    Component {
        id: lezAccountsTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            StatusMessage {
                theme: root.theme
                tone: "info"
                title: qsTr("LEZ accounts")
                message: qsTr("Public LEZ account lookup stays in Accounts. Local wallet account discovery waits for a stable wallet JSON source.")
                Layout.fillWidth: true
            }

            Panel {
                theme: root.theme
                title: qsTr("Lookup")

                ColumnLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    CopyRow {
                        theme: root.theme
                        label: qsTr("L2 Accounts")
                        value: qsTr("Accounts")
                        copyText: ""
                    }

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Open Accounts")
                        onClicked: root.model.selectView("accounts")
                    }
                }
            }
        }
    }

    Component {
        id: privateSyncTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            StatusMessage {
                theme: root.theme
                tone: "warning"
                title: qsTr("Manual sync")
                message: qsTr("Private wallet sync is not run automatically. Configure the profile before launching external sync commands.")
                Layout.fillWidth: true
            }

            Panel {
                theme: root.theme
                title: qsTr("State")

                ColumnLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Private context")
                        value: root.model.localWalletLookupTarget.length ? root.model.localWalletLookupTarget : "-"
                        copyText: root.model.localWalletLookupTarget
                    }

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Wallet home")
                        value: root.model.walletHomeDisplayLabel()
                        copyText: ""
                    }
                }
            }
        }
    }

    Component {
        id: bedrockNotesTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Panel {
                theme: root.theme
                title: qsTr("Bedrock Wallet")

                GridLayout {
                    columns: root.width < 760 ? 1 : 3
                    columnSpacing: root.theme.gap
                    rowSpacing: root.theme.gap
                    Layout.fillWidth: true

                    FieldRow {
                        theme: root.theme
                        label: qsTr("Public key")
                        sourceText: root.model.walletPublicKeyProbe
                        syncSourceText: true
                        placeholderText: qsTr("64 hex characters")
                        Layout.columnSpan: root.width < 760 ? 1 : 2
                        onTextEdited: text => { if (root.model.walletPublicKeyProbe !== text) root.model.walletPublicKeyProbe = text }
                    }

                    FieldRow {
                        theme: root.theme
                        label: qsTr("Tip")
                        sourceText: root.model.bedrockWalletBalanceTip
                        syncSourceText: true
                        placeholderText: qsTr("Optional 64-hex header id")
                        onTextEdited: text => { if (root.model.bedrockWalletBalanceTip !== text) root.model.bedrockWalletBalanceTip = text }
                    }
                }

                RowLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Save")
                        onClicked: root.model.saveWalletState()
                    }

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Query Balance")
                        primary: true
                        onClicked: {
                            root.model.saveWalletState()
                            root.model.queryBedrockWalletBalance()
                        }
                    }

                    Item {
                        Layout.fillWidth: true
                    }
                }
            }

            StatusMessage {
                visible: root.model.bedrockWalletBalanceError.length > 0
                theme: root.theme
                tone: "error"
                title: qsTr("Balance query failed")
                message: root.model.bedrockWalletBalanceError
                Layout.fillWidth: true
            }

            Panel {
                visible: root.model.bedrockWalletBalanceValue !== null
                theme: root.theme
                title: qsTr("Balance")

                ColumnLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    CopyRow {
                        theme: root.theme
                        label: qsTr("Public key")
                        value: root.model.walletPublicKeyProbe
                        copyText: root.model.walletPublicKeyProbe
                    }

                    Text {
                        text: root.balanceJson()
                        color: root.theme.text
                        textFormat: Text.PlainText
                        wrapMode: Text.WrapAnywhere
                        font.family: "monospace"
                        font.pixelSize: root.theme.dataText
                        Layout.fillWidth: true
                    }
                }
            }
        }
    }

    Component {
        id: operationsTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Panel {
                theme: root.theme
                title: qsTr("Recent Operations")

                ColumnLayout {
                    spacing: 0
                    Layout.fillWidth: true

                    OperationRow {
                        theme: root.theme
                        header: true
                        columns: [qsTr("Time"), qsTr("Operation"), qsTr("Status"), qsTr("Detail")]
                    }

                    Repeater {
                        model: root.operationRows()

                        OperationRow {
                            required property var modelData

                            theme: root.theme
                            columns: [modelData.time, modelData.label, modelData.status, modelData.detail]
                            status: modelData.status
                        }
                    }
                }
            }
        }
    }

    function tabComponent(tab) {
        switch (String(tab || "")) {
        case "privateSync":
            return privateSyncTab
        case "bedrockNotes":
            return bedrockNotesTab
        case "operations":
            return operationsTab
        default:
            return profilesTab
        }
    }

    function localStatusText() {
        const status = root.model.localWalletStatus || null
        if (!status) {
            return root.model.localWalletStatusError.length ? qsTr("Down") : qsTr("Unknown")
        }
        const value = String(status.status || "unknown")
        return value.length ? value[0].toUpperCase() + value.slice(1) : qsTr("Unknown")
    }

    function localStatusDetail() {
        const status = root.model.localWalletStatus || null
        if (root.model.localWalletStatusError.length) {
            return root.shortText(root.model.localWalletStatusError, 36)
        }
        if (status && status.detail) {
            return root.shortText(status.detail, 36)
        }
        return qsTr("Not checked")
    }

    function localStatusTone() {
        const status = root.model.localWalletStatus || null
        const value = status && status.status ? String(status.status) : ""
        if (root.model.localWalletStatusError.length || value === "down") {
            return "error"
        }
        if (!value.length || value === "degraded" || value === "unknown") {
            return "warning"
        }
        if (value === "ok") {
            return "success"
        }
        return "neutral"
    }

    function balanceJson() {
        try {
            return JSON.stringify(root.model.bedrockWalletBalanceValue, null, 2)
        } catch (error) {
            return String(root.model.bedrockWalletBalanceValue || "")
        }
    }

    function operationRows() {
        const rows = Array.isArray(root.model.localWalletOperations) ? root.model.localWalletOperations.slice() : []
        if (!rows.length) {
            return [{ time: "-", label: qsTr("No operations"), status: "-", detail: "-" }]
        }
        rows.reverse()
        return rows
    }

    function shortText(value, limit) {
        const text = String(value || "")
        const max = Math.max(8, Number(limit || 24))
        if (text.length <= max) {
            return text.length ? text : "-"
        }
        return text.slice(0, Math.max(4, max - 9)) + "..." + text.slice(-6)
    }

    function endpointLabel(value) {
        const text = String(value || "").trim()
        if (!text.length) {
            return qsTr("Not configured")
        }
        return text.replace(/^https?:\/\//, "").replace(/\/$/, "")
    }

    component CopyRow: GridLayout {
        id: copyRoot

        required property Theme theme
        property string label: ""
        property string value: "-"
        property string copyText: value

        columns: 2
        columnSpacing: copyRoot.theme.gap
        rowSpacing: copyRoot.theme.gapTiny
        Layout.fillWidth: true

        Text {
            text: copyRoot.label
            color: copyRoot.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: copyRoot.theme.secondaryText
            font.weight: Font.Medium
            Layout.preferredWidth: 132
        }

        LinkCell {
            theme: copyRoot.theme
            text: copyRoot.value
            copyable: copyRoot.copyText.length > 0
            copyText: copyRoot.copyText
            monospace: true
            wrap: true
            Layout.fillWidth: true
        }
    }

    component OperationRow: Item {
        id: rowRoot

        required property Theme theme
        property var columns: []
        property string status: ""
        property bool header: false

        Layout.fillWidth: true
        Layout.preferredHeight: rowRoot.header ? 34 : 40

        Rectangle {
            anchors.fill: parent
            color: rowRoot.header ? rowRoot.theme.field : "transparent"
            border.width: 0
        }

        GridLayout {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            columns: 4
            columnSpacing: 10

            Repeater {
                model: 4

                Text {
                    required property int index

                    text: String(rowRoot.columns[index] || "-")
                    color: rowRoot.textColor(index)
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    font.family: rowRoot.header ? "" : "monospace"
                    font.pixelSize: rowRoot.header ? rowRoot.theme.labelText : rowRoot.theme.dataText
                    font.weight: rowRoot.header ? Font.DemiBold : Font.Normal
                    font.capitalization: rowRoot.header ? Font.AllUppercase : Font.MixedCase
                    Layout.preferredWidth: rowRoot.columnWidth(index)
                    Layout.fillWidth: index === 3
                }
            }
        }

        function textColor(index) {
            if (rowRoot.header) {
                return rowRoot.theme.textMuted
            }
            if (index === 2) {
                if (rowRoot.status === "ok") {
                    return rowRoot.theme.success
                }
                if (rowRoot.status === "down") {
                    return rowRoot.theme.error
                }
                if (rowRoot.status === "degraded" || rowRoot.status === "unknown") {
                    return rowRoot.theme.warning
                }
            }
            return rowRoot.theme.text
        }

        function columnWidth(index) {
            if (index === 0) {
                return 88
            }
            if (index === 1) {
                return 150
            }
            if (index === 2) {
                return 90
            }
            return 260
        }
    }
}
