pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQml.Models
import QtQuick.Layouts
import "../components"
import "../services/BridgeHelpers.js" as BridgeHelpers
import "../state"
import "../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model
    property string lastOperation: qsTr("None")
    property string activeTopic: "/logos-inspector/1/chat/proto"

    width: parent ? parent.width : 900
    spacing: root.theme.gapLarge

    ListModel {
        id: deliveryTabs

        ListElement { value: "messages"; label: "Messages" }
        ListElement { value: "node"; label: "Node" }
        ListElement { value: "operations"; label: "Operations" }
    }

    ListModel {
        id: operationLog
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Network / Delivery")
        title: qsTr("Delivery")
        layerLabel: qsTr("Network")
        subtitle: qsTr("Subscribe to content topics and send messages through the configured Delivery source.")
        Layout.fillWidth: true
    }

    SourceStrip {
        theme: root.theme
        sources: root.sourceBadges()
        Layout.fillWidth: true
    }

    GridLayout {
        columns: root.width < 760 ? 2 : 4
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gap
        Layout.fillWidth: true

        StatusChip {
            theme: root.theme
            label: qsTr("Source")
            value: root.model.deliverySourceLabel()
            tone: root.deliveryModuleSource() ? "success" : "warning"
            Layout.fillWidth: true
        }

        StatusChip {
            theme: root.theme
            label: qsTr("Topic")
            value: root.topicShortText(root.activeTopic)
            tone: root.validContentTopic(root.activeTopic) ? "success" : "warning"
            Layout.fillWidth: true
        }

        StatusChip {
            theme: root.theme
            label: qsTr("Network")
            value: root.model.messagingNetworkPreset
            tone: "neutral"
            Layout.fillWidth: true
        }

        StatusChip {
            theme: root.theme
            label: qsTr("Last")
            value: root.lastOperation
            tone: root.model.resultIsError && root.model.resultOwner === root.model.currentView ? "error" : "neutral"
            Layout.fillWidth: true
        }
    }

    TabSwitch {
        theme: root.theme
        current: root.model.deliveryAppTab
        options: deliveryTabs
        Layout.fillWidth: true
        onSelected: value => root.model.deliveryAppTab = value
    }

    Loader {
        active: true
        sourceComponent: root.tabComponent(root.model.deliveryAppTab)
        Layout.fillWidth: true
    }

    Panel {
        visible: root.model.pageHasOutput("messaging")
        theme: root.theme
        title: root.model.resultIsError ? qsTr("Operation error") : qsTr("Operation result")

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Text {
                text: root.model.resultTitle
                color: root.model.resultIsError ? root.theme.error : root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
                font.weight: Font.Medium
                elide: Text.ElideRight
                Layout.fillWidth: true
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Clear")
                Layout.preferredWidth: 84
                onClicked: root.model.clearResult()
            }
        }

        TextArea {
            readOnly: true
            text: root.model.resultText.length ? root.model.resultText : qsTr("No response body.")
            wrapMode: TextArea.Wrap
            color: root.model.resultIsError ? root.theme.warning : root.theme.text
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
            Layout.preferredHeight: 220

            background: Rectangle {
                color: root.model.resultIsError ? root.theme.errorMuted : root.theme.field
                radius: root.theme.radius
                border.width: 1
                border.color: root.model.resultIsError ? root.theme.error : root.theme.outline
            }
        }
    }

    Component {
        id: messagesTab

        Panel {
            theme: root.theme
            title: qsTr("Messages")

            StatusMessage {
                visible: !root.deliveryModuleSource()
                theme: root.theme
                tone: "warning"
                title: qsTr("Module source required")
                message: qsTr("Select the Delivery module source in Settings to run subscribe, unsubscribe, and send operations.")
                Layout.fillWidth: true
            }

            FieldRow {
                id: topicField

                theme: root.theme
                label: qsTr("Content topic")
                placeholderText: qsTr("/myapp/1/chat/proto")
                sourceText: root.activeTopic
                syncSourceText: true
                Layout.fillWidth: true
                onTextEdited: text => root.activeTopic = text
            }

            TextAreaField {
                id: payloadField

                theme: root.theme
                label: qsTr("Payload")
                rows: 5
                placeholderText: qsTr("message")
                Layout.fillWidth: true
            }

            RowLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Subscribe")
                    primary: true
                    enabled: root.messageControlsEnabled(topicField.text)
                    Layout.preferredWidth: 124
                    onClicked: root.runDelivery("deliverySubscribe", [topicField.text.trim()], qsTr("Subscribe"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Unsubscribe")
                    enabled: root.messageControlsEnabled(topicField.text)
                    Layout.preferredWidth: 136
                    onClicked: root.runDelivery("deliveryUnsubscribe", [topicField.text.trim()], qsTr("Unsubscribe"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Send")
                    enabled: root.messageControlsEnabled(topicField.text) && payloadField.text.length > 0
                    Layout.preferredWidth: 104
                    onClicked: root.runDelivery("deliverySend", [topicField.text.trim(), payloadField.text], qsTr("Send message"))
                }

                Item {
                    Layout.fillWidth: true
                }
            }
        }
    }

    Component {
        id: nodeTab

        Panel {
            theme: root.theme
            title: qsTr("Node")

            TextAreaField {
                id: nodeConfig

                theme: root.theme
                label: qsTr("Config JSON")
                rows: 7
                text: root.defaultNodeConfig()
                Layout.fillWidth: true
            }

            RowLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Create")
                    primary: true
                    enabled: !root.model.busy && root.deliveryModuleSource() && nodeConfig.text.trim().length > 0
                    Layout.preferredWidth: 112
                    onClicked: root.runDelivery("deliveryCreateNode", [nodeConfig.text.trim()], qsTr("Create node"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Start")
                    enabled: !root.model.busy && root.deliveryModuleSource()
                    Layout.preferredWidth: 96
                    onClicked: root.runDelivery("deliveryStart", [], qsTr("Start node"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Stop")
                    enabled: !root.model.busy && root.deliveryModuleSource()
                    Layout.preferredWidth: 96
                    onClicked: root.runDelivery("deliveryStop", [], qsTr("Stop node"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Settings")
                    enabled: !root.model.busy
                    Layout.preferredWidth: 112
                    onClicked: root.model.openSettings("network", "messaging")
                }

                Item {
                    Layout.fillWidth: true
                }
            }
        }
    }

    Component {
        id: operationsTab

        Panel {
            theme: root.theme
            title: qsTr("Operations")

            ColumnLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                Repeater {
                    model: operationLog.count > 0 ? operationLog : emptyOperationModel

                    delegate: OperationRow {
                        required property string time
                        required property string label
                        required property string status
                        required property string detail

                        theme: root.theme
                        timeText: time
                        labelText: label
                        statusText: status
                        detailText: detail
                    }
                }
            }
        }
    }

    ListModel {
        id: emptyOperationModel

        ListElement {
            time: "-"
            label: "No operations"
            status: "-"
            detail: "-"
        }
    }

    function tabComponent(tab) {
        switch (String(tab || "")) {
        case "node":
            return nodeTab
        case "operations":
            return operationsTab
        default:
            return messagesTab
        }
    }

    function sourceBadges() {
        const sources = [qsTr("Delivery"), root.model.deliverySourceLabel()]
        if (root.deliveryModuleSource()) {
            sources.push(root.model.deliveryModule)
        } else {
            sources.push(root.model.messagingRestUrl)
        }
        sources.push(root.model.messagingNetworkPreset)
        return sources
    }

    function deliveryModuleSource() {
        const mode = String(root.model.messagingSourceMode || "").toLowerCase()
        return mode === "module" || mode === "basecamp" || mode === "basecamp-module" || mode === "basecamp module"
    }

    function deliveryArgs(extra) {
        const args = [root.model.messagingSourceMode, root.model.messagingRestUrl]
        return args.concat(extra || [])
    }

    function runDelivery(method, args, label) {
        const response = root.model.callInspector(method, root.deliveryArgs(args), label)
        root.appendOperation(label, response)
        root.lastOperation = response.ok ? label : qsTr("Error")
        return response
    }

    function appendOperation(label, response) {
        operationLog.insert(0, {
            time: root.timeText(),
            label: String(label || ""),
            status: response && response.ok ? qsTr("ok") : qsTr("error"),
            detail: response && response.ok ? root.operationSummary(response.value) : String((response && response.error) || "")
        })
        while (operationLog.count > 20) {
            operationLog.remove(operationLog.count - 1)
        }
    }

    function operationPayload(value) {
        if (value && value.value && value.value.result && value.value.result.value !== undefined) {
            return value.value.result.value
        }
        if (value && value.result && value.result.value !== undefined) {
            return value.result.value
        }
        if (value && value.value !== undefined) {
            return value.value
        }
        return value
    }

    function operationSummary(value) {
        const payload = root.operationPayload(value)
        if (payload === undefined || payload === null) {
            return qsTr("No value")
        }
        if (typeof payload === "string") {
            return payload
        }
        if (typeof payload === "boolean") {
            return payload ? qsTr("true") : qsTr("false")
        }
        return BridgeHelpers.formatValue(payload).replace(/\s+/g, " ").slice(0, 180)
    }

    function messageControlsEnabled(topic) {
        return !root.model.busy && root.deliveryModuleSource() && root.validContentTopic(topic)
    }

    function validContentTopic(topic) {
        const value = String(topic || "").trim()
        return /^\/[^/]+\/[^/]+\/[^/]+\/[^/]+$/.test(value)
    }

    function topicShortText(topic) {
        const value = String(topic || "").trim()
        if (!value.length) {
            return qsTr("Required")
        }
        if (value.length <= 28) {
            return value
        }
        return value.slice(0, 12) + "..." + value.slice(value.length - 12)
    }

    function defaultNodeConfig() {
        return BridgeHelpers.formatValue({
            logLevel: "INFO",
            mode: "Core",
            preset: root.model.messagingNetworkPreset || "logos.test"
        })
    }

    function timeText() {
        return Qt.formatTime(new Date(), "HH:mm:ss")
    }

    component StatusChip: Rectangle {
        id: chipRoot

        required property Theme theme
        property string label: ""
        property string value: "-"
        property string tone: "neutral"

        radius: chipRoot.theme.radius
        color: chipRoot.fillColor()
        border.width: 1
        border.color: chipRoot.borderColor()
        implicitHeight: 50
        Layout.minimumWidth: 150

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: chipRoot.theme.gapSmall
            spacing: 2

            Text {
                text: chipRoot.label
                color: chipRoot.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: chipRoot.theme.labelText
                font.weight: Font.DemiBold
                font.capitalization: Font.AllUppercase
                elide: Text.ElideRight
                Layout.fillWidth: true
            }

            Text {
                text: chipRoot.value.length ? chipRoot.value : "-"
                color: chipRoot.theme.text
                textFormat: Text.PlainText
                font.pixelSize: chipRoot.theme.secondaryText
                font.weight: Font.Medium
                elide: Text.ElideMiddle
                Layout.fillWidth: true
            }
        }

        function fillColor() {
            if (chipRoot.tone === "success") {
                return chipRoot.theme.successMuted
            }
            if (chipRoot.tone === "warning") {
                return chipRoot.theme.warningMuted
            }
            if (chipRoot.tone === "error") {
                return chipRoot.theme.errorMuted
            }
            return chipRoot.theme.field
        }

        function borderColor() {
            if (chipRoot.tone === "success") {
                return chipRoot.theme.success
            }
            if (chipRoot.tone === "warning") {
                return chipRoot.theme.warning
            }
            if (chipRoot.tone === "error") {
                return chipRoot.theme.error
            }
            return chipRoot.theme.outlineMuted
        }
    }

    component OperationRow: Rectangle {
        id: opRoot

        required property Theme theme
        property string timeText: ""
        property string labelText: ""
        property string statusText: ""
        property string detailText: ""

        radius: opRoot.theme.radius
        color: opRoot.theme.field
        border.width: 1
        border.color: opRoot.statusText === "error" ? opRoot.theme.error : opRoot.theme.outlineMuted
        implicitHeight: 62
        Layout.fillWidth: true

        GridLayout {
            anchors.fill: parent
            anchors.margins: opRoot.theme.gapSmall
            columns: 4
            columnSpacing: opRoot.theme.gap

            Text {
                text: opRoot.timeText
                color: opRoot.theme.textDim
                textFormat: Text.PlainText
                font.family: "monospace"
                font.pixelSize: opRoot.theme.dataText
                Layout.preferredWidth: 64
            }

            Text {
                text: opRoot.labelText
                color: opRoot.theme.text
                textFormat: Text.PlainText
                font.pixelSize: opRoot.theme.secondaryText
                font.weight: Font.Medium
                elide: Text.ElideRight
                Layout.preferredWidth: 150
            }

            Text {
                text: opRoot.statusText
                color: opRoot.statusText === "error" ? opRoot.theme.error : opRoot.theme.success
                textFormat: Text.PlainText
                font.pixelSize: opRoot.theme.secondaryText
                font.weight: Font.DemiBold
                Layout.preferredWidth: 56
            }

            Text {
                text: opRoot.detailText
                color: opRoot.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: opRoot.theme.dataText
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }
    }
}
