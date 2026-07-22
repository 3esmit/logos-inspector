use serde_json::Value;

use crate::source_routing::{SourceProbeKey, shared::plan::ModuleProbeStep};

pub(crate) fn delivery_module_probe_plan(
    info_id: Option<&str>,
    runtime_diagnostics_enabled: bool,
    runtime_metrics_enabled: bool,
) -> Vec<ModuleProbeStep<'_>> {
    if !runtime_diagnostics_enabled {
        if runtime_metrics_enabled {
            return vec![ModuleProbeStep::keyed(
                "collectOpenMetricsText",
                SourceProbeKey::DeliveryCollectOpenMetricsText,
            )];
        }
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

pub(crate) fn delivery_advertised_identity_probe_plan(
    available_info_ids: &Value,
) -> Vec<ModuleProbeStep<'static>> {
    [
        ("MyPeerId", SourceProbeKey::DeliveryMyPeerId),
        ("MyENR", SourceProbeKey::DeliveryMyEnr),
        ("MyMultiaddresses", SourceProbeKey::DeliveryMyMultiaddresses),
    ]
    .into_iter()
    .filter(|(info_id, _key)| delivery_info_id_advertised(available_info_ids, info_id))
    .map(|(info_id, key)| ModuleProbeStep::keyed_with_args("getNodeInfo", vec![info_id], key))
    .collect()
}

fn delivery_info_id_advertised(available_info_ids: &Value, info_id: &str) -> bool {
    match available_info_ids {
        Value::String(value) => value
            .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .any(|candidate| candidate == info_id),
        Value::Array(values) => values
            .iter()
            .filter_map(Value::as_str)
            .any(|candidate| candidate == info_id),
        _ => false,
    }
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
        let steps = delivery_module_probe_plan(Some("MyPeerId"), true, false);
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
    fn delivery_plan_defers_default_identity_until_availability_is_known() {
        let steps = delivery_module_probe_plan(None, true, false);

        assert!(steps.iter().all(|step| step.method != "getNodeInfo"));
    }

    #[test]
    fn delivery_identity_plan_uses_only_advertised_node_info() {
        let steps = delivery_advertised_identity_probe_plan(&serde_json::json!(
            "@[Version, MyMultiaddresses, MyPeerId]"
        ));
        let identity_steps = steps
            .iter()
            .map(|step| (step.args.first().copied(), step.key))
            .collect::<Vec<_>>();

        assert_eq!(
            identity_steps,
            vec![
                (Some("MyPeerId"), Some(SourceProbeKey::DeliveryMyPeerId)),
                (
                    Some("MyMultiaddresses"),
                    Some(SourceProbeKey::DeliveryMyMultiaddresses)
                ),
            ]
        );
    }

    #[test]
    fn delivery_identity_plan_accepts_array_and_rejects_partial_names() {
        let steps =
            delivery_advertised_identity_probe_plan(&serde_json::json!(["NotMyPeerId", "MyENR"]));

        assert_eq!(steps.len(), 1);
        assert_eq!(
            steps.first().and_then(|step| step.args.first()).copied(),
            Some("MyENR")
        );
        assert_eq!(
            steps.first().and_then(|step| step.key),
            Some(SourceProbeKey::DeliveryMyEnr)
        );
    }

    #[test]
    fn delivery_plan_leaves_unknown_node_info_unkeyed() {
        let steps = delivery_module_probe_plan(Some("Unknown"), true, false);
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
        let steps = delivery_module_probe_plan(Some("MyPeerId"), false, false);

        assert!(steps.is_empty());
    }

    #[test]
    fn delivery_metrics_plan_probes_only_openmetrics() {
        let steps = delivery_module_probe_plan(Some("MyPeerId"), false, true);

        assert_eq!(steps.len(), 1);
        assert_eq!(
            steps.first().map(|step| step.method),
            Some("collectOpenMetricsText")
        );
        assert_eq!(
            steps.first().and_then(|step| step.key),
            Some(SourceProbeKey::DeliveryCollectOpenMetricsText)
        );
        assert!(steps.first().is_some_and(|step| step.args.is_empty()));
    }
}
