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
                MetricsProbeSpec, SourceReportBuilder, SourceReportKind, keyed_probe_err,
                keyed_probe_result, source_text_metrics_report, unsupported_source_report,
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
        }
    }
}

pub async fn delivery_source_report(
    source_mode: &str,
    rest_endpoint: Option<&str>,
    metrics_endpoint: Option<&str>,
    runtime_diagnostics_enabled: bool,
    module_transport: &SharedModuleTransport,
) -> SourceReport {
    match MessagingAdapter::select(source_mode, rest_endpoint, metrics_endpoint) {
        MessagingAdapter::Module { transport } => {
            let health_endpoint =
                if transport == crate::modules::logos_core::ModuleTransportKind::LogoscoreCli {
                    optional(rest_endpoint)
                } else {
                    None
                };
            module_source_report(
                SourceReportKind::Delivery(DeliverySourceReportKind::Module),
                layer::module_report(
                    module_transport,
                    transport,
                    None,
                    runtime_diagnostics_enabled,
                    health_endpoint.is_some(),
                )
                .await,
                health_endpoint,
            )
            .await
        }
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
        ),
        DeliveryProbeStep::new(
            SourceProbeKey::DeliveryContentTopics,
            "delivery_network_monitor.contentTopics",
            "/contenttopics",
            DeliveryProbeNormalizer::Identity,
        ),
    ]
}

async fn module_source_report(
    kind: SourceReportKind,
    report: ModuleReport,
    health_endpoint: Option<&str>,
) -> SourceReport {
    let module_enr = report
        .probes
        .iter()
        .find(|probe| {
            probe.ok && probe.probe_key.as_deref() == Some(SourceProbeKey::DeliveryMyEnr.as_str())
        })
        .and_then(|probe| probe.value.as_ref())
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let adapter = match report.adapter {
        crate::modules::logos_core::ModuleTransportKind::Module => AdapterConnectionType::Module,
        crate::modules::logos_core::ModuleTransportKind::LogoscoreCli => {
            AdapterConnectionType::LogoscoreCli
        }
    };
    let evidence =
        SourceEvidence::new(report.module, report.module_info, report.probes).with_adapter(adapter);
    let mut builder = SourceReportBuilder::from_evidence(kind, evidence);
    if let Some(endpoint) = health_endpoint {
        append_module_health(&mut builder, endpoint, module_enr.as_deref()).await;
    }
    builder.finish()
}

async fn append_module_health(
    report: &mut SourceReportBuilder,
    endpoint: &str,
    module_enr: Option<&str>,
) {
    let info_step = DeliveryProbeStep::new(
        SourceProbeKey::DeliveryInfo,
        "delivery_rest.info",
        "/info",
        DeliveryProbeNormalizer::Info,
    );
    let health_step = DeliveryProbeStep::new(
        SourceProbeKey::DeliveryHealth,
        "delivery_rest.health",
        "/health",
        DeliveryProbeNormalizer::Health,
    );
    let (info_probe, health_probe) = tokio::join!(
        bounded_http_json_probe(endpoint, &info_step),
        bounded_http_json_probe(endpoint, &health_step),
    );
    let rest_enr = info_probe
        .value
        .as_ref()
        .and_then(|value| scalar_field(value, &["enrUri", "enr"]))
        .and_then(|value| value.as_str().map(ToOwned::to_owned));
    report.push_probe(info_probe);
    let health_source = transport::probe_status_source(endpoint, &health_step.path);
    if let Some(error) = delivery_health_identity_error(module_enr, rest_enr.as_deref()) {
        report.push_probe(keyed_probe_err(
            SourceProbeKey::DeliveryHealth,
            health_step.label,
            health_source,
            error,
        ));
        return;
    }
    let health_value = health_probe.value.clone();
    report.push_probe(health_probe);
    if let Some(value) = health_value.as_ref() {
        push_delivery_probe(
            report,
            SourceProbeKey::DeliveryNodeHealth,
            "nodeHealth",
            &health_source,
            value,
            &["nodeHealth"],
        );
        push_delivery_probe(
            report,
            SourceProbeKey::DeliveryConnectionStatus,
            "connectionStatus",
            &health_source,
            value,
            &["connectionStatus"],
        );
        push_delivery_probe(
            report,
            SourceProbeKey::DeliveryProtocolsHealth,
            "protocolsHealth",
            &health_source,
            value,
            &["protocolsHealth"],
        );
    }
}

fn delivery_health_identity_error(
    module_enr: Option<&str>,
    rest_enr: Option<&str>,
) -> Option<&'static str> {
    match (module_enr, rest_enr) {
        (None, _) => {
            Some("Delivery module ENR is unavailable; REST health identity cannot be verified")
        }
        (Some(_), None) => {
            Some("Delivery REST ENR is unavailable; health identity cannot be verified")
        }
        (Some(expected), Some(observed)) if expected != observed => {
            Some("Delivery REST health endpoint does not match the module identity")
        }
        (Some(_), Some(_)) => None,
    }
}

async fn bounded_http_json_probe(endpoint: &str, step: &DeliveryProbeStep) -> ProbeReport {
    keyed_probe_result(
        step.key,
        step.label,
        transport::probe_status_source(endpoint, &step.path),
        transport::probe_status_value(endpoint, &step.path)
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
        http_json_probe(endpoint, all_peers_step),
        http_json_probe(endpoint, content_topics_step),
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
    use std::{
        io::{ErrorKind, Read as _, Write as _},
        net::TcpListener,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        thread,
        time::{Duration, Instant},
    };

    use anyhow::{Context as _, Result, anyhow, bail, ensure};
    use serde_json::{Value, json};

    use super::*;
    use crate::modules::logos_core::{
        ModuleCall, ModuleCallFuture, ModuleCallReply, ModuleTransport, ModuleTransportKind,
    };

    struct ReducedHealthTransport {
        calls: Arc<AtomicUsize>,
    }

    impl ModuleTransport for ReducedHealthTransport {
        fn kind(&self) -> ModuleTransportKind {
            ModuleTransportKind::LogoscoreCli
        }

        fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            let value = match call.method() {
                "getAvailableNodeInfoIDs" => json!(["MyENR"]),
                "getNodeInfo" => json!("enr:test"),
                method => {
                    let method = method.to_owned();
                    return Box::pin(async move { bail!("unexpected call `{method}`") });
                }
            };
            Box::pin(async move {
                Ok(ModuleCallReply::new(
                    ModuleTransportKind::LogoscoreCli,
                    value,
                ))
            })
        }
    }

    fn spawn_health_server(rest_enr: &str) -> Result<(String, thread::JoinHandle<Result<usize>>)> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let info_body = json!({ "enrUri": rest_enr }).to_string();
        let health_body = json!({
            "nodeHealth": "READY",
            "connectionStatus": "Connected",
            "protocolsHealth": [
                { "Relay": "READY" },
                { "Store": "NOT_MOUNTED" },
                {
                    "Rendezvous": "NOT_READY",
                    "desc": "No Rendezvous peers are available yet"
                },
                { "Store Client": "READY" }
            ]
        })
        .to_string();
        let server = thread::spawn(move || -> Result<usize> {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut served = 0;
            while served < 2 && Instant::now() < deadline {
                let (mut stream, _address) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                };
                stream.set_read_timeout(Some(Duration::from_secs(1)))?;
                let mut request = [0_u8; 1024];
                let length = stream.read(&mut request)?;
                let request = String::from_utf8_lossy(
                    request
                        .get(..length)
                        .context("Delivery status request exceeded read buffer")?,
                );
                let body = if request.starts_with("GET /info ") {
                    info_body.as_str()
                } else if request.starts_with("GET /health ") {
                    health_body.as_str()
                } else {
                    bail!("unexpected Delivery status request")
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )?;
                served += 1;
            }
            Ok(served)
        });
        Ok((endpoint, server))
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

        assert!(steps.iter().any(|step| step.path == "/allpeersinfo"));
        assert!(steps.iter().any(|step| step.path == "/contenttopics"));
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

    #[test]
    fn module_health_requires_matching_rest_identity() {
        assert_eq!(
            delivery_health_identity_error(Some("enr:a"), Some("enr:a")),
            None
        );
        assert!(delivery_health_identity_error(Some("enr:a"), Some("enr:b")).is_some());
        assert!(delivery_health_identity_error(None, Some("enr:a")).is_some());
        assert!(delivery_health_identity_error(Some("enr:a"), None).is_some());
    }

    #[tokio::test]
    async fn reduced_cli_report_binds_and_reports_live_health() -> Result<()> {
        let (endpoint, server) = spawn_health_server("enr:test")?;
        let calls = Arc::new(AtomicUsize::new(0));
        let transport: SharedModuleTransport = Arc::new(ReducedHealthTransport {
            calls: Arc::clone(&calls),
        });

        let report =
            delivery_source_report("logoscore_cli", Some(&endpoint), None, false, &transport).await;
        let served = server
            .join()
            .map_err(|_| anyhow!("Delivery health server thread panicked"))??;
        let node_health = report.probes.iter().find(|probe| {
            probe.probe_key.as_deref() == Some(SourceProbeKey::DeliveryNodeHealth.as_str())
        });
        let connection = report.probes.iter().find(|probe| {
            probe.probe_key.as_deref() == Some(SourceProbeKey::DeliveryConnectionStatus.as_str())
        });
        let protocols = report.probes.iter().find(|probe| {
            probe.probe_key.as_deref() == Some(SourceProbeKey::DeliveryProtocolsHealth.as_str())
        });

        ensure!(
            served == 2,
            "Delivery health server received {served} requests"
        );
        ensure!(
            calls.load(Ordering::Relaxed) == 2,
            "reduced health binding dispatched unexpected module calls"
        );
        ensure!(report.health.ready, "reduced Delivery report was not ready");
        ensure!(
            node_health.and_then(|probe| probe.value.as_ref())
                == Some(&Value::String("READY".into())),
            "reduced Delivery report omitted node health"
        );
        ensure!(
            connection.and_then(|probe| probe.value.as_ref())
                == Some(&Value::String("Connected".into())),
            "reduced Delivery report omitted connection status"
        );
        let expected_protocols = json!([
            { "Relay": "READY" },
            { "Store": "NOT_MOUNTED" },
            {
                "Rendezvous": "NOT_READY",
                "desc": "No Rendezvous peers are available yet"
            },
            { "Store Client": "READY" }
        ]);
        ensure!(
            protocols.and_then(|probe| probe.value.as_ref()) == Some(&expected_protocols),
            "reduced Delivery report omitted protocol health"
        );
        ensure!(
            report.probe_facts.iter().any(|fact| {
                fact.key == SourceProbeKey::DeliveryProtocolsHealth.as_str()
                    && fact.ok
                    && fact.value.as_ref() == Some(&expected_protocols)
            }),
            "reduced Delivery facts omitted protocol health"
        );
        Ok(())
    }

    #[tokio::test]
    async fn cli_report_rejects_protocol_health_from_mismatched_identity() -> Result<()> {
        let (endpoint, server) = spawn_health_server("enr:other")?;
        let calls = Arc::new(AtomicUsize::new(0));
        let transport: SharedModuleTransport = Arc::new(ReducedHealthTransport {
            calls: Arc::clone(&calls),
        });

        let report =
            delivery_source_report("logoscore_cli", Some(&endpoint), None, false, &transport).await;
        let served = server
            .join()
            .map_err(|_| anyhow!("Delivery health server thread panicked"))??;
        let derived_keys = [
            SourceProbeKey::DeliveryNodeHealth,
            SourceProbeKey::DeliveryConnectionStatus,
            SourceProbeKey::DeliveryProtocolsHealth,
        ];

        ensure!(
            served == 2,
            "Delivery health server received {served} requests"
        );
        ensure!(
            calls.load(Ordering::Relaxed) == 2,
            "mismatched health binding dispatched unexpected module calls"
        );
        ensure!(!report.health.ready, "mismatched health report was ready");
        ensure!(
            report.probes.iter().any(|probe| {
                probe.probe_key.as_deref() == Some(SourceProbeKey::DeliveryHealth.as_str())
                    && !probe.ok
                    && probe.error.as_deref()
                        == Some("Delivery REST health endpoint does not match the module identity")
            }),
            "mismatched health report omitted identity failure"
        );
        ensure!(
            derived_keys.iter().all(|key| {
                report
                    .probes
                    .iter()
                    .all(|probe| probe.probe_key.as_deref() != Some(key.as_str()))
                    && report
                        .probe_facts
                        .iter()
                        .all(|fact| fact.key != key.as_str())
            }),
            "mismatched health report exposed identity-bound fields"
        );
        Ok(())
    }
}
