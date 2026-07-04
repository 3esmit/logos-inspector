use anyhow::{Context as _, Result, bail};
use serde::Serialize;
use serde_json::{Value, json};

use crate::{ProbeReport, logoscore, response_excerpt};

const BLOCKCHAIN_MODULE: &str = "blockchain_module";
const STORAGE_MODULE: &str = "storage_module";
const DELIVERY_MODULE: &str = "delivery_module";
const CAPABILITY_MODULE: &str = "capability_module";
const DEFAULT_DELIVERY_REST_ENDPOINT: &str = "http://127.0.0.1:8645";
const DEFAULT_DELIVERY_METRICS_ENDPOINT: &str = "http://127.0.0.1:8008/metrics";
const DEFAULT_STORAGE_REST_ENDPOINT: &str = "http://127.0.0.1:8080/api/storage/v1";
const DEFAULT_STORAGE_METRICS_ENDPOINT: &str = "http://127.0.0.1:8008/metrics";

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
    let _ = address;
    ModuleReport {
        module: BLOCKCHAIN_MODULE.to_owned(),
        module_info: module_info_probe(BLOCKCHAIN_MODULE),
        probes: Vec::new(),
    }
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
    ModuleReport {
        module: STORAGE_MODULE.to_owned(),
        module_info: module_info_probe(STORAGE_MODULE),
        probes,
    }
}

pub async fn storage_source_report(
    source_mode: &str,
    rest_endpoint: Option<&str>,
    metrics_endpoint: Option<&str>,
    cid: Option<&str>,
    privileged_debug_enabled: bool,
) -> ModuleReport {
    match normalized_storage_source_mode(source_mode) {
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
    ModuleReport {
        module: "storage_rest".to_owned(),
        module_info: space_probe,
        probes,
    }
}

async fn storage_metrics_report(metrics_endpoint: Option<&str>) -> ModuleReport {
    let endpoint = optional(metrics_endpoint).unwrap_or(DEFAULT_STORAGE_METRICS_ENDPOINT);
    let metrics = raw_http_text_url(endpoint).await;
    match metrics {
        Ok(text) => ModuleReport {
            module: "storage_metrics".to_owned(),
            module_info: ProbeReport::ok(
                "storage_metrics.scrape",
                endpoint,
                json!({
                    "bytes": text.len(),
                    "lines": text.lines().count(),
                }),
            ),
            probes: vec![ProbeReport::ok(
                "storage_metrics.collectMetrics",
                endpoint,
                text,
            )],
        },
        Err(error) => {
            let error = error.to_string();
            ModuleReport {
                module: "storage_metrics".to_owned(),
                module_info: ProbeReport::err("storage_metrics.scrape", endpoint, &error),
                probes: vec![ProbeReport::err(
                    "storage_metrics.collectMetrics",
                    endpoint,
                    error,
                )],
            }
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
    ModuleReport {
        module: format!("storage_{mode}"),
        module_info: ProbeReport::err(
            "storage source",
            mode,
            format!("storage source mode `{mode}` is not implemented"),
        ),
        probes: Vec::new(),
    }
}

fn normalized_storage_source_mode(source_mode: &str) -> &'static str {
    match source_mode.trim().to_ascii_lowercase().as_str() {
        "module" | "basecamp" | "basecamp-module" | "basecamp module" => "rest",
        "rest" | "standalone-rest" | "standalone rest" | "direct-rest" => "rest",
        "metrics" | "metrics-only" | "metrics only" => "metrics",
        "c-library" | "c library" | "library" => "unsupported",
        "local-os" | "local os" | "local diagnostics" => "unsupported",
        "auto" => "rest",
        _ => "unsupported",
    }
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
    ModuleReport {
        module: DELIVERY_MODULE.to_owned(),
        module_info: module_info_probe(DELIVERY_MODULE),
        probes,
    }
}

pub async fn delivery_source_report(
    source_mode: &str,
    rest_endpoint: Option<&str>,
    metrics_endpoint: Option<&str>,
) -> ModuleReport {
    match normalized_delivery_source_mode(source_mode) {
        "rest" => delivery_rest_report(rest_endpoint, metrics_endpoint).await,
        "metrics" => delivery_metrics_report(metrics_endpoint).await,
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
    ModuleReport {
        module: "delivery_rest".to_owned(),
        module_info: health_probe,
        probes,
    }
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
        Ok(text) => ModuleReport {
            module: "delivery_metrics".to_owned(),
            module_info: ProbeReport::ok(
                "delivery_metrics.scrape",
                endpoint,
                json!({
                    "bytes": text.len(),
                    "lines": text.lines().count(),
                }),
            ),
            probes: vec![ProbeReport::ok(
                "delivery_metrics.collectOpenMetricsText",
                endpoint,
                text,
            )],
        },
        Err(error) => {
            let error = error.to_string();
            ModuleReport {
                module: "delivery_metrics".to_owned(),
                module_info: ProbeReport::err("delivery_metrics.scrape", endpoint, &error),
                probes: vec![ProbeReport::err(
                    "delivery_metrics.collectOpenMetricsText",
                    endpoint,
                    error,
                )],
            }
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

fn unsupported_delivery_source_report(mode: &str) -> ModuleReport {
    ModuleReport {
        module: format!("delivery_{mode}"),
        module_info: ProbeReport::err(
            "delivery source",
            mode,
            format!("delivery source mode `{mode}` is not implemented"),
        ),
        probes: Vec::new(),
    }
}

fn normalized_delivery_source_mode(source_mode: &str) -> &'static str {
    match source_mode.trim().to_ascii_lowercase().as_str() {
        "module" | "basecamp" | "basecamp-module" | "basecamp module" => "rest",
        "rest" | "direct-rest" | "direct waku rest" | "waku-rest" => "rest",
        "metrics" | "metrics-only" | "metrics only" => "metrics",
        "network-monitor" | "network monitor" => "network-monitor",
        "discovery-crawler" | "discovery crawler" | "crawler" => "discovery-crawler",
        "auto" => "rest",
        _ => "unsupported",
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
    ModuleReport {
        module: CAPABILITY_MODULE.to_owned(),
        module_info: module_info_probe(CAPABILITY_MODULE),
        probes: Vec::new(),
    }
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
