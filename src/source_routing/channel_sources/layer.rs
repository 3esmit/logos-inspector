use anyhow::{Context as _, Result};
use serde_json::{Value, json};

use crate::source_routing::adapter::SourceModePolicy;

use super::{ChannelSourceRole, indexer, sequencer};

pub(crate) use indexer::SOURCE_MODES as INDEXER_SOURCE_MODES;
pub(crate) use sequencer::SOURCE_MODES as SEQUENCER_SOURCE_MODES;

#[must_use]
pub(crate) fn source_modes_for_role(role: ChannelSourceRole) -> &'static [SourceModePolicy] {
    match role {
        ChannelSourceRole::Sequencer => sequencer::SOURCE_MODES,
        ChannelSourceRole::Indexer => indexer::SOURCE_MODES,
    }
}

#[must_use]
pub(crate) const fn module_id_for_role(role: ChannelSourceRole) -> &'static str {
    match role {
        ChannelSourceRole::Sequencer => sequencer::MODULE_ID,
        ChannelSourceRole::Indexer => indexer::MODULE_ID,
    }
}

#[must_use]
pub(crate) const fn managed_sequencer_program() -> &'static str {
    sequencer::MANAGED_PROGRAM
}

#[must_use]
pub(crate) fn managed_sequencer_config(
    network_id: &str,
    data_dir: &str,
    endpoint: Option<&str>,
    port: Option<u16>,
) -> Value {
    sequencer::managed_config(network_id, data_dir, endpoint, port)
}

#[must_use]
pub(crate) fn managed_indexer_config(
    network_id: &str,
    data_dir: &str,
    endpoint: Option<&str>,
    port: Option<u16>,
) -> Value {
    indexer::managed_config(network_id, data_dir, endpoint, port)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutionZoneReadErrorKind {
    Unavailable,
    Protocol,
    Capability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExecutionZoneReadError {
    pub(crate) kind: ExecutionZoneReadErrorKind,
}

pub(crate) type ExecutionZoneReadResult<T> = std::result::Result<T, ExecutionZoneReadError>;

pub(super) fn managed_config(
    node: &str,
    network_id: &str,
    data_dir: &str,
    endpoint: Option<&str>,
    port: Option<u16>,
) -> Value {
    json!({
        "network_id": network_id,
        "node": node,
        "data_dir": data_dir,
        "endpoint": endpoint,
        "port": port,
    })
}

pub(super) fn optional_u64(value: Value) -> ExecutionZoneReadResult<Option<u64>> {
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        .map(Some)
        .ok_or(ExecutionZoneReadError {
            kind: ExecutionZoneReadErrorKind::Protocol,
        })
}

pub(super) const fn capability_error() -> ExecutionZoneReadError {
    ExecutionZoneReadError {
        kind: ExecutionZoneReadErrorKind::Capability,
    }
}

pub(super) fn map_read_error(error: anyhow::Error) -> ExecutionZoneReadError {
    let kind = if crate::lez::is_evidence_capability_error(&error) {
        ExecutionZoneReadErrorKind::Capability
    } else if crate::lez::is_evidence_protocol_error(&error) {
        ExecutionZoneReadErrorKind::Protocol
    } else {
        ExecutionZoneReadErrorKind::Unavailable
    };
    ExecutionZoneReadError { kind }
}

pub(super) async fn blocking_module_call<T, F>(label: &'static str, call: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(call)
        .await
        .with_context(|| format!("{label} worker failed"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_routing::adapter::contract_tests::assert_layer_contract;

    #[test]
    fn execution_zone_node_layers_satisfy_shared_seam_contract() {
        assert_layer_contract("execution_zone.sequencer", SEQUENCER_SOURCE_MODES);
        assert_layer_contract("execution_zone.indexer", INDEXER_SOURCE_MODES);
    }

    #[test]
    fn module_ids_are_owned_by_node_role() {
        assert_eq!(module_id_for_role(ChannelSourceRole::Sequencer), "lez_core");
        assert_eq!(
            module_id_for_role(ChannelSourceRole::Indexer),
            "lez_indexer_module"
        );
    }

    #[test]
    fn node_layers_share_transport_error_classification() {
        let protocol = map_read_error(crate::lez::evidence_protocol_error("invalid evidence"));
        let capability =
            map_read_error(crate::lez::evidence_capability_error("missing capability"));
        let unavailable = map_read_error(anyhow::anyhow!("transport failed"));

        assert_eq!(protocol.kind, ExecutionZoneReadErrorKind::Protocol);
        assert_eq!(capability.kind, ExecutionZoneReadErrorKind::Capability);
        assert_eq!(unavailable.kind, ExecutionZoneReadErrorKind::Unavailable);
    }
}
