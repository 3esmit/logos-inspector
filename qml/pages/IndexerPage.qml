pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQml.Models
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

    ListModel {
        id: indexerTabs

        ListElement { value: "status"; label: "Dashboard" }
        ListElement { value: "rpc"; label: "RPC" }
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Diagnostics / LEZ Indexer")
        title: qsTr("LEZ Indexer Diagnostics")
        layerLabel: qsTr("Diagnostics")
        subtitle: qsTr("Probe local or remote indexer health, finalized head, and raw JSON-RPC methods.")
        Layout.fillWidth: true
    }

    GridLayout {
        columns: root.width < 760 ? 2 : 4
        columnSpacing: 12
        rowSpacing: 12
        Layout.fillWidth: true

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Endpoint")
            value: root.endpointLabel(root.model.indexerUrl)
            delta: root.shortEndpoint(root.model.indexerUrl)
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Profile")
            value: root.profileLabel(root.model.networkProfile)
            delta: qsTr("Network profile")
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Health")
            value: root.indexerHealthText()
            delta: root.indexerHealthDelta()
            deltaColor: root.indexerHealthColor()
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Finalized")
            value: root.indexerHeadText()
            delta: qsTr("Indexer head")
            deltaColor: root.indexerHeadText() !== "-" ? root.theme.success : root.theme.textMuted
        }
    }

    Panel {
        theme: root.theme
        title: root.model.indexerTab === "status" ? qsTr("Indexer status") : qsTr("Indexer JSON-RPC")

        TabSwitch {
            theme: root.theme
            current: root.model.indexerTab
            options: indexerTabs
            onSelected: value => root.model.indexerTab = value
        }

        Loader {
            active: true
            sourceComponent: root.model.indexerTab === "status" ? statusForm : rpcForm
            Layout.fillWidth: true
        }
    }

    Panel {
        visible: root.model.pageHasOutput("indexer")
        theme: root.theme
        title: root.model.resultIsError ? qsTr("Indexer error") : qsTr("Indexer response")

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Text {
                text: root.model.resultTitle
                color: root.model.resultIsError ? root.theme.error : root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
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

        StatusMessage {
            visible: root.model.resultIsError
            theme: root.theme
            tone: "warning"
            title: qsTr("Call failed")
            message: root.model.resultText
            Layout.fillWidth: true
        }

        GridLayout {
            visible: !root.model.resultIsError
            columns: root.width < 760 ? 2 : 4
            columnSpacing: 12
            rowSpacing: 12
            Layout.fillWidth: true

            MetricCard {
                theme: root.theme
                compact: true
                label: qsTr("Status")
                value: root.responseStatusText()
                delta: root.responseSourceText()
                deltaColor: root.responseStatusColor()
            }

            MetricCard {
                theme: root.theme
                compact: true
                label: qsTr("Head")
                value: root.responseHeadText()
                delta: qsTr("Finalized block")
                deltaColor: root.responseHeadText() !== "-" ? root.theme.success : root.theme.textMuted
            }

            MetricCard {
                theme: root.theme
                compact: true
                label: qsTr("Payload")
                value: root.responsePayloadText()
                delta: root.responseKindText()
            }

            MetricCard {
                theme: root.theme
                compact: true
                label: qsTr("Endpoint")
                value: root.endpointLabel(root.responseEndpoint())
                delta: root.shortEndpoint(root.responseEndpoint())
            }
        }

        TextArea {
            readOnly: true
            text: root.model.resultText.length ? root.model.resultText : qsTr("No response body.")
            wrapMode: TextArea.Wrap
            color: root.model.resultText.length ? root.theme.text : root.theme.textMuted
            selectedTextColor: root.theme.selectedText
            selectionColor: root.theme.accent
            textFormat: Text.PlainText
            font.family: "monospace"
            font.pixelSize: root.theme.secondaryText
            leftPadding: 12
            rightPadding: 12
            topPadding: 10
            bottomPadding: 10
            Layout.fillWidth: true
            Layout.preferredHeight: root.model.resultIsError ? 120 : 220

            background: Rectangle {
                color: root.model.resultIsError ? root.theme.errorMuted : root.theme.field
                radius: root.theme.radius
                border.width: 1
                border.color: root.model.resultIsError ? root.theme.error : root.theme.outline
            }
        }
    }

    Component {
        id: statusForm

        ColumnLayout {
            spacing: 12
            Layout.fillWidth: true

            StatusMessage {
                theme: root.theme
                tone: "info"
                title: qsTr("JSON-RPC POST")
                message: qsTr("Deep health calls checkHealth. Finalized head calls getLastFinalizedBlockId.")
                Layout.fillWidth: true
            }

            RowLayout {
                spacing: 10
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Deep health")
                    primary: true
                    enabled: !root.model.busy
                    Layout.preferredWidth: 132
                    accessibleName: qsTr("Run indexer deep health")
                    onClicked: root.model.callInspector("indexerHealth", [root.model.indexerUrl], qsTr("Indexer health"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Finalized head")
                    enabled: !root.model.busy
                    Layout.preferredWidth: 148
                    accessibleName: qsTr("Fetch indexer finalized head")
                    onClicked: root.model.callInspector("indexerFinalizedHead", [root.model.indexerUrl], qsTr("Indexer head"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Overview")
                    enabled: !root.model.busy
                    Layout.preferredWidth: 112
                    accessibleName: qsTr("Run indexer overview")
                    onClicked: root.model.callInspector("overview", [root.model.sequencerUrl, root.model.indexerUrl, root.model.nodeUrl], qsTr("Indexer dashboard"))
                }
            }
        }
    }

    Component {
        id: rpcForm

        ColumnLayout {
            spacing: 12

            FieldRow {
                id: method
                theme: root.theme
                label: qsTr("Method")
                text: "getLastFinalizedBlockId"
                placeholderText: qsTr("JSON-RPC method")
            }

            TextAreaField {
                id: params
                theme: root.theme
                label: qsTr("Params JSON")
                text: "[]"
                rows: 4
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Call indexer")
                primary: true
                enabled: !root.model.busy && method.text.trim().length > 0 && params.text.trim().length > 0
                Layout.preferredWidth: 132
                accessibleName: qsTr("Call indexer JSON-RPC")
                onClicked: root.model.callInspector("rawRpc", [root.model.indexerUrl, method.text, params.text], qsTr("Indexer RPC"))
            }
        }
    }

    function activeValue() {
        return root.model.pageHasOutput("indexer") ? root.model.resultValue : null
    }

    function activeIndexerProbe() {
        const value = root.activeValue()
        if (value && typeof value === "object" && !Array.isArray(value) && value.indexer !== undefined) {
            return value.indexer
        }
        const overview = root.model.dashboardOverview
        if (overview && overview.indexer !== undefined) {
            return overview.indexer
        }
        return null
    }

    function indexerHealthText() {
        const probe = root.activeIndexerProbe()
        const value = root.activeValue()
        if (probe && probe.health) {
            return probe.health.ok ? qsTr("Healthy") : qsTr("Error")
        }
        if (value && typeof value === "object" && !Array.isArray(value) && value.status !== undefined) {
            const status = root.valueText(value.status)
            return status === "healthy" ? qsTr("Healthy") : status
        }
        if (root.model.pageHasOutput("indexer") && root.model.resultIsError) {
            return qsTr("Error")
        }
        return qsTr("Unknown")
    }

    function indexerHealthDelta() {
        const probe = root.activeIndexerProbe()
        if (probe && probe.health && !probe.health.ok) {
            return root.valueText(probe.health.error)
        }
        if (root.model.pageHasOutput("indexer") && root.model.resultIsError) {
            return root.model.resultText
        }
        return qsTr("checkHealth")
    }

    function indexerHealthColor() {
        const text = root.indexerHealthText()
        if (text === qsTr("Healthy") || text === "healthy") {
            return root.theme.success
        }
        if (text === qsTr("Error")) {
            return root.theme.warning
        }
        return root.theme.textMuted
    }

    function indexerHeadText() {
        const probe = root.activeIndexerProbe()
        const value = root.activeValue()
        if (probe && probe.head) {
            return root.valueText(probe.head.value)
        }
        if (root.model.pageHasOutput("indexer") && root.model.resultTitle === qsTr("Indexer head")) {
            return root.valueText(value)
        }
        if (value && typeof value === "object" && !Array.isArray(value) && value.head !== undefined) {
            return root.valueText(value.head)
        }
        return "-"
    }

    function responseStatusText() {
        const probe = root.responseProbe()
        const value = root.activeValue()
        if (probe && probe.health) {
            return probe.health.ok ? qsTr("Reachable") : qsTr("Error")
        }
        if (value && typeof value === "object" && !Array.isArray(value) && value.status !== undefined) {
            return root.valueText(value.status)
        }
        return qsTr("OK")
    }

    function responseHeadText() {
        const probe = root.responseProbe()
        const value = root.activeValue()
        if (probe && probe.head) {
            return root.valueText(probe.head.value)
        }
        if (value && typeof value === "object" && !Array.isArray(value) && value.head !== undefined) {
            return root.valueText(value.head)
        }
        if (root.model.resultTitle === qsTr("Indexer head")) {
            return root.valueText(value)
        }
        return "-"
    }

    function responseStatusColor() {
        const status = root.responseStatusText()
        if (status === qsTr("Reachable") || status === "reachable" || status === qsTr("OK")) {
            return root.theme.success
        }
        if (status === qsTr("Error")) {
            return root.theme.warning
        }
        return root.theme.textMuted
    }

    function responseSourceText() {
        return root.model.resultTitle.length ? root.model.resultTitle : qsTr("Indexer call")
    }

    function responsePayloadText() {
        const value = root.activeValue()
        if (value === null || value === undefined) {
            return "-"
        }
        if (Array.isArray(value)) {
            return root.numberText(value.length)
        }
        if (typeof value === "object") {
            return root.numberText(Object.keys(value).length)
        }
        return root.valueText(value)
    }

    function responseKindText() {
        const value = root.activeValue()
        if (Array.isArray(value)) {
            return qsTr("Array items")
        }
        if (value && typeof value === "object") {
            return qsTr("Object fields")
        }
        return qsTr("Scalar value")
    }

    function responseProbe() {
        const value = root.activeValue()
        if (value && typeof value === "object" && !Array.isArray(value) && value.indexer !== undefined) {
            return value.indexer
        }
        return null
    }

    function responseEndpoint() {
        const probe = root.responseProbe()
        if (probe && probe.endpoint !== undefined) {
            return String(probe.endpoint || "")
        }
        return root.model.indexerUrl
    }

    function endpointLabel(value) {
        const text = String(value || "")
        if (!text.length) {
            return "-"
        }
        if (text.indexOf("127.0.0.1") >= 0 || text.indexOf("localhost") >= 0) {
            return qsTr("Local")
        }
        if (text.indexOf("testnet") >= 0) {
            return qsTr("Testnet")
        }
        return qsTr("Custom")
    }

    function shortEndpoint(value) {
        const text = String(value || "")
        if (!text.length) {
            return qsTr("Not configured")
        }
        return text.replace(/^https?:\/\//, "").replace(/\/$/, "")
    }

    function profileLabel(value) {
        if (value === "local") {
            return qsTr("Local")
        }
        return qsTr("Default")
    }

    function valueText(value) {
        if (value === undefined || value === null || value === "") {
            return "-"
        }
        if (typeof value === "number") {
            return value % 1 === 0 ? value.toLocaleString(Qt.locale(), "f", 0) : String(value)
        }
        if (typeof value === "object") {
            return JSON.stringify(value)
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
}
