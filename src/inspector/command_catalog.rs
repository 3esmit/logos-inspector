use super::{
    operations::{OperationBridgeCommand, operation_bridge_command},
    runtime_methods::RuntimeMethod,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InspectorCommand {
    Operation(OperationBridgeCommand),
    Runtime(RuntimeMethod),
    CapabilitiesReport,
    CallModule,
}

pub(crate) fn inspector_command(method: &str) -> Option<InspectorCommand> {
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

    use anyhow::{Result, bail};

    use super::*;
    use crate::inspector::{operations, runtime_methods};

    #[test]
    fn catalog_owns_operation_names() -> Result<()> {
        for method in operations::operation_bridge_command_names() {
            if !matches!(
                inspector_command(method),
                Some(InspectorCommand::Operation(_))
            ) {
                bail!("operation method `{method}` missing from inspector catalog");
            }
        }
        Ok(())
    }

    #[test]
    fn catalog_owns_runtime_names() -> Result<()> {
        for method in runtime_methods::RUNTIME_METHODS {
            let name = method.as_str();
            if inspector_command(name) != Some(InspectorCommand::Runtime(*method)) {
                bail!("runtime method `{name}` missing from inspector catalog");
            }
        }
        Ok(())
    }

    #[test]
    fn catalog_names_are_unique() -> Result<()> {
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
}
