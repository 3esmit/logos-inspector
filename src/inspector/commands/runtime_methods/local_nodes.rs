use anyhow::{Context as _, Result, bail};
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{
    inspection::NetworkScope,
    local_devnet_list as local_devnet_list_report,
    local_nodes::local_module_catalog as local_module_catalog_report,
    local_nodes::local_node_package_catalog as local_node_package_catalog_report,
    local_nodes::{
        ChannelIndexerConfigRequest, NodeKind, basecamp_channel_indexer_config,
        basecamp_channel_indexer_status, basecamp_local_nodes_status,
        basecamp_save_channel_indexer_config,
        channel_indexer_config as channel_indexer_config_report,
        channel_indexer_status as channel_indexer_status_report,
        local_node_config as local_node_config_report,
        save_channel_indexer_config as save_channel_indexer_config_report,
        save_local_node_config as save_local_node_config_report,
        validate_channel_indexer_config as validate_channel_indexer_config_report,
        validate_local_node_config as validate_local_node_config_report,
    },
    local_nodes_status as local_nodes_status_report,
    modules::logos_core::{ModuleTransportKind, SharedModuleTransport},
    support::args::Args,
};

use super::super::value::to_value;
use super::RuntimeMethodEntry;

pub(super) const METHOD_CATALOG: &[RuntimeMethodEntry] = &[
    RuntimeMethodEntry::with_module_transport("localNodesStatus", local_nodes_status),
    RuntimeMethodEntry::sync("localNodeConfig", local_node_config),
    RuntimeMethodEntry::sync("localNodeConfigValidate", local_node_config_validate),
    RuntimeMethodEntry::sync("localNodeConfigSave", local_node_config_save),
    RuntimeMethodEntry::with_module_transport("channelIndexerConfig", channel_indexer_config),
    RuntimeMethodEntry::sync(
        "channelIndexerConfigValidate",
        channel_indexer_config_validate,
    ),
    RuntimeMethodEntry::with_module_transport(
        "channelIndexerConfigSave",
        channel_indexer_config_save,
    ),
    RuntimeMethodEntry::with_module_transport("channelIndexerStatus", channel_indexer_status),
    RuntimeMethodEntry::sync("localDevnetList", local_devnet_list),
    RuntimeMethodEntry::with_runtime("localNodePackageCatalog", local_node_package_catalog),
    RuntimeMethodEntry::with_module_transport("localModuleCatalog", local_module_catalog),
];

pub(super) fn local_nodes_status(
    runtime: &Runtime,
    args: Value,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    let args = Args::new(args)?;
    let profile = args.optional_string(0).unwrap_or("default");
    if module_transport.kind() == ModuleTransportKind::Module {
        return to_value(
            runtime.block_on(basecamp_local_nodes_status(profile, &module_transport))?,
        );
    }
    to_value(local_nodes_status_report(profile)?)
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

pub(super) fn channel_indexer_config(
    runtime: &Runtime,
    args: Value,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    let args = Args::new(args)?;
    let profile = args.optional_string(0).unwrap_or("default");
    let request = channel_indexer_config_request(&args, 1)?;
    if module_transport.kind() == ModuleTransportKind::Module {
        return to_value(runtime.block_on(basecamp_channel_indexer_config(
            profile,
            &request,
            &module_transport,
        ))?);
    }
    to_value(channel_indexer_config_report(profile, request)?)
}

pub(super) fn channel_indexer_config_validate(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(validate_channel_indexer_config_report(
        args.optional_string(0).unwrap_or("default"),
        channel_indexer_config_request(&args, 1)?,
        args.string(2, "Channel Indexer configuration text")?,
    )?)
}

pub(super) fn channel_indexer_config_save(
    runtime: &Runtime,
    args: Value,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    let args = Args::new(args)?;
    let profile = args.optional_string(0).unwrap_or("default");
    let request = channel_indexer_config_request(&args, 1)?;
    let text = args.string(2, "Channel Indexer configuration text")?;
    let revision = args.string(3, "Channel Indexer configuration revision")?;
    let confirmation = args.optional_string(4);
    if module_transport.kind() == ModuleTransportKind::Module {
        return to_value(runtime.block_on(basecamp_save_channel_indexer_config(
            profile,
            &request,
            text,
            revision,
            confirmation,
            &module_transport,
        ))?);
    }
    to_value(save_channel_indexer_config_report(
        profile,
        request,
        text,
        revision,
        confirmation,
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

pub(super) fn channel_indexer_status(
    runtime: &Runtime,
    args: Value,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    let args = Args::new(args)?;
    let network_scope = serde_json::from_value::<NetworkScope>(
        args.value(1)
            .cloned()
            .context("Channel Indexer network scope is required")?,
    )
    .context("Channel Indexer network scope is invalid")?;
    let profile = args.optional_string(0).unwrap_or("default");
    let channel_id = args.string(2, "Channel Indexer Channel ID")?;
    if module_transport.kind() == ModuleTransportKind::Module {
        return to_value(runtime.block_on(basecamp_channel_indexer_status(
            profile,
            &module_transport,
            &network_scope,
            channel_id,
        ))?);
    }
    to_value(channel_indexer_status_report(
        profile,
        &network_scope,
        channel_id,
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

pub(super) fn local_module_catalog(
    _runtime: &Runtime,
    args: Value,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    if module_transport.kind() == ModuleTransportKind::Module {
        bail!("local module package management is unavailable inside Basecamp");
    }
    let args = Args::new(args)?;
    to_value(local_module_catalog_report(args.optional_string(0))?)
}
