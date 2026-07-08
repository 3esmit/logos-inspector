pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../components"
import "../state"
import "../theme"
import "../utils/UiFormat.js" as UiFormat

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model
    property string moduleKind: "blockchain"
    property string title: ""
    property string subtitle: ""
    readonly property bool hasResponse: root.model.pageHasOutput(root.moduleKind)
    readonly property var responseValue: root.hasResponse ? root.model.resultValue : null
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
        title: root.model.resultIsError ? qsTr("Module error") : qsTr("Module response")

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
            visible: !root.model.resultIsError && root.responseProbeModel.length > 0
            theme: root.theme
            rows: root.responseProbeModel
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
                    enabled: !root.model.busy
                    Layout.fillWidth: true
                    accessibleName: qsTr("Refresh blockchain node")
                    onClicked: root.model.callInspector("blockchainNode", root.model.blockchainArgs([]), qsTr("Blockchain node"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Load blocks")
                    enabled: !root.model.busy && slotFrom.text.trim().length > 0 && slotTo.text.trim().length > 0
                    Layout.fillWidth: true
                    accessibleName: qsTr("Load blockchain blocks")
                    onClicked: root.model.callInspector("blockchainBlocks", root.model.blockchainArgs([slotFrom.text, slotTo.text]), qsTr("Blockchain blocks"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Load block")
                    enabled: !root.model.busy && blockId.text.trim().length > 0
                    Layout.fillWidth: true
                    accessibleName: qsTr("Load blockchain block")
                    onClicked: root.model.callInspector("blockchainBlock", root.model.blockchainArgs([blockId.text.trim()]), qsTr("Blockchain block"))
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
                    enabled: !root.model.busy
                    Layout.fillWidth: true
                    accessibleName: qsTr("Run storage source report")
                    onClicked: root.model.callInspector("storageSourceReport", [
                        root.model.effectiveStorageSourceMode(root.model.storageSourceMode),
                        root.model.configuredStorageRestUrl(),
                        root.model.storageMetricsUrl,
                        cid.text.trim(),
                        root.model.storagePrivilegedDebugEnabled
                    ], qsTr("Storage report"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Check")
                    enabled: !root.model.busy
                    Layout.fillWidth: true
                    accessibleName: qsTr("Query storage status")
                    onClicked: root.model.queryNetworkConnection("storage", true, cid.text.trim().length > 0)
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("CID exists")
                    enabled: !root.model.busy && cid.text.trim().length > 0
                    Layout.fillWidth: true
                    accessibleName: qsTr("Check storage CID existence")
                    onClicked: root.model.callInspector("storageExists", [root.model.effectiveStorageSourceMode(root.model.storageSourceMode), root.model.configuredStorageRestUrl(), cid.text.trim()], qsTr("Storage CID"))
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
                    enabled: !root.model.busy
                    Layout.fillWidth: true
                    accessibleName: qsTr("Run delivery source report")
                    onClicked: root.model.callInspector("deliverySourceReport", root.model.deliverySourceReportArgs(), qsTr("Messaging report"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Check")
                    enabled: !root.model.busy
                    Layout.fillWidth: true
                    accessibleName: qsTr("Query delivery status")
                    onClicked: root.model.queryNetworkConnection("messaging", true)
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Settings")
                    enabled: !root.model.busy
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
                enabled: !root.model.busy
                Layout.fillWidth: true
                accessibleName: qsTr("Fetch LogosCore status")
                onClicked: root.model.callInspector("logoscoreStatus", [], qsTr("LogosCore status"))
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Settings")
                enabled: !root.model.busy
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
        switch (kind) {
        case "storage":
            return qsTr("Storage")
        case "messaging":
            return qsTr("Messaging")
        case "capabilities":
            return qsTr("Capabilities")
        default:
            return qsTr("L1 Node")
        }
    }

    function moduleLayer() {
        if (root.moduleKind === "blockchain") {
            return qsTr("L1 Bedrock")
        }
        return qsTr("Diagnostics")
    }

    function moduleName(kind) {
        switch (kind) {
        case "storage":
            return root.model.storageModule
        case "messaging":
            return root.model.deliveryModule
        case "capabilities":
            return root.model.inspectorModule
        default:
            return root.model.nodeUrl
        }
    }

    function modulePanelTitle() {
        return qsTr("%1 tools").arg(root.moduleLabel(root.moduleKind))
    }

    function moduleMessageTitle() {
        if (root.moduleKind === "blockchain") {
            return qsTr("Node and LogosCore")
        }
        return qsTr("LogosCore module")
    }

    function moduleMessage() {
        switch (root.moduleKind) {
        case "storage":
            return qsTr("Run storage REST metadata probes, then check a specific CID through the configured source.")
        case "messaging":
            return qsTr("Inspect delivery REST metadata without leaving the Messaging surface.")
        case "capabilities":
            return qsTr("Check LogosCore status and source configuration from one place.")
        default:
            return qsTr("Probe the configured blockchain node and block windows from this screen.")
        }
    }

    function moduleTargetText() {
        if (root.moduleKind === "blockchain") {
            return root.endpointLabel(root.model.nodeUrl)
        }
        return qsTr("Local")
    }

    function moduleTargetDetail() {
        if (root.moduleKind === "blockchain") {
            return root.shortEndpoint(root.model.nodeUrl)
        }
        return qsTr("LogosCore bridge")
    }

    function moduleStatusText() {
        if (!root.hasResponse) {
            return qsTr("Idle")
        }
        if (root.model.resultIsError) {
            return qsTr("Error")
        }
        return root.responseStatusText()
    }

    function moduleStatusDelta() {
        if (!root.hasResponse) {
            return qsTr("Awaiting call")
        }
        if (root.model.resultIsError) {
            return root.model.resultText
        }
        return root.responseSourceText()
    }

    function moduleStatusColor() {
        if (!root.hasResponse) {
            return root.theme.textMuted
        }
        if (root.model.resultIsError) {
            return root.theme.warning
        }
        return root.responseStatusColor()
    }

    function moduleProbeText() {
        if (root.responseProbeModel.length > 0) {
            return root.numberText(root.responseProbeModel.length)
        }
        return root.expectedProbeText()
    }

    function moduleProbeDelta() {
        if (root.responseProbeModel.length > 0) {
            return root.responseProbeDelta()
        }
        return qsTr("Default probe plan")
    }

    function expectedProbeText() {
        switch (root.moduleKind) {
        case "storage":
            return "10"
        case "messaging":
            return "12"
        case "capabilities":
            return "1"
        default:
            return "5"
        }
    }

    function responseStatusText() {
        if (root.model.resultIsError) {
            return qsTr("Error")
        }
        const rows = root.responseProbeModel
        if (!rows.length) {
            return qsTr("OK")
        }
        const ok = root.responseProbeOkCount()
        if (ok === rows.length) {
            return qsTr("OK")
        }
        if (ok === 0) {
            return qsTr("Error")
        }
        return qsTr("Partial")
    }

    function responseStatusColor() {
        const status = root.responseStatusText()
        if (status === qsTr("OK")) {
            return root.theme.success
        }
        if (status === qsTr("Partial")) {
            return root.theme.warning
        }
        return root.theme.error
    }

    function responseSourceText() {
        return root.model.resultTitle.length ? root.model.resultTitle : root.moduleLabel(root.moduleKind)
    }

    function responseProbeOkCount() {
        const rows = root.responseProbeModel
        let ok = 0
        for (let i = 0; i < rows.length; ++i) {
            if (rows[i].ok) {
                ok += 1
            }
        }
        return ok
    }

    function responseProbeOkText() {
        const rows = root.responseProbeModel
        if (!rows.length) {
            return root.hasResponse && !root.model.resultIsError ? qsTr("Yes") : "-"
        }
        return qsTr("%1/%2").arg(root.responseProbeOkCount()).arg(rows.length)
    }

    function responseProbeDelta() {
        const rows = root.responseProbeModel
        if (!rows.length) {
            return qsTr("No probe breakdown")
        }
        return qsTr("%1 probe(s)").arg(rows.length)
    }

    function responsePayloadText() {
        const value = root.responseValue
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
        const value = root.responseValue
        if (Array.isArray(value)) {
            return qsTr("Array items")
        }
        if (value && typeof value === "object") {
            return qsTr("Object fields")
        }
        return qsTr("Scalar value")
    }

    function responseTargetText() {
        const value = root.responseValue
        if (value && typeof value === "object" && !Array.isArray(value) && value.endpoint !== undefined) {
            return root.endpointLabel(value.endpoint)
        }
        if (root.moduleKind === "blockchain") {
            return root.endpointLabel(root.model.nodeUrl)
        }
        return qsTr("Local")
    }

    function responseTargetDetail() {
        const value = root.responseValue
        if (value && typeof value === "object" && !Array.isArray(value) && value.endpoint !== undefined) {
            return root.shortEndpoint(value.endpoint)
        }
        return root.moduleTargetDetail()
    }

    function responseProbeRows() {
        const rows = []
        const value = root.responseValue
        if (!value || typeof value !== "object" || Array.isArray(value)) {
            return rows
        }

        if (root.isProbe(value)) {
            root.pushProbe(rows, value, root.responseSourceText(), "")
            return rows
        }

        root.appendModuleReport(rows, value, "")

        root.pushNamedProbe(rows, value, "cryptarchia_info", qsTr("Cryptarchia info"), "")
        root.pushNamedProbe(rows, value, "headers", qsTr("Headers"), "")
        root.pushNamedProbe(rows, value, "network_info", qsTr("Network info"), "")
        root.pushNamedProbe(rows, value, "mantle_metrics", qsTr("Mantle metrics"), "")
        root.pushNamedProbe(rows, value, "status", qsTr("LogosCore status"), "")

        root.appendModuleReport(rows, value.blockchain, qsTr("Blockchain"))
        root.appendModuleReport(rows, value.storage, qsTr("Storage"))
        root.appendModuleReport(rows, value.delivery, qsTr("Messaging"))
        root.appendModuleReport(rows, value.capabilities, qsTr("Capabilities"))

        return rows
    }

    function blockchainPeerIdProbe() {
        const value = root.responseValue
        if (!value || typeof value !== "object" || Array.isArray(value)) {
            return root.model.moduleProbe("blockchain", "get_peer_id")
        }
        if (root.isBlockchainModuleReport(value)) {
            return root.findModuleProbe(value, "get_peer_id")
        }
        if (root.isBlockchainModuleReport(value.blockchain)) {
            return root.findModuleProbe(value.blockchain, "get_peer_id")
        }
        return root.model.moduleProbe("blockchain", "get_peer_id")
    }

    function blockchainPeerIdText() {
        const probe = root.blockchainPeerIdProbe()
        if (!probe) {
            return qsTr("Unavailable")
        }
        if (probe.ok !== true) {
            return probe.error ? qsTr("Unavailable: %1").arg(root.valueText(probe.error)) : qsTr("Unavailable")
        }
        const value = root.probeScalarText(probe.value)
        return value.length > 0 ? value : qsTr("Unavailable")
    }

    function blockchainPeerIdCopyText() {
        const probe = root.blockchainPeerIdProbe()
        if (!probe || probe.ok !== true) {
            return ""
        }
        return root.probeScalarText(probe.value)
    }

    function probeScalarText(value) {
        if (value === undefined || value === null || value === "") {
            return ""
        }
        const scalar = root.model.scalarValue(value)
        if (scalar === null || scalar === undefined || scalar === "") {
            return root.valueText(value)
        }
        return String(scalar)
    }

    function isBlockchainModuleReport(value) {
        return value && typeof value === "object" && !Array.isArray(value) && String(value.module || "") === root.model.blockchainModule
    }

    function findModuleProbe(report, method) {
        if (!report || typeof report !== "object" || Array.isArray(report)) {
            return null
        }
        const wanted = String(method || "")
        const fact = root.model.sourceProbeFact(report, wanted)
        if (fact) {
            return fact
        }
        const probes = Array.isArray(report.probes) ? report.probes : []
        for (let i = 0; i < probes.length; ++i) {
            const probe = probes[i] || {}
            if (String(probe.probe_key || probe.key || "") === wanted) {
                return probe
            }
            const label = String(probe.label || "")
            const source = String(probe.source || "")
            if (label.indexOf("." + wanted) >= 0 || source.indexOf(" " + wanted) >= 0) {
                return probe
            }
        }
        return null
    }

    function appendModuleReport(rows, report, prefix) {
        if (!report || typeof report !== "object" || Array.isArray(report)) {
            return
        }
        const labelPrefix = prefix.length ? prefix : root.moduleDisplayName(report.module)
        const facts = Array.isArray(report.probe_facts) ? report.probe_facts : []
        if (facts.length > 0) {
            for (let i = 0; i < facts.length; ++i) {
                root.pushProbe(rows, facts[i], qsTr("Probe fact"), labelPrefix)
            }
            return
        }
        if (root.isProbe(report.module_info)) {
            root.pushProbe(rows, report.module_info, qsTr("Module info"), labelPrefix)
        }
        const probes = Array.isArray(report.probes) ? report.probes : []
        for (let i = 0; i < probes.length; ++i) {
            root.pushProbe(rows, probes[i], qsTr("Probe"), labelPrefix)
        }
    }

    function pushNamedProbe(rows, value, key, label, prefix) {
        if (value && root.isProbe(value[key])) {
            root.pushProbe(rows, value[key], label, prefix)
        }
    }

    function pushProbe(rows, probe, fallbackLabel, prefix) {
        if (!root.isProbe(probe)) {
            return
        }
        const baseLabel = String(probe.label || probe.key || fallbackLabel || "-")
        const label = prefix && prefix.length ? qsTr("%1 / %2").arg(prefix).arg(baseLabel) : baseLabel
        rows.push({
            label: label,
            source: String(probe.source || ""),
            ok: !!probe.ok,
            detail: probe.ok ? root.valueSummary(probe.value) : root.valueText(probe.error)
        })
    }

    function isProbe(value) {
        return value && typeof value === "object" && !Array.isArray(value) && value.ok !== undefined
    }

    function moduleDisplayName(name) {
        switch (String(name || "")) {
        case root.model.storageModule:
            return qsTr("Storage")
        case root.model.deliveryModule:
            return qsTr("Messaging")
        case root.model.capabilityModule:
            return qsTr("Capabilities")
        case root.model.blockchainModule:
            return qsTr("Blockchain")
        default:
            return ""
        }
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
