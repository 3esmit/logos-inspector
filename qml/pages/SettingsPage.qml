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
        id: profileOptions

        ListElement {
            key: "default"
            label: "Testnet"
            summary: "Public LEZ, local indexer and node defaults"
        }
        ListElement {
            key: "testnet-indexer-local"
            label: "Testnet + local indexer"
            summary: "Remote sequencer with local indexer probes"
        }
        ListElement {
            key: "local-node"
            label: "Local Logos node"
            summary: "Testnet LEZ with local base-chain node"
        }
        ListElement {
            key: "local"
            label: "Local sequencer"
            summary: "Local sequencer, indexer, and node"
        }
        ListElement {
            key: "custom"
            label: "Custom"
            summary: "Manual endpoint override"
        }
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Settings")
        title: qsTr("Settings")
        subtitle: qsTr("Configure network profiles and verify the sequencer, indexer, and blockchain node endpoints in use.")
        Layout.fillWidth: true

        ActionButton {
            theme: root.theme
            text: qsTr("Refresh")
            primary: true
            enabled: !root.model.busy
            Layout.preferredWidth: 104
            accessibleName: qsTr("Refresh endpoint status")
            onClicked: root.model.refreshDashboard()
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Dashboard")
            enabled: !root.model.busy
            Layout.preferredWidth: 116
            accessibleName: qsTr("Open dashboard")
            onClicked: root.model.selectView("overview")
        }
    }

    GridLayout {
        columns: root.width < 760 ? 2 : 4
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gap
        Layout.fillWidth: true

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Profile")
            value: root.profileLabel(root.model.networkProfile)
            delta: root.profileSummary(root.model.networkProfile)
            deltaColor: root.model.networkProfile === "custom" ? root.theme.warning : root.theme.textMuted
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Sequencer")
            value: root.probeStatusText("sequencer", "health")
            delta: root.sequencerDelta()
            deltaColor: root.probeStatusColor("sequencer", "health")
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Indexer")
            value: root.probeStatusText("indexer", "health")
            delta: root.indexerDelta()
            deltaColor: root.probeStatusColor("indexer", "health")
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Node")
            value: root.probeStatusText("node", "consensus")
            delta: root.nodeDelta()
            deltaColor: root.probeStatusColor("node", "consensus")
        }
    }

    Panel {
        theme: root.theme
        title: qsTr("Network profile")

        ColumnLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Text {
                text: qsTr("Profile")
                color: root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
                font.weight: Font.Medium
                Layout.fillWidth: true
            }

            ProfileComboBox {
                id: profilePicker

                theme: root.theme
                options: profileOptions
                currentIndex: root.profileIndexFor(root.model.networkProfile)
                Layout.fillWidth: true
                onProfileActivated: index => root.applyProfileIndex(index)
            }

            StatusMessage {
                theme: root.theme
                tone: root.model.networkProfile === "custom" ? "warning" : "info"
                title: root.profileLabel(root.model.networkProfile)
                message: root.profileDetail()
                Layout.fillWidth: true
            }
        }
    }

    Panel {
        theme: root.theme
        title: qsTr("Endpoints")

        GridLayout {
            columns: root.width < 880 ? 1 : 3
            columnSpacing: root.theme.gap
            rowSpacing: root.theme.gap
            Layout.fillWidth: true

            EndpointEditor {
                theme: root.theme
                title: qsTr("Sequencer")
                endpoint: root.model.sequencerUrl
                status: root.probeStatusText("sequencer", "health")
                statusColor: root.probeStatusColor("sequencer", "health")
                onEndpointEdited: value => root.updateSequencerUrl(value)
            }

            EndpointEditor {
                theme: root.theme
                title: qsTr("Indexer")
                endpoint: root.model.indexerUrl
                status: root.probeStatusText("indexer", "health")
                statusColor: root.probeStatusColor("indexer", "health")
                onEndpointEdited: value => root.updateIndexerUrl(value)
            }

            EndpointEditor {
                theme: root.theme
                title: qsTr("Blockchain node")
                endpoint: root.model.nodeUrl
                status: root.probeStatusText("node", "consensus")
                statusColor: root.probeStatusColor("node", "consensus")
                onEndpointEdited: value => root.updateNodeUrl(value)
            }
        }

        GridLayout {
            columns: root.width < 680 ? 1 : 4
            columnSpacing: root.theme.gapSmall
            rowSpacing: root.theme.gapSmall
            Layout.fillWidth: true

            ActionButton {
                theme: root.theme
                text: qsTr("Refresh status")
                primary: true
                enabled: !root.model.busy
                Layout.fillWidth: true
                accessibleName: qsTr("Refresh endpoint status")
                onClicked: root.model.refreshDashboard()
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Sequencer head")
                enabled: !root.model.busy
                Layout.fillWidth: true
                accessibleName: qsTr("Fetch sequencer head")
                onClicked: root.model.callInspector("head", [root.model.sequencerUrl], qsTr("Sequencer head"))
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Indexer head")
                enabled: !root.model.busy
                Layout.fillWidth: true
                accessibleName: qsTr("Fetch indexer finalized head")
                onClicked: root.model.callInspector("indexerFinalizedHead", [root.model.indexerUrl], qsTr("Indexer head"))
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Reset profile")
                enabled: !root.model.busy
                Layout.fillWidth: true
                accessibleName: qsTr("Reset endpoints to selected profile")
                onClicked: root.resetSelectedProfile()
            }
        }
    }

    Panel {
        theme: root.theme
        title: qsTr("Endpoint status")
        Layout.fillWidth: true

        StatusMessage {
            visible: root.model.dashboardOverview === null
            theme: root.theme
            tone: "info"
            title: qsTr("No probe loaded")
            message: qsTr("Refresh status to populate live health, head, and consensus values.")
            Layout.fillWidth: true
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

                StatusRow {
                    theme: root.theme
                    header: true
                    columns: [qsTr("Service"), qsTr("Status"), qsTr("Endpoint"), qsTr("Observed")]
                }

                StatusRow {
                    theme: root.theme
                    columns: [qsTr("Sequencer"), root.probeStatusText("sequencer", "health"), root.model.sequencerUrl, root.sequencerObserved()]
                    statusColor: root.probeStatusColor("sequencer", "health")
                }

                StatusRow {
                    theme: root.theme
                    columns: [qsTr("Indexer"), root.probeStatusText("indexer", "health"), root.model.indexerUrl, root.indexerObserved()]
                    statusColor: root.probeStatusColor("indexer", "health")
                }

                StatusRow {
                    theme: root.theme
                    columns: [qsTr("Blockchain node"), root.probeStatusText("node", "consensus"), root.model.nodeUrl, root.nodeObserved()]
                    statusColor: root.probeStatusColor("node", "consensus")
                }
            }
        }
    }

    function overview() {
        return root.model.dashboardOverview || {}
    }

    function probe(section, field) {
        const target = root.overview()[section]
        return target ? target[field] || null : null
    }

    function probeStatusText(section, field) {
        const target = root.probe(section, field)
        if (!target) {
            return qsTr("Unknown")
        }
        return target.ok ? qsTr("OK") : qsTr("Error")
    }

    function probeStatusColor(section, field) {
        const target = root.probe(section, field)
        if (!target) {
            return root.theme.textMuted
        }
        return target.ok ? root.theme.success : root.theme.warning
    }

    function probeValue(section, field) {
        const target = root.probe(section, field)
        return target && target.value !== undefined && target.value !== null ? target.value : null
    }

    function probeError(section, field) {
        const target = root.probe(section, field)
        return target && target.error ? String(target.error) : ""
    }

    function sequencerDelta() {
        const error = root.probeError("sequencer", "health")
        if (error.length > 0) {
            return error
        }
        const head = root.probeValue("sequencer", "head")
        if (head !== null) {
            return qsTr("Head %1").arg(root.valueText(head))
        }
        return root.shortEndpoint(root.model.sequencerUrl)
    }

    function indexerDelta() {
        const error = root.probeError("indexer", "health")
        if (error.length > 0) {
            return error
        }
        const head = root.probeValue("indexer", "head")
        if (head !== null) {
            return qsTr("Head %1").arg(root.valueText(head))
        }
        return root.shortEndpoint(root.model.indexerUrl)
    }

    function nodeDelta() {
        const error = root.probeError("node", "consensus")
        if (error.length > 0) {
            return error
        }
        const consensus = root.probeValue("node", "consensus")
        const info = consensus && consensus.cryptarchia_info ? consensus.cryptarchia_info : null
        if (info && info.slot !== undefined) {
            return qsTr("Slot %1").arg(root.valueText(info.slot))
        }
        return root.shortEndpoint(root.model.nodeUrl)
    }

    function sequencerObserved() {
        const error = root.probeError("sequencer", "health")
        if (error.length > 0) {
            return error
        }
        const head = root.probeValue("sequencer", "head")
        const programs = root.probeValue("sequencer", "programs")
        const parts = []
        if (head !== null) {
            parts.push(qsTr("head %1").arg(root.valueText(head)))
        }
        if (programs !== null) {
            parts.push(qsTr("%1 program(s)").arg(root.valueText(programs)))
        }
        return parts.length ? parts.join(" / ") : qsTr("No sequencer probe")
    }

    function indexerObserved() {
        const error = root.probeError("indexer", "health")
        if (error.length > 0) {
            return error
        }
        const head = root.probeValue("indexer", "head")
        const programs = root.probeValue("indexer", "programs")
        const parts = []
        if (head !== null) {
            parts.push(qsTr("head %1").arg(root.valueText(head)))
        }
        if (programs !== null) {
            parts.push(qsTr("%1 program(s)").arg(root.valueText(programs)))
        }
        return parts.length ? parts.join(" / ") : qsTr("No indexer probe")
    }

    function nodeObserved() {
        const error = root.probeError("node", "consensus")
        if (error.length > 0) {
            return error
        }
        const consensus = root.probeValue("node", "consensus")
        const info = consensus && consensus.cryptarchia_info ? consensus.cryptarchia_info : null
        if (info) {
            const slot = info.slot !== undefined ? root.valueText(info.slot) : "-"
            const lib = info.lib_slot !== undefined ? root.valueText(info.lib_slot) : "-"
            return qsTr("slot %1 / lib %2").arg(slot).arg(lib)
        }
        return qsTr("No consensus probe")
    }

    function updateSequencerUrl(value) {
        root.model.sequencerUrl = String(value || "").trim()
        root.syncProfileFromEndpoints()
    }

    function updateIndexerUrl(value) {
        root.model.indexerUrl = String(value || "").trim()
        root.syncProfileFromEndpoints()
    }

    function updateNodeUrl(value) {
        root.model.nodeUrl = String(value || "").trim()
        root.syncProfileFromEndpoints()
    }

    function syncProfileFromEndpoints() {
        root.model.networkProfile = root.inferProfile(root.model.sequencerUrl, root.model.indexerUrl, root.model.nodeUrl)
    }

    function applyProfileIndex(index) {
        if (index === 4) {
            root.model.networkProfile = "custom"
            return
        }
        root.model.applyProfile(index)
    }

    function resetSelectedProfile() {
        const index = root.profileIndexFor(root.model.networkProfile)
        root.model.applyProfile(index === 4 ? 0 : index)
    }

    function profileIndexFor(value) {
        if (value === "testnet-indexer-local") {
            return 1
        }
        if (value === "local-node") {
            return 2
        }
        if (value === "local") {
            return 3
        }
        if (value === "custom") {
            return 4
        }
        return 0
    }

    function inferProfile(sequencer, indexer, node) {
        const seq = root.normalizeEndpoint(sequencer)
        const idx = root.normalizeEndpoint(indexer)
        const nod = root.normalizeEndpoint(node)
        const testnetSeq = root.normalizeEndpoint("https://testnet.lez.logos.co/")
        const localSeq = root.normalizeEndpoint("http://127.0.0.1:3040/")
        const localIndexer = root.normalizeEndpoint("http://127.0.0.1:8779/")
        const localNode = root.normalizeEndpoint("http://127.0.0.1:8080/")

        if (seq === localSeq && idx === localIndexer && nod === localNode) {
            return "local"
        }
        if (seq === testnetSeq && idx === localIndexer && nod === localNode) {
            return root.model.networkProfile === "testnet-indexer-local" || root.model.networkProfile === "local-node" ? root.model.networkProfile : "default"
        }
        return "custom"
    }

    function profileLabel(value) {
        if (value === "local") {
            return qsTr("Local")
        }
        if (value === "local-node") {
            return qsTr("Local node")
        }
        if (value === "testnet-indexer-local") {
            return qsTr("Mixed")
        }
        if (value === "custom") {
            return qsTr("Custom")
        }
        return qsTr("Testnet")
    }

    function profileSummary(value) {
        if (value === "local") {
            return qsTr("All endpoints local")
        }
        if (value === "local-node") {
            return qsTr("Local node focus")
        }
        if (value === "testnet-indexer-local") {
            return qsTr("Remote LEZ, local indexer")
        }
        if (value === "custom") {
            return qsTr("Manual endpoints")
        }
        return qsTr("Default testnet")
    }

    function profileDetail() {
        return qsTr("%1 / %2 / %3")
            .arg(root.shortEndpoint(root.model.sequencerUrl))
            .arg(root.shortEndpoint(root.model.indexerUrl))
            .arg(root.shortEndpoint(root.model.nodeUrl))
    }

    function normalizeEndpoint(value) {
        return String(value || "").trim().replace(/\/+$/, "")
    }

    function shortEndpoint(value) {
        const text = String(value || "")
        if (!text.length) {
            return qsTr("Not configured")
        }
        return text.replace(/^https?:\/\//, "").replace(/\/$/, "")
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

    component ProfileComboBox: ComboBox {
        id: comboRoot

        required property Theme theme
        property ListModel options
        signal profileActivated(int index)

        model: comboRoot.options
        textRole: "label"
        valueRole: "key"
        hoverEnabled: true
        implicitHeight: comboRoot.theme.controlHeight
        Accessible.role: Accessible.ComboBox
        Accessible.name: qsTr("Network profile")
        onActivated: index => comboRoot.profileActivated(index)

        contentItem: Text {
            text: comboRoot.displayText
            color: comboRoot.enabled ? comboRoot.theme.text : comboRoot.theme.textDim
            textFormat: Text.PlainText
            font.pixelSize: comboRoot.theme.primaryText
            font.weight: Font.Medium
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
            leftPadding: 12
            rightPadding: 36
        }

        indicator: Text {
            x: comboRoot.width - width - 14
            y: (comboRoot.height - height) / 2
            text: "v"
            color: comboRoot.enabled ? comboRoot.theme.textMuted : comboRoot.theme.textDim
            textFormat: Text.PlainText
            font.pixelSize: comboRoot.theme.secondaryText
            font.weight: Font.DemiBold
        }

        background: Rectangle {
            radius: comboRoot.theme.radius
            color: comboRoot.hovered || comboRoot.activeFocus ? comboRoot.theme.surfaceRaised : comboRoot.theme.field
            border.width: comboRoot.activeFocus ? 2 : 1
            border.color: comboRoot.activeFocus ? comboRoot.theme.accent : comboRoot.theme.outlineMuted
        }

        delegate: ItemDelegate {
            id: delegateRoot

            required property int index
            required property string label
            required property string summary

            width: comboRoot.width
            implicitHeight: 54
            hoverEnabled: true
            highlighted: comboRoot.highlightedIndex === index

            contentItem: ColumnLayout {
                spacing: comboRoot.theme.gapTiny

                Text {
                    text: delegateRoot.label
                    color: delegateRoot.highlighted ? comboRoot.theme.selectedText : comboRoot.theme.text
                    textFormat: Text.PlainText
                    font.pixelSize: comboRoot.theme.secondaryText
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }

                Text {
                    text: delegateRoot.summary
                    color: delegateRoot.highlighted ? comboRoot.theme.selectedText : comboRoot.theme.textMuted
                    textFormat: Text.PlainText
                    font.pixelSize: comboRoot.theme.dataText
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }

            background: Rectangle {
                color: delegateRoot.highlighted ? comboRoot.theme.accent : (delegateRoot.hovered ? comboRoot.theme.hover : "transparent")
                radius: comboRoot.theme.radius
            }
        }

        popup: Popup {
            y: comboRoot.height + comboRoot.theme.gapTiny
            width: comboRoot.width
            implicitHeight: Math.min(contentItem.implicitHeight + 8, 296)
            padding: 4

            contentItem: ListView {
                clip: true
                implicitHeight: contentHeight
                model: comboRoot.popup.visible ? comboRoot.delegateModel : null
                currentIndex: comboRoot.highlightedIndex
            }

            background: Rectangle {
                color: comboRoot.theme.surfaceRaised
                radius: comboRoot.theme.radius
                border.width: 1
                border.color: comboRoot.theme.outline
            }
        }
    }

    component EndpointEditor: ColumnLayout {
        id: editorRoot

        required property Theme theme
        property string title: ""
        property string endpoint: ""
        property string status: ""
        property color statusColor: theme.textMuted
        signal endpointEdited(string value)

        spacing: editorRoot.theme.gapSmall
        Layout.fillWidth: true

        RowLayout {
            spacing: editorRoot.theme.gapSmall
            Layout.fillWidth: true

            Text {
                text: editorRoot.title
                color: editorRoot.theme.text
                textFormat: Text.PlainText
                font.pixelSize: editorRoot.theme.primaryText
                font.weight: Font.DemiBold
                elide: Text.ElideRight
                Layout.fillWidth: true
            }

            StatusPill {
                theme: editorRoot.theme
                text: editorRoot.status
                colorToken: editorRoot.statusColor
            }
        }

        FieldRow {
            theme: editorRoot.theme
            label: qsTr("URL")
            text: editorRoot.endpoint
            placeholderText: qsTr("Endpoint URL")
            onTextChanged: editorRoot.endpointEdited(text)
        }
    }

    component StatusPill: Rectangle {
        id: pillRoot

        required property Theme theme
        property string text: ""
        property color colorToken: theme.textMuted

        radius: pillRoot.theme.radius
        color: pillRoot.colorToken === pillRoot.theme.success ? pillRoot.theme.successMuted : (pillRoot.colorToken === pillRoot.theme.warning ? pillRoot.theme.warningMuted : pillRoot.theme.field)
        border.width: 1
        border.color: pillRoot.colorToken
        implicitWidth: pillText.implicitWidth + 18
        implicitHeight: 26

        Text {
            id: pillText

            anchors.centerIn: parent
            text: pillRoot.text.length ? pillRoot.text : qsTr("Unknown")
            color: pillRoot.colorToken === pillRoot.theme.textMuted ? pillRoot.theme.textMuted : pillRoot.theme.text
            textFormat: Text.PlainText
            font.pixelSize: pillRoot.theme.dataText
            font.weight: Font.DemiBold
        }
    }

    component StatusRow: Item {
        id: rowRoot

        required property Theme theme
        property var columns: []
        property color statusColor: theme.textMuted
        property bool header: false

        visible: !(rowRoot.header && root.width < 620)
        Layout.fillWidth: true
        implicitHeight: !rowRoot.visible ? 0 : (rowRoot.header ? 36 : (root.width < 620 ? Math.max(92, narrowBody.implicitHeight + 16) : Math.max(46, rowGrid.implicitHeight + 16)))

        Rectangle {
            anchors.fill: parent
            color: rowRoot.header ? rowRoot.theme.field : "transparent"
            border.width: 0
        }

        GridLayout {
            id: rowGrid

            visible: root.width >= 620
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            anchors.topMargin: rowRoot.header ? 0 : 8
            anchors.bottomMargin: rowRoot.header ? 0 : 8
            columns: 4
            columnSpacing: 10

            Repeater {
                model: 4

                RowLayout {
                    id: statusCell

                    required property int index

                    spacing: rowRoot.theme.gapSmall
                    Layout.preferredWidth: rowRoot.columnWidth(statusCell.index)
                    Layout.fillWidth: statusCell.index === 2 || statusCell.index === 3
                    Layout.alignment: rowRoot.header ? Qt.AlignVCenter : Qt.AlignTop

                    Rectangle {
                        visible: !rowRoot.header && statusCell.index === 1
                        color: rowRoot.statusColor
                        radius: 3
                        Layout.preferredWidth: 6
                        Layout.preferredHeight: 6
                        Layout.alignment: Qt.AlignVCenter
                    }

                    Text {
                        text: String(rowRoot.columns[statusCell.index] || "-")
                        color: rowRoot.textColor(statusCell.index)
                        textFormat: Text.PlainText
                        font.family: rowRoot.header || statusCell.index < 2 ? "" : "monospace"
                        font.pixelSize: rowRoot.header ? rowRoot.theme.labelText : rowRoot.theme.dataText
                        font.weight: rowRoot.header || statusCell.index === 0 ? Font.DemiBold : Font.Normal
                        font.capitalization: rowRoot.header ? Font.AllUppercase : Font.MixedCase
                        wrapMode: rowRoot.header ? Text.NoWrap : Text.WrapAnywhere
                        elide: rowRoot.header ? Text.ElideRight : Text.ElideNone
                        Layout.fillWidth: true
                    }
                }
            }
        }

        ColumnLayout {
            id: narrowBody

            visible: !rowRoot.header && root.width < 620
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            anchors.topMargin: 8
            anchors.bottomMargin: 8
            spacing: rowRoot.theme.gapSmall

            RowLayout {
                spacing: rowRoot.theme.gapSmall
                Layout.fillWidth: true

                Text {
                    text: String(rowRoot.columns[0] || "-")
                    color: rowRoot.theme.text
                    textFormat: Text.PlainText
                    font.pixelSize: rowRoot.theme.secondaryText
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }

                Rectangle {
                    color: rowRoot.statusColor
                    radius: 3
                    Layout.preferredWidth: 6
                    Layout.preferredHeight: 6
                    Layout.alignment: Qt.AlignVCenter
                }

                Text {
                    text: String(rowRoot.columns[1] || "-")
                    color: rowRoot.statusColor === rowRoot.theme.textMuted ? rowRoot.theme.textMuted : rowRoot.theme.text
                    textFormat: Text.PlainText
                    font.pixelSize: rowRoot.theme.dataText
                    font.weight: Font.DemiBold
                    Layout.alignment: Qt.AlignVCenter
                }
            }

            Text {
                text: String(rowRoot.columns[2] || "-")
                color: rowRoot.theme.textMuted
                textFormat: Text.PlainText
                font.family: "monospace"
                font.pixelSize: rowRoot.theme.dataText
                wrapMode: Text.WrapAnywhere
                Layout.fillWidth: true
            }

            Text {
                text: String(rowRoot.columns[3] || "-")
                color: rowRoot.theme.textMuted
                textFormat: Text.PlainText
                font.family: "monospace"
                font.pixelSize: rowRoot.theme.dataText
                wrapMode: Text.WrapAnywhere
                Layout.fillWidth: true
            }
        }

        function textColor(index) {
            if (rowRoot.header) {
                return rowRoot.theme.textMuted
            }
            if (index === 1) {
                return rowRoot.statusColor === rowRoot.theme.textMuted ? rowRoot.theme.textMuted : rowRoot.theme.text
            }
            return index === 0 ? rowRoot.theme.text : rowRoot.theme.textMuted
        }

        function columnWidth(index) {
            if (index === 0) {
                return 132
            }
            if (index === 1) {
                return 92
            }
            return 220
        }
    }
}
