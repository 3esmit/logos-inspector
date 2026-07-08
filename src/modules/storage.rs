use crate::source_routing::{
    SourceReport, SourceReportBuilder, StorageSourceReportKind, storage_module_probe_plan,
};

use super::base::{STORAGE_MODULE, call_probe, call_source_probe, module_info_probe};

pub fn storage_report(cid: Option<&str>, privileged_debug_enabled: bool) -> SourceReport {
    let probes = storage_module_probe_plan(cid, privileged_debug_enabled)
        .into_iter()
        .map(|step| match step.key {
            Some(key) => call_source_probe(STORAGE_MODULE, step.method, &step.args, key),
            None => call_probe(STORAGE_MODULE, step.method, &step.args),
        })
        .collect();
    let module_info = module_info_probe(STORAGE_MODULE);
    SourceReportBuilder::storage(STORAGE_MODULE, StorageSourceReportKind::Module, module_info)
        .with_probes(probes)
        .finish()
}
