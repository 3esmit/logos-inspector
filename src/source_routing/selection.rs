use crate::support::args::Args;
use anyhow::Result;

use super::{CoreEndpointMode, CoreSourceMode, core, core::layer::BedrockAdapter};

pub(crate) struct SourceEndpoint<'a> {
    pub(crate) mode: CoreEndpointMode,
    pub(crate) endpoint: &'a str,
    pub(crate) next_index: usize,
    pub(crate) module: &'static str,
}

impl<'a> SourceEndpoint<'a> {
    #[must_use]
    pub(crate) const fn adapter(&self) -> BedrockAdapter<'a> {
        match self.mode {
            CoreEndpointMode::Rpc => BedrockAdapter::rpc(self.endpoint),
            CoreEndpointMode::Module => BedrockAdapter::module(),
        }
    }
}

impl Args {
    pub(crate) fn source_endpoint(&self, index: usize, label: &str) -> Result<SourceEndpoint<'_>> {
        let first = self.string(index, label)?;
        if let Some(mode) = CoreSourceMode::from_token(first) {
            let adapter = match mode {
                CoreSourceMode::Rpc => BedrockAdapter::rpc(self.string(index + 1, label)?),
                CoreSourceMode::Module => BedrockAdapter::module(),
            };
            return Ok(source_endpoint_from_adapter(
                adapter,
                match mode {
                    CoreSourceMode::Rpc => index + 2,
                    CoreSourceMode::Module => index + 1,
                },
                core::adapters::BLOCKCHAIN_MODULE,
            ));
        }
        Ok(source_endpoint_from_adapter(
            BedrockAdapter::rpc(first),
            index + 1,
            core::adapters::BLOCKCHAIN_MODULE,
        ))
    }
}

fn source_endpoint_from_adapter<'a>(
    adapter: BedrockAdapter<'a>,
    next_index: usize,
    module: &'static str,
) -> SourceEndpoint<'a> {
    match adapter {
        BedrockAdapter::Rpc { endpoint } => SourceEndpoint {
            mode: CoreEndpointMode::Rpc,
            endpoint,
            next_index,
            module,
        },
        BedrockAdapter::Module => SourceEndpoint {
            mode: CoreEndpointMode::Module,
            endpoint: "",
            next_index,
            module,
        },
    }
}
