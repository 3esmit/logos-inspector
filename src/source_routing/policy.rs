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
    supports_mutating_diagnostics: true,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliverySourceReportKind {
    Module,
    Rest,
    Metrics,
    NetworkMonitor,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageSourceReportKind {
    Module,
    Rest,
    Metrics,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceProbeKey {
    DeliveryAvailableConfigs,
    DeliveryAvailableNodeInfoIds,
    DeliveryCollectOpenMetricsText,
    DeliveryConnectionStatus,
    DeliveryContentTopics,
    DeliveryEnrUri,
    DeliveryHealth,
    DeliveryInfo,
    DeliveryListenAddresses,
    DeliveryMetricsScrape,
    DeliveryMyEnr,
    DeliveryMyMultiaddresses,
    DeliveryMyPeerId,
    DeliveryNodeHealth,
    DeliveryNodeInfoMetrics,
    DeliveryNodeInfoVersion,
    DeliveryPeerId,
    DeliveryProtocolsHealth,
    DeliveryAllPeersInfo,
    DeliveryVersion,
    StorageCollectMetrics,
    StorageDataDir,
    StorageDebug,
    StorageExists,
    StorageManifests,
    StorageMetricsScrape,
    StorageModuleVersion,
    StoragePeerId,
    StoragePrivilegedProbe,
    StorageSpace,
    StorageSpr,
    StorageVersion,
}

impl SourceProbeKey {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeliveryAvailableConfigs => "getAvailableConfigs",
            Self::DeliveryAvailableNodeInfoIds => "getAvailableNodeInfoIDs",
            Self::DeliveryCollectOpenMetricsText => "collectOpenMetricsText",
            Self::DeliveryConnectionStatus => "connectionStatus",
            Self::DeliveryContentTopics => "contentTopics",
            Self::DeliveryEnrUri => "enrUri",
            Self::DeliveryHealth => "health",
            Self::DeliveryInfo => "info",
            Self::DeliveryListenAddresses => "listenAddresses",
            Self::DeliveryMetricsScrape => "metricsScrape",
            Self::DeliveryMyEnr => "MyENR",
            Self::DeliveryMyMultiaddresses => "MyMultiaddresses",
            Self::DeliveryMyPeerId => "MyPeerId",
            Self::DeliveryNodeHealth => "nodeHealth",
            Self::DeliveryNodeInfoMetrics => "Metrics",
            Self::DeliveryNodeInfoVersion => "Version",
            Self::DeliveryPeerId => "peerId",
            Self::DeliveryProtocolsHealth => "protocolsHealth",
            Self::DeliveryAllPeersInfo => "allPeersInfo",
            Self::DeliveryVersion => "version",
            Self::StorageCollectMetrics => "collectMetrics",
            Self::StorageDataDir => "dataDir",
            Self::StorageDebug => "debug",
            Self::StorageExists => "exists",
            Self::StorageManifests => "manifests",
            Self::StorageMetricsScrape => "metricsScrape",
            Self::StorageModuleVersion => "moduleVersion",
            Self::StoragePeerId => "peerId",
            Self::StoragePrivilegedProbe => "privilegedProbe",
            Self::StorageSpace => "space",
            Self::StorageSpr => "spr",
            Self::StorageVersion => "version",
        }
    }
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
        summary: "Use Basecamp module APIs where the installed modules expose Inspector data",
        implemented: true,
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
        summary: "Use storage_module through logoscore for manifests, CID checks, uploads, downloads, and node storage operations",
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
    CoreSourceMode::from_token(value).map_or("auto", CoreSourceMode::normalized)
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
pub fn delivery_source_facts(
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
            "identity",
            "Identity",
            probe_facts,
            &[SourceProbeKey::StoragePeerId, SourceProbeKey::StorageSpr],
        ),
        any_probe_fact(
            "space",
            "Repository space",
            probe_facts,
            &[SourceProbeKey::StorageSpace],
        ),
        any_probe_fact(
            "manifest_listing",
            "Manifest listing",
            probe_facts,
            &[SourceProbeKey::StorageManifests],
        ),
        any_probe_fact(
            "debug",
            "Debug topology",
            probe_facts,
            &[SourceProbeKey::StorageDebug],
        ),
        storage_metrics_fact(probe_facts),
    ];
    if let Some(probe) = source_probe_fact(probe_facts, SourceProbeKey::StorageExists) {
        facts.push(probe_fact("cid_exists", "CID existence", probe));
    }
    if kind == StorageSourceReportKind::Rest {
        facts.push(SourceCapabilityFact::available(
            "rest_api",
            "REST API",
            "REST probes available",
            None,
        ));
    } else if kind == StorageSourceReportKind::Module {
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
    kind: DeliverySourceReportKind,
    probe_facts: &[SourceProbeFact],
) -> Vec<SourceCapabilityFact> {
    let mut facts = vec![
        any_probe_fact(
            "identity",
            "Identity",
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
            "health",
            "Health endpoint",
            probe_facts,
            &[
                SourceProbeKey::DeliveryHealth,
                SourceProbeKey::DeliveryNodeHealth,
                SourceProbeKey::DeliveryConnectionStatus,
            ],
        ),
        delivery_metrics_fact(probe_facts),
        delivery_protocol_fact(
            "relay",
            "Relay",
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
            "store",
            "Store",
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
            "filter",
            "Filter",
            &["waku_filter", "waku_filter_peers", "waku_filter_requests"],
            &["filter"],
            probe_facts,
        ),
        delivery_protocol_fact(
            "lightpush",
            "Lightpush",
            &["waku_lightpush", "waku_lightpush_peers", "lightpush"],
            &["lightpush"],
            probe_facts,
        ),
        any_probe_fact(
            "network_monitor",
            "Network monitor",
            probe_facts,
            &[
                SourceProbeKey::DeliveryAllPeersInfo,
                SourceProbeKey::DeliveryContentTopics,
            ],
        ),
    ];
    if kind == DeliverySourceReportKind::Rest {
        facts.push(SourceCapabilityFact::available(
            "rest_api",
            "REST API",
            "REST probes available",
            None,
        ));
    } else if kind == DeliverySourceReportKind::Module {
        facts.push(SourceCapabilityFact::available(
            "module_api",
            "Module API",
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
    key: &str,
    label: &str,
    facts: &[SourceProbeFact],
    probe_keys: &[SourceProbeKey],
) -> SourceCapabilityFact {
    let mut fallback = None;
    for probe_key in probe_keys {
        if let Some(probe) = source_probe_fact(facts, *probe_key) {
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

fn probe_fact(key: &str, label: &str, probe: &SourceProbeFact) -> SourceCapabilityFact {
    if probe.ok {
        SourceCapabilityFact::available(key, label, probe.evidence.clone(), probe.value.clone())
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

fn storage_metrics_fact(facts: &[SourceProbeFact]) -> SourceCapabilityFact {
    let probe = source_probe_fact(facts, SourceProbeKey::StorageCollectMetrics);
    if storage_metrics_evidence_present(facts) {
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

fn delivery_metrics_fact(facts: &[SourceProbeFact]) -> SourceCapabilityFact {
    let probe = source_probe_fact(facts, SourceProbeKey::DeliveryCollectOpenMetricsText)
        .or_else(|| source_probe_fact(facts, SourceProbeKey::DeliveryNodeInfoMetrics));
    if delivery_metrics_evidence_present(facts) {
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
    key: &str,
    label: &str,
    metric_needles: &[&str],
    protocol_needles: &[&str],
    facts: &[SourceProbeFact],
) -> SourceCapabilityFact {
    if metric_text_contains(facts, metric_needles) {
        return SourceCapabilityFact::available(key, label, "metric family observed", None);
    }
    if protocol_health_contains(facts, protocol_needles) {
        return SourceCapabilityFact::available(key, label, "protocol health observed", None);
    }
    SourceCapabilityFact::unavailable(key, label, "not observed")
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

    fn probe_ok(key: SourceProbeKey, value: impl serde::Serialize) -> ProbeReport {
        ProbeReport::ok("renamed probe", "opaque source", value).with_probe_key(key.as_str())
    }

    fn probe_err(key: SourceProbeKey, error: &str) -> ProbeReport {
        ProbeReport::err("renamed probe", "opaque source", error).with_probe_key(key.as_str())
    }

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
    fn core_source_policy_preserves_basecamp_module_mode() {
        assert_eq!(normalized_core_source_mode("module"), "module");
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
