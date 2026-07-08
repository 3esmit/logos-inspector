use serde::Serialize;
use serde_json::Value;

use crate::ProbeReport;
use crate::source_routing::policy::{delivery_source_facts, storage_source_facts};

use super::report::SourceReportKind;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceFacts {
    pub health: SourceHealthFacts,
    pub probe_facts: Vec<SourceProbeFact>,
    pub capability_facts: Vec<SourceCapabilityFact>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceProbeFact {
    pub key: String,
    pub label: String,
    pub source: String,
    pub ok: bool,
    pub evidence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceHealthFacts {
    pub reachable: bool,
    pub ready: bool,
    pub status: SourceHealthStatus,
    pub summary: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceHealthStatus {
    Healthy,
    Degraded,
    Unavailable,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceCapabilityFact {
    pub key: String,
    pub label: String,
    pub available: bool,
    pub evidence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
}

impl SourceCapabilityFact {
    pub(crate) fn available(
        key: impl Into<String>,
        label: impl Into<String>,
        evidence: impl Into<String>,
        value: Option<Value>,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            available: true,
            evidence: evidence.into(),
            value,
        }
    }

    pub(crate) fn unavailable(
        key: impl Into<String>,
        label: impl Into<String>,
        evidence: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            available: false,
            evidence: evidence.into(),
            value: None,
        }
    }
}

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
