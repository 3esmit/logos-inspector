use serde_json::Value;

use super::{CapabilityBuildMode, catalog::default_connector, runtime_evidence};

#[derive(Debug, Clone, Default)]
pub(super) struct CapabilityRuntimeInputs {
    pub(super) provided: bool,
    network_connector_config: Option<Value>,
    wallet_connector_config: Option<Value>,
    node_url: String,
    storage_rest_url: String,
    messaging_rest_url: String,
    pub(super) storage_mutating_diagnostics_enabled: bool,
    pub(super) messaging_mutating_diagnostics_enabled: bool,
    pub(super) wallet_profile_configured: bool,
    pub(super) wallet_home_configured: bool,
    pub(super) local_nodes_enabled: bool,
    pub(super) local_devnet_enabled: bool,
    source_reports: Option<Value>,
    diagnostics_reports: Option<Value>,
}

impl CapabilityRuntimeInputs {
    pub(super) fn provided_empty() -> Self {
        Self {
            provided: true,
            ..Self::default()
        }
    }

    pub(super) fn from_value(value: Option<&Value>) -> Self {
        let Some(value) = value.filter(|value| value.is_object()) else {
            return Self::default();
        };
        Self {
            provided: true,
            network_connector_config: value.get("network_connector_config").cloned(),
            wallet_connector_config: value.get("wallet_connector_config").cloned(),
            node_url: string_input(value, "node_url"),
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

    pub(super) fn connector_for(
        &self,
        build_mode: CapabilityBuildMode,
        scope: &str,
    ) -> ResolvedConnector {
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

    pub(super) fn endpoint_for(&self, scope: &str, connector: &ResolvedConnector) -> String {
        if !connector.endpoint.is_empty() {
            return connector.endpoint.clone();
        }
        match scope {
            "l1" => self.node_url.clone(),
            "storage" => self.storage_rest_url.clone(),
            "delivery" => self.messaging_rest_url.clone(),
            _ => String::new(),
        }
    }

    pub(super) fn source_report_for(&self, scope: &str) -> Option<&Value> {
        let reports = self.source_reports.as_ref()?.as_object()?;
        let keys: &[&str] = match scope {
            "l1" => &["l1", "blockchain", "node"],
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

    pub(super) fn diagnostics_report(&self) -> Option<&Value> {
        self.diagnostics_reports
            .as_ref()
            .filter(|value| runtime_evidence::report_has_runtime_evidence(value))
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
pub(super) struct ResolvedConnector {
    pub(super) id: String,
    pub(super) provenance: String,
    pub(super) endpoint: String,
}

impl ResolvedConnector {
    pub(super) fn build_default(default: &str) -> Self {
        Self {
            id: default.to_owned(),
            provenance: "build_default".to_owned(),
            endpoint: String::new(),
        }
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
