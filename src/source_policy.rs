use serde::Serialize;

use crate::network::NetworkProfile;

pub const TESTNET_SEQUENCER_ENDPOINT: &str = "https://testnet.lez.logos.co/";
pub const LOCAL_SEQUENCER_ENDPOINT: &str = "http://127.0.0.1:3040/";
pub const DEFAULT_SEQUENCER_ENDPOINT: &str = TESTNET_SEQUENCER_ENDPOINT;
pub const DEFAULT_INDEXER_ENDPOINT: &str = "http://127.0.0.1:8779/";
pub const DEFAULT_NODE_ENDPOINT: &str = "http://127.0.0.1:8080/";
pub const DEFAULT_DELIVERY_REST_ENDPOINT: &str = "http://127.0.0.1:8645";
pub const DEFAULT_DELIVERY_METRICS_ENDPOINT: &str = "http://127.0.0.1:8008/metrics";
pub const DEFAULT_STORAGE_REST_ENDPOINT: &str = "http://127.0.0.1:8080/api/storage/v1";
pub const DEFAULT_STORAGE_METRICS_ENDPOINT: &str = "http://127.0.0.1:8008/metrics";

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
    pub implemented: bool,
}

const CORE_SOURCE_MODES: &[SourceModePolicy] = &[
    SourceModePolicy {
        key: "auto",
        aliases: &["auto"],
        effective: "rpc",
        label_key: "auto_rpc",
        implemented: true,
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
        implemented: true,
    },
    SourceModePolicy {
        key: "module",
        aliases: &["module", "basecamp", "basecamp-module", "basecamp module"],
        effective: "module",
        label_key: "basecamp_module",
        implemented: false,
    },
];

const DELIVERY_SOURCE_MODES: &[SourceModePolicy] = &[
    SourceModePolicy {
        key: "auto",
        aliases: &["auto"],
        effective: "rest",
        label_key: "auto_delivery_rest",
        implemented: true,
    },
    SourceModePolicy {
        key: "module",
        aliases: &["module", "basecamp", "basecamp-module", "basecamp module"],
        effective: "module",
        label_key: "delivery_module",
        implemented: true,
    },
    SourceModePolicy {
        key: "rest",
        aliases: &["rest", "direct-rest", "direct waku rest", "waku-rest"],
        effective: "rest",
        label_key: "delivery_rest",
        implemented: true,
    },
    SourceModePolicy {
        key: "metrics",
        aliases: &["metrics", "metrics-only", "metrics only"],
        effective: "metrics",
        label_key: "metrics_only",
        implemented: true,
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
        implemented: true,
    },
    SourceModePolicy {
        key: "unsupported",
        aliases: &["unsupported"],
        effective: "unsupported",
        label_key: "unsupported",
        implemented: false,
    },
];

const STORAGE_SOURCE_MODES: &[SourceModePolicy] = &[
    SourceModePolicy {
        key: "auto",
        aliases: &["auto"],
        effective: "rest",
        label_key: "auto_storage_rest",
        implemented: true,
    },
    SourceModePolicy {
        key: "module",
        aliases: &["module", "basecamp", "basecamp-module", "basecamp module"],
        effective: "module",
        label_key: "storage_module",
        implemented: true,
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
        implemented: true,
    },
    SourceModePolicy {
        key: "metrics",
        aliases: &["metrics", "metrics-only", "metrics only"],
        effective: "metrics",
        label_key: "metrics_only",
        implemented: true,
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
        implemented: false,
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

#[must_use]
pub fn normalized_core_source_mode(value: &str) -> &'static str {
    CoreSourceMode::from_token(value)
        .filter(|mode| *mode != CoreSourceMode::Module)
        .map_or("auto", CoreSourceMode::normalized)
}

#[must_use]
pub fn source_policy_report(network_profiles: &[NetworkProfile]) -> SourcePolicyReport {
    SourcePolicyReport {
        version: 1,
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
}
