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
    let (method, args, completion) = invocation.into_parts();
    let runtime = CliCommandRuntime::new()?;
    let (value, post_result_error) = if completion.requires_signal_aware_shutdown() {
        runtime.call_signal_aware(method, args)?.into_parts()
    } else {
        (runtime.call(method, args)?, None)
    };
    print_json(&value)?;
    completion.validate(&value)?;
    match post_result_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
