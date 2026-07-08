use crate::source_routing::{DeliverySourceReportKind, SourceProbeKey, SourceReportBuilder};

use super::base::{
    DELIVERY_MODULE, ModuleReport, call_probe, call_source_probe, module_info_probe, optional,
};

pub fn delivery_report(info_id: Option<&str>) -> ModuleReport {
    let mut probes = vec![
        call_source_probe(
            DELIVERY_MODULE,
            "version",
            &[],
            SourceProbeKey::DeliveryVersion,
        ),
        call_source_probe(
            DELIVERY_MODULE,
            "getAvailableNodeInfoIDs",
            &[],
            SourceProbeKey::DeliveryAvailableNodeInfoIds,
        ),
        call_source_probe(
            DELIVERY_MODULE,
            "getAvailableConfigs",
            &[],
            SourceProbeKey::DeliveryAvailableConfigs,
        ),
        call_source_probe(
            DELIVERY_MODULE,
            "collectOpenMetricsText",
            &[],
            SourceProbeKey::DeliveryCollectOpenMetricsText,
        ),
    ];
    for (info_id, key) in [
        ("Version", SourceProbeKey::DeliveryNodeInfoVersion),
        ("Metrics", SourceProbeKey::DeliveryNodeInfoMetrics),
        ("MyMultiaddresses", SourceProbeKey::DeliveryMyMultiaddresses),
        ("MyENR", SourceProbeKey::DeliveryMyEnr),
        ("MyPeerId", SourceProbeKey::DeliveryMyPeerId),
    ] {
        probes.push(call_source_probe(
            DELIVERY_MODULE,
            "getNodeInfo",
            &[info_id],
            key,
        ));
    }
    if let Some(info_id) = optional(info_id) {
        let probe = call_probe(DELIVERY_MODULE, "getNodeInfo", &[info_id]);
        probes.push(match delivery_node_info_probe_key(info_id) {
            Some(key) => probe.with_probe_key(key.as_str()),
            None => probe,
        });
    }
    let module_info = module_info_probe(DELIVERY_MODULE);
    SourceReportBuilder::delivery(
        DELIVERY_MODULE,
        DeliverySourceReportKind::Module,
        module_info,
    )
    .with_probes(probes)
    .finish()
}

fn delivery_node_info_probe_key(info_id: &str) -> Option<SourceProbeKey> {
    match info_id {
        "Version" => Some(SourceProbeKey::DeliveryNodeInfoVersion),
        "Metrics" => Some(SourceProbeKey::DeliveryNodeInfoMetrics),
        "MyMultiaddresses" => Some(SourceProbeKey::DeliveryMyMultiaddresses),
        "MyENR" => Some(SourceProbeKey::DeliveryMyEnr),
        "MyPeerId" => Some(SourceProbeKey::DeliveryMyPeerId),
        _ => None,
    }
}
