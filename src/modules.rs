use anyhow::{Context as _, Result, bail};
use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    ProbeReport, logoscore, response_excerpt,
    source_policy::{
        DEFAULT_DELIVERY_METRICS_ENDPOINT, DEFAULT_DELIVERY_REST_ENDPOINT,
        DEFAULT_STORAGE_METRICS_ENDPOINT, DEFAULT_STORAGE_REST_ENDPOINT, DeliverySourceMode,
        StorageSourceMode,
    },
};

const BLOCKCHAIN_MODULE: &str = "blockchain_module";
const STORAGE_MODULE: &str = "storage_module";
const DELIVERY_MODULE: &str = "delivery_module";
const CAPABILITY_MODULE: &str = "capability_module";

#[derive(Debug, Clone, Serialize)]
pub struct LogosModulesReport {
    pub status: ProbeReport,
    pub blockchain: ModuleReport,
    pub storage: ModuleReport,
    pub delivery: ModuleReport,
    pub capabilities: ModuleReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleReport {
    pub module: String,
    pub module_info: ProbeReport,
    pub probes: Vec<ProbeReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<SourceHealthFacts>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub capability_facts: Vec<SourceCapabilityFact>,
}

impl ModuleReport {
    fn new(module: impl Into<String>, module_info: ProbeReport, probes: Vec<ProbeReport>) -> Self {
        Self {
            module: module.into(),
            module_info,
            probes,
            health: None,
            capability_facts: Vec::new(),
        }
    }

    fn with_source_facts(
        mut self,
        health: SourceHealthFacts,
        capability_facts: Vec<SourceCapabilityFact>,
    ) -> Self {
        self.health = Some(health);
        self.capability_facts = capability_facts;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceHealthFacts {
    pub reachable: bool,
    pub ready: bool,
    pub status: SourceHealthStatus,
    pub summary: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceHealthStatus {
    Healthy,
    Degraded,
    Unavailable,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceCapabilityFact {
    pub key: String,
    pub label: String,
    pub available: bool,
    pub evidence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
}

impl SourceCapabilityFact {
    fn available(
        key: impl Into<String>,
        label: impl Into<String>,
        evidence: impl Into<String>,
        value: Option<Value>,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            available: true,
            evidence: evidence.into(),
            value,
        }
    }

    fn unavailable(
        key: impl Into<String>,
        label: impl Into<String>,
        evidence: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            available: false,
            evidence: evidence.into(),
            value: None,
        }
    }
}

pub fn logoscore_status_report() -> ProbeReport {
    ProbeReport::from_result(
        "logoscore status",
        "logoscore status --json",
        logoscore::status(),
    )
}

pub fn modules_report() -> LogosModulesReport {
    LogosModulesReport {
        status: logoscore_status_report(),
        blockchain: blockchain_module_report(None),
        storage: storage_report(None, false),
        delivery: delivery_report(None),
        capabilities: capabilities_report(),
    }
}

pub fn blockchain_module_report(address: Option<&str>) -> ModuleReport {
    let mut probes = vec![
        call_probe(BLOCKCHAIN_MODULE, "get_peer_id", &[]),
        call_probe(BLOCKCHAIN_MODULE, "get_cryptarchia_info", &[]),
        call_probe(BLOCKCHAIN_MODULE, "get_blockchain_state", &[]),
        call_probe(BLOCKCHAIN_MODULE, "wallet_get_known_addresses", &[]),
    ];
    if let Some(address) = optional(address) {
        probes.extend([
            call_probe(BLOCKCHAIN_MODULE, "wallet_get_balance", &[address]),
            call_probe(BLOCKCHAIN_MODULE, "wallet_get_notes", &[address]),
            call_probe(
                BLOCKCHAIN_MODULE,
                "wallet_get_claimable_vouchers",
                &[address],
            ),
        ]);
    }
    ModuleReport::new(
        BLOCKCHAIN_MODULE,
        module_info_probe(BLOCKCHAIN_MODULE),
        probes,
    )
}

pub fn storage_report(cid: Option<&str>, privileged_debug_enabled: bool) -> ModuleReport {
    let mut probes = vec![
        call_probe(STORAGE_MODULE, "version", &[]),
        call_probe(STORAGE_MODULE, "moduleVersion", &[]),
        call_probe(STORAGE_MODULE, "dataDir", &[]),
        call_probe(STORAGE_MODULE, "peerId", &[]),
        call_probe(STORAGE_MODULE, "spr", &[]),
        call_probe(STORAGE_MODULE, "space", &[]),
        call_probe(STORAGE_MODULE, "manifests", &[]),
        call_probe(STORAGE_MODULE, "collectMetrics", &[]),
    ];
    if privileged_debug_enabled {
        probes.push(call_probe(STORAGE_MODULE, "debug", &[]));
    }
    if let Some(cid) = optional(cid) {
        probes.push(call_probe(STORAGE_MODULE, "exists", &[cid]));
    }
    let module_info = module_info_probe(STORAGE_MODULE);
    ModuleReport::new(STORAGE_MODULE, module_info.clone(), probes.clone()).with_source_facts(
        storage_source_health(STORAGE_MODULE, &module_info, &probes),
        storage_source_capability_facts(STORAGE_MODULE, &module_info, &probes),
    )
}

pub async fn storage_source_report(
    source_mode: &str,
    rest_endpoint: Option<&str>,
    metrics_endpoint: Option<&str>,
    cid: Option<&str>,
    privileged_debug_enabled: bool,
) -> ModuleReport {
    match normalized_storage_source_mode(source_mode) {
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
    let space_probe = ProbeReport::from_result("storage_rest.space", space_source.clone(), space);
    let mut probes = vec![
        space_probe.clone(),
        ProbeReport::from_result("storage_rest.spr", spr_source, spr),
        ProbeReport::from_result("storage_rest.peerId", peer_id_source, peer_id),
        ProbeReport::from_result("storage_rest.manifests", data_source, manifests),
    ];
    if privileged_debug_enabled {
        let debug_source = http_url(endpoint, "/debug/info");
        probes.push(ProbeReport::from_result(
            "storage_rest.debug",
            debug_source,
            raw_http_value(endpoint, "/debug/info").await,
        ));
    } else {
        probes.push(ProbeReport::ok(
            "storage_rest.privilegedProbe",
            "disabled",
            json!({ "skipped": true }),
        ));
    }
    if let Some(cid) = optional(cid) {
        let path = format!("/data/{cid}/exists");
        probes.push(ProbeReport::from_result(
            "storage_rest.exists",
            http_url(endpoint, &path),
            raw_http_value(endpoint, &path)
                .await
                .map(|value| normalize_storage_exists(value, cid)),
        ));
    }
    if let Some(metrics_endpoint) = optional(metrics_endpoint) {
        probes.push(storage_metrics_probe(metrics_endpoint).await);
    }
    ModuleReport::new("storage_rest", space_probe.clone(), probes.clone()).with_source_facts(
        storage_source_health("storage_rest", &space_probe, &probes),
        storage_source_capability_facts("storage_rest", &space_probe, &probes),
    )
}

async fn storage_metrics_report(metrics_endpoint: Option<&str>) -> ModuleReport {
    let endpoint = optional(metrics_endpoint).unwrap_or(DEFAULT_STORAGE_METRICS_ENDPOINT);
    let metrics = raw_http_text_url(endpoint).await;
    match metrics {
        Ok(text) => {
            let module_info = ProbeReport::ok(
                "storage_metrics.scrape",
                endpoint,
                json!({
                    "bytes": text.len(),
                    "lines": text.lines().count(),
                }),
            );
            let probes = vec![ProbeReport::ok(
                "storage_metrics.collectMetrics",
                endpoint,
                text,
            )];
            ModuleReport::new("storage_metrics", module_info.clone(), probes.clone())
                .with_source_facts(
                    storage_source_health("storage_metrics", &module_info, &probes),
                    storage_source_capability_facts("storage_metrics", &module_info, &probes),
                )
        }
        Err(error) => {
            let error = error.to_string();
            let module_info = ProbeReport::err("storage_metrics.scrape", endpoint, &error);
            let probes = vec![ProbeReport::err(
                "storage_metrics.collectMetrics",
                endpoint,
                error,
            )];
            ModuleReport::new("storage_metrics", module_info.clone(), probes.clone())
                .with_source_facts(
                    storage_source_health("storage_metrics", &module_info, &probes),
                    storage_source_capability_facts("storage_metrics", &module_info, &probes),
                )
        }
    }
}

async fn storage_metrics_probe(metrics_endpoint: &str) -> ProbeReport {
    ProbeReport::from_result(
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
    let probes = Vec::new();
    ModuleReport::new(module.clone(), module_info.clone(), probes.clone()).with_source_facts(
        unsupported_source_health(&module_info),
        storage_source_capability_facts(&module, &module_info, &probes),
    )
}

fn normalized_storage_source_mode(source_mode: &str) -> &'static str {
    StorageSourceMode::from_token(source_mode)
        .effective()
        .as_str()
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

pub fn delivery_report(info_id: Option<&str>) -> ModuleReport {
    let mut probes = vec![
        call_probe(DELIVERY_MODULE, "version", &[]),
        call_probe(DELIVERY_MODULE, "getAvailableNodeInfoIDs", &[]),
        call_probe(DELIVERY_MODULE, "getAvailableConfigs", &[]),
        call_probe(DELIVERY_MODULE, "collectOpenMetricsText", &[]),
    ];
    for info_id in [
        "Version",
        "Metrics",
        "MyMultiaddresses",
        "MyENR",
        "MyPeerId",
    ] {
        probes.push(call_probe(DELIVERY_MODULE, "getNodeInfo", &[info_id]));
    }
    if let Some(info_id) = optional(info_id) {
        probes.push(call_probe(DELIVERY_MODULE, "getNodeInfo", &[info_id]));
    }
    let module_info = module_info_probe(DELIVERY_MODULE);
    ModuleReport::new(DELIVERY_MODULE, module_info.clone(), probes.clone()).with_source_facts(
        delivery_source_health(DELIVERY_MODULE, &module_info, &probes),
        delivery_source_capability_facts(DELIVERY_MODULE, &module_info, &probes),
    )
}

pub async fn delivery_source_report(
    source_mode: &str,
    rest_endpoint: Option<&str>,
    metrics_endpoint: Option<&str>,
) -> ModuleReport {
    match normalized_delivery_source_mode(source_mode) {
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
    );
    let info_probe = ProbeReport::from_result(
        "delivery_rest.info",
        info_source.clone(),
        info.map(normalize_delivery_info),
    );
    let version_probe = ProbeReport::from_result(
        "delivery_rest.version",
        version_source.clone(),
        version.map(normalize_delivery_version),
    );
    let mut probes = vec![
        info_probe.clone(),
        version_probe.clone(),
        health_probe.clone(),
    ];
    if let Some(value) = health_probe.value.as_ref() {
        push_delivery_probe(
            &mut probes,
            "nodeHealth",
            &health_source,
            value,
            &["nodeHealth"],
        );
        push_delivery_probe(
            &mut probes,
            "connectionStatus",
            &health_source,
            value,
            &["connectionStatus"],
        );
        push_delivery_probe(
            &mut probes,
            "protocolsHealth",
            &health_source,
            value,
            &["protocolsHealth"],
        );
    }
    if let Some(value) = info_probe.value.as_ref() {
        push_delivery_probe(
            &mut probes,
            "peerId",
            &info_source,
            value,
            &["peerId", "peer_id"],
        );
        push_delivery_probe(
            &mut probes,
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
            "enrUri",
            &info_source,
            value,
            &["enrUri", "enr_uri"],
        );
        push_delivery_probe(
            &mut probes,
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
        delivery_source_health("delivery_rest", &health_probe, &probes),
        delivery_source_capability_facts("delivery_rest", &health_probe, &probes),
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
    method: &str,
    source: &str,
    value: &Value,
    keys: &[&str],
) {
    if let Some(value) = scalar_field(value, keys) {
        probes.push(ProbeReport::ok(
            format!("delivery_rest.{method}"),
            source,
            value,
        ));
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
            );
            let probes = vec![ProbeReport::ok(
                "delivery_metrics.collectOpenMetricsText",
                endpoint,
                text,
            )];
            ModuleReport::new("delivery_metrics", module_info.clone(), probes.clone())
                .with_source_facts(
                    delivery_source_health("delivery_metrics", &module_info, &probes),
                    delivery_source_capability_facts("delivery_metrics", &module_info, &probes),
                )
        }
        Err(error) => {
            let error = error.to_string();
            let module_info = ProbeReport::err("delivery_metrics.scrape", endpoint, &error);
            let probes = vec![ProbeReport::err(
                "delivery_metrics.collectOpenMetricsText",
                endpoint,
                error,
            )];
            ModuleReport::new("delivery_metrics", module_info.clone(), probes.clone())
                .with_source_facts(
                    delivery_source_health("delivery_metrics", &module_info, &probes),
                    delivery_source_capability_facts("delivery_metrics", &module_info, &probes),
                )
        }
    }
}

async fn metrics_probe(metrics_endpoint: &str) -> ProbeReport {
    ProbeReport::from_result(
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
    let all_peers_probe = ProbeReport::from_result(
        "delivery_network_monitor.allPeersInfo",
        all_peers_source,
        all_peers,
    );
    let mut probes = vec![
        all_peers_probe.clone(),
        ProbeReport::from_result(
            "delivery_network_monitor.contentTopics",
            content_topics_source,
            content_topics,
        ),
    ];
    if let Some(metrics_endpoint) = optional(metrics_endpoint) {
        probes.push(ProbeReport::from_result(
            "delivery_network_monitor.collectOpenMetricsText",
            metrics_endpoint,
            raw_http_text_url(metrics_endpoint).await,
        ));
    }
    ModuleReport::new(
        "delivery_network_monitor",
        all_peers_probe.clone(),
        probes.clone(),
    )
    .with_source_facts(
        delivery_source_health("delivery_network_monitor", &all_peers_probe, &probes),
        delivery_source_capability_facts("delivery_network_monitor", &all_peers_probe, &probes),
    )
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
        unsupported_source_health(&module_info),
        delivery_source_capability_facts(&module, &module_info, &probes),
    )
}

fn normalized_delivery_source_mode(source_mode: &str) -> &'static str {
    DeliverySourceMode::from_token(source_mode)
        .effective()
        .as_str()
}

fn storage_source_health(
    module: &str,
    module_info: &ProbeReport,
    probes: &[ProbeReport],
) -> SourceHealthFacts {
    let reachable = source_reachable(module_info, probes);
    let ready = match module {
        "storage_rest" => {
            probe_ok(module_info, probes, "peerId")
                && probe_ok(module_info, probes, "spr")
                && probe_ok(module_info, probes, "space")
                && probe_ok(module_info, probes, "manifests")
        }
        "storage_metrics" => storage_metrics_evidence_present(module_info, probes),
        STORAGE_MODULE => ["peerId", "spr", "space", "debug", "manifests"]
            .iter()
            .any(|method| probe_ok(module_info, probes, method)),
        _ => false,
    };
    source_health(
        reachable,
        ready,
        false,
        if ready {
            source_ready_summary("storage source ready", module_info, probes)
        } else if reachable {
            "storage source degraded".to_owned()
        } else {
            "storage source unavailable".to_owned()
        },
        if ready {
            "required storage facts observed".to_owned()
        } else {
            source_report_error(module_info, probes)
                .unwrap_or_else(|| "required storage facts missing".to_owned())
        },
    )
}

fn delivery_source_health(
    module: &str,
    module_info: &ProbeReport,
    probes: &[ProbeReport],
) -> SourceHealthFacts {
    let reachable = source_reachable(module_info, probes);
    let metrics_ready = delivery_metrics_evidence_present(module_info, probes);
    let ready = match module {
        "delivery_metrics" => metrics_ready,
        "delivery_network_monitor" => {
            probe_ok(module_info, probes, "allPeersInfo")
                || probe_ok(module_info, probes, "contentTopics")
                || metrics_ready
        }
        "delivery_rest" => {
            probe_ok(module_info, probes, "health")
                && health_value_ok(probe_value(module_info, probes, "nodeHealth"), false)
                && health_value_ok(probe_value(module_info, probes, "connectionStatus"), false)
        }
        DELIVERY_MODULE => {
            let node_health = report_probe(module_info, probes, "nodeHealth");
            let connection_status = report_probe(module_info, probes, "connectionStatus");
            if node_health.is_none() && connection_status.is_none() {
                delivery_module_runtime_healthy(module_info, probes)
            } else {
                health_value_ok(probe_value(module_info, probes, "nodeHealth"), false)
                    && health_value_ok(probe_value(module_info, probes, "connectionStatus"), false)
            }
        }
        _ => false,
    };
    source_health(
        reachable,
        ready,
        false,
        if ready {
            source_ready_summary("delivery source ready", module_info, probes)
        } else if reachable {
            "delivery source degraded".to_owned()
        } else {
            "delivery source unavailable".to_owned()
        },
        if ready {
            delivery_ready_detail(module, module_info, probes)
        } else {
            source_report_error(module_info, probes)
                .unwrap_or_else(|| "required delivery facts missing".to_owned())
        },
    )
}

fn unsupported_source_health(module_info: &ProbeReport) -> SourceHealthFacts {
    source_health(
        false,
        false,
        true,
        "unsupported source".to_owned(),
        source_report_error(module_info, &[])
            .unwrap_or_else(|| "source mode unsupported".to_owned()),
    )
}

fn source_health(
    reachable: bool,
    ready: bool,
    unsupported: bool,
    summary: String,
    detail: String,
) -> SourceHealthFacts {
    let status = if unsupported {
        SourceHealthStatus::Unsupported
    } else if ready {
        SourceHealthStatus::Healthy
    } else if reachable {
        SourceHealthStatus::Degraded
    } else {
        SourceHealthStatus::Unavailable
    };
    SourceHealthFacts {
        reachable,
        ready,
        status,
        summary,
        detail,
    }
}

fn storage_source_capability_facts(
    module: &str,
    module_info: &ProbeReport,
    probes: &[ProbeReport],
) -> Vec<SourceCapabilityFact> {
    let mut facts = vec![
        any_probe_fact(
            module_info,
            probes,
            "identity",
            "Identity",
            &["peerId", "spr"],
        ),
        any_probe_fact(module_info, probes, "space", "Repository space", &["space"]),
        any_probe_fact(
            module_info,
            probes,
            "manifest_listing",
            "Manifest listing",
            &["manifests"],
        ),
        any_probe_fact(module_info, probes, "debug", "Debug topology", &["debug"]),
        storage_metrics_fact(module_info, probes),
    ];
    if let Some(probe) = report_probe(module_info, probes, "exists") {
        facts.push(probe_fact("cid_exists", "CID existence", probe));
    }
    if module == "storage_rest" {
        facts.push(SourceCapabilityFact::available(
            "rest_api",
            "REST API",
            "REST probes available",
            None,
        ));
    } else if module == STORAGE_MODULE {
        facts.push(SourceCapabilityFact::available(
            "module_api",
            "Module API",
            "LogosCore storage module available",
            None,
        ));
    }
    facts
}

fn delivery_source_capability_facts(
    module: &str,
    module_info: &ProbeReport,
    probes: &[ProbeReport],
) -> Vec<SourceCapabilityFact> {
    let mut facts = vec![
        any_probe_fact(
            module_info,
            probes,
            "identity",
            "Identity",
            &[
                "peerId",
                "MyPeerId",
                "enrUri",
                "MyENR",
                "listenAddresses",
                "MyMultiaddresses",
            ],
        ),
        any_probe_fact(
            module_info,
            probes,
            "health",
            "Health endpoint",
            &["health", "nodeHealth", "connectionStatus"],
        ),
        delivery_metrics_fact(module_info, probes),
        delivery_protocol_fact(
            module_info,
            probes,
            "relay",
            "Relay",
            &[
                "waku_relay",
                "waku_pubsub",
                "libp2p_pubsub_peers",
                "waku_node_messages_total",
            ],
            &["relay"],
        ),
        delivery_protocol_fact(
            module_info,
            probes,
            "store",
            "Store",
            &[
                "waku_store",
                "waku_store_peers",
                "waku_store_messages",
                "waku_store_queries_total",
            ],
            &["store"],
        ),
        delivery_protocol_fact(
            module_info,
            probes,
            "filter",
            "Filter",
            &["waku_filter", "waku_filter_peers", "waku_filter_requests"],
            &["filter"],
        ),
        delivery_protocol_fact(
            module_info,
            probes,
            "lightpush",
            "Lightpush",
            &["waku_lightpush", "waku_lightpush_peers", "lightpush"],
            &["lightpush"],
        ),
        any_probe_fact(
            module_info,
            probes,
            "network_monitor",
            "Network monitor",
            &["allPeersInfo", "contentTopics"],
        ),
    ];
    if module == "delivery_rest" {
        facts.push(SourceCapabilityFact::available(
            "rest_api",
            "REST API",
            "REST probes available",
            None,
        ));
    } else if module == DELIVERY_MODULE {
        facts.push(SourceCapabilityFact::available(
            "module_api",
            "Module API",
            "LogosCore delivery module available",
            None,
        ));
    }
    facts
}

fn source_ready_summary(
    fallback: &str,
    module_info: &ProbeReport,
    probes: &[ProbeReport],
) -> String {
    ["version", "moduleVersion", "Version"]
        .iter()
        .find_map(|method| probe_value(module_info, probes, method))
        .map(value_summary)
        .filter(|value| !value.is_empty() && value != "n/a")
        .map(|value| format!("version {value}"))
        .unwrap_or_else(|| fallback.to_owned())
}

fn delivery_ready_detail(
    module: &str,
    module_info: &ProbeReport,
    probes: &[ProbeReport],
) -> String {
    if module == "delivery_rest" {
        let node = probe_value(module_info, probes, "nodeHealth")
            .map(value_summary)
            .unwrap_or_else(|| "unknown".to_owned());
        let connection = probe_value(module_info, probes, "connectionStatus")
            .map(value_summary)
            .unwrap_or_else(|| "unknown".to_owned());
        return format!("node health {node}; connection {connection}");
    }
    if delivery_metrics_evidence_present(module_info, probes) {
        return "delivery metrics observed".to_owned();
    }
    "required delivery facts observed".to_owned()
}

fn source_reachable(module_info: &ProbeReport, probes: &[ProbeReport]) -> bool {
    module_info.ok || probes.iter().any(|probe| probe.ok)
}

fn source_report_error(module_info: &ProbeReport, probes: &[ProbeReport]) -> Option<String> {
    module_info
        .error
        .as_ref()
        .filter(|error| !error.is_empty())
        .cloned()
        .or_else(|| {
            probes.iter().find_map(|probe| {
                probe
                    .error
                    .as_ref()
                    .filter(|error| !error.is_empty())
                    .cloned()
            })
        })
}

fn any_probe_fact(
    module_info: &ProbeReport,
    probes: &[ProbeReport],
    key: &str,
    label: &str,
    methods: &[&str],
) -> SourceCapabilityFact {
    let mut fallback = None;
    for method in methods {
        if let Some(probe) = report_probe(module_info, probes, method) {
            if probe.ok {
                return probe_fact(key, label, probe);
            }
            if fallback.is_none() {
                fallback = Some(probe);
            }
        }
    }
    fallback.map_or_else(
        || SourceCapabilityFact::unavailable(key, label, "not observed"),
        |probe| probe_fact(key, label, probe),
    )
}

fn probe_fact(key: &str, label: &str, probe: &ProbeReport) -> SourceCapabilityFact {
    if probe.ok {
        SourceCapabilityFact::available(
            key,
            label,
            probe
                .value
                .as_ref()
                .map(value_summary)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "observed".to_owned()),
            probe.value.clone(),
        )
    } else {
        SourceCapabilityFact::unavailable(
            key,
            label,
            probe
                .error
                .clone()
                .filter(|error| !error.is_empty())
                .unwrap_or_else(|| "unavailable".to_owned()),
        )
    }
}

fn storage_metrics_fact(module_info: &ProbeReport, probes: &[ProbeReport]) -> SourceCapabilityFact {
    let probe = report_probe(module_info, probes, "collectMetrics");
    if storage_metrics_evidence_present(module_info, probes) {
        return SourceCapabilityFact::available(
            "metrics",
            "Metrics",
            "OpenMetrics text observed",
            probe.and_then(|probe| probe.value.clone()),
        );
    }
    probe.map_or_else(
        || SourceCapabilityFact::unavailable("metrics", "Metrics", "not observed"),
        |probe| {
            if probe.ok {
                SourceCapabilityFact::unavailable("metrics", "Metrics", "metrics response empty")
            } else {
                probe_fact("metrics", "Metrics", probe)
            }
        },
    )
}

fn delivery_metrics_fact(
    module_info: &ProbeReport,
    probes: &[ProbeReport],
) -> SourceCapabilityFact {
    let probe = report_probe(module_info, probes, "collectOpenMetricsText")
        .or_else(|| report_probe(module_info, probes, "Metrics"));
    if delivery_metrics_evidence_present(module_info, probes) {
        return SourceCapabilityFact::available(
            "metrics",
            "Metrics",
            "known Waku metric family observed",
            probe.and_then(|probe| probe.value.clone()),
        );
    }
    probe.map_or_else(
        || SourceCapabilityFact::unavailable("metrics", "Metrics", "not observed"),
        |probe| {
            if probe.ok {
                SourceCapabilityFact::unavailable(
                    "metrics",
                    "Metrics",
                    "no known Waku metric family observed",
                )
            } else {
                probe_fact("metrics", "Metrics", probe)
            }
        },
    )
}

fn delivery_protocol_fact(
    module_info: &ProbeReport,
    probes: &[ProbeReport],
    key: &str,
    label: &str,
    metric_needles: &[&str],
    protocol_needles: &[&str],
) -> SourceCapabilityFact {
    if metric_text_contains(module_info, probes, metric_needles) {
        return SourceCapabilityFact::available(key, label, "metric family observed", None);
    }
    if protocol_health_contains(module_info, probes, protocol_needles) {
        return SourceCapabilityFact::available(key, label, "protocol health observed", None);
    }
    SourceCapabilityFact::unavailable(key, label, "not observed")
}

fn storage_metrics_evidence_present(module_info: &ProbeReport, probes: &[ProbeReport]) -> bool {
    open_metrics_text(module_info, probes, &["collectMetrics"])
        .map(|text| text.lines().any(|line| !line.trim().is_empty()))
        .unwrap_or(false)
}

fn delivery_metrics_evidence_present(module_info: &ProbeReport, probes: &[ProbeReport]) -> bool {
    metric_text_contains(
        module_info,
        probes,
        &[
            "libp2p_peers",
            "waku_peers",
            "libp2p_pubsub_peers",
            "waku_node_messages_total",
            "waku_node_errors_total",
            "waku_store_queries_total",
            "waku_filter_peers",
            "waku_lightpush_peers",
        ],
    )
}

fn metric_text_contains(
    module_info: &ProbeReport,
    probes: &[ProbeReport],
    needles: &[&str],
) -> bool {
    open_metrics_text(
        module_info,
        probes,
        &["collectOpenMetricsText", "collectMetrics", "Metrics"],
    )
    .map(|text| needles.iter().any(|needle| text.contains(needle)))
    .unwrap_or(false)
}

fn protocol_health_contains(
    module_info: &ProbeReport,
    probes: &[ProbeReport],
    needles: &[&str],
) -> bool {
    probe_value(module_info, probes, "protocolsHealth")
        .map(|value| value.to_string().to_lowercase())
        .map(|text| needles.iter().any(|needle| text.contains(needle)))
        .unwrap_or(false)
}

fn delivery_module_runtime_healthy(module_info: &ProbeReport, probes: &[ProbeReport]) -> bool {
    ["Metrics", "collectOpenMetricsText"]
        .iter()
        .any(|method| probe_has_runtime_value(report_probe(module_info, probes, method)))
}

fn probe_has_runtime_value(probe: Option<&ProbeReport>) -> bool {
    let Some(probe) = probe else {
        return false;
    };
    if !probe.ok {
        return false;
    }
    match probe.value.as_ref() {
        Some(Value::Array(items)) => !items.is_empty(),
        Some(Value::Object(object)) => !object.is_empty(),
        Some(value) => !value_summary(value).trim().is_empty(),
        None => false,
    }
}

fn open_metrics_text(
    module_info: &ProbeReport,
    probes: &[ProbeReport],
    methods: &[&str],
) -> Option<String> {
    for method in methods {
        let Some(value) = probe_value(module_info, probes, method) else {
            continue;
        };
        if let Some(text) = open_metrics_text_from_value(value)
            && !text.trim().is_empty()
        {
            return Some(text);
        }
    }
    None
}

fn open_metrics_text_from_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Object(object) => ["value", "result", "metrics", "text"]
            .iter()
            .find_map(|key| object.get(*key).and_then(open_metrics_text_from_value)),
        _ => None,
    }
}

fn probe_ok(module_info: &ProbeReport, probes: &[ProbeReport], method: &str) -> bool {
    report_probe(module_info, probes, method)
        .map(|probe| probe.ok)
        .unwrap_or(false)
}

fn probe_value<'a>(
    module_info: &'a ProbeReport,
    probes: &'a [ProbeReport],
    method: &str,
) -> Option<&'a Value> {
    report_probe(module_info, probes, method)
        .filter(|probe| probe.ok)
        .and_then(|probe| probe.value.as_ref())
}

fn report_probe<'a>(
    module_info: &'a ProbeReport,
    probes: &'a [ProbeReport],
    method: &str,
) -> Option<&'a ProbeReport> {
    if probe_matches(module_info, method) {
        return Some(module_info);
    }
    probes.iter().find(|probe| probe_matches(probe, method))
}

fn probe_matches(probe: &ProbeReport, method: &str) -> bool {
    if method.is_empty() {
        return false;
    }
    let label = probe.label.as_str();
    let source = probe.source.as_str();
    label.contains(&format!(".{method}"))
        || label.contains(&format!("({method})"))
        || label.ends_with(method)
        || source.contains(&format!(" {method}"))
        || source.contains(&format!("/{method}"))
}

fn health_value_ok(value: Option<&Value>, unknown_ok: bool) -> bool {
    let Some(value) = value else {
        return unknown_ok;
    };
    if let Some(boolean) = value.as_bool() {
        return boolean;
    }
    let text = scalar_text(value)
        .unwrap_or_else(|| value.to_string())
        .trim()
        .to_lowercase();
    if text.is_empty() {
        return unknown_ok;
    }
    let normalized = text
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect::<String>();
    if ["ready", "healthy", "ok", "connected", "true"].contains(&normalized.as_str()) {
        return true;
    }
    if [
        "initializing",
        "synchronizing",
        "notready",
        "notmounted",
        "shuttingdown",
        "eventlooplagging",
        "disconnected",
        "partiallyconnected",
        "false",
    ]
    .contains(&normalized.as_str())
        || text.contains("not")
        || text.contains("unhealthy")
        || text.contains("error")
        || text.contains("fail")
        || text.contains("down")
        || text.contains("disconnect")
    {
        return false;
    }
    unknown_ok
}

fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::String(value) => Some(value.clone()),
        Value::Object(object) => ["value", "result", "status", "health"]
            .iter()
            .find_map(|key| object.get(*key).and_then(scalar_text)),
        Value::Array(_) => None,
    }
}

fn value_summary(value: &Value) -> String {
    match value {
        Value::Null => "n/a".to_owned(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(items) => {
            if items.is_empty() {
                "empty".to_owned()
            } else {
                format!("{} item(s)", items.len())
            }
        }
        Value::Object(object) => ["result", "value"]
            .iter()
            .find_map(|key| object.get(*key).map(value_summary))
            .unwrap_or_else(|| format!("{} field(s)", object.len())),
    }
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
    let response = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to call {url}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .context("failed to read http response body")?;
    if !status.is_success() {
        bail!(
            "http call `{url}` failed with status {status}: {}",
            response_excerpt(&text)
        );
    }
    Ok(text)
}

fn http_url(endpoint: &str, path: &str) -> String {
    let endpoint = endpoint.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    format!("{endpoint}/{path}")
}

pub fn capabilities_report() -> ModuleReport {
    ModuleReport::new(
        CAPABILITY_MODULE,
        module_info_probe(CAPABILITY_MODULE),
        Vec::new(),
    )
}

fn module_info_probe(module: &str) -> ProbeReport {
    ProbeReport::from_result(
        format!("{module} info"),
        format!("logoscore module-info {module} --json"),
        logoscore::module_info(module),
    )
}

fn call_probe(module: &str, method: &str, args: &[&str]) -> ProbeReport {
    let args = args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
    let args_label = if args.is_empty() {
        String::new()
    } else {
        format!("({})", args.join(", "))
    };
    let source_args = if args.is_empty() {
        String::new()
    } else {
        format!(" {}", args.join(" "))
    };
    ProbeReport::from_result(
        format!("{module}.{method}{args_label}"),
        format!("logoscore call {module} {method}{source_args}"),
        logoscore::call(module, method, &args),
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
    fn storage_source_normalizer_keeps_module_source() {
        assert_eq!(normalized_storage_source_mode("module"), "module");
        assert_eq!(normalized_storage_source_mode("basecamp module"), "module");
    }

    #[test]
    fn delivery_source_normalizer_keeps_network_monitor_source() {
        assert_eq!(
            normalized_delivery_source_mode("network-monitor"),
            "network-monitor"
        );
        assert_eq!(
            normalized_delivery_source_mode("discovery crawler"),
            "network-monitor"
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
    fn storage_source_health_requires_rest_core_facts() {
        let module_info = ProbeReport::ok("storage_rest.space", "http://storage/space", json!({}));
        let probes = vec![
            module_info.clone(),
            ProbeReport::ok("storage_rest.spr", "http://storage/spr", "spr-a"),
            ProbeReport::ok("storage_rest.peerId", "http://storage/peerid", "peer-a"),
            ProbeReport::ok("storage_rest.manifests", "http://storage/data", json!([])),
        ];

        let health = storage_source_health("storage_rest", &module_info, &probes);
        let facts = storage_source_capability_facts("storage_rest", &module_info, &probes);

        assert!(health.reachable);
        assert!(health.ready);
        assert_eq!(health.status, SourceHealthStatus::Healthy);
        assert!(
            facts
                .iter()
                .any(|fact| fact.key == "identity" && fact.available)
        );
    }

    #[test]
    fn storage_source_health_marks_rest_missing_facts_degraded() {
        let module_info = ProbeReport::ok("storage_rest.space", "http://storage/space", json!({}));
        let probes = vec![module_info.clone()];

        let health = storage_source_health("storage_rest", &module_info, &probes);

        assert!(health.reachable);
        assert!(!health.ready);
        assert_eq!(health.status, SourceHealthStatus::Degraded);
    }

    #[test]
    fn delivery_source_health_requires_known_metrics_family() {
        let module_info = ProbeReport::ok(
            "delivery_metrics.scrape",
            "http://metrics",
            json!({ "bytes": 20, "lines": 1 }),
        );
        let generic_metrics = vec![ProbeReport::ok(
            "delivery_metrics.collectOpenMetricsText",
            "http://metrics",
            "process_cpu_seconds_total 3\n",
        )];
        let waku_metrics = vec![ProbeReport::ok(
            "delivery_metrics.collectOpenMetricsText",
            "http://metrics",
            "waku_store_queries_total 3\n",
        )];

        assert!(!delivery_source_health("delivery_metrics", &module_info, &generic_metrics).ready);
        assert!(delivery_source_health("delivery_metrics", &module_info, &waku_metrics).ready);
        assert!(
            delivery_source_capability_facts("delivery_metrics", &module_info, &waku_metrics)
                .iter()
                .any(|fact| fact.key == "store" && fact.available)
        );
    }

    #[test]
    fn delivery_rest_health_requires_node_and_connection_health() {
        let module_info = ProbeReport::ok(
            "delivery_rest.health",
            "http://delivery/health",
            json!({ "status": "ok" }),
        );
        let missing_connection = vec![
            module_info.clone(),
            ProbeReport::ok(
                "delivery_rest.nodeHealth",
                "http://delivery/health",
                "healthy",
            ),
        ];
        let connected = vec![
            module_info.clone(),
            ProbeReport::ok(
                "delivery_rest.nodeHealth",
                "http://delivery/health",
                "healthy",
            ),
            ProbeReport::ok(
                "delivery_rest.connectionStatus",
                "http://delivery/health",
                "connected",
            ),
        ];

        assert!(!delivery_source_health("delivery_rest", &module_info, &missing_connection).ready);
        assert!(delivery_source_health("delivery_rest", &module_info, &connected).ready);
    }
}
