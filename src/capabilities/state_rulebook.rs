use super::{
    availability::{
        CapabilityState, available_state, input_required_state, loading_state,
        merge_state_constraints, state_from_unavailable, unavailable_state,
    },
    catalog::{CapabilitySpec, provider_instance_known, provider_instance_supports},
    diagnostics_evidence::diagnostics_state,
    runtime_evidence,
    runtime_inputs::{CapabilityRuntimeInputs, ResolvedConnector},
};
use crate::{
    source_routing::network_adapter_policy_for_connector,
    support::settings_backup::SETTINGS_BACKUP_MAX_BYTES,
};

pub(super) fn capability_state(
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
        "storage" => storage_state(inputs, connector, spec.sub_capabilities),
        "delivery" => delivery_state(inputs, connector, spec.sub_capabilities),
        "wallet" => wallet_state(inputs, spec.sub_capabilities),
        "wallet.l1" | "wallet.l2" => wallet_state(inputs, spec.sub_capabilities),
        "local_nodes" => local_nodes_state(inputs, spec.sub_capabilities),
        "diagnostics" => diagnostics_state(inputs, spec.sub_capabilities),
        _ => available_state(),
    }
}

fn source_report_state(
    inputs: &CapabilityRuntimeInputs,
    scope: &str,
    label: &str,
    sub_capabilities: &[&str],
) -> CapabilityState {
    runtime_evidence::source_report_state(inputs, scope, label, sub_capabilities)
}

fn endpoint_backed_state(
    inputs: &CapabilityRuntimeInputs,
    scope: &str,
    connector: &ResolvedConnector,
    sub_capabilities: &[&str],
) -> CapabilityState {
    let requires_endpoint = network_adapter_policy_for_connector(&connector.id)
        .is_some_and(|adapter| adapter.inputs.iter().any(|input| input.required));
    if requires_endpoint && inputs.endpoint_for(scope, connector).is_empty() {
        return input_required_state(
            sub_capabilities,
            format!("{} endpoint is required", scope_label(scope)),
        );
    }
    adapter_constrained_state(
        source_report_state(inputs, scope, scope_label(scope), sub_capabilities),
        connector,
        sub_capabilities,
    )
}

fn storage_state(
    inputs: &CapabilityRuntimeInputs,
    connector: &ResolvedConnector,
    sub_capabilities: &[&str],
) -> CapabilityState {
    let Some(adapter) = network_adapter_policy_for_connector(&connector.id) else {
        return unavailable_state(
            sub_capabilities,
            format!("connector `{}` does not provide Storage", connector.id),
        );
    };
    if adapter.inputs.iter().any(|input| input.required)
        && inputs.endpoint_for("storage", connector).is_empty()
    {
        return input_required_state(
            sub_capabilities,
            "Storage adapter endpoint is required".to_owned(),
        );
    }
    let state = adapter_constrained_state(
        source_report_state(inputs, "storage", "Storage", sub_capabilities),
        connector,
        sub_capabilities,
    );
    let state = match storage_backup_download_transport(&connector.id) {
        Some(transport)
            if !inputs.source_report_for("storage").is_some_and(|report| {
                storage_backup_download_contract_supported(report, transport)
            }) =>
        {
            merge_state_constraints(
                state,
                vec!["storage.backup.sync_read_by_cid".to_owned()],
                vec![format!(
                    "Storage module lacks the bounded {} backup download contract",
                    transport.label()
                )],
                vec![format!(
                    "storage.backup.sync_read_by_cid requires Storage download v2 operation identity and {} readiness",
                    transport.readiness_label()
                )],
            )
        }
        _ => state,
    };
    if connector.id != "direct_storage_rest" || inputs.storage_mutating_diagnostics_enabled {
        return state;
    }
    merge_state_constraints(
        state,
        vec![
            "storage.content.upload".to_owned(),
            "storage.rest.upload".to_owned(),
            "storage.content.download_to_file".to_owned(),
            "storage.content.remove".to_owned(),
        ],
        vec!["Storage mutating diagnostics are disabled".to_owned()],
        Vec::new(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StorageBackupDownloadTransport {
    BasecampHost,
    LogoscoreWatch,
}

impl StorageBackupDownloadTransport {
    const fn label(self) -> &'static str {
        match self {
            Self::BasecampHost => "Basecamp host-event",
            Self::LogoscoreWatch => "LogosCore CLI",
        }
    }

    const fn readiness_label(self) -> &'static str {
        match self {
            Self::BasecampHost => "Basecamp host-events v1",
            Self::LogoscoreWatch => "LogosCore watch v1",
        }
    }
}

fn storage_backup_download_transport(connector_id: &str) -> Option<StorageBackupDownloadTransport> {
    match connector_id {
        "storage_module" => Some(StorageBackupDownloadTransport::BasecampHost),
        "logoscore_cli_storage_module" => Some(StorageBackupDownloadTransport::LogoscoreWatch),
        _ => None,
    }
}

fn storage_backup_download_contract_supported(
    report: &serde_json::Value,
    transport: StorageBackupDownloadTransport,
) -> bool {
    let Some(module_info) = storage_module_info_value(report) else {
        return false;
    };
    let methods = module_info
        .get("methods")
        .and_then(serde_json::Value::as_array);
    let events = module_info
        .get("events")
        .and_then(serde_json::Value::as_array);
    let method_matches = |name: &str, signature: &str| {
        methods.is_some_and(|methods| {
            methods.iter().any(|method| {
                method.get("name").and_then(serde_json::Value::as_str) == Some(name)
                    && method.get("signature").and_then(serde_json::Value::as_str)
                        == Some(signature)
                    && method
                        .get("isInvokable")
                        .and_then(serde_json::Value::as_bool)
                        == Some(true)
            })
        })
    };
    let metadata_supported = method_matches("downloadProtocol", "downloadProtocol()")
        && method_matches(
            "downloadToUrlV2",
            "downloadToUrlV2(QString,QString,bool,int,QString,int)",
        )
        && method_matches("downloadCancelV2", "downloadCancelV2(QString)")
        && events.is_some_and(|events| {
            events.iter().any(|event| {
                event.get("name").and_then(serde_json::Value::as_str)
                    == Some("storageDownloadDoneV2")
                    && event.get("signature").and_then(serde_json::Value::as_str)
                        == Some("storageDownloadDoneV2(QString)")
            })
        });
    metadata_supported && storage_backup_download_readiness_supported(report, transport)
}

fn storage_backup_download_readiness_supported(
    report: &serde_json::Value,
    transport: StorageBackupDownloadTransport,
) -> bool {
    report
        .get("probes")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|probes| {
            probes.iter().any(|probe| {
                probe.get("probe_key").and_then(serde_json::Value::as_str)
                    == Some("backupDownloadReadiness")
                    && probe.get("ok").and_then(serde_json::Value::as_bool) == Some(true)
                    && probe
                        .pointer("/value/shared_staging")
                        .and_then(serde_json::Value::as_bool)
                        == Some(true)
                    && probe
                        .pointer("/value/contract/protocol")
                        .and_then(serde_json::Value::as_str)
                        == Some("logos.storage.download")
                    && probe
                        .pointer("/value/contract/version")
                        .and_then(serde_json::Value::as_u64)
                        == Some(2)
                    && probe
                        .pointer("/value/contract/moduleOperationIdOwner")
                        .and_then(serde_json::Value::as_str)
                        == Some("caller")
                    && probe
                        .pointer("/value/contract/cancelTimeoutMs")
                        .and_then(serde_json::Value::as_u64)
                        == Some(15_000)
                    && probe
                        .pointer("/value/contract/maxDownloadBytes")
                        .and_then(serde_json::Value::as_u64)
                        .is_some_and(|max_bytes| max_bytes >= SETTINGS_BACKUP_MAX_BYTES as u64)
                    && storage_backup_event_transport_supported(probe, transport)
            })
        })
}

fn storage_backup_event_transport_supported(
    probe: &serde_json::Value,
    transport: StorageBackupDownloadTransport,
) -> bool {
    match transport {
        StorageBackupDownloadTransport::BasecampHost => {
            probe
                .pointer("/value/event_transport/protocol")
                .and_then(serde_json::Value::as_str)
                == Some("basecamp.host-events")
                && probe
                    .pointer("/value/event_transport/version")
                    .and_then(serde_json::Value::as_u64)
                    == Some(1)
                && probe
                    .pointer("/value/event_transport/ready")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                && probe
                    .pointer("/value/event_transport/module")
                    .and_then(serde_json::Value::as_str)
                    == Some("storage_module")
                && probe
                    .pointer("/value/event_transport/event")
                    .and_then(serde_json::Value::as_str)
                    == Some("storageDownloadDoneV2")
        }
        StorageBackupDownloadTransport::LogoscoreWatch => {
            probe
                .pointer("/value/watch_protocol/protocol")
                .and_then(serde_json::Value::as_str)
                == Some("logoscore.watch")
                && probe
                    .pointer("/value/watch_protocol/version")
                    .and_then(serde_json::Value::as_u64)
                    == Some(1)
                && probe
                    .pointer("/value/watch_protocol/ready")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
        }
    }
}

fn storage_module_info_value(report: &serde_json::Value) -> Option<&serde_json::Value> {
    let module_info = report.get("module_info")?;
    if module_info.get("ok").is_some()
        && module_info.get("ok").and_then(serde_json::Value::as_bool) != Some(true)
    {
        return None;
    }
    [
        module_info.pointer("/value/value"),
        module_info.get("value"),
        Some(module_info),
    ]
    .into_iter()
    .flatten()
    .find(|value| value.get("methods").is_some())
}

fn delivery_state(
    inputs: &CapabilityRuntimeInputs,
    connector: &ResolvedConnector,
    sub_capabilities: &[&str],
) -> CapabilityState {
    let Some(adapter) = network_adapter_policy_for_connector(&connector.id) else {
        return unavailable_state(
            sub_capabilities,
            format!("connector `{}` does not provide Delivery", connector.id),
        );
    };
    if adapter.inputs.iter().any(|input| input.required)
        && inputs.endpoint_for("delivery", connector).is_empty()
    {
        return input_required_state(
            sub_capabilities,
            "Delivery adapter endpoint is required".to_owned(),
        );
    }
    let state = adapter_constrained_state(
        source_report_state(inputs, "delivery", "Delivery", sub_capabilities),
        connector,
        sub_capabilities,
    );
    if connector.id != "direct_delivery_rest" || inputs.messaging_mutating_diagnostics_enabled {
        return state;
    }
    merge_state_constraints(
        state,
        vec![
            "delivery.subscribe".to_owned(),
            "delivery.unsubscribe".to_owned(),
            "delivery.send".to_owned(),
            "delivery.node.start".to_owned(),
            "delivery.node.stop".to_owned(),
        ],
        vec!["Delivery mutating diagnostics are disabled".to_owned()],
        Vec::new(),
    )
}

fn adapter_constrained_state(
    state: CapabilityState,
    connector: &ResolvedConnector,
    sub_capabilities: &[&str],
) -> CapabilityState {
    let Some(adapter) = network_adapter_policy_for_connector(&connector.id) else {
        return state;
    };
    let unavailable = sub_capabilities
        .iter()
        .filter(|capability| !adapter.capabilities.contains(capability))
        .map(|capability| (*capability).to_owned())
        .collect::<Vec<_>>();
    let warnings = if unavailable.is_empty() {
        Vec::new()
    } else {
        vec![format!(
            "{} adapter does not implement every capability",
            connector.id
        )]
    };
    merge_state_constraints(state, unavailable, warnings, Vec::new())
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

fn scope_label(scope: &str) -> &'static str {
    match scope {
        "l1" => "L1 RPC",
        "storage" => "storage REST",
        "delivery" => "delivery REST",
        _ => "capability",
    }
}
