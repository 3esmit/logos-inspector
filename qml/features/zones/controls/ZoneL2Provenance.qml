pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../theme"
import "../ZonePresentation.js" as Presentation

GridLayout {
    id: root

    required property Theme theme
    property var source: null
    property var route: null
    property string routeCompleteness: ""
    readonly property bool singleColumn: width < 620

    objectName: "zoneL2Provenance"
    visible: root.source !== null || root.route !== null
    columns: root.singleColumn ? 1 : 2
    columnSpacing: root.theme.gapXLarge
    rowSpacing: root.theme.gapLarge
    Layout.fillWidth: true

    ZoneFactSection {
        visible: root.source !== null
        theme: root.theme
        title: qsTr("Source Provenance")
        rows: root.sourceRows()
    }

    ZoneFactSection {
        visible: root.route !== null
        theme: root.theme
        title: qsTr("Read Route")
        rows: root.routeRows()
    }

    function sourceRows() {
        const value = root.source || ({})
        return [{
            label: qsTr("Source ID"),
            value: Presentation.text(value.source_id),
            copyable: true,
            monospace: true
        }, {
            label: qsTr("Role"),
            value: Presentation.words(value.source_role)
        }, {
            label: qsTr("Finality"),
            value: Presentation.words(value.finality),
            tone: String(value.finality || "") === "finalized" ? "success" : "warning"
        }, {
            label: qsTr("Retrieval"),
            value: Presentation.words(value.retrieval)
        }, {
            label: qsTr("Config revision"),
            value: Presentation.numberText(value.source_config_revision)
        }]
    }

    function routeRows() {
        const value = root.route || ({})
        const attempts = Array.isArray(value.attempts) ? value.attempts : []
        return [{
            label: qsTr("Policy"),
            value: Presentation.words(value.policy)
        }, {
            label: qsTr("Completeness"),
            value: Presentation.words(root.routeCompleteness),
            tone: root.routeCompleteness === "degraded" ? "warning" : "success"
        }, {
            label: qsTr("Attempts"),
            value: root.attemptSummary(attempts)
        }]
    }

    function attemptSummary(attempts) {
        if (!attempts.length) {
            return "-"
        }
        return attempts.map(function (attempt) {
            return qsTr("%1 %2")
                .arg(Presentation.words(attempt && attempt.source_role))
                .arg(Presentation.words(attempt && attempt.outcome))
        }).join(qsTr(" / "))
    }
}
