use anyhow::{Context as _, Result};
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{
    inspection::NetworkScope, local_devnet_list as local_devnet_list_report,
    local_nodes::channel_indexer_status as channel_indexer_status_report,
    local_nodes::local_node_package_catalog as local_node_package_catalog_report,
    local_nodes_status as local_nodes_status_report, support::args::Args,
};

use super::super::value::to_value;
use super::RuntimeMethodEntry;

pub(super) const METHOD_CATALOG: &[RuntimeMethodEntry] = &[
    RuntimeMethodEntry::sync("localNodesStatus", local_nodes_status),
    RuntimeMethodEntry::sync("channelIndexerStatus", channel_indexer_status),
    RuntimeMethodEntry::sync("localDevnetList", local_devnet_list),
    RuntimeMethodEntry::with_runtime("localNodePackageCatalog", local_node_package_catalog),
];

pub(super) fn local_nodes_status(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(local_nodes_status_report(
        args.optional_string(0).unwrap_or("default"),
    )?)
}

pub(super) fn channel_indexer_status(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let network_scope = serde_json::from_value::<NetworkScope>(
        args.value(1)
            .cloned()
            .context("Channel Indexer network scope is required")?,
    )
    .context("Channel Indexer network scope is invalid")?;
    to_value(channel_indexer_status_report(
        args.optional_string(0).unwrap_or("default"),
        &network_scope,
        args.string(2, "Channel Indexer Channel ID")?,
    )?)
}

pub(super) fn local_devnet_list(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(local_devnet_list_report(
        args.optional_string(0).unwrap_or("default"),
    )?)
}

pub(super) fn local_node_package_catalog(_runtime: &Runtime, args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(local_node_package_catalog_report(args.optional_string(0))?)
}
