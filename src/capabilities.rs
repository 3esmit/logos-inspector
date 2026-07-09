use serde::Serialize;
use serde_json::Value;

mod availability;
mod catalog;
mod diagnostics_evidence;
mod runtime_evidence;
mod state_rulebook;

use catalog::{
    capability_specs, connector_scopes, default_connector, provider_instances, provider_types,
};
use state_rulebook::capability_state;

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
    let inputs = CapabilityRuntimeInputs::provided_empty();
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

mod runtime_inputs;

use runtime_inputs::{CapabilityRuntimeInputs, ResolvedConnector};

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
