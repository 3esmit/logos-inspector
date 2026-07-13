use super::{
    CapabilityBuildMode, CapabilityConnectorScopeReport, CapabilityProviderInstanceReport,
    CapabilityProviderTypeReport,
};
use crate::source_routing::{
    AdapterConnectionType, SourceModePolicy, capability_provider_mode_policies,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct CapabilitySpec {
    pub(super) key: &'static str,
    pub(super) label: &'static str,
    pub(super) sub_capabilities: &'static [&'static str],
}

pub(super) fn provider_types() -> &'static [CapabilityProviderTypeReport] {
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
            key: "direct_metrics",
            label: "Direct metrics endpoint",
        },
        CapabilityProviderTypeReport {
            key: "network_monitor",
            label: "Network monitor",
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

const COMPOSED_PROVIDERS: &[CapabilityProviderInstanceReport] = &[
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
];

const OPERATION_PROVIDERS: &[CapabilityProviderInstanceReport] = &[
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
];

pub(super) fn provider_instances() -> Vec<CapabilityProviderInstanceReport> {
    let mut providers = Vec::new();
    providers.extend_from_slice(COMPOSED_PROVIDERS);
    providers.extend(capability_provider_mode_policies().map(provider_from_mode));
    providers.extend_from_slice(OPERATION_PROVIDERS);
    providers
}

fn provider_from_mode(mode: &'static SourceModePolicy) -> CapabilityProviderInstanceReport {
    CapabilityProviderInstanceReport {
        id: mode.adapter.connector_id,
        provider_type: provider_type_for(mode.adapter.connection_type),
        label: mode.label,
        module: mode.adapter.module_id,
        endpoint_role: mode.adapter.endpoint_role,
        capabilities: mode.adapter.capability_scopes,
    }
}

const fn provider_type_for(connection_type: AdapterConnectionType) -> &'static str {
    match connection_type {
        AdapterConnectionType::Module => "module",
        AdapterConnectionType::Rpc => "direct_rpc",
        AdapterConnectionType::Rest => "direct_rest",
        AdapterConnectionType::Metrics => "direct_metrics",
        AdapterConnectionType::NetworkMonitor => "network_monitor",
    }
}

pub(super) fn connector_scopes(
    build_mode: CapabilityBuildMode,
) -> Vec<CapabilityConnectorScopeReport> {
    [
        ("network_profile", "l1", "l1_connector", "l1"),
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

pub(super) fn provider_instance_known(connector: &str) -> bool {
    capability_provider_mode_policies().any(|mode| mode.adapter.connector_id == connector)
        || registry_owned_providers().any(|provider| provider.id == connector)
}

pub(super) fn provider_instance_supports(connector: &str, capability_key: &str) -> bool {
    capability_provider_mode_policies().any(|mode| {
        mode.adapter.connector_id == connector
            && mode.adapter.capability_scopes.contains(&capability_key)
    }) || registry_owned_providers()
        .any(|provider| provider.id == connector && provider.capabilities.contains(&capability_key))
}

fn registry_owned_providers() -> impl Iterator<Item = &'static CapabilityProviderInstanceReport> {
    COMPOSED_PROVIDERS.iter().chain(OPERATION_PROVIDERS)
}

pub(super) fn default_connector(
    build_mode: CapabilityBuildMode,
    capability_key: &str,
) -> &'static str {
    match (build_mode, capability_key) {
        (CapabilityBuildMode::Basecamp, "l1" | "wallet.l1") => "blockchain_module",
        (CapabilityBuildMode::Basecamp, "storage") => "storage_module",
        (CapabilityBuildMode::Basecamp, "delivery") => "delivery_module",
        (CapabilityBuildMode::Basecamp, "wallet.l2") => "lez_core",
        (_, "wallet") => "composed_wallet",
        (_, "l1") => "direct_l1_rpc",
        (_, "storage") => "direct_storage_rest",
        (_, "delivery") => "direct_delivery_rest",
        (_, "wallet.l1" | "wallet.l2") => "composed_wallet",
        (_, "local_nodes") => "local_node_control",
        (_, "diagnostics") => "module_diagnostics_metrics",
        _ => "unconfigured",
    }
}

pub(super) fn capability_specs() -> &'static [CapabilitySpec] {
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
            key: "storage",
            label: "Storage",
            sub_capabilities: &[
                "storage.identity.read",
                "storage.space.read",
                "storage.manifests.read",
                "storage.metrics.read",
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
                "delivery.metrics.read",
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
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn provider_instances_derive_node_owned_adapter_facts() {
        let providers = provider_instances();
        let mut ids = BTreeSet::new();
        for provider in &providers {
            assert!(
                ids.insert(provider.id),
                "duplicate provider `{}`",
                provider.id
            );
        }

        for mode in capability_provider_mode_policies() {
            let provider = providers
                .iter()
                .find(|provider| provider.id == mode.adapter.connector_id);
            assert!(
                provider.is_some(),
                "missing provider `{}`",
                mode.adapter.connector_id
            );
            let Some(provider) = provider else {
                continue;
            };
            assert_eq!(
                provider.provider_type,
                provider_type_for(mode.adapter.connection_type)
            );
            assert_eq!(provider.label, mode.label);
            assert_eq!(provider.module, mode.adapter.module_id);
            assert_eq!(provider.endpoint_role, mode.adapter.endpoint_role);
            assert_eq!(provider.capabilities, mode.adapter.capability_scopes);
        }
    }
}
