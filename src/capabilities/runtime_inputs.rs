use serde_json::Value;

use super::{
    CapabilityBuildMode, catalog::default_connector, evidence::CapabilityEvidenceSnapshot,
};

#[derive(Debug, Clone, Default)]
pub(super) struct CapabilityRuntimeInputs {
    pub(super) provided: bool,
    network_connector_config: Option<Value>,
    wallet_connector_config: Option<Value>,
    node_url: String,
    storage_rest_url: String,
    storage_metrics_url: String,
    messaging_rest_url: String,
    messaging_metrics_url: String,
    pub(super) messaging_store_peer_address: String,
    l1_configuration_generation: Option<u64>,
    storage_configuration_generation: Option<u64>,
    delivery_configuration_generation: Option<u64>,
    pub(super) wallet_profile_configured: bool,
    pub(super) wallet_home_configured: bool,
    pub(super) wallet_instruction_submit_ready: bool,
    pub(super) local_nodes_enabled: bool,
    pub(super) local_devnet_enabled: bool,
    evidence: CapabilityEvidenceSnapshot,
}

impl CapabilityRuntimeInputs {
    pub(super) fn provided_empty() -> Self {
        Self {
            provided: true,
            ..Self::default()
        }
    }

    pub(super) fn from_value(value: Option<&Value>) -> Self {
        Self::from_value_with_evidence(value, CapabilityEvidenceSnapshot::default())
    }

    pub(super) fn from_value_with_evidence(
        value: Option<&Value>,
        evidence: CapabilityEvidenceSnapshot,
    ) -> Self {
        let Some(value) = value.filter(|value| value.is_object()) else {
            return Self {
                evidence,
                ..Self::default()
            };
        };
        Self {
            provided: true,
            network_connector_config: value.get("network_connector_config").cloned(),
            wallet_connector_config: value.get("wallet_connector_config").cloned(),
            node_url: string_input(value, "node_url"),
            storage_rest_url: string_input(value, "storage_rest_url"),
            storage_metrics_url: string_input(value, "storage_metrics_url"),
            messaging_rest_url: string_input(value, "messaging_rest_url"),
            messaging_metrics_url: string_input(value, "messaging_metrics_url"),
            messaging_store_peer_address: string_input(value, "messaging_store_peer_address"),
            l1_configuration_generation: configuration_generation(value, &["l1", "blockchain"]),
            storage_configuration_generation: configuration_generation(value, &["storage"]),
            delivery_configuration_generation: configuration_generation(
                value,
                &["delivery", "messaging"],
            ),
            wallet_profile_configured: bool_input(value, "wallet_profile_configured"),
            wallet_home_configured: bool_input(value, "wallet_home_configured"),
            wallet_instruction_submit_ready: bool_input(value, "wallet_instruction_submit_ready"),
            local_nodes_enabled: bool_input(value, "local_nodes_enabled"),
            local_devnet_enabled: bool_input(value, "local_devnet_enabled"),
            evidence,
        }
    }

    pub(super) fn with_evidence(mut self, evidence: CapabilityEvidenceSnapshot) -> Self {
        self.evidence = evidence;
        self
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
            "storage" if connector.id == "storage_metrics" => self.storage_metrics_url.clone(),
            "storage" => self.storage_rest_url.clone(),
            "delivery" if connector.id == "delivery_metrics" => self.messaging_metrics_url.clone(),
            "delivery" => self.messaging_rest_url.clone(),
            _ => String::new(),
        }
    }

    pub(super) fn metrics_endpoint_for(&self, scope: &str) -> &str {
        match scope {
            "storage" => &self.storage_metrics_url,
            "delivery" => &self.messaging_metrics_url,
            _ => "",
        }
    }

    pub(super) fn configuration_generation_for(&self, scope: &str) -> Option<u64> {
        match scope {
            "l1" => self.l1_configuration_generation,
            "storage" => self.storage_configuration_generation,
            "delivery" => self.delivery_configuration_generation,
            _ => None,
        }
    }

    pub(super) fn source_report_for(&self, scope: &str) -> Option<&Value> {
        self.evidence
            .source_report(scope)
            .filter(|value| value.is_object())
    }

    pub(super) fn diagnostics_report(&self) -> Option<Value> {
        self.evidence
            .diagnostics_report()
            .filter(super::runtime_evidence::report_has_runtime_evidence)
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

fn configuration_generation(value: &Value, keys: &[&str]) -> Option<u64> {
    let generations = value.get("configuration_generations")?;
    keys.iter().find_map(|key| match generations.get(*key) {
        Some(Value::Number(value)) => value.as_u64(),
        Some(Value::String(value)) => value.trim().parse().ok(),
        _ => None,
    })
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
