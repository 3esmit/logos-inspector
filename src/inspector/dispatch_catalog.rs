use anyhow::{Result, bail};
use serde_json::{Value, json};
use tokio::runtime::Runtime;

use super::{
    operations::{RuntimeOperationInterface, operation_route},
    runtime_methods,
};
use crate::source_routing::Args;

pub(crate) struct DispatchContext<'a> {
    pub(crate) runtime: &'a Runtime,
    pub(crate) operations: &'a RuntimeOperationInterface,
    pub(crate) call_core_module: &'a dyn Fn(&str, &str, Value) -> Result<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InspectorCommand {
    Operation,
    Runtime,
    CapabilitiesReport,
    CallModule,
}

const OPERATION_CONTROL_METHODS: &[&str] = &[
    "nodeOperationStart",
    "nodeOperationStatus",
    "nodeOperationEvents",
    "nodeOperationCancel",
    "storageOperationStatus",
    "storageOperationCancel",
];

pub(crate) fn dispatch(
    context: &DispatchContext<'_>,
    method: &str,
    args: Value,
) -> Result<Option<Value>> {
    let Some(command) = lookup(method) else {
        return Ok(None);
    };
    match command {
        InspectorCommand::Operation => {
            context
                .operations
                .try_bridge_call(context.runtime, method, &args)
        }
        InspectorCommand::Runtime => runtime_methods::try_handle(context.runtime, method, args),
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

fn lookup(method: &str) -> Option<InspectorCommand> {
    if OPERATION_CONTROL_METHODS.contains(&method) || operation_route(method).is_some() {
        return Some(InspectorCommand::Operation);
    }
    if runtime_methods::is_runtime_method(method) {
        return Some(InspectorCommand::Runtime);
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

    use anyhow::{Context as _, Result};
    use serde_json::json;
    use tokio::runtime::Runtime;

    use super::*;
    use crate::inspector::operations::{RuntimeOperationInterface, operation_method_names};

    #[test]
    fn catalog_owns_operation_names() -> Result<()> {
        for method in operation_method_names().chain(OPERATION_CONTROL_METHODS.iter().copied()) {
            if lookup(method) != Some(InspectorCommand::Operation) {
                bail!("operation method `{method}` missing from dispatch catalog");
            }
        }
        Ok(())
    }

    #[test]
    fn catalog_owns_runtime_names() -> Result<()> {
        for method in runtime_methods::RUNTIME_METHODS {
            let name = method.as_str();
            if lookup(name) != Some(InspectorCommand::Runtime) {
                bail!("runtime method `{name}` missing from dispatch catalog");
            }
        }
        Ok(())
    }

    #[test]
    fn catalog_names_are_unique() -> Result<()> {
        let mut names = HashSet::new();
        for method in operation_method_names()
            .chain(OPERATION_CONTROL_METHODS.iter().copied())
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
