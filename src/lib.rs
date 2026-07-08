pub mod blockchain;
#[cfg(feature = "cli")]
pub mod cli;
pub mod decode;

mod inspector {
    pub mod bridge;

    pub(crate) mod command_surface;
    pub(crate) mod commands;
    pub(crate) mod value;
}
pub mod lez;
pub mod local_nodes;
pub mod modules;
pub mod overview;
pub mod probe;
mod public_surface;
pub mod rpc;
pub mod social;
pub mod source_routing;
pub(crate) mod support;
pub mod wallet;

pub use public_surface::{
    bridge, idl_decode, inspection, local_indexer, logoscore, network, program_decode,
};

// Compatibility shims. First-party code should prefer the domain modules above.
pub use public_surface::compat::*;
pub(crate) use support::entity_id::{normalize_account_id_text, parse_account_id, parse_hash};
pub(crate) use support::http_response::response_excerpt;
pub(crate) use support::json_value::{enum_payload, value_list_strings, value_to_string};

pub const ACCOUNT_TRANSACTION_LIMIT: usize = 20;
