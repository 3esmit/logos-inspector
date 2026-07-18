use crate::ProbeReport;
use crate::modules::logos_core::{ModuleTransportKind, SharedModuleTransport};
use crate::source_routing::{
    ModuleProbeStep, SourceProbeKey, delivery_advertised_health_identity_probe_plan,
    delivery_advertised_identity_probe_plan, delivery_module_probe_plan,
};

use super::base::{
    DELIVERY_MODULE, ModuleReport, call_probe, call_source_probe, module_info_probe, optional,
    unavailable_metadata_probe,
};

async fn delivery_probe(
    module_transport: &SharedModuleTransport,
    adapter: ModuleTransportKind,
    step: &ModuleProbeStep<'_>,
) -> ProbeReport {
    match step.key {
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
    }
}

pub async fn delivery_report(
    module_transport: &SharedModuleTransport,
    adapter: ModuleTransportKind,
    info_id: Option<&str>,
    runtime_diagnostics_enabled: bool,
) -> ModuleReport {
    delivery_report_with_identity_binding(
        module_transport,
        adapter,
        info_id,
        runtime_diagnostics_enabled,
        false,
        false,
    )
    .await
}

pub(crate) async fn delivery_report_with_identity_binding(
    module_transport: &SharedModuleTransport,
    adapter: ModuleTransportKind,
    info_id: Option<&str>,
    runtime_diagnostics_enabled: bool,
    runtime_metrics_enabled: bool,
    health_identity_required: bool,
) -> ModuleReport {
    let mut probes = Vec::new();
    for step in delivery_module_probe_plan(
        optional(info_id),
        runtime_diagnostics_enabled,
        runtime_metrics_enabled,
        health_identity_required,
    ) {
        probes.push(delivery_probe(module_transport, adapter, &step).await);
    }
    let identity_steps = probes
        .iter()
        .find(|probe| {
            probe.probe_key.as_deref()
                == Some(SourceProbeKey::DeliveryAvailableNodeInfoIds.as_str())
        })
        .and_then(|probe| probe.value.as_ref())
        .map(|available| {
            if runtime_diagnostics_enabled {
                delivery_advertised_identity_probe_plan(available)
            } else if health_identity_required {
                delivery_advertised_health_identity_probe_plan(available)
            } else {
                Vec::new()
            }
        })
        .unwrap_or_default();
    for step in identity_steps {
        let already_probed = step.key.is_some_and(|key| {
            probes
                .iter()
                .any(|probe| probe.probe_key.as_deref() == Some(key.as_str()))
        });
        if !already_probed {
            probes.push(delivery_probe(module_transport, adapter, &step).await);
        }
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

    struct AdvertisedIdentityTransport {
        identity_calls: Arc<AtomicUsize>,
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

    impl ModuleTransport for AdvertisedIdentityTransport {
        fn kind(&self) -> ModuleTransportKind {
            ModuleTransportKind::Module
        }

        fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
            let value = match call.method() {
                "version" => json!("1.0.0"),
                "getAvailableNodeInfoIDs" => json!("@[Version, MyPeerId, MyENR]"),
                "collectOpenMetricsText" => json!("libp2p_peers 1\n"),
                "getNodeInfo" => {
                    self.identity_calls.fetch_add(1, Ordering::Relaxed);
                    json!("identity value")
                }
                _ => json!({}),
            };
            Box::pin(async move { Ok(ModuleCallReply::new(ModuleTransportKind::Module, value)) })
        }

        fn module_info(&self, _module: String) -> ModuleDiagnosticFuture<'_> {
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

    #[tokio::test]
    async fn metrics_only_report_dispatches_one_runtime_call() -> Result<()> {
        let calls = Arc::new(AtomicUsize::new(0));
        let module_info_calls = Arc::new(AtomicUsize::new(0));
        let transport: SharedModuleTransport = Arc::new(RecordingTransport {
            calls: Arc::clone(&calls),
            module_info_calls: Arc::clone(&module_info_calls),
        });

        let report = delivery_report_with_identity_binding(
            &transport,
            ModuleTransportKind::Module,
            Some("MyPeerId"),
            false,
            true,
            true,
        )
        .await;

        if calls.load(Ordering::Relaxed) != 1 {
            bail!("metrics-only Delivery report did not dispatch exactly one module call");
        }
        if module_info_calls.load(Ordering::Relaxed) != 0 {
            bail!("metrics-only Delivery report queried module metadata");
        }
        if report.probes.len() != 1
            || report
                .probes
                .first()
                .and_then(|probe| probe.probe_key.as_deref())
                != Some(SourceProbeKey::DeliveryCollectOpenMetricsText.as_str())
        {
            bail!("metrics-only Delivery report returned unexpected probes");
        }
        Ok(())
    }

    #[tokio::test]
    async fn full_report_probes_advertised_identity_without_duplicates() {
        let identity_calls = Arc::new(AtomicUsize::new(0));
        let transport: SharedModuleTransport = Arc::new(AdvertisedIdentityTransport {
            identity_calls: Arc::clone(&identity_calls),
        });

        let report = delivery_report(
            &transport,
            ModuleTransportKind::Module,
            Some("MyPeerId"),
            true,
        )
        .await;
        let peer_id_probes = report
            .probes
            .iter()
            .filter(|probe| {
                probe.probe_key.as_deref() == Some(SourceProbeKey::DeliveryMyPeerId.as_str())
            })
            .count();
        let enr_probes = report
            .probes
            .iter()
            .filter(|probe| {
                probe.probe_key.as_deref() == Some(SourceProbeKey::DeliveryMyEnr.as_str())
            })
            .count();
        let multiaddress_probes = report
            .probes
            .iter()
            .filter(|probe| {
                probe.probe_key.as_deref()
                    == Some(SourceProbeKey::DeliveryMyMultiaddresses.as_str())
            })
            .count();

        assert_eq!(identity_calls.load(Ordering::Relaxed), 2);
        assert_eq!(peer_id_probes, 1);
        assert_eq!(enr_probes, 1);
        assert_eq!(multiaddress_probes, 0);
    }

    #[tokio::test]
    async fn reduced_health_report_probes_only_advertised_enr() {
        let identity_calls = Arc::new(AtomicUsize::new(0));
        let transport: SharedModuleTransport = Arc::new(AdvertisedIdentityTransport {
            identity_calls: Arc::clone(&identity_calls),
        });

        let report = delivery_report_with_identity_binding(
            &transport,
            ModuleTransportKind::Module,
            None,
            false,
            false,
            true,
        )
        .await;
        let keys = report
            .probes
            .iter()
            .filter_map(|probe| probe.probe_key.as_deref())
            .collect::<Vec<_>>();

        assert_eq!(identity_calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            keys,
            [
                SourceProbeKey::DeliveryAvailableNodeInfoIds.as_str(),
                SourceProbeKey::DeliveryMyEnr.as_str(),
            ]
        );
    }
}
