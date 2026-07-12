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
    let requires_endpoint = !connector.id.ends_with("_module") && connector.id != "lez_core";
    if requires_endpoint && inputs.endpoint_for(scope, connector).is_empty() {
        return input_required_state(
            sub_capabilities,
            format!("{} endpoint is required", scope_label(scope)),
        );
    }
    source_report_state(inputs, scope, scope_label(scope), sub_capabilities)
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
