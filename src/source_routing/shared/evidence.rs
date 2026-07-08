use crate::ProbeReport;

#[derive(Debug, Clone)]
pub(crate) struct SourceEvidence {
    pub(crate) module: String,
    pub(crate) module_info: ProbeReport,
    pub(crate) probes: Vec<ProbeReport>,
}

impl SourceEvidence {
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
