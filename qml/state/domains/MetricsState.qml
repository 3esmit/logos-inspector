import QtQml
import "../metrics/AppModelMetrics.js" as AppModelMetrics

QtObject {
    id: root

    required property var model

    function dashboardMetricValue(key) {
        return AppModelMetrics.dashboardMetricValue(model, key)
    }

    function dashboardMetricText(key) {
        return AppModelMetrics.dashboardMetricText(model, key)
    }

    function openMetricValue(kind, names) {
        return AppModelMetrics.openMetricValue(model, kind, names)
    }

    function moduleReport(kind) {
        return AppModelMetrics.moduleReport(model, kind)
    }

    function moduleProbeValue(kind, method) {
        return AppModelMetrics.moduleProbeValue(model, kind, method)
    }

    function defaultFooterFieldSelections() {
        return AppModelMetrics.defaultFooterFieldSelections(model)
    }
}
