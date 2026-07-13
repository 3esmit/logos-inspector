use anyhow::{Context as _, Result};
use reqwest::{Method, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use super::adapters::STORAGE_MODULE;
use crate::{
    modules::ModuleReport,
    modules::logos_core::LogoscoreCliRuntime,
    source_routing::{
        DEFAULT_STORAGE_METRICS_ENDPOINT, DEFAULT_STORAGE_REST_ENDPOINT,
        adapter::{
            AdapterConnectionType, AdapterInitialization, AdapterInputPolicy,
            ManagedLifecycleOutcome, ManagedModuleCallSpec, ManagedNodeAction, SourceAdapterPolicy,
            SourceModePolicy,
        },
    },
    support::raw_source_transport::request_success,
};

#[must_use]
pub(crate) const fn module_id() -> &'static str {
    STORAGE_MODULE
}

pub(crate) fn is_module_source(args: &crate::support::args::Args) -> bool {
    super::adapters::is_storage_module_source(args)
}

pub(crate) fn operation_args(
    args: &crate::support::args::Args,
    uses_mutating_flag: bool,
    action_label: &str,
) -> Result<Option<crate::source_routing::shared::module_bridge::ModuleCallArgs>> {
    super::adapters::storage_args(args, uses_mutating_flag, action_label)
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
            "init",
            "init(QString)",
            vec![format!("@{config_path}")],
        )),
        ManagedNodeAction::Start => {
            Some(ManagedModuleCallSpec::new("start", "start()", Vec::new()))
        }
        ManagedNodeAction::Stop => Some(ManagedModuleCallSpec::new("stop", "stop()", Vec::new())),
        ManagedNodeAction::Destroy => Some(ManagedModuleCallSpec::new(
            "destroy",
            "destroy()",
            Vec::new(),
        )),
    }
}

#[must_use]
pub(crate) fn managed_config(data_dir: &str) -> Value {
    json!({
        "data-dir": data_dir,
        "log-level": "INFO",
        "nat": "none",
        "network": "logos.test",
    })
}

#[must_use]
pub(crate) const fn managed_lifecycle_event(action: ManagedNodeAction) -> Option<&'static str> {
    match action {
        ManagedNodeAction::Start => Some("storageStart"),
        ManagedNodeAction::Stop => Some("storageStop"),
        ManagedNodeAction::Initialize | ManagedNodeAction::Destroy => None,
    }
}

pub(crate) fn managed_lifecycle_outcome(
    payload: Option<&Value>,
) -> Result<ManagedLifecycleOutcome> {
    let payload = payload.context("storage lifecycle event has no payload")?;
    let payload = match payload {
        Value::String(text) => serde_json::from_str(text),
        value => serde_json::from_value(value.clone()),
    };
    let payload: StorageLifecyclePayload =
        payload.context("storage lifecycle event payload is not valid JSON")?;
    Ok(ManagedLifecycleOutcome {
        success: payload.success,
        detail: payload.message,
    })
}

#[derive(Debug, Deserialize)]
struct StorageLifecyclePayload {
    success: bool,
    #[serde(default)]
    message: String,
}

const REST_INPUTS: &[AdapterInputPolicy] = &[
    AdapterInputPolicy {
        key: "rest_endpoint",
        label: "REST URL",
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
    "storage.identity.read",
    "storage.space.read",
    "storage.manifests.read",
    "storage.content.exists",
    "storage.content.read_by_cid",
    "storage.content.upload",
    "storage.content.download_to_file",
    "storage.content.remove",
];
const REST_CAPABILITIES: &[&str] = &[
    "storage.identity.read",
    "storage.space.read",
    "storage.manifests.read",
    "storage.content.exists",
    "storage.content.read_by_cid",
    "storage.content.upload",
    "storage.backup.sync_read_by_cid",
    "storage.backup.sync_upload",
    "storage.rest.read_by_cid",
    "storage.rest.upload",
    "storage.content.download_to_file",
    "storage.content.remove",
    "storage.metrics.read",
];
const METRICS_CAPABILITIES: &[&str] = &["storage.metrics.read"];

pub(crate) const STORAGE_SOURCE_MODES: &[SourceModePolicy] = &[
    SourceModePolicy {
        key: "module",
        aliases: &["module", "basecamp", "basecamp-module", "basecamp module"],
        effective: "module",
        label_key: "storage_module",
        label: "Storage module",
        source_label: "Storage module",
        summary: "Use storage_module through logoscore",
        implemented: true,
        adapter: SourceAdapterPolicy {
            connector_id: STORAGE_MODULE,
            connection_type: AdapterConnectionType::Module,
            target: "module",
            module_id: Some(STORAGE_MODULE),
            inputs: &[],
            capabilities: MODULE_CAPABILITIES,
            supports_cid_probe: true,
            supports_mutating_diagnostics: true,
            capability_scopes: &["storage"],
            endpoint_role: None,
        },
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
        summary: "Inspect Storage through its REST API and optional metrics endpoint",
        implemented: true,
        adapter: SourceAdapterPolicy {
            connector_id: "direct_storage_rest",
            connection_type: AdapterConnectionType::Rest,
            target: "rest_endpoint",
            module_id: None,
            inputs: REST_INPUTS,
            capabilities: REST_CAPABILITIES,
            supports_cid_probe: true,
            supports_mutating_diagnostics: true,
            capability_scopes: &["storage"],
            endpoint_role: Some("storage_rest_url"),
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
            connector_id: "storage_metrics",
            connection_type: AdapterConnectionType::Metrics,
            target: "metrics_endpoint",
            module_id: None,
            inputs: METRICS_INPUTS,
            capabilities: METRICS_CAPABILITIES,
            supports_cid_probe: false,
            supports_mutating_diagnostics: false,
            capability_scopes: &["storage"],
            endpoint_role: Some("storage_metrics_url"),
        },
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StorageAdapter<'a> {
    Module,
    Rest {
        endpoint: &'a str,
        metrics_endpoint: Option<&'a str>,
    },
    Metrics {
        endpoint: &'a str,
    },
    Unsupported {
        mode: &'a str,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StorageReportInputs {
    pub(crate) source_mode: String,
    pub(crate) rest_endpoint: Option<String>,
    pub(crate) metrics_endpoint: Option<String>,
    pub(crate) cid: Option<String>,
    pub(crate) privileged_debug_enabled: bool,
}

#[derive(Debug, Default, Deserialize)]
struct StorageReportEnvelope {
    #[serde(default)]
    options: StorageReportOptions,
}

#[derive(Debug, Default, Deserialize)]
struct StorageReportOptions {
    #[serde(default)]
    cid: String,
    #[serde(default)]
    privileged_debug_enabled: bool,
}

impl<'a> StorageAdapter<'a> {
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
    pub(crate) fn select(
        source_mode: &'a str,
        rest_endpoint: Option<&'a str>,
        metrics_endpoint: Option<&'a str>,
    ) -> Self {
        match crate::source_routing::StorageSourceMode::from_token(source_mode) {
            crate::source_routing::StorageSourceMode::Module => Self::module(),
            crate::source_routing::StorageSourceMode::Rest => Self::rest(
                present(rest_endpoint).unwrap_or(DEFAULT_STORAGE_REST_ENDPOINT),
                present(metrics_endpoint),
            ),
            crate::source_routing::StorageSourceMode::Metrics => {
                Self::metrics(present(metrics_endpoint).unwrap_or(DEFAULT_STORAGE_METRICS_ENDPOINT))
            }
            crate::source_routing::StorageSourceMode::Unsupported => {
                Self::Unsupported { mode: source_mode }
            }
        }
    }
}

pub(crate) fn report_inputs(args: &crate::support::args::Args) -> Result<StorageReportInputs> {
    let value = args
        .value(0)
        .context("Storage adapter initialization is required")?;
    let initialization = AdapterInitialization::parse(value, STORAGE_SOURCE_MODES, "rest")?;
    let envelope: StorageReportEnvelope = serde_json::from_value(value.clone())
        .context("Storage adapter initialization must be an object")?;
    let StorageReportOptions {
        cid,
        privileged_debug_enabled,
    } = envelope.options;
    let cid = cid.trim().to_owned();
    Ok(StorageReportInputs {
        source_mode: initialization.source_mode().to_owned(),
        rest_endpoint: initialization.input("rest_endpoint").map(ToOwned::to_owned),
        metrics_endpoint: initialization
            .input("metrics_endpoint")
            .map(ToOwned::to_owned),
        cid: if cid.is_empty() { None } else { Some(cid) },
        privileged_debug_enabled,
    })
}

fn present(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[must_use]
pub(crate) fn module_report(cid: Option<&str>, privileged_debug_enabled: bool) -> ModuleReport {
    crate::modules::storage_report(cid, privileged_debug_enabled)
}

pub(crate) async fn module_call(method: &'static str, args: Vec<Value>) -> Result<Value> {
    blocking_module_call("Storage module call", move || {
        crate::source_routing::shared::module_bridge::call_value(STORAGE_MODULE, method, &args)
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
            STORAGE_MODULE,
            method,
            value,
            &context,
        ),
    )
}

pub(crate) async fn manifests(endpoint: &str) -> Result<Value> {
    crate::raw_http_json(endpoint, "/data").await
}

pub(crate) async fn manifest(endpoint: &str, cid: &str) -> Result<Value> {
    crate::raw_http_json(endpoint, &format!("/data/{cid}/network/manifest")).await
}

pub(crate) async fn exists(endpoint: &str, cid: &str) -> Result<Value> {
    crate::raw_http_json(endpoint, &format!("/data/{cid}/exists")).await
}

pub(crate) async fn probe_value(endpoint: &str, path: &str) -> Result<Value> {
    let url = crate::source_routing::shared::http::rest_url(endpoint, path);
    let text = crate::source_routing::shared::http::raw_http_text_url(&url).await?;
    Ok(parse_probe_text(&text))
}

pub(crate) async fn probe_metrics(endpoint: &str) -> Result<String> {
    crate::source_routing::shared::http::raw_http_text_url(endpoint).await
}

pub(crate) async fn fetch(endpoint: &str, cid: &str) -> Result<Value> {
    crate::source_routing::shared::http::rest_json_request(
        Method::POST,
        endpoint,
        &format!("/data/{cid}/network"),
        None,
    )
    .await
}

pub(crate) async fn upload(endpoint: &str, path: &str, block_size: u64) -> Result<Value> {
    super::adapters::storage_rest_upload(endpoint, path, block_size).await
}

pub(crate) async fn upload_bytes(
    endpoint: &str,
    filename: &str,
    bytes: &[u8],
    block_size: u64,
) -> Result<Value> {
    super::adapters::storage_rest_upload_bytes(endpoint, filename, bytes, block_size).await
}

pub(crate) async fn download_bytes(endpoint: &str, cid: &str, local_only: bool) -> Result<Vec<u8>> {
    super::adapters::storage_rest_download_bytes(endpoint, cid, local_only).await
}

pub(crate) async fn download_response(
    endpoint: &str,
    cid: &str,
    local_only: bool,
) -> Result<Response> {
    let route = if local_only {
        format!("/data/{cid}")
    } else {
        format!("/data/{cid}/network/stream")
    };
    let url = crate::source_routing::shared::http::rest_url(endpoint, &route);
    request_success(
        reqwest::Client::new().get(&url),
        &url,
        "storage download",
        "failed to read storage download error body",
    )
    .await
}

pub(crate) async fn remove(endpoint: &str, cid: &str) -> Result<Value> {
    crate::source_routing::shared::http::rest_empty_request(
        Method::DELETE,
        endpoint,
        &format!("/data/{cid}"),
        None,
    )
    .await?;
    Ok(json!({
        "removed": true,
        "cid": cid,
        "endpoint": endpoint,
    }))
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
    fn storage_adapters_satisfy_shared_seam_contract() {
        assert_layer_contract("storage", STORAGE_SOURCE_MODES);
    }

    #[test]
    fn storage_managed_calls_satisfy_shared_contract() {
        assert_managed_module_contract(
            "storage",
            module_id(),
            &[
                ManagedNodeAction::Initialize,
                ManagedNodeAction::Start,
                ManagedNodeAction::Stop,
                ManagedNodeAction::Destroy,
            ],
            managed_call_spec,
        );
    }

    #[test]
    fn storage_adapter_initializers_only_take_supported_inputs() {
        assert_eq!(StorageAdapter::module(), StorageAdapter::Module);
        assert_eq!(
            StorageAdapter::metrics("http://metrics"),
            StorageAdapter::Metrics {
                endpoint: "http://metrics"
            }
        );
        assert_eq!(
            StorageAdapter::rest("http://rest", Some("http://metrics")),
            StorageAdapter::Rest {
                endpoint: "http://rest",
                metrics_endpoint: Some("http://metrics")
            }
        );
    }

    #[test]
    fn storage_report_boundary_parses_compact_adapter_inputs() -> Result<()> {
        let module = crate::support::args::Args::new(json!([{
            "source_mode": "module",
            "inputs": {},
            "options": { "cid": "cid-a", "privileged_debug_enabled": true }
        }]))?;
        let metrics = crate::support::args::Args::new(json!([{
            "source_mode": "metrics",
            "inputs": { "metrics_endpoint": "http://metrics" }
        }]))?;

        if report_inputs(&module)?.rest_endpoint.is_some()
            || report_inputs(&module)?.cid.as_deref() != Some("cid-a")
            || report_inputs(&metrics)?.metrics_endpoint.as_deref() != Some("http://metrics")
            || report_inputs(&metrics)?.cid.is_some()
        {
            anyhow::bail!("compact Storage report inputs were parsed incorrectly");
        }
        Ok(())
    }
}
