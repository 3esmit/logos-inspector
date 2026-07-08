use serde::Serialize;

use crate::{ProbeReport, modules::ModuleReport};

use crate::source_routing::{
    DeliverySourceReportKind, SourceFacts, SourceProbeKey, StorageSourceReportKind,
    delivery_source_facts, storage_source_facts,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceReportKind {
    Delivery(DeliverySourceReportKind),
    Storage(StorageSourceReportKind),
}

#[derive(Debug)]
pub(crate) struct SourceReportBuilder {
    module: String,
    kind: SourceReportKind,
    module_info: ProbeReport,
    probes: Vec<ProbeReport>,
}

impl SourceReportBuilder {
    pub(crate) fn delivery(
        module: impl Into<String>,
        kind: DeliverySourceReportKind,
        module_info: ProbeReport,
    ) -> Self {
        Self {
            module: module.into(),
            kind: SourceReportKind::Delivery(kind),
            module_info,
            probes: Vec::new(),
        }
    }

    pub(crate) fn storage(
        module: impl Into<String>,
        kind: StorageSourceReportKind,
        module_info: ProbeReport,
    ) -> Self {
        Self {
            module: module.into(),
            kind: SourceReportKind::Storage(kind),
            module_info,
            probes: Vec::new(),
        }
    }

    pub(crate) fn include_module_info_probe(mut self) -> Self {
        self.probes.push(self.module_info.clone());
        self
    }

    pub(crate) fn with_probes(mut self, probes: Vec<ProbeReport>) -> Self {
        self.probes = probes;
        self
    }

    pub(crate) fn push_probe(&mut self, probe: ProbeReport) {
        self.probes.push(probe);
    }

    pub(crate) fn push_ok(
        &mut self,
        key: SourceProbeKey,
        label: impl Into<String>,
        source: impl Into<String>,
        value: impl Serialize,
    ) {
        self.push_probe(keyed_probe_ok(key, label, source, value));
    }

    pub(crate) fn push_result<T, E>(
        &mut self,
        key: SourceProbeKey,
        label: impl Into<String>,
        source: impl Into<String>,
        result: Result<T, E>,
    ) where
        T: Serialize,
        E: std::fmt::Display,
    {
        self.push_probe(keyed_probe_result(key, label, source, result));
    }

    pub(crate) fn finish(self) -> ModuleReport {
        let facts = self.source_facts();
        ModuleReport::new(self.module, self.module_info, self.probes).with_source_facts(facts)
    }

    fn source_facts(&self) -> SourceFacts {
        match self.kind {
            SourceReportKind::Delivery(kind) => {
                delivery_source_facts(kind, &self.module_info, &self.probes)
            }
            SourceReportKind::Storage(kind) => {
                storage_source_facts(kind, &self.module_info, &self.probes)
            }
        }
    }
}

pub(crate) fn keyed_probe_result<T, E>(
    key: SourceProbeKey,
    label: impl Into<String>,
    source: impl Into<String>,
    result: Result<T, E>,
) -> ProbeReport
where
    T: Serialize,
    E: std::fmt::Display,
{
    ProbeReport::from_result(label, source, result).with_probe_key(key.as_str())
}

pub(crate) fn keyed_probe_ok(
    key: SourceProbeKey,
    label: impl Into<String>,
    source: impl Into<String>,
    value: impl Serialize,
) -> ProbeReport {
    ProbeReport::ok(label, source, value).with_probe_key(key.as_str())
}

pub(crate) fn keyed_probe_err(
    key: SourceProbeKey,
    label: impl Into<String>,
    source: impl Into<String>,
    error: impl std::fmt::Display,
) -> ProbeReport {
    ProbeReport::err(label, source, error).with_probe_key(key.as_str())
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};

    use super::*;

    #[test]
    fn keyed_probe_result_attaches_probe_key() -> Result<()> {
        let probe = keyed_probe_result(
            SourceProbeKey::StoragePeerId,
            "storage.peer",
            "source",
            Ok::<_, String>("peer-a"),
        );

        if probe.probe_key.as_deref() != Some(SourceProbeKey::StoragePeerId.as_str()) {
            bail!("probe key was not attached: {probe:?}");
        }
        if probe.value.as_ref().and_then(serde_json::Value::as_str) != Some("peer-a") {
            bail!("probe value was not preserved: {probe:?}");
        }
        Ok(())
    }

    #[test]
    fn source_report_builder_attaches_source_facts() -> Result<()> {
        let module_info = keyed_probe_ok(
            SourceProbeKey::StoragePeerId,
            "storage_rest.peerId",
            "http://storage/peerid",
            "peer-a",
        );
        let report = SourceReportBuilder::storage(
            "storage_rest",
            StorageSourceReportKind::Rest,
            module_info,
        )
        .include_module_info_probe()
        .finish();

        if report.health.is_none() {
            bail!("source health facts were not attached");
        }
        if !report
            .probe_facts
            .iter()
            .any(|fact| fact.key == SourceProbeKey::StoragePeerId.as_str())
        {
            bail!("source probe facts were not attached: {report:?}");
        }
        Ok(())
    }
}
