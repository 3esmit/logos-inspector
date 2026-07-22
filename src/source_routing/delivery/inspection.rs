use serde_json::Value;

use super::{
    layer::{self, MessagingAdapter},
    transport,
};
use crate::{
    ProbeReport,
    modules::{ModuleReport, logos_core::SharedModuleTransport},
    source_routing::{
        AdapterConnectionType, DeliverySourceReportKind, SourceProbeKey, SourceReport,
        shared::{
            evidence::SourceEvidence,
            http,
            report::{
                MetricsProbeSpec, SourceReportBuilder, SourceReportKind, keyed_probe_result,
                source_text_metrics_report, unsupported_source_report,
            },
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeliveryProbeNormalizer {
    Identity,
    Health,
    Info,
    Version,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeliveryProbeStep {
    key: SourceProbeKey,
    label: &'static str,
    path: String,
    normalizer: DeliveryProbeNormalizer,
    response_limit: usize,
}

impl DeliveryProbeStep {
    fn new(
        key: SourceProbeKey,
        label: &'static str,
        path: impl Into<String>,
        normalizer: DeliveryProbeNormalizer,
    ) -> Self {
        Self {
            key,
            label,
            path: path.into(),
            normalizer,
            response_limit: transport::DELIVERY_PROBE_RESPONSE_LIMIT,
        }
    }

    fn with_response_limit(mut self, response_limit: usize) -> Self {
        self.response_limit = response_limit;
        self
    }
}

pub async fn delivery_source_report(
    source_mode: &str,
    rest_endpoint: Option<&str>,
    metrics_endpoint: Option<&str>,
    runtime_diagnostics_enabled: bool,
    module_transport: &SharedModuleTransport,
) -> SourceReport {
    delivery_source_report_with_runtime_metrics(
        source_mode,
        rest_endpoint,
        metrics_endpoint,
        runtime_diagnostics_enabled,
        false,
        module_transport,
    )
    .await
}

pub(crate) async fn delivery_source_report_with_runtime_metrics(
    source_mode: &str,
    rest_endpoint: Option<&str>,
    metrics_endpoint: Option<&str>,
    runtime_diagnostics_enabled: bool,
    runtime_metrics_enabled: bool,
    module_transport: &SharedModuleTransport,
) -> SourceReport {
    match MessagingAdapter::select(source_mode, rest_endpoint, metrics_endpoint) {
        MessagingAdapter::Module { transport } => module_source_report(
            SourceReportKind::Delivery(DeliverySourceReportKind::Module),
            layer::module_report(
                module_transport,
                transport,
                None,
                runtime_diagnostics_enabled,
                runtime_metrics_enabled,
            )
            .await,
        ),
        MessagingAdapter::Rest {
            endpoint,
            metrics_endpoint,
        } => delivery_rest_report(endpoint, metrics_endpoint).await,
        MessagingAdapter::Metrics { endpoint } => delivery_metrics_report(endpoint).await,
        MessagingAdapter::NetworkMonitor {
            endpoint,
            metrics_endpoint,
        } => delivery_network_monitor_report(endpoint, metrics_endpoint).await,
        MessagingAdapter::Unsupported { mode } => unsupported_delivery_source_report(mode),
    }
}

fn delivery_rest_probe_plan() -> Vec<DeliveryProbeStep> {
    vec![
        DeliveryProbeStep::new(
            SourceProbeKey::DeliveryHealth,
            "delivery_rest.health",
            "/health",
            DeliveryProbeNormalizer::Health,
        ),
        DeliveryProbeStep::new(
            SourceProbeKey::DeliveryInfo,
            "delivery_rest.info",
            "/info",
            DeliveryProbeNormalizer::Info,
        ),
        DeliveryProbeStep::new(
            SourceProbeKey::DeliveryVersion,
            "delivery_rest.version",
            "/version",
            DeliveryProbeNormalizer::Version,
        ),
    ]
}

fn delivery_network_monitor_probe_plan() -> Vec<DeliveryProbeStep> {
    vec![
        DeliveryProbeStep::new(
            SourceProbeKey::DeliveryAllPeersInfo,
            "delivery_network_monitor.allPeersInfo",
            "/allpeersinfo",
            DeliveryProbeNormalizer::Identity,
        )
        .with_response_limit(transport::DELIVERY_PEER_RESPONSE_LIMIT),
        DeliveryProbeStep::new(
            SourceProbeKey::DeliveryContentTopics,
            "delivery_network_monitor.contentTopics",
            "/contenttopics",
            DeliveryProbeNormalizer::Identity,
        ),
    ]
}

fn module_source_report(kind: SourceReportKind, report: ModuleReport) -> SourceReport {
    let adapter = match report.adapter {
        crate::modules::logos_core::ModuleTransportKind::Module => AdapterConnectionType::Module,
        crate::modules::logos_core::ModuleTransportKind::LogoscoreCli => {
            AdapterConnectionType::LogoscoreCli
        }
    };
    let evidence =
        SourceEvidence::new(report.module, report.module_info, report.probes).with_adapter(adapter);
    SourceReportBuilder::from_evidence(kind, evidence).finish()
}

async fn bounded_http_json_probe(endpoint: &str, step: &DeliveryProbeStep) -> ProbeReport {
    keyed_probe_result(
        step.key,
        step.label,
        transport::probe_json_source(endpoint, &step.path),
        transport::probe_json_value_bounded(endpoint, &step.path, step.response_limit)
            .await
            .map(|value| normalize_http_probe_value(value, step.normalizer)),
    )
}

fn probe_step(plan: &[DeliveryProbeStep], key: SourceProbeKey) -> Option<&DeliveryProbeStep> {
    plan.iter().find(|step| step.key == key)
}

async fn http_json_probe(endpoint: &str, step: &DeliveryProbeStep) -> ProbeReport {
    keyed_probe_result(
        step.key,
        step.label,
        http::rest_url(endpoint, &step.path),
        transport::probe_value(endpoint, &step.path)
            .await
            .map(|value| normalize_http_probe_value(value, step.normalizer)),
    )
}

fn normalize_http_probe_value(value: Value, normalizer: DeliveryProbeNormalizer) -> Value {
    match normalizer {
        DeliveryProbeNormalizer::Identity => value,
        DeliveryProbeNormalizer::Health => normalize_delivery_health(value),
        DeliveryProbeNormalizer::Info => normalize_delivery_info(value),
        DeliveryProbeNormalizer::Version => normalize_delivery_version(value),
    }
}

async fn delivery_rest_report(endpoint: &str, metrics_endpoint: Option<&str>) -> SourceReport {
    let plan = delivery_rest_probe_plan();
    let Some(health_step) = probe_step(&plan, SourceProbeKey::DeliveryHealth) else {
        return unsupported_delivery_source_report("rest");
    };
    let Some(info_step) = probe_step(&plan, SourceProbeKey::DeliveryInfo) else {
        return unsupported_delivery_source_report("rest");
    };
    let Some(version_step) = probe_step(&plan, SourceProbeKey::DeliveryVersion) else {
        return unsupported_delivery_source_report("rest");
    };
    let (health_probe, info_probe, version_probe) = tokio::join!(
        http_json_probe(endpoint, health_step),
        http_json_probe(endpoint, info_step),
        http_json_probe(endpoint, version_step),
    );
    let health_value = health_probe.value.clone();
    let info_value = info_probe.value.clone();
    let mut report = SourceReportBuilder::delivery(
        "delivery_rest",
        DeliverySourceReportKind::Rest,
        health_probe,
    )
    .with_probes(vec![info_probe, version_probe])
    .include_module_info_probe();
    if let Some(value) = health_value.as_ref() {
        push_delivery_probe(
            &mut report,
            SourceProbeKey::DeliveryNodeHealth,
            "nodeHealth",
            &http::rest_url(endpoint, &health_step.path),
            value,
            &["nodeHealth"],
        );
        push_delivery_probe(
            &mut report,
            SourceProbeKey::DeliveryConnectionStatus,
            "connectionStatus",
            &http::rest_url(endpoint, &health_step.path),
            value,
            &["connectionStatus"],
        );
        push_delivery_probe(
            &mut report,
            SourceProbeKey::DeliveryProtocolsHealth,
            "protocolsHealth",
            &http::rest_url(endpoint, &health_step.path),
            value,
            &["protocolsHealth"],
        );
    }
    if let Some(value) = info_value.as_ref() {
        push_delivery_probe(
            &mut report,
            SourceProbeKey::DeliveryPeerId,
            "peerId",
            &http::rest_url(endpoint, &info_step.path),
            value,
            &["peerId", "peer_id"],
        );
        push_delivery_probe(
            &mut report,
            SourceProbeKey::DeliveryListenAddresses,
            "listenAddresses",
            &http::rest_url(endpoint, &info_step.path),
            value,
            &[
                "listenAddresses",
                "listen_addresses",
                "multiaddrs",
                "multiAddresses",
            ],
        );
        push_delivery_probe(
            &mut report,
            SourceProbeKey::DeliveryEnrUri,
            "enrUri",
            &http::rest_url(endpoint, &info_step.path),
            value,
            &["enrUri", "enr_uri"],
        );
        push_delivery_probe(
            &mut report,
            SourceProbeKey::DeliveryNodeInfoVersion,
            "Version",
            &http::rest_url(endpoint, &info_step.path),
            value,
            &["version", "Version"],
        );
    }
    if let Some(metrics_endpoint) = optional(metrics_endpoint) {
        report.push_probe(delivery_metrics_probe(metrics_endpoint).await);
    }
    report.finish()
}

fn normalize_delivery_health(value: Value) -> Value {
    match value {
        Value::Object(mut object) => {
            if let Some(value) = object.remove("health") {
                object.insert("nodeHealth".to_owned(), value);
            }
            Value::Object(object)
        }
        value => value,
    }
}

fn normalize_delivery_info(value: Value) -> Value {
    match value {
        Value::Object(mut object) => {
            if let Some(value) = object.remove("enr") {
                object.insert("enrUri".to_owned(), value);
            }
            if let Some(value) = object.remove("addresses") {
                object.insert("listenAddresses".to_owned(), value);
            }
            Value::Object(object)
        }
        value => value,
    }
}

fn normalize_delivery_version(value: Value) -> Value {
    scalar_field(&value, &["version", "Version", "value", "result"]).unwrap_or(value)
}

fn push_delivery_probe(
    report: &mut SourceReportBuilder,
    key: SourceProbeKey,
    method: &str,
    source: &str,
    value: &Value,
    keys: &[&str],
) {
    if let Some(value) = scalar_field(value, keys) {
        report.push_ok(key, format!("delivery_rest.{method}"), source, value);
    }
}

async fn delivery_metrics_report(endpoint: &str) -> SourceReport {
    source_text_metrics_report(
        "delivery_metrics",
        SourceReportKind::Delivery(DeliverySourceReportKind::Metrics),
        endpoint,
        MetricsProbeSpec {
            key: SourceProbeKey::DeliveryMetricsScrape,
            label: "delivery_metrics.scrape",
        },
        MetricsProbeSpec {
            key: SourceProbeKey::DeliveryCollectOpenMetricsText,
            label: "delivery_metrics.collectOpenMetricsText",
        },
        transport::probe_metrics(endpoint).await,
    )
}

async fn delivery_metrics_probe(metrics_endpoint: &str) -> ProbeReport {
    keyed_probe_result(
        SourceProbeKey::DeliveryCollectOpenMetricsText,
        "delivery_rest.collectOpenMetricsText",
        metrics_endpoint,
        transport::probe_metrics(metrics_endpoint).await,
    )
}

async fn delivery_network_monitor_report(
    endpoint: &str,
    metrics_endpoint: Option<&str>,
) -> SourceReport {
    let plan = delivery_network_monitor_probe_plan();
    let Some(all_peers_step) = probe_step(&plan, SourceProbeKey::DeliveryAllPeersInfo) else {
        return unsupported_delivery_source_report("network-monitor");
    };
    let Some(content_topics_step) = probe_step(&plan, SourceProbeKey::DeliveryContentTopics) else {
        return unsupported_delivery_source_report("network-monitor");
    };
    let (all_peers_probe, content_topics_probe) = tokio::join!(
        bounded_http_json_probe(endpoint, all_peers_step),
        bounded_http_json_probe(endpoint, content_topics_step),
    );
    let mut report = SourceReportBuilder::delivery(
        "delivery_network_monitor",
        DeliverySourceReportKind::NetworkMonitor,
        all_peers_probe,
    )
    .include_module_info_probe();
    report.push_probe(content_topics_probe);
    if let Some(metrics_endpoint) = optional(metrics_endpoint) {
        report.push_result(
            SourceProbeKey::DeliveryCollectOpenMetricsText,
            "delivery_network_monitor.collectOpenMetricsText",
            metrics_endpoint,
            transport::probe_metrics(metrics_endpoint).await,
        );
    }
    report.finish()
}

fn unsupported_delivery_source_report(mode: &str) -> SourceReport {
    unsupported_source_report(
        "delivery",
        "delivery",
        SourceReportKind::Delivery(DeliverySourceReportKind::Unsupported),
        mode,
    )
}

fn optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn scalar_field(value: &Value, keys: &[&str]) -> Option<Value> {
    match value {
        Value::Object(object) => {
            for key in keys {
                if let Some(value) = object.get(*key) {
                    return match value {
                        Value::Object(_) => {
                            scalar_field(value, keys).or_else(|| Some(value.clone()))
                        }
                        _ => Some(value.clone()),
                    };
                }
            }
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use anyhow::{Result, ensure};
    use serde_json::json;

    use super::*;
    use crate::modules::logos_core::{
        ModuleCall, ModuleCallFuture, ModuleCallReply, ModuleDiagnosticFuture, ModuleTransport,
        ModuleTransportKind,
    };

    struct CliMetricsTransport {
        calls: Arc<AtomicUsize>,
        module_info_calls: Arc<AtomicUsize>,
    }

    impl ModuleTransport for CliMetricsTransport {
        fn kind(&self) -> ModuleTransportKind {
            ModuleTransportKind::LogoscoreCli
        }

        fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            let method = call.method().to_owned();
            Box::pin(async move {
                ensure!(
                    method == "collectOpenMetricsText",
                    "unexpected CLI Delivery call `{method}`"
                );
                Ok(ModuleCallReply::new(
                    ModuleTransportKind::LogoscoreCli,
                    json!("libp2p_peers 1\n"),
                ))
            })
        }
        fn module_info(&self, _module: String) -> ModuleDiagnosticFuture<'_> {
            self.module_info_calls.fetch_add(1, Ordering::Relaxed);
            Box::pin(async { Ok(json!({ "name": "delivery_module", "methods": [] })) })
        }
    }

    #[test]
    fn rest_probe_plan_declares_base_health_info_version_steps() {
        let steps = delivery_rest_probe_plan();
        let keys = steps.iter().map(|step| step.key).collect::<Vec<_>>();

        assert_eq!(steps.len(), 3);
        assert!(keys.contains(&SourceProbeKey::DeliveryHealth));
        assert!(keys.contains(&SourceProbeKey::DeliveryInfo));
        assert!(keys.contains(&SourceProbeKey::DeliveryVersion));
    }

    #[test]
    fn network_monitor_probe_plan_declares_monitor_endpoints() {
        let steps = delivery_network_monitor_probe_plan();

        let peers = steps.iter().find(|step| step.path == "/allpeersinfo");
        let topics = steps.iter().find(|step| step.path == "/contenttopics");
        assert_eq!(
            peers.map(|step| step.response_limit),
            Some(transport::DELIVERY_PEER_RESPONSE_LIMIT)
        );
        assert_eq!(
            topics.map(|step| step.response_limit),
            Some(transport::DELIVERY_PROBE_RESPONSE_LIMIT)
        );
    }

    #[test]
    fn rest_normalizers_expose_current_fields() {
        let health = normalize_delivery_health(json!({
            "nodeHealth": "Ready",
            "connectionStatus": "Connected",
            "protocolsHealth": [
                { "/vac/waku/relay/2.0.0": "Ready" }
            ]
        }));
        assert_eq!(
            scalar_field(&health, &["nodeHealth"]).as_ref(),
            Some(&json!("Ready"))
        );
        assert_eq!(
            scalar_field(&health, &["connectionStatus"]).as_ref(),
            Some(&json!("Connected"))
        );

        let info = normalize_delivery_info(json!({
            "peerId": "peer-a",
            "listenAddresses": ["/ip4/127.0.0.1/tcp/0"],
            "enrUri": "enr:-abc"
        }));
        assert_eq!(
            scalar_field(&info, &["peerId"]).as_ref(),
            Some(&json!("peer-a"))
        );
        assert_eq!(
            scalar_field(&info, &["enrUri"]).as_ref(),
            Some(&json!("enr:-abc"))
        );
    }

    #[tokio::test]
    async fn cli_report_ignores_rest_endpoint_and_uses_cli_evidence() -> Result<()> {
        let calls = Arc::new(AtomicUsize::new(0));
        let module_info_calls = Arc::new(AtomicUsize::new(0));
        let transport: SharedModuleTransport = Arc::new(CliMetricsTransport {
            calls: Arc::clone(&calls),
            module_info_calls: Arc::clone(&module_info_calls),
        });

        let report = delivery_source_report_with_runtime_metrics(
            "logoscore_cli",
            Some("http://127.0.0.1:9"),
            None,
            false,
            true,
            &transport,
        )
        .await;
        let keys = report
            .probes
            .iter()
            .filter_map(|probe| probe.probe_key.as_deref())
            .collect::<Vec<_>>();

        ensure!(
            calls.load(Ordering::Relaxed) == 1,
            "CLI Delivery source dispatched unexpected runtime calls"
        );
        ensure!(
            module_info_calls.load(Ordering::Relaxed) == 1,
            "CLI Delivery source did not query module metadata"
        );
        ensure!(report.health.ready, "CLI Delivery report was not ready");
        ensure!(
            keys == [SourceProbeKey::DeliveryCollectOpenMetricsText.as_str()],
            "CLI Delivery source returned unexpected probes: {keys:?}"
        );
        ensure!(
            report.probes.iter().all(|probe| {
                probe.probe_key.as_deref() != Some(SourceProbeKey::DeliveryHealth.as_str())
                    && probe.probe_key.as_deref() != Some(SourceProbeKey::DeliveryInfo.as_str())
            }),
            "CLI Delivery source leaked REST health probes"
        );
        Ok(())
    }
}
