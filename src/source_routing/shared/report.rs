use serde::Serialize;

use crate::ProbeReport;

use crate::source_routing::{
    DeliverySourceReportKind, SourceCapabilityFact, SourceFacts, SourceHealthFacts,
    SourceProbeFact, SourceProbeKey, StorageSourceReportKind,
};

use super::evidence::SourceEvidence;
use super::facts::source_facts_for_report;

#[derive(Debug, Clone, Serialize)]
pub struct SourceReport {
    pub module: String,
    pub module_info: ProbeReport,
    pub probes: Vec<ProbeReport>,
    pub health: SourceHealthFacts,
    pub probe_facts: Vec<SourceProbeFact>,
    pub capability_facts: Vec<SourceCapabilityFact>,
}

impl SourceReport {
    fn new(
        module: impl Into<String>,
        module_info: ProbeReport,
        probes: Vec<ProbeReport>,
        facts: SourceFacts,
    ) -> Self {
        Self {
            module: module.into(),
            module_info,
            probes,
            health: facts.health,
            probe_facts: facts.probe_facts,
            capability_facts: facts.capability_facts,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceReportKind {
    Delivery(DeliverySourceReportKind),
    Storage(StorageSourceReportKind),
}

#[derive(Debug)]
pub(crate) struct SourceReportBuilder {
    kind: SourceReportKind,
    evidence: SourceEvidence,
}

pub(crate) fn source_text_metrics_report<E>(
    module: impl Into<String>,
    kind: SourceReportKind,
    endpoint: &str,
    scrape: MetricsProbeSpec,
    collect: MetricsProbeSpec,
    result: Result<String, E>,
) -> SourceReport
where
    E: std::fmt::Display,
{
    let module = module.into();
    match result {
        Ok(text) => {
            let module_info = keyed_probe_ok(
                scrape.key,
                scrape.label,
                endpoint,
                serde_json::json!({
                    "bytes": text.len(),
                    "lines": text.lines().count(),
                }),
            );
            let mut report = SourceReportBuilder::new(module, kind, module_info);
            report.push_ok(collect.key, collect.label, endpoint, text);
            report.finish()
        }
        Err(error) => {
            let error = error.to_string();
            let module_info = keyed_probe_err(scrape.key, scrape.label, endpoint, &error);
            let mut report = SourceReportBuilder::new(module, kind, module_info);
            report.push_probe(keyed_probe_err(collect.key, collect.label, endpoint, error));
            report.finish()
        }
    }
}

pub(crate) fn unsupported_source_report(
    module_prefix: &str,
    family_label: &str,
    kind: SourceReportKind,
    mode: &str,
) -> SourceReport {
    let module = format!("{module_prefix}_{mode}");
    let module_info = ProbeReport::err(
        format!("{family_label} source"),
        mode,
        format!("{family_label} source mode `{mode}` is not implemented"),
    );
    SourceReportBuilder::new(module, kind, module_info).finish()
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MetricsProbeSpec {
    pub(crate) key: SourceProbeKey,
    pub(crate) label: &'static str,
}

impl SourceReportBuilder {
    pub(crate) fn new(
        module: impl Into<String>,
        kind: SourceReportKind,
        module_info: ProbeReport,
    ) -> Self {
        Self {
            kind,
            evidence: SourceEvidence::new(module, module_info, Vec::new()),
        }
    }

    pub(crate) fn from_evidence(kind: SourceReportKind, evidence: SourceEvidence) -> Self {
        Self { kind, evidence }
    }

    pub(crate) fn delivery(
        module: impl Into<String>,
        kind: DeliverySourceReportKind,
        module_info: ProbeReport,
    ) -> Self {
        Self::new(module, SourceReportKind::Delivery(kind), module_info)
    }

    pub(crate) fn storage(
        module: impl Into<String>,
        kind: StorageSourceReportKind,
        module_info: ProbeReport,
    ) -> Self {
        Self::new(module, SourceReportKind::Storage(kind), module_info)
    }

    pub(crate) fn include_module_info_probe(mut self) -> Self {
        self.evidence.probes.push(self.evidence.module_info.clone());
        self
    }

    pub(crate) fn with_probes(mut self, probes: Vec<ProbeReport>) -> Self {
        self.evidence.probes = probes;
        self
    }

    pub(crate) fn push_probe(&mut self, probe: ProbeReport) {
        self.evidence.probes.push(probe);
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

    pub(crate) fn finish(self) -> SourceReport {
        let facts = self.source_facts();
        SourceReport::new(
            self.evidence.module,
            self.evidence.module_info,
            self.evidence.probes,
            facts,
        )
    }

    fn source_facts(&self) -> SourceFacts {
        source_facts_for_report(self.kind, &self.evidence.module_info, &self.evidence.probes)
    }
}

pub(crate) fn source_report_from_evidence(
    kind: SourceReportKind,
    evidence: SourceEvidence,
) -> SourceReport {
    SourceReportBuilder::from_evidence(kind, evidence).finish()
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

        if !report.health.reachable {
            bail!("source health facts were not attached: {report:?}");
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

    #[test]
    fn source_report_serializes_with_stable_top_level_shape() -> Result<()> {
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
        let value = serde_json::to_value(report)?;

        for key in [
            "module",
            "module_info",
            "probes",
            "health",
            "probe_facts",
            "capability_facts",
        ] {
            if value.get(key).is_none() {
                bail!("source report missing `{key}`: {value}");
            }
        }
        Ok(())
    }
}
