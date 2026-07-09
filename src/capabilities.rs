use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityBuildMode {
    Basecamp,
    Standalone,
}

impl CapabilityBuildMode {
    #[must_use]
    pub fn from_prefers_basecamp(prefers_basecamp: bool) -> Self {
        if prefers_basecamp {
            Self::Basecamp
        } else {
            Self::Standalone
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Basecamp => "basecamp",
            Self::Standalone => "standalone",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityRegistryReport {
    pub schema_version: u8,
    pub build_mode: &'static str,
    pub selection_policy: &'static str,
    pub provider_types: Vec<CapabilityProviderTypeReport>,
    pub provider_instances: Vec<CapabilityProviderInstanceReport>,
    pub connector_scopes: Vec<CapabilityConnectorScopeReport>,
    pub capabilities: Vec<CapabilityReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityProviderTypeReport {
    pub key: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityProviderInstanceReport {
    pub id: &'static str,
    pub provider_type: &'static str,
    pub label: &'static str,
    pub module: Option<&'static str>,
    pub endpoint_role: Option<&'static str>,
    pub capabilities: &'static [&'static str],
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityConnectorScopeReport {
    pub owner: &'static str,
    pub scope: &'static str,
    pub setting_key: &'static str,
    pub capability_key: &'static str,
    pub default_connector: &'static str,
    pub persisted_auto: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityReport {
    pub key: &'static str,
    pub label: &'static str,
    pub status: String,
    pub default_connector: String,
    pub configured_connector: String,
    pub connector_provenance: String,
    pub provider_instance: String,
    pub sub_capabilities: &'static [&'static str],
    pub unavailable_sub_capabilities: Vec<String>,
    pub warnings: Vec<String>,
    pub compact_errors: Vec<String>,
}

#[must_use]
pub fn capability_registry_report(build_mode: CapabilityBuildMode) -> CapabilityRegistryReport {
    let inputs = CapabilityRuntimeInputs {
        provided: true,
        ..CapabilityRuntimeInputs::default()
    };
    capability_registry_report_with_inputs(build_mode, &inputs)
}

#[must_use]
pub fn capability_registry_report_with_value(
    build_mode: CapabilityBuildMode,
    value: Option<&Value>,
) -> CapabilityRegistryReport {
    let inputs = CapabilityRuntimeInputs::from_value(value);
    capability_registry_report_with_inputs(build_mode, &inputs)
}

fn capability_registry_report_with_inputs(
    build_mode: CapabilityBuildMode,
    inputs: &CapabilityRuntimeInputs,
) -> CapabilityRegistryReport {
    CapabilityRegistryReport {
        schema_version: 1,
        build_mode: build_mode.as_str(),
        selection_policy: "configured_connector_or_build_default",
        provider_types: provider_types().to_vec(),
        provider_instances: provider_instances().to_vec(),
        connector_scopes: connector_scopes(build_mode),
        capabilities: capabilities(build_mode, inputs),
    }
}

#[derive(Debug, Clone, Default)]
struct CapabilityRuntimeInputs {
    provided: bool,
    network_connector_config: Option<Value>,
    wallet_connector_config: Option<Value>,
    node_url: String,
    indexer_url: String,
    sequencer_url: String,
    storage_rest_url: String,
    messaging_rest_url: String,
    storage_mutating_diagnostics_enabled: bool,
    messaging_mutating_diagnostics_enabled: bool,
    wallet_profile_configured: bool,
    wallet_home_configured: bool,
    local_nodes_enabled: bool,
    local_devnet_enabled: bool,
    source_reports: Option<Value>,
    diagnostics_reports: Option<Value>,
}

impl CapabilityRuntimeInputs {
    fn from_value(value: Option<&Value>) -> Self {
        let Some(value) = value.filter(|value| value.is_object()) else {
            return Self::default();
        };
        Self {
            provided: true,
            network_connector_config: value.get("network_connector_config").cloned(),
            wallet_connector_config: value.get("wallet_connector_config").cloned(),
            node_url: string_input(value, "node_url"),
            indexer_url: string_input(value, "indexer_url"),
            sequencer_url: string_input(value, "sequencer_url"),
            storage_rest_url: string_input(value, "storage_rest_url"),
            messaging_rest_url: string_input(value, "messaging_rest_url"),
            storage_mutating_diagnostics_enabled: bool_input(
                value,
                "storage_mutating_diagnostics_enabled",
            ),
            messaging_mutating_diagnostics_enabled: bool_input(
                value,
                "messaging_mutating_diagnostics_enabled",
            ),
            wallet_profile_configured: bool_input(value, "wallet_profile_configured"),
            wallet_home_configured: bool_input(value, "wallet_home_configured"),
            local_nodes_enabled: bool_input(value, "local_nodes_enabled"),
            local_devnet_enabled: bool_input(value, "local_devnet_enabled"),
            source_reports: value.get("source_reports").cloned(),
            diagnostics_reports: value.get("diagnostics_reports").cloned(),
        }
    }

    fn connector_for(&self, build_mode: CapabilityBuildMode, scope: &str) -> ResolvedConnector {
        let default = default_connector(build_mode, scope);
        let owner = connector_owner(scope);
        let config = match owner {
            ConnectorOwner::WalletProfile => self.wallet_connector_config.as_ref(),
            ConnectorOwner::NetworkProfile => self.network_connector_config.as_ref(),
        };
        let Some(config) = config else {
            return ResolvedConnector::build_default(default);
        };
        let scopes = config
            .get("scopes")
            .filter(|value| value.is_object())
            .unwrap_or(config);
        let Some(entry) = scopes.get(scope).filter(|value| value.is_object()) else {
            return ResolvedConnector::build_default(default);
        };
        let id = first_string(
            entry,
            &[
                "connector_id",
                "connectorId",
                "id",
                "provider_instance",
                "providerInstance",
            ],
        )
        .unwrap_or_else(|| default.to_owned());
        let provenance = first_string(entry, &["provenance", "connector_provenance"])
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                if id == default {
                    "build_default".to_owned()
                } else if owner == ConnectorOwner::WalletProfile {
                    "wallet_profile".to_owned()
                } else {
                    "network_profile".to_owned()
                }
            });
        ResolvedConnector {
            id,
            provenance,
            endpoint: first_string(entry, &["endpoint", "url", "rest_endpoint", "rpc_endpoint"])
                .unwrap_or_default(),
        }
    }

    fn endpoint_for(&self, scope: &str, connector: &ResolvedConnector) -> String {
        if !connector.endpoint.is_empty() {
            return connector.endpoint.clone();
        }
        match scope {
            "l1" => self.node_url.clone(),
            "lez.indexer" => self.indexer_url.clone(),
            "lez.sequencer" => self.sequencer_url.clone(),
            "storage" => self.storage_rest_url.clone(),
            "delivery" => self.messaging_rest_url.clone(),
            _ => String::new(),
        }
    }

    fn source_report_for(&self, scope: &str) -> Option<&Value> {
        let reports = self.source_reports.as_ref()?.as_object()?;
        let keys: &[&str] = match scope {
            "l1" => &["l1", "blockchain", "node"],
            "lez.indexer" => &["lez.indexer", "indexer", "lez_indexer"],
            "lez.sequencer" => &["lez.sequencer", "sequencer", "execution", "lez_sequencer"],
            "storage" => &["storage", "storage_source"],
            "delivery" => &[
                "delivery",
                "delivery_source",
                "messaging",
                "messaging_source",
            ],
            _ => &[scope],
        };
        keys.iter()
            .find_map(|key| reports.get(*key).filter(|value| value.is_object()))
    }

    fn diagnostics_report(&self) -> Option<&Value> {
        self.diagnostics_reports
            .as_ref()
            .filter(|value| report_has_runtime_evidence(value))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectorOwner {
    NetworkProfile,
    WalletProfile,
}

fn connector_owner(scope: &str) -> ConnectorOwner {
    if scope.starts_with("wallet.") {
        ConnectorOwner::WalletProfile
    } else {
        ConnectorOwner::NetworkProfile
    }
}

#[derive(Debug, Clone)]
struct ResolvedConnector {
    id: String,
    provenance: String,
    endpoint: String,
}

impl ResolvedConnector {
    fn build_default(default: &str) -> Self {
        Self {
            id: default.to_owned(),
            provenance: "build_default".to_owned(),
            endpoint: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct CapabilityState {
    status: &'static str,
    unavailable_sub_capabilities: Vec<String>,
    warnings: Vec<String>,
    compact_errors: Vec<String>,
}

fn provider_types() -> &'static [CapabilityProviderTypeReport] {
    &[
        CapabilityProviderTypeReport {
            key: "composed",
            label: "Composed capability",
        },
        CapabilityProviderTypeReport {
            key: "unconfigured",
            label: "Unconfigured connector",
        },
        CapabilityProviderTypeReport {
            key: "module",
            label: "Basecamp module",
        },
        CapabilityProviderTypeReport {
            key: "direct_rpc",
            label: "Direct RPC endpoint",
        },
        CapabilityProviderTypeReport {
            key: "direct_rest",
            label: "Direct REST endpoint",
        },
        CapabilityProviderTypeReport {
            key: "local_control",
            label: "Local control",
        },
        CapabilityProviderTypeReport {
            key: "module_diagnostics",
            label: "Module diagnostics",
        },
    ]
}

fn provider_instances() -> &'static [CapabilityProviderInstanceReport] {
    &[
        CapabilityProviderInstanceReport {
            id: "composed_lez",
            provider_type: "composed",
            label: "LEZ composed capability",
            module: None,
            endpoint_role: None,
            capabilities: &["lez"],
        },
        CapabilityProviderInstanceReport {
            id: "composed_wallet",
            provider_type: "composed",
            label: "Wallet composed capability",
            module: None,
            endpoint_role: None,
            capabilities: &["wallet", "wallet.l1", "wallet.l2"],
        },
        CapabilityProviderInstanceReport {
            id: "unconfigured",
            provider_type: "unconfigured",
            label: "Unconfigured connector",
            module: None,
            endpoint_role: None,
            capabilities: &["wallet.l1", "wallet.l2"],
        },
        CapabilityProviderInstanceReport {
            id: "blockchain_module",
            provider_type: "module",
            label: "Blockchain module",
            module: Some("blockchain_module"),
            endpoint_role: None,
            capabilities: &["l1", "wallet.l1"],
        },
        CapabilityProviderInstanceReport {
            id: "lez_indexer_module",
            provider_type: "module",
            label: "LEZ Indexer module",
            module: Some("lez_indexer_module"),
            endpoint_role: None,
            capabilities: &["lez.indexer"],
        },
        CapabilityProviderInstanceReport {
            id: "storage_module",
            provider_type: "module",
            label: "Storage module",
            module: Some("storage_module"),
            endpoint_role: None,
            capabilities: &["storage"],
        },
        CapabilityProviderInstanceReport {
            id: "delivery_module",
            provider_type: "module",
            label: "Delivery module",
            module: Some("delivery_module"),
            endpoint_role: None,
            capabilities: &["delivery"],
        },
        CapabilityProviderInstanceReport {
            id: "lez_core",
            provider_type: "module",
            label: "LEZ core",
            module: Some("lez_core"),
            endpoint_role: None,
            capabilities: &["wallet.l2"],
        },
        CapabilityProviderInstanceReport {
            id: "direct_l1_rpc",
            provider_type: "direct_rpc",
            label: "Direct L1 RPC",
            module: None,
            endpoint_role: Some("node_url"),
            capabilities: &["l1"],
        },
        CapabilityProviderInstanceReport {
            id: "direct_indexer_rpc",
            provider_type: "direct_rpc",
            label: "Direct LEZ Indexer RPC",
            module: None,
            endpoint_role: Some("indexer_url"),
            capabilities: &["lez.indexer"],
        },
        CapabilityProviderInstanceReport {
            id: "direct_sequencer_rpc",
            provider_type: "direct_rpc",
            label: "Direct LEZ Sequencer RPC",
            module: None,
            endpoint_role: Some("sequencer_url"),
            capabilities: &["lez.sequencer"],
        },
        CapabilityProviderInstanceReport {
            id: "direct_storage_rest",
            provider_type: "direct_rest",
            label: "Direct Storage REST",
            module: None,
            endpoint_role: Some("storage_rest_url"),
            capabilities: &["storage"],
        },
        CapabilityProviderInstanceReport {
            id: "direct_delivery_rest",
            provider_type: "direct_rest",
            label: "Direct Delivery REST",
            module: None,
            endpoint_role: Some("messaging_rest_url"),
            capabilities: &["delivery"],
        },
        CapabilityProviderInstanceReport {
            id: "local_node_control",
            provider_type: "local_control",
            label: "Local node control",
            module: None,
            endpoint_role: Some("local_nodes"),
            capabilities: &["local_nodes"],
        },
        CapabilityProviderInstanceReport {
            id: "module_diagnostics_metrics",
            provider_type: "module_diagnostics",
            label: "Module diagnostics and metrics",
            module: None,
            endpoint_role: Some("diagnostics"),
            capabilities: &["diagnostics"],
        },
    ]
}

fn connector_scopes(build_mode: CapabilityBuildMode) -> Vec<CapabilityConnectorScopeReport> {
    [
        ("network_profile", "l1", "l1_connector", "l1"),
        (
            "network_profile",
            "lez.indexer",
            "lez_indexer_connector",
            "lez.indexer",
        ),
        (
            "network_profile",
            "lez.sequencer",
            "lez_sequencer_connector",
            "lez.sequencer",
        ),
        ("network_profile", "storage", "storage_connector", "storage"),
        (
            "network_profile",
            "delivery",
            "delivery_connector",
            "delivery",
        ),
        (
            "wallet_profile",
            "wallet.l1",
            "wallet_l1_connector",
            "wallet.l1",
        ),
        (
            "wallet_profile",
            "wallet.l2",
            "wallet_l2_connector",
            "wallet.l2",
        ),
        (
            "local_settings",
            "local_nodes",
            "local_nodes_enabled",
            "local_nodes",
        ),
    ]
    .into_iter()
    .map(
        |(owner, scope, setting_key, capability_key)| CapabilityConnectorScopeReport {
            owner,
            scope,
            setting_key,
            capability_key,
            default_connector: default_connector(build_mode, capability_key),
            persisted_auto: false,
        },
    )
    .collect()
}

fn capabilities(
    build_mode: CapabilityBuildMode,
    inputs: &CapabilityRuntimeInputs,
) -> Vec<CapabilityReport> {
    capability_specs()
        .iter()
        .map(|spec| {
            let default = default_connector(build_mode, spec.key);
            let connector = resolved_connector(build_mode, inputs, spec.key, default);
            let state = capability_state(build_mode, inputs, spec, &connector);
            CapabilityReport {
                key: spec.key,
                label: spec.label,
                status: state.status.to_owned(),
                default_connector: default.to_owned(),
                configured_connector: connector.id.clone(),
                connector_provenance: connector.provenance,
                provider_instance: connector.id,
                sub_capabilities: spec.sub_capabilities,
                unavailable_sub_capabilities: state.unavailable_sub_capabilities,
                warnings: state.warnings,
                compact_errors: state.compact_errors,
            }
        })
        .collect()
}

fn resolved_connector(
    build_mode: CapabilityBuildMode,
    inputs: &CapabilityRuntimeInputs,
    capability_key: &str,
    default: &str,
) -> ResolvedConnector {
    match capability_key {
        "l1" | "lez.indexer" | "lez.sequencer" | "storage" | "delivery" | "wallet.l1"
        | "wallet.l2" => inputs.connector_for(build_mode, capability_key),
        _ => ResolvedConnector::build_default(default),
    }
}

fn capability_state(
    build_mode: CapabilityBuildMode,
    inputs: &CapabilityRuntimeInputs,
    spec: &CapabilitySpec,
    connector: &ResolvedConnector,
) -> CapabilityState {
    if !inputs.provided {
        return loading_state(
            spec.sub_capabilities,
            format!("{} runtime inputs are required", spec.label),
        );
    }
    if !provider_instance_known(&connector.id) {
        return unavailable_state(
            spec.sub_capabilities,
            format!("connector `{}` is not registered", connector.id),
        );
    }
    if !provider_instance_supports(&connector.id, spec.key) {
        return unavailable_state(
            spec.sub_capabilities,
            format!(
                "connector `{}` does not provide capability `{}`",
                connector.id, spec.key
            ),
        );
    }
    if connector.id == "unconfigured" {
        return input_required_state(
            spec.sub_capabilities,
            format!("{} connector is not configured", spec.label),
        );
    }

    match spec.key {
        "l1" => endpoint_backed_state(inputs, "l1", connector, spec.sub_capabilities),
        "lez" => composed_lez_state(build_mode, inputs, spec.sub_capabilities),
        "lez.indexer" => {
            endpoint_backed_state(inputs, "lez.indexer", connector, spec.sub_capabilities)
        }
        "lez.sequencer" => {
            endpoint_backed_state(inputs, "lez.sequencer", connector, spec.sub_capabilities)
        }
        "storage" => storage_state(inputs, connector, spec.sub_capabilities),
        "delivery" => delivery_state(inputs, connector, spec.sub_capabilities),
        "wallet" => wallet_state(inputs, spec.sub_capabilities),
        "wallet.l1" | "wallet.l2" => wallet_state(inputs, spec.sub_capabilities),
        "local_nodes" => local_nodes_state(inputs, spec.sub_capabilities),
        "diagnostics" => diagnostics_state(inputs, spec.sub_capabilities),
        _ => available_state(),
    }
}

fn available_state() -> CapabilityState {
    CapabilityState {
        status: "available",
        unavailable_sub_capabilities: Vec::new(),
        warnings: Vec::new(),
        compact_errors: Vec::new(),
    }
}

fn loading_state(sub_capabilities: &[&str], detail: String) -> CapabilityState {
    CapabilityState {
        status: "loading",
        unavailable_sub_capabilities: all_sub_capabilities(sub_capabilities),
        warnings: Vec::new(),
        compact_errors: vec![detail],
    }
}

fn source_report_state(
    inputs: &CapabilityRuntimeInputs,
    scope: &str,
    label: &str,
    sub_capabilities: &[&str],
) -> CapabilityState {
    let Some(report) = inputs.source_report_for(scope) else {
        return loading_state(
            sub_capabilities,
            format!("{label} provider probe has not run"),
        );
    };
    let explicit_unavailable = string_list(report.get("unavailable_sub_capabilities"));
    let compact_error = source_report_error(report);
    let health = report.get("health").filter(|value| value.is_object());
    let ready = health
        .and_then(|value| value.get("ready"))
        .and_then(Value::as_bool);
    let reachable = health
        .and_then(|value| value.get("reachable"))
        .and_then(Value::as_bool);
    let status = health
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if ready == Some(true) || status == "healthy" || status == "ready" {
        return state_from_unavailable(
            sub_capabilities,
            explicit_unavailable,
            Vec::new(),
            Vec::new(),
        );
    }
    if reachable == Some(true) || status == "degraded" {
        let unavailable = if explicit_unavailable.is_empty() {
            all_sub_capabilities(sub_capabilities)
        } else {
            explicit_unavailable
        };
        return state_from_unavailable(
            sub_capabilities,
            unavailable,
            compact_error.clone(),
            compact_error,
        );
    }
    if health.is_some() || report_has_probe_failure(report) {
        return unavailable_state(
            sub_capabilities,
            compact_error
                .first()
                .cloned()
                .unwrap_or_else(|| format!("{label} provider probe failed")),
        );
    }
    loading_state(
        sub_capabilities,
        format!("{label} provider probe has no health result"),
    )
}

fn report_has_probe_failure(report: &Value) -> bool {
    ["probe_facts", "probes"].iter().any(|field| {
        report
            .get(*field)
            .and_then(Value::as_array)
            .is_some_and(|rows| {
                rows.iter()
                    .any(|row| row.get("ok").and_then(Value::as_bool) == Some(false))
            })
    })
}

fn report_has_runtime_evidence(value: &Value) -> bool {
    if !value.is_object() {
        return false;
    }
    if value.get("health").is_some_and(|health| health.is_object())
        || value
            .get("unavailable_sub_capabilities")
            .is_some_and(Value::is_array)
        || value.get("last_known").is_some_and(value_has_content)
        || report_has_probe_failure(value)
    {
        return true;
    }
    value.as_object().is_some_and(|object| {
        object
            .values()
            .any(|item| item.is_object() && report_has_runtime_evidence(item))
    })
}

fn value_has_content(value: &Value) -> bool {
    match value {
        Value::String(text) => !text.trim().is_empty(),
        Value::Array(values) => values.iter().any(value_has_content),
        Value::Object(values) => values.values().any(value_has_content),
        _ => false,
    }
}

fn source_report_error(report: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    if let Some(health) = report.get("health").filter(|value| value.is_object()) {
        push_error_text(&mut errors, health.get("detail"));
        push_error_text(&mut errors, health.get("summary"));
    }
    for field in ["probe_facts", "probes"] {
        if let Some(rows) = report.get(field).and_then(Value::as_array) {
            for row in rows {
                if row.get("ok").and_then(Value::as_bool) == Some(false) {
                    push_error_text(&mut errors, row.get("error"));
                }
            }
        }
    }
    dedup_strings(errors)
}

fn push_error_text(errors: &mut Vec<String>, value: Option<&Value>) {
    let Some(text) = value.and_then(Value::as_str).map(str::trim) else {
        return;
    };
    if !text.is_empty() {
        errors.push(text.to_owned());
    }
}

fn merge_state_constraints(
    mut state: CapabilityState,
    unavailable: Vec<String>,
    warnings: Vec<String>,
    compact_errors: Vec<String>,
) -> CapabilityState {
    append_unique(&mut state.unavailable_sub_capabilities, unavailable);
    append_unique(&mut state.warnings, warnings);
    append_unique(&mut state.compact_errors, compact_errors);
    if state.status == "available" && !state.unavailable_sub_capabilities.is_empty() {
        state.status = "degraded";
    }
    state
}

fn endpoint_backed_state(
    inputs: &CapabilityRuntimeInputs,
    scope: &str,
    connector: &ResolvedConnector,
    sub_capabilities: &[&str],
) -> CapabilityState {
    let requires_endpoint = !connector.id.ends_with("_module") && connector.id != "lez_core";
    if requires_endpoint && inputs.endpoint_for(scope, connector).is_empty() {
        return input_required_state(
            sub_capabilities,
            format!("{} endpoint is required", scope_label(scope)),
        );
    }
    source_report_state(inputs, scope, scope_label(scope), sub_capabilities)
}

fn composed_lez_state(
    build_mode: CapabilityBuildMode,
    inputs: &CapabilityRuntimeInputs,
    sub_capabilities: &[&str],
) -> CapabilityState {
    let indexer = inputs.connector_for(build_mode, "lez.indexer");
    let sequencer = inputs.connector_for(build_mode, "lez.sequencer");
    let indexer_caps = prefixed_sub_capabilities(sub_capabilities, "lez.indexer.");
    let sequencer_caps = prefixed_sub_capabilities(sub_capabilities, "lez.sequencer.");
    let indexer_state = connector_capability_precheck(&indexer, "lez.indexer", &indexer_caps)
        .unwrap_or_else(|| endpoint_backed_state(inputs, "lez.indexer", &indexer, &indexer_caps));
    let sequencer_state =
        connector_capability_precheck(&sequencer, "lez.sequencer", &sequencer_caps).unwrap_or_else(
            || endpoint_backed_state(inputs, "lez.sequencer", &sequencer, &sequencer_caps),
        );
    let indexer_ready = capability_state_usable(&indexer_state);
    let sequencer_ready = capability_state_usable(&sequencer_state);

    let mut unavailable = Vec::new();
    for capability in sub_capabilities {
        let indexer_unavailable = capability.starts_with("lez.indexer.")
            && (!indexer_ready || state_marks_unavailable(&indexer_state, capability));
        let sequencer_unavailable = capability.starts_with("lez.sequencer.")
            && (!sequencer_ready || state_marks_unavailable(&sequencer_state, capability));
        let target_resolution_unavailable =
            *capability == "lez.target_resolution" && !indexer_ready && !sequencer_ready;
        if indexer_unavailable || sequencer_unavailable || target_resolution_unavailable {
            unavailable.push((*capability).to_owned());
        }
    }

    let mut warnings = Vec::new();
    append_unique(&mut warnings, indexer_state.warnings);
    append_unique(&mut warnings, sequencer_state.warnings);
    let mut compact_errors = Vec::new();
    append_unique(&mut compact_errors, indexer_state.compact_errors);
    append_unique(&mut compact_errors, sequencer_state.compact_errors);
    state_from_unavailable(sub_capabilities, unavailable, warnings, compact_errors)
}

fn storage_state(
    inputs: &CapabilityRuntimeInputs,
    connector: &ResolvedConnector,
    sub_capabilities: &[&str],
) -> CapabilityState {
    match connector.id.as_str() {
        "storage_module" => {
            let state = source_report_state(inputs, "storage", "Storage", sub_capabilities);
            let unavailable = vec![
                "storage.rest.read_by_cid".to_owned(),
                "storage.rest.upload".to_owned(),
                "storage.backup.sync_read_by_cid".to_owned(),
                "storage.backup.sync_upload".to_owned(),
            ];
            merge_state_constraints(
                state,
                unavailable,
                vec![
                    "Storage module does not support synchronous backup CID read/upload paths"
                        .to_owned(),
                ],
                Vec::new(),
            )
        }
        "direct_storage_rest" => {
            if inputs.endpoint_for("storage", connector).is_empty() {
                return input_required_state(
                    sub_capabilities,
                    "storage REST endpoint is required".to_owned(),
                );
            }
            let state = source_report_state(inputs, "storage", "Storage", sub_capabilities);
            let unavailable = if inputs.storage_mutating_diagnostics_enabled {
                Vec::new()
            } else {
                vec![
                    "storage.content.upload".to_owned(),
                    "storage.rest.upload".to_owned(),
                    "storage.content.download_to_file".to_owned(),
                    "storage.content.remove".to_owned(),
                ]
            };
            let warnings = if unavailable.is_empty() {
                Vec::new()
            } else {
                vec!["Storage mutating diagnostics are disabled".to_owned()]
            };
            merge_state_constraints(state, unavailable, warnings, Vec::new())
        }
        _ => unavailable_state(
            sub_capabilities,
            format!("connector `{}` does not provide Storage", connector.id),
        ),
    }
}

fn delivery_state(
    inputs: &CapabilityRuntimeInputs,
    connector: &ResolvedConnector,
    sub_capabilities: &[&str],
) -> CapabilityState {
    match connector.id.as_str() {
        "delivery_module" => source_report_state(inputs, "delivery", "Delivery", sub_capabilities),
        "direct_delivery_rest" => {
            if inputs.endpoint_for("delivery", connector).is_empty() {
                return input_required_state(
                    sub_capabilities,
                    "delivery REST endpoint is required".to_owned(),
                );
            }
            let state = source_report_state(inputs, "delivery", "Delivery", sub_capabilities);
            let unavailable = if inputs.messaging_mutating_diagnostics_enabled {
                Vec::new()
            } else {
                vec![
                    "delivery.subscribe".to_owned(),
                    "delivery.unsubscribe".to_owned(),
                    "delivery.send".to_owned(),
                    "delivery.node.start".to_owned(),
                    "delivery.node.stop".to_owned(),
                ]
            };
            let warnings = if unavailable.is_empty() {
                Vec::new()
            } else {
                vec!["Delivery mutating diagnostics are disabled".to_owned()]
            };
            merge_state_constraints(state, unavailable, warnings, Vec::new())
        }
        _ => unavailable_state(
            sub_capabilities,
            format!("connector `{}` does not provide Delivery", connector.id),
        ),
    }
}

fn diagnostics_state(
    inputs: &CapabilityRuntimeInputs,
    sub_capabilities: &[&str],
) -> CapabilityState {
    let Some(report) = inputs.diagnostics_report() else {
        return loading_state(
            sub_capabilities,
            "Diagnostics runtime evidence has not loaded".to_owned(),
        );
    };
    let mut unavailable = all_sub_capabilities(sub_capabilities);
    let mut warnings = string_list(report.get("warnings"));
    let mut compact_errors = source_report_error(report);
    mark_diagnostics_runtime_evidence(report, &mut unavailable);
    append_unique(
        &mut unavailable,
        string_list(report.get("unavailable_sub_capabilities")),
    );
    append_diagnostics_report_constraints(
        report,
        &mut unavailable,
        &mut warnings,
        &mut compact_errors,
    );
    let mut state = state_from_unavailable(sub_capabilities, unavailable, warnings, compact_errors);
    if state.status == "unavailable" {
        state.status = "degraded";
    }
    state
}

fn mark_diagnostics_runtime_evidence(report: &Value, unavailable: &mut Vec<String>) {
    if report
        .get("module_reports")
        .and_then(Value::as_object)
        .is_some_and(|module_reports| {
            module_reports
                .values()
                .any(|report| report.is_object() && report_has_runtime_evidence(report))
        })
    {
        for key in [
            "diagnostics.modules.status.read",
            "diagnostics.modules.info.read",
            "diagnostics.modules.metrics.read",
        ] {
            remove_unavailable(unavailable, key);
        }
    }

    if let Some(source_reports) = report.get("source_reports").and_then(Value::as_object) {
        for (key, source_report) in source_reports {
            if !source_report.is_object() || !report_has_runtime_evidence(source_report) {
                continue;
            }
            let Some(sub_capability) = diagnostics_source_sub_capability(key) else {
                continue;
            };
            remove_unavailable(unavailable, sub_capability);
            remove_unavailable(unavailable, "diagnostics.provider.probe");
        }
    }
}

fn append_diagnostics_report_constraints(
    report: &Value,
    unavailable: &mut Vec<String>,
    warnings: &mut Vec<String>,
    compact_errors: &mut Vec<String>,
) {
    if let Some(module_reports) = report.get("module_reports").and_then(Value::as_object) {
        for report in module_reports.values().filter(|value| value.is_object()) {
            if diagnostics_nested_report_unavailable(report) {
                append_unique(
                    unavailable,
                    vec![
                        "diagnostics.modules.status.read".to_owned(),
                        "diagnostics.modules.info.read".to_owned(),
                        "diagnostics.modules.metrics.read".to_owned(),
                    ],
                );
                append_unique(
                    warnings,
                    vec!["Module diagnostics report is unavailable".to_owned()],
                );
                append_unique(compact_errors, diagnostics_nested_report_errors(report));
            }
        }
    }

    if let Some(source_reports) = report.get("source_reports").and_then(Value::as_object) {
        for (key, source_report) in source_reports {
            if !source_report.is_object() || !diagnostics_nested_report_unavailable(source_report) {
                continue;
            }
            let Some(sub_capability) = diagnostics_source_sub_capability(key) else {
                continue;
            };
            append_unique(unavailable, vec![sub_capability.to_owned()]);
            append_unique(
                warnings,
                vec![format!(
                    "{} diagnostics report is unavailable",
                    diagnostics_source_label(key)
                )],
            );
            append_unique(
                compact_errors,
                diagnostics_nested_report_errors(source_report),
            );
        }
    }

    if let Some(last_known) = report.get("last_known").and_then(Value::as_object) {
        for (key, value) in last_known {
            let Some(detail) = value
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
            else {
                continue;
            };
            let Some(sub_capability) = diagnostics_last_known_sub_capability(key) else {
                continue;
            };
            append_unique(unavailable, vec![sub_capability.to_owned()]);
            append_unique(
                warnings,
                vec![format!(
                    "{} diagnostics are based on last-known error state",
                    diagnostics_source_label(key)
                )],
            );
            append_unique(compact_errors, vec![detail.to_owned()]);
        }
    }
}

fn diagnostics_nested_report_unavailable(report: &Value) -> bool {
    !string_list(report.get("unavailable_sub_capabilities")).is_empty()
        || report_has_probe_failure(report)
        || module_info_failed(report)
        || report_health_unavailable(report)
}

fn diagnostics_nested_report_errors(report: &Value) -> Vec<String> {
    let mut errors = source_report_error(report);
    if let Some(module_info) = report.get("module_info").filter(|value| value.is_object()) {
        push_error_text(&mut errors, module_info.get("error"));
    }
    dedup_strings(errors)
}

fn module_info_failed(report: &Value) -> bool {
    report
        .get("module_info")
        .filter(|value| value.is_object())
        .and_then(|module_info| module_info.get("ok"))
        .and_then(Value::as_bool)
        == Some(false)
}

fn report_health_unavailable(report: &Value) -> bool {
    let Some(health) = report.get("health").filter(|value| value.is_object()) else {
        return false;
    };
    if health.get("ready").and_then(Value::as_bool) == Some(false) {
        return true;
    }
    let status = health
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        status.as_str(),
        "degraded" | "error" | "failed" | "unavailable" | "unsupported"
    )
}

fn diagnostics_source_sub_capability(key: &str) -> Option<&'static str> {
    match key {
        "l1" | "blockchain" | "node" => Some("diagnostics.l1.read"),
        "lez.indexer" | "indexer" | "lez_indexer" => Some("diagnostics.lez.indexer.read"),
        "lez.sequencer" | "sequencer" | "execution" | "lez_sequencer" => {
            Some("diagnostics.lez.sequencer.read")
        }
        "storage" | "storage_source" => Some("diagnostics.storage.read"),
        "delivery" | "delivery_source" | "messaging" | "messaging_source" => {
            Some("diagnostics.delivery.read")
        }
        _ => None,
    }
}

fn diagnostics_last_known_sub_capability(key: &str) -> Option<&'static str> {
    match key {
        "wallet" => Some("diagnostics.wallet.read"),
        "local_nodes" | "localNodes" => Some("diagnostics.local_nodes.read"),
        _ => diagnostics_source_sub_capability(key),
    }
}

fn diagnostics_source_label(key: &str) -> &'static str {
    match key {
        "l1" | "blockchain" | "node" => "L1",
        "lez.indexer" | "indexer" | "lez_indexer" => "LEZ Indexer",
        "lez.sequencer" | "sequencer" | "execution" | "lez_sequencer" => "LEZ Sequencer",
        "storage" | "storage_source" => "Storage",
        "delivery" | "delivery_source" | "messaging" | "messaging_source" => "Delivery",
        "wallet" => "Wallet",
        "local_nodes" | "localNodes" => "Local Nodes",
        _ => "Provider",
    }
}

fn wallet_state(inputs: &CapabilityRuntimeInputs, sub_capabilities: &[&str]) -> CapabilityState {
    if !inputs.wallet_profile_configured {
        return input_required_state(sub_capabilities, "wallet profile is required".to_owned());
    }
    if inputs.wallet_home_configured {
        return available_state();
    }

    let unavailable: Vec<String> = sub_capabilities
        .iter()
        .filter(|capability| wallet_sub_capability_needs_home(capability))
        .map(|capability| (*capability).to_owned())
        .collect();
    state_from_unavailable(
        sub_capabilities,
        unavailable,
        vec!["Wallet home is required for signing and mutating wallet actions".to_owned()],
        Vec::new(),
    )
}

fn local_nodes_state(
    inputs: &CapabilityRuntimeInputs,
    sub_capabilities: &[&str],
) -> CapabilityState {
    if !inputs.local_nodes_enabled {
        return input_required_state(sub_capabilities, "local nodes are not enabled".to_owned());
    }
    let unavailable = if inputs.local_devnet_enabled {
        Vec::new()
    } else {
        vec!["local_nodes.sequencer.control".to_owned()]
    };
    let warnings = if unavailable.is_empty() {
        Vec::new()
    } else {
        vec!["Local devnet is required for sequencer control".to_owned()]
    };
    state_from_unavailable(sub_capabilities, unavailable, warnings, Vec::new())
}

fn wallet_sub_capability_needs_home(capability: &str) -> bool {
    capability.ends_with(".accounts.create")
        || capability.ends_with(".sign")
        || capability.ends_with(".submit")
        || capability.ends_with(".channels.action")
        || capability.ends_with(".private_sync")
        || capability.ends_with(".program.deploy")
        || capability == "wallet.command.run"
}

fn prefixed_sub_capabilities<'a>(sub_capabilities: &[&'a str], prefix: &str) -> Vec<&'a str> {
    sub_capabilities
        .iter()
        .copied()
        .filter(|capability| capability.starts_with(prefix))
        .collect()
}

fn capability_state_usable(state: &CapabilityState) -> bool {
    matches!(state.status, "available" | "degraded")
}

fn state_marks_unavailable(state: &CapabilityState, capability: &str) -> bool {
    state
        .unavailable_sub_capabilities
        .iter()
        .any(|unavailable| unavailable == capability)
}

fn state_from_unavailable(
    sub_capabilities: &[&str],
    unavailable_sub_capabilities: Vec<String>,
    warnings: Vec<String>,
    compact_errors: Vec<String>,
) -> CapabilityState {
    let status = if unavailable_sub_capabilities.is_empty() {
        "available"
    } else if unavailable_sub_capabilities.len() >= sub_capabilities.len() {
        "unavailable"
    } else {
        "degraded"
    };
    CapabilityState {
        status,
        unavailable_sub_capabilities,
        warnings,
        compact_errors,
    }
}

fn input_required_state(sub_capabilities: &[&str], error: String) -> CapabilityState {
    CapabilityState {
        status: "input_required",
        unavailable_sub_capabilities: all_sub_capabilities(sub_capabilities),
        warnings: Vec::new(),
        compact_errors: vec![error],
    }
}

fn unavailable_state(sub_capabilities: &[&str], error: String) -> CapabilityState {
    CapabilityState {
        status: "unavailable",
        unavailable_sub_capabilities: all_sub_capabilities(sub_capabilities),
        warnings: Vec::new(),
        compact_errors: vec![error],
    }
}

fn all_sub_capabilities(sub_capabilities: &[&str]) -> Vec<String> {
    sub_capabilities
        .iter()
        .map(|capability| (*capability).to_owned())
        .collect()
}

fn connector_capability_precheck(
    connector: &ResolvedConnector,
    capability_key: &str,
    sub_capabilities: &[&str],
) -> Option<CapabilityState> {
    if !provider_instance_known(&connector.id) {
        return Some(unavailable_state(
            sub_capabilities,
            format!("connector `{}` is not registered", connector.id),
        ));
    }
    if !provider_instance_supports(&connector.id, capability_key) {
        return Some(unavailable_state(
            sub_capabilities,
            format!(
                "connector `{}` does not provide capability `{capability_key}`",
                connector.id
            ),
        ));
    }
    if connector.id == "unconfigured" {
        return Some(input_required_state(
            sub_capabilities,
            format!("{capability_key} connector is not configured"),
        ));
    }
    None
}

fn provider_instance_known(connector: &str) -> bool {
    provider_instances()
        .iter()
        .any(|provider| provider.id == connector)
}

fn provider_instance_supports(connector: &str, capability_key: &str) -> bool {
    provider_instances()
        .iter()
        .any(|provider| provider.id == connector && provider.capabilities.contains(&capability_key))
}

fn remove_unavailable(target: &mut Vec<String>, key: &str) {
    target.retain(|value| value != key);
}

fn scope_label(scope: &str) -> &'static str {
    match scope {
        "l1" => "L1 RPC",
        "lez.indexer" => "LEZ indexer RPC",
        "lez.sequencer" => "LEZ sequencer RPC",
        "storage" => "storage REST",
        "delivery" => "delivery REST",
        _ => "capability",
    }
}

fn string_input(value: &Value, key: &str) -> String {
    value
        .get(key)
        .or_else(|| value.get("source").and_then(|source| source.get(key)))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
        .to_owned()
}

fn bool_input(value: &Value, key: &str) -> bool {
    value
        .get(key)
        .or_else(|| value.get("source").and_then(|source| source.get(key)))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn append_unique(target: &mut Vec<String>, incoming: Vec<String>) {
    for value in incoming {
        if !value.is_empty() && !target.iter().any(|current| current == &value) {
            target.push(value);
        }
    }
}

fn dedup_strings(values: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    append_unique(&mut result, values);
    result
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    })
}

fn default_connector(build_mode: CapabilityBuildMode, capability_key: &str) -> &'static str {
    match (build_mode, capability_key) {
        (CapabilityBuildMode::Basecamp, "l1" | "wallet.l1") => "blockchain_module",
        (CapabilityBuildMode::Basecamp, "lez.indexer") => "lez_indexer_module",
        (CapabilityBuildMode::Basecamp, "storage") => "storage_module",
        (CapabilityBuildMode::Basecamp, "delivery") => "delivery_module",
        (CapabilityBuildMode::Basecamp, "wallet.l2") => "lez_core",
        (_, "lez") => "composed_lez",
        (_, "wallet") => "composed_wallet",
        (_, "l1") => "direct_l1_rpc",
        (_, "lez.indexer") => "direct_indexer_rpc",
        (_, "lez.sequencer") => "direct_sequencer_rpc",
        (_, "storage") => "direct_storage_rest",
        (_, "delivery") => "direct_delivery_rest",
        (_, "wallet.l1" | "wallet.l2") => "composed_wallet",
        (_, "local_nodes") => "local_node_control",
        (_, "diagnostics") => "module_diagnostics_metrics",
        _ => "unconfigured",
    }
}

#[derive(Debug, Clone, Copy)]
struct CapabilitySpec {
    key: &'static str,
    label: &'static str,
    sub_capabilities: &'static [&'static str],
}

fn capability_specs() -> &'static [CapabilitySpec] {
    &[
        CapabilitySpec {
            key: "l1",
            label: "L1 inspection",
            sub_capabilities: &[
                "l1.blocks.read",
                "l1.transactions.read",
                "l1.channels.read",
                "l1.wallet_balance.read",
                "l1.live_blocks.observe",
            ],
        },
        CapabilitySpec {
            key: "lez",
            label: "LEZ inspection",
            sub_capabilities: &[
                "lez.indexer.blocks.finalized.read",
                "lez.indexer.transactions.finalized.read",
                "lez.indexer.account_history.read",
                "lez.indexer.transfers.read",
                "lez.sequencer.health",
                "lez.sequencer.blocks.pending.read",
                "lez.sequencer.transactions.pending.read",
                "lez.sequencer.transactions.trace",
                "lez.sequencer.accounts.read",
                "lez.sequencer.programs.read",
                "lez.target_resolution",
            ],
        },
        CapabilitySpec {
            key: "lez.indexer",
            label: "LEZ Indexer",
            sub_capabilities: &[
                "lez.indexer.blocks.finalized.read",
                "lez.indexer.transactions.finalized.read",
                "lez.indexer.account_history.read",
                "lez.indexer.transfers.read",
                "lez.target_resolution",
            ],
        },
        CapabilitySpec {
            key: "lez.sequencer",
            label: "LEZ Sequencer",
            sub_capabilities: &[
                "lez.sequencer.health",
                "lez.sequencer.blocks.pending.read",
                "lez.sequencer.transactions.pending.read",
                "lez.sequencer.transactions.trace",
                "lez.sequencer.accounts.read",
                "lez.sequencer.programs.read",
                "lez.target_resolution",
            ],
        },
        CapabilitySpec {
            key: "storage",
            label: "Storage",
            sub_capabilities: &[
                "storage.identity.read",
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
            ],
        },
        CapabilitySpec {
            key: "delivery",
            label: "Delivery",
            sub_capabilities: &[
                "delivery.identity.read",
                "delivery.topics.read",
                "delivery.store.query",
                "delivery.subscribe",
                "delivery.unsubscribe",
                "delivery.send",
                "delivery.node.start",
                "delivery.node.stop",
                "delivery.network_monitor.read",
            ],
        },
        CapabilitySpec {
            key: "wallet",
            label: "Wallet",
            sub_capabilities: &[
                "wallet.l1.profile.read",
                "wallet.l1.accounts.read",
                "wallet.l1.accounts.create",
                "wallet.l1.sign",
                "wallet.l1.submit",
                "wallet.l1.channels.action",
                "wallet.l2.profile.read",
                "wallet.l2.accounts.read",
                "wallet.l2.private_sync",
                "wallet.l2.program.deploy",
                "wallet.l2.instruction.preview",
                "wallet.l2.instruction.submit",
                "wallet.command.run",
            ],
        },
        CapabilitySpec {
            key: "wallet.l1",
            label: "L1 Wallet",
            sub_capabilities: &[
                "wallet.l1.profile.read",
                "wallet.l1.accounts.read",
                "wallet.l1.accounts.create",
                "wallet.l1.sign",
                "wallet.l1.submit",
                "wallet.l1.channels.action",
                "wallet.command.run",
            ],
        },
        CapabilitySpec {
            key: "wallet.l2",
            label: "L2 Wallet",
            sub_capabilities: &[
                "wallet.l2.profile.read",
                "wallet.l2.accounts.read",
                "wallet.l2.private_sync",
                "wallet.l2.program.deploy",
                "wallet.l2.instruction.preview",
                "wallet.l2.instruction.submit",
                "wallet.command.run",
            ],
        },
        CapabilitySpec {
            key: "local_nodes",
            label: "Local Nodes",
            sub_capabilities: &[
                "local_nodes.devnet.read",
                "local_nodes.devnet.create",
                "local_nodes.devnet.load",
                "local_nodes.devnet.delete",
                "local_nodes.node.install",
                "local_nodes.node.uninstall",
                "local_nodes.node.start",
                "local_nodes.node.stop",
                "local_nodes.node.purge",
                "local_nodes.sequencer.control",
            ],
        },
        CapabilitySpec {
            key: "diagnostics",
            label: "Diagnostics",
            sub_capabilities: &[
                "diagnostics.modules.status.read",
                "diagnostics.modules.info.read",
                "diagnostics.modules.metrics.read",
                "diagnostics.provider.probe",
                "diagnostics.l1.read",
                "diagnostics.lez.indexer.read",
                "diagnostics.lez.sequencer.read",
                "diagnostics.storage.read",
                "diagnostics.delivery.read",
                "diagnostics.wallet.read",
                "diagnostics.local_nodes.read",
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};
    use serde_json::Value;

    use super::*;

    #[test]
    fn basecamp_defaults_use_module_backed_connectors() -> Result<()> {
        let value =
            serde_json::to_value(capability_registry_report(CapabilityBuildMode::Basecamp))?;

        assert_default(&value, "l1", "blockchain_module")?;
        assert_default(&value, "lez.indexer", "lez_indexer_module")?;
        assert_default(&value, "lez.sequencer", "direct_sequencer_rpc")?;
        assert_default(&value, "storage", "storage_module")?;
        assert_default(&value, "delivery", "delivery_module")?;
        assert_default(&value, "wallet.l1", "blockchain_module")?;
        assert_default(&value, "wallet.l2", "lez_core")?;
        Ok(())
    }

    #[test]
    fn standalone_defaults_do_not_use_module_backed_connectors() -> Result<()> {
        let value =
            serde_json::to_value(capability_registry_report(CapabilityBuildMode::Standalone))?;

        for key in ["l1", "lez.indexer", "storage", "delivery"] {
            let connector = default_connector_for(&value, key)?;
            if connector.ends_with("_module") {
                bail!("standalone capability `{key}` defaulted to module connector");
            }
        }
        assert_default(&value, "lez.sequencer", "direct_sequencer_rpc")?;
        Ok(())
    }

    #[test]
    fn registry_report_serializes_expected_contract_fields() -> Result<()> {
        let value =
            serde_json::to_value(capability_registry_report(CapabilityBuildMode::Standalone))?;

        if value.get("schema_version").and_then(Value::as_u64) != Some(1) {
            bail!("unexpected schema version: {value}");
        }
        for key in [
            "provider_types",
            "provider_instances",
            "connector_scopes",
            "capabilities",
        ] {
            if !value.get(key).is_some_and(Value::is_array) {
                bail!("registry report missing array `{key}`: {value}");
            }
        }
        let Some(storage) = capability_for(&value, "storage") else {
            bail!("storage capability missing: {value}");
        };
        if storage
            .get("sub_capabilities")
            .and_then(Value::as_array)
            .is_none_or(|items| items.is_empty())
        {
            bail!("storage capability missing sub-capabilities: {storage}");
        }
        Ok(())
    }

    #[test]
    fn registry_report_without_runtime_evidence_does_not_mark_providers_available() -> Result<()> {
        let basecamp =
            serde_json::to_value(capability_registry_report(CapabilityBuildMode::Basecamp))?;
        let standalone =
            serde_json::to_value(capability_registry_report(CapabilityBuildMode::Standalone))?;

        let Some(basecamp_storage) = capability_for(&basecamp, "storage") else {
            bail!("basecamp storage capability missing: {basecamp}");
        };
        if basecamp_storage.get("status").and_then(Value::as_str) != Some("loading") {
            bail!("basecamp storage should wait for runtime probe: {basecamp_storage}");
        }
        if basecamp_storage
            .get("compact_errors")
            .and_then(Value::as_array)
            .is_none_or(|items| {
                !items.iter().any(|value| {
                    value
                        .as_str()
                        .is_some_and(|text| text.contains("provider probe has not run"))
                })
            })
        {
            bail!("basecamp storage should explain missing runtime probe: {basecamp_storage}");
        }

        let Some(standalone_storage) = capability_for(&standalone, "storage") else {
            bail!("standalone storage capability missing: {standalone}");
        };
        if standalone_storage.get("status").and_then(Value::as_str) != Some("input_required") {
            bail!("standalone storage should require endpoint input: {standalone_storage}");
        }
        let Some(standalone_wallet) = capability_for(&standalone, "wallet.l1") else {
            bail!("standalone wallet capability missing: {standalone}");
        };
        if standalone_wallet.get("status").and_then(Value::as_str) != Some("input_required") {
            bail!("unconfigured standalone wallet should require input: {standalone_wallet}");
        }
        Ok(())
    }

    #[test]
    fn runtime_inputs_use_l1_provider_probe_for_availability() -> Result<()> {
        let loading_inputs = serde_json::json!({
            "node_url": "http://127.0.0.1:8545"
        });
        let loading = serde_json::to_value(capability_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&loading_inputs),
        ))?;
        let Some(loading_l1) = capability_for(&loading, "l1") else {
            bail!("l1 capability missing: {loading}");
        };
        if loading_l1.get("status").and_then(Value::as_str) != Some("loading") {
            bail!("l1 should wait for provider probe: {loading_l1}");
        }

        let ready_inputs = serde_json::json!({
            "node_url": "http://127.0.0.1:8545",
            "source_reports": {
                "blockchain": {
                    "health": {
                        "ready": true,
                        "reachable": true,
                        "status": "ready",
                        "detail": "node reachable"
                    },
                    "probe_facts": [
                        { "key": "blockchain.connection", "ok": true }
                    ]
                }
            }
        });
        let ready = serde_json::to_value(capability_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&ready_inputs),
        ))?;
        let Some(ready_l1) = capability_for(&ready, "l1") else {
            bail!("l1 capability missing: {ready}");
        };
        if ready_l1.get("status").and_then(Value::as_str) != Some("available") {
            bail!("l1 should be available with healthy provider probe: {ready_l1}");
        }
        Ok(())
    }

    #[test]
    fn runtime_inputs_compose_lez_from_child_provider_reports() -> Result<()> {
        let inputs = serde_json::json!({
            "indexer_url": "http://127.0.0.1:8081",
            "sequencer_url": "http://127.0.0.1:8082",
            "source_reports": {
                "indexer": {
                    "health": {
                        "ready": true,
                        "reachable": true,
                        "status": "ready",
                        "detail": "indexer reachable"
                    }
                },
                "execution": {
                    "health": {
                        "ready": false,
                        "reachable": false,
                        "status": "unavailable",
                        "detail": "sequencer refused connection"
                    },
                    "probe_facts": [
                        {
                            "key": "execution.connection",
                            "ok": false,
                            "error": "sequencer refused connection"
                        }
                    ]
                }
            }
        });
        let value = serde_json::to_value(capability_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&inputs),
        ))?;
        let Some(lez) = capability_for(&value, "lez") else {
            bail!("lez capability missing: {value}");
        };
        if lez.get("status").and_then(Value::as_str) != Some("degraded") {
            bail!("lez should degrade when sequencer probe fails: {lez}");
        }
        let unavailable = lez
            .get("unavailable_sub_capabilities")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if !unavailable
            .iter()
            .any(|value| value.as_str() == Some("lez.sequencer.health"))
        {
            bail!("lez should mark sequencer sub-capabilities unavailable: {lez}");
        }
        if unavailable
            .iter()
            .any(|value| value.as_str() == Some("lez.indexer.blocks.finalized.read"))
        {
            bail!("lez should keep indexer sub-capabilities available: {lez}");
        }
        Ok(())
    }

    #[test]
    fn runtime_inputs_gate_local_nodes_by_settings() -> Result<()> {
        let disabled_inputs = serde_json::json!({});
        let disabled = serde_json::to_value(capability_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&disabled_inputs),
        ))?;
        let Some(disabled_local_nodes) = capability_for(&disabled, "local_nodes") else {
            bail!("local nodes capability missing: {disabled}");
        };
        if disabled_local_nodes.get("status").and_then(Value::as_str) != Some("input_required") {
            bail!("local nodes should require enabled setting: {disabled_local_nodes}");
        }

        let enabled_inputs = serde_json::json!({
            "local_nodes_enabled": true,
            "local_devnet_enabled": false
        });
        let enabled = serde_json::to_value(capability_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&enabled_inputs),
        ))?;
        let Some(enabled_local_nodes) = capability_for(&enabled, "local_nodes") else {
            bail!("local nodes capability missing: {enabled}");
        };
        if enabled_local_nodes.get("status").and_then(Value::as_str) != Some("degraded") {
            bail!("local nodes should degrade without local devnet: {enabled_local_nodes}");
        }
        let unavailable = enabled_local_nodes
            .get("unavailable_sub_capabilities")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if !unavailable
            .iter()
            .any(|value| value.as_str() == Some("local_nodes.sequencer.control"))
        {
            bail!("local sequencer control should require local devnet: {enabled_local_nodes}");
        }
        Ok(())
    }

    #[test]
    fn runtime_inputs_resolve_configured_connectors_and_probe_availability() -> Result<()> {
        let inputs = serde_json::json!({
        "network_connector_config": {
            "scopes": {
                "storage": {
                    "connector_id": "direct_storage_rest",
                    "provenance": "network_profile"
                }
            }
        },
            "storage_rest_url": "http://127.0.0.1:8080/api/storage/v1",
            "storage_mutating_diagnostics_enabled": false,
            "wallet_profile_configured": true,
            "wallet_home_configured": false,
            "source_reports": {
                "storage": {
                    "health": {
                        "reachable": true,
                        "ready": true,
                        "status": "healthy",
                        "detail": "required storage facts observed"
                    },
                    "probe_facts": [
                        { "key": "space", "ok": true, "value": { "used": 1 } }
                    ],
                    "probes": []
                }
            }
        });
        let value = serde_json::to_value(capability_registry_report_with_value(
            CapabilityBuildMode::Basecamp,
            Some(&inputs),
        ))?;

        let Some(storage) = capability_for(&value, "storage") else {
            bail!("storage capability missing: {value}");
        };
        if storage.get("configured_connector").and_then(Value::as_str)
            != Some("direct_storage_rest")
        {
            bail!("storage did not use configured connector: {storage}");
        }
        if storage.get("connector_provenance").and_then(Value::as_str) != Some("network_profile") {
            bail!("storage did not preserve connector provenance: {storage}");
        }
        if storage.get("status").and_then(Value::as_str) != Some("degraded") {
            bail!("storage should be degraded without mutating diagnostics: {storage}");
        }
        let unavailable = storage
            .get("unavailable_sub_capabilities")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if !unavailable
            .iter()
            .any(|value| value.as_str() == Some("storage.content.upload"))
        {
            bail!("storage upload should be unavailable: {storage}");
        }

        let Some(wallet) = capability_for(&value, "wallet.l1") else {
            bail!("wallet capability missing: {value}");
        };
        if wallet.get("status").and_then(Value::as_str) != Some("degraded") {
            bail!("wallet should be degraded when home is missing: {wallet}");
        }
        Ok(())
    }

    #[test]
    fn storage_module_does_not_satisfy_backup_sync_sub_capabilities() -> Result<()> {
        let inputs = serde_json::json!({
            "network_connector_config": {
                "scopes": {
                    "storage": {
                        "connector_id": "storage_module",
                        "provenance": "network_profile"
                    }
                }
            },
            "source_reports": {
                "storage": {
                    "health": {
                        "ready": true,
                        "reachable": true,
                        "status": "ready",
                        "detail": "storage module reachable"
                    }
                }
            }
        });
        let value = serde_json::to_value(capability_registry_report_with_value(
            CapabilityBuildMode::Basecamp,
            Some(&inputs),
        ))?;
        let Some(storage) = capability_for(&value, "storage") else {
            bail!("storage capability missing: {value}");
        };

        if storage.get("status").and_then(Value::as_str) != Some("degraded") {
            bail!("storage module should degrade backup sync sub-capabilities: {storage}");
        }
        for key in [
            "storage.backup.sync_read_by_cid",
            "storage.backup.sync_upload",
        ] {
            if !unavailable_contains(storage, key) {
                bail!("storage module should not satisfy `{key}`: {storage}");
            }
        }
        Ok(())
    }

    #[test]
    fn wallet_scoped_connectors_are_read_from_wallet_profile_config() -> Result<()> {
        let inputs = serde_json::json!({
            "network_connector_config": {
                "scopes": {
                    "wallet.l2": {
                        "connector_id": "storage_module",
                        "provenance": "network_profile"
                    }
                }
            },
            "wallet_connector_config": {
                "scopes": {
                    "wallet.l2": {
                        "connector_id": "lez_core",
                        "provenance": "wallet_profile"
                    }
                }
            },
            "wallet_profile_configured": true,
            "wallet_home_configured": true
        });
        let value = serde_json::to_value(capability_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&inputs),
        ))?;
        let Some(wallet_l2) = capability_for(&value, "wallet.l2") else {
            bail!("wallet.l2 capability missing: {value}");
        };

        if wallet_l2
            .get("configured_connector")
            .and_then(Value::as_str)
            != Some("lez_core")
        {
            bail!("wallet.l2 should use wallet connector config: {wallet_l2}");
        }
        if wallet_l2
            .get("connector_provenance")
            .and_then(Value::as_str)
            != Some("wallet_profile")
        {
            bail!("wallet.l2 should preserve wallet connector provenance: {wallet_l2}");
        }
        Ok(())
    }

    #[test]
    fn runtime_inputs_reject_known_connector_for_wrong_network_scope() -> Result<()> {
        let inputs = serde_json::json!({
            "network_connector_config": {
                "scopes": {
                    "l1": {
                        "connector_id": "direct_storage_rest",
                        "provenance": "network_profile"
                    }
                }
            },
            "source_reports": {
                "blockchain": {
                    "health": {
                        "ready": true,
                        "reachable": true,
                        "status": "ready",
                        "detail": "node reachable"
                    }
                }
            }
        });
        let value = serde_json::to_value(capability_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&inputs),
        ))?;
        let Some(l1) = capability_for(&value, "l1") else {
            bail!("l1 capability missing: {value}");
        };

        if l1.get("status").and_then(Value::as_str) != Some("unavailable") {
            bail!("wrong-scope l1 connector should be unavailable: {l1}");
        }
        if !compact_errors_contain(l1, "does not provide capability `l1`") {
            bail!("wrong-scope l1 connector should report scope error: {l1}");
        }
        Ok(())
    }

    #[test]
    fn wallet_scoped_connector_must_provide_requested_scope() -> Result<()> {
        let inputs = serde_json::json!({
            "wallet_connector_config": {
                "scopes": {
                    "wallet.l2": {
                        "connector_id": "direct_storage_rest",
                        "provenance": "wallet_profile"
                    }
                }
            },
            "wallet_profile_configured": true,
            "wallet_home_configured": true
        });
        let value = serde_json::to_value(capability_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&inputs),
        ))?;
        let Some(wallet_l2) = capability_for(&value, "wallet.l2") else {
            bail!("wallet.l2 capability missing: {value}");
        };

        if wallet_l2.get("status").and_then(Value::as_str) != Some("unavailable") {
            bail!("wrong-scope wallet.l2 connector should be unavailable: {wallet_l2}");
        }
        if !compact_errors_contain(wallet_l2, "does not provide capability `wallet.l2`") {
            bail!("wrong-scope wallet.l2 connector should report scope error: {wallet_l2}");
        }
        Ok(())
    }

    #[test]
    fn diagnostics_require_runtime_evidence_before_availability() -> Result<()> {
        let empty_shell = serde_json::json!({
            "diagnostics_reports": {
                "module_reports": {
                    "blockchain": null,
                    "storage": null,
                    "delivery": null
                },
                "source_reports": {
                    "l1": null,
                    "storage": null,
                    "delivery": null
                },
                "last_known": {
                    "local_nodes": "",
                    "wallet": ""
                }
            }
        });
        let loading = serde_json::to_value(capability_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&empty_shell),
        ))?;
        let Some(loading_diagnostics) = capability_for(&loading, "diagnostics") else {
            bail!("diagnostics capability missing: {loading}");
        };
        if loading_diagnostics.get("status").and_then(Value::as_str) != Some("loading") {
            bail!(
                "empty diagnostics shell should not count as runtime evidence: {loading_diagnostics}"
            );
        }

        let evidence = serde_json::json!({
            "diagnostics_reports": {
                "source_reports": {
                    "storage": {
                        "health": {
                            "ready": true,
                            "reachable": true,
                            "status": "ready"
                        }
                    }
                },
                "unavailable_sub_capabilities": [
                    "diagnostics.wallet.read"
                ],
                "warnings": [
                    "wallet diagnostics have not run"
                ]
            }
        });
        let degraded = serde_json::to_value(capability_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&evidence),
        ))?;
        let Some(degraded_diagnostics) = capability_for(&degraded, "diagnostics") else {
            bail!("diagnostics capability missing: {degraded}");
        };
        if degraded_diagnostics.get("status").and_then(Value::as_str) != Some("degraded") {
            bail!("diagnostics should degrade from runtime evidence: {degraded_diagnostics}");
        }
        if !unavailable_contains(degraded_diagnostics, "diagnostics.wallet.read") {
            bail!(
                "diagnostics should preserve unavailable runtime sub-capability: {degraded_diagnostics}"
            );
        }
        if unavailable_contains(degraded_diagnostics, "diagnostics.storage.read") {
            bail!(
                "storage diagnostics evidence should enable storage diagnostics: {degraded_diagnostics}"
            );
        }
        if !unavailable_contains(degraded_diagnostics, "diagnostics.delivery.read") {
            bail!(
                "missing delivery evidence should block delivery diagnostics: {degraded_diagnostics}"
            );
        }
        Ok(())
    }

    #[test]
    fn diagnostics_source_report_failure_marks_matching_sub_capability_unavailable() -> Result<()> {
        let inputs = serde_json::json!({
            "diagnostics_reports": {
                "source_reports": {
                    "storage": {
                        "health": {
                            "ready": false,
                            "reachable": true,
                            "status": "degraded",
                            "detail": "storage probe timed out",
                            "summary": "storage source degraded"
                        },
                        "probe_facts": [{
                            "key": "storage.health",
                            "ok": false,
                            "error": "storage probe timed out"
                        }]
                    }
                }
            }
        });
        let value = serde_json::to_value(capability_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&inputs),
        ))?;
        let Some(diagnostics) = capability_for(&value, "diagnostics") else {
            bail!("diagnostics capability missing: {value}");
        };

        if diagnostics.get("status").and_then(Value::as_str) != Some("degraded") {
            bail!("storage probe failure should degrade diagnostics: {diagnostics}");
        }
        if !unavailable_contains(diagnostics, "diagnostics.storage.read") {
            bail!("storage probe failure should block storage diagnostics: {diagnostics}");
        }
        if !compact_errors_contain(diagnostics, "storage probe timed out") {
            bail!("storage probe error should be preserved: {diagnostics}");
        }
        Ok(())
    }

    #[test]
    fn diagnostics_last_known_errors_degrade_matching_sub_capability() -> Result<()> {
        let inputs = serde_json::json!({
            "diagnostics_reports": {
                "last_known": {
                    "wallet": "wallet status probe failed"
                }
            }
        });
        let value = serde_json::to_value(capability_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&inputs),
        ))?;
        let Some(diagnostics) = capability_for(&value, "diagnostics") else {
            bail!("diagnostics capability missing: {value}");
        };

        if diagnostics.get("status").and_then(Value::as_str) != Some("degraded") {
            bail!("last-known wallet error should degrade diagnostics: {diagnostics}");
        }
        if !unavailable_contains(diagnostics, "diagnostics.wallet.read") {
            bail!("last-known wallet error should block wallet diagnostics: {diagnostics}");
        }
        Ok(())
    }

    #[test]
    fn diagnostics_live_evidence_only_enables_matching_sub_capability() -> Result<()> {
        let inputs = serde_json::json!({
            "diagnostics_reports": {
                "source_reports": {
                    "delivery": {
                        "health": {
                            "ready": true,
                            "reachable": true,
                            "status": "ready",
                            "detail": "delivery source ready",
                            "summary": "delivery source ready"
                        },
                        "probe_facts": [{
                            "key": "delivery.health",
                            "ok": true
                        }]
                    }
                }
            }
        });
        let value = serde_json::to_value(capability_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&inputs),
        ))?;
        let Some(diagnostics) = capability_for(&value, "diagnostics") else {
            bail!("diagnostics capability missing: {value}");
        };

        if diagnostics.get("status").and_then(Value::as_str) != Some("degraded") {
            bail!("partial live diagnostics evidence should be degraded: {diagnostics}");
        }
        if unavailable_contains(diagnostics, "diagnostics.delivery.read") {
            bail!("delivery evidence should enable delivery diagnostics: {diagnostics}");
        }
        if !unavailable_contains(diagnostics, "diagnostics.storage.read") {
            bail!("missing storage evidence should block storage diagnostics: {diagnostics}");
        }
        Ok(())
    }

    #[test]
    fn runtime_inputs_wait_for_provider_probe_before_availability() -> Result<()> {
        let inputs = serde_json::json!({
                "network_connector_config": {
                    "scopes": {
                    "storage": {
                        "connector_id": "direct_storage_rest",
                        "provenance": "network_profile"
                    }
                }
            },
            "storage_rest_url": "http://127.0.0.1:8080/api/storage/v1",
            "storage_mutating_diagnostics_enabled": true
        });
        let value = serde_json::to_value(capability_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&inputs),
        ))?;
        let Some(storage) = capability_for(&value, "storage") else {
            bail!("storage capability missing: {value}");
        };

        if storage.get("configured_connector").and_then(Value::as_str)
            != Some("direct_storage_rest")
        {
            bail!("storage connector should not fall back: {storage}");
        }
        if storage.get("status").and_then(Value::as_str) != Some("loading") {
            bail!("storage should wait for provider probe: {storage}");
        }
        if storage
            .get("compact_errors")
            .and_then(Value::as_array)
            .is_none_or(|items| {
                !items.iter().any(|value| {
                    value
                        .as_str()
                        .is_some_and(|text| text.contains("provider probe has not run"))
                })
            })
        {
            bail!("storage loading state should explain missing provider probe: {storage}");
        }
        Ok(())
    }

    #[test]
    fn runtime_inputs_wait_for_module_provider_probe_before_availability() -> Result<()> {
        let inputs = serde_json::json!({
            "network_connector_config": {
                "scopes": {
                    "storage": {
                        "connector_id": "storage_module",
                        "provenance": "network_profile"
                    }
                }
            }
        });
        let value = serde_json::to_value(capability_registry_report_with_value(
            CapabilityBuildMode::Basecamp,
            Some(&inputs),
        ))?;
        let Some(storage) = capability_for(&value, "storage") else {
            bail!("storage capability missing: {value}");
        };

        if storage.get("configured_connector").and_then(Value::as_str) != Some("storage_module") {
            bail!("storage connector should remain module-backed: {storage}");
        }
        if storage.get("status").and_then(Value::as_str) != Some("loading") {
            bail!("storage module should wait for provider probe: {storage}");
        }
        Ok(())
    }

    #[test]
    fn runtime_inputs_surface_provider_probe_failure_without_fallback() -> Result<()> {
        let inputs = serde_json::json!({
            "network_connector_config": {
                "scopes": {
                    "storage": {
                        "connector_id": "direct_storage_rest",
                        "provenance": "network_profile"
                    }
                }
            },
            "storage_rest_url": "http://127.0.0.1:8080/api/storage/v1",
            "storage_mutating_diagnostics_enabled": true,
            "source_reports": {
                "storage": {
                    "health": {
                        "reachable": false,
                        "ready": false,
                        "status": "unavailable",
                        "detail": "storage connection refused"
                    },
                    "probe_facts": [
                        { "key": "space", "ok": false, "error": "space probe failed" }
                    ],
                    "probes": []
                }
            }
        });
        let value = serde_json::to_value(capability_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&inputs),
        ))?;
        let Some(storage) = capability_for(&value, "storage") else {
            bail!("storage capability missing: {value}");
        };

        if storage.get("configured_connector").and_then(Value::as_str)
            != Some("direct_storage_rest")
        {
            bail!("storage connector should not fall back: {storage}");
        }
        if storage.get("status").and_then(Value::as_str) != Some("unavailable") {
            bail!("storage should surface provider probe failure: {storage}");
        }
        if storage
            .get("compact_errors")
            .and_then(Value::as_array)
            .is_none_or(|items| {
                !items.iter().any(|value| {
                    value
                        .as_str()
                        .is_some_and(|text| text.contains("connection refused"))
                })
            })
        {
            bail!("storage should carry provider probe error: {storage}");
        }
        Ok(())
    }

    #[test]
    fn runtime_inputs_reject_unknown_connectors() -> Result<()> {
        let inputs = serde_json::json!({
            "network_connector_config": {
                "scopes": {
                    "storage": {
                        "connector_id": "storage_metrics",
                        "provenance": "network_profile"
                    }
                }
            },
            "storage_rest_url": "http://127.0.0.1:8080/api/storage/v1"
        });
        let value = serde_json::to_value(capability_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&inputs),
        ))?;
        let Some(storage) = capability_for(&value, "storage") else {
            bail!("storage capability missing: {value}");
        };

        if storage.get("status").and_then(Value::as_str) != Some("unavailable") {
            bail!("unknown storage connector should be unavailable: {storage}");
        }
        if storage
            .get("compact_errors")
            .and_then(Value::as_array)
            .is_none_or(|items| items.is_empty())
        {
            bail!("unknown storage connector should report compact error: {storage}");
        }
        Ok(())
    }

    fn assert_default(value: &Value, key: &str, expected: &str) -> Result<()> {
        let actual = default_connector_for(value, key)?;
        if actual != expected {
            bail!("capability `{key}` default connector `{actual}` != `{expected}`");
        }
        Ok(())
    }

    fn default_connector_for<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
        capability_for(value, key)
            .and_then(|capability| capability.get("default_connector"))
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("missing default connector for `{key}`"))
    }

    fn capability_for<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
        value
            .get("capabilities")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|capability| capability.get("key").and_then(Value::as_str) == Some(key))
    }

    fn unavailable_contains(capability: &Value, key: &str) -> bool {
        capability
            .get("unavailable_sub_capabilities")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|value| value.as_str() == Some(key))
    }

    fn compact_errors_contain(capability: &Value, text: &str) -> bool {
        capability
            .get("compact_errors")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|value| value.as_str().is_some_and(|value| value.contains(text)))
    }
}
