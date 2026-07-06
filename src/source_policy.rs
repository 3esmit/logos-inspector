use serde::Serialize;
use serde_json::Value;

use crate::{ProbeReport, network::NetworkProfile};

pub const TESTNET_SEQUENCER_ENDPOINT: &str = "https://testnet.lez.logos.co/";
pub const LOCAL_SEQUENCER_ENDPOINT: &str = "http://127.0.0.1:3040/";
pub const DEFAULT_SEQUENCER_ENDPOINT: &str = TESTNET_SEQUENCER_ENDPOINT;
pub const DEFAULT_INDEXER_ENDPOINT: &str = "http://127.0.0.1:8779/";
pub const DEFAULT_NODE_ENDPOINT: &str = "http://127.0.0.1:8080/";
pub const DEFAULT_DELIVERY_REST_ENDPOINT: &str = "http://127.0.0.1:8645";
pub const DEFAULT_DELIVERY_METRICS_ENDPOINT: &str = "http://127.0.0.1:8008/metrics";
pub const DEFAULT_STORAGE_REST_ENDPOINT: &str = "http://127.0.0.1:8080/api/storage/v1";
pub const DEFAULT_STORAGE_METRICS_ENDPOINT: &str = "http://127.0.0.1:8008/metrics";

const STORAGE_MODULE: &str = "storage_module";
const DELIVERY_MODULE: &str = "delivery_module";

const RPC_ADAPTER: SourceAdapterPolicy = SourceAdapterPolicy {
    target: "rpc_endpoint",
    uses_rest_endpoint: false,
    uses_metrics_endpoint: false,
    supports_cid_probe: false,
    supports_mutating_diagnostics: false,
};
const MODULE_ADAPTER: SourceAdapterPolicy = SourceAdapterPolicy {
    target: "module",
    uses_rest_endpoint: false,
    uses_metrics_endpoint: false,
    supports_cid_probe: false,
    supports_mutating_diagnostics: true,
};
const NONE_ADAPTER: SourceAdapterPolicy = SourceAdapterPolicy {
    target: "none",
    uses_rest_endpoint: false,
    uses_metrics_endpoint: false,
    supports_cid_probe: false,
    supports_mutating_diagnostics: false,
};
const DELIVERY_REST_ADAPTER: SourceAdapterPolicy = SourceAdapterPolicy {
    target: "rest_endpoint",
    uses_rest_endpoint: true,
    uses_metrics_endpoint: true,
    supports_cid_probe: false,
    supports_mutating_diagnostics: true,
};
const DELIVERY_METRICS_ADAPTER: SourceAdapterPolicy = SourceAdapterPolicy {
    target: "metrics_endpoint",
    uses_rest_endpoint: false,
    uses_metrics_endpoint: true,
    supports_cid_probe: false,
    supports_mutating_diagnostics: false,
};
const DELIVERY_MONITOR_ADAPTER: SourceAdapterPolicy = SourceAdapterPolicy {
    target: "rest_endpoint",
    uses_rest_endpoint: true,
    uses_metrics_endpoint: true,
    supports_cid_probe: false,
    supports_mutating_diagnostics: false,
};
const STORAGE_MODULE_ADAPTER: SourceAdapterPolicy = SourceAdapterPolicy {
    target: "module",
    uses_rest_endpoint: false,
    uses_metrics_endpoint: false,
    supports_cid_probe: true,
    supports_mutating_diagnostics: false,
};
const STORAGE_REST_ADAPTER: SourceAdapterPolicy = SourceAdapterPolicy {
    target: "rest_endpoint",
    uses_rest_endpoint: true,
    uses_metrics_endpoint: true,
    supports_cid_probe: true,
    supports_mutating_diagnostics: true,
};
const STORAGE_METRICS_ADAPTER: SourceAdapterPolicy = SourceAdapterPolicy {
    target: "metrics_endpoint",
    uses_rest_endpoint: false,
    uses_metrics_endpoint: true,
    supports_cid_probe: false,
    supports_mutating_diagnostics: false,
};
const FALLBACK_CORE_SOURCE_MODE: SourceModePolicy = SourceModePolicy {
    key: "auto",
    aliases: &["auto"],
    effective: "rpc",
    label_key: "auto_rpc",
    label: "Auto",
    source_label: "Auto: Direct RPC",
    summary: "Use configured direct RPC endpoint",
    implemented: true,
    adapter: RPC_ADAPTER,
};
const FALLBACK_UNSUPPORTED_SOURCE_MODE: SourceModePolicy = SourceModePolicy {
    key: "unsupported",
    aliases: &["unsupported"],
    effective: "unsupported",
    label_key: "unsupported",
    label: "Unsupported saved source",
    source_label: "Unsupported source",
    summary: "Select a supported source to replace this saved value",
    implemented: false,
    adapter: NONE_ADAPTER,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreSourceMode {
    Auto,
    Rpc,
    Module,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreEndpointMode {
    Rpc,
    Module,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliverySourceMode {
    Auto,
    Module,
    Rest,
    Metrics,
    NetworkMonitor,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageSourceMode {
    Auto,
    Module,
    Rest,
    Metrics,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFamily {
    Core,
    Delivery,
    Storage,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourcePolicyReport {
    pub version: u8,
    pub defaults: SourcePolicyDefaults,
    pub network_profiles: Vec<NetworkProfile>,
    pub source_modes: SourceModeFamilies,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourcePolicyDefaults {
    pub sequencer_endpoint: &'static str,
    pub local_sequencer_endpoint: &'static str,
    pub indexer_endpoint: &'static str,
    pub node_endpoint: &'static str,
    pub delivery_rest_endpoint: &'static str,
    pub delivery_metrics_endpoint: &'static str,
    pub storage_rest_endpoint: &'static str,
    pub storage_metrics_endpoint: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceModeFamilies {
    pub core: &'static [SourceModePolicy],
    pub delivery: &'static [SourceModePolicy],
    pub storage: &'static [SourceModePolicy],
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceModePolicy {
    pub key: &'static str,
    pub aliases: &'static [&'static str],
    pub effective: &'static str,
    pub label_key: &'static str,
    pub label: &'static str,
    pub source_label: &'static str,
    pub summary: &'static str,
    pub implemented: bool,
    pub adapter: SourceAdapterPolicy,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SourceAdapterPolicy {
    pub target: &'static str,
    pub uses_rest_endpoint: bool,
    pub uses_metrics_endpoint: bool,
    pub supports_cid_probe: bool,
    pub supports_mutating_diagnostics: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceFacts {
    pub health: SourceHealthFacts,
    pub capability_facts: Vec<SourceCapabilityFact>,
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

const CORE_SOURCE_MODES: &[SourceModePolicy] = &[
    SourceModePolicy {
        key: "auto",
        aliases: &["auto"],
        effective: "rpc",
        label_key: "auto_rpc",
        label: "Auto",
        source_label: "Auto: Direct RPC",
        summary: "Use configured direct RPC endpoint",
        implemented: true,
        adapter: RPC_ADAPTER,
    },
    SourceModePolicy {
        key: "rpc",
        aliases: &[
            "rpc",
            "direct-rpc",
            "direct rpc",
            "standalone",
            "standalone-rpc",
            "standalone rpc",
        ],
        effective: "rpc",
        label_key: "direct_rpc",
        label: "Direct RPC",
        source_label: "Direct RPC",
        summary: "Use configured standalone RPC endpoint",
        implemented: true,
        adapter: RPC_ADAPTER,
    },
    SourceModePolicy {
        key: "module",
        aliases: &["module", "basecamp", "basecamp-module", "basecamp module"],
        effective: "module",
        label_key: "basecamp_module",
        label: "Basecamp module",
        source_label: "Basecamp module",
        summary: "Use Basecamp module transport",
        implemented: false,
        adapter: MODULE_ADAPTER,
    },
];

const DELIVERY_SOURCE_MODES: &[SourceModePolicy] = &[
    SourceModePolicy {
        key: "auto",
        aliases: &["auto"],
        effective: "rest",
        label_key: "auto_delivery_rest",
        label: "Auto",
        source_label: "Auto: Direct Waku REST",
        summary: "Use direct Waku REST",
        implemented: true,
        adapter: DELIVERY_REST_ADAPTER,
    },
    SourceModePolicy {
        key: "module",
        aliases: &["module", "basecamp", "basecamp-module", "basecamp module"],
        effective: "module",
        label_key: "delivery_module",
        label: "Delivery module",
        source_label: "Delivery module",
        summary: "Use delivery_module through logoscore for node lifecycle, subscriptions, and sends",
        implemented: true,
        adapter: MODULE_ADAPTER,
    },
    SourceModePolicy {
        key: "rest",
        aliases: &["rest", "direct-rest", "direct waku rest", "waku-rest"],
        effective: "rest",
        label_key: "delivery_rest",
        label: "Direct Waku REST",
        source_label: "Direct Waku REST",
        summary: "Read-only health, info, version, and optional metrics",
        implemented: true,
        adapter: DELIVERY_REST_ADAPTER,
    },
    SourceModePolicy {
        key: "metrics",
        aliases: &["metrics", "metrics-only", "metrics only"],
        effective: "metrics",
        label_key: "metrics_only",
        label: "Metrics only",
        source_label: "Metrics only",
        summary: "Scrape a Prometheus/OpenMetrics endpoint",
        implemented: true,
        adapter: DELIVERY_METRICS_ADAPTER,
    },
    SourceModePolicy {
        key: "network-monitor",
        aliases: &[
            "network-monitor",
            "network monitor",
            "discovery-crawler",
            "discovery crawler",
            "crawler",
        ],
        effective: "network-monitor",
        label_key: "network_monitor",
        label: "Network monitor",
        source_label: "Network monitor",
        summary: "Inspect fleet topology from allpeersinfo, contenttopics, and metrics",
        implemented: true,
        adapter: DELIVERY_MONITOR_ADAPTER,
    },
    SourceModePolicy {
        key: "unsupported",
        aliases: &["unsupported"],
        effective: "unsupported",
        label_key: "unsupported",
        label: "Unsupported saved source",
        source_label: "Unsupported source",
        summary: "Select a supported source to replace this saved value",
        implemented: false,
        adapter: NONE_ADAPTER,
    },
];

const STORAGE_SOURCE_MODES: &[SourceModePolicy] = &[
    SourceModePolicy {
        key: "auto",
        aliases: &["auto"],
        effective: "rest",
        label_key: "auto_storage_rest",
        label: "Auto",
        source_label: "Auto: Standalone REST",
        summary: "Use standalone REST",
        implemented: true,
        adapter: STORAGE_REST_ADAPTER,
    },
    SourceModePolicy {
        key: "module",
        aliases: &["module", "basecamp", "basecamp-module", "basecamp module"],
        effective: "module",
        label_key: "storage_module",
        label: "Storage module",
        source_label: "Storage module",
        summary: "Use storage_module through logoscore for read-only node and content checks",
        implemented: true,
        adapter: STORAGE_MODULE_ADAPTER,
    },
    SourceModePolicy {
        key: "rest",
        aliases: &[
            "rest",
            "standalone",
            "standalone-rest",
            "standalone rest",
            "direct-rest",
            "direct rest",
        ],
        effective: "rest",
        label_key: "storage_rest",
        label: "Standalone REST",
        source_label: "Standalone REST",
        summary: "Read-only space, identity, local data, debug, and metrics",
        implemented: true,
        adapter: STORAGE_REST_ADAPTER,
    },
    SourceModePolicy {
        key: "metrics",
        aliases: &["metrics", "metrics-only", "metrics only"],
        effective: "metrics",
        label_key: "metrics_only",
        label: "Metrics only",
        source_label: "Metrics only",
        summary: "Scrape a Prometheus/OpenMetrics endpoint",
        implemented: true,
        adapter: STORAGE_METRICS_ADAPTER,
    },
    SourceModePolicy {
        key: "unsupported",
        aliases: &[
            "c-library",
            "c library",
            "library",
            "local-os",
            "local os",
            "local diagnostics",
            "unsupported",
        ],
        effective: "unsupported",
        label_key: "unsupported",
        label: "Unsupported saved source",
        source_label: "Unsupported source",
        summary: "Select a supported source to replace this saved value",
        implemented: false,
        adapter: NONE_ADAPTER,
    },
];

impl CoreSourceMode {
    pub fn from_token(value: &str) -> Option<Self> {
        match normalized(value).as_str() {
            "auto" => Some(Self::Auto),
            "rpc" | "direct-rpc" | "direct rpc" | "standalone" | "standalone-rpc"
            | "standalone rpc" => Some(Self::Rpc),
            "module" | "basecamp" | "basecamp-module" | "basecamp module" => Some(Self::Module),
            _ => None,
        }
    }

    pub fn normalized(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Rpc => "rpc",
            Self::Module => "module",
        }
    }

    pub fn effective(self) -> CoreEndpointMode {
        match self {
            Self::Module => CoreEndpointMode::Module,
            Self::Auto | Self::Rpc => CoreEndpointMode::Rpc,
        }
    }
}

impl CoreEndpointMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rpc => "rpc",
            Self::Module => "module",
        }
    }
}

impl DeliverySourceMode {
    pub fn from_token(value: &str) -> Self {
        match normalized(value).as_str() {
            "auto" => Self::Auto,
            "module" | "basecamp" | "basecamp-module" | "basecamp module" => Self::Module,
            "rest" | "direct-rest" | "direct waku rest" | "waku-rest" => Self::Rest,
            "metrics" | "metrics-only" | "metrics only" => Self::Metrics,
            "network-monitor" | "network monitor" | "discovery-crawler" | "discovery crawler"
            | "crawler" => Self::NetworkMonitor,
            _ => Self::Unsupported,
        }
    }

    pub fn effective(self) -> Self {
        match self {
            Self::Auto => Self::Rest,
            value => value,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Module => "module",
            Self::Rest => "rest",
            Self::Metrics => "metrics",
            Self::NetworkMonitor => "network-monitor",
            Self::Unsupported => "unsupported",
        }
    }

    pub fn is_source_token(value: &str) -> bool {
        DELIVERY_SOURCE_MODES
            .iter()
            .any(|mode| source_mode_matches(mode, value))
    }
}

impl StorageSourceMode {
    pub fn from_token(value: &str) -> Self {
        match normalized(value).as_str() {
            "auto" => Self::Auto,
            "module" | "basecamp" | "basecamp-module" | "basecamp module" => Self::Module,
            "rest" | "standalone" | "standalone-rest" | "standalone rest" | "direct-rest"
            | "direct rest" => Self::Rest,
            "metrics" | "metrics-only" | "metrics only" => Self::Metrics,
            _ => Self::Unsupported,
        }
    }

    pub fn effective(self) -> Self {
        match self {
            Self::Auto => Self::Rest,
            value => value,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Module => "module",
            Self::Rest => "rest",
            Self::Metrics => "metrics",
            Self::Unsupported => "unsupported",
        }
    }

    pub fn is_source_token(value: &str) -> bool {
        STORAGE_SOURCE_MODES
            .iter()
            .any(|mode| source_mode_matches(mode, value))
    }
}

impl SourceFamily {
    #[must_use]
    pub fn modes(self) -> &'static [SourceModePolicy] {
        match self {
            Self::Core => CORE_SOURCE_MODES,
            Self::Delivery => DELIVERY_SOURCE_MODES,
            Self::Storage => STORAGE_SOURCE_MODES,
        }
    }

    #[must_use]
    pub fn from_domain(domain: &str) -> Option<Self> {
        match domain {
            "delivery" => Some(Self::Delivery),
            "storage" => Some(Self::Storage),
            _ => None,
        }
    }
}

#[must_use]
pub fn normalized_core_source_mode(value: &str) -> &'static str {
    CoreSourceMode::from_token(value)
        .filter(|mode| *mode != CoreSourceMode::Module)
        .map_or("auto", CoreSourceMode::normalized)
}

#[must_use]
pub fn source_mode_policy(family: SourceFamily, value: &str) -> &'static SourceModePolicy {
    family
        .modes()
        .iter()
        .find(|mode| source_mode_matches(mode, value))
        .unwrap_or_else(|| {
            family
                .modes()
                .iter()
                .find(|mode| mode.key == fallback_source_mode_key(family))
                .unwrap_or_else(|| fallback_source_mode(family))
        })
}

#[must_use]
pub fn normalized_source_mode(family: SourceFamily, value: &str) -> &'static str {
    source_mode_policy(family, value).key
}

#[must_use]
pub fn effective_source_mode(family: SourceFamily, value: &str) -> &'static str {
    source_mode_policy(family, value).effective
}

#[must_use]
pub fn source_mode_is_token(family: SourceFamily, value: &str) -> bool {
    family
        .modes()
        .iter()
        .any(|mode| source_mode_matches(mode, value))
}

#[must_use]
pub fn default_source_mode_for_domain(domain: &str) -> &'static str {
    match SourceFamily::from_domain(domain) {
        Some(SourceFamily::Delivery | SourceFamily::Storage) => "rest",
        _ => "rpc",
    }
}

#[must_use]
pub fn default_endpoint_for_domain(domain: &str) -> &'static str {
    match SourceFamily::from_domain(domain) {
        Some(SourceFamily::Delivery) => DEFAULT_DELIVERY_REST_ENDPOINT,
        Some(SourceFamily::Storage) => DEFAULT_STORAGE_REST_ENDPOINT,
        _ => "",
    }
}

#[must_use]
pub fn source_policy_report(network_profiles: &[NetworkProfile]) -> SourcePolicyReport {
    SourcePolicyReport {
        version: 2,
        defaults: SourcePolicyDefaults {
            sequencer_endpoint: DEFAULT_SEQUENCER_ENDPOINT,
            local_sequencer_endpoint: LOCAL_SEQUENCER_ENDPOINT,
            indexer_endpoint: DEFAULT_INDEXER_ENDPOINT,
            node_endpoint: DEFAULT_NODE_ENDPOINT,
            delivery_rest_endpoint: DEFAULT_DELIVERY_REST_ENDPOINT,
            delivery_metrics_endpoint: DEFAULT_DELIVERY_METRICS_ENDPOINT,
            storage_rest_endpoint: DEFAULT_STORAGE_REST_ENDPOINT,
            storage_metrics_endpoint: DEFAULT_STORAGE_METRICS_ENDPOINT,
        },
        network_profiles: network_profiles.to_vec(),
        source_modes: SourceModeFamilies {
            core: CORE_SOURCE_MODES,
            delivery: DELIVERY_SOURCE_MODES,
            storage: STORAGE_SOURCE_MODES,
        },
    }
}

#[must_use]
pub fn storage_source_facts(
    module: &str,
    module_info: &ProbeReport,
    probes: &[ProbeReport],
) -> SourceFacts {
    SourceFacts {
        health: storage_source_health(module, module_info, probes),
        capability_facts: storage_source_capability_facts(module, module_info, probes),
    }
}

#[must_use]
pub fn delivery_source_facts(
    module: &str,
    module_info: &ProbeReport,
    probes: &[ProbeReport],
) -> SourceFacts {
    SourceFacts {
        health: delivery_source_health(module, module_info, probes),
        capability_facts: delivery_source_capability_facts(module, module_info, probes),
    }
}

fn fallback_source_mode_key(family: SourceFamily) -> &'static str {
    match family {
        SourceFamily::Core => "auto",
        SourceFamily::Delivery | SourceFamily::Storage => "unsupported",
    }
}

fn fallback_source_mode(family: SourceFamily) -> &'static SourceModePolicy {
    match family {
        SourceFamily::Core => &FALLBACK_CORE_SOURCE_MODE,
        SourceFamily::Delivery | SourceFamily::Storage => &FALLBACK_UNSUPPORTED_SOURCE_MODE,
    }
}

fn storage_source_health(
    module: &str,
    module_info: &ProbeReport,
    probes: &[ProbeReport],
) -> SourceHealthFacts {
    if !matches!(module, "storage_rest" | "storage_metrics" | STORAGE_MODULE) {
        return unsupported_source_health(module_info);
    }
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
    if !matches!(
        module,
        "delivery_rest" | "delivery_metrics" | "delivery_network_monitor" | DELIVERY_MODULE
    ) {
        return unsupported_source_health(module_info);
    }
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

fn source_mode_matches(mode: &SourceModePolicy, value: &str) -> bool {
    let value = normalized(value);
    mode.key == value || mode.aliases.iter().any(|alias| *alias == value)
}

fn normalized(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    use crate::ProbeReport;

    #[test]
    fn storage_source_modes_normalize_aliases_to_effective_modes() {
        assert_eq!(
            StorageSourceMode::from_token("basecamp module")
                .effective()
                .as_str(),
            "module"
        );
        assert_eq!(
            StorageSourceMode::from_token("standalone rest")
                .effective()
                .as_str(),
            "rest"
        );
        assert_eq!(
            StorageSourceMode::from_token("local diagnostics")
                .effective()
                .as_str(),
            "unsupported"
        );
    }

    #[test]
    fn delivery_source_modes_keep_network_monitor_aliases() {
        assert_eq!(
            DeliverySourceMode::from_token("discovery crawler")
                .effective()
                .as_str(),
            "network-monitor"
        );
        assert!(DeliverySourceMode::is_source_token("direct waku rest"));
    }

    #[test]
    fn core_source_policy_keeps_basecamp_out_of_saved_gui_modes() {
        assert_eq!(normalized_core_source_mode("module"), "auto");
        assert_eq!(
            CoreSourceMode::from_token("basecamp")
                .map(CoreSourceMode::effective)
                .map(CoreEndpointMode::as_str),
            Some("module")
        );
    }

    #[test]
    fn source_policy_report_exposes_labels_and_adapter_facts() {
        let policy = source_policy_report(&[]);
        let delivery_rest = policy
            .source_modes
            .delivery
            .iter()
            .find(|mode| mode.key == "rest");
        let storage_auto = policy
            .source_modes
            .storage
            .iter()
            .find(|mode| mode.key == "auto");

        assert_eq!(policy.version, 2);
        assert_eq!(
            delivery_rest.map(|mode| mode.label),
            Some("Direct Waku REST")
        );
        assert_eq!(
            delivery_rest.map(|mode| mode.adapter.target),
            Some("rest_endpoint")
        );
        assert!(delivery_rest.is_some_and(|mode| mode.adapter.uses_rest_endpoint));
        assert!(delivery_rest.is_some_and(|mode| mode.adapter.uses_metrics_endpoint));
        assert_eq!(
            storage_auto.map(|mode| mode.source_label),
            Some("Auto: Standalone REST")
        );
        assert!(storage_auto.is_some_and(|mode| mode.adapter.supports_cid_probe));
    }

    #[test]
    fn source_policy_helpers_normalize_domain_defaults_and_aliases() {
        assert_eq!(
            effective_source_mode(SourceFamily::Storage, "basecamp module"),
            "module"
        );
        assert_eq!(
            effective_source_mode(SourceFamily::Delivery, "discovery crawler"),
            "network-monitor"
        );
        assert_eq!(
            normalized_source_mode(SourceFamily::Storage, "local-os"),
            "unsupported"
        );
        assert!(source_mode_is_token(
            SourceFamily::Delivery,
            "direct waku rest"
        ));
        assert_eq!(default_source_mode_for_domain("storage"), "rest");
        assert_eq!(
            default_endpoint_for_domain("delivery"),
            DEFAULT_DELIVERY_REST_ENDPOINT
        );
    }

    #[test]
    fn storage_source_facts_require_rest_core_facts() {
        let module_info = ProbeReport::ok("storage_rest.space", "http://storage/space", json!({}));
        let probes = vec![
            module_info.clone(),
            ProbeReport::ok("storage_rest.spr", "http://storage/spr", "spr-a"),
            ProbeReport::ok("storage_rest.peerId", "http://storage/peerid", "peer-a"),
            ProbeReport::ok("storage_rest.manifests", "http://storage/data", json!([])),
        ];

        let facts = storage_source_facts("storage_rest", &module_info, &probes);

        assert!(facts.health.reachable);
        assert!(facts.health.ready);
        assert_eq!(facts.health.status, SourceHealthStatus::Healthy);
        assert!(
            facts
                .capability_facts
                .iter()
                .any(|fact| fact.key == "identity" && fact.available)
        );
    }

    #[test]
    fn storage_source_facts_mark_rest_missing_facts_degraded() {
        let module_info = ProbeReport::ok("storage_rest.space", "http://storage/space", json!({}));
        let probes = vec![module_info.clone()];

        let facts = storage_source_facts("storage_rest", &module_info, &probes);

        assert!(facts.health.reachable);
        assert!(!facts.health.ready);
        assert_eq!(facts.health.status, SourceHealthStatus::Degraded);
    }

    #[test]
    fn delivery_source_facts_require_known_metrics_family() {
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

        assert!(
            !delivery_source_facts("delivery_metrics", &module_info, &generic_metrics)
                .health
                .ready
        );
        let facts = delivery_source_facts("delivery_metrics", &module_info, &waku_metrics);
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

        assert!(
            !delivery_source_facts("delivery_rest", &module_info, &missing_connection)
                .health
                .ready
        );
        assert!(
            delivery_source_facts("delivery_rest", &module_info, &connected)
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

        let facts = storage_source_facts("storage_unsupported", &module_info, &[]);

        assert_eq!(facts.health.status, SourceHealthStatus::Unsupported);
        assert!(!facts.health.reachable);
        assert!(!facts.health.ready);
    }
}
