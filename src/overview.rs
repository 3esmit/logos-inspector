use serde::Serialize;
use serde_json::Value;

use crate::{
    indexer_health, last_sequencer_block_id, logos_node_cryptarchia_info, probe::ProbeField,
    raw_json_rpc_optional_result, sequencer_health, sequencer_program_ids,
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
    let node = NodeProbe {
        endpoint: node_endpoint.to_owned(),
        consensus: match logos_node_cryptarchia_info(node_endpoint).await {
            Ok(value) => ProbeField::ok(value),
            Err(err) => ProbeField::err(err),
        },
    };

    let sequencer = ServiceProbe {
        endpoint: sequencer_endpoint.to_owned(),
        health: match sequencer_health(sequencer_endpoint).await {
            Ok(()) => ProbeField::ok("ok"),
            Err(err) => ProbeField::err(err),
        },
        head: match last_sequencer_block_id(sequencer_endpoint).await {
            Ok(head) => ProbeField::ok(head),
            Err(err) => ProbeField::err(err),
        },
        programs: Some(match sequencer_program_ids(sequencer_endpoint).await {
            Ok(programs) => ProbeField::ok(programs.len()),
            Err(err) => ProbeField::err(err),
        }),
    };

    let indexer_head = match raw_json_rpc_optional_result(
        indexer_endpoint,
        "getLastFinalizedBlockId",
        Value::Array(vec![]),
    )
    .await
    {
        Ok(value) => ProbeField::ok(value),
        Err(err) => ProbeField::err(err),
    };
    let indexer_health = match indexer_health(indexer_endpoint).await {
        Ok(value) => ProbeField::ok(value),
        Err(err) => ProbeField::err(err),
    };

    let indexer = ServiceProbe {
        endpoint: indexer_endpoint.to_owned(),
        health: indexer_health,
        head: indexer_head,
        programs: None,
    };

    OverviewReport {
        product: "Logos Inspector",
        scopes: inspector_scopes(),
        node,
        sequencer,
        indexer,
    }
}
