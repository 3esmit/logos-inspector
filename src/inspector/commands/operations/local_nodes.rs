use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::{LocalNodeActionRequest, local_nodes_action, source_routing::Args};

use super::super::value::{blocking_value, to_value};
use super::NodeOperationRequest;
use super::spec::{OperationCatalogEntry, OperationDomain, OperationMethod};

pub(super) const OPERATION_CATALOG: &[OperationCatalogEntry] = &[OperationCatalogEntry::new(
    OperationMethod::LocalNodesAction,
    "localNodesAction",
    OperationDomain::LocalNodes,
    "Local node action",
)];

pub(super) async fn execute_local_nodes_action(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let action_request = serde_json::from_value::<LocalNodeActionRequest>(
        args.value(1)
            .cloned()
            .context("local node action request is required")?,
    )
    .context("failed to parse local node action request")?;
    let profile = args.optional_string(0).unwrap_or("default").to_owned();
    let confirmation = args.optional_string(2).map(ToOwned::to_owned);
    blocking_value("local node action", move || {
        to_value(local_nodes_action(
            &profile,
            action_request,
            confirmation.as_deref(),
        )?)
    })
    .await
}
