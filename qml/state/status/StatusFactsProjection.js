.pragma library

.import "../metrics/DashboardMetricCatalog.js" as DashboardMetricCatalog

function dashboardGraphKeys() {
    return DashboardMetricCatalog.dashboardGraphKeys()
}

function selectedDashboardGraphItems(model) {
    return DashboardMetricCatalog.selectedDashboardGraphItems(model.metrics)
}

function dashboardGraphItem(model, key) {
    return DashboardMetricCatalog.dashboardGraphItem(model.metrics, key)
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
    return DashboardMetricCatalog.dashboardMetricText(model.metrics, value)
}
