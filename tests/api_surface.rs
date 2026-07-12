#[test]
fn primary_domain_modules_expose_boundaries() {
    let _ = logos_inspector::bridge::InspectorBridge::error_json("probe");
    let _ = logos_inspector::program_decode::spel_idl_report;
    fn accepts_program_info(_: Option<logos_inspector::lez::ProgramFileInfo>) {}
    accepts_program_info(None);
    let _ = logos_inspector::local_nodes::is_default_local_indexer_endpoint;
    let _ = logos_inspector::overview::inspector_scopes;
    let _ = logos_inspector::probe::ProbeReport::ok("label", "source", true);
    let _ = logos_inspector::blockchain::logos_node_cryptarchia_info;
    let _ = logos_inspector::rpc::raw_rpc_report;
    let _ = logos_inspector::wallet::LOCAL_WALLET_HOME_ENV;
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
fn network_inspection_exposes_legacy_and_zone_boundaries() {
    let _ = logos_inspector::inspection::l1::channels::channel_scan;
    let _ = logos_inspector::inspection::l2::lez::sequencer_health;
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
fn legacy_root_and_network_shims_still_compile() {
    let _ = logos_inspector::decode::spel_idl_report;
    let _ = logos_inspector::idl_decode::spel_idl_report;
    let _ = logos_inspector::network_profiles;
    let _ = logos_inspector::resolve_network_endpoints;
    let _ = logos_inspector::network::network_profiles;
    let _ = logos_inspector::logos_node_cryptarchia_info;
    let _ = logos_inspector::raw_rpc_report;
    let _ = logos_inspector::sequencer_health;
}
