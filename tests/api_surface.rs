#[test]
fn domain_modules_expose_primary_boundaries() {
    let _ = logos_inspector::bridge::InspectorBridge::error_json("probe");
    let _ = logos_inspector::decode::spel_idl_report;
    fn accepts_program_info(_: Option<logos_inspector::lez::ProgramFileInfo>) {}
    accepts_program_info(None);
    let _ = logos_inspector::local_nodes::is_default_local_indexer_endpoint;
    let _ = logos_inspector::network::network_profiles;
    let _ = logos_inspector::overview::inspector_scopes;
    let _ = logos_inspector::probe::ProbeReport::ok("label", "source", true);
    let _ = logos_inspector::rpc::raw_rpc_report;
    let _ = logos_inspector::wallet::LOCAL_WALLET_HOME_ENV;
}
