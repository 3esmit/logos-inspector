use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};

use super::{
    layer::{self, StorageAdapter},
    transport,
};
use crate::{
    ProbeReport,
    modules::{ModuleReport, logos_core::SharedModuleTransport},
    source_routing::{
        AdapterConnectionType, SourceProbeKey, SourceReport, StorageSourceReportKind,
        shared::{
            evidence::SourceEvidence,
            http,
            report::{
                MetricsProbeSpec, SourceReportBuilder, SourceReportKind, keyed_probe_result,
                source_report_from_evidence, source_text_metrics_report, unsupported_source_report,
            },
        },
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum StorageProbeNormalizer {
    Identity,
    Space,
    Manifests,
    Spr,
    PeerId,
    Exists(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StorageProbeStep {
    key: SourceProbeKey,
    label: &'static str,
    path: String,
    normalizer: StorageProbeNormalizer,
}

impl StorageProbeStep {
    fn new(
        key: SourceProbeKey,
        label: &'static str,
        path: impl Into<String>,
        normalizer: StorageProbeNormalizer,
    ) -> Self {
        Self {
            key,
            label,
            path: path.into(),
            normalizer,
        }
    }
}

pub async fn storage_source_report(
    source_mode: &str,
    rest_endpoint: Option<&str>,
    metrics_endpoint: Option<&str>,
    cid: Option<&str>,
    privileged_debug_enabled: bool,
    runtime_diagnostics_enabled: bool,
    module_transport: &SharedModuleTransport,
) -> SourceReport {
    match StorageAdapter::select(source_mode, rest_endpoint, metrics_endpoint) {
        StorageAdapter::Module { transport } => module_source_report(
            SourceReportKind::Storage(StorageSourceReportKind::Module),
            layer::module_report(
                module_transport,
                transport,
                cid,
                privileged_debug_enabled,
                runtime_diagnostics_enabled,
            )
            .await,
        ),
        StorageAdapter::Rest {
            endpoint,
            metrics_endpoint,
        } => storage_rest_report(endpoint, metrics_endpoint, cid, privileged_debug_enabled).await,
        StorageAdapter::Metrics { endpoint } => storage_metrics_report(endpoint).await,
        StorageAdapter::Unsupported { mode } => unsupported_storage_source_report(mode),
    }
}

fn storage_rest_probe_plan(
    cid: Option<&str>,
    privileged_debug_enabled: bool,
) -> Vec<StorageProbeStep> {
    let mut steps = vec![
        StorageProbeStep::new(
            SourceProbeKey::StorageSpace,
            "storage_rest.space",
            "/space",
            StorageProbeNormalizer::Space,
        ),
        StorageProbeStep::new(
            SourceProbeKey::StorageSpr,
            "storage_rest.spr",
            "/spr",
            StorageProbeNormalizer::Spr,
        ),
        StorageProbeStep::new(
            SourceProbeKey::StoragePeerId,
            "storage_rest.peerId",
            "/peerid",
            StorageProbeNormalizer::PeerId,
        ),
        StorageProbeStep::new(
            SourceProbeKey::StorageManifests,
            "storage_rest.manifests",
            "/data",
            StorageProbeNormalizer::Manifests,
        ),
    ];
    if privileged_debug_enabled {
        steps.push(StorageProbeStep::new(
            SourceProbeKey::StorageDebug,
            "storage_rest.debug",
            "/debug/info",
            StorageProbeNormalizer::Identity,
        ));
    }
    if let Some(cid) = optional(cid) {
        steps.push(StorageProbeStep::new(
            SourceProbeKey::StorageExists,
            "storage_rest.exists",
            format!("/data/{cid}/exists"),
            StorageProbeNormalizer::Exists(cid.to_owned()),
        ));
    }
    steps
}

fn module_source_report(kind: SourceReportKind, report: ModuleReport) -> SourceReport {
    let adapter = match report.adapter {
        crate::modules::logos_core::ModuleTransportKind::Module => AdapterConnectionType::Module,
        crate::modules::logos_core::ModuleTransportKind::LogoscoreCli => {
            AdapterConnectionType::LogoscoreCli
        }
    };
    source_report_from_evidence(
        kind,
        SourceEvidence::new(report.module, report.module_info, report.probes).with_adapter(adapter),
    )
}

fn probe_step(plan: &[StorageProbeStep], key: SourceProbeKey) -> Option<&StorageProbeStep> {
    plan.iter().find(|step| step.key == key)
}

fn optional_probe_steps<'a>(
    plan: &'a [StorageProbeStep],
    handled: &[SourceProbeKey],
) -> Vec<&'a StorageProbeStep> {
    plan.iter()
        .filter(|step| !handled.contains(&step.key))
        .collect()
}

async fn http_json_probe(endpoint: &str, step: &StorageProbeStep) -> ProbeReport {
    keyed_probe_result(
        step.key,
        step.label,
        http::rest_url(endpoint, &step.path),
        transport::probe_value(endpoint, &step.path)
            .await
            .and_then(|value| normalize_http_probe_value(value, &step.normalizer)),
    )
}

fn normalize_http_probe_value(value: Value, normalizer: &StorageProbeNormalizer) -> Result<Value> {
    match normalizer {
        StorageProbeNormalizer::Identity => Ok(value),
        StorageProbeNormalizer::Space => normalize_storage_space(value),
        StorageProbeNormalizer::Manifests => normalize_storage_manifests(value),
        StorageProbeNormalizer::Spr => normalize_storage_spr(value),
        StorageProbeNormalizer::PeerId => normalize_storage_peer_id(value),
        StorageProbeNormalizer::Exists(cid) => normalize_storage_exists(value, cid),
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
        http_json_probe(endpoint, space_step),
        http_json_probe(endpoint, spr_step),
        http_json_probe(endpoint, peer_id_step),
        http_json_probe(endpoint, manifests_step),
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
        report.push_probe(http_json_probe(endpoint, step).await);
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
        transport::probe_metrics(endpoint).await,
    )
}

async fn storage_metrics_probe(metrics_endpoint: &str) -> ProbeReport {
    keyed_probe_result(
        SourceProbeKey::StorageCollectMetrics,
        "storage_rest.collectMetrics",
        metrics_endpoint,
        transport::probe_metrics(metrics_endpoint).await,
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

fn normalize_storage_space(value: Value) -> Result<Value> {
    let object = value
        .as_object()
        .context("storage REST space response must be an object")?;
    for field in [
        "totalBlocks",
        "quotaMaxBytes",
        "quotaUsedBytes",
        "quotaReservedBytes",
    ] {
        if object.get(field).and_then(Value::as_u64).is_none() {
            bail!("storage REST space response has no nonnegative integer `{field}`");
        }
    }
    Ok(value)
}

fn normalize_storage_manifests(value: Value) -> Result<Value> {
    match value {
        Value::Object(mut object) => match object.remove("content") {
            Some(Value::Array(items)) => Ok(Value::Array(items)),
            _ => bail!("storage REST manifests response must contain a `content` array"),
        },
        // Older compatible Storage endpoints returned the content list directly.
        Value::Array(items) => Ok(Value::Array(items)),
        _ => bail!("storage REST manifests response must be an object or array"),
    }
}

fn normalize_storage_spr(value: Value) -> Result<Value> {
    normalize_required_text(value, &["spr", "value", "result"], "SPR", |value| {
        value.starts_with("spr:")
    })
}

fn normalize_storage_peer_id(value: Value) -> Result<Value> {
    normalize_required_text(
        value,
        &["peerId", "peer_id", "id", "value", "result"],
        "peer ID",
        |value| !value.chars().any(char::is_whitespace),
    )
}

fn normalize_storage_exists(value: Value, cid: &str) -> Result<Value> {
    let value = scalar_field(&value, &[cid, "exists", "has", "value", "result"])
        .context("storage REST exists response has no boolean value")?;
    if !value.is_boolean() {
        bail!("storage REST exists response must contain a boolean value");
    }
    Ok(value)
}

fn normalize_required_text(
    value: Value,
    keys: &[&str],
    label: &str,
    accepts_legacy_text: impl FnOnce(&str) -> bool,
) -> Result<Value> {
    let structured = value.is_object();
    let value = scalar_field(&value, keys).unwrap_or(value);
    let value = value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("storage REST {label} response must be a non-empty string"))?;
    if !structured && !accepts_legacy_text(value) {
        bail!("storage REST {label} response must use the documented JSON shape");
    }
    Ok(Value::String(value.to_owned()))
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

    use anyhow::{Result, bail};
    use serde_json::json;

    use super::*;
    use crate::modules::logos_core::ModuleTransportKind;

    #[derive(Debug)]
    struct RecordingBasecampTransport {
        calls: Arc<AtomicUsize>,
    }

    impl crate::modules::logos_core::ModuleTransport for RecordingBasecampTransport {
        fn kind(&self) -> ModuleTransportKind {
            ModuleTransportKind::Module
        }

        fn call(
            &self,
            _call: crate::modules::logos_core::ModuleCall,
        ) -> crate::modules::logos_core::ModuleCallFuture<'_> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {
                Ok(crate::modules::logos_core::ModuleCallReply::new(
                    ModuleTransportKind::Module,
                    json!({}),
                ))
            })
        }
    }

    #[test]
    fn rest_probe_plan_includes_optional_debug_and_cid_steps() {
        let steps = storage_rest_probe_plan(Some("cid-1"), true);
        let keys = steps.iter().map(|step| step.key).collect::<Vec<_>>();

        assert!(keys.contains(&SourceProbeKey::StorageDebug));
        assert!(keys.contains(&SourceProbeKey::StorageExists));
        assert!(steps.iter().any(|step| step.path == "/data/cid-1/exists"));
    }

    #[test]
    fn rest_normalizers_unwrap_current_scalar_shapes() -> Result<()> {
        assert_eq!(
            normalize_storage_peer_id(json!({ "id": "peer-a" }))?,
            json!("peer-a")
        );
        assert_eq!(
            normalize_storage_spr(json!({ "spr": "spr-a" }))?,
            json!("spr-a")
        );
        assert_eq!(
            normalize_storage_exists(json!({ "cid-a": true }), "cid-a")?,
            json!(true)
        );
        assert_eq!(
            normalize_storage_exists(json!({ "has": false }), "cid-a")?,
            json!(false)
        );
        assert_eq!(
            normalize_storage_space(json!({
                "totalBlocks": 1,
                "quotaMaxBytes": 2,
                "quotaUsedBytes": 3,
                "quotaReservedBytes": 4,
            }))?,
            json!({
                "totalBlocks": 1,
                "quotaMaxBytes": 2,
                "quotaUsedBytes": 3,
                "quotaReservedBytes": 4,
            })
        );
        assert_eq!(
            normalize_storage_manifests(json!({ "content": [] }))?,
            json!([])
        );
        Ok(())
    }

    #[test]
    fn core_rest_normalizers_reject_plaintext_proxy_responses() {
        let proxy_response = json!("Used HTTP Method is not allowed. POST is required");
        for normalizer in [
            StorageProbeNormalizer::Space,
            StorageProbeNormalizer::Spr,
            StorageProbeNormalizer::PeerId,
            StorageProbeNormalizer::Manifests,
        ] {
            assert!(
                normalize_http_probe_value(proxy_response.clone(), &normalizer).is_err(),
                "plaintext proxy response was accepted by {normalizer:?}"
            );
        }
    }

    #[test]
    fn module_report_adapter_derives_source_facts_from_keyed_evidence() {
        let module_info = ProbeReport::ok("storage module", "module-info", json!({}))
            .with_probe_key(SourceProbeKey::StoragePeerId.as_str());
        let report = ModuleReport::new(
            ModuleTransportKind::Module,
            "storage_module",
            module_info,
            Vec::new(),
        );

        let source_report = module_source_report(
            SourceReportKind::Storage(StorageSourceReportKind::Module),
            report,
        );

        assert!(source_report.health.reachable);
        assert_eq!(source_report.adapter, Some(AdapterConnectionType::Module));
        assert!(
            source_report
                .probe_facts
                .iter()
                .any(|fact| fact.key == SourceProbeKey::StoragePeerId.as_str())
        );
    }

    #[tokio::test]
    async fn logoscore_source_never_falls_back_when_basecamp_transport_is_active() -> Result<()> {
        let calls = Arc::new(AtomicUsize::new(0));
        let module_transport: SharedModuleTransport = Arc::new(RecordingBasecampTransport {
            calls: Arc::clone(&calls),
        });

        let report = storage_source_report(
            "logoscore_cli",
            None,
            None,
            None,
            false,
            false,
            &module_transport,
        )
        .await;

        if report.adapter != Some(AdapterConnectionType::LogoscoreCli) {
            bail!("source adapter identity was lost: {report:?}");
        }
        if calls.load(Ordering::SeqCst) != 0 {
            bail!("mismatched source reached active Basecamp transport");
        }
        if report.probes.iter().any(|probe| probe.ok) {
            bail!("mismatched source unexpectedly produced a successful probe: {report:?}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn healthy_basecamp_source_uses_live_module_version_as_identity() -> Result<()> {
        let calls = Arc::new(AtomicUsize::new(0));
        let module_transport: SharedModuleTransport = Arc::new(RecordingBasecampTransport {
            calls: Arc::clone(&calls),
        });

        let report =
            storage_source_report("module", None, None, None, false, true, &module_transport).await;

        if report.adapter != Some(AdapterConnectionType::Module)
            || report.module_info.probe_key.as_deref()
                != Some(SourceProbeKey::StorageModuleVersion.as_str())
            || !report.health.reachable
            || !report.health.ready
        {
            bail!("healthy Basecamp report was degraded by metadata handling: {report:?}");
        }
        if calls.load(Ordering::SeqCst) == 0 {
            bail!("Basecamp report did not use injected transport");
        }
        Ok(())
    }

    #[tokio::test]
    async fn metadata_only_basecamp_source_skips_storage_runtime_calls() -> Result<()> {
        let calls = Arc::new(AtomicUsize::new(0));
        let module_transport: SharedModuleTransport = Arc::new(RecordingBasecampTransport {
            calls: Arc::clone(&calls),
        });

        let _report =
            storage_source_report("module", None, None, None, false, false, &module_transport)
                .await;

        if calls.load(Ordering::SeqCst) != 0 {
            bail!("metadata-only Storage source invoked a runtime method");
        }
        Ok(())
    }
}
