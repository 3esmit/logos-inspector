use anyhow::{Context as _, Result};
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{
    inspection::NetworkScope,
    local_devnet_list as local_devnet_list_report,
    local_nodes::local_node_package_catalog as local_node_package_catalog_report,
    local_nodes::{
        ChannelIndexerConfigRequest, NodeKind,
        channel_indexer_config as channel_indexer_config_report,
        channel_indexer_status as channel_indexer_status_report,
        local_node_config as local_node_config_report,
        save_channel_indexer_config as save_channel_indexer_config_report,
        save_local_node_config as save_local_node_config_report,
        validate_channel_indexer_config as validate_channel_indexer_config_report,
        validate_local_node_config as validate_local_node_config_report,
    },
    local_nodes_status as local_nodes_status_report,
    support::args::Args,
};

use super::super::value::to_value;
use super::RuntimeMethodEntry;

pub(super) const METHOD_CATALOG: &[RuntimeMethodEntry] = &[
    RuntimeMethodEntry::sync("localNodesStatus", local_nodes_status),
    RuntimeMethodEntry::sync("localNodeConfig", local_node_config),
    RuntimeMethodEntry::sync("localNodeConfigValidate", local_node_config_validate),
    RuntimeMethodEntry::sync("localNodeConfigSave", local_node_config_save),
    RuntimeMethodEntry::sync("channelIndexerConfig", channel_indexer_config),
    RuntimeMethodEntry::sync(
        "channelIndexerConfigValidate",
        channel_indexer_config_validate,
    ),
    RuntimeMethodEntry::sync("channelIndexerConfigSave", channel_indexer_config_save),
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

pub(super) fn local_node_config(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(local_node_config_report(
        args.optional_string(0).unwrap_or("default"),
        node_kind(&args, 1)?,
    )?)
}

pub(super) fn local_node_config_validate(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(validate_local_node_config_report(
        args.optional_string(0).unwrap_or("default"),
        node_kind(&args, 1)?,
        args.string(2, "node configuration text")?,
    )?)
}

pub(super) fn local_node_config_save(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(save_local_node_config_report(
        args.optional_string(0).unwrap_or("default"),
        node_kind(&args, 1)?,
        args.string(2, "node configuration text")?,
        args.string(3, "node configuration revision")?,
        args.optional_string(4),
    )?)
}

pub(super) fn channel_indexer_config(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(channel_indexer_config_report(
        args.optional_string(0).unwrap_or("default"),
        channel_indexer_config_request(&args, 1)?,
    )?)
}

pub(super) fn channel_indexer_config_validate(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(validate_channel_indexer_config_report(
        args.optional_string(0).unwrap_or("default"),
        channel_indexer_config_request(&args, 1)?,
        args.string(2, "Channel Indexer configuration text")?,
    )?)
}

pub(super) fn channel_indexer_config_save(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(save_channel_indexer_config_report(
        args.optional_string(0).unwrap_or("default"),
        channel_indexer_config_request(&args, 1)?,
        args.string(2, "Channel Indexer configuration text")?,
        args.string(3, "Channel Indexer configuration revision")?,
        args.optional_string(4),
    )?)
}

fn node_kind(args: &Args, index: usize) -> Result<NodeKind> {
    serde_json::from_value(
        args.value(index)
            .cloned()
            .context("local node kind is required")?,
    )
    .context("local node kind is invalid")
}

fn channel_indexer_config_request(
    args: &Args,
    index: usize,
) -> Result<ChannelIndexerConfigRequest> {
    serde_json::from_value(
        args.value(index)
            .cloned()
            .context("Channel Indexer configuration request is required")?,
    )
    .context("Channel Indexer configuration request is invalid")
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
