use serde_json::{Value, json};

use crate::{
    ProbeReport,
    source_routing::{
        DEFAULT_DELIVERY_METRICS_ENDPOINT, DEFAULT_DELIVERY_REST_ENDPOINT,
        DeliverySourceReportKind, SourceFamily, SourceProbeKey, delivery_source_facts,
        effective_source_mode,
    },
};

use super::base::{
    DELIVERY_MODULE, ModuleReport, call_probe, call_source_probe, http_url, module_info_probe,
    optional, raw_http_text_url, raw_http_value, scalar_field,
};

pub fn delivery_report(info_id: Option<&str>) -> ModuleReport {
    let mut probes = vec![
        call_source_probe(
            DELIVERY_MODULE,
            "version",
            &[],
            SourceProbeKey::DeliveryVersion,
        ),
        call_source_probe(
            DELIVERY_MODULE,
            "getAvailableNodeInfoIDs",
            &[],
            SourceProbeKey::DeliveryAvailableNodeInfoIds,
        ),
        call_source_probe(
            DELIVERY_MODULE,
            "getAvailableConfigs",
            &[],
            SourceProbeKey::DeliveryAvailableConfigs,
        ),
        call_source_probe(
            DELIVERY_MODULE,
            "collectOpenMetricsText",
            &[],
            SourceProbeKey::DeliveryCollectOpenMetricsText,
        ),
    ];
    for (info_id, key) in [
        ("Version", SourceProbeKey::DeliveryNodeInfoVersion),
        ("Metrics", SourceProbeKey::DeliveryNodeInfoMetrics),
        ("MyMultiaddresses", SourceProbeKey::DeliveryMyMultiaddresses),
        ("MyENR", SourceProbeKey::DeliveryMyEnr),
        ("MyPeerId", SourceProbeKey::DeliveryMyPeerId),
    ] {
        probes.push(call_source_probe(
            DELIVERY_MODULE,
            "getNodeInfo",
            &[info_id],
            key,
        ));
    }
    if let Some(info_id) = optional(info_id) {
        let probe = call_probe(DELIVERY_MODULE, "getNodeInfo", &[info_id]);
        probes.push(match delivery_node_info_probe_key(info_id) {
            Some(key) => probe.with_probe_key(key.as_str()),
            None => probe,
        });
    }
    let module_info = module_info_probe(DELIVERY_MODULE);
    ModuleReport::new(DELIVERY_MODULE, module_info.clone(), probes.clone()).with_source_facts(
        delivery_source_facts(DeliverySourceReportKind::Module, &module_info, &probes),
    )
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
    let health_probe = ProbeReport::from_result(
        "delivery_rest.health",
        health_source.clone(),
        health.map(normalize_delivery_health),
    )
    .with_probe_key(SourceProbeKey::DeliveryHealth.as_str());
    let info_probe = ProbeReport::from_result(
        "delivery_rest.info",
        info_source.clone(),
        info.map(normalize_delivery_info),
    )
    .with_probe_key(SourceProbeKey::DeliveryInfo.as_str());
    let version_probe = ProbeReport::from_result(
        "delivery_rest.version",
        version_source.clone(),
        version.map(normalize_delivery_version),
    )
    .with_probe_key(SourceProbeKey::DeliveryVersion.as_str());
    let mut probes = vec![
        info_probe.clone(),
        version_probe.clone(),
        health_probe.clone(),
    ];
    if let Some(value) = health_probe.value.as_ref() {
        push_delivery_probe(
            &mut probes,
            SourceProbeKey::DeliveryNodeHealth,
            "nodeHealth",
            &health_source,
            value,
            &["nodeHealth"],
        );
        push_delivery_probe(
            &mut probes,
            SourceProbeKey::DeliveryConnectionStatus,
            "connectionStatus",
            &health_source,
            value,
            &["connectionStatus"],
        );
        push_delivery_probe(
            &mut probes,
            SourceProbeKey::DeliveryProtocolsHealth,
            "protocolsHealth",
            &health_source,
            value,
            &["protocolsHealth"],
        );
    }
    if let Some(value) = info_probe.value.as_ref() {
        push_delivery_probe(
            &mut probes,
            SourceProbeKey::DeliveryPeerId,
            "peerId",
            &info_source,
            value,
            &["peerId", "peer_id"],
        );
        push_delivery_probe(
            &mut probes,
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
            &mut probes,
            SourceProbeKey::DeliveryEnrUri,
            "enrUri",
            &info_source,
            value,
            &["enrUri", "enr_uri"],
        );
        push_delivery_probe(
            &mut probes,
            SourceProbeKey::DeliveryNodeInfoVersion,
            "Version",
            &info_source,
            value,
            &["version", "Version"],
        );
    }
    if let Some(metrics_endpoint) = optional(metrics_endpoint) {
        probes.push(metrics_probe(metrics_endpoint).await);
    }
    ModuleReport::new("delivery_rest", health_probe.clone(), probes.clone()).with_source_facts(
        delivery_source_facts(DeliverySourceReportKind::Rest, &health_probe, &probes),
    )
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
    probes: &mut Vec<ProbeReport>,
    key: SourceProbeKey,
    method: &str,
    source: &str,
    value: &Value,
    keys: &[&str],
) {
    if let Some(value) = scalar_field(value, keys) {
        probes.push(
            ProbeReport::ok(format!("delivery_rest.{method}"), source, value)
                .with_probe_key(key.as_str()),
        );
    }
}

async fn delivery_metrics_report(metrics_endpoint: Option<&str>) -> ModuleReport {
    let endpoint = optional(metrics_endpoint).unwrap_or(DEFAULT_DELIVERY_METRICS_ENDPOINT);
    let metrics = raw_http_text_url(endpoint).await;
    match metrics {
        Ok(text) => {
            let module_info = ProbeReport::ok(
                "delivery_metrics.scrape",
                endpoint,
                json!({
                    "bytes": text.len(),
                    "lines": text.lines().count(),
                }),
            )
            .with_probe_key(SourceProbeKey::DeliveryMetricsScrape.as_str());
            let probes = vec![
                ProbeReport::ok("delivery_metrics.collectOpenMetricsText", endpoint, text)
                    .with_probe_key(SourceProbeKey::DeliveryCollectOpenMetricsText.as_str()),
            ];
            ModuleReport::new("delivery_metrics", module_info.clone(), probes.clone())
                .with_source_facts(delivery_source_facts(
                    DeliverySourceReportKind::Metrics,
                    &module_info,
                    &probes,
                ))
        }
        Err(error) => {
            let error = error.to_string();
            let module_info = ProbeReport::err("delivery_metrics.scrape", endpoint, &error)
                .with_probe_key(SourceProbeKey::DeliveryMetricsScrape.as_str());
            let probes = vec![
                ProbeReport::err("delivery_metrics.collectOpenMetricsText", endpoint, error)
                    .with_probe_key(SourceProbeKey::DeliveryCollectOpenMetricsText.as_str()),
            ];
            ModuleReport::new("delivery_metrics", module_info.clone(), probes.clone())
                .with_source_facts(delivery_source_facts(
                    DeliverySourceReportKind::Metrics,
                    &module_info,
                    &probes,
                ))
        }
    }
}

async fn metrics_probe(metrics_endpoint: &str) -> ProbeReport {
    ProbeReport::from_result(
        "delivery_rest.collectOpenMetricsText",
        metrics_endpoint,
        raw_http_text_url(metrics_endpoint).await,
    )
    .with_probe_key(SourceProbeKey::DeliveryCollectOpenMetricsText.as_str())
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
    let all_peers_probe = ProbeReport::from_result(
        "delivery_network_monitor.allPeersInfo",
        all_peers_source,
        all_peers,
    )
    .with_probe_key(SourceProbeKey::DeliveryAllPeersInfo.as_str());
    let mut probes = vec![
        all_peers_probe.clone(),
        ProbeReport::from_result(
            "delivery_network_monitor.contentTopics",
            content_topics_source,
            content_topics,
        )
        .with_probe_key(SourceProbeKey::DeliveryContentTopics.as_str()),
    ];
    if let Some(metrics_endpoint) = optional(metrics_endpoint) {
        probes.push(
            ProbeReport::from_result(
                "delivery_network_monitor.collectOpenMetricsText",
                metrics_endpoint,
                raw_http_text_url(metrics_endpoint).await,
            )
            .with_probe_key(SourceProbeKey::DeliveryCollectOpenMetricsText.as_str()),
        );
    }
    ModuleReport::new(
        "delivery_network_monitor",
        all_peers_probe.clone(),
        probes.clone(),
    )
    .with_source_facts(delivery_source_facts(
        DeliverySourceReportKind::NetworkMonitor,
        &all_peers_probe,
        &probes,
    ))
}

fn unsupported_delivery_source_report(mode: &str) -> ModuleReport {
    let module = format!("delivery_{mode}");
    let module_info = ProbeReport::err(
        "delivery source",
        mode,
        format!("delivery source mode `{mode}` is not implemented"),
    );
    let probes = Vec::new();
    ModuleReport::new(module.clone(), module_info.clone(), probes.clone()).with_source_facts(
        delivery_source_facts(DeliverySourceReportKind::Unsupported, &module_info, &probes),
    )
}

fn delivery_node_info_probe_key(info_id: &str) -> Option<SourceProbeKey> {
    match info_id {
        "Version" => Some(SourceProbeKey::DeliveryNodeInfoVersion),
        "Metrics" => Some(SourceProbeKey::DeliveryNodeInfoMetrics),
        "MyMultiaddresses" => Some(SourceProbeKey::DeliveryMyMultiaddresses),
        "MyENR" => Some(SourceProbeKey::DeliveryMyEnr),
        "MyPeerId" => Some(SourceProbeKey::DeliveryMyPeerId),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

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
