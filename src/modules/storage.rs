use serde_json::{Value, json};

use crate::{
    ProbeReport,
    source_routing::{
        DEFAULT_STORAGE_METRICS_ENDPOINT, DEFAULT_STORAGE_REST_ENDPOINT, SourceFamily,
        SourceProbeKey, StorageSourceReportKind, effective_source_mode, storage_source_facts,
    },
};

use super::base::{
    ModuleReport, STORAGE_MODULE, call_source_probe, http_url, module_info_probe, optional,
    raw_http_text_url, raw_http_value, scalar_field,
};

pub fn storage_report(cid: Option<&str>, privileged_debug_enabled: bool) -> ModuleReport {
    let mut probes = vec![
        call_source_probe(
            STORAGE_MODULE,
            "version",
            &[],
            SourceProbeKey::StorageVersion,
        ),
        call_source_probe(
            STORAGE_MODULE,
            "moduleVersion",
            &[],
            SourceProbeKey::StorageModuleVersion,
        ),
        call_source_probe(
            STORAGE_MODULE,
            "dataDir",
            &[],
            SourceProbeKey::StorageDataDir,
        ),
        call_source_probe(STORAGE_MODULE, "peerId", &[], SourceProbeKey::StoragePeerId),
        call_source_probe(STORAGE_MODULE, "spr", &[], SourceProbeKey::StorageSpr),
        call_source_probe(STORAGE_MODULE, "space", &[], SourceProbeKey::StorageSpace),
        call_source_probe(
            STORAGE_MODULE,
            "manifests",
            &[],
            SourceProbeKey::StorageManifests,
        ),
        call_source_probe(
            STORAGE_MODULE,
            "collectMetrics",
            &[],
            SourceProbeKey::StorageCollectMetrics,
        ),
    ];
    if privileged_debug_enabled {
        probes.push(call_source_probe(
            STORAGE_MODULE,
            "debug",
            &[],
            SourceProbeKey::StorageDebug,
        ));
    }
    if let Some(cid) = optional(cid) {
        probes.push(call_source_probe(
            STORAGE_MODULE,
            "exists",
            &[cid],
            SourceProbeKey::StorageExists,
        ));
    }
    let module_info = module_info_probe(STORAGE_MODULE);
    ModuleReport::new(STORAGE_MODULE, module_info.clone(), probes.clone()).with_source_facts(
        storage_source_facts(StorageSourceReportKind::Module, &module_info, &probes),
    )
}

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
    let space_probe = ProbeReport::from_result("storage_rest.space", space_source.clone(), space)
        .with_probe_key(SourceProbeKey::StorageSpace.as_str());
    let mut probes = vec![
        space_probe.clone(),
        ProbeReport::from_result("storage_rest.spr", spr_source, spr)
            .with_probe_key(SourceProbeKey::StorageSpr.as_str()),
        ProbeReport::from_result("storage_rest.peerId", peer_id_source, peer_id)
            .with_probe_key(SourceProbeKey::StoragePeerId.as_str()),
        ProbeReport::from_result("storage_rest.manifests", data_source, manifests)
            .with_probe_key(SourceProbeKey::StorageManifests.as_str()),
    ];
    if privileged_debug_enabled {
        let debug_source = http_url(endpoint, "/debug/info");
        probes.push(
            ProbeReport::from_result(
                "storage_rest.debug",
                debug_source,
                raw_http_value(endpoint, "/debug/info").await,
            )
            .with_probe_key(SourceProbeKey::StorageDebug.as_str()),
        );
    } else {
        probes.push(
            ProbeReport::ok(
                "storage_rest.privilegedProbe",
                "disabled",
                json!({ "skipped": true }),
            )
            .with_probe_key(SourceProbeKey::StoragePrivilegedProbe.as_str()),
        );
    }
    if let Some(cid) = optional(cid) {
        let path = format!("/data/{cid}/exists");
        probes.push(
            ProbeReport::from_result(
                "storage_rest.exists",
                http_url(endpoint, &path),
                raw_http_value(endpoint, &path)
                    .await
                    .map(|value| normalize_storage_exists(value, cid)),
            )
            .with_probe_key(SourceProbeKey::StorageExists.as_str()),
        );
    }
    if let Some(metrics_endpoint) = optional(metrics_endpoint) {
        probes.push(storage_metrics_probe(metrics_endpoint).await);
    }
    ModuleReport::new("storage_rest", space_probe.clone(), probes.clone()).with_source_facts(
        storage_source_facts(StorageSourceReportKind::Rest, &space_probe, &probes),
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
            )
            .with_probe_key(SourceProbeKey::StorageMetricsScrape.as_str());
            let probes = vec![
                ProbeReport::ok("storage_metrics.collectMetrics", endpoint, text)
                    .with_probe_key(SourceProbeKey::StorageCollectMetrics.as_str()),
            ];
            ModuleReport::new("storage_metrics", module_info.clone(), probes.clone())
                .with_source_facts(storage_source_facts(
                    StorageSourceReportKind::Metrics,
                    &module_info,
                    &probes,
                ))
        }
        Err(error) => {
            let error = error.to_string();
            let module_info = ProbeReport::err("storage_metrics.scrape", endpoint, &error)
                .with_probe_key(SourceProbeKey::StorageMetricsScrape.as_str());
            let probes = vec![
                ProbeReport::err("storage_metrics.collectMetrics", endpoint, error)
                    .with_probe_key(SourceProbeKey::StorageCollectMetrics.as_str()),
            ];
            ModuleReport::new("storage_metrics", module_info.clone(), probes.clone())
                .with_source_facts(storage_source_facts(
                    StorageSourceReportKind::Metrics,
                    &module_info,
                    &probes,
                ))
        }
    }
}

async fn storage_metrics_probe(metrics_endpoint: &str) -> ProbeReport {
    ProbeReport::from_result(
        "storage_rest.collectMetrics",
        metrics_endpoint,
        raw_http_text_url(metrics_endpoint).await,
    )
    .with_probe_key(SourceProbeKey::StorageCollectMetrics.as_str())
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
        storage_source_facts(StorageSourceReportKind::Unsupported, &module_info, &probes),
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
}
