use anyhow::Context as _;

use crate::{
    ProbeReport,
    modules::logos_core::{ModuleTransportKind, SharedModuleTransport},
    source_routing::{SourceProbeKey, storage_module_probe_plan},
};

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
    if adapter == ModuleTransportKind::LogoscoreCli {
        probes.push(logoscore_backup_download_readiness_probe(module_transport).await);
    }
    ModuleReport::new(adapter, STORAGE_MODULE, module_info, probes)
}

async fn logoscore_backup_download_readiness_probe(
    module_transport: &SharedModuleTransport,
) -> ProbeReport {
    let result = match module_transport.logoscore_cli_transport() {
        Some(transport) => {
            let runtime = transport.runtime();
            tokio::task::spawn_blocking(move || runtime.storage_backup_download_readiness())
                .await
                .context("Storage backup readiness worker failed")
                .and_then(|result| result)
        }
        None => Err(anyhow::anyhow!(
            "active LogosCore CLI transport does not expose its runtime"
        )),
    };
    ProbeReport::from_result(
        "storage backup download readiness",
        "logoscore watch storage_module --event storageDownloadDoneV2 --json --watch-protocol v1",
        result,
    )
    .with_probe_key(SourceProbeKey::StorageBackupDownloadReadiness.as_str())
}
