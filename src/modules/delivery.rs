use crate::modules::logos_core::{ModuleTransportKind, SharedModuleTransport};
use crate::source_routing::delivery_module_probe_plan;

use super::base::{
    DELIVERY_MODULE, ModuleReport, call_probe, call_source_probe, module_info_probe, optional,
    unavailable_metadata_probe,
};

pub async fn delivery_report(
    module_transport: &SharedModuleTransport,
    adapter: ModuleTransportKind,
    info_id: Option<&str>,
) -> ModuleReport {
    let mut probes = Vec::new();
    for step in delivery_module_probe_plan(optional(info_id)) {
        probes.push(match step.key {
            Some(key) => {
                call_source_probe(
                    module_transport,
                    adapter,
                    DELIVERY_MODULE,
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
                    DELIVERY_MODULE,
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
                    == Some(crate::source_routing::SourceProbeKey::DeliveryVersion.as_str())
            })
            .cloned()
            .unwrap_or_else(|| unavailable_metadata_probe(adapter, DELIVERY_MODULE)),
        ModuleTransportKind::LogoscoreCli => {
            module_info_probe(module_transport, adapter, DELIVERY_MODULE).await
        }
    };
    ModuleReport::new(adapter, DELIVERY_MODULE, module_info, probes)
}
