use crate::modules::logos_core::{ModuleTransportKind, SharedModuleTransport};
use crate::source_routing::storage_module_probe_plan;

use super::base::{
    ModuleReport, STORAGE_MODULE, call_probe, call_source_probe, module_info_probe,
    unavailable_metadata_probe,
};

pub async fn storage_report(
    module_transport: &SharedModuleTransport,
    adapter: ModuleTransportKind,
    cid: Option<&str>,
    privileged_debug_enabled: bool,
) -> ModuleReport {
    let mut probes = Vec::new();
    for step in storage_module_probe_plan(cid, privileged_debug_enabled) {
        probes.push(match step.key {
            Some(key) => {
                call_source_probe(
                    module_transport,
                    adapter,
                    STORAGE_MODULE,
                    step.method,
                    &step.args,
                    key,
                )
                .await
            }
            None => {
                call_probe(
                    module_transport,
                    adapter,
                    STORAGE_MODULE,
                    step.method,
                    &step.args,
                )
                .await
            }
        });
    }
    let module_info = match adapter {
        ModuleTransportKind::Module => probes
            .iter()
            .find(|probe| {
                probe.probe_key.as_deref()
                    == Some(crate::source_routing::SourceProbeKey::StorageModuleVersion.as_str())
            })
            .cloned()
            .unwrap_or_else(|| unavailable_metadata_probe(adapter, STORAGE_MODULE)),
        ModuleTransportKind::LogoscoreCli => {
            module_info_probe(module_transport, adapter, STORAGE_MODULE).await
        }
    };
    ModuleReport::new(adapter, STORAGE_MODULE, module_info, probes)
}
