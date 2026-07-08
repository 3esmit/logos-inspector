use crate::source_routing::{ModuleProbeStep, SourceProbeKey};

pub(crate) fn storage_module_probe_plan<'a>(
    cid: Option<&'a str>,
    privileged_debug_enabled: bool,
) -> Vec<ModuleProbeStep<'a>> {
    let mut steps = vec![
        ModuleProbeStep::keyed("version", SourceProbeKey::StorageVersion),
        ModuleProbeStep::keyed("moduleVersion", SourceProbeKey::StorageModuleVersion),
        ModuleProbeStep::keyed("dataDir", SourceProbeKey::StorageDataDir),
        ModuleProbeStep::keyed("peerId", SourceProbeKey::StoragePeerId),
        ModuleProbeStep::keyed("spr", SourceProbeKey::StorageSpr),
        ModuleProbeStep::keyed("space", SourceProbeKey::StorageSpace),
        ModuleProbeStep::keyed("manifests", SourceProbeKey::StorageManifests),
        ModuleProbeStep::keyed("collectMetrics", SourceProbeKey::StorageCollectMetrics),
    ];
    if privileged_debug_enabled {
        steps.push(ModuleProbeStep::keyed(
            "debug",
            SourceProbeKey::StorageDebug,
        ));
    }
    if let Some(cid) = cid.map(str::trim).filter(|cid| !cid.is_empty()) {
        steps.push(ModuleProbeStep::keyed_with_args(
            "exists",
            vec![cid],
            SourceProbeKey::StorageExists,
        ));
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_plan_includes_optional_debug_and_cid_steps() {
        let steps = storage_module_probe_plan(Some("cid-1"), true);
        let keys = steps.iter().filter_map(|step| step.key).collect::<Vec<_>>();

        assert!(keys.contains(&SourceProbeKey::StorageDebug));
        assert!(keys.contains(&SourceProbeKey::StorageExists));
        assert!(
            steps
                .iter()
                .any(|step| step.method == "exists" && step.args == ["cid-1"])
        );
    }
}
