#[test]
fn primary_domain_modules_expose_boundaries() {
    let _ = logos_inspector::bridge::InspectorBridge::error_json("probe");
    let _ = logos_inspector::logoscore::status;
    let _ = logos_inspector::program_decode::spel_idl_report;
    fn accepts_program_info(_: Option<logos_inspector::ProgramFileInfo>) {}
    accepts_program_info(None);
    let _ = logos_inspector::probe::ProbeReport::ok("label", "source", true);
    let _ = logos_inspector::wallet::LOCAL_WALLET_HOME_ENV;
}

struct PublicModuleTransport;

impl logos_inspector::module_transport::ModuleTransport for PublicModuleTransport {
    fn kind(&self) -> logos_inspector::module_transport::ModuleTransportKind {
        logos_inspector::module_transport::ModuleTransportKind::Module
    }

    fn call(
        &self,
        call: logos_inspector::module_transport::ModuleCall,
    ) -> logos_inspector::module_transport::ModuleCallFuture<'_> {
        Box::pin(async move {
            Ok(logos_inspector::module_transport::ModuleCallReply::new(
                logos_inspector::module_transport::ModuleTransportKind::Module,
                serde_json::json!({ "method": call.method() }),
            ))
        })
    }
}

#[test]
fn module_transport_port_is_publicly_composable() {
    assert!(
        logos_inspector::bridge::InspectorBridge::with_module_transport(PublicModuleTransport)
            .is_ok()
    );
}

#[test]
fn source_routing_exposes_adapter_boundaries() {
    let _ = logos_inspector::source_routing::network_profiles;
    let _ = logos_inspector::source_routing::resolve_network_endpoints;
    let _ = logos_inspector::source_routing::source_policy_report;
    let _ = logos_inspector::source_routing::delivery_source_report;
    let _ = logos_inspector::source_routing::storage_source_report;
    let _ = logos_inspector::source_routing::channel_sources::load_channel_source_configs;
    let _ = logos_inspector::source_routing::channel_sources::ChannelSourceTarget::Module {
        module_id: "lez_core".to_owned(),
    };
    fn accepts_channel_monitor(
        _: Option<logos_inspector::source_routing::channel_sources::ChannelSourceMonitor>,
    ) {
    }
    accepts_channel_monitor(None);
}

#[test]
fn sequencer_attestation_basis_is_publicly_composable() {
    use logos_inspector::{
        inspection::NetworkScope,
        source_routing::channel_sources::{
            FinalizedL1EvidenceBasis, SequencerAttestationBasis, SequencerAttestationReceipt,
        },
    };

    let _ = SequencerAttestationBasis::RpcReported {};
    let _ = SequencerAttestationBasis::UserTrustedFinalizedL1Evidence(Box::new(
        FinalizedL1EvidenceBasis {
            network_scope: NetworkScope::GenesisId {
                genesis_id: "a".repeat(64),
            },
            catalog_source_fingerprint: format!("sha256:{}", "b".repeat(64)),
            l1_slot: 1,
            l1_block_id: "c".repeat(64),
            transaction_hash: "d".repeat(64),
            operation_index: 0,
            l2_block_id: 2,
            l2_header_hash: "e".repeat(64),
            l2_signature: "f".repeat(128),
        },
    ));
    let _ = SequencerAttestationReceipt {
        channel_id: "a".repeat(64),
        target_fingerprint: format!("sha256:{}", "b".repeat(64)),
        attested_at_unix: 1,
        basis: SequencerAttestationBasis::RpcReported {},
    };
}

#[test]
fn source_routing_mutation_bypasses_remain_retired() {
    let facade = include_str!("../src/source_routing/channel_sources.rs");
    let store = include_str!("../src/source_routing/channel_sources/store.rs");
    let retired_functions = [
        ["apply_channel_source_", "config"].concat(),
        ["record_sequencer_", "attestation"].concat(),
    ];

    for function in retired_functions {
        assert!(
            !facade.contains(&function),
            "retired mutation bypass `{function}` was re-exported"
        );
        assert!(
            !store.contains(&format!("pub fn {function}")),
            "retired mutation bypass `{function}` was made public"
        );
    }
}

#[test]
fn network_inspection_exposes_zone_boundaries() {
    let _ = logos_inspector::inspection::zones::classify_zone;
    let _ = logos_inspector::inspection::classify_zone;
    let _ = logos_inspector::inspection::classify_catalog_zone;
    let _ = logos_inspector::inspection::project_catalog_zones;
    let _ = logos_inspector::inspection::sources::project_zone_sources;
    let _ = logos_inspector::inspection::ZoneKind::Unknown;
    let _fact_gates = logos_inspector::inspection::ZoneFactGates {
        presence_facts: false,
        point_snapshot_facts: false,
        replay_facts: false,
        absence_facts: false,
    };
    let _schema = logos_inspector::inspection::catalog::CatalogSchemaMetadata::current();
    let _ = logos_inspector::inspection::catalog::CatalogInvalidationReason::RecordDecode;
    fn accepts_catalog(_: Option<logos_inspector::inspection::catalog::ZoneCatalog>) {}
    accepts_catalog(None);
}

#[test]
fn root_models_and_network_helpers_still_compile() {
    let _ = logos_inspector::decode::spel_idl_report;
    let _ = logos_inspector::idl_decode::spel_idl_report;
    let _ = logos_inspector::network_profiles;
    let _ = logos_inspector::resolve_network_endpoints;
    let _ = logos_inspector::network::network_profiles;
    let _ = logos_inspector::summarize_channel_operations;
    let _ = logos_inspector::summarize_transaction;
    fn accepts_rpc_report(_: Option<logos_inspector::RawRpcReport>) {}
    accepts_rpc_report(None);
}
