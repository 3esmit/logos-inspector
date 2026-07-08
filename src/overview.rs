use std::{future::Future, pin::Pin};

use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

use crate::{
    blockchain::logos_node_cryptarchia_info,
    lez::{
        ProgramIdEntry, indexer_health, last_sequencer_block_id, sequencer_health,
        sequencer_program_ids,
    },
    probe::ProbeField,
    rpc::raw_json_rpc_optional_result,
};

type OverviewProbeFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

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
    let endpoints = OverviewEndpoints::new(sequencer_endpoint, indexer_endpoint, node_endpoint);
    overview_with_adapter(&endpoints, &DirectOverviewProbeAdapter).await
}

#[derive(Debug, Clone)]
struct OverviewEndpoints {
    sequencer: String,
    indexer: String,
    node: String,
}

impl OverviewEndpoints {
    fn new(sequencer_endpoint: &str, indexer_endpoint: &str, node_endpoint: &str) -> Self {
        Self {
            sequencer: sequencer_endpoint.to_owned(),
            indexer: indexer_endpoint.to_owned(),
            node: node_endpoint.to_owned(),
        }
    }
}

trait OverviewProbeAdapter {
    fn node_consensus<'a>(&'a self, endpoint: &'a str) -> OverviewProbeFuture<'a, Value>;

    fn sequencer_health<'a>(&'a self, endpoint: &'a str) -> OverviewProbeFuture<'a, ()>;

    fn sequencer_head<'a>(&'a self, endpoint: &'a str) -> OverviewProbeFuture<'a, u64>;

    fn sequencer_programs<'a>(
        &'a self,
        endpoint: &'a str,
    ) -> OverviewProbeFuture<'a, Vec<ProgramIdEntry>>;

    fn indexer_head<'a>(&'a self, endpoint: &'a str) -> OverviewProbeFuture<'a, Value>;

    fn indexer_health<'a>(&'a self, endpoint: &'a str) -> OverviewProbeFuture<'a, Value>;
}

struct DirectOverviewProbeAdapter;

impl OverviewProbeAdapter for DirectOverviewProbeAdapter {
    fn node_consensus<'a>(&'a self, endpoint: &'a str) -> OverviewProbeFuture<'a, Value> {
        Box::pin(async move { logos_node_cryptarchia_info(endpoint).await })
    }

    fn sequencer_health<'a>(&'a self, endpoint: &'a str) -> OverviewProbeFuture<'a, ()> {
        Box::pin(async move { sequencer_health(endpoint).await })
    }

    fn sequencer_head<'a>(&'a self, endpoint: &'a str) -> OverviewProbeFuture<'a, u64> {
        Box::pin(async move { last_sequencer_block_id(endpoint).await })
    }

    fn sequencer_programs<'a>(
        &'a self,
        endpoint: &'a str,
    ) -> OverviewProbeFuture<'a, Vec<ProgramIdEntry>> {
        Box::pin(async move { sequencer_program_ids(endpoint).await })
    }

    fn indexer_head<'a>(&'a self, endpoint: &'a str) -> OverviewProbeFuture<'a, Value> {
        Box::pin(async move {
            raw_json_rpc_optional_result(endpoint, "getLastFinalizedBlockId", Value::Array(vec![]))
                .await
        })
    }

    fn indexer_health<'a>(&'a self, endpoint: &'a str) -> OverviewProbeFuture<'a, Value> {
        Box::pin(async move { indexer_health(endpoint).await })
    }
}

struct OverviewProbeResults {
    node_consensus: Result<Value>,
    sequencer_health: Result<()>,
    sequencer_head: Result<u64>,
    sequencer_programs: Result<Vec<ProgramIdEntry>>,
    indexer_head: Result<Value>,
    indexer_health: Result<Value>,
}

async fn overview_with_adapter(
    endpoints: &OverviewEndpoints,
    adapter: &impl OverviewProbeAdapter,
) -> OverviewReport {
    let (
        node_consensus,
        sequencer_health,
        sequencer_head,
        sequencer_programs,
        indexer_head,
        indexer_health,
    ) = tokio::join!(
        adapter.node_consensus(&endpoints.node),
        adapter.sequencer_health(&endpoints.sequencer),
        adapter.sequencer_head(&endpoints.sequencer),
        adapter.sequencer_programs(&endpoints.sequencer),
        adapter.indexer_head(&endpoints.indexer),
        adapter.indexer_health(&endpoints.indexer),
    );

    overview_report_from_probe_results(
        endpoints,
        OverviewProbeResults {
            node_consensus,
            sequencer_health,
            sequencer_head,
            sequencer_programs,
            indexer_head,
            indexer_health,
        },
    )
}

fn overview_report_from_probe_results(
    endpoints: &OverviewEndpoints,
    results: OverviewProbeResults,
) -> OverviewReport {
    let node = node_probe(
        &endpoints.node,
        overview_probe(OverviewProbeKey::NodeConsensus, results.node_consensus),
    );
    let sequencer = service_probe(
        &endpoints.sequencer,
        overview_probe(
            OverviewProbeKey::SequencerHealth,
            results.sequencer_health.map(|()| "ok"),
        ),
        overview_probe(OverviewProbeKey::SequencerHead, results.sequencer_head),
        Some(overview_probe(
            OverviewProbeKey::SequencerPrograms,
            results.sequencer_programs.map(|programs| programs.len()),
        )),
    );
    let indexer = service_probe(
        &endpoints.indexer,
        overview_probe(OverviewProbeKey::IndexerHealth, results.indexer_health),
        overview_probe(OverviewProbeKey::IndexerHead, results.indexer_head),
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

fn overview_probe<T, E>(key: OverviewProbeKey, result: std::result::Result<T, E>) -> OverviewProbe
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
    use anyhow::{Result, anyhow, ensure};
    use serde_json::json;

    use super::*;

    struct ReadyOverviewAdapter;

    impl OverviewProbeAdapter for ReadyOverviewAdapter {
        fn node_consensus<'a>(&'a self, _endpoint: &'a str) -> OverviewProbeFuture<'a, Value> {
            ok(json!({"mode": "normal"}))
        }

        fn sequencer_health<'a>(&'a self, _endpoint: &'a str) -> OverviewProbeFuture<'a, ()> {
            ok(())
        }

        fn sequencer_head<'a>(&'a self, _endpoint: &'a str) -> OverviewProbeFuture<'a, u64> {
            ok(42)
        }

        fn sequencer_programs<'a>(
            &'a self,
            _endpoint: &'a str,
        ) -> OverviewProbeFuture<'a, Vec<ProgramIdEntry>> {
            ok(vec![
                ProgramIdEntry {
                    label: "first".to_owned(),
                    base58: "program-1".to_owned(),
                    hex: "01".to_owned(),
                },
                ProgramIdEntry {
                    label: "second".to_owned(),
                    base58: "program-2".to_owned(),
                    hex: "02".to_owned(),
                },
            ])
        }

        fn indexer_head<'a>(&'a self, _endpoint: &'a str) -> OverviewProbeFuture<'a, Value> {
            ok(json!("head-1"))
        }

        fn indexer_health<'a>(&'a self, _endpoint: &'a str) -> OverviewProbeFuture<'a, Value> {
            ok(json!({"ready": true}))
        }
    }

    struct DegradedOverviewAdapter;

    impl OverviewProbeAdapter for DegradedOverviewAdapter {
        fn node_consensus<'a>(&'a self, _endpoint: &'a str) -> OverviewProbeFuture<'a, Value> {
            err("node unavailable")
        }

        fn sequencer_health<'a>(&'a self, _endpoint: &'a str) -> OverviewProbeFuture<'a, ()> {
            ok(())
        }

        fn sequencer_head<'a>(&'a self, _endpoint: &'a str) -> OverviewProbeFuture<'a, u64> {
            err("head unavailable")
        }

        fn sequencer_programs<'a>(
            &'a self,
            _endpoint: &'a str,
        ) -> OverviewProbeFuture<'a, Vec<ProgramIdEntry>> {
            ok(Vec::new())
        }

        fn indexer_head<'a>(&'a self, _endpoint: &'a str) -> OverviewProbeFuture<'a, Value> {
            ok(Value::Null)
        }

        fn indexer_health<'a>(&'a self, _endpoint: &'a str) -> OverviewProbeFuture<'a, Value> {
            err("indexer unavailable")
        }
    }

    #[test]
    fn overview_with_adapter_projects_ready_sources() -> Result<()> {
        let endpoints = OverviewEndpoints::new("seq", "idx", "node");
        let runtime = tokio::runtime::Runtime::new()?;

        let report = runtime.block_on(overview_with_adapter(&endpoints, &ReadyOverviewAdapter));

        ensure!(report.product == "Logos Inspector", "unexpected product");
        ensure!(report.scopes.len() == 4, "unexpected scope count");
        ensure!(report.node.endpoint == "node", "unexpected node endpoint");
        ensure!(
            report.node.consensus.value == Some(json!({"mode": "normal"})),
            "unexpected node consensus"
        );
        ensure!(
            report.sequencer.endpoint == "seq",
            "unexpected sequencer endpoint"
        );
        ensure!(
            report.sequencer.health.value == Some(json!("ok")),
            "unexpected sequencer health"
        );
        ensure!(
            report.sequencer.head.value == Some(json!(42)),
            "unexpected sequencer head"
        );
        ensure!(
            report
                .sequencer
                .programs
                .as_ref()
                .and_then(|field| field.value.as_ref())
                == Some(&json!(2)),
            "unexpected sequencer program count"
        );
        ensure!(
            report.indexer.endpoint == "idx",
            "unexpected indexer endpoint"
        );
        ensure!(
            report.indexer.health.value == Some(json!({"ready": true})),
            "unexpected indexer health"
        );
        ensure!(
            report.indexer.head.value == Some(json!("head-1")),
            "unexpected indexer head"
        );
        Ok(())
    }

    #[test]
    fn overview_with_adapter_preserves_degraded_source_errors() -> Result<()> {
        let endpoints = OverviewEndpoints::new("seq", "idx", "node");
        let runtime = tokio::runtime::Runtime::new()?;

        let report = runtime.block_on(overview_with_adapter(&endpoints, &DegradedOverviewAdapter));

        ensure!(!report.node.consensus.ok, "node consensus should fail");
        ensure!(
            report.node.consensus.error.as_deref() == Some("node unavailable"),
            "unexpected node error"
        );
        ensure!(report.sequencer.health.ok, "sequencer health should pass");
        ensure!(!report.sequencer.head.ok, "sequencer head should fail");
        ensure!(
            report.sequencer.head.error.as_deref() == Some("head unavailable"),
            "unexpected sequencer head error"
        );
        ensure!(!report.indexer.health.ok, "indexer health should fail");
        ensure!(
            report.indexer.health.error.as_deref() == Some("indexer unavailable"),
            "unexpected indexer health error"
        );
        Ok(())
    }

    fn ok<'a, T: Send + 'a>(value: T) -> OverviewProbeFuture<'a, T> {
        Box::pin(async move { Ok(value) })
    }

    fn err<'a, T: Send + 'a>(message: &'static str) -> OverviewProbeFuture<'a, T> {
        Box::pin(async move { Err(anyhow!(message)) })
    }
}
