mod args;
mod io;
mod planner;
mod runtime;

use anyhow::Result;

pub use args::{Args, CliArgs, EndpointArgs, Mode};
use io::print_json;
use runtime::CliCommandRuntime;

pub fn run(args: CliArgs) -> Result<()> {
    let invocation = args.into_command().invocation()?;
    let runtime = CliCommandRuntime::new()?;
    let value = runtime.call(invocation.method, invocation.args)?;
    print_json(&value)
}
