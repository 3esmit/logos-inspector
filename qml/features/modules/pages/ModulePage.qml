pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../components"
import "../../../state"
import "../../../state/modules/ModuleReportPresentation.js" as ModuleReportPresentation
import "../../../state/source_operations/NodeOperationRequest.js" as NodeOperationRequest
import "../../../theme"
import "../../../utils/UiFormat.js" as UiFormat

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model
    property string moduleKind: "blockchain"
    property string title: ""
    property string subtitle: ""
    readonly property bool hasResponse: root.model.pageHasOutput(root.moduleKind)
    readonly property bool chainControlBusy: root.moduleKind === "blockchain"
        && (root.model.chainPages.operationPending("module-control.node")
            || root.model.chainPages.operationPending("module-control.blocks")
            || root.model.chainPages.operationPending("module-control.block"))
    readonly property bool requestBusy: root.model.shell.busy || root.model.asyncPresentationBusy
        || root.chainControlBusy
    readonly property var responseValue: root.hasResponse ? root.model.shell.resultValue : null
    readonly property var responseProbeModel: root.responseProbeRows()

    width: parent ? parent.width : 900
    spacing: 16

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / %1").arg(root.title)
        title: root.title
        layerLabel: root.moduleLayer()
        subtitle: root.subtitle
        Layout.fillWidth: true
    }

    GridLayout {
        columns: root.width < 760 ? 2 : 4
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gap
        Layout.fillWidth: true

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Module")
            value: root.moduleLabel(root.moduleKind)
            delta: root.moduleName(root.moduleKind)
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Target")
            value: root.moduleTargetText()
            delta: root.moduleTargetDetail()
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Status")
            value: root.moduleStatusText()
            delta: root.moduleStatusDelta()
            deltaColor: root.moduleStatusColor()
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Probes")
            value: root.moduleProbeText()
            delta: root.moduleProbeDelta()
            deltaColor: root.responseProbeOkCount() === root.responseProbeModel.length && root.responseProbeModel.length > 0 ? root.theme.success : root.theme.textMuted
        }
    }

    Panel {
        theme: root.theme
        title: root.modulePanelTitle()

        StatusMessage {
            theme: root.theme
            tone: "info"
            title: root.moduleMessageTitle()
            message: root.moduleMessage()
            Layout.fillWidth: true
        }

        Loader {
            active: true
            sourceComponent: root.controlsFor(root.moduleKind)
            Layout.fillWidth: true
        }
    }

    Panel {
        visible: root.hasResponse
        theme: root.theme
        title: root.model.shell.resultIsError ? qsTr("Module error") : qsTr("Module response")

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Text {
                text: root.model.shell.resultTitle
                color: root.model.shell.resultIsError ? root.theme.error : root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
                font.weight: Font.Medium
                elide: Text.ElideRight
                Layout.fillWidth: true
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Clear")
                enabled: root.model.shell.resultText.length > 0 || root.model.shell.resultValue !== null
                Layout.preferredWidth: 84
                onClicked: root.model.shell.clearResult()
            }
        }

        StatusMessage {
            visible: root.model.shell.resultIsError
            theme: root.theme
            tone: "warning"
            title: qsTr("Call failed")
            message: root.model.shell.resultText
            Layout.fillWidth: true
        }

        GridLayout {
            visible: !root.model.shell.resultIsError
            columns: root.width < 760 ? 2 : 4
            columnSpacing: root.theme.gap
            rowSpacing: root.theme.gap
            Layout.fillWidth: true

            MetricCard {
                theme: root.theme
                compact: true
                label: qsTr("Result")
                value: root.responseStatusText()
                delta: root.responseSourceText()
                deltaColor: root.responseStatusColor()
            }

            MetricCard {
                theme: root.theme
                compact: true
                label: qsTr("OK")
                value: root.responseProbeOkText()
                delta: root.responseProbeDelta()
                deltaColor: root.responseStatusColor()
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
                label: qsTr("Source")
                value: root.responseTargetText()
                delta: root.responseTargetDetail()
            }
        }

        ProbeList {
            visible: !root.model.shell.resultIsError && root.responseProbeModel.length > 0
            theme: root.theme
            rows: root.responseProbeModel
        }

        TextArea {
            objectName: "moduleRawResponse"
            readOnly: true
            text: root.model.shell.resultText.length ? root.model.shell.resultText : qsTr("No response body.")
            wrapMode: TextArea.Wrap
            color: root.model.shell.resultText.length ? root.theme.text : root.theme.textMuted
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
            Layout.preferredHeight: root.model.shell.resultIsError ? 120 : 220
            Accessible.role: Accessible.StaticText
            Accessible.name: root.model.shell.resultIsError
                ? qsTr("Raw module error response")
                : qsTr("Raw module response")
            Accessible.description: text

            background: Rectangle {
                color: root.model.shell.resultIsError ? root.theme.errorMuted : root.theme.field
                radius: root.theme.radius
                border.width: 1
                border.color: root.model.shell.resultIsError ? root.theme.error : root.theme.outline
            }
        }
    }

    function controlsFor(kind) {
        switch (kind) {
        case "storage":
            return storageControls
        case "messaging":
            return messagingControls
        case "capabilities":
            return capabilitiesControls
        default:
            return blockchainControls
        }
    }

    Component {
        id: blockchainControls

        ColumnLayout {
            spacing: 12

            GridLayout {
                columns: root.width < 680 ? 1 : 2
                columnSpacing: root.theme.gap
                rowSpacing: root.theme.gap
                Layout.fillWidth: true

                FieldRow {
                    id: slotFrom
                    theme: root.theme
                    label: qsTr("Slot from")
                    placeholderText: qsTr("49600")
                    Layout.fillWidth: true
                }

                FieldRow {
                    id: slotTo
                    theme: root.theme
                    label: qsTr("Slot to")
                    placeholderText: qsTr("49620")
                    Layout.fillWidth: true
                }
            }

            FieldRow {
                id: blockId
                theme: root.theme
                label: qsTr("Block id")
                placeholderText: qsTr("Optional block id")
            }

            GridLayout {
                columns: root.width < 680 ? 1 : 3
                columnSpacing: root.theme.gapSmall
                rowSpacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Refresh node")
                    primary: true
                    enabled: !root.requestBusy
                    Layout.fillWidth: true
                    accessibleName: qsTr("Refresh blockchain node")
                    onClicked: root.model.presentBlockchainOperation("module-control.node",
                        "blockchainNode", [], qsTr("Blockchain node"), root.moduleKind)
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Load blocks")
                    enabled: !root.requestBusy && slotFrom.text.trim().length > 0 && slotTo.text.trim().length > 0
                    Layout.fillWidth: true
                    accessibleName: qsTr("Load blockchain blocks")
                    onClicked: root.model.presentBlockchainOperation("module-control.blocks",
                        "blockchainBlocks", [slotFrom.text, slotTo.text],
                        qsTr("Blockchain blocks"), root.moduleKind)
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Load block")
                    enabled: !root.requestBusy && blockId.text.trim().length > 0
                    Layout.fillWidth: true
                    accessibleName: qsTr("Load blockchain block")
                    onClicked: root.model.presentBlockchainOperation("module-control.block",
                        "blockchainBlock", [blockId.text.trim()],
                        qsTr("Blockchain block"), root.moduleKind)
                }
            }
        }
    }

    Component {
        id: storageControls

        ColumnLayout {
            spacing: 12

            FieldRow {
                id: cid
                theme: root.theme
                label: qsTr("CID")
                placeholderText: qsTr("Optional CID for exists lookup")
            }

            GridLayout {
                columns: root.width < 680 ? 1 : 3
                columnSpacing: root.theme.gapSmall
                rowSpacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("REST report")
                    primary: true
                    enabled: !root.requestBusy
                    Layout.fillWidth: true
                    accessibleName: qsTr("Run storage source report")
                    onClicked: {
                        root.model.storageCidProbe = cid.text.trim()
                        root.model.metrics.queryNetworkConnection(
                            "storage",
                            true,
                            cid.text.trim().length > 0
                        )
                    }
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Check")
                    enabled: !root.model.shell.busy
                    Layout.fillWidth: true
                    accessibleName: qsTr("Query storage status")
                    onClicked: {
                        root.model.storageCidProbe = cid.text.trim()
                        root.model.metrics.queryNetworkConnection(
                            "storage",
                            true,
                            cid.text.trim().length > 0
                        )
                    }
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("CID exists")
                    enabled: !root.requestBusy && cid.text.trim().length > 0
                    Layout.fillWidth: true
                    accessibleName: qsTr("Check storage CID existence")
                    onClicked: {
                        root.model.callInspectorAsync("storageExists", [NodeOperationRequest.envelope(
                            root.model.sourceRouting.storageOperationAdapter(),
                            { cid: cid.text.trim() },
                            false
                        )], qsTr("Storage CID"))
                    }
                }
            }
        }
    }

    Component {
        id: messagingControls

        ColumnLayout {
            spacing: 12

            GridLayout {
                columns: root.width < 680 ? 1 : 3
                columnSpacing: root.theme.gapSmall
                rowSpacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("REST report")
                    primary: true
                    enabled: !root.requestBusy
                    Layout.fillWidth: true
                    accessibleName: qsTr("Run delivery source report")
                    onClicked: root.model.metrics.queryNetworkConnection("messaging", true)
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Check")
                    enabled: !root.model.shell.busy
                    Layout.fillWidth: true
                    accessibleName: qsTr("Query delivery status")
                    onClicked: root.model.metrics.queryNetworkConnection("messaging", true)
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Settings")
                    enabled: !root.model.shell.busy
                    Layout.fillWidth: true
                    accessibleName: qsTr("Open delivery settings")
                    onClicked: root.model.openSettings("network", "messaging")
                }
            }
        }
    }

    Component {
        id: capabilitiesControls

        GridLayout {
            columns: root.width < 680 ? 1 : 2
            columnSpacing: root.theme.gapSmall
            rowSpacing: root.theme.gapSmall
            Layout.fillWidth: true

            ActionButton {
                theme: root.theme
                text: qsTr("Core status")
                primary: true
                enabled: !root.requestBusy
                Layout.fillWidth: true
                accessibleName: qsTr("Fetch LogosCore status")
                onClicked: root.model.callInspectorAsync("logoscoreStatus", [], qsTr("LogosCore status"))
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Settings")
                enabled: !root.model.shell.busy
                Layout.fillWidth: true
                accessibleName: qsTr("Open network settings")
                onClicked: root.model.openSettings("network", "blockchain")
            }
        }
    }

    component ProbeList: ColumnLayout {
        id: listRoot

        required property Theme theme
        property var rows: []

        spacing: 6
        Layout.fillWidth: true

        Text {
            text: qsTr("Probe results")
            color: listRoot.theme.text
            textFormat: Text.PlainText
            font.pixelSize: listRoot.theme.primaryText
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        Frame {
            padding: 0
            Layout.fillWidth: true

            background: Rectangle {
                color: listRoot.theme.field
                radius: listRoot.theme.radius
                border.width: 1
                border.color: listRoot.theme.outlineMuted
            }

            contentItem: ColumnLayout {
                spacing: 0

                Repeater {
                    model: listRoot.rows

                    ProbeRow {
                        required property var modelData

                        theme: listRoot.theme
                        label: String(modelData.label || "-")
                        source: String(modelData.source || "")
                        detail: String(modelData.detail || "-")
                        ok: !!modelData.ok
                    }
                }
            }
        }
    }

    component ProbeRow: Item {
        id: rowRoot

        required property Theme theme
        property string label: ""
        property string source: ""
        property string detail: ""
        property bool ok: false

        Layout.fillWidth: true
        implicitHeight: Math.max(52, rowGrid.implicitHeight + 18)

        GridLayout {
            id: rowGrid

            anchors.fill: parent
            anchors.leftMargin: rowRoot.theme.gap
            anchors.rightMargin: rowRoot.theme.gap
            anchors.topMargin: rowRoot.theme.gapSmall
            anchors.bottomMargin: rowRoot.theme.gapSmall
            columns: root.width < 720 ? 2 : 3
            columnSpacing: root.theme.gap
            rowSpacing: 3

            Rectangle {
                color: rowRoot.ok ? rowRoot.theme.successMuted : rowRoot.theme.errorMuted
                radius: rowRoot.theme.radius
                border.width: 1
                border.color: rowRoot.ok ? rowRoot.theme.success : rowRoot.theme.error
                Layout.preferredWidth: 68
                Layout.preferredHeight: 26
                Layout.alignment: Qt.AlignTop

                Text {
                    anchors.centerIn: parent
                    text: rowRoot.ok ? qsTr("OK") : qsTr("Error")
                    color: rowRoot.ok ? rowRoot.theme.success : rowRoot.theme.error
                    textFormat: Text.PlainText
                    font.pixelSize: rowRoot.theme.labelText
                    font.weight: Font.DemiBold
                }
            }

            ColumnLayout {
                spacing: 2
                Layout.fillWidth: true

                Text {
                    text: rowRoot.label
                    color: rowRoot.theme.text
                    textFormat: Text.PlainText
                    font.pixelSize: rowRoot.theme.secondaryText
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }

                Text {
                    visible: rowRoot.source.length > 0
                    text: rowRoot.source
                    color: rowRoot.theme.textDim
                    textFormat: Text.PlainText
                    font.family: "monospace"
                    font.pixelSize: rowRoot.theme.labelText
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }

            Text {
                visible: root.width >= 720
                text: rowRoot.detail
                color: rowRoot.ok ? rowRoot.theme.textMuted : rowRoot.theme.warning
                textFormat: Text.PlainText
                font.family: "monospace"
                font.pixelSize: rowRoot.theme.dataText
                elide: Text.ElideRight
                Layout.fillWidth: true
            }

            Text {
                visible: root.width < 720
                text: rowRoot.detail
                color: rowRoot.ok ? rowRoot.theme.textMuted : rowRoot.theme.warning
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
                font.family: "monospace"
                font.pixelSize: rowRoot.theme.dataText
                Layout.columnSpan: 2
                Layout.fillWidth: true
            }
        }
    }

    function moduleLabel(kind) {
        return ModuleReportPresentation.moduleLabel(root, kind)
    }

    function moduleLayer() {
        return ModuleReportPresentation.moduleLayer(root)
    }

    function moduleName(kind) {
        return ModuleReportPresentation.moduleName(root, kind)
    }

    function modulePanelTitle() {
        return ModuleReportPresentation.modulePanelTitle(root)
    }

    function moduleMessageTitle() {
        return ModuleReportPresentation.moduleMessageTitle(root)
    }

    function moduleMessage() {
        return ModuleReportPresentation.moduleMessage(root)
    }

    function moduleTargetText() {
        return ModuleReportPresentation.moduleTargetText(root)
    }

    function moduleTargetDetail() {
        return ModuleReportPresentation.moduleTargetDetail(root)
    }

    function moduleStatusText() {
        return ModuleReportPresentation.moduleStatusText(root)
    }

    function moduleStatusDelta() {
        return ModuleReportPresentation.moduleStatusDelta(root)
    }

    function moduleStatusColor() {
        return ModuleReportPresentation.moduleStatusColor(root)
    }

    function moduleProbeText() {
        return ModuleReportPresentation.moduleProbeText(root)
    }

    function moduleProbeDelta() {
        return ModuleReportPresentation.moduleProbeDelta(root)
    }

    function expectedProbeText() {
        return ModuleReportPresentation.expectedProbeText(root)
    }

    function responseStatusText() {
        return ModuleReportPresentation.responseStatusText(root)
    }

    function responseStatusColor() {
        return ModuleReportPresentation.responseStatusColor(root)
    }

    function responseSourceText() {
        return ModuleReportPresentation.responseSourceText(root)
    }

    function responseProbeOkCount() {
        return ModuleReportPresentation.responseProbeOkCount(root)
    }

    function responseProbeOkText() {
        return ModuleReportPresentation.responseProbeOkText(root)
    }

    function responseProbeDelta() {
        return ModuleReportPresentation.responseProbeDelta(root)
    }

    function responsePayloadText() {
        return ModuleReportPresentation.responsePayloadText(root)
    }

    function responseKindText() {
        return ModuleReportPresentation.responseKindText(root)
    }

    function responseTargetText() {
        return ModuleReportPresentation.responseTargetText(root)
    }

    function responseTargetDetail() {
        return ModuleReportPresentation.responseTargetDetail(root)
    }

    function responseProbeRows() {
        return ModuleReportPresentation.responseProbeRows(root)
    }

    function blockchainPeerIdProbe() {
        return ModuleReportPresentation.blockchainPeerIdProbe(root)
    }

    function blockchainPeerIdText() {
        return ModuleReportPresentation.blockchainPeerIdText(root)
    }

    function blockchainPeerIdCopyText() {
        return ModuleReportPresentation.blockchainPeerIdCopyText(root)
    }

    function probeScalarText(value) {
        return ModuleReportPresentation.probeScalarText(root, value)
    }

    function isBlockchainModuleReport(value) {
        return ModuleReportPresentation.isBlockchainModuleReport(root, value)
    }

    function findModuleProbe(report, method) {
        return ModuleReportPresentation.findModuleProbe(report, method)
    }

    function appendModuleReport(rows, report, prefix) {
        ModuleReportPresentation.appendModuleReport(root, rows, report, prefix)
    }

    function pushNamedProbe(rows, value, key, label, prefix) {
        ModuleReportPresentation.pushNamedProbe(root, rows, value, key, label, prefix)
    }

    function pushProbe(rows, probe, fallbackLabel, prefix) {
        ModuleReportPresentation.pushProbe(root, rows, probe, fallbackLabel, prefix)
    }

    function isProbe(value) {
        return ModuleReportPresentation.isProbe(value)
    }

    function moduleDisplayName(name) {
        return ModuleReportPresentation.moduleDisplayName(root, name)
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

    function valueSummary(value) {
        if (value === undefined || value === null || value === "" || typeof value === "object") {
            return UiFormat.valueSummary(value, {
                emptyText: "-",
                shortArrayLimit: -1,
                objectSummary: "fields"
            })
        }
        return root.valueText(value)
    }

    function valueText(value) {
        return UiFormat.valueText(value, {
            emptyText: "-",
            objectMode: "json"
        })
    }

    function numberText(value) {
        return UiFormat.numberText(value, {
            emptyText: "-",
            coerceNumericStrings: true
        })
    }
}
