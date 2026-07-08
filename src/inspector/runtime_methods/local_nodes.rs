use anyhow::Result;
use serde_json::Value;

use crate::{
    local_devnet_list as local_devnet_list_report, local_nodes_status as local_nodes_status_report,
    source_routing::Args,
};

use super::super::bridge::to_value;

pub(super) fn local_nodes_status(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(local_nodes_status_report(
        args.optional_string(0).unwrap_or("default"),
    )?)
}

pub(super) fn local_devnet_list(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(local_devnet_list_report(
        args.optional_string(0).unwrap_or("default"),
    )?)
}
