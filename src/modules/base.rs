use serde::Serialize;
use serde_json::Value;

use crate::{ProbeReport, source_routing::SourceProbeKey};

use super::{
    delivery::delivery_report,
    logos_core::{ModuleCall, ModuleTransportKind, SharedModuleTransport, dispatch_module_call},
    storage::storage_report,
};

pub(super) const BLOCKCHAIN_MODULE: &str = "blockchain_module";
pub(super) const STORAGE_MODULE: &str = "storage_module";
pub(super) const DELIVERY_MODULE: &str = "delivery_module";
const CAPABILITY_MODULE: &str = "capability_module";

#[derive(Debug, Clone, Serialize)]
pub struct LogosModulesReport {
    pub adapter: ModuleTransportKind,
    pub status: ProbeReport,
    pub blockchain: ModuleReport,
    pub storage: ModuleReport,
    pub delivery: ModuleReport,
    pub capabilities: ModuleReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleReport {
    pub adapter: ModuleTransportKind,
    pub module: String,
    pub module_info: ProbeReport,
    pub probes: Vec<ProbeReport>,
}

impl ModuleReport {
    pub(crate) fn new(
        adapter: ModuleTransportKind,
        module: impl Into<String>,
        module_info: ProbeReport,
        probes: Vec<ProbeReport>,
    ) -> Self {
        Self {
            adapter,
            module: module.into(),
            module_info,
            probes,
        }
    }
}

pub async fn logoscore_status_report(module_transport: &SharedModuleTransport) -> ProbeReport {
    let (label, source) = match module_transport.kind() {
        ModuleTransportKind::Module => ("module transport status", "module"),
        ModuleTransportKind::LogoscoreCli => ("logoscore status", "logoscore status --json"),
    };
    ProbeReport::from_result(label, source, module_transport.status().await)
}

pub async fn modules_report(module_transport: &SharedModuleTransport) -> LogosModulesReport {
    let adapter = module_transport.kind();
    LogosModulesReport {
        adapter,
        status: logoscore_status_report(module_transport).await,
        blockchain: blockchain_module_report(module_transport, adapter, None).await,
        storage: storage_report(module_transport, adapter, None, false).await,
        delivery: delivery_report(module_transport, adapter, None).await,
        capabilities: capabilities_report(module_transport, adapter).await,
    }
}

pub async fn blockchain_module_report(
    module_transport: &SharedModuleTransport,
    adapter: ModuleTransportKind,
    address: Option<&str>,
) -> ModuleReport {
    let mut probes = vec![
        call_probe(
            module_transport,
            adapter,
            BLOCKCHAIN_MODULE,
            "get_cryptarchia_info",
            &[],
        )
        .await,
        call_probe(
            module_transport,
            adapter,
            BLOCKCHAIN_MODULE,
            "wallet_get_known_addresses",
            &[],
        )
        .await,
    ];
    if let Some(address) = optional(address) {
        probes.push(
            call_probe(
                module_transport,
                adapter,
                BLOCKCHAIN_MODULE,
                "wallet_get_balance",
                &[address],
            )
            .await,
        );
    }
    let module_info = match adapter {
        ModuleTransportKind::Module => probes
            .first()
            .cloned()
            .unwrap_or_else(|| unavailable_metadata_probe(adapter, BLOCKCHAIN_MODULE)),
        ModuleTransportKind::LogoscoreCli => {
            module_info_probe(module_transport, adapter, BLOCKCHAIN_MODULE).await
        }
    };
    ModuleReport::new(adapter, BLOCKCHAIN_MODULE, module_info, probes)
}

pub async fn capabilities_report(
    module_transport: &SharedModuleTransport,
    adapter: ModuleTransportKind,
) -> ModuleReport {
    ModuleReport::new(
        adapter,
        CAPABILITY_MODULE,
        module_info_probe(module_transport, adapter, CAPABILITY_MODULE).await,
        Vec::new(),
    )
}

pub(super) async fn module_info_probe(
    module_transport: &SharedModuleTransport,
    adapter: ModuleTransportKind,
    module: &str,
) -> ProbeReport {
    let actual = module_transport.kind();
    if adapter != actual {
        return ProbeReport::err(
            format!("{module} info"),
            adapter.as_str(),
            transport_mismatch(adapter, actual),
        );
    }
    let source = match adapter {
        ModuleTransportKind::Module => adapter.as_str().to_owned(),
        ModuleTransportKind::LogoscoreCli => {
            format!("logoscore module-info {module} --json")
        }
    };
    ProbeReport::from_result(
        format!("{module} info"),
        source,
        module_transport.module_info(module.to_owned()).await,
    )
}

pub(super) fn unavailable_metadata_probe(
    adapter: ModuleTransportKind,
    module: &str,
) -> ProbeReport {
    ProbeReport::ok(
        format!("{module} info"),
        adapter.as_str(),
        serde_json::json!({
            "supported": false,
            "adapter": adapter,
            "reason": "module metadata is unavailable through this transport",
        }),
    )
}

pub(super) async fn call_probe(
    module_transport: &SharedModuleTransport,
    adapter: ModuleTransportKind,
    module: &str,
    method: &str,
    args: &[&str],
) -> ProbeReport {
    call_module_probe(module_transport, adapter, module, method, args, None).await
}

pub(super) async fn call_source_probe(
    module_transport: &SharedModuleTransport,
    adapter: ModuleTransportKind,
    module: &str,
    method: &str,
    args: &[&str],
    key: SourceProbeKey,
) -> ProbeReport {
    call_module_probe(module_transport, adapter, module, method, args, Some(key)).await
}

async fn call_module_probe(
    module_transport: &SharedModuleTransport,
    adapter: ModuleTransportKind,
    module: &str,
    method: &str,
    args: &[&str],
    key: Option<SourceProbeKey>,
) -> ProbeReport {
    let args = args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
    let args_label = if args.is_empty() {
        String::new()
    } else {
        format!("({})", args.join(", "))
    };
    let source_args = if args.is_empty() {
        String::new()
    } else {
        format!(" {}", args.join(" "))
    };
    let source = match adapter {
        ModuleTransportKind::Module => {
            format!("module call {module} {method}{source_args}")
        }
        ModuleTransportKind::LogoscoreCli => {
            format!("logoscore call {module} {method}{source_args}")
        }
    };
    let result = match ModuleCall::new(
        adapter,
        module,
        method,
        args.into_iter().map(Value::String).collect(),
    ) {
        Ok(call) => dispatch_module_call(module_transport.as_ref(), call)
            .await
            .map(|reply| reply.into_value()),
        Err(error) => Err(error),
    };
    let probe = ProbeReport::from_result(format!("{module}.{method}{args_label}"), source, result);
    match key {
        Some(key) => probe.with_probe_key(key.as_str()),
        None => probe,
    }
}

fn transport_mismatch(expected: ModuleTransportKind, actual: ModuleTransportKind) -> String {
    format!(
        "resolved module transport `{}` is unavailable; active transport is `{}`",
        expected.as_str(),
        actual.as_str()
    )
}

pub(super) fn optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
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

    #[derive(Debug)]
    struct FakeBasecampTransport;

    impl super::super::logos_core::ModuleTransport for FakeBasecampTransport {
        fn kind(&self) -> ModuleTransportKind {
            ModuleTransportKind::Module
        }

        fn call(&self, call: ModuleCall) -> super::super::logos_core::ModuleCallFuture<'_> {
            Box::pin(async move {
                Ok(super::super::logos_core::ModuleCallReply::new(
                    ModuleTransportKind::Module,
                    json!({ "method": call.method() }),
                ))
            })
        }
    }

    struct FakeCliTransport {
        status_calls: Arc<AtomicUsize>,
        module_info_calls: Arc<AtomicUsize>,
    }

    impl super::super::logos_core::ModuleTransport for FakeCliTransport {
        fn kind(&self) -> ModuleTransportKind {
            ModuleTransportKind::LogoscoreCli
        }

        fn call(&self, call: ModuleCall) -> super::super::logos_core::ModuleCallFuture<'_> {
            Box::pin(async move {
                Ok(super::super::logos_core::ModuleCallReply::new(
                    ModuleTransportKind::LogoscoreCli,
                    json!({ "method": call.method() }),
                ))
            })
        }

        fn status(&self) -> super::super::logos_core::ModuleDiagnosticFuture<'_> {
            self.status_calls.fetch_add(1, Ordering::Relaxed);
            Box::pin(async { Ok(json!({ "runner": "fake_cli", "value": {} })) })
        }

        fn module_info(
            &self,
            module: String,
        ) -> super::super::logos_core::ModuleDiagnosticFuture<'_> {
            self.module_info_calls.fetch_add(1, Ordering::Relaxed);
            Box::pin(async move { Ok(json!({ "runner": "fake_cli", "module": module })) })
        }
    }

    #[test]
    fn modules_report_serializes_storage_and_delivery_as_module_surface() -> Result<()> {
        let probe = ProbeReport::ok("ok", "test", serde_json::json!({}));
        let value = serde_json::to_value(LogosModulesReport {
            adapter: ModuleTransportKind::LogoscoreCli,
            status: probe.clone(),
            blockchain: ModuleReport::new(
                ModuleTransportKind::LogoscoreCli,
                "blockchain_module",
                probe.clone(),
                Vec::new(),
            ),
            storage: ModuleReport::new(
                ModuleTransportKind::LogoscoreCli,
                "storage_module",
                probe.clone(),
                Vec::new(),
            ),
            delivery: ModuleReport::new(
                ModuleTransportKind::LogoscoreCli,
                "delivery_module",
                probe.clone(),
                Vec::new(),
            ),
            capabilities: ModuleReport::new(
                ModuleTransportKind::LogoscoreCli,
                "capability_module",
                probe,
                Vec::new(),
            ),
        })?;

        if value.get("adapter").and_then(Value::as_str) != Some("logoscore_cli") {
            bail!("module report adapter identity was not serialized: {value}");
        }

        for key in ["storage", "delivery"] {
            let report = value
                .get(key)
                .and_then(serde_json::Value::as_object)
                .ok_or_else(|| anyhow::anyhow!("missing `{key}` module report"))?;
            for source_key in ["health", "probe_facts", "capability_facts"] {
                if report.contains_key(source_key) {
                    bail!("module report `{key}` leaked `{source_key}`: {report:?}");
                }
            }
            for module_key in ["adapter", "module", "module_info", "probes"] {
                if !report.contains_key(module_key) {
                    bail!("module report `{key}` missing `{module_key}`: {report:?}");
                }
            }
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_report_uses_injected_module_transport_without_cli_metadata() -> Result<()> {
        let transport: SharedModuleTransport = Arc::new(FakeBasecampTransport);

        let report = blockchain_module_report(&transport, ModuleTransportKind::Module, None).await;

        if report.adapter != ModuleTransportKind::Module {
            bail!("module adapter identity was not preserved: {report:?}");
        }
        if !report.module_info.ok
            || report.module_info.label != "blockchain_module.get_cryptarchia_info"
        {
            bail!("Basecamp live identity probe was not used as module info: {report:?}");
        }
        if report
            .probes
            .iter()
            .any(|probe| !probe.ok || !probe.source.starts_with("module call blockchain_module"))
        {
            bail!("Basecamp probes did not use injected module transport: {report:?}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn cli_reports_use_injected_diagnostics_without_global_cli_fallback() -> Result<()> {
        let status_calls = Arc::new(AtomicUsize::new(0));
        let module_info_calls = Arc::new(AtomicUsize::new(0));
        let transport: SharedModuleTransport = Arc::new(FakeCliTransport {
            status_calls: Arc::clone(&status_calls),
            module_info_calls: Arc::clone(&module_info_calls),
        });

        let report = modules_report(&transport).await;

        if status_calls.load(Ordering::Relaxed) != 1 {
            bail!("module report bypassed injected CLI status capability");
        }
        if module_info_calls.load(Ordering::Relaxed) != 4 {
            bail!("module report bypassed injected CLI metadata capability");
        }
        if report
            .status
            .value
            .as_ref()
            .and_then(|value| value.get("runner"))
            != Some(&json!("fake_cli"))
        {
            bail!("module report did not preserve injected CLI status: {report:?}");
        }
        for module in [
            report.blockchain,
            report.storage,
            report.delivery,
            report.capabilities,
        ] {
            if module
                .module_info
                .value
                .as_ref()
                .and_then(|value| value.get("runner"))
                != Some(&json!("fake_cli"))
            {
                bail!("module report did not preserve injected CLI metadata: {module:?}");
            }
        }
        Ok(())
    }
}
