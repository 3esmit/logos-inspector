import QtQuick
import QtTest
import "../../qml/state/modules/ModuleReportPresentation.js" as ModuleReportPresentation

TestCase {
    id: testRoot

    name: "ModuleReportPresentation"

    QtObject {
        id: theme

        property string textMuted: "muted"
        property string warning: "warning"
        property string success: "success"
        property string error: "error"
    }

    QtObject {
        id: model

        property QtObject shell: QtObject {
            property string resultTitle: "Storage report"
            property bool resultIsError: false
            property string resultText: ""
        }
        property string nodeUrl: "http://127.0.0.1:4000"
        property string storageModule: "logos_storage"
        property string deliveryModule: "logos_delivery"
        property string capabilityModule: "logos_capabilities"
        property string blockchainModule: "logos_blockchain"
        property string inspectorModule: "logos_inspector"
        property QtObject metrics: QtObject {
            function moduleProbe(kind, method) {
                return { ok: true, value: "peer-id", probe_key: method }
            }
            function scalarValue(value) { return value }
        }
    }

    QtObject {
        id: root

        property var model: model
        property var theme: theme
        property string moduleKind: "storage"
        property bool hasResponse: true
        property var responseValue: null
        property var responseProbeModel: ModuleReportPresentation.responseProbeRows(root)

        function numberText(value) { return String(value) }
        function valueText(value) { return String(value) }
        function valueSummary(value) { return typeof value === "object" ? "fields" : String(value) }
        function endpointLabel(value) { return String(value).indexOf("127.0.0.1") >= 0 ? "Local" : "Custom" }
        function shortEndpoint(value) { return String(value).replace(/^https?:\/\//, "") }
    }

    function test_probe_rows_extract_module_report_facts() {
        root.responseValue = {
            module: "logos_storage",
            probe_facts: [
                { ok: true, label: "status", value: "ok", source: "module" },
                { ok: false, label: "exists", error: "missing", source: "module" }
            ]
        }

        const rows = ModuleReportPresentation.responseProbeRows(root)
        compare(rows.length, 2)
        compare(rows[0].label, "Storage / status")
        compare(rows[1].detail, "missing")
        compare(ModuleReportPresentation.responseStatusText(root), "Partial")
    }

    function test_response_cards_use_presentation_policy() {
        root.moduleKind = "messaging"
        root.responseValue = { endpoint: "http://127.0.0.1:9000", a: 1, b: 2 }

        compare(ModuleReportPresentation.moduleLabel(root, "messaging"), "Messaging")
        compare(ModuleReportPresentation.responsePayloadText(root), "3")
        compare(ModuleReportPresentation.responseTargetText(root), "Local")
        compare(ModuleReportPresentation.moduleProbeDelta(root), "Default probe plan")
    }

    function test_blockchain_peer_id_projection_falls_back_to_model_probe() {
        root.moduleKind = "blockchain"
        root.responseValue = null

        compare(ModuleReportPresentation.blockchainPeerIdText(root), "peer-id")
        compare(ModuleReportPresentation.blockchainPeerIdCopyText(root), "peer-id")
    }
}
