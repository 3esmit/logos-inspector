use serde::Serialize;

use crate::{
    ProbeReport, logoscore,
    source_routing::{SourceProbeKey, SourceReport},
};

use super::{delivery_report, storage_report};

pub(super) const BLOCKCHAIN_MODULE: &str = "blockchain_module";
pub(super) const STORAGE_MODULE: &str = "storage_module";
pub(super) const DELIVERY_MODULE: &str = "delivery_module";
const CAPABILITY_MODULE: &str = "capability_module";

#[derive(Debug, Clone, Serialize)]
pub struct LogosModulesReport {
    pub status: ProbeReport,
    pub blockchain: ModuleReport,
    pub storage: SourceReport,
    pub delivery: SourceReport,
    pub capabilities: ModuleReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleReport {
    pub module: String,
    pub module_info: ProbeReport,
    pub probes: Vec<ProbeReport>,
}

impl ModuleReport {
    pub(crate) fn new(
        module: impl Into<String>,
        module_info: ProbeReport,
        probes: Vec<ProbeReport>,
    ) -> Self {
        Self {
            module: module.into(),
            module_info,
            probes,
        }
    }
}

pub fn logoscore_status_report() -> ProbeReport {
    ProbeReport::from_result(
        "logoscore status",
        "logoscore status --json",
        logoscore::status(),
    )
}

pub fn modules_report() -> LogosModulesReport {
    LogosModulesReport {
        status: logoscore_status_report(),
        blockchain: blockchain_module_report(None),
        storage: storage_report(None, false),
        delivery: delivery_report(None),
        capabilities: capabilities_report(),
    }
}

pub fn blockchain_module_report(address: Option<&str>) -> ModuleReport {
    let mut probes = vec![
        call_probe(BLOCKCHAIN_MODULE, "get_cryptarchia_info", &[]),
        call_probe(BLOCKCHAIN_MODULE, "wallet_get_known_addresses", &[]),
    ];
    if let Some(address) = optional(address) {
        probes.push(call_probe(
            BLOCKCHAIN_MODULE,
            "wallet_get_balance",
            &[address],
        ));
    }
    ModuleReport::new(
        BLOCKCHAIN_MODULE,
        module_info_probe(BLOCKCHAIN_MODULE),
        probes,
    )
}

pub fn capabilities_report() -> ModuleReport {
    ModuleReport::new(
        CAPABILITY_MODULE,
        module_info_probe(CAPABILITY_MODULE),
        Vec::new(),
    )
}

pub(super) fn module_info_probe(module: &str) -> ProbeReport {
    ProbeReport::from_result(
        format!("{module} info"),
        format!("logoscore module-info {module} --json"),
        logoscore::module_info(module),
    )
}

pub(super) fn call_probe(module: &str, method: &str, args: &[&str]) -> ProbeReport {
    call_module_probe(module, method, args, None)
}

pub(super) fn call_source_probe(
    module: &str,
    method: &str,
    args: &[&str],
    key: SourceProbeKey,
) -> ProbeReport {
    call_module_probe(module, method, args, Some(key))
}

fn call_module_probe(
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
    let probe = ProbeReport::from_result(
        format!("{module}.{method}{args_label}"),
        format!("logoscore call {module} {method}{source_args}"),
        logoscore::call(module, method, &args),
    );
    match key {
        Some(key) => probe.with_probe_key(key.as_str()),
        None => probe,
    }
}

pub(super) fn optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}
