use serde::Serialize;

use crate::{ProbeReport, logoscore};

const BLOCKCHAIN_MODULE: &str = "blockchain_module";
const STORAGE_MODULE: &str = "storage_module";
const DELIVERY_MODULE: &str = "delivery_module";
const CAPABILITY_MODULE: &str = "capability_module";

#[derive(Debug, Clone, Serialize)]
pub struct LogosModulesReport {
    pub status: ProbeReport,
    pub blockchain: ModuleReport,
    pub storage: ModuleReport,
    pub delivery: ModuleReport,
    pub capabilities: ModuleReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleReport {
    pub module: String,
    pub module_info: ProbeReport,
    pub probes: Vec<ProbeReport>,
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
        storage: storage_report(None),
        delivery: delivery_report(None),
        capabilities: capabilities_report(),
    }
}

pub fn blockchain_module_report(address: Option<&str>) -> ModuleReport {
    let mut probes = vec![
        call_probe(BLOCKCHAIN_MODULE, "get_cryptarchia_info", &[]),
        call_probe(BLOCKCHAIN_MODULE, "get_peer_id", &[]),
        call_probe(BLOCKCHAIN_MODULE, "wallet_get_known_addresses", &[]),
        call_probe(BLOCKCHAIN_MODULE, "wallet_get_claimable_vouchers", &[]),
    ];
    if let Some(address) = optional(address) {
        probes.push(call_probe(
            BLOCKCHAIN_MODULE,
            "wallet_get_balance",
            &[address],
        ));
        probes.push(call_probe(
            BLOCKCHAIN_MODULE,
            "wallet_get_notes",
            &[address],
        ));
    }
    ModuleReport {
        module: BLOCKCHAIN_MODULE.to_owned(),
        module_info: module_info_probe(BLOCKCHAIN_MODULE),
        probes,
    }
}

pub fn storage_report(cid: Option<&str>) -> ModuleReport {
    let mut probes = vec![
        call_probe(STORAGE_MODULE, "version", &[]),
        call_probe(STORAGE_MODULE, "moduleVersion", &[]),
        call_probe(STORAGE_MODULE, "dataDir", &[]),
        call_probe(STORAGE_MODULE, "peerId", &[]),
        call_probe(STORAGE_MODULE, "spr", &[]),
        call_probe(STORAGE_MODULE, "space", &[]),
        call_probe(STORAGE_MODULE, "manifests", &[]),
        call_probe(STORAGE_MODULE, "collectMetrics", &[]),
        call_probe(STORAGE_MODULE, "debug", &[]),
    ];
    if let Some(cid) = optional(cid) {
        probes.push(call_probe(STORAGE_MODULE, "exists", &[cid]));
    }
    ModuleReport {
        module: STORAGE_MODULE.to_owned(),
        module_info: module_info_probe(STORAGE_MODULE),
        probes,
    }
}

pub fn delivery_report(info_id: Option<&str>) -> ModuleReport {
    let mut probes = vec![
        call_probe(DELIVERY_MODULE, "version", &[]),
        call_probe(DELIVERY_MODULE, "getAvailableNodeInfoIDs", &[]),
        call_probe(DELIVERY_MODULE, "getAvailableConfigs", &[]),
        call_probe(DELIVERY_MODULE, "collectOpenMetricsText", &[]),
    ];
    for info_id in [
        "Version",
        "Metrics",
        "MyMultiaddresses",
        "MyENR",
        "MyPeerId",
        "MyBoundPorts",
        "MyMixPubKey",
    ] {
        probes.push(call_probe(DELIVERY_MODULE, "getNodeInfo", &[info_id]));
    }
    if let Some(info_id) = optional(info_id) {
        probes.push(call_probe(DELIVERY_MODULE, "getNodeInfo", &[info_id]));
    }
    ModuleReport {
        module: DELIVERY_MODULE.to_owned(),
        module_info: module_info_probe(DELIVERY_MODULE),
        probes,
    }
}

pub fn capabilities_report() -> ModuleReport {
    ModuleReport {
        module: CAPABILITY_MODULE.to_owned(),
        module_info: module_info_probe(CAPABILITY_MODULE),
        probes: Vec::new(),
    }
}

fn module_info_probe(module: &str) -> ProbeReport {
    ProbeReport::from_result(
        format!("{module} info"),
        format!("logoscore module-info {module} --json"),
        logoscore::module_info(module),
    )
}

fn call_probe(module: &str, method: &str, args: &[&str]) -> ProbeReport {
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
    ProbeReport::from_result(
        format!("{module}.{method}{args_label}"),
        format!("logoscore call {module} {method}{source_args}"),
        logoscore::call(module, method, &args),
    )
}

fn optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}
