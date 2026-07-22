use anyhow::{Context as _, Result};
use serde_json::{Value, json};

use crate::{
    ProbeReport,
    modules::logos_core::{
        ModuleCall, ModuleTransportKind, SharedModuleTransport, dispatch_module_call,
        normalize_module_call_value,
    },
    source_routing::{SourceProbeKey, storage_module_probe_plan},
    support::{
        settings_backup::SETTINGS_BACKUP_MAX_BYTES,
        storage_download_contract::{
            STORAGE_DOWNLOAD_V2_METHOD, is_storage_download_v2_method_signature,
        },
    },
};

use super::base::{
    ModuleReport, STORAGE_MODULE, call_probe, call_source_probe, module_info_probe,
    unavailable_metadata_probe,
};

const STORAGE_DOWNLOAD_PROTOCOL_METHOD: &str = "downloadProtocol";
const STORAGE_DOWNLOAD_DONE_EVENT: &str = "storageDownloadDoneV2";
const BASECAMP_EVENT_TRANSPORT_PROTOCOL: &str = "basecamp.host-events";
const BASECAMP_EVENT_TRANSPORT_VERSION: u64 = 1;

pub async fn storage_report(
    module_transport: &SharedModuleTransport,
    adapter: ModuleTransportKind,
    cid: Option<&str>,
    privileged_debug_enabled: bool,
    runtime_diagnostics_enabled: bool,
) -> ModuleReport {
    let mut probes = Vec::new();
    for step in
        storage_module_probe_plan(cid, privileged_debug_enabled, runtime_diagnostics_enabled)
    {
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
    let metadata = if runtime_diagnostics_enabled {
        module_info_probe(module_transport, adapter, STORAGE_MODULE).await
    } else {
        unavailable_metadata_probe(adapter, STORAGE_MODULE)
    };
    let module_info = match adapter {
        ModuleTransportKind::Module
            if runtime_diagnostics_enabled && storage_module_info_value(&metadata).is_none() =>
        {
            probes
                .iter()
                .find(|probe| {
                    probe.probe_key.as_deref()
                        == Some(SourceProbeKey::StorageModuleVersion.as_str())
                })
                .cloned()
                .unwrap_or_else(|| unavailable_metadata_probe(adapter, STORAGE_MODULE))
        }
        ModuleTransportKind::Module | ModuleTransportKind::LogoscoreCli => metadata.clone(),
    };
    if runtime_diagnostics_enabled {
        match adapter {
            ModuleTransportKind::Module => probes
                .push(basecamp_backup_download_readiness_probe(module_transport, &metadata).await),
            ModuleTransportKind::LogoscoreCli => {
                probes.push(logoscore_backup_download_readiness_probe(module_transport).await);
            }
        }
    }
    ModuleReport::new(adapter, STORAGE_MODULE, module_info, probes)
}

async fn basecamp_backup_download_readiness_probe(
    module_transport: &SharedModuleTransport,
    module_info: &ProbeReport,
) -> ProbeReport {
    ProbeReport::from_result(
        "storage backup download readiness",
        "basecamp host-events storage_module storageDownloadDoneV2",
        basecamp_backup_download_readiness(module_transport, module_info).await,
    )
    .with_probe_key(SourceProbeKey::StorageBackupDownloadReadiness.as_str())
}

async fn basecamp_backup_download_readiness(
    module_transport: &SharedModuleTransport,
    module_info: &ProbeReport,
) -> Result<Value> {
    anyhow::ensure!(
        module_transport.kind() == ModuleTransportKind::Module,
        "Basecamp backup readiness requires the host module transport"
    );
    ensure_storage_backup_download_metadata(module_info)?;

    let call = ModuleCall::new(
        ModuleTransportKind::Module,
        STORAGE_MODULE,
        STORAGE_DOWNLOAD_PROTOCOL_METHOD,
        Vec::new(),
    )?;
    let protocol = dispatch_module_call(module_transport.as_ref(), call)
        .await
        .context("failed to query the Storage backup download protocol")?
        .into_value();
    let protocol =
        normalize_module_call_value(STORAGE_MODULE, STORAGE_DOWNLOAD_PROTOCOL_METHOD, protocol)?;
    ensure_storage_backup_download_protocol(&protocol)?;

    anyhow::ensure!(
        module_transport.supports_shared_file_staging(),
        "Basecamp host transport does not expose shared file staging"
    );
    anyhow::ensure!(
        module_transport.native_runtime_module_events_ready(),
        "Basecamp host transport does not own healthy native runtime module-event ingress"
    );
    let _subscription = module_transport
        .subscribe_module_event(STORAGE_MODULE, STORAGE_DOWNLOAD_DONE_EVENT)
        .context("Basecamp host transport cannot subscribe to Storage download completion")?;

    Ok(json!({
        "contract": protocol,
        "shared_staging": true,
        "event_transport": {
            "protocol": BASECAMP_EVENT_TRANSPORT_PROTOCOL,
            "version": BASECAMP_EVENT_TRANSPORT_VERSION,
            "ready": true,
            "native_runtime_event_owner": true,
            "module": STORAGE_MODULE,
            "event": STORAGE_DOWNLOAD_DONE_EVENT,
        },
    }))
}

fn ensure_storage_backup_download_metadata(module_info: &ProbeReport) -> Result<()> {
    let value = storage_module_info_value(module_info).ok_or_else(|| {
        let detail = module_info
            .error
            .as_deref()
            .unwrap_or("Storage module metadata did not contain callable methods");
        anyhow::anyhow!("Storage module metadata is unavailable: {detail}")
    })?;
    let methods = value
        .get("methods")
        .and_then(Value::as_array)
        .context("Storage module metadata does not contain methods")?;
    let events = value
        .get("events")
        .and_then(Value::as_array)
        .context("Storage module metadata does not contain events")?;
    for (name, signature) in [
        ("downloadProtocol", "downloadProtocol()"),
        ("downloadCancelV2", "downloadCancelV2(QString)"),
    ] {
        anyhow::ensure!(
            methods.iter().any(|method| {
                method.get("name").and_then(Value::as_str) == Some(name)
                    && method.get("signature").and_then(Value::as_str) == Some(signature)
                    && method.get("isInvokable").and_then(Value::as_bool) == Some(true)
            }),
            "Storage module metadata does not expose invokable `{signature}`"
        );
    }
    anyhow::ensure!(
        methods.iter().any(|method| {
            method.get("name").and_then(Value::as_str) == Some(STORAGE_DOWNLOAD_V2_METHOD)
                && method
                    .get("signature")
                    .and_then(Value::as_str)
                    .is_some_and(is_storage_download_v2_method_signature)
                && method.get("isInvokable").and_then(Value::as_bool) == Some(true)
        }),
        "Storage module metadata does not expose an invokable versioned download method"
    );
    anyhow::ensure!(
        events.iter().any(|event| {
            event.get("name").and_then(Value::as_str) == Some(STORAGE_DOWNLOAD_DONE_EVENT)
                && event.get("signature").and_then(Value::as_str)
                    == Some("storageDownloadDoneV2(QString)")
        }),
        "Storage module metadata does not expose `storageDownloadDoneV2(QString)`"
    );
    Ok(())
}

fn storage_module_info_value(module_info: &ProbeReport) -> Option<&Value> {
    if !module_info.ok {
        return None;
    }
    let module_info = module_info.value.as_ref()?;
    [
        module_info.pointer("/value/value"),
        module_info.get("value"),
        Some(module_info),
    ]
    .into_iter()
    .flatten()
    .find(|value| value.get("methods").is_some())
}

fn ensure_storage_backup_download_protocol(protocol: &Value) -> Result<()> {
    anyhow::ensure!(
        protocol.get("protocol").and_then(Value::as_str) == Some("logos.storage.download")
            && protocol.get("version").and_then(Value::as_u64) == Some(2)
            && protocol
                .get("moduleOperationIdOwner")
                .and_then(Value::as_str)
                == Some("caller")
            && protocol.get("cancelTimeoutMs").and_then(Value::as_u64) == Some(15_000)
            && protocol
                .get("maxDownloadBytes")
                .and_then(Value::as_u64)
                .is_some_and(|max_bytes| max_bytes >= SETTINGS_BACKUP_MAX_BYTES as u64),
        "storage_module returned an incompatible backup download protocol"
    );
    Ok(())
}

async fn logoscore_backup_download_readiness_probe(
    module_transport: &SharedModuleTransport,
) -> ProbeReport {
    let result = match module_transport.logoscore_cli_transport() {
        Some(transport) => {
            let runtime = transport.runtime();
            tokio::task::spawn_blocking(move || {
                runtime.and_then(|runtime| runtime.storage_backup_download_readiness())
            })
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

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use anyhow::{Result, bail};

    use super::*;
    use crate::modules::logos_core::{
        BoxedModuleEventSubscription, ModuleCallFuture, ModuleCallReply, ModuleDiagnosticFuture,
        ModuleEventSubscription, ModuleTransport, ModuleTransportEvent, ModuleTransportResult,
    };

    struct EmptySubscription;

    impl ModuleEventSubscription for EmptySubscription {
        fn next_within(&mut self, _timeout: Duration) -> Result<Option<ModuleTransportEvent>> {
            Ok(None)
        }
    }

    struct FakeBasecampTransport {
        module_info: Value,
        module_info_calls: Arc<AtomicUsize>,
        protocol: Value,
        shared_staging: bool,
        subscribable: bool,
        native_events_ready: bool,
    }

    impl ModuleTransport for FakeBasecampTransport {
        fn kind(&self) -> ModuleTransportKind {
            ModuleTransportKind::Module
        }

        fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
            let protocol = self.protocol.clone();
            Box::pin(async move {
                let value = if call.method() == STORAGE_DOWNLOAD_PROTOCOL_METHOD {
                    protocol
                } else {
                    json!({})
                };
                Ok(ModuleCallReply::new(ModuleTransportKind::Module, value))
            })
        }

        fn module_info(&self, _module: String) -> ModuleDiagnosticFuture<'_> {
            self.module_info_calls.fetch_add(1, Ordering::Relaxed);
            let module_info = self.module_info.clone();
            Box::pin(async move { Ok(module_info) })
        }

        fn subscribe_module_event(
            &self,
            _module: &str,
            _event: &str,
        ) -> ModuleTransportResult<BoxedModuleEventSubscription> {
            if !self.subscribable {
                bail!("host event subscription unavailable");
            }
            Ok(Box::new(EmptySubscription))
        }

        fn supports_shared_file_staging(&self) -> bool {
            self.shared_staging
        }

        fn native_runtime_module_events_ready(&self) -> bool {
            self.native_events_ready
        }
    }

    fn module_info_with_download_signature(download_signature: &str) -> Value {
        json!({
            "name": STORAGE_MODULE,
            "methods": [
                {
                    "isInvokable": true,
                    "name": "downloadProtocol",
                    "signature": "downloadProtocol()"
                },
                {
                    "isInvokable": true,
                    "name": "downloadToUrlV2",
                    "signature": download_signature
                },
                {
                    "isInvokable": true,
                    "name": "downloadCancelV2",
                    "signature": "downloadCancelV2(QString)"
                }
            ],
            "events": [{
                "name": STORAGE_DOWNLOAD_DONE_EVENT,
                "signature": "storageDownloadDoneV2(QString)"
            }]
        })
    }

    fn exact_module_info() -> Value {
        module_info_with_download_signature("downloadToUrlV2(QString,QString,bool,int,QString,int)")
    }

    fn exact_protocol() -> Value {
        json!({
            "protocol": "logos.storage.download",
            "version": 2,
            "moduleOperationIdOwner": "caller",
            "cancelTimeoutMs": 15_000,
            "maxDownloadBytes": SETTINGS_BACKUP_MAX_BYTES as u64,
        })
    }

    fn fake_transport() -> FakeBasecampTransport {
        FakeBasecampTransport {
            module_info: exact_module_info(),
            module_info_calls: Arc::new(AtomicUsize::new(0)),
            protocol: exact_protocol(),
            shared_staging: true,
            subscribable: true,
            native_events_ready: true,
        }
    }

    #[tokio::test]
    async fn basecamp_report_advertises_exact_backup_download_readiness() -> Result<()> {
        let transport: SharedModuleTransport = Arc::new(fake_transport());

        let report =
            storage_report(&transport, ModuleTransportKind::Module, None, false, true).await;
        let readiness = report
            .probes
            .iter()
            .find(|probe| {
                probe.probe_key.as_deref()
                    == Some(SourceProbeKey::StorageBackupDownloadReadiness.as_str())
            })
            .ok_or_else(|| anyhow::anyhow!("Basecamp readiness probe missing"))?;

        if storage_module_info_value(&report.module_info).is_none() {
            bail!("Basecamp module metadata was not preserved: {report:?}");
        }
        if !readiness.ok
            || readiness
                .value
                .as_ref()
                .and_then(|value| value.pointer("/event_transport/protocol"))
                .and_then(Value::as_str)
                != Some(BASECAMP_EVENT_TRANSPORT_PROTOCOL)
        {
            bail!("Basecamp backup readiness was not established: {readiness:?}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_report_accepts_universal_versioned_download_metadata() -> Result<()> {
        let transport: SharedModuleTransport = Arc::new(FakeBasecampTransport {
            module_info: module_info_with_download_signature(
                crate::support::storage_download_contract::STORAGE_DOWNLOAD_V2_UNIVERSAL_METHOD_SIGNATURE,
            ),
            ..fake_transport()
        });

        let report =
            storage_report(&transport, ModuleTransportKind::Module, None, false, true).await;
        let readiness = report
            .probes
            .iter()
            .find(|probe| {
                probe.probe_key.as_deref()
                    == Some(SourceProbeKey::StorageBackupDownloadReadiness.as_str())
            })
            .ok_or_else(|| anyhow::anyhow!("Basecamp readiness probe missing"))?;

        if !readiness.ok {
            bail!(
                "universal Storage V2 metadata was not accepted for Basecamp readiness: {readiness:?}"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn metadata_only_report_skips_storage_runtime_probes() -> Result<()> {
        let fake_transport = fake_transport();
        let module_info_calls = Arc::clone(&fake_transport.module_info_calls);
        let transport: SharedModuleTransport = Arc::new(fake_transport);

        let report =
            storage_report(&transport, ModuleTransportKind::Module, None, false, false).await;

        if !report.probes.is_empty() {
            bail!(
                "metadata-only Storage report invoked runtime probes: {:?}",
                report.probes
            );
        }
        if module_info_calls.load(Ordering::Relaxed) != 0 {
            bail!("metadata-only Storage report queried Storage module metadata");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_readiness_fails_closed_on_host_contract_mismatches() -> Result<()> {
        let mut bad_metadata = exact_module_info();
        *bad_metadata
            .get_mut("events")
            .context("fake Storage metadata has no events")? = json!([]);
        let mut bad_protocol = exact_protocol();
        *bad_protocol
            .get_mut("version")
            .context("fake Storage protocol has no version")? = json!(1);
        let cases = [
            (
                FakeBasecampTransport {
                    module_info: bad_metadata,
                    ..fake_transport()
                },
                "metadata",
            ),
            (
                FakeBasecampTransport {
                    protocol: bad_protocol,
                    ..fake_transport()
                },
                "protocol",
            ),
            (
                FakeBasecampTransport {
                    shared_staging: false,
                    ..fake_transport()
                },
                "shared staging",
            ),
            (
                FakeBasecampTransport {
                    subscribable: false,
                    ..fake_transport()
                },
                "event subscription",
            ),
            (
                FakeBasecampTransport {
                    native_events_ready: false,
                    ..fake_transport()
                },
                "native event ownership",
            ),
        ];

        for (transport, mismatch) in cases {
            let module_info =
                ProbeReport::ok("storage_module info", "module", &transport.module_info);
            let transport: SharedModuleTransport = Arc::new(transport);
            let readiness =
                basecamp_backup_download_readiness_probe(&transport, &module_info).await;
            if readiness.ok {
                bail!("{mismatch} mismatch overclaimed Basecamp readiness: {readiness:?}");
            }
        }
        Ok(())
    }
}
