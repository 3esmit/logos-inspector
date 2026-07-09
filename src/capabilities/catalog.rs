use super::{
    CapabilityBuildMode, CapabilityConnectorScopeReport, CapabilityProviderInstanceReport,
    CapabilityProviderTypeReport,
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
            key: "local_control",
            label: "Local control",
        },
        CapabilityProviderTypeReport {
            key: "module_diagnostics",
            label: "Module diagnostics",
        },
    ]
}

pub(super) fn provider_instances() -> &'static [CapabilityProviderInstanceReport] {
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

pub(super) fn connector_scopes(
    build_mode: CapabilityBuildMode,
) -> Vec<CapabilityConnectorScopeReport> {
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

pub(super) fn provider_instance_known(connector: &str) -> bool {
    provider_instances()
        .iter()
        .any(|provider| provider.id == connector)
}

pub(super) fn provider_instance_supports(connector: &str, capability_key: &str) -> bool {
    provider_instances()
        .iter()
        .any(|provider| provider.id == connector && provider.capabilities.contains(&capability_key))
}

pub(super) fn default_connector(
    build_mode: CapabilityBuildMode,
    capability_key: &str,
) -> &'static str {
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
