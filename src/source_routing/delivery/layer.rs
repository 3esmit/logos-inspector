use anyhow::{Context as _, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use reqwest::Method;
use serde_json::{Value, json};

use super::adapters::DELIVERY_MODULE;
use crate::{
    modules::ModuleReport,
    modules::logos_core::LogoscoreCliRuntime,
    source_routing::{
        DEFAULT_DELIVERY_METRICS_ENDPOINT, DEFAULT_DELIVERY_REST_ENDPOINT,
        adapter::{
            AdapterConnectionType, AdapterInitialization, AdapterInputPolicy,
            ManagedLifecycleOutcome, ManagedModuleCallSpec, ManagedNodeAction, SourceAdapterPolicy,
            SourceModePolicy,
        },
    },
};

#[must_use]
pub(crate) const fn module_id() -> &'static str {
    DELIVERY_MODULE
}

pub(crate) fn message_args(
    args: &crate::support::args::Args,
    action_label: &str,
) -> Result<Option<crate::source_routing::shared::module_bridge::ModuleCallArgs>> {
    super::adapters::delivery_message_args(args, action_label)
}

pub(crate) fn lifecycle_args(
    args: &crate::support::args::Args,
    action_label: &str,
) -> Result<Vec<Value>> {
    super::adapters::delivery_lifecycle_args(args, action_label)
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
        "preset": "logos.test",
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
    "delivery.topics.read",
    "delivery.store.query",
    "delivery.subscribe",
    "delivery.unsubscribe",
    "delivery.send",
    "delivery.node.start",
    "delivery.node.stop",
];
const REST_CAPABILITIES: &[&str] = &[
    "delivery.identity.read",
    "delivery.topics.read",
    "delivery.store.query",
    "delivery.subscribe",
    "delivery.unsubscribe",
    "delivery.send",
    "delivery.node.start",
    "delivery.node.stop",
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
        summary: "Use delivery_module through logoscore",
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
    Module,
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
}

impl<'a> MessagingAdapter<'a> {
    #[must_use]
    pub(crate) const fn module() -> Self {
        Self::Module
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
            crate::source_routing::DeliverySourceMode::Module => Self::module(),
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
    Ok(MessagingReportInputs {
        source_mode: initialization.source_mode().to_owned(),
        rest_endpoint: initialization.input("rest_endpoint").map(ToOwned::to_owned),
        metrics_endpoint: initialization
            .input("metrics_endpoint")
            .map(ToOwned::to_owned),
    })
}

fn present(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[must_use]
pub(crate) fn module_report(content_topic: Option<&str>) -> ModuleReport {
    crate::modules::delivery_report(content_topic)
}

pub(crate) async fn module_call(method: &'static str, args: Vec<Value>) -> Result<Value> {
    blocking_module_call("Messaging module call", move || {
        crate::source_routing::shared::module_bridge::call_value(DELIVERY_MODULE, method, &args)
    })
    .await
}

pub(crate) async fn module_dispatch(
    method: &'static str,
    args: Vec<Value>,
    context: Vec<(&'static str, String)>,
) -> Result<Value> {
    let value = module_call(method, args).await?;
    Ok(
        crate::source_routing::shared::module_bridge::dispatch_result(
            DELIVERY_MODULE,
            method,
            value,
            &context,
        ),
    )
}

pub(crate) async fn update_subscription(
    endpoint: &str,
    topic: &str,
    subscribe: bool,
) -> Result<Value> {
    let method = if subscribe {
        Method::POST
    } else {
        Method::DELETE
    };
    crate::source_routing::shared::http::rest_empty_request(
        method,
        endpoint,
        "/relay/v1/auto/subscriptions",
        Some(json!([topic])),
    )
    .await?;
    Ok(json!({
        "subscribed": subscribe,
        "contentTopic": topic,
        "endpoint": endpoint,
    }))
}

pub(crate) async fn send(endpoint: &str, topic: &str, payload: &str) -> Result<Value> {
    crate::source_routing::shared::http::rest_empty_request(
        Method::POST,
        endpoint,
        "/relay/v1/auto/messages",
        Some(json!({
            "contentTopic": topic,
            "payload": BASE64_STANDARD.encode(payload.as_bytes()),
        })),
    )
    .await?;
    Ok(json!({
        "sent": true,
        "contentTopic": topic,
        "bytes": payload.len(),
        "endpoint": endpoint,
    }))
}

pub(crate) async fn probe_value(endpoint: &str, path: &str) -> Result<Value> {
    let url = crate::source_routing::shared::http::rest_url(endpoint, path);
    let text = crate::source_routing::shared::http::raw_http_text_url(&url).await?;
    Ok(parse_probe_text(&text))
}

pub(crate) async fn probe_metrics(endpoint: &str) -> Result<String> {
    crate::source_routing::shared::http::raw_http_text_url(endpoint).await
}

pub(crate) async fn store_query(
    endpoint: &str,
    query: crate::source_routing::DeliveryStoreQuery<'_>,
) -> Result<(String, Value)> {
    let url = store_query_url(endpoint, query)?;
    let value = crate::source_routing::shared::http::raw_http_json_url(url.as_str()).await?;
    Ok((url.to_string(), value))
}

pub(crate) fn store_query_url(
    endpoint: &str,
    query: crate::source_routing::DeliveryStoreQuery<'_>,
) -> Result<url::Url> {
    super::adapters::delivery_store_query_url(endpoint, query)
}

async fn blocking_module_call<T, F>(label: &'static str, call: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(call)
        .await
        .with_context(|| format!("{label} worker failed"))?
}

fn parse_probe_text(text: &str) -> Value {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Value::Null;
    }
    serde_json::from_str(trimmed).unwrap_or_else(|_| Value::String(trimmed.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_routing::adapter::{
        ManagedNodeAction,
        contract_tests::{assert_layer_contract, assert_managed_module_contract},
    };

    #[test]
    fn messaging_adapters_satisfy_shared_seam_contract() {
        assert_layer_contract("messaging", MESSAGING_SOURCE_MODES);
    }

    #[test]
    fn messaging_managed_calls_satisfy_shared_contract() {
        assert_managed_module_contract(
            "messaging",
            module_id(),
            &[
                ManagedNodeAction::Initialize,
                ManagedNodeAction::Start,
                ManagedNodeAction::Stop,
            ],
            managed_call_spec,
        );
    }

    #[test]
    fn messaging_adapter_initializers_only_take_supported_inputs() {
        assert_eq!(MessagingAdapter::module(), MessagingAdapter::Module);
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
    fn messaging_report_boundary_parses_compact_adapter_inputs() -> Result<()> {
        let module = crate::support::args::Args::new(json!([{
            "source_mode": "module",
            "inputs": {}
        }]))?;
        let metrics = crate::support::args::Args::new(json!([{
            "source_mode": "metrics",
            "inputs": { "metrics_endpoint": "http://metrics" }
        }]))?;

        if report_inputs(&module)?.rest_endpoint.is_some()
            || report_inputs(&module)?.metrics_endpoint.is_some()
            || report_inputs(&metrics)?.metrics_endpoint.as_deref() != Some("http://metrics")
        {
            anyhow::bail!("compact Messaging report inputs were parsed incorrectly");
        }
        Ok(())
    }
}
