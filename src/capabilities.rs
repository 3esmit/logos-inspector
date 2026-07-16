use serde::Serialize;
use serde_json::Value;

mod availability;
mod catalog;
mod diagnostics_evidence;
mod evidence;
mod runtime_evidence;
mod state_rulebook;

pub(crate) use evidence::CapabilityRegistry;

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
        provider_instances: provider_instances(),
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
            let state = capability_state(inputs, spec, &connector);
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
        "l1" | "storage" | "delivery" | "wallet.l1" | "wallet.l2" => {
            inputs.connector_for(build_mode, capability_key)
        }
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
    use anyhow::{Context as _, Result, bail};
    use serde_json::{Value, json};

    use super::*;

    fn test_registry_report_with_value(
        build_mode: CapabilityBuildMode,
        value: Option<&Value>,
    ) -> CapabilityRegistryReport {
        let evidence = evidence::CapabilityEvidenceSnapshot::from_legacy_test_value(value);
        let inputs = CapabilityRuntimeInputs::from_value_with_evidence(value, evidence);
        capability_registry_report_with_inputs(build_mode, &inputs)
    }

    #[test]
    fn basecamp_defaults_use_module_backed_connectors() -> Result<()> {
        let value =
            serde_json::to_value(capability_registry_report(CapabilityBuildMode::Basecamp))?;

        assert_default(&value, "l1", "blockchain_module")?;
        assert_default(&value, "storage", "storage_module")?;
        assert_default(&value, "delivery", "delivery_module")?;
        assert_default(&value, "wallet.l1", "blockchain_module")?;
        assert_default(&value, "wallet.l2", "lez_core")?;
        Ok(())
    }

    #[test]
    fn standalone_defaults_use_operational_testnet_connectors() -> Result<()> {
        let value =
            serde_json::to_value(capability_registry_report(CapabilityBuildMode::Standalone))?;

        for (key, expected, expected_type) in [
            ("l1", "direct_l1_rpc", "direct_rpc"),
            ("storage", "logoscore_cli_storage_module", "logoscore_cli"),
            ("delivery", "logoscore_cli_delivery_module", "logoscore_cli"),
        ] {
            if default_connector_for(&value, key)? != expected {
                bail!("standalone capability `{key}` used an unexpected default connector");
            }
            let provider_type = value
                .get("provider_instances")
                .and_then(Value::as_array)
                .and_then(|providers| {
                    providers.iter().find(|provider| {
                        provider.get("id").and_then(Value::as_str) == Some(expected)
                    })
                })
                .and_then(|provider| provider.get("provider_type"))
                .and_then(Value::as_str);
            if provider_type != Some(expected_type) {
                bail!("standalone capability `{key}` used an unexpected provider type");
            }
        }
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
        if standalone_storage.get("status").and_then(Value::as_str) != Some("loading") {
            bail!("standalone storage should wait for LogosCore CLI probe: {standalone_storage}");
        }
        if standalone_storage
            .get("configured_connector")
            .and_then(Value::as_str)
            != Some("logoscore_cli_storage_module")
        {
            bail!("standalone storage should use LogosCore CLI: {standalone_storage}");
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
        let loading = serde_json::to_value(test_registry_report_with_value(
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
        let ready = serde_json::to_value(test_registry_report_with_value(
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
    fn runtime_inputs_gate_local_nodes_by_settings() -> Result<()> {
        let disabled_inputs = serde_json::json!({});
        let disabled = serde_json::to_value(test_registry_report_with_value(
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
        let enabled = serde_json::to_value(test_registry_report_with_value(
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
        let value = serde_json::to_value(test_registry_report_with_value(
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
        if storage.get("status").and_then(Value::as_str) != Some("available") {
            bail!("storage should remain available with legacy mutation input: {storage}");
        }
        let unavailable = storage
            .get("unavailable_sub_capabilities")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if unavailable
            .iter()
            .any(|value| value.as_str() == Some("storage.content.upload"))
        {
            bail!("storage upload should remain available: {storage}");
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
    fn delivery_store_queries_require_a_rest_connector() -> Result<()> {
        let delivery_capability = |build_mode, connector_id: &str, endpoint: Option<&str>| {
            let inputs = json!({
                "network_connector_config": {
                    "scopes": {
                        "delivery": {
                            "connector_id": connector_id,
                            "provenance": "test"
                        }
                    }
                },
                "messaging_rest_url": endpoint.unwrap_or_default(),
                "source_reports": {
                    "delivery": {
                        "health": {
                            "ready": true,
                            "reachable": true,
                            "status": "ready",
                            "detail": "delivery source reachable"
                        }
                    }
                }
            });
            let value =
                serde_json::to_value(test_registry_report_with_value(build_mode, Some(&inputs)))?;
            capability_for(&value, "delivery")
                .cloned()
                .context("delivery capability missing")
        };

        for (build_mode, connector_id) in [
            (CapabilityBuildMode::Basecamp, "delivery_module"),
            (
                CapabilityBuildMode::Standalone,
                "logoscore_cli_delivery_module",
            ),
        ] {
            let delivery = delivery_capability(build_mode, connector_id, None)?;
            if !unavailable_contains(&delivery, "delivery.store.query") {
                bail!("{connector_id} overclaimed Delivery Store queries: {delivery}");
            }
            if unavailable_contains(&delivery, "delivery.send") {
                bail!("{connector_id} should retain Delivery send: {delivery}");
            }
        }

        let rest = delivery_capability(
            CapabilityBuildMode::Standalone,
            "direct_delivery_rest",
            Some("http://127.0.0.1:8645"),
        )?;
        if unavailable_contains(&rest, "delivery.store.query") {
            bail!("Direct Waku REST should provide Delivery Store queries: {rest}");
        }
        Ok(())
    }

    #[test]
    fn storage_module_fails_backup_sync_read_closed_without_runtime_contract() -> Result<()> {
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
        let value = serde_json::to_value(test_registry_report_with_value(
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
    fn basecamp_storage_backup_read_requires_exact_host_runtime_contract() -> Result<()> {
        let source_report = || {
            json!({
                "health": {
                    "ready": true,
                    "reachable": true,
                    "status": "ready",
                    "detail": "storage module reachable"
                },
                "module_info": {
                    "ok": true,
                    "value": {
                        "name": "storage_module",
                        "methods": [
                            {
                                "isInvokable": true,
                                "name": "downloadProtocol",
                                "signature": "downloadProtocol()"
                            },
                            {
                                "isInvokable": true,
                                "name": "downloadToUrlV2",
                                "signature": "downloadToUrlV2(QString,QString,bool,int,QString,int)"
                            },
                            {
                                "isInvokable": true,
                                "name": "downloadCancelV2",
                                "signature": "downloadCancelV2(QString)"
                            }
                        ],
                        "events": [{
                            "name": "storageDownloadDoneV2",
                            "signature": "storageDownloadDoneV2(QString)"
                        }]
                    }
                },
                "probes": [{
                    "probe_key": "backupDownloadReadiness",
                    "label": "storage backup download readiness",
                    "source": "basecamp host-events storage_module storageDownloadDoneV2",
                    "ok": true,
                    "value": {
                        "shared_staging": true,
                        "contract": {
                            "protocol": "logos.storage.download",
                            "version": 2,
                            "moduleOperationIdOwner": "caller",
                            "cancelTimeoutMs": 15_000,
                            "maxDownloadBytes": 1_073_741_824_u64
                        },
                        "event_transport": {
                            "protocol": "basecamp.host-events",
                            "version": 1,
                            "ready": true,
                            "native_runtime_event_owner": true,
                            "module": "storage_module",
                            "event": "storageDownloadDoneV2"
                        }
                    }
                }]
            })
        };
        let storage_capability = |storage: Value| -> Result<Value> {
            let value = serde_json::to_value(test_registry_report_with_value(
                CapabilityBuildMode::Basecamp,
                Some(&json!({
                    "network_connector_config": {
                        "scopes": {
                            "storage": {
                                "connector_id": "storage_module",
                                "provenance": "build_default"
                            }
                        }
                    },
                    "source_reports": { "storage": storage }
                })),
            ))?;
            capability_for(&value, "storage")
                .cloned()
                .context("Basecamp Storage capability missing")
        };

        let supported = storage_capability(source_report())?;
        if unavailable_contains(&supported, "storage.backup.sync_read_by_cid") {
            bail!("exact Basecamp runtime contract was not credited: {supported}");
        }
        if !unavailable_contains(&supported, "storage.backup.sync_upload") {
            bail!("Basecamp backup upload was overclaimed: {supported}");
        }

        for (pointer, replacement, mismatch) in [
            (
                "/module_info/value/methods/0/isInvokable",
                json!(false),
                "non-invokable protocol method",
            ),
            (
                "/module_info/value/methods/1/signature",
                json!("downloadToUrlV2(QString)"),
                "wrong download method signature",
            ),
            (
                "/module_info/value/events/0/signature",
                json!("storageDownloadDoneV2()"),
                "wrong terminal event signature",
            ),
            ("/probes/0/ok", json!(false), "failed readiness probe"),
            (
                "/probes/0/value/shared_staging",
                json!(false),
                "missing shared staging",
            ),
            (
                "/probes/0/value/contract/protocol",
                json!("logos.storage.download.other"),
                "wrong download protocol",
            ),
            (
                "/probes/0/value/contract/version",
                json!(1),
                "wrong download protocol version",
            ),
            (
                "/probes/0/value/contract/moduleOperationIdOwner",
                json!("module"),
                "wrong operation identity owner",
            ),
            (
                "/probes/0/value/contract/cancelTimeoutMs",
                json!(14_999),
                "wrong cancellation timeout",
            ),
            (
                "/probes/0/value/contract/maxDownloadBytes",
                json!(crate::support::settings_backup::SETTINGS_BACKUP_MAX_BYTES as u64 - 1),
                "insufficient byte limit",
            ),
            (
                "/probes/0/value/event_transport/protocol",
                json!("logoscore.watch"),
                "wrong host-event protocol",
            ),
            (
                "/probes/0/value/event_transport/version",
                json!(2),
                "wrong host-event protocol version",
            ),
            (
                "/probes/0/value/event_transport/ready",
                json!(false),
                "unready host event subscription",
            ),
            (
                "/probes/0/value/event_transport/native_runtime_event_owner",
                json!(false),
                "unhealthy native event ownership",
            ),
            (
                "/probes/0/value/event_transport/module",
                json!("blockchain_module"),
                "wrong subscribed module",
            ),
            (
                "/probes/0/value/event_transport/event",
                json!("storageDownloadProgressV2"),
                "wrong subscribed event",
            ),
        ] {
            let mut report = source_report();
            *report
                .pointer_mut(pointer)
                .with_context(|| format!("missing test report field `{pointer}`"))? = replacement;
            let capability = storage_capability(report)?;
            if !unavailable_contains(&capability, "storage.backup.sync_read_by_cid") {
                bail!("{mismatch} overclaimed Basecamp backup read: {capability}");
            }
        }
        Ok(())
    }

    #[test]
    fn logoscore_storage_backup_read_requires_exact_runtime_contract() -> Result<()> {
        let source_report = |events: Value| {
            json!({
                "health": {
                    "ready": true,
                    "reachable": true,
                    "status": "ready",
                    "detail": "storage module reachable"
                },
                "module_info": {
                    "ok": true,
                    "value": {
                        "value": {
                            "name": "storage_module",
                            "methods": [
                                {
                                    "isInvokable": true,
                                    "name": "downloadProtocol",
                                    "signature": "downloadProtocol()"
                                },
                                {
                                    "isInvokable": true,
                                    "name": "downloadToUrlV2",
                                    "signature": "downloadToUrlV2(QString,QString,bool,int,QString,int)"
                                },
                                {
                                    "isInvokable": true,
                                    "name": "downloadCancelV2",
                                    "signature": "downloadCancelV2(QString)"
                                }
                            ],
                            "events": events
                        }
                    }
                },
                "probes": [{
                    "probe_key": "backupDownloadReadiness",
                    "label": "storage backup download readiness",
                    "source": "logoscore watch storage_module --event storageDownloadDoneV2 --json --watch-protocol v1",
                    "ok": true,
                    "value": {
                        "shared_staging": true,
                        "contract": {
                            "protocol": "logos.storage.download",
                            "version": 2,
                            "moduleOperationIdOwner": "caller",
                            "cancelTimeoutMs": 15_000,
                            "maxDownloadBytes": 1_073_741_824_u64
                        },
                        "watch_protocol": {
                            "protocol": "logoscore.watch",
                            "version": 1,
                            "ready": true
                        }
                    }
                }]
            })
        };
        let report = |storage: Value| {
            test_registry_report_with_value(
                CapabilityBuildMode::Standalone,
                Some(&json!({
                    "network_connector_config": {
                        "scopes": {
                            "storage": {
                                "connector_id": "logoscore_cli_storage_module",
                                "provenance": "build_default"
                            }
                        }
                    },
                    "source_reports": { "storage": storage }
                })),
            )
        };
        let replace = |value: &mut Value, pointer: &str, replacement: Value| -> Result<()> {
            *value
                .pointer_mut(pointer)
                .with_context(|| format!("missing test report field `{pointer}`"))? = replacement;
            Ok(())
        };

        let supported = serde_json::to_value(report(source_report(json!([
            { "name": "storageDownloadDoneV2", "signature": "storageDownloadDoneV2(QString)" }
        ]))))?;
        let unsupported = serde_json::to_value(report(source_report(json!([]))))?;
        let wrong_signature = serde_json::to_value(report(source_report(json!([
            { "name": "storageDownloadDoneV2", "signature": "storageDownloadDoneV2()" }
        ]))))?;
        let mut failed_readiness = source_report(json!([
            { "name": "storageDownloadDoneV2", "signature": "storageDownloadDoneV2(QString)" }
        ]));
        replace(&mut failed_readiness, "/probes/0/ok", json!(false))?;
        let failed_readiness = serde_json::to_value(report(failed_readiness))?;
        let mut failed_probe = source_report(json!([
            { "name": "storageDownloadDoneV2", "signature": "storageDownloadDoneV2(QString)" }
        ]));
        replace(&mut failed_probe, "/module_info/ok", json!(false))?;
        let failed_probe = serde_json::to_value(report(failed_probe))?;
        let mut not_invokable = source_report(json!([
            { "name": "storageDownloadDoneV2", "signature": "storageDownloadDoneV2(QString)" }
        ]));
        replace(
            &mut not_invokable,
            "/module_info/value/value/methods/0/isInvokable",
            json!(false),
        )?;
        let not_invokable = serde_json::to_value(report(not_invokable))?;
        let mut wrong_method_signature = source_report(json!([
            { "name": "storageDownloadDoneV2", "signature": "storageDownloadDoneV2(QString)" }
        ]));
        replace(
            &mut wrong_method_signature,
            "/module_info/value/value/methods/1/signature",
            json!("downloadToUrlV2(QString,QString,bool,int)"),
        )?;
        let wrong_method_signature = serde_json::to_value(report(wrong_method_signature))?;
        let mut wrong_protocol = source_report(json!([
            { "name": "storageDownloadDoneV2", "signature": "storageDownloadDoneV2(QString)" }
        ]));
        replace(
            &mut wrong_protocol,
            "/probes/0/value/contract/version",
            json!(1),
        )?;
        let wrong_protocol = serde_json::to_value(report(wrong_protocol))?;
        let mut wrong_cancel_timeout = source_report(json!([
            { "name": "storageDownloadDoneV2", "signature": "storageDownloadDoneV2(QString)" }
        ]));
        replace(
            &mut wrong_cancel_timeout,
            "/probes/0/value/contract/cancelTimeoutMs",
            json!(12_000),
        )?;
        let wrong_cancel_timeout = serde_json::to_value(report(wrong_cancel_timeout))?;
        let mut insufficient_download_limit = source_report(json!([
            { "name": "storageDownloadDoneV2", "signature": "storageDownloadDoneV2(QString)" }
        ]));
        replace(
            &mut insufficient_download_limit,
            "/probes/0/value/contract/maxDownloadBytes",
            json!(16 * 1024 * 1024 - 1),
        )?;
        let insufficient_download_limit =
            serde_json::to_value(report(insufficient_download_limit))?;
        let mut wrong_watch_protocol = source_report(json!([
            { "name": "storageDownloadDoneV2", "signature": "storageDownloadDoneV2(QString)" }
        ]));
        replace(
            &mut wrong_watch_protocol,
            "/probes/0/value/watch_protocol/ready",
            json!(false),
        )?;
        let wrong_watch_protocol = serde_json::to_value(report(wrong_watch_protocol))?;
        let supported = capability_for(&supported, "storage")
            .context("supported Storage capability missing")?;
        let unsupported = capability_for(&unsupported, "storage")
            .context("unsupported Storage capability missing")?;
        let wrong_signature = capability_for(&wrong_signature, "storage")
            .context("wrong-signature Storage capability missing")?;
        let failed_probe = capability_for(&failed_probe, "storage")
            .context("failed-probe Storage capability missing")?;
        let not_invokable = capability_for(&not_invokable, "storage")
            .context("not-invokable Storage capability missing")?;
        let failed_readiness = capability_for(&failed_readiness, "storage")
            .context("failed-readiness Storage capability missing")?;
        let wrong_method_signature = capability_for(&wrong_method_signature, "storage")
            .context("wrong-method-signature Storage capability missing")?;
        let wrong_protocol = capability_for(&wrong_protocol, "storage")
            .context("wrong-protocol Storage capability missing")?;
        let wrong_cancel_timeout = capability_for(&wrong_cancel_timeout, "storage")
            .context("wrong-cancel-timeout Storage capability missing")?;
        let insufficient_download_limit =
            capability_for(&insufficient_download_limit, "storage")
                .context("insufficient-download-limit Storage capability missing")?;
        let wrong_watch_protocol = capability_for(&wrong_watch_protocol, "storage")
            .context("wrong-watch-protocol Storage capability missing")?;

        if unavailable_contains(supported, "storage.backup.sync_read_by_cid") {
            bail!("exact runtime contract was not credited: {supported}");
        }
        if !unavailable_contains(unsupported, "storage.backup.sync_read_by_cid") {
            bail!("missing runtime event overclaimed backup read: {unsupported}");
        }
        if !unavailable_contains(wrong_signature, "storage.backup.sync_read_by_cid") {
            bail!("wrong runtime event signature overclaimed backup read: {wrong_signature}");
        }
        if !unavailable_contains(failed_probe, "storage.backup.sync_read_by_cid") {
            bail!("failed module-info probe overclaimed backup read: {failed_probe}");
        }
        if !unavailable_contains(not_invokable, "storage.backup.sync_read_by_cid") {
            bail!("non-invokable download method overclaimed backup read: {not_invokable}");
        }
        if !unavailable_contains(failed_readiness, "storage.backup.sync_read_by_cid") {
            bail!("failed readiness probe overclaimed backup read: {failed_readiness}");
        }
        if !unavailable_contains(wrong_method_signature, "storage.backup.sync_read_by_cid") {
            bail!(
                "wrong download method signature overclaimed backup read: {wrong_method_signature}"
            );
        }
        if !unavailable_contains(wrong_protocol, "storage.backup.sync_read_by_cid") {
            bail!("wrong Storage download protocol overclaimed backup read: {wrong_protocol}");
        }
        if !unavailable_contains(wrong_cancel_timeout, "storage.backup.sync_read_by_cid") {
            bail!(
                "wrong Storage cancellation timeout overclaimed backup read: {wrong_cancel_timeout}"
            );
        }
        if !unavailable_contains(
            insufficient_download_limit,
            "storage.backup.sync_read_by_cid",
        ) {
            bail!(
                "insufficient producer byte limit overclaimed backup read: {insufficient_download_limit}"
            );
        }
        if !unavailable_contains(wrong_watch_protocol, "storage.backup.sync_read_by_cid") {
            bail!("unready watch protocol overclaimed backup read: {wrong_watch_protocol}");
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
        let value = serde_json::to_value(test_registry_report_with_value(
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
        let value = serde_json::to_value(test_registry_report_with_value(
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
        let value = serde_json::to_value(test_registry_report_with_value(
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
        let loading = serde_json::to_value(test_registry_report_with_value(
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
        let degraded = serde_json::to_value(test_registry_report_with_value(
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
    fn diagnostics_source_report_failure_keeps_report_reader_available() -> Result<()> {
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
        let value = serde_json::to_value(test_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&inputs),
        ))?;
        let Some(diagnostics) = capability_for(&value, "diagnostics") else {
            bail!("diagnostics capability missing: {value}");
        };

        if diagnostics.get("status").and_then(Value::as_str) != Some("degraded") {
            bail!("storage probe failure should degrade diagnostics: {diagnostics}");
        }
        if unavailable_contains(diagnostics, "diagnostics.storage.read") {
            bail!("a completed report should keep storage diagnostics readable: {diagnostics}");
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
        let value = serde_json::to_value(test_registry_report_with_value(
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
        let value = serde_json::to_value(test_registry_report_with_value(
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
        let value = serde_json::to_value(test_registry_report_with_value(
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
        let value = serde_json::to_value(test_registry_report_with_value(
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
        let value = serde_json::to_value(test_registry_report_with_value(
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
                        "connector_id": "unknown_storage_adapter",
                        "provenance": "network_profile"
                    }
                }
            },
            "storage_rest_url": "http://127.0.0.1:8080/api/storage/v1"
        });
        let value = serde_json::to_value(test_registry_report_with_value(
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

    #[test]
    fn direct_storage_rest_enables_shared_idl_sync_capabilities() -> Result<()> {
        let inputs = serde_json::json!({
            "network_connector_config": {
                "scopes": {
                    "storage": {
                        "connector_id": "direct_storage_rest",
                        "provenance": "network_profile"
                    }
                }
            },
            "storage_rest_url": "http://127.0.0.1:18080/api/storage/v1",
            "source_reports": {
                "storage": {
                    "health": {
                        "reachable": true,
                        "ready": true,
                        "status": "healthy",
                        "detail": "Storage REST is reachable"
                    },
                    "probe_facts": [
                        { "key": "space", "ok": true, "value": { "used": 1 } }
                    ],
                    "probes": []
                }
            }
        });
        let value = serde_json::to_value(test_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&inputs),
        ))?;
        let Some(storage) = capability_for(&value, "storage") else {
            bail!("storage capability missing: {value}");
        };
        let Some(advertised) = storage.get("sub_capabilities").and_then(Value::as_array) else {
            bail!("storage sub-capabilities missing: {storage}");
        };

        for capability in [
            "storage.shared_idl.sync_read",
            "storage.shared_idl.sync_upload",
        ] {
            if !advertised
                .iter()
                .any(|value| value.as_str() == Some(capability))
            {
                bail!("Storage registry does not declare `{capability}`: {storage}");
            }
            if unavailable_contains(storage, capability) {
                bail!("Storage REST unexpectedly blocks `{capability}`: {storage}");
            }
        }
        Ok(())
    }

    #[test]
    fn logoscore_storage_enables_shared_idl_upload_without_sync_read() -> Result<()> {
        let inputs = serde_json::json!({
            "network_connector_config": {
                "scopes": {
                    "storage": {
                        "connector_id": "logoscore_cli_storage_module",
                        "provenance": "build_default"
                    }
                }
            },
            "source_reports": {
                "storage": {
                    "health": {
                        "reachable": true,
                        "ready": true,
                        "status": "healthy",
                        "detail": "Storage module is reachable"
                    },
                    "probes": []
                }
            }
        });
        let value = serde_json::to_value(test_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&inputs),
        ))?;
        let Some(storage) = capability_for(&value, "storage") else {
            bail!("storage capability missing: {value}");
        };

        if unavailable_contains(storage, "storage.shared_idl.sync_upload") {
            bail!("LogosCore CLI should allow Shared IDL upload: {storage}");
        }
        if !unavailable_contains(storage, "storage.shared_idl.sync_read") {
            bail!("LogosCore CLI must not claim Shared IDL synchronous reads: {storage}");
        }
        Ok(())
    }

    #[test]
    fn storage_metrics_adapter_exposes_only_declared_capability() -> Result<()> {
        let inputs = serde_json::json!({
            "network_connector_config": {
                "scopes": {
                    "storage": {
                        "connector_id": "storage_metrics",
                        "provenance": "network_profile"
                    }
                }
            },
            "storage_metrics_url": "http://127.0.0.1:8008/metrics",
            "source_reports": {
                "storage": {
                    "health": {
                        "status": "ready",
                        "ready": true,
                        "reachable": true
                    }
                }
            }
        });
        let value = serde_json::to_value(test_registry_report_with_value(
            CapabilityBuildMode::Standalone,
            Some(&inputs),
        ))?;
        let Some(storage) = capability_for(&value, "storage") else {
            bail!("storage capability missing: {value}");
        };

        if unavailable_contains(storage, "storage.metrics.read")
            || !unavailable_contains(storage, "storage.identity.read")
        {
            bail!("metrics adapter capability mask is incorrect: {storage}");
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
