use std::sync::Arc;

use anyhow::{Context as _, Result};
use serde_json::Value;
use tokio::runtime::Runtime;

use super::{decode_object_request, zone_catalog::ZoneCatalogCommandInterface};
use crate::{
    inspection::l2::{
        L2ReadErrorCode, L2ReadFailure, ZoneL2AccountActivityQuery, ZoneL2AccountNoncesQuery,
        ZoneL2AccountQuery, ZoneL2BlockDetailQuery, ZoneL2BlocksQuery, ZoneL2CommitmentProofQuery,
        ZoneL2ProgramsQuery, ZoneL2Request, ZoneL2Router, ZoneL2TransactionQuery,
        ZoneL2TransactionTraceQuery, ZoneL2TransfersQuery,
    },
    support::bridge_envelope::structured_bridge_error,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ZoneL2Command {
    Blocks,
    BlockDetail,
    Transaction,
    TransactionTrace,
    Account,
    AccountActivity,
    Programs,
    CommitmentProof,
    AccountNonces,
    Transfers,
}

const COMMANDS: [(&str, ZoneL2Command); 10] = [
    ("zoneL2Blocks", ZoneL2Command::Blocks),
    ("zoneL2BlockDetail", ZoneL2Command::BlockDetail),
    ("zoneL2Transaction", ZoneL2Command::Transaction),
    ("zoneL2TransactionTrace", ZoneL2Command::TransactionTrace),
    ("zoneL2Account", ZoneL2Command::Account),
    ("zoneL2AccountActivity", ZoneL2Command::AccountActivity),
    ("zoneL2Programs", ZoneL2Command::Programs),
    ("zoneL2CommitmentProof", ZoneL2Command::CommitmentProof),
    ("zoneL2AccountNonces", ZoneL2Command::AccountNonces),
    ("zoneL2Transfers", ZoneL2Command::Transfers),
];

pub(crate) fn zone_l2_command(method: &str) -> Option<ZoneL2Command> {
    COMMANDS
        .iter()
        .find_map(|(name, command)| (*name == method).then_some(*command))
}

#[cfg(test)]
pub(crate) fn zone_l2_command_names() -> impl Iterator<Item = &'static str> {
    COMMANDS.iter().map(|(name, _)| *name)
}

pub(crate) struct ZoneL2CommandInterface {
    catalog: Arc<ZoneCatalogCommandInterface>,
    router: ZoneL2Router,
}

impl ZoneL2CommandInterface {
    #[must_use]
    pub(crate) fn new(catalog: Arc<ZoneCatalogCommandInterface>) -> Self {
        Self {
            catalog,
            router: ZoneL2Router::default(),
        }
    }

    pub(crate) fn bridge_call(
        &self,
        runtime: &Runtime,
        command: ZoneL2Command,
        args: &Value,
    ) -> Result<Value> {
        macro_rules! execute {
            ($query:ty, $method:ident, $name:literal) => {{
                let request: ZoneL2Request<$query> = decode_object_request(args, $name)?;
                let facts = match self.catalog.context_snapshot(runtime) {
                    Ok(facts) => facts,
                    Err(_) => {
                        return structured_failure(
                            &request,
                            L2ReadFailure::new(
                                L2ReadErrorCode::Internal,
                                "Zone state could not be read",
                            ),
                        );
                    }
                };
                match runtime.block_on(self.router.$method(&facts, request.clone())) {
                    Ok(report) => serde_json::to_value(report).context(concat!(
                        "failed to serialize ",
                        $name,
                        " report"
                    )),
                    Err(failure) => structured_failure(&request, failure),
                }
            }};
        }

        match command {
            ZoneL2Command::Blocks => execute!(ZoneL2BlocksQuery, blocks, "zoneL2Blocks"),
            ZoneL2Command::BlockDetail => {
                execute!(ZoneL2BlockDetailQuery, block_detail, "zoneL2BlockDetail")
            }
            ZoneL2Command::Transaction => {
                execute!(ZoneL2TransactionQuery, transaction, "zoneL2Transaction")
            }
            ZoneL2Command::TransactionTrace => execute!(
                ZoneL2TransactionTraceQuery,
                transaction_trace,
                "zoneL2TransactionTrace"
            ),
            ZoneL2Command::Account => execute!(ZoneL2AccountQuery, account, "zoneL2Account"),
            ZoneL2Command::AccountActivity => execute!(
                ZoneL2AccountActivityQuery,
                account_activity,
                "zoneL2AccountActivity"
            ),
            ZoneL2Command::Programs => execute!(ZoneL2ProgramsQuery, programs, "zoneL2Programs"),
            ZoneL2Command::CommitmentProof => execute!(
                ZoneL2CommitmentProofQuery,
                commitment_proof,
                "zoneL2CommitmentProof"
            ),
            ZoneL2Command::AccountNonces => execute!(
                ZoneL2AccountNoncesQuery,
                account_nonces,
                "zoneL2AccountNonces"
            ),
            ZoneL2Command::Transfers => {
                execute!(ZoneL2TransfersQuery, transfers, "zoneL2Transfers")
            }
        }
    }
}

fn structured_failure<T>(request: &ZoneL2Request<T>, failure: L2ReadFailure) -> Result<Value> {
    Err(structured_bridge_error(
        failure.message.clone(),
        failure.details(request),
    )?)
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};
    use serde_json::json;

    use super::*;

    #[test]
    fn l2_request_decoder_rejects_connection_targets() -> Result<()> {
        let args = json!([{
            "context": {
                "network_scope": { "kind": "genesis_id", "genesis_id": "11".repeat(32) },
                "channel_id": "22".repeat(32),
                "zone_kind": "sequencer_zone",
                "selected_sequencer_source_id": null,
                "indexer_source_id": null,
                "source_config_revision": 0,
                "context_revision": 1
            },
            "request_revision": 1,
            "query": {
                "exact_source_id": null,
                "endpoint": "https://forbidden.example"
            }
        }]);
        if decode_object_request::<ZoneL2Request<ZoneL2ProgramsQuery>>(&args, "zoneL2Programs")
            .is_ok()
        {
            bail!("Zone L2 request accepted an endpoint");
        }
        Ok(())
    }
}
