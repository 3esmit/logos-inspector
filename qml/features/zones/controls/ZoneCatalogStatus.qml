pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../theme"
import "../ZonePresentation.js" as Presentation

Rectangle {
    id: root

    required property Theme theme
    required property var zoneState
    readonly property bool compactGrid: width < 680
    readonly property string tone: Presentation.catalogTone(
        root.zoneState && root.zoneState.verification,
        root.zoneState && root.zoneState.coverage
    )
    readonly property var facts: root.statusFacts()

    objectName: "zoneCatalogStatus"
    implicitHeight: statusGrid.implicitHeight + root.theme.gapLarge
        + (catalogError.visible ? catalogError.implicitHeight + root.theme.gapSmall : 0)
    radius: root.theme.radius
    color: root.theme.surface
    border.width: 1
    border.color: root.theme.outlineMuted

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: root.theme.gapSmall
        spacing: root.theme.gapSmall

        GridLayout {
            id: statusGrid

            columns: root.compactGrid ? 2 : 4
            columnSpacing: 0
            rowSpacing: root.theme.gapSmall
            Layout.fillWidth: true

            Repeater {
                model: root.facts

                Item {
                    id: factItem

                    required property var modelData
                    required property int index

                    implicitHeight: 40
                    Layout.fillWidth: true
                    Layout.minimumWidth: 110

                    Rectangle {
                        visible: factItem.index % statusGrid.columns !== 0
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.bottom: parent.bottom
                        width: 1
                        color: root.theme.outlineMuted
                    }

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: factItem.index % statusGrid.columns === 0 ? root.theme.gapTiny : root.theme.gap
                        anchors.rightMargin: root.theme.gap
                        spacing: root.theme.gapSmall

                        ToneDot {
                            theme: root.theme
                            tone: factItem.modelData.tone
                            Layout.preferredWidth: 7
                            Layout.preferredHeight: 7
                            Layout.alignment: Qt.AlignTop
                            Layout.topMargin: 5
                        }

                        ColumnLayout {
                            spacing: 0
                            Layout.fillWidth: true

                            Text {
                                text: factItem.modelData.label
                                color: root.theme.textDim
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                font.pixelSize: root.theme.labelText
                                Layout.fillWidth: true
                            }

                            Text {
                                text: factItem.modelData.value
                                color: root.theme.text
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                font.pixelSize: root.theme.secondaryText
                                font.weight: Font.DemiBold
                                Layout.fillWidth: true
                            }
                        }
                    }
                }
            }
        }

        Text {
            id: catalogError

            visible: text.length > 0
            text: String(root.zoneState && (root.zoneState.currentError
                || root.zoneState.statusError
                || root.zoneState.configureError) || "")
            color: root.theme.error
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: root.theme.dataText
            Layout.fillWidth: true
        }
    }

    function statusFacts() {
        const state = root.zoneState || ({})
        const coverage = state.coverage || ({})
        const ingestion = state.ingestion || ({})
        const floor = coverage.coverage_floor === undefined || coverage.coverage_floor === null
            ? "-" : Presentation.numberText(coverage.coverage_floor)
        const scanned = coverage.scanned_through_slot === undefined || coverage.scanned_through_slot === null
            ? "-" : Presentation.numberText(coverage.scanned_through_slot)
        return [{
            label: qsTr("Catalog verification"),
            value: Presentation.words(state.verification),
            tone: root.tone
        }, {
            label: qsTr("Coverage"),
            value: qsTr("%1 / prefix %2")
                .arg(Presentation.words(coverage.status))
                .arg(Presentation.words(coverage.prefix_status)),
            tone: root.tone
        }, {
            label: qsTr("Finalized range"),
            value: qsTr("%1 - %2").arg(floor).arg(scanned),
            tone: Number(coverage.gap_count || 0) > 0 ? "warning" : root.tone
        }, {
            label: qsTr("Catalog facts"),
            value: qsTr("%1 Zones / %2 gaps")
                .arg(Presentation.numberText(ingestion.discovered_zone_count))
                .arg(Presentation.numberText(coverage.gap_count)),
            tone: Number(coverage.gap_count || 0) > 0 ? "warning" : root.tone
        }]
    }
}
