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
    runtime_diagnostics_enabled: bool,
) -> ModuleReport {
    let mut probes = Vec::new();
    for step in delivery_module_probe_plan(optional(info_id), runtime_diagnostics_enabled) {
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
    let module_info = if !runtime_diagnostics_enabled {
        unavailable_metadata_probe(adapter, DELIVERY_MODULE)
    } else {
        match adapter {
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
        }
    };
    ModuleReport::new(adapter, DELIVERY_MODULE, module_info, probes)
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use anyhow::{Result, bail};
    use serde_json::json;

    use super::*;
    use crate::modules::logos_core::{
        ModuleCall, ModuleCallFuture, ModuleCallReply, ModuleDiagnosticFuture, ModuleTransport,
    };

    struct RecordingTransport {
        calls: Arc<AtomicUsize>,
        module_info_calls: Arc<AtomicUsize>,
    }

    impl ModuleTransport for RecordingTransport {
        fn kind(&self) -> ModuleTransportKind {
            ModuleTransportKind::Module
        }

        fn call(&self, _call: ModuleCall) -> ModuleCallFuture<'_> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Box::pin(async { Ok(ModuleCallReply::new(ModuleTransportKind::Module, json!({}))) })
        }

        fn module_info(&self, _module: String) -> ModuleDiagnosticFuture<'_> {
            self.module_info_calls.fetch_add(1, Ordering::Relaxed);
            Box::pin(async { Ok(json!({})) })
        }
    }

    #[tokio::test]
    async fn metadata_only_report_skips_delivery_runtime_calls() -> Result<()> {
        let calls = Arc::new(AtomicUsize::new(0));
        let module_info_calls = Arc::new(AtomicUsize::new(0));
        let transport: SharedModuleTransport = Arc::new(RecordingTransport {
            calls: Arc::clone(&calls),
            module_info_calls: Arc::clone(&module_info_calls),
        });

        let report = delivery_report(&transport, ModuleTransportKind::Module, None, false).await;

        if !report.probes.is_empty() {
            bail!(
                "metadata-only Delivery report invoked runtime probes: {:?}",
                report.probes
            );
        }
        if calls.load(Ordering::Relaxed) != 0 {
            bail!("metadata-only Delivery report dispatched a module call");
        }
        if module_info_calls.load(Ordering::Relaxed) != 0 {
            bail!("metadata-only Delivery report queried module metadata");
        }
        Ok(())
    }
}
