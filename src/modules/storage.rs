use crate::source_routing::{SourceProbeKey, SourceReportBuilder, StorageSourceReportKind};

use super::base::{ModuleReport, STORAGE_MODULE, call_source_probe, module_info_probe, optional};

pub fn storage_report(cid: Option<&str>, privileged_debug_enabled: bool) -> ModuleReport {
    let mut probes = vec![
        call_source_probe(
            STORAGE_MODULE,
            "version",
            &[],
            SourceProbeKey::StorageVersion,
        ),
        call_source_probe(
            STORAGE_MODULE,
            "moduleVersion",
            &[],
            SourceProbeKey::StorageModuleVersion,
        ),
        call_source_probe(
            STORAGE_MODULE,
            "dataDir",
            &[],
            SourceProbeKey::StorageDataDir,
        ),
        call_source_probe(STORAGE_MODULE, "peerId", &[], SourceProbeKey::StoragePeerId),
        call_source_probe(STORAGE_MODULE, "spr", &[], SourceProbeKey::StorageSpr),
        call_source_probe(STORAGE_MODULE, "space", &[], SourceProbeKey::StorageSpace),
        call_source_probe(
            STORAGE_MODULE,
            "manifests",
            &[],
            SourceProbeKey::StorageManifests,
        ),
        call_source_probe(
            STORAGE_MODULE,
            "collectMetrics",
            &[],
            SourceProbeKey::StorageCollectMetrics,
        ),
    ];
    if privileged_debug_enabled {
        probes.push(call_source_probe(
            STORAGE_MODULE,
            "debug",
            &[],
            SourceProbeKey::StorageDebug,
        ));
    }
    if let Some(cid) = optional(cid) {
        probes.push(call_source_probe(
            STORAGE_MODULE,
            "exists",
            &[cid],
            SourceProbeKey::StorageExists,
        ));
    }
    let module_info = module_info_probe(STORAGE_MODULE);
    SourceReportBuilder::storage(STORAGE_MODULE, StorageSourceReportKind::Module, module_info)
        .with_probes(probes)
        .finish()
}
