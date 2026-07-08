use serde::Serialize;
use serde_json::Value;

use crate::{
    blockchain::logos_node_cryptarchia_info,
    lez::{indexer_health, last_sequencer_block_id, sequencer_health, sequencer_program_ids},
    probe::ProbeField,
    rpc::raw_json_rpc_optional_result,
};

#[derive(Debug, Clone, Serialize)]
pub struct InspectorScope {
    pub name: &'static str,
    pub area: &'static str,
    pub status: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceProbe {
    pub endpoint: String,
    pub health: ProbeField,
    pub head: ProbeField,
    pub programs: Option<ProbeField>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeProbe {
    pub endpoint: String,
    pub consensus: ProbeField,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverviewReport {
    pub product: &'static str,
    pub scopes: Vec<InspectorScope>,
    pub node: NodeProbe,
    pub sequencer: ServiceProbe,
    pub indexer: ServiceProbe,
}

#[must_use]
pub fn inspector_scopes() -> Vec<InspectorScope> {
    vec![
        InspectorScope {
            name: "Logos Blockchain",
            area: "base chain, blocks, transactions, services",
            status: "active",
        },
        InspectorScope {
            name: "Logos Execution Zone",
            area: "LEZ sequencer, indexer, accounts, programs",
            status: "active",
        },
        InspectorScope {
            name: "Logos Messaging",
            area: "message transport and routing inspection",
            status: "active",
        },
        InspectorScope {
            name: "Logos Storage",
            area: "storage node and content inspection",
            status: "active",
        },
    ]
}

pub async fn overview(
    sequencer_endpoint: &str,
    indexer_endpoint: &str,
    node_endpoint: &str,
) -> OverviewReport {
    let (
        node_consensus,
        sequencer_health,
        sequencer_head,
        sequencer_programs,
        indexer_head,
        indexer_health,
    ) = tokio::join!(
        logos_node_cryptarchia_info(node_endpoint),
        sequencer_health(sequencer_endpoint),
        last_sequencer_block_id(sequencer_endpoint),
        sequencer_program_ids(sequencer_endpoint),
        raw_json_rpc_optional_result(
            indexer_endpoint,
            "getLastFinalizedBlockId",
            Value::Array(vec![]),
        ),
        indexer_health(indexer_endpoint),
    );

    let node = node_probe(
        node_endpoint,
        overview_probe(OverviewProbeKey::NodeConsensus, node_consensus),
    );
    let sequencer = service_probe(
        sequencer_endpoint,
        overview_probe(
            OverviewProbeKey::SequencerHealth,
            sequencer_health.map(|()| "ok"),
        ),
        overview_probe(OverviewProbeKey::SequencerHead, sequencer_head),
        Some(overview_probe(
            OverviewProbeKey::SequencerPrograms,
            sequencer_programs.map(|programs| programs.len()),
        )),
    );
    let indexer = service_probe(
        indexer_endpoint,
        overview_probe(OverviewProbeKey::IndexerHealth, indexer_health),
        overview_probe(OverviewProbeKey::IndexerHead, indexer_head),
        None,
    );

    OverviewReport {
        product: "Logos Inspector",
        scopes: inspector_scopes(),
        node,
        sequencer,
        indexer,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverviewProbeKey {
    NodeConsensus,
    SequencerHealth,
    SequencerHead,
    SequencerPrograms,
    IndexerHealth,
    IndexerHead,
}

#[derive(Debug, Clone)]
struct OverviewProbe {
    key: OverviewProbeKey,
    field: ProbeField,
}

fn overview_probe<T, E>(key: OverviewProbeKey, result: Result<T, E>) -> OverviewProbe
where
    T: Serialize,
    E: std::fmt::Display,
{
    OverviewProbe {
        key,
        field: ProbeField::from_result(result),
    }
}

fn node_probe(endpoint: &str, consensus: OverviewProbe) -> NodeProbe {
    debug_assert_eq!(consensus.key, OverviewProbeKey::NodeConsensus);
    NodeProbe {
        endpoint: endpoint.to_owned(),
        consensus: consensus.field,
    }
}

fn service_probe(
    endpoint: &str,
    health: OverviewProbe,
    head: OverviewProbe,
    programs: Option<OverviewProbe>,
) -> ServiceProbe {
    debug_assert!(matches!(
        health.key,
        OverviewProbeKey::SequencerHealth | OverviewProbeKey::IndexerHealth
    ));
    debug_assert!(matches!(
        head.key,
        OverviewProbeKey::SequencerHead | OverviewProbeKey::IndexerHead
    ));
    if let Some(programs) = programs.as_ref() {
        debug_assert_eq!(programs.key, OverviewProbeKey::SequencerPrograms);
    }
    ServiceProbe {
        endpoint: endpoint.to_owned(),
        health: health.field,
        head: head.field,
        programs: programs.map(|programs| programs.field),
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use serde_json::json;

    use super::*;

    #[test]
    fn overview_probe_projects_success() {
        let probe = overview_probe(OverviewProbeKey::SequencerHead, Ok::<_, io::Error>(42_u64));

        assert_eq!(probe.key, OverviewProbeKey::SequencerHead);
        assert!(probe.field.ok);
        assert_eq!(probe.field.value, Some(json!(42)));
        assert_eq!(probe.field.error, None);
    }

    #[test]
    fn overview_probe_projects_error() {
        let error = io::Error::other("node unavailable");
        let probe = overview_probe::<u64, _>(OverviewProbeKey::NodeConsensus, Err(error));

        assert_eq!(probe.key, OverviewProbeKey::NodeConsensus);
        assert!(!probe.field.ok);
        assert_eq!(probe.field.value, None);
        assert_eq!(probe.field.error.as_deref(), Some("node unavailable"));
    }
}
