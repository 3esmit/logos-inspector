use anyhow::{Result, bail};
use serde_json::{Value, json};
use tokio::runtime::Runtime;

use super::{
    operations::{OperationBridgeCommand, RuntimeOperationInterface, operation_bridge_command},
    runtime_methods::{self, RuntimeMethod},
};
use crate::source_routing::Args;

pub(crate) struct DispatchContext<'a> {
    pub(crate) runtime: &'a Runtime,
    pub(crate) operations: &'a RuntimeOperationInterface,
    pub(crate) call_core_module: &'a dyn Fn(&str, &str, Value) -> Result<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InspectorCommand {
    Operation(OperationBridgeCommand),
    Runtime(RuntimeMethod),
    CapabilitiesReport,
    CallModule,
}

pub(crate) fn dispatch_inspector_command(
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

fn inspector_command(method: &str) -> Option<InspectorCommand> {
    if let Some(command) = operation_bridge_command(method) {
        return Some(InspectorCommand::Operation(command));
    }
    if let Some(method) = RuntimeMethod::from_str(method) {
        return Some(InspectorCommand::Runtime(method));
    }
    match method {
        "capabilitiesReport" => Some(InspectorCommand::CapabilitiesReport),
        "callModule" => Some(InspectorCommand::CallModule),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use anyhow::{Context as _, Result, bail};
    use serde_json::json;
    use tokio::runtime::Runtime;

    use super::*;
    use crate::inspector::{operations, runtime_methods};

    #[test]
    fn surface_owns_operation_names() -> Result<()> {
        for method in operations::operation_bridge_command_names() {
            if !matches!(
                inspector_command(method),
                Some(InspectorCommand::Operation(_))
            ) {
                bail!("operation method `{method}` missing from inspector surface");
            }
        }
        Ok(())
    }

    #[test]
    fn surface_owns_runtime_names() -> Result<()> {
        for method in runtime_methods::RUNTIME_METHODS {
            let name = method.as_str();
            if inspector_command(name) != Some(InspectorCommand::Runtime(*method)) {
                bail!("runtime method `{name}` missing from inspector surface");
            }
        }
        Ok(())
    }

    #[test]
    fn surface_names_are_unique() -> Result<()> {
        let mut names = HashSet::new();
        for method in operations::operation_bridge_command_names()
            .chain(
                runtime_methods::RUNTIME_METHODS
                    .iter()
                    .map(|method| method.as_str()),
            )
            .chain(["capabilitiesReport", "callModule"])
        {
            if !names.insert(method) {
                bail!("duplicate inspector method `{method}`");
            }
        }
        Ok(())
    }

    #[test]
    fn surface_dispatches_call_module_special() -> Result<()> {
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

        let value = dispatch_inspector_command(
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
    fn surface_reports_unknown_methods_as_none() -> Result<()> {
        let runtime = Runtime::new().context("runtime")?;
        let operations = RuntimeOperationInterface::default();
        let call_core_module = |_: &str, _: &str, _: Value| unreachable!();
        let context = DispatchContext {
            runtime: &runtime,
            operations: &operations,
            call_core_module: &call_core_module,
        };

        if dispatch_inspector_command(&context, "missingMethod", json!([]))?.is_some() {
            bail!("unknown method should not dispatch");
        }
        Ok(())
    }
}
