use anyhow::{Result, bail};
use serde_json::{Value, json};
use tokio::runtime::Runtime;

use super::{
    command_catalog::{InspectorCommand, inspector_command},
    operations::RuntimeOperationInterface,
    runtime_methods,
};
use crate::source_routing::Args;

pub(crate) struct DispatchContext<'a> {
    pub(crate) runtime: &'a Runtime,
    pub(crate) operations: &'a RuntimeOperationInterface,
    pub(crate) call_core_module: &'a dyn Fn(&str, &str, Value) -> Result<Value>,
}

pub(crate) fn dispatch(
    context: &DispatchContext<'_>,
    method: &str,
    args: Value,
) -> Result<Option<Value>> {
    let Some(command) = inspector_command(method) else {
        return Ok(None);
    };
    match command {
        InspectorCommand::Operation(command) => context
            .operations
            .bridge_call(context.runtime, command, &args)
            .map(Some),
        InspectorCommand::Runtime(method) => {
            runtime_methods::handle(context.runtime, method, args).map(Some)
        }
        InspectorCommand::CapabilitiesReport => {
            bail!("capability_module does not expose Inspector capability listing")
        }
        InspectorCommand::CallModule => {
            let args = Args::new(args)?;
            let module = args.string(0, "module name")?;
            let method = args.string(1, "method name")?;
            let call_args = args.value(2).cloned().unwrap_or_else(|| json!([]));
            (context.call_core_module)(module, method, call_args).map(Some)
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{Context as _, Result};
    use serde_json::json;
    use tokio::runtime::Runtime;

    use super::*;
    use crate::inspector::operations::RuntimeOperationInterface;

    #[test]
    fn dispatch_handles_call_module_special() -> Result<()> {
        let runtime = Runtime::new().context("runtime")?;
        let operations = RuntimeOperationInterface::default();
        let call_core_module = |module: &str, method: &str, args: Value| {
            Ok(json!({
                "module": module,
                "method": method,
                "args": args,
            }))
        };
        let context = DispatchContext {
            runtime: &runtime,
            operations: &operations,
            call_core_module: &call_core_module,
        };

        let value = dispatch(
            &context,
            "callModule",
            json!(["module_a", "method_b", ["arg"]]),
        )?
        .context("callModule should dispatch")?;

        if value
            != json!({
                "module": "module_a",
                "method": "method_b",
                "args": ["arg"],
            })
        {
            bail!("unexpected callModule dispatch value: {value}");
        }
        Ok(())
    }

    #[test]
    fn dispatch_reports_unknown_methods_as_none() -> Result<()> {
        let runtime = Runtime::new().context("runtime")?;
        let operations = RuntimeOperationInterface::default();
        let call_core_module = |_: &str, _: &str, _: Value| unreachable!();
        let context = DispatchContext {
            runtime: &runtime,
            operations: &operations,
            call_core_module: &call_core_module,
        };

        if dispatch(&context, "missingMethod", json!([]))?.is_some() {
            bail!("unknown method should not dispatch");
        }
        Ok(())
    }
}
