import QtQml
import QtTest
import "../../qml/services"
import "../../qml/state"
import "fixtures"

TestCase {
    id: testRoot

    name: "SourceInspectionSession"

    BridgeHostFixture {
        id: fakeHost
    }

    AsyncBridgeHostFixture {
        id: asyncHost
    }

    BridgeClient {
        id: bridgeClient

        host: fakeHost
    }

    AppModel {
        id: model

        bridge: bridgeClient
    }

    QtObject {
        id: theme

        property string textMuted: "#777777"
        property string error: "#cc3333"
        property string success: "#228844"
    }

    SourceInspectionSession {
        id: storageSession

        model: model
        theme: theme
        family: "storage"
    }

    SourceInspectionSession {
        id: deliverySession

        model: model
        theme: theme
        family: "delivery"
    }

    function init() {
        fakeHost.reset()
        asyncHost.reset()
        bridgeClient.host = fakeHost
        model.setNetworkConnectorMode("storage", "rest")
        model.setNetworkConnectorMode("delivery", "rest")
        model.metrics.observationTimeoutMs = 45000
        model.storageCidProbe = ""
        model.metrics.storageModuleReport = null
        model.metrics.messagingModuleReport = null
        model.metrics.storageSourceReport = null
        model.metrics.messagingSourceReport = null
        model.metrics.networkConnectionStatus = ({})
        model.metrics.networkConnectionStatusRevision += 1
        model.metrics.networkConnectionPending = ({})
        model.metrics.networkConnectionPendingRevision += 1
        model.metrics.activeObservationLeases = ({})
        model.metrics.observationWaiters = ({})
    }

    function test_family_views_expose_complete_page_contracts() {
        verify(Array.isArray(storageSession.view.healthRows))
        verify(Array.isArray(storageSession.view.activeOperationRows))
        verify(Array.isArray(storageSession.view.networkDebugRows))
        verify(Array.isArray(storageSession.view.capacityRows))
        verify(Array.isArray(storageSession.view.cidRows))
        compare(storageSession.view.sourceShortLabel, "REST")

        verify(Array.isArray(deliverySession.view.healthRows))
        verify(Array.isArray(deliverySession.view.protocolRows))
        verify(Array.isArray(deliverySession.view.throughputRows))
        verify(Array.isArray(deliverySession.view.topicRows))
        verify(Array.isArray(deliverySession.view.topicDetailRows))
        compare(deliverySession.view.sourceShortLabel, "REST")
    }

    function test_storage_view_reacts_to_selected_cid() {
        compare(storageSession.view.cidRows.length, 2)

        model.storageCidProbe = "z-test"

        tryCompare(storageSession.view.cidRows, "length", 5)
        compare(storageSession.view.cidRows[0].copyText, "z-test")
    }

    function test_storage_exists_result_is_not_relabeled_after_cid_edit() {
        model.storageCidProbe = "cid-a"
        fakeHost.responses = ({
            storageSourceReport: {
                ok: true,
                value: {
                    health: {
                        ready: true,
                        status: "healthy",
                        summary: "source ready",
                        detail: "ready"
                    },
                    probes: [{
                        probe_key: "exists",
                        label: "storage_module.exists",
                        ok: true,
                        value: false,
                        error: null
                    }]
                },
                text: "OK",
                error: ""
            }
        })

        storageSession.refresh(false, true)

        tryVerify(function () {
            return model.metrics.sourceReport("storage") !== null
        })
        compare(fakeHost.lastArgs[0].options.cid, "cid-a")
        verify(model.metrics.reportProbe(
            model.metrics.sourceReport("storage"), "exists") !== null)
        tryCompare(storageSession.view.cidRows[1], "value", "false")

        model.storageCidProbe = "cid-b"

        tryCompare(storageSession.view.cidRows[0], "copyText", "cid-b")
        compare(storageSession.view.cidRows[1].value, "Not queried")
    }

    function test_configured_source_report_never_falls_back_to_module_report() {
        model.metrics.setModuleReport("storage", {
            marker: "stale-module",
            probes: [{
                probe_key: "space",
                label: "storage_module.space",
                ok: true,
                value: { marker: "module-space" },
                error: null
            }]
        })
        model.metrics.setSourceReport("storage", {
            marker: "fresh-source",
            health: {
                ready: false,
                status: "degraded",
                summary: "source degraded",
                detail: "capacity unavailable"
            },
            probes: [{
                probe_key: "space",
                label: "storage_rest.space",
                ok: true,
                value: { marker: "source-space" },
                error: null
            }]
        }, { origin: "test" })
        model.metrics.networkConnectionStatus = ({
            storage: {
                known: true,
                ok: false,
                transportOk: true,
                text: "Error",
                detail: "source degraded",
                checkedAt: "10:00:00",
                stale: false
            }
        })
        model.metrics.networkConnectionStatusRevision += 1

        compare(storageSession.view.report.marker, "fresh-source")
        compare(storageSession.view.capacitySummary, "1 field(s)")
        compare(storageSession.view.status.ok, false)
        compare(storageSession.view.healthText, "Problem")
    }

    function test_refresh_populates_configured_source_report() {
        fakeHost.responses = ({
            storageSourceReport: {
                ok: true,
                value: {
                    marker: "refreshed-source",
                    health: {
                        ready: true,
                        status: "healthy",
                        summary: "source ready",
                        detail: "ready"
                    },
                    probes: []
                },
                text: "OK",
                error: ""
            }
        })

        storageSession.refresh(false, false)

        tryVerify(function () {
            return storageSession.view.report
                && storageSession.view.report.marker === "refreshed-source"
        })
        verify(!storageSession.view.pending)
        verify(storageSession.view.status.ok)
        compare(fakeHost.lastArgs[0].configuration_generation,
            model.metrics.familyConfigurationGeneration("storage"))
    }

    function test_delivery_refresh_times_out_when_async_callback_is_lost() {
        bridgeClient.host = asyncHost
        asyncHost.deferAsyncRequests = true
        model.metrics.observationTimeoutMs = 1

        deliverySession.refresh(true, false)

        tryCompare(asyncHost.pendingAsyncRequests, "length", 1)
        compare(asyncHost.lastMethod, "deliverySourceReport")
        verify(deliverySession.view.pending)
        tryVerify(function () { return !deliverySession.view.pending }, 500)
        verify(deliverySession.view.status.known)
        verify(!deliverySession.view.status.ok)
        compare(deliverySession.view.status.detail, "Source observation timed out.")
        verify(model.shell.resultIsError)

        verify(asyncHost.completeAsyncAt(0, {
            ok: true,
            value: { marker: "late" },
            text: "OK",
            error: ""
        }))
        compare(deliverySession.view.status.detail, "Source observation timed out.")
        verify(model.shell.resultIsError)
    }

    function test_hard_failure_marks_retained_report_last_known() {
        const reportCheckedAtMs = Date.UTC(2026, 0, 2, 8, 30, 0)
        const reportCheckedAt = model.metrics.observationTimeText(reportCheckedAtMs)
        model.metrics.setSourceReport("storage", {
            marker: "last-known",
            health: {
                ready: true,
                status: "healthy",
                summary: "source ready",
                detail: "ready"
            },
            probes: []
        }, { origin: "test", checkedAtMs: reportCheckedAtMs })
        model.metrics.networkConnectionStatus = ({
            storage: {
                known: true,
                ok: false,
                transportOk: false,
                text: "Error",
                detail: "transport down",
                checkedAt: "10:01:00",
                stale: true
            }
        })
        model.metrics.networkConnectionStatusRevision += 1

        compare(storageSession.view.report.marker, "last-known")
        verify(storageSession.view.statusLine.indexOf("last completed report") >= 0)
        verify(storageSession.view.statusLine.indexOf(reportCheckedAt) >= 0)
        verify(storageSession.view.statusLine.indexOf("10:01:00") >= 0)
        verify(storageSession.view.freshnessText.indexOf(reportCheckedAt) >= 0)
        verify(storageSession.view.sourceBadges.indexOf("last known") >= 0)
    }
}
