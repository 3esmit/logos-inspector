pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../state"

SourceSettingsPanel {
    id: root

    property real pageWidth: 900
    property AppModel modelRef
    property var sourceOptions

    busy: root.modelRef ? root.modelRef.shell.busy : false
    queryAccessibleName: qsTr("Query Storage status")

    StatusMessage {
        visible: root.sourceSettingsLocked()
        theme: root.theme
        tone: "warning"
        title: qsTr("Storage source locked")
        message: qsTr("Connector, endpoint, network preset, and network debug settings stay locked until the Storage operation finishes or cancellation is confirmed. Manage it from Storage > Operations.")
        Layout.fillWidth: true
    }

    SourceSettingsGrid {
        theme: root.theme
        pageWidth: root.pageWidth

        ComboField {
            theme: root.theme
            label: qsTr("Connector")
            accessibleName: qsTr("Storage connector")
            options: root.sourceOptions
            currentIndex: root.sourceIndexFor(root.modelRef.currentConnectorSourceMode("storage", "rest"))
            enabled: !root.sourceSettingsLocked()
            onActivated: index => root.modelRef.setNetworkConnectorMode("storage", root.sourceModeAt(index))
        }

        FieldRow {
            visible: root.storageRestEnabled()
            theme: root.theme
            label: qsTr("REST URL")
            sourceText: root.modelRef.sourceRouting.configuredStorageRestUrl()
            syncSourceText: true
            placeholderText: qsTr("http://127.0.0.1:8080/api/storage/v1")
            enabled: !root.sourceSettingsLocked()
            onTextEdited: text => root.modelRef.setNetworkConnectorEndpoint("storage", text)
        }

        FieldRow {
            visible: root.storageMetricsEnabled()
            theme: root.theme
            label: qsTr("Metrics URL")
            sourceText: root.modelRef.sourceRouting.configuredStorageMetricsUrl()
            syncSourceText: true
            placeholderText: qsTr("http://127.0.0.1:8008/metrics")
            enabled: !root.sourceSettingsLocked()
            onTextEdited: text => {
                const value = String(text || "").trim()
                if (root.storageSourceMode() === "metrics") {
                    root.modelRef.setNetworkConnectorEndpoint("storage", value)
                } else {
                    root.modelRef.storageMetricsUrl = value
                }
            }
        }

        FieldRow {
            theme: root.theme
            label: qsTr("Network preset")
            sourceText: root.modelRef.storageNetworkPreset
            syncSourceText: true
            placeholderText: qsTr("logos.test")
            enabled: !root.sourceSettingsLocked()
            onTextEdited: text => root.modelRef.storageNetworkPreset = String(text || "").trim()
        }

        FieldRow {
            visible: root.storageDataEnabled()
            theme: root.theme
            label: qsTr("CID local exists")
            sourceText: root.modelRef.storageCidProbe
            syncSourceText: true
            placeholderText: qsTr("Optional CID")
            onTextEdited: text => root.modelRef.storageCidProbe = String(text || "").trim()
        }

        RefreshRateField {
            theme: root.theme
            accessibleName: qsTr("Storage auto refresh")
            accessibleDescription: qsTr("Automatic Storage status refresh interval in seconds. Set to 0 to turn it off.")
            value: root.modelRef.metrics.storageRefreshRate
            onRateEdited: value => root.modelRef.metrics.setNetworkConnectionRate("storage", value)
        }

        SecondsField {
            theme: root.theme
            label: qsTr("Rolling window")
            accessibleName: qsTr("Storage rolling window")
            accessibleDescription: qsTr("Storage metrics rolling window in seconds.")
            value: root.modelRef.storageRollingWindow
            onValueEdited: value => root.modelRef.storageRollingWindow = value
        }
    }

    Flow {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        SafetyToggle {
            theme: root.theme
            text: qsTr("Show local paths")
            detail: qsTr("Shows full local storage paths in diagnostics and enables their Copy actions.")
            checked: root.modelRef.storageLocalDiagnosticsEnabled
            onToggled: root.modelRef.storageLocalDiagnosticsEnabled = checked
        }

        SafetyToggle {
            visible: root.storageNetworkDebugAvailable()
            theme: root.theme
            text: qsTr("Include network debug details")
            detail: qsTr("Queries peer identity, addresses, public records, and the DHT routing table during Storage status checks. Read-only; may expose network topology.")
            checked: root.modelRef.storagePrivilegedDebugEnabled
            enabled: !root.sourceSettingsLocked()
            onToggled: root.modelRef.storagePrivilegedDebugEnabled = checked
        }
    }

    StatusMessage {
        visible: root.storageSourceMode() === "unsupported"
        theme: root.theme
        tone: "warning"
        title: qsTr("Source unavailable")
        message: root.unavailableSourceMessage()
        Layout.fillWidth: true
    }

    function sourceIndexFor(value) {
        return root.modelRef.sourceRouting.sourceModeIndexFor("storage", value, root.sourceOptions)
    }

    function sourceModeAt(index) {
        return root.modelRef.sourceRouting.sourceModeAt(index, root.sourceOptions)
    }

    function storageSource() {
        return root.modelRef.sourceRouting.storageSourceView()
    }

    function storageSourceMode() {
        return root.storageSource().effectiveMode
    }

    function storageRestEnabled() {
        return root.storageSource().usesRestEndpoint
    }

    function storageMetricsEnabled() {
        return root.storageSource().usesMetricsEndpoint
    }

    function storageDataEnabled() {
        return root.storageSource().supportsCidProbe
    }

    function storageNetworkDebugAvailable() {
        return root.modelRef
            && root.modelRef.sourceRouting.storageSourceSupportsNetworkDebug()
    }

    function sourceSettingsLocked() {
        return root.modelRef && root.modelRef.storageApp.sourceSettingsLocked
    }

    function unavailableSourceMessage() {
        const choices = root.sourceChoiceText()
        return choices.length
            ? qsTr("The configured connector no longer has an adapter. Select %1.").arg(choices)
            : qsTr("The configured connector no longer has an adapter. Select another available connector.")
    }

    function sourceChoiceText() {
        const labels = []
        const options = root.sourceOptions
        const count = options && options.count !== undefined
            ? Number(options.count)
            : (Array.isArray(options) ? options.length : 0)
        for (let i = 0; i < count; ++i) {
            const option = options && typeof options.get === "function"
                ? options.get(i)
                : options[i]
            const label = String(option && option.label || "").trim()
            if (label.length) {
                labels.push(label)
            }
        }
        if (labels.length < 2) {
            return labels.length ? labels[0] : ""
        }
        if (labels.length === 2) {
            return qsTr("%1 or %2").arg(labels[0]).arg(labels[1])
        }
        const last = labels.pop()
        return qsTr("%1, or %2").arg(labels.join(", ")).arg(last)
    }
}
