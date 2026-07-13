use crate::source_routing::{SourceProbeKey, shared::plan::ModuleProbeStep};

pub(crate) fn delivery_module_probe_plan(info_id: Option<&str>) -> Vec<ModuleProbeStep<'_>> {
    let mut steps = vec![
        ModuleProbeStep::keyed("version", SourceProbeKey::DeliveryVersion),
        ModuleProbeStep::keyed(
            "getAvailableNodeInfoIDs",
            SourceProbeKey::DeliveryAvailableNodeInfoIds,
        ),
        ModuleProbeStep::keyed(
            "getAvailableConfigs",
            SourceProbeKey::DeliveryAvailableConfigs,
        ),
        ModuleProbeStep::keyed(
            "collectOpenMetricsText",
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
        steps.push(ModuleProbeStep::keyed_with_args(
            "getNodeInfo",
            vec![info_id],
            key,
        ));
    }
    if let Some(info_id) = info_id.map(str::trim).filter(|info_id| !info_id.is_empty()) {
        match delivery_node_info_probe_key(info_id) {
            Some(key) => steps.push(ModuleProbeStep::keyed_with_args(
                "getNodeInfo",
                vec![info_id],
                key,
            )),
            None => steps.push(ModuleProbeStep::unkeyed("getNodeInfo", vec![info_id])),
        }
    }
    steps
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_plan_keys_known_node_info_steps() {
        let steps = delivery_module_probe_plan(Some("MyPeerId"));
        let peer_id_steps = steps
            .iter()
            .filter(|step| step.key == Some(SourceProbeKey::DeliveryMyPeerId))
            .collect::<Vec<_>>();

        assert_eq!(peer_id_steps.len(), 2);
        assert!(
            peer_id_steps
                .iter()
                .all(|step| step.method == "getNodeInfo" && step.args == ["MyPeerId"])
        );
    }

    #[test]
    fn delivery_plan_leaves_unknown_node_info_unkeyed() {
        let steps = delivery_module_probe_plan(Some("Unknown"));
        let custom = steps.last();

        assert!(custom.is_some(), "missing custom node info probe");
        let Some(custom) = custom else {
            return;
        };
        assert_eq!(custom.method, "getNodeInfo");
        assert_eq!(custom.args, ["Unknown"]);
        assert_eq!(custom.key, None);
    }
}
