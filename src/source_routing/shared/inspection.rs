use anyhow::Result;
use serde_json::{Value, json};

use super::report::{SourceReportBuilder, keyed_probe_err, keyed_probe_ok, keyed_probe_result};
use crate::source_routing::{
    DEFAULT_DELIVERY_METRICS_ENDPOINT, DEFAULT_DELIVERY_REST_ENDPOINT,
    DEFAULT_STORAGE_METRICS_ENDPOINT, DEFAULT_STORAGE_REST_ENDPOINT, DeliverySourceReportKind,
    SourceFamily, SourceProbeKey, StorageSourceReportKind, effective_source_mode,
};
use crate::{
    ProbeReport,
    modules::{ModuleReport, delivery_report, storage_report},
    read_response_text,
};

pub async fn storage_source_report(
    source_mode: &str,
    rest_endpoint: Option<&str>,
    metrics_endpoint: Option<&str>,
    cid: Option<&str>,
    privileged_debug_enabled: bool,
) -> ModuleReport {
    match effective_source_mode(SourceFamily::Storage, source_mode) {
        "module" => storage_report(cid, privileged_debug_enabled),
        "rest" => {
            storage_rest_report(
                rest_endpoint,
                metrics_endpoint,
                cid,
                privileged_debug_enabled,
            )
            .await
        }
        "metrics" => storage_metrics_report(metrics_endpoint).await,
        mode => unsupported_storage_source_report(mode),
    }
}

pub async fn delivery_source_report(
    source_mode: &str,
    rest_endpoint: Option<&str>,
    metrics_endpoint: Option<&str>,
) -> ModuleReport {
    match effective_source_mode(SourceFamily::Delivery, source_mode) {
        "module" => delivery_report(None),
        "rest" => delivery_rest_report(rest_endpoint, metrics_endpoint).await,
        "metrics" => delivery_metrics_report(metrics_endpoint).await,
        "network-monitor" => delivery_network_monitor_report(rest_endpoint, metrics_endpoint).await,
        mode => unsupported_delivery_source_report(mode),
    }
}

async fn storage_rest_report(
    rest_endpoint: Option<&str>,
    metrics_endpoint: Option<&str>,
    cid: Option<&str>,
    privileged_debug_enabled: bool,
) -> ModuleReport {
    let endpoint = optional(rest_endpoint).unwrap_or(DEFAULT_STORAGE_REST_ENDPOINT);
    let space_source = http_url(endpoint, "/space");
    let spr_source = http_url(endpoint, "/spr");
    let peer_id_source = http_url(endpoint, "/peerid");
    let data_source = http_url(endpoint, "/data");
    let (space, spr, peer_id, data) = tokio::join!(
        raw_http_value(endpoint, "/space"),
        raw_http_value(endpoint, "/spr"),
        raw_http_value(endpoint, "/peerid"),
        raw_http_value(endpoint, "/data"),
    );
    let spr = spr.map(normalize_storage_spr);
    let peer_id = peer_id.map(normalize_storage_peer_id);
    let manifests = data.map(normalize_storage_manifests);
    let space_probe = keyed_probe_result(
        SourceProbeKey::StorageSpace,
        "storage_rest.space",
        space_source.clone(),
        space,
    );
    let mut report =
        SourceReportBuilder::storage("storage_rest", StorageSourceReportKind::Rest, space_probe)
            .include_module_info_probe();
    report.push_result(
        SourceProbeKey::StorageSpr,
        "storage_rest.spr",
        spr_source,
        spr,
    );
    report.push_result(
        SourceProbeKey::StoragePeerId,
        "storage_rest.peerId",
        peer_id_source,
        peer_id,
    );
    report.push_result(
        SourceProbeKey::StorageManifests,
        "storage_rest.manifests",
        data_source,
        manifests,
    );
    if privileged_debug_enabled {
        let debug_source = http_url(endpoint, "/debug/info");
        report.push_result(
            SourceProbeKey::StorageDebug,
            "storage_rest.debug",
            debug_source,
            raw_http_value(endpoint, "/debug/info").await,
        );
    } else {
        report.push_ok(
            SourceProbeKey::StoragePrivilegedProbe,
            "storage_rest.privilegedProbe",
            "disabled",
            json!({ "skipped": true }),
        );
    }
    if let Some(cid) = optional(cid) {
        let path = format!("/data/{cid}/exists");
        report.push_result(
            SourceProbeKey::StorageExists,
            "storage_rest.exists",
            http_url(endpoint, &path),
            raw_http_value(endpoint, &path)
                .await
                .map(|value| normalize_storage_exists(value, cid)),
        );
    }
    if let Some(metrics_endpoint) = optional(metrics_endpoint) {
        report.push_probe(storage_metrics_probe(metrics_endpoint).await);
    }
    report.finish()
}

async fn storage_metrics_report(metrics_endpoint: Option<&str>) -> ModuleReport {
    let endpoint = optional(metrics_endpoint).unwrap_or(DEFAULT_STORAGE_METRICS_ENDPOINT);
    let metrics = raw_http_text_url(endpoint).await;
    match metrics {
        Ok(text) => {
            let module_info = keyed_probe_ok(
                SourceProbeKey::StorageMetricsScrape,
                "storage_metrics.scrape",
                endpoint,
                json!({
                    "bytes": text.len(),
                    "lines": text.lines().count(),
                }),
            );
            let mut report = SourceReportBuilder::storage(
                "storage_metrics",
                StorageSourceReportKind::Metrics,
                module_info,
            );
            report.push_ok(
                SourceProbeKey::StorageCollectMetrics,
                "storage_metrics.collectMetrics",
                endpoint,
                text,
            );
            report.finish()
        }
        Err(error) => {
            let error = error.to_string();
            let module_info = keyed_probe_err(
                SourceProbeKey::StorageMetricsScrape,
                "storage_metrics.scrape",
                endpoint,
                &error,
            );
            let mut report = SourceReportBuilder::storage(
                "storage_metrics",
                StorageSourceReportKind::Metrics,
                module_info,
            );
            report.push_probe(keyed_probe_err(
                SourceProbeKey::StorageCollectMetrics,
                "storage_metrics.collectMetrics",
                endpoint,
                error,
            ));
            report.finish()
        }
    }
}

async fn storage_metrics_probe(metrics_endpoint: &str) -> ProbeReport {
    keyed_probe_result(
        SourceProbeKey::StorageCollectMetrics,
        "storage_rest.collectMetrics",
        metrics_endpoint,
        raw_http_text_url(metrics_endpoint).await,
    )
}

fn unsupported_storage_source_report(mode: &str) -> ModuleReport {
    let module = format!("storage_{mode}");
    let module_info = ProbeReport::err(
        "storage source",
        mode,
        format!("storage source mode `{mode}` is not implemented"),
    );
    SourceReportBuilder::storage(module, StorageSourceReportKind::Unsupported, module_info).finish()
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

async fn delivery_rest_report(
    rest_endpoint: Option<&str>,
    metrics_endpoint: Option<&str>,
) -> ModuleReport {
    let endpoint = optional(rest_endpoint).unwrap_or(DEFAULT_DELIVERY_REST_ENDPOINT);
    let health_source = http_url(endpoint, "/health");
    let info_source = http_url(endpoint, "/info");
    let version_source = http_url(endpoint, "/version");
    let (health, info, version) = tokio::join!(
        raw_http_value(endpoint, "/health"),
        raw_http_value(endpoint, "/info"),
        raw_http_value(endpoint, "/version"),
    );
    let health_probe = keyed_probe_result(
        SourceProbeKey::DeliveryHealth,
        "delivery_rest.health",
        health_source.clone(),
        health.map(normalize_delivery_health),
    );
    let info_probe = keyed_probe_result(
        SourceProbeKey::DeliveryInfo,
        "delivery_rest.info",
        info_source.clone(),
        info.map(normalize_delivery_info),
    );
    let version_probe = keyed_probe_result(
        SourceProbeKey::DeliveryVersion,
        "delivery_rest.version",
        version_source.clone(),
        version.map(normalize_delivery_version),
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
            &health_source,
            value,
            &["nodeHealth"],
        );
        push_delivery_probe(
            &mut report,
            SourceProbeKey::DeliveryConnectionStatus,
            "connectionStatus",
            &health_source,
            value,
            &["connectionStatus"],
        );
        push_delivery_probe(
            &mut report,
            SourceProbeKey::DeliveryProtocolsHealth,
            "protocolsHealth",
            &health_source,
            value,
            &["protocolsHealth"],
        );
    }
    if let Some(value) = info_value.as_ref() {
        push_delivery_probe(
            &mut report,
            SourceProbeKey::DeliveryPeerId,
            "peerId",
            &info_source,
            value,
            &["peerId", "peer_id"],
        );
        push_delivery_probe(
            &mut report,
            SourceProbeKey::DeliveryListenAddresses,
            "listenAddresses",
            &info_source,
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
            &info_source,
            value,
            &["enrUri", "enr_uri"],
        );
        push_delivery_probe(
            &mut report,
            SourceProbeKey::DeliveryNodeInfoVersion,
            "Version",
            &info_source,
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

async fn delivery_metrics_report(metrics_endpoint: Option<&str>) -> ModuleReport {
    let endpoint = optional(metrics_endpoint).unwrap_or(DEFAULT_DELIVERY_METRICS_ENDPOINT);
    let metrics = raw_http_text_url(endpoint).await;
    match metrics {
        Ok(text) => {
            let module_info = keyed_probe_ok(
                SourceProbeKey::DeliveryMetricsScrape,
                "delivery_metrics.scrape",
                endpoint,
                json!({
                    "bytes": text.len(),
                    "lines": text.lines().count(),
                }),
            );
            let mut report = SourceReportBuilder::delivery(
                "delivery_metrics",
                DeliverySourceReportKind::Metrics,
                module_info,
            );
            report.push_ok(
                SourceProbeKey::DeliveryCollectOpenMetricsText,
                "delivery_metrics.collectOpenMetricsText",
                endpoint,
                text,
            );
            report.finish()
        }
        Err(error) => {
            let error = error.to_string();
            let module_info = keyed_probe_err(
                SourceProbeKey::DeliveryMetricsScrape,
                "delivery_metrics.scrape",
                endpoint,
                &error,
            );
            let mut report = SourceReportBuilder::delivery(
                "delivery_metrics",
                DeliverySourceReportKind::Metrics,
                module_info,
            );
            report.push_probe(keyed_probe_err(
                SourceProbeKey::DeliveryCollectOpenMetricsText,
                "delivery_metrics.collectOpenMetricsText",
                endpoint,
                error,
            ));
            report.finish()
        }
    }
}

async fn delivery_metrics_probe(metrics_endpoint: &str) -> ProbeReport {
    keyed_probe_result(
        SourceProbeKey::DeliveryCollectOpenMetricsText,
        "delivery_rest.collectOpenMetricsText",
        metrics_endpoint,
        raw_http_text_url(metrics_endpoint).await,
    )
}

async fn delivery_network_monitor_report(
    rest_endpoint: Option<&str>,
    metrics_endpoint: Option<&str>,
) -> ModuleReport {
    let endpoint = optional(rest_endpoint).unwrap_or(DEFAULT_DELIVERY_REST_ENDPOINT);
    let all_peers_source = http_url(endpoint, "/allpeersinfo");
    let content_topics_source = http_url(endpoint, "/contenttopics");
    let (all_peers, content_topics) = tokio::join!(
        raw_http_value(endpoint, "/allpeersinfo"),
        raw_http_value(endpoint, "/contenttopics"),
    );
    let all_peers_probe = keyed_probe_result(
        SourceProbeKey::DeliveryAllPeersInfo,
        "delivery_network_monitor.allPeersInfo",
        all_peers_source,
        all_peers,
    );
    let mut report = SourceReportBuilder::delivery(
        "delivery_network_monitor",
        DeliverySourceReportKind::NetworkMonitor,
        all_peers_probe,
    )
    .include_module_info_probe();
    report.push_result(
        SourceProbeKey::DeliveryContentTopics,
        "delivery_network_monitor.contentTopics",
        content_topics_source,
        content_topics,
    );
    if let Some(metrics_endpoint) = optional(metrics_endpoint) {
        report.push_result(
            SourceProbeKey::DeliveryCollectOpenMetricsText,
            "delivery_network_monitor.collectOpenMetricsText",
            metrics_endpoint,
            raw_http_text_url(metrics_endpoint).await,
        );
    }
    report.finish()
}

fn unsupported_delivery_source_report(mode: &str) -> ModuleReport {
    let module = format!("delivery_{mode}");
    let module_info = ProbeReport::err(
        "delivery source",
        mode,
        format!("delivery source mode `{mode}` is not implemented"),
    );
    SourceReportBuilder::delivery(module, DeliverySourceReportKind::Unsupported, module_info)
        .finish()
}

async fn raw_http_value(endpoint: &str, path: &str) -> Result<Value> {
    let text = raw_http_text_url(&http_url(endpoint, path)).await?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Value::Null);
    }
    match serde_json::from_str(trimmed) {
        Ok(value) => Ok(value),
        Err(_) => Ok(Value::String(trimmed.to_owned())),
    }
}

async fn raw_http_text_url(url: &str) -> Result<String> {
    read_response_text(
        reqwest::Client::new().get(url),
        url,
        "failed to read http response body",
        true,
    )
    .await
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
}
