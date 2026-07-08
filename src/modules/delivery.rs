use crate::source_routing::{
    DeliverySourceReportKind, SourceReport, SourceReportBuilder, delivery_module_probe_plan,
};

use super::base::{DELIVERY_MODULE, call_probe, call_source_probe, module_info_probe, optional};

pub fn delivery_report(info_id: Option<&str>) -> SourceReport {
    let probes = delivery_module_probe_plan(optional(info_id))
        .into_iter()
        .map(|step| match step.key {
            Some(key) => call_source_probe(DELIVERY_MODULE, step.method, &step.args, key),
            None => call_probe(DELIVERY_MODULE, step.method, &step.args),
        })
        .collect();
    let module_info = module_info_probe(DELIVERY_MODULE);
    SourceReportBuilder::delivery(
        DELIVERY_MODULE,
        DeliverySourceReportKind::Module,
        module_info,
    )
    .with_probes(probes)
    .finish()
}
