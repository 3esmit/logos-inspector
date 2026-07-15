import QtQml
import "source_routing/SourceDiagnosticsProjection.js" as SourceDiagnostics
import "source_routing/SourceInspectionReadModel.js" as SourceInspectionReadModel

QtObject {
    id: root

    required property var model
    required property var theme
    required property string family

    readonly property var view: SourceInspectionReadModel.build(model, theme, family)

    function refresh(showResult, includeCidProbe) {
        const networkKind = family === "storage" ? "storage" : "messaging"
        return model.metrics.queryNetworkConnection(
            networkKind,
            showResult === true,
            includeCidProbe === true,
            "source-inspection"
        )
    }

    function diagnosticsGateDetailText(gate, fallbackLabel) {
        return SourceDiagnostics.diagnosticsGateDetailText(gate, fallbackLabel)
    }

}
