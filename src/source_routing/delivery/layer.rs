use anyhow::{Context as _, Result};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    modules::ModuleReport,
    modules::logos_core::{LogoscoreCliRuntime, ModuleTransportKind, SharedModuleTransport},
    source_routing::{
        DEFAULT_DELIVERY_METRICS_ENDPOINT, DEFAULT_DELIVERY_REST_ENDPOINT,
        adapter::{
            AdapterConnectionType, AdapterInitialization, AdapterInputPolicy,
            ManagedLifecycleOutcome, ManagedModuleCallSpec, ManagedNodeAction, ManagedNodeContract,
            SourceAdapterPolicy, SourceModePolicy,
        },
    },
};

const DELIVERY_MODULE: &str = "delivery_module";

static MANAGED_CONTRACT: ManagedNodeContract = ManagedNodeContract::new(
    DELIVERY_MODULE,
    ensure_managed_module,
    call_managed_module,
    managed_call_spec,
    Some(managed_lifecycle_event),
    Some(decode_managed_lifecycle_event),
);

#[must_use]
pub(crate) const fn managed_contract() -> &'static ManagedNodeContract {
    &MANAGED_CONTRACT
}

#[must_use]
pub(crate) const fn module_id() -> &'static str {
    DELIVERY_MODULE
}

pub(crate) fn ensure_managed_module(runtime: &LogoscoreCliRuntime) -> Result<()> {
    runtime.ensure_module_loaded(module_id())
}

pub(crate) fn call_managed_module(
    runtime: &LogoscoreCliRuntime,
    method: &str,
    signature: &str,
    args: &[String],
) -> Result<Value> {
    runtime.call_checked(module_id(), method, signature, args)
}

#[must_use]
pub(crate) fn managed_call_spec(
    action: ManagedNodeAction,
    config_path: &str,
) -> Option<ManagedModuleCallSpec> {
    match action {
        ManagedNodeAction::Initialize => Some(ManagedModuleCallSpec::new(
            "createNode",
            "createNode(QString)",
            vec![format!("@{config_path}")],
        )),
        ManagedNodeAction::Start => {
            Some(ManagedModuleCallSpec::new("start", "start()", Vec::new()))
        }
        ManagedNodeAction::Stop => Some(ManagedModuleCallSpec::new("stop", "stop()", Vec::new())),
        ManagedNodeAction::Destroy => None,
    }
}

#[must_use]
pub(crate) fn managed_config(port: Option<u16>) -> Value {
    json!({
        "mode": "Core",
        "preset": crate::testnet::LOGOS_TESTNET_PRESET,
        "tcpPort": 30303,
        "discv5UdpPort": 9000,
        "discv5Discovery": true,
        "nat": "any",
        "rest": true,
        "restAddress": "127.0.0.1",
        "restPort": port.unwrap_or(8645),
        "logLevel": "INFO",
        "logFormat": "TEXT",
    })
}

#[must_use]
pub(crate) const fn managed_lifecycle_event(action: ManagedNodeAction) -> Option<&'static str> {
    match action {
        ManagedNodeAction::Start => Some("nodeStarted"),
        ManagedNodeAction::Stop => Some("nodeStopped"),
        ManagedNodeAction::Initialize | ManagedNodeAction::Destroy => None,
    }
}

pub(crate) fn managed_lifecycle_outcome(
    success: Option<&Value>,
    detail: Option<&Value>,
) -> Result<ManagedLifecycleOutcome> {
    let success = success
        .and_then(Value::as_bool)
        .context("delivery lifecycle event has no success flag")?;
    let detail = detail
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    Ok(ManagedLifecycleOutcome { success, detail })
}

fn decode_managed_lifecycle_event(
    data: &serde_json::Map<String, Value>,
) -> Result<ManagedLifecycleOutcome> {
    managed_lifecycle_outcome(data.get("arg0"), data.get("arg1"))
}

const REST_INPUTS: &[AdapterInputPolicy] = &[
    AdapterInputPolicy {
        key: "rest_endpoint",
        label: "Waku REST URL",
        required: true,
    },
    AdapterInputPolicy {
        key: "metrics_endpoint",
        label: "Metrics URL",
        required: false,
    },
];
const METRICS_INPUTS: &[AdapterInputPolicy] = &[AdapterInputPolicy {
    key: "metrics_endpoint",
    label: "Metrics URL",
    required: true,
}];

const MODULE_CAPABILITIES: &[&str] = &[
    "delivery.identity.read",
    "delivery.subscribe",
    "delivery.unsubscribe",
    "delivery.send",
    "delivery.node.start",
    "delivery.node.stop",
];
const REST_CAPABILITIES: &[&str] = &[
    "delivery.identity.read",
    "delivery.store.query",
    "delivery.subscribe",
    "delivery.unsubscribe",
    "delivery.send",
    "delivery.metrics.read",
];
const METRICS_CAPABILITIES: &[&str] = &["delivery.metrics.read"];
const MONITOR_CAPABILITIES: &[&str] = &[
    "delivery.identity.read",
    "delivery.topics.read",
    "delivery.network_monitor.read",
    "delivery.metrics.read",
];

pub(crate) const MESSAGING_SOURCE_MODES: &[SourceModePolicy] = &[
    SourceModePolicy {
        key: "module",
        aliases: &["module", "basecamp", "basecamp-module", "basecamp module"],
        effective: "module",
        label_key: "delivery_module",
        label: "Delivery module",
        source_label: "Delivery module",
        summary: "Use the host-provided Delivery module API",
        implemented: true,
        adapter: SourceAdapterPolicy {
            connector_id: DELIVERY_MODULE,
            connection_type: AdapterConnectionType::Module,
            target: "module",
            module_id: Some(DELIVERY_MODULE),
            inputs: &[],
            capabilities: MODULE_CAPABILITIES,
            supports_cid_probe: false,
            supports_mutating_diagnostics: true,
            capability_scopes: &["delivery"],
            endpoint_role: None,
        },
    },
    SourceModePolicy {
        key: "logoscore_cli",
        aliases: &["logoscore_cli", "logoscore-cli", "logoscore cli"],
        effective: "module",
        label_key: "logoscore_cli",
        label: "LogosCore CLI",
        source_label: "LogosCore CLI (Delivery)",
        summary: "Call delivery_module with logoscore call",
        implemented: true,
        adapter: SourceAdapterPolicy {
            connector_id: "logoscore_cli_delivery_module",
            connection_type: AdapterConnectionType::LogoscoreCli,
            target: "module",
            module_id: Some(DELIVERY_MODULE),
            inputs: &[],
            capabilities: MODULE_CAPABILITIES,
            supports_cid_probe: false,
            supports_mutating_diagnostics: true,
            capability_scopes: &["delivery"],
            endpoint_role: None,
        },
    },
    SourceModePolicy {
        key: "rest",
        aliases: &["rest", "direct-rest", "direct waku rest", "waku-rest"],
        effective: "rest",
        label_key: "delivery_rest",
        label: "Direct Waku REST",
        source_label: "Direct Waku REST",
        summary: "Inspect Delivery through Waku REST and optional metrics",
        implemented: true,
        adapter: SourceAdapterPolicy {
            connector_id: "direct_delivery_rest",
            connection_type: AdapterConnectionType::Rest,
            target: "rest_endpoint",
            module_id: None,
            inputs: REST_INPUTS,
            capabilities: REST_CAPABILITIES,
            supports_cid_probe: false,
            supports_mutating_diagnostics: true,
            capability_scopes: &["delivery"],
            endpoint_role: Some("messaging_rest_url"),
        },
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
        adapter: SourceAdapterPolicy {
            connector_id: "delivery_metrics",
            connection_type: AdapterConnectionType::Metrics,
            target: "metrics_endpoint",
            module_id: None,
            inputs: METRICS_INPUTS,
            capabilities: METRICS_CAPABILITIES,
            supports_cid_probe: false,
            supports_mutating_diagnostics: false,
            capability_scopes: &["delivery"],
            endpoint_role: Some("messaging_metrics_url"),
        },
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
        summary: "Inspect Delivery fleet topology and optional metrics",
        implemented: true,
        adapter: SourceAdapterPolicy {
            connector_id: "delivery_network_monitor",
            connection_type: AdapterConnectionType::NetworkMonitor,
            target: "rest_endpoint",
            module_id: None,
            inputs: REST_INPUTS,
            capabilities: MONITOR_CAPABILITIES,
            supports_cid_probe: false,
            supports_mutating_diagnostics: false,
            capability_scopes: &["delivery"],
            endpoint_role: Some("messaging_rest_url"),
        },
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MessagingAdapter<'a> {
    Module {
        transport: ModuleTransportKind,
    },
    Rest {
        endpoint: &'a str,
        metrics_endpoint: Option<&'a str>,
    },
    Metrics {
        endpoint: &'a str,
    },
    NetworkMonitor {
        endpoint: &'a str,
        metrics_endpoint: Option<&'a str>,
    },
    Unsupported {
        mode: &'a str,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MessagingReportInputs {
    pub(crate) source_mode: String,
    pub(crate) rest_endpoint: Option<String>,
    pub(crate) metrics_endpoint: Option<String>,
    pub(crate) runtime_diagnostics_enabled: bool,
    pub(crate) runtime_metrics_enabled: bool,
}

#[derive(Debug, Default, Deserialize)]
struct MessagingReportEnvelope {
    #[serde(default)]
    options: MessagingReportOptions,
}

#[derive(Debug, Default, Deserialize)]
struct MessagingReportOptions {
    #[serde(default)]
    runtime_diagnostics_enabled: bool,
    #[serde(default)]
    runtime_metrics_enabled: bool,
    #[serde(default)]
    health_endpoint: Option<String>,
}

impl<'a> MessagingAdapter<'a> {
    #[must_use]
    pub(crate) const fn module(transport: ModuleTransportKind) -> Self {
        Self::Module { transport }
    }

    #[must_use]
    pub(crate) const fn rest(endpoint: &'a str, metrics_endpoint: Option<&'a str>) -> Self {
        Self::Rest {
            endpoint,
            metrics_endpoint,
        }
    }

    #[must_use]
    pub(crate) const fn metrics(endpoint: &'a str) -> Self {
        Self::Metrics { endpoint }
    }

    #[must_use]
    pub(crate) const fn network_monitor(
        endpoint: &'a str,
        metrics_endpoint: Option<&'a str>,
    ) -> Self {
        Self::NetworkMonitor {
            endpoint,
            metrics_endpoint,
        }
    }

    #[must_use]
    pub(crate) fn select(
        source_mode: &'a str,
        rest_endpoint: Option<&'a str>,
        metrics_endpoint: Option<&'a str>,
    ) -> Self {
        match crate::source_routing::DeliverySourceMode::from_token(source_mode) {
            crate::source_routing::DeliverySourceMode::Module => {
                Self::module(ModuleTransportKind::Module)
            }
            crate::source_routing::DeliverySourceMode::LogoscoreCli => {
                Self::module(ModuleTransportKind::LogoscoreCli)
            }
            crate::source_routing::DeliverySourceMode::Rest => Self::rest(
                present(rest_endpoint).unwrap_or(DEFAULT_DELIVERY_REST_ENDPOINT),
                present(metrics_endpoint),
            ),
            crate::source_routing::DeliverySourceMode::Metrics => Self::metrics(
                present(metrics_endpoint).unwrap_or(DEFAULT_DELIVERY_METRICS_ENDPOINT),
            ),
            crate::source_routing::DeliverySourceMode::NetworkMonitor => Self::network_monitor(
                present(rest_endpoint).unwrap_or(DEFAULT_DELIVERY_REST_ENDPOINT),
                present(metrics_endpoint),
            ),
            crate::source_routing::DeliverySourceMode::Unsupported => {
                Self::Unsupported { mode: source_mode }
            }
        }
    }
}

pub(crate) fn report_inputs(args: &crate::support::args::Args) -> Result<MessagingReportInputs> {
    let value = args
        .value(0)
        .context("Messaging adapter initialization is required")?;
    let initialization = AdapterInitialization::parse(value, MESSAGING_SOURCE_MODES, "rest")?;
    let envelope: MessagingReportEnvelope = serde_json::from_value(value.clone())
        .context("Messaging adapter initialization must be an object")?;
    let source_mode = initialization.source_mode().to_owned();
    let rest_endpoint = if crate::source_routing::DeliverySourceMode::from_token(&source_mode)
        == crate::source_routing::DeliverySourceMode::LogoscoreCli
    {
        envelope.options.health_endpoint
    } else {
        initialization.input("rest_endpoint").map(ToOwned::to_owned)
    };
    Ok(MessagingReportInputs {
        source_mode,
        rest_endpoint,
        metrics_endpoint: initialization
            .input("metrics_endpoint")
            .map(ToOwned::to_owned),
        runtime_diagnostics_enabled: envelope.options.runtime_diagnostics_enabled,
        runtime_metrics_enabled: envelope.options.runtime_metrics_enabled,
    })
}

fn present(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

pub(crate) async fn module_report(
    module_transport: &SharedModuleTransport,
    transport: ModuleTransportKind,
    content_topic: Option<&str>,
    runtime_diagnostics_enabled: bool,
    runtime_metrics_enabled: bool,
    health_identity_required: bool,
) -> ModuleReport {
    crate::modules::delivery_report_with_identity_binding(
        module_transport,
        transport,
        content_topic,
        runtime_diagnostics_enabled,
        runtime_metrics_enabled,
        health_identity_required,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_routing::adapter::{
        ManagedNodeAction,
        contract_tests::{
            EndpointAdapterBehavior, assert_endpoint_adapter_contract, assert_layer_contract,
            assert_managed_lifecycle_behavior, assert_managed_module_contract,
        },
    };
    use anyhow::bail;

    #[test]
    fn messaging_adapters_satisfy_shared_seam_contract() {
        assert_layer_contract("messaging", MESSAGING_SOURCE_MODES);
    }

    #[test]
    fn messaging_managed_calls_satisfy_shared_contract() {
        assert_managed_module_contract(
            "messaging",
            managed_contract(),
            &[
                ManagedNodeAction::Initialize,
                ManagedNodeAction::Start,
                ManagedNodeAction::Stop,
            ],
        );
    }

    #[test]
    fn messaging_lifecycle_events_satisfy_shared_behavior_contract() -> Result<()> {
        assert_managed_lifecycle_behavior(
            "messaging",
            managed_contract(),
            ManagedNodeAction::Start,
            "nodeStarted",
            json!({ "arg0": true, "arg1": "started" }),
            true,
            "started",
        )
    }

    #[test]
    fn messaging_endpoint_adapters_satisfy_shared_behavior_contract() {
        assert_endpoint_adapter_contract(
            "messaging",
            MESSAGING_SOURCE_MODES,
            |mode, rest, metrics| match MessagingAdapter::select(mode, rest, metrics) {
                MessagingAdapter::Module { .. } => EndpointAdapterBehavior::Module {
                    module_id: module_id(),
                },
                MessagingAdapter::Rest {
                    endpoint,
                    metrics_endpoint,
                } => EndpointAdapterBehavior::Endpoint {
                    connection_type: AdapterConnectionType::Rest,
                    endpoint: endpoint.to_owned(),
                    metrics_endpoint: metrics_endpoint.map(ToOwned::to_owned),
                },
                MessagingAdapter::Metrics { endpoint } => EndpointAdapterBehavior::Endpoint {
                    connection_type: AdapterConnectionType::Metrics,
                    endpoint: endpoint.to_owned(),
                    metrics_endpoint: None,
                },
                MessagingAdapter::NetworkMonitor {
                    endpoint,
                    metrics_endpoint,
                } => EndpointAdapterBehavior::Endpoint {
                    connection_type: AdapterConnectionType::NetworkMonitor,
                    endpoint: endpoint.to_owned(),
                    metrics_endpoint: metrics_endpoint.map(ToOwned::to_owned),
                },
                MessagingAdapter::Unsupported { .. } => EndpointAdapterBehavior::Unsupported,
            },
        );
    }

    #[test]
    fn messaging_adapter_initializers_only_take_supported_inputs() {
        assert_eq!(
            MessagingAdapter::module(ModuleTransportKind::Module),
            MessagingAdapter::Module {
                transport: ModuleTransportKind::Module
            }
        );
        assert_eq!(
            MessagingAdapter::select("logoscore_cli", None, None),
            MessagingAdapter::Module {
                transport: ModuleTransportKind::LogoscoreCli
            }
        );
        assert_eq!(
            MessagingAdapter::metrics("http://metrics"),
            MessagingAdapter::Metrics {
                endpoint: "http://metrics"
            }
        );
        assert_eq!(
            MessagingAdapter::network_monitor("http://rest", None),
            MessagingAdapter::NetworkMonitor {
                endpoint: "http://rest",
                metrics_endpoint: None
            }
        );
    }

    #[test]
    fn messaging_module_adapters_do_not_advertise_store_queries() {
        let supports_store_query = |key: &str| {
            MESSAGING_SOURCE_MODES
                .iter()
                .find(|mode| mode.key == key)
                .is_some_and(|mode| mode.adapter.capabilities.contains(&"delivery.store.query"))
        };

        assert!(!supports_store_query("module"));
        assert!(!supports_store_query("logoscore_cli"));
        assert!(supports_store_query("rest"));
    }

    #[test]
    fn messaging_cli_health_endpoint_is_report_only() -> Result<()> {
        let cli = MESSAGING_SOURCE_MODES
            .iter()
            .find(|mode| mode.key == "logoscore_cli")
            .context("LogosCore CLI Delivery source policy is missing")?;

        if !cli.adapter.inputs.is_empty() {
            bail!("LogosCore CLI health endpoint leaked into module adapter inputs");
        }
        Ok(())
    }

    #[test]
    fn messaging_rest_adapter_does_not_advertise_module_lifecycle() -> Result<()> {
        let rest = MESSAGING_SOURCE_MODES
            .iter()
            .find(|mode| mode.key == "rest")
            .context("REST delivery source policy is missing")?;

        for lifecycle in ["delivery.node.start", "delivery.node.stop"] {
            if rest.adapter.capabilities.contains(&lifecycle) {
                bail!("REST delivery source overclaimed `{lifecycle}`");
            }
        }
        Ok(())
    }

    #[test]
    fn messaging_report_boundary_parses_compact_adapter_inputs() -> Result<()> {
        let module = crate::support::args::Args::new(json!([{
            "source_mode": "module",
            "inputs": {},
            "options": {
                "runtime_diagnostics_enabled": true,
                "runtime_metrics_enabled": true
            }
        }]))?;
        let metrics = crate::support::args::Args::new(json!([{
            "source_mode": "metrics",
            "inputs": { "metrics_endpoint": "http://metrics" }
        }]))?;
        let cli = crate::support::args::Args::new(json!([{
            "source_mode": "logoscore_cli",
            "inputs": {},
            "options": {
                "runtime_diagnostics_enabled": true,
                "health_endpoint": "http://delivery"
            }
        }]))?;

        if report_inputs(&module)?.rest_endpoint.is_some()
            || report_inputs(&module)?.metrics_endpoint.is_some()
            || !report_inputs(&module)?.runtime_diagnostics_enabled
            || !report_inputs(&module)?.runtime_metrics_enabled
            || report_inputs(&metrics)?.metrics_endpoint.as_deref() != Some("http://metrics")
            || report_inputs(&metrics)?.runtime_diagnostics_enabled
            || report_inputs(&metrics)?.runtime_metrics_enabled
            || report_inputs(&cli)?.rest_endpoint.as_deref() != Some("http://delivery")
            || !report_inputs(&cli)?.runtime_diagnostics_enabled
            || report_inputs(&cli)?.runtime_metrics_enabled
        {
            anyhow::bail!("compact Messaging report inputs were parsed incorrectly");
        }
        Ok(())
    }
}
