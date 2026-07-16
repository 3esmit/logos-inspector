use crate::source_routing::{SourceProbeKey, shared::plan::ModuleProbeStep};

pub(crate) fn storage_module_probe_plan<'a>(
    cid: Option<&'a str>,
    privileged_debug_enabled: bool,
    runtime_diagnostics_enabled: bool,
) -> Vec<ModuleProbeStep<'a>> {
    if !runtime_diagnostics_enabled {
        return Vec::new();
    }
    let mut steps = vec![
        ModuleProbeStep::keyed("version", SourceProbeKey::StorageVersion),
        ModuleProbeStep::keyed("moduleVersion", SourceProbeKey::StorageModuleVersion),
        ModuleProbeStep::keyed("dataDir", SourceProbeKey::StorageDataDir),
        ModuleProbeStep::keyed("peerId", SourceProbeKey::StoragePeerId),
        ModuleProbeStep::keyed("spr", SourceProbeKey::StorageSpr),
        ModuleProbeStep::keyed("space", SourceProbeKey::StorageSpace),
        ModuleProbeStep::keyed("manifests", SourceProbeKey::StorageManifests),
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
        let steps = storage_module_probe_plan(Some("cid-1"), true, true);
        let keys = steps.iter().filter_map(|step| step.key).collect::<Vec<_>>();

        assert!(keys.contains(&SourceProbeKey::StorageDebug));
        assert!(keys.contains(&SourceProbeKey::StorageExists));
        assert!(
            steps
                .iter()
                .any(|step| step.method == "exists" && step.args == ["cid-1"])
        );
    }

    #[test]
    fn storage_plan_defers_runtime_probes_until_explicitly_enabled() {
        let steps = storage_module_probe_plan(None, false, false);

        assert!(steps.is_empty());
    }
}
