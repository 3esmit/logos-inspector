use crate::ProbeReport;

use crate::source_routing::policy::{SourceFacts, delivery_source_facts, storage_source_facts};

use super::report::SourceReportKind;

#[must_use]
pub(crate) fn source_facts_for_report(
    kind: SourceReportKind,
    module_info: &ProbeReport,
    probes: &[ProbeReport],
) -> SourceFacts {
    match kind {
        SourceReportKind::Delivery(kind) => delivery_source_facts(kind, module_info, probes),
        SourceReportKind::Storage(kind) => storage_source_facts(kind, module_info, probes),
    }
}
