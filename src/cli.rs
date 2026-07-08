mod args;
mod io;
mod planner;
mod runtime;

use anyhow::Result;

pub use args::{Args, CliArgs, EndpointArgs, Mode, SequencerArgs};
use io::{print_json, print_text_value};
use runtime::{CliCommandRuntime, maybe_bootstrap_default_local_indexer};

pub fn run(args: CliArgs) -> Result<()> {
    let invocation = args.into_command().invocation()?;
    if let Some(endpoint) = invocation.bootstrap_indexer_endpoint.as_deref() {
        maybe_bootstrap_default_local_indexer(endpoint)?;
    }
    let runtime = CliCommandRuntime::new()?;
    let value = runtime.call(invocation.method, invocation.args)?;
    match invocation.output {
        planner::CliOutput::Json => print_json(&value),
        planner::CliOutput::Text => print_text_value(&value),
    }
}
