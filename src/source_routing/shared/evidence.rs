use crate::{ProbeReport, source_routing::AdapterConnectionType};

#[derive(Debug, Clone)]
pub(crate) struct SourceEvidence {
    pub(crate) adapter: Option<AdapterConnectionType>,
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
            adapter: None,
            module: module.into(),
            module_info,
            probes,
        }
    }

    pub(crate) fn with_adapter(mut self, adapter: AdapterConnectionType) -> Self {
        self.adapter = Some(adapter);
        self
    }

    pub(crate) fn with_optional_adapter(mut self, adapter: Option<AdapterConnectionType>) -> Self {
        self.adapter = adapter;
        self
    }
}
