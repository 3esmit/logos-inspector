use anyhow::Result;
use serde_json::{Value, json};

use super::evidence::SourceEvidence;
use super::plan::{
    HttpJsonProbeStep, HttpProbeNormalizer, delivery_network_monitor_probe_plan,
    delivery_rest_probe_plan, storage_rest_probe_plan,
};
use super::report::{
    MetricsProbeSpec, SourceReportBuilder, SourceReportKind, keyed_probe_result,
    source_report_from_evidence, source_text_metrics_report, unsupported_source_report,
};
use crate::source_routing::{
    DeliverySourceReportKind, SourceProbeKey, SourceReport, StorageSourceReportKind,
    delivery::layer::MessagingAdapter, storage::layer::StorageAdapter,
};
use crate::{
    ProbeReport,
    modules::ModuleReport,
    source_routing::{messaging_layer, storage_layer},
};

#[derive(Debug, Clone, Copy)]
enum ProbeNode {
    Storage,
    Messaging,
}

pub async fn storage_source_report(
    source_mode: &str,
    rest_endpoint: Option<&str>,
    metrics_endpoint: Option<&str>,
    cid: Option<&str>,
    privileged_debug_enabled: bool,
) -> SourceReport {
    match StorageAdapter::select(source_mode, rest_endpoint, metrics_endpoint) {
        StorageAdapter::Module => module_source_report(
            SourceReportKind::Storage(StorageSourceReportKind::Module),
            storage_layer::module_report(cid, privileged_debug_enabled),
        ),
        StorageAdapter::Rest {
            endpoint,
            metrics_endpoint,
        } => storage_rest_report(endpoint, metrics_endpoint, cid, privileged_debug_enabled).await,
        StorageAdapter::Metrics { endpoint } => storage_metrics_report(endpoint).await,
        StorageAdapter::Unsupported { mode } => unsupported_storage_source_report(mode),
    }
}

pub async fn delivery_source_report(
    source_mode: &str,
    rest_endpoint: Option<&str>,
    metrics_endpoint: Option<&str>,
) -> SourceReport {
    match MessagingAdapter::select(source_mode, rest_endpoint, metrics_endpoint) {
        MessagingAdapter::Module => module_source_report(
            SourceReportKind::Delivery(DeliverySourceReportKind::Module),
            messaging_layer::module_report(None),
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

fn module_source_report(kind: SourceReportKind, report: ModuleReport) -> SourceReport {
    source_report_from_evidence(
        kind,
        SourceEvidence::new(report.module, report.module_info, report.probes),
    )
}

fn probe_step(plan: &[HttpJsonProbeStep], key: SourceProbeKey) -> Option<&HttpJsonProbeStep> {
    plan.iter().find(|step| step.key == key)
}

fn optional_probe_steps<'a>(
    plan: &'a [HttpJsonProbeStep],
    handled: &[SourceProbeKey],
) -> Vec<&'a HttpJsonProbeStep> {
    plan.iter()
        .filter(|step| !handled.contains(&step.key))
        .collect()
}

async fn http_json_probe(node: ProbeNode, endpoint: &str, step: &HttpJsonProbeStep) -> ProbeReport {
    keyed_probe_result(
        step.key,
        step.label,
        http_url(endpoint, &step.path),
        probe_value(node, endpoint, &step.path)
            .await
            .map(|value| normalize_http_probe_value(value, &step.normalizer)),
    )
}

fn normalize_http_probe_value(value: Value, normalizer: &HttpProbeNormalizer) -> Value {
    match normalizer {
        HttpProbeNormalizer::Identity => value,
        HttpProbeNormalizer::StorageManifests => normalize_storage_manifests(value),
        HttpProbeNormalizer::StorageSpr => normalize_storage_spr(value),
        HttpProbeNormalizer::StoragePeerId => normalize_storage_peer_id(value),
        HttpProbeNormalizer::StorageExists(cid) => normalize_storage_exists(value, cid),
        HttpProbeNormalizer::DeliveryHealth => normalize_delivery_health(value),
        HttpProbeNormalizer::DeliveryInfo => normalize_delivery_info(value),
        HttpProbeNormalizer::DeliveryVersion => normalize_delivery_version(value),
    }
}

async fn storage_rest_report(
    endpoint: &str,
    metrics_endpoint: Option<&str>,
    cid: Option<&str>,
    privileged_debug_enabled: bool,
) -> SourceReport {
    let plan = storage_rest_probe_plan(cid, privileged_debug_enabled);
    let Some(space_step) = probe_step(&plan, SourceProbeKey::StorageSpace) else {
        return unsupported_storage_source_report("rest");
    };
    let Some(spr_step) = probe_step(&plan, SourceProbeKey::StorageSpr) else {
        return unsupported_storage_source_report("rest");
    };
    let Some(peer_id_step) = probe_step(&plan, SourceProbeKey::StoragePeerId) else {
        return unsupported_storage_source_report("rest");
    };
    let Some(manifests_step) = probe_step(&plan, SourceProbeKey::StorageManifests) else {
        return unsupported_storage_source_report("rest");
    };
    let (space_probe, spr_probe, peer_id_probe, manifests_probe) = tokio::join!(
        http_json_probe(ProbeNode::Storage, endpoint, space_step),
        http_json_probe(ProbeNode::Storage, endpoint, spr_step),
        http_json_probe(ProbeNode::Storage, endpoint, peer_id_step),
        http_json_probe(ProbeNode::Storage, endpoint, manifests_step),
    );
    let mut report =
        SourceReportBuilder::storage("storage_rest", StorageSourceReportKind::Rest, space_probe)
            .include_module_info_probe();
    report.push_probe(spr_probe);
    report.push_probe(peer_id_probe);
    report.push_probe(manifests_probe);
    if !privileged_debug_enabled {
        report.push_ok(
            SourceProbeKey::StoragePrivilegedProbe,
            "storage_rest.privilegedProbe",
            "disabled",
            json!({ "skipped": true }),
        );
    }
    for step in optional_probe_steps(
        &plan,
        &[
            SourceProbeKey::StorageSpace,
            SourceProbeKey::StorageSpr,
            SourceProbeKey::StoragePeerId,
            SourceProbeKey::StorageManifests,
        ],
    ) {
        report.push_probe(http_json_probe(ProbeNode::Storage, endpoint, step).await);
    }
    if let Some(metrics_endpoint) = optional(metrics_endpoint) {
        report.push_probe(storage_metrics_probe(metrics_endpoint).await);
    }
    report.finish()
}

async fn storage_metrics_report(endpoint: &str) -> SourceReport {
    source_text_metrics_report(
        "storage_metrics",
        SourceReportKind::Storage(StorageSourceReportKind::Metrics),
        endpoint,
        MetricsProbeSpec {
            key: SourceProbeKey::StorageMetricsScrape,
            label: "storage_metrics.scrape",
        },
        MetricsProbeSpec {
            key: SourceProbeKey::StorageCollectMetrics,
            label: "storage_metrics.collectMetrics",
        },
        storage_layer::probe_metrics(endpoint).await,
    )
}

async fn storage_metrics_probe(metrics_endpoint: &str) -> ProbeReport {
    keyed_probe_result(
        SourceProbeKey::StorageCollectMetrics,
        "storage_rest.collectMetrics",
        metrics_endpoint,
        storage_layer::probe_metrics(metrics_endpoint).await,
    )
}

fn unsupported_storage_source_report(mode: &str) -> SourceReport {
    unsupported_source_report(
        "storage",
        "storage",
        SourceReportKind::Storage(StorageSourceReportKind::Unsupported),
        mode,
    )
}

fn normalize_storage_manifests(value: Value) -> Value {
    match value {
        Value::Object(mut object) => match object.remove("content") {
            Some(Value::Array(items)) => Value::Array(items),
            Some(content) => {
                object.insert("content".to_owned(), content);
                Value::Object(object)
            }
            None => Value::Object(object),
        },
        value => value,
    }
}

fn normalize_storage_spr(value: Value) -> Value {
    scalar_field(&value, &["spr", "value", "result"]).unwrap_or(value)
}

fn normalize_storage_peer_id(value: Value) -> Value {
    scalar_field(&value, &["peerId", "peer_id", "id", "value", "result"]).unwrap_or(value)
}

fn normalize_storage_exists(value: Value, cid: &str) -> Value {
    scalar_field(&value, &[cid, "exists", "has", "value", "result"]).unwrap_or(value)
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
        http_json_probe(ProbeNode::Messaging, endpoint, health_step),
        http_json_probe(ProbeNode::Messaging, endpoint, info_step),
        http_json_probe(ProbeNode::Messaging, endpoint, version_step),
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
            &http_url(endpoint, &health_step.path),
            value,
            &["nodeHealth"],
        );
        push_delivery_probe(
            &mut report,
            SourceProbeKey::DeliveryConnectionStatus,
            "connectionStatus",
            &http_url(endpoint, &health_step.path),
            value,
            &["connectionStatus"],
        );
        push_delivery_probe(
            &mut report,
            SourceProbeKey::DeliveryProtocolsHealth,
            "protocolsHealth",
            &http_url(endpoint, &health_step.path),
            value,
            &["protocolsHealth"],
        );
    }
    if let Some(value) = info_value.as_ref() {
        push_delivery_probe(
            &mut report,
            SourceProbeKey::DeliveryPeerId,
            "peerId",
            &http_url(endpoint, &info_step.path),
            value,
            &["peerId", "peer_id"],
        );
        push_delivery_probe(
            &mut report,
            SourceProbeKey::DeliveryListenAddresses,
            "listenAddresses",
            &http_url(endpoint, &info_step.path),
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
            &http_url(endpoint, &info_step.path),
            value,
            &["enrUri", "enr_uri"],
        );
        push_delivery_probe(
            &mut report,
            SourceProbeKey::DeliveryNodeInfoVersion,
            "Version",
            &http_url(endpoint, &info_step.path),
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
        messaging_layer::probe_metrics(endpoint).await,
    )
}

async fn delivery_metrics_probe(metrics_endpoint: &str) -> ProbeReport {
    keyed_probe_result(
        SourceProbeKey::DeliveryCollectOpenMetricsText,
        "delivery_rest.collectOpenMetricsText",
        metrics_endpoint,
        messaging_layer::probe_metrics(metrics_endpoint).await,
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
        http_json_probe(ProbeNode::Messaging, endpoint, all_peers_step),
        http_json_probe(ProbeNode::Messaging, endpoint, content_topics_step),
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
            messaging_layer::probe_metrics(metrics_endpoint).await,
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

async fn probe_value(node: ProbeNode, endpoint: &str, path: &str) -> Result<Value> {
    match node {
        ProbeNode::Storage => storage_layer::probe_value(endpoint, path).await,
        ProbeNode::Messaging => messaging_layer::probe_value(endpoint, path).await,
    }
}

fn http_url(endpoint: &str, path: &str) -> String {
    super::http::rest_url(endpoint, path)
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
    use serde_json::json;

    use super::*;

    #[test]
    fn storage_rest_normalizers_unwrap_current_scalar_shapes() {
        assert_eq!(
            normalize_storage_peer_id(json!({ "id": "peer-a" })),
            json!("peer-a")
        );
        assert_eq!(
            normalize_storage_spr(json!({ "spr": "spr-a" })),
            json!("spr-a")
        );
        assert_eq!(
            normalize_storage_exists(json!({ "cid-a": true }), "cid-a"),
            json!(true)
        );
        assert_eq!(
            normalize_storage_exists(json!({ "has": false }), "cid-a"),
            json!(false)
        );
    }

    #[test]
    fn delivery_rest_normalizers_expose_current_api_fields() {
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
        assert_eq!(
            scalar_field(&health, &["protocolsHealth"]).as_ref(),
            Some(&json!([{ "/vac/waku/relay/2.0.0": "Ready" }]))
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
    fn module_report_adapter_derives_source_facts_from_keyed_evidence() {
        let module_info = ProbeReport::ok("storage module", "module-info", json!({}))
            .with_probe_key(SourceProbeKey::StoragePeerId.as_str());
        let report = ModuleReport::new("storage_module", module_info, Vec::new());

        let source_report = module_source_report(
            SourceReportKind::Storage(StorageSourceReportKind::Module),
            report,
        );

        assert!(source_report.health.reachable);
        assert!(
            source_report
                .probe_facts
                .iter()
                .any(|fact| fact.key == SourceProbeKey::StoragePeerId.as_str())
        );
    }
}
