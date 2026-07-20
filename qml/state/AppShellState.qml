import QtQml
import "app/AppModelCore.js" as AppModelCore

QtObject {
    id: root

    required property var model

    property string currentView: "overview"
    property string statusText: qsTr("Ready")
    property bool busy: false
    property string resultTitle: qsTr("Output")
    property string resultText: ""
    property var resultValue: null
    property bool resultIsError: false
    property string resultOwner: ""
    property int resultGeneration: 0
    property var navExpanded: ({ l1: true, zones: true, network: true, diagnostics: false, local: true, system: true })
    property int navRevision: 0
    property var zoneMenuSelections: ({})
    property int zoneMenuRevision: 0
    property var navigationBackStack: []
    property var navigationForwardStack: []
    property int navigationRevision: 0
    property bool navigationRestoring: false
    readonly property int navigationHistoryLimit: 80
    property string settingsSection: "general"
    property string settingsNetworkSection: "blockchain"
    property string settingsUiSection: "footer"

    function navTreeItems() { return AppModelCore.navTreeItems(model) }
    function navRows() { return AppModelCore.navRows(model) }
    function navGroupExpanded(key) { return AppModelCore.navGroupExpanded(model, key) }
    function toggleNavGroup(key) { return AppModelCore.toggleNavGroup(model, key) }
    function expandNavGroupForView(view) { return AppModelCore.expandNavGroupForView(model, view) }
    function parentNavKeyForView(view) { return AppModelCore.parentNavKeyForView(model, view) }
    function navItemForView(view) { return AppModelCore.navItemForView(model, view) }
    function layerForView(view) { return AppModelCore.layerForView(model, view) }
    function navLabelForView(view) { return AppModelCore.navLabelForView(model, view) }
    function navTokenForView(view) { return AppModelCore.navTokenForView(model, view) }
    function navItemForQuery(query) { return AppModelCore.navItemForQuery(model, query) }
    function navItemMatches(item, normalized) { return AppModelCore.navItemMatches(model, item, normalized) }
    function zoneMenuEnabled(key) { return AppModelCore.zoneMenuEnabled(model, key) }
    function setZoneMenuEnabled(key, enabled) { return AppModelCore.setZoneMenuEnabled(model, key, enabled) }
    function zoneMenuGroups() { return AppModelCore.zoneMenuGroups(model) }
    function viewTitle() { return AppModelCore.viewTitle(model) }
    function normalizedNavigationView(view) { return AppModelCore.normalizedNavigationView(model, view) }
    function navigationSnapshot() { return AppModelCore.navigationSnapshot(model) }
    function pushNavigationHistory() { return AppModelCore.pushNavigationHistory(model) }
    function restoreNavigationSnapshot(snapshot) { return AppModelCore.restoreNavigationSnapshot(model, snapshot) }
    function canNavigateBack() { return AppModelCore.canNavigateBack(model) }
    function canNavigateForward() { return AppModelCore.canNavigateForward(model) }
    function navigateBack() { return AppModelCore.navigateBack(model) }
    function navigateForward() { return AppModelCore.navigateForward(model) }
    function navigationBackLabel() { return AppModelCore.navigationBackLabel(model) }
    function navigationForwardLabel() { return AppModelCore.navigationForwardLabel(model) }
    function selectView(view, recordHistory) { return AppModelCore.selectView(model, view, recordHistory) }
    function openSettings(section, subsection, recordHistory) { return AppModelCore.openSettings(model, section, subsection, recordHistory) }
    function clearResult() { return AppModelCore.clearResult(model) }
    function setResult(title, text, isError, value, owner) { return AppModelCore.setResult(model, title, text, isError, value, owner) }
    function pageHasOutput(view) { return AppModelCore.pageHasOutput(model, view) }
}
