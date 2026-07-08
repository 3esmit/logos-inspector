use serde::Serialize;
use serde_json::Value;

use crate::ProbeReport;
use crate::source_routing::policy::{
    DeliverySourceReportKind, SourceCapabilityKey, SourceProbeKey, StorageSourceReportKind,
};

use super::report::SourceReportKind;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceFacts {
    pub health: SourceHealthFacts,
    pub probe_facts: Vec<SourceProbeFact>,
    pub capability_facts: Vec<SourceCapabilityFact>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceProbeFact {
    pub key: String,
    pub label: String,
    pub source: String,
    pub ok: bool,
    pub evidence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
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
    pub(crate) fn available(
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

    pub(crate) fn unavailable(
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

#[must_use]
pub(crate) fn storage_source_facts(
    kind: StorageSourceReportKind,
    module_info: &ProbeReport,
    probes: &[ProbeReport],
) -> SourceFacts {
    let probe_facts = source_probe_facts(module_info, probes);
    SourceFacts {
        health: storage_source_health(kind, module_info, probes, &probe_facts),
        capability_facts: storage_source_capability_facts(kind, &probe_facts),
        probe_facts,
    }
}

#[must_use]
pub(crate) fn delivery_source_facts(
    kind: DeliverySourceReportKind,
    module_info: &ProbeReport,
    probes: &[ProbeReport],
) -> SourceFacts {
    let probe_facts = source_probe_facts(module_info, probes);
    SourceFacts {
        health: delivery_source_health(kind, module_info, probes, &probe_facts),
        capability_facts: delivery_source_capability_facts(kind, &probe_facts),
        probe_facts,
    }
}

fn source_probe_facts(module_info: &ProbeReport, probes: &[ProbeReport]) -> Vec<SourceProbeFact> {
    let mut facts = Vec::new();
    push_source_probe_fact(&mut facts, module_info);
    for probe in probes {
        push_source_probe_fact(&mut facts, probe);
    }
    facts
}

fn push_source_probe_fact(facts: &mut Vec<SourceProbeFact>, probe: &ProbeReport) {
    let Some(key) = probe
        .probe_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty())
    else {
        return;
    };
    let fact = SourceProbeFact {
        key: key.to_owned(),
        label: probe.label.clone(),
        source: probe.source.clone(),
        ok: probe.ok,
        evidence: if probe.ok {
            probe
                .value
                .as_ref()
                .map(value_summary)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "observed".to_owned())
        } else {
            probe
                .error
                .clone()
                .filter(|error| !error.is_empty())
                .unwrap_or_else(|| "unavailable".to_owned())
        },
        value: probe.value.clone(),
        error: probe.error.clone(),
    };
    if let Some(existing) = facts.iter_mut().find(|existing| existing.key == fact.key) {
        if !existing.ok && fact.ok {
            *existing = fact;
        }
    } else {
        facts.push(fact);
    }
}

fn source_probe_fact(facts: &[SourceProbeFact], key: SourceProbeKey) -> Option<&SourceProbeFact> {
    let key = key.as_str();
    facts.iter().find(|fact| fact.key == key)
}

fn source_probe_ok(facts: &[SourceProbeFact], key: SourceProbeKey) -> bool {
    source_probe_fact(facts, key)
        .map(|fact| fact.ok)
        .unwrap_or(false)
}

fn source_probe_value(facts: &[SourceProbeFact], key: SourceProbeKey) -> Option<&Value> {
    source_probe_fact(facts, key)
        .filter(|fact| fact.ok)
        .and_then(|fact| fact.value.as_ref())
}

fn storage_source_health(
    kind: StorageSourceReportKind,
    module_info: &ProbeReport,
    probes: &[ProbeReport],
    facts: &[SourceProbeFact],
) -> SourceHealthFacts {
    if kind == StorageSourceReportKind::Unsupported {
        return unsupported_source_health(module_info);
    }
    let reachable = source_reachable(module_info, probes);
    let ready = match kind {
        StorageSourceReportKind::Rest => {
            source_probe_ok(facts, SourceProbeKey::StoragePeerId)
                && source_probe_ok(facts, SourceProbeKey::StorageSpr)
                && source_probe_ok(facts, SourceProbeKey::StorageSpace)
                && source_probe_ok(facts, SourceProbeKey::StorageManifests)
        }
        StorageSourceReportKind::Metrics => storage_metrics_evidence_present(facts),
        StorageSourceReportKind::Module => [
            SourceProbeKey::StoragePeerId,
            SourceProbeKey::StorageSpr,
            SourceProbeKey::StorageSpace,
            SourceProbeKey::StorageDebug,
            SourceProbeKey::StorageManifests,
        ]
        .iter()
        .any(|key| source_probe_ok(facts, *key)),
        StorageSourceReportKind::Unsupported => false,
    };
    source_health(
        reachable,
        ready,
        false,
        if ready {
            source_ready_summary("storage source ready", facts)
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
    kind: DeliverySourceReportKind,
    module_info: &ProbeReport,
    probes: &[ProbeReport],
    facts: &[SourceProbeFact],
) -> SourceHealthFacts {
    if kind == DeliverySourceReportKind::Unsupported {
        return unsupported_source_health(module_info);
    }
    let reachable = source_reachable(module_info, probes);
    let metrics_ready = delivery_metrics_evidence_present(facts);
    let ready = match kind {
        DeliverySourceReportKind::Metrics => metrics_ready,
        DeliverySourceReportKind::NetworkMonitor => {
            source_probe_ok(facts, SourceProbeKey::DeliveryAllPeersInfo)
                || source_probe_ok(facts, SourceProbeKey::DeliveryContentTopics)
                || metrics_ready
        }
        DeliverySourceReportKind::Rest => {
            source_probe_ok(facts, SourceProbeKey::DeliveryHealth)
                && health_value_ok(
                    source_probe_value(facts, SourceProbeKey::DeliveryNodeHealth),
                    false,
                )
                && health_value_ok(
                    source_probe_value(facts, SourceProbeKey::DeliveryConnectionStatus),
                    false,
                )
        }
        DeliverySourceReportKind::Module => {
            let node_health = source_probe_fact(facts, SourceProbeKey::DeliveryNodeHealth);
            let connection_status =
                source_probe_fact(facts, SourceProbeKey::DeliveryConnectionStatus);
            if node_health.is_none() && connection_status.is_none() {
                delivery_module_runtime_healthy(facts)
            } else {
                health_value_ok(
                    source_probe_value(facts, SourceProbeKey::DeliveryNodeHealth),
                    false,
                ) && health_value_ok(
                    source_probe_value(facts, SourceProbeKey::DeliveryConnectionStatus),
                    false,
                )
            }
        }
        DeliverySourceReportKind::Unsupported => false,
    };
    source_health(
        reachable,
        ready,
        false,
        if ready {
            source_ready_summary("delivery source ready", facts)
        } else if reachable {
            "delivery source degraded".to_owned()
        } else {
            "delivery source unavailable".to_owned()
        },
        if ready {
            delivery_ready_detail(kind, facts)
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
    kind: StorageSourceReportKind,
    probe_facts: &[SourceProbeFact],
) -> Vec<SourceCapabilityFact> {
    let mut facts = vec![
        any_probe_fact(
            SourceCapabilityKey::Identity,
            probe_facts,
            &[SourceProbeKey::StoragePeerId, SourceProbeKey::StorageSpr],
        ),
        any_probe_fact(
            SourceCapabilityKey::Space,
            probe_facts,
            &[SourceProbeKey::StorageSpace],
        ),
        any_probe_fact(
            SourceCapabilityKey::ManifestListing,
            probe_facts,
            &[SourceProbeKey::StorageManifests],
        ),
        any_probe_fact(
            SourceCapabilityKey::Debug,
            probe_facts,
            &[SourceProbeKey::StorageDebug],
        ),
        storage_metrics_fact(probe_facts),
    ];
    if let Some(probe) = source_probe_fact(probe_facts, SourceProbeKey::StorageExists) {
        facts.push(probe_fact(SourceCapabilityKey::CidExists, probe));
    }
    if kind == StorageSourceReportKind::Rest {
        facts.push(capability_available(
            SourceCapabilityKey::RestApi,
            "REST probes available",
            None,
        ));
    } else if kind == StorageSourceReportKind::Module {
        facts.push(capability_available(
            SourceCapabilityKey::ModuleApi,
            "LogosCore storage module available",
            None,
        ));
    }
    facts
}

fn delivery_source_capability_facts(
    kind: DeliverySourceReportKind,
    probe_facts: &[SourceProbeFact],
) -> Vec<SourceCapabilityFact> {
    let mut facts = vec![
        any_probe_fact(
            SourceCapabilityKey::Identity,
            probe_facts,
            &[
                SourceProbeKey::DeliveryPeerId,
                SourceProbeKey::DeliveryMyPeerId,
                SourceProbeKey::DeliveryEnrUri,
                SourceProbeKey::DeliveryMyEnr,
                SourceProbeKey::DeliveryListenAddresses,
                SourceProbeKey::DeliveryMyMultiaddresses,
            ],
        ),
        any_probe_fact(
            SourceCapabilityKey::Health,
            probe_facts,
            &[
                SourceProbeKey::DeliveryHealth,
                SourceProbeKey::DeliveryNodeHealth,
                SourceProbeKey::DeliveryConnectionStatus,
            ],
        ),
        delivery_metrics_fact(probe_facts),
        delivery_protocol_fact(
            SourceCapabilityKey::Relay,
            &[
                "waku_relay",
                "waku_pubsub",
                "libp2p_pubsub_peers",
                "waku_node_messages_total",
            ],
            &["relay"],
            probe_facts,
        ),
        delivery_protocol_fact(
            SourceCapabilityKey::Store,
            &[
                "waku_store",
                "waku_store_peers",
                "waku_store_messages",
                "waku_store_queries_total",
            ],
            &["store"],
            probe_facts,
        ),
        delivery_protocol_fact(
            SourceCapabilityKey::Filter,
            &["waku_filter", "waku_filter_peers", "waku_filter_requests"],
            &["filter"],
            probe_facts,
        ),
        delivery_protocol_fact(
            SourceCapabilityKey::Lightpush,
            &["waku_lightpush", "waku_lightpush_peers", "lightpush"],
            &["lightpush"],
            probe_facts,
        ),
        any_probe_fact(
            SourceCapabilityKey::NetworkMonitor,
            probe_facts,
            &[
                SourceProbeKey::DeliveryAllPeersInfo,
                SourceProbeKey::DeliveryContentTopics,
            ],
        ),
    ];
    if kind == DeliverySourceReportKind::Rest {
        facts.push(capability_available(
            SourceCapabilityKey::RestApi,
            "REST probes available",
            None,
        ));
    } else if kind == DeliverySourceReportKind::Module {
        facts.push(capability_available(
            SourceCapabilityKey::ModuleApi,
            "LogosCore delivery module available",
            None,
        ));
    }
    facts
}

fn source_ready_summary(fallback: &str, facts: &[SourceProbeFact]) -> String {
    [
        SourceProbeKey::StorageVersion,
        SourceProbeKey::StorageModuleVersion,
        SourceProbeKey::DeliveryVersion,
        SourceProbeKey::DeliveryNodeInfoVersion,
    ]
    .iter()
    .find_map(|key| source_probe_value(facts, *key))
    .map(value_summary)
    .filter(|value| !value.is_empty() && value != "n/a")
    .map(|value| format!("version {value}"))
    .unwrap_or_else(|| fallback.to_owned())
}

fn delivery_ready_detail(kind: DeliverySourceReportKind, facts: &[SourceProbeFact]) -> String {
    if kind == DeliverySourceReportKind::Rest {
        let node = source_probe_value(facts, SourceProbeKey::DeliveryNodeHealth)
            .map(value_summary)
            .unwrap_or_else(|| "unknown".to_owned());
        let connection = source_probe_value(facts, SourceProbeKey::DeliveryConnectionStatus)
            .map(value_summary)
            .unwrap_or_else(|| "unknown".to_owned());
        return format!("node health {node}; connection {connection}");
    }
    if delivery_metrics_evidence_present(facts) {
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
    key: SourceCapabilityKey,
    facts: &[SourceProbeFact],
    probe_keys: &[SourceProbeKey],
) -> SourceCapabilityFact {
    let mut fallback = None;
    for probe_key in probe_keys {
        if let Some(probe) = source_probe_fact(facts, *probe_key) {
            if probe.ok {
                return probe_fact(key, probe);
            }
            if fallback.is_none() {
                fallback = Some(probe);
            }
        }
    }
    fallback.map_or_else(
        || capability_unavailable(key, "not observed"),
        |probe| probe_fact(key, probe),
    )
}

fn probe_fact(key: SourceCapabilityKey, probe: &SourceProbeFact) -> SourceCapabilityFact {
    if probe.ok {
        capability_available(key, probe.evidence.clone(), probe.value.clone())
    } else {
        capability_unavailable(
            key,
            probe
                .error
                .clone()
                .filter(|error| !error.is_empty())
                .unwrap_or_else(|| "unavailable".to_owned()),
        )
    }
}

fn capability_available(
    key: SourceCapabilityKey,
    evidence: impl Into<String>,
    value: Option<Value>,
) -> SourceCapabilityFact {
    SourceCapabilityFact::available(key.as_str(), key.label(), evidence, value)
}

fn capability_unavailable(
    key: SourceCapabilityKey,
    evidence: impl Into<String>,
) -> SourceCapabilityFact {
    SourceCapabilityFact::unavailable(key.as_str(), key.label(), evidence)
}

fn storage_metrics_fact(facts: &[SourceProbeFact]) -> SourceCapabilityFact {
    let probe = source_probe_fact(facts, SourceProbeKey::StorageCollectMetrics);
    if storage_metrics_evidence_present(facts) {
        return capability_available(
            SourceCapabilityKey::Metrics,
            "OpenMetrics text observed",
            probe.and_then(|probe| probe.value.clone()),
        );
    }
    probe.map_or_else(
        || capability_unavailable(SourceCapabilityKey::Metrics, "not observed"),
        |probe| {
            if probe.ok {
                capability_unavailable(SourceCapabilityKey::Metrics, "metrics response empty")
            } else {
                probe_fact(SourceCapabilityKey::Metrics, probe)
            }
        },
    )
}

fn delivery_metrics_fact(facts: &[SourceProbeFact]) -> SourceCapabilityFact {
    let probe = source_probe_fact(facts, SourceProbeKey::DeliveryCollectOpenMetricsText)
        .or_else(|| source_probe_fact(facts, SourceProbeKey::DeliveryNodeInfoMetrics));
    if delivery_metrics_evidence_present(facts) {
        return capability_available(
            SourceCapabilityKey::Metrics,
            "known Waku metric family observed",
            probe.and_then(|probe| probe.value.clone()),
        );
    }
    probe.map_or_else(
        || capability_unavailable(SourceCapabilityKey::Metrics, "not observed"),
        |probe| {
            if probe.ok {
                capability_unavailable(
                    SourceCapabilityKey::Metrics,
                    "no known Waku metric family observed",
                )
            } else {
                probe_fact(SourceCapabilityKey::Metrics, probe)
            }
        },
    )
}

fn delivery_protocol_fact(
    key: SourceCapabilityKey,
    metric_needles: &[&str],
    protocol_needles: &[&str],
    facts: &[SourceProbeFact],
) -> SourceCapabilityFact {
    if metric_text_contains(facts, metric_needles) {
        return capability_available(key, "metric family observed", None);
    }
    if protocol_health_contains(facts, protocol_needles) {
        return capability_available(key, "protocol health observed", None);
    }
    capability_unavailable(key, "not observed")
}

fn storage_metrics_evidence_present(facts: &[SourceProbeFact]) -> bool {
    open_metrics_text(facts, &[SourceProbeKey::StorageCollectMetrics])
        .map(|text| text.lines().any(|line| !line.trim().is_empty()))
        .unwrap_or(false)
}

fn delivery_metrics_evidence_present(facts: &[SourceProbeFact]) -> bool {
    metric_text_contains(
        facts,
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

fn metric_text_contains(facts: &[SourceProbeFact], needles: &[&str]) -> bool {
    open_metrics_text(
        facts,
        &[
            SourceProbeKey::DeliveryCollectOpenMetricsText,
            SourceProbeKey::StorageCollectMetrics,
            SourceProbeKey::DeliveryNodeInfoMetrics,
        ],
    )
    .map(|text| needles.iter().any(|needle| text.contains(needle)))
    .unwrap_or(false)
}

fn protocol_health_contains(facts: &[SourceProbeFact], needles: &[&str]) -> bool {
    source_probe_value(facts, SourceProbeKey::DeliveryProtocolsHealth)
        .map(|value| value.to_string().to_lowercase())
        .map(|text| needles.iter().any(|needle| text.contains(needle)))
        .unwrap_or(false)
}

fn delivery_module_runtime_healthy(facts: &[SourceProbeFact]) -> bool {
    [
        SourceProbeKey::DeliveryNodeInfoMetrics,
        SourceProbeKey::DeliveryCollectOpenMetricsText,
    ]
    .iter()
    .any(|key| probe_has_runtime_value(source_probe_fact(facts, *key)))
}

fn probe_has_runtime_value(probe: Option<&SourceProbeFact>) -> bool {
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

fn open_metrics_text(facts: &[SourceProbeFact], keys: &[SourceProbeKey]) -> Option<String> {
    for key in keys {
        let Some(value) = source_probe_value(facts, *key) else {
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

#[must_use]
pub(crate) fn source_facts_for_report(
    kind: SourceReportKind,
    module_info: &ProbeReport,
    probes: &[ProbeReport],
) -> SourceFacts {
    match kind {
        SourceReportKind::Delivery(kind) => delivery_source_facts(kind, module_info, probes),
        SourceReportKind::Storage(kind) => storage_source_facts(kind, module_info, probes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn probe_ok(key: SourceProbeKey, value: impl serde::Serialize) -> ProbeReport {
        ProbeReport::ok("renamed probe", "opaque source", value).with_probe_key(key.as_str())
    }

    fn probe_err(key: SourceProbeKey, error: &str) -> ProbeReport {
        ProbeReport::err("renamed probe", "opaque source", error).with_probe_key(key.as_str())
    }

    #[test]
    fn storage_source_facts_require_rest_core_facts() {
        let module_info = probe_ok(SourceProbeKey::StorageSpace, json!({}));
        let probes = vec![
            module_info.clone(),
            probe_ok(SourceProbeKey::StorageSpr, "spr-a"),
            probe_ok(SourceProbeKey::StoragePeerId, "peer-a"),
            probe_ok(SourceProbeKey::StorageManifests, json!([])),
        ];

        let facts = storage_source_facts(StorageSourceReportKind::Rest, &module_info, &probes);

        assert!(facts.health.reachable);
        assert!(facts.health.ready);
        assert_eq!(facts.health.status, SourceHealthStatus::Healthy);
        assert!(
            facts
                .probe_facts
                .iter()
                .any(|fact| fact.key == SourceProbeKey::StoragePeerId.as_str() && fact.ok)
        );
        assert!(
            facts
                .capability_facts
                .iter()
                .any(|fact| fact.key == "identity" && fact.available)
        );
    }

    #[test]
    fn storage_source_facts_mark_rest_missing_facts_degraded() {
        let module_info = probe_ok(SourceProbeKey::StorageSpace, json!({}));
        let probes = vec![module_info.clone()];

        let facts = storage_source_facts(StorageSourceReportKind::Rest, &module_info, &probes);

        assert!(facts.health.reachable);
        assert!(!facts.health.ready);
        assert_eq!(facts.health.status, SourceHealthStatus::Degraded);
    }

    #[test]
    fn delivery_source_facts_require_known_metrics_family() {
        let module_info = probe_ok(
            SourceProbeKey::DeliveryMetricsScrape,
            json!({ "bytes": 20, "lines": 1 }),
        );
        let generic_metrics = vec![probe_ok(
            SourceProbeKey::DeliveryCollectOpenMetricsText,
            "process_cpu_seconds_total 3\n",
        )];
        let waku_metrics = vec![probe_ok(
            SourceProbeKey::DeliveryCollectOpenMetricsText,
            "waku_store_queries_total 3\n",
        )];

        assert!(
            !delivery_source_facts(
                DeliverySourceReportKind::Metrics,
                &module_info,
                &generic_metrics
            )
            .health
            .ready
        );
        let facts = delivery_source_facts(
            DeliverySourceReportKind::Metrics,
            &module_info,
            &waku_metrics,
        );
        assert!(facts.health.ready);
        assert!(
            facts
                .capability_facts
                .iter()
                .any(|fact| fact.key == "store" && fact.available)
        );
    }

    #[test]
    fn delivery_rest_source_facts_require_node_and_connection_health() {
        let module_info = probe_ok(SourceProbeKey::DeliveryHealth, json!({ "status": "ok" }));
        let missing_connection = vec![
            module_info.clone(),
            probe_ok(SourceProbeKey::DeliveryNodeHealth, "healthy"),
        ];
        let connected = vec![
            module_info.clone(),
            probe_ok(SourceProbeKey::DeliveryNodeHealth, "healthy"),
            probe_ok(SourceProbeKey::DeliveryConnectionStatus, "connected"),
        ];

        assert!(
            !delivery_source_facts(
                DeliverySourceReportKind::Rest,
                &module_info,
                &missing_connection
            )
            .health
            .ready
        );
        assert!(
            delivery_source_facts(DeliverySourceReportKind::Rest, &module_info, &connected)
                .health
                .ready
        );
    }

    #[test]
    fn unsupported_source_facts_keep_unsupported_status() {
        let module_info = ProbeReport::err(
            "storage source",
            "unsupported",
            "storage source mode `unsupported` is not implemented",
        );

        let facts = storage_source_facts(StorageSourceReportKind::Unsupported, &module_info, &[]);

        assert_eq!(facts.health.status, SourceHealthStatus::Unsupported);
        assert!(!facts.health.reachable);
        assert!(!facts.health.ready);
    }

    #[test]
    fn source_probe_facts_preserve_failed_keyed_probe_without_label_matching() {
        let module_info = probe_ok(SourceProbeKey::StorageSpace, json!({}));
        let probes = vec![probe_err(SourceProbeKey::StoragePeerId, "peer unavailable")];

        let facts = storage_source_facts(StorageSourceReportKind::Module, &module_info, &probes);
        let peer = facts
            .probe_facts
            .iter()
            .find(|fact| fact.key == SourceProbeKey::StoragePeerId.as_str());

        assert_eq!(peer.map(|fact| fact.ok), Some(false));
        assert_eq!(
            peer.and_then(|fact| fact.error.as_deref()),
            Some("peer unavailable")
        );
        assert!(facts.health.ready);
    }
}
