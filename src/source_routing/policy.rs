use super::{NetworkProfile, network_profiles};
use serde::Serialize;

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
    key: "rpc",
    aliases: &[],
    effective: "rpc",
    label_key: "direct_rpc",
    label: "Direct RPC",
    source_label: "Direct RPC",
    summary: "Use configured standalone RPC endpoint",
    implemented: true,
    adapter: RPC_ADAPTER,
};
const FALLBACK_DELIVERY_SOURCE_MODE: SourceModePolicy = SourceModePolicy {
    key: "rest",
    aliases: &[],
    effective: "rest",
    label_key: "delivery_rest",
    label: "Direct Waku REST",
    source_label: "Direct Waku REST",
    summary: "Read-only health, info, version, and optional metrics",
    implemented: true,
    adapter: DELIVERY_REST_ADAPTER,
};
const FALLBACK_STORAGE_SOURCE_MODE: SourceModePolicy = SourceModePolicy {
    key: "rest",
    aliases: &[],
    effective: "rest",
    label_key: "storage_rest",
    label: "Standalone REST",
    source_label: "Standalone REST",
    summary: "Read-only space, identity, local data, debug, and metrics",
    implemented: true,
    adapter: STORAGE_REST_ADAPTER,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreSourceMode {
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
    Module,
    Rest,
    Metrics,
    NetworkMonitor,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageSourceMode {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceCapabilityKey {
    Identity,
    Space,
    ManifestListing,
    Debug,
    Metrics,
    CidExists,
    Health,
    Relay,
    Store,
    Filter,
    Lightpush,
    NetworkMonitor,
    RestApi,
    ModuleApi,
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

impl SourceCapabilityKey {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Space => "space",
            Self::ManifestListing => "manifest_listing",
            Self::Debug => "debug",
            Self::Metrics => "metrics",
            Self::CidExists => "cid_exists",
            Self::Health => "health",
            Self::Relay => "relay",
            Self::Store => "store",
            Self::Filter => "filter",
            Self::Lightpush => "lightpush",
            Self::NetworkMonitor => "network_monitor",
            Self::RestApi => "rest_api",
            Self::ModuleApi => "module_api",
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Identity => "Identity",
            Self::Space => "Repository space",
            Self::ManifestListing => "Manifest listing",
            Self::Debug => "Debug topology",
            Self::Metrics => "Metrics",
            Self::CidExists => "CID existence",
            Self::Health => "Health endpoint",
            Self::Relay => "Relay",
            Self::Store => "Store",
            Self::Filter => "Filter",
            Self::Lightpush => "Lightpush",
            Self::NetworkMonitor => "Delivery Network Monitor",
            Self::RestApi => "REST API",
            Self::ModuleApi => "Module API",
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

#[derive(Debug, Clone, Copy, Default)]
pub struct SourcePolicyCatalog;

impl SourcePolicyCatalog {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    #[must_use]
    pub fn mode_policy(self, family: SourceFamily, value: &str) -> &'static SourceModePolicy {
        self.mode_policy_for_token(family, value)
            .unwrap_or_else(|| self.fallback_mode_policy(family))
    }

    #[must_use]
    pub fn normalized_source_mode(self, family: SourceFamily, value: &str) -> &'static str {
        self.mode_policy(family, value).key
    }

    #[must_use]
    pub fn effective_source_mode(self, family: SourceFamily, value: &str) -> &'static str {
        self.mode_policy(family, value).effective
    }

    #[must_use]
    pub fn source_mode_is_token(self, family: SourceFamily, value: &str) -> bool {
        family
            .modes()
            .iter()
            .any(|mode| source_mode_matches(mode, value))
    }

    #[must_use]
    pub fn default_source_mode_for_domain(self, domain: &str) -> &'static str {
        match SourceFamily::from_domain(domain) {
            Some(SourceFamily::Delivery | SourceFamily::Storage) => "rest",
            _ => "rpc",
        }
    }

    #[must_use]
    pub fn default_endpoint_for_domain(self, domain: &str) -> &'static str {
        match SourceFamily::from_domain(domain) {
            Some(SourceFamily::Delivery) => DEFAULT_DELIVERY_REST_ENDPOINT,
            Some(SourceFamily::Storage) => DEFAULT_STORAGE_REST_ENDPOINT,
            _ => "",
        }
    }

    #[must_use]
    pub fn report(self) -> SourcePolicyReport {
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
            network_profiles: network_profiles().to_vec(),
            source_modes: SourceModeFamilies {
                core: CORE_SOURCE_MODES,
                delivery: DELIVERY_SOURCE_MODES,
                storage: STORAGE_SOURCE_MODES,
            },
        }
    }

    fn mode_policy_for_token(
        self,
        family: SourceFamily,
        value: &str,
    ) -> Option<&'static SourceModePolicy> {
        family
            .modes()
            .iter()
            .find(|mode| source_mode_matches(mode, value))
    }

    fn fallback_mode_policy(self, family: SourceFamily) -> &'static SourceModePolicy {
        family
            .modes()
            .iter()
            .find(|mode| mode.key == fallback_source_mode_key(family))
            .unwrap_or_else(|| fallback_source_mode(family))
    }
}

const CORE_SOURCE_MODES: &[SourceModePolicy] = &[
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
            "delivery-network-monitor",
            "delivery network monitor",
            "discovery-crawler",
            "discovery crawler",
        ],
        effective: "network-monitor",
        label_key: "delivery_network_monitor",
        label: "Delivery Network Monitor",
        source_label: "Delivery Network Monitor",
        summary: "Inspect Delivery fleet topology from allpeersinfo, contenttopics, and metrics",
        implemented: true,
        adapter: DELIVERY_MONITOR_ADAPTER,
    },
];

const STORAGE_SOURCE_MODES: &[SourceModePolicy] = &[
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
];

impl CoreSourceMode {
    pub fn from_token(value: &str) -> Option<Self> {
        let policy = SourcePolicyCatalog::new().mode_policy_for_token(SourceFamily::Core, value)?;
        match policy.key {
            "rpc" => Some(Self::Rpc),
            "module" => Some(Self::Module),
            _ => None,
        }
    }

    pub fn normalized(self) -> &'static str {
        match self {
            Self::Rpc => "rpc",
            Self::Module => "module",
        }
    }

    pub fn effective(self) -> CoreEndpointMode {
        match self {
            Self::Module => CoreEndpointMode::Module,
            Self::Rpc => CoreEndpointMode::Rpc,
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
        SourcePolicyCatalog::new()
            .mode_policy_for_token(SourceFamily::Delivery, value)
            .map(|policy| match policy.key {
                "module" => Self::Module,
                "rest" => Self::Rest,
                "metrics" => Self::Metrics,
                "network-monitor" => Self::NetworkMonitor,
                _ => Self::Unsupported,
            })
            .unwrap_or(Self::Unsupported)
    }

    pub fn effective(self) -> Self {
        self
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::Rest => "rest",
            Self::Metrics => "metrics",
            Self::NetworkMonitor => "network-monitor",
            Self::Unsupported => "unsupported",
        }
    }

    pub fn is_source_token(value: &str) -> bool {
        SourcePolicyCatalog::new().source_mode_is_token(SourceFamily::Delivery, value)
    }
}

impl StorageSourceMode {
    pub fn from_token(value: &str) -> Self {
        SourcePolicyCatalog::new()
            .mode_policy_for_token(SourceFamily::Storage, value)
            .map(|policy| match policy.key {
                "module" => Self::Module,
                "rest" => Self::Rest,
                "metrics" => Self::Metrics,
                _ => Self::Unsupported,
            })
            .unwrap_or(Self::Unsupported)
    }

    pub fn effective(self) -> Self {
        self
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::Rest => "rest",
            Self::Metrics => "metrics",
            Self::Unsupported => "unsupported",
        }
    }

    pub fn is_source_token(value: &str) -> bool {
        SourcePolicyCatalog::new().source_mode_is_token(SourceFamily::Storage, value)
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
    CoreSourceMode::from_token(value).map_or("rpc", CoreSourceMode::normalized)
}

#[must_use]
pub fn source_mode_policy(family: SourceFamily, value: &str) -> &'static SourceModePolicy {
    SourcePolicyCatalog::new().mode_policy(family, value)
}

#[must_use]
pub fn normalized_source_mode(family: SourceFamily, value: &str) -> &'static str {
    SourcePolicyCatalog::new().normalized_source_mode(family, value)
}

#[must_use]
pub fn effective_source_mode(family: SourceFamily, value: &str) -> &'static str {
    SourcePolicyCatalog::new().effective_source_mode(family, value)
}

#[must_use]
pub fn source_mode_is_token(family: SourceFamily, value: &str) -> bool {
    SourcePolicyCatalog::new().source_mode_is_token(family, value)
}

#[must_use]
pub fn default_source_mode_for_domain(domain: &str) -> &'static str {
    SourcePolicyCatalog::new().default_source_mode_for_domain(domain)
}

#[must_use]
pub fn default_endpoint_for_domain(domain: &str) -> &'static str {
    SourcePolicyCatalog::new().default_endpoint_for_domain(domain)
}

#[must_use]
pub fn source_policy_report() -> SourcePolicyReport {
    SourcePolicyCatalog::new().report()
}

fn fallback_source_mode_key(family: SourceFamily) -> &'static str {
    match family {
        SourceFamily::Core => "rpc",
        SourceFamily::Delivery | SourceFamily::Storage => "rest",
    }
}

fn fallback_source_mode(family: SourceFamily) -> &'static SourceModePolicy {
    match family {
        SourceFamily::Core => &FALLBACK_CORE_SOURCE_MODE,
        SourceFamily::Delivery => &FALLBACK_DELIVERY_SOURCE_MODE,
        SourceFamily::Storage => &FALLBACK_STORAGE_SOURCE_MODE,
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
    fn delivery_source_modes_scope_network_monitor_aliases() {
        assert_eq!(
            DeliverySourceMode::from_token("discovery crawler")
                .effective()
                .as_str(),
            "network-monitor"
        );
        assert_eq!(
            DeliverySourceMode::from_token("delivery network monitor")
                .effective()
                .as_str(),
            "network-monitor"
        );
        assert_eq!(
            DeliverySourceMode::from_token("network monitor").as_str(),
            "unsupported"
        );
        assert_eq!(
            DeliverySourceMode::from_token("crawler").as_str(),
            "unsupported"
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
    fn source_policy_catalog_owns_mode_defaults_and_report_shape() {
        let catalog = SourcePolicyCatalog::new();

        assert_eq!(
            catalog
                .mode_policy(SourceFamily::Delivery, "delivery network monitor")
                .key,
            "network-monitor"
        );
        assert_eq!(
            catalog.normalized_source_mode(SourceFamily::Storage, "local diagnostics"),
            "rest"
        );
        assert!(!catalog.source_mode_is_token(SourceFamily::Storage, "local diagnostics"));
        assert_eq!(
            catalog.effective_source_mode(SourceFamily::Core, "basecamp"),
            "module"
        );
        assert_eq!(catalog.default_source_mode_for_domain("storage"), "rest");
        assert_eq!(
            catalog.default_endpoint_for_domain("delivery"),
            DEFAULT_DELIVERY_REST_ENDPOINT
        );

        let report = catalog.report();
        assert_eq!(report.version, 2);
        assert!(
            report
                .source_modes
                .delivery
                .iter()
                .any(|mode| mode.key == "network-monitor")
        );
    }

    #[test]
    fn source_policy_report_exposes_labels_and_adapter_facts() {
        let policy = source_policy_report();
        let delivery_rest = policy
            .source_modes
            .delivery
            .iter()
            .find(|mode| mode.key == "rest");
        let storage_rest = policy
            .source_modes
            .storage
            .iter()
            .find(|mode| mode.key == "rest");
        let delivery_network_monitor = policy
            .source_modes
            .delivery
            .iter()
            .find(|mode| mode.key == "network-monitor");

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
            storage_rest.map(|mode| mode.source_label),
            Some("Standalone REST")
        );
        assert!(storage_rest.is_some_and(|mode| mode.adapter.supports_cid_probe));
        assert_eq!(
            delivery_network_monitor.map(|mode| mode.label),
            Some("Delivery Network Monitor")
        );
        assert_eq!(
            delivery_network_monitor.map(|mode| mode.source_label),
            Some("Delivery Network Monitor")
        );
        assert!(delivery_network_monitor.is_some_and(|mode| {
            !mode.aliases.contains(&"network monitor") && !mode.aliases.contains(&"crawler")
        }));
        assert!(
            !policy
                .source_modes
                .core
                .iter()
                .any(|mode| mode.key == "auto")
        );
        assert!(
            !policy
                .source_modes
                .delivery
                .iter()
                .any(|mode| mode.key == "auto" || mode.label == "Unsupported saved source")
        );
        assert!(
            !policy
                .source_modes
                .storage
                .iter()
                .any(|mode| mode.key == "auto" || mode.label == "Unsupported saved source")
        );
    }

    #[test]
    fn source_capability_key_keeps_wire_contract() {
        let expected = [
            (SourceCapabilityKey::Identity, "identity", "Identity"),
            (SourceCapabilityKey::Space, "space", "Repository space"),
            (
                SourceCapabilityKey::ManifestListing,
                "manifest_listing",
                "Manifest listing",
            ),
            (SourceCapabilityKey::Debug, "debug", "Debug topology"),
            (SourceCapabilityKey::Metrics, "metrics", "Metrics"),
            (
                SourceCapabilityKey::CidExists,
                "cid_exists",
                "CID existence",
            ),
            (SourceCapabilityKey::Health, "health", "Health endpoint"),
            (SourceCapabilityKey::Relay, "relay", "Relay"),
            (SourceCapabilityKey::Store, "store", "Store"),
            (SourceCapabilityKey::Filter, "filter", "Filter"),
            (SourceCapabilityKey::Lightpush, "lightpush", "Lightpush"),
            (
                SourceCapabilityKey::NetworkMonitor,
                "network_monitor",
                "Delivery Network Monitor",
            ),
            (SourceCapabilityKey::RestApi, "rest_api", "REST API"),
            (SourceCapabilityKey::ModuleApi, "module_api", "Module API"),
        ];

        for (key, wire_key, label) in expected {
            assert_eq!(key.as_str(), wire_key);
            assert_eq!(key.label(), label);
        }
    }

    #[test]
    fn qml_fallback_catalog_tracks_rust_source_policy() -> Result<(), String> {
        let catalog_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("qml/state/source_routing/SourcePolicyCatalog.generated.js");
        let catalog = std::fs::read_to_string(catalog_path)
            .map_err(|error| format!("QML source policy catalog is readable: {error}"))?;
        let expected = generated_source_policy_catalog()
            .map_err(|error| format!("failed to generate source policy catalog: {error}"))?;
        if catalog != expected {
            return Err(
                "QML generated source policy catalog does not match Rust source policy".to_owned(),
            );
        }
        Ok(())
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
            "rest"
        );
        assert!(!source_mode_is_token(SourceFamily::Storage, "local-os"));
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

    fn generated_source_policy_catalog() -> Result<String, serde_json::Error> {
        let policy_json = serde_json::to_string(&source_policy_report())?;
        let policy_literal = serde_json::to_string(&policy_json)?;
        Ok(format!(
            "const SOURCE_POLICY_JSON = {policy_literal}\n\nfunction sourcePolicy() {{\n    return JSON.parse(SOURCE_POLICY_JSON)\n}}\n"
        ))
    }
}
