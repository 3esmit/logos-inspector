use crate::source_routing::{SourceProbeKey, shared::plan::ModuleProbeStep};

pub(crate) fn delivery_module_probe_plan(
    info_id: Option<&str>,
    runtime_diagnostics_enabled: bool,
) -> Vec<ModuleProbeStep<'_>> {
    if !runtime_diagnostics_enabled {
        return Vec::new();
    }
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
    fn delivery_plan_keys_explicit_known_node_info_step() {
        let steps = delivery_module_probe_plan(Some("MyPeerId"), true);
        let peer_id_steps = steps
            .iter()
            .filter(|step| step.key == Some(SourceProbeKey::DeliveryMyPeerId))
            .collect::<Vec<_>>();

        assert_eq!(peer_id_steps.len(), 1);
        assert!(
            peer_id_steps
                .iter()
                .all(|step| step.method == "getNodeInfo" && step.args == ["MyPeerId"])
        );
    }

    #[test]
    fn delivery_plan_does_not_probe_node_info_without_explicit_request() {
        let steps = delivery_module_probe_plan(None, true);

        assert!(steps.iter().all(|step| step.method != "getNodeInfo"));
    }

    #[test]
    fn delivery_plan_leaves_unknown_node_info_unkeyed() {
        let steps = delivery_module_probe_plan(Some("Unknown"), true);
        let custom = steps.last();

        assert!(custom.is_some(), "missing custom node info probe");
        let Some(custom) = custom else {
            return;
        };
        assert_eq!(custom.method, "getNodeInfo");
        assert_eq!(custom.args, ["Unknown"]);
        assert_eq!(custom.key, None);
    }

    #[test]
    fn delivery_plan_defers_runtime_probes_until_explicitly_enabled() {
        let steps = delivery_module_probe_plan(Some("MyPeerId"), false);

        assert!(steps.is_empty());
    }
}
