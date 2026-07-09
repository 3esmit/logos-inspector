.pragma library

.import "../metrics/DashboardMetricCatalog.js" as DashboardMetricCatalog

function dashboardGraphKeys() {
    return DashboardMetricCatalog.dashboardGraphKeys()
}

function selectedDashboardGraphItems(model) {
    return DashboardMetricCatalog.selectedDashboardGraphItems(model)
}

function dashboardGraphItem(model, key) {
    return DashboardMetricCatalog.dashboardGraphItem(model, key)
}

function dashboardMetricTone(key, numeric) {
    return DashboardMetricCatalog.dashboardMetricTone(key, numeric)
}

function dashboardMetricGroup(key) {
    return DashboardMetricCatalog.dashboardMetricGroup(key)
}

function dashboardMetricLabel(key) {
    return DashboardMetricCatalog.dashboardMetricLabel(key)
}

function dashboardMetricText(model, value) {
    return DashboardMetricCatalog.dashboardMetricText(model, value)
}
