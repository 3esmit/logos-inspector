use std::sync::Arc;

use anyhow::{Context as _, Result};
use serde_json::{Value, json};
use tokio::runtime::Runtime;

use super::commands::{
    operations::{OperationBridgeCommand, RuntimeOperationInterface, operation_bridge_command},
    runtime_methods::{self, RuntimeMethodEntry},
    zone_catalog::{ZoneCatalogCommand, ZoneCatalogCommandInterface, zone_catalog_command},
    zone_l2::{ZoneL2Command, ZoneL2CommandInterface, zone_l2_command},
};
use super::value::to_value;
use crate::{
    inspection::catalog::DirectZoneCatalogWorker, modules::logos_core, support::args::Args,
};

pub(crate) const INSPECTOR_MODULE: &str = "logos_inspector";

pub(crate) trait CoreModuleCaller {
    fn call(&self, module: &str, method: &str, args: Value) -> Result<Value>;
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct LogosCoreModuleCaller;

impl CoreModuleCaller for LogosCoreModuleCaller {
    fn call(&self, module: &str, method: &str, args: Value) -> Result<Value> {
        let args = Args::new(args)?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| value.to_string())
            })
            .collect::<Vec<_>>();
        to_value(logos_core::call(module, method, &args)?)
    }
}

pub(crate) struct InspectorCommandSurface<C = LogosCoreModuleCaller> {
    operations: RuntimeOperationInterface,
    core_modules: C,
    zone_catalog: Arc<ZoneCatalogCommandInterface>,
    zone_l2: ZoneL2CommandInterface,
    runtime: Runtime,
}

impl InspectorCommandSurface<LogosCoreModuleCaller> {
    pub(crate) fn new() -> Result<Self> {
        Self::with_core_modules(LogosCoreModuleCaller)
    }
}

impl<C> InspectorCommandSurface<C>
where
    C: CoreModuleCaller,
{
    pub(crate) fn with_core_modules(core_modules: C) -> Result<Self> {
        let runtime = Runtime::new().context("failed to create tokio runtime")?;
        let catalog_worker = Arc::new(DirectZoneCatalogWorker::for_config_dir()?);
        let zone_catalog = Arc::new(ZoneCatalogCommandInterface::with_worker(
            &runtime,
            catalog_worker,
        ));
        let zone_l2 = ZoneL2CommandInterface::new(zone_catalog.clone());
        Ok(Self {
            operations: RuntimeOperationInterface::default(),
            core_modules,
            zone_catalog,
            zone_l2,
            runtime,
        })
    }

    pub(crate) fn call_module(&self, module: &str, method: &str, args: Value) -> Result<Value> {
        if module == INSPECTOR_MODULE {
            self.call_inspector(method, args)
        } else {
            self.core_modules.call(module, method, args)
        }
    }

    pub(crate) fn call_inspector(&self, method: &str, args: Value) -> Result<Value> {
        self.dispatch_inspector(method, args)?
            .with_context(|| format!("unknown inspector method `{method}`"))
    }

    fn dispatch_inspector(&self, method: &str, args: Value) -> Result<Option<Value>> {
        let Some(command) = inspector_command(method) else {
            return Ok(None);
        };
        match command {
            InspectorCommand::Operation(command) => self
                .operations
                .bridge_call(&self.runtime, command, &args)
                .map(Some),
            InspectorCommand::Runtime(method) => {
                runtime_methods::handle(&self.runtime, method, args).map(Some)
            }
            InspectorCommand::ZoneCatalog(command) => self
                .zone_catalog
                .bridge_call(&self.runtime, command, &args)
                .map(Some),
            InspectorCommand::ZoneL2(command) => self
                .zone_l2
                .bridge_call(&self.runtime, command, &args)
                .map(Some),
            InspectorCommand::CallModule => {
                let args = Args::new(args)?;
                let module = args.string(0, "module name")?;
                let method = args.string(1, "method name")?;
                let call_args = args.value(2).cloned().unwrap_or_else(|| json!([]));
                self.core_modules.call(module, method, call_args).map(Some)
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn operations_for_test(&self) -> &RuntimeOperationInterface {
        &self.operations
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InspectorCommand {
    Operation(OperationBridgeCommand),
    Runtime(&'static RuntimeMethodEntry),
    ZoneCatalog(ZoneCatalogCommand),
    ZoneL2(ZoneL2Command),
    CallModule,
}

fn inspector_command(method: &str) -> Option<InspectorCommand> {
    if let Some(command) = operation_bridge_command(method) {
        return Some(InspectorCommand::Operation(command));
    }
    if let Some(method) = runtime_methods::lookup(method) {
        return Some(InspectorCommand::Runtime(method));
    }
    if let Some(command) = zone_catalog_command(method) {
        return Some(InspectorCommand::ZoneCatalog(command));
    }
    if let Some(command) = zone_l2_command(method) {
        return Some(InspectorCommand::ZoneL2(command));
    }
    match method {
        "callModule" => Some(InspectorCommand::CallModule),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use anyhow::{Context as _, Result, bail};
    use serde_json::json;

    use super::*;
    use crate::inspector::commands::{operations, runtime_methods, zone_catalog, zone_l2};

    #[derive(Debug, Default)]
    struct FakeCoreModules;

    impl CoreModuleCaller for FakeCoreModules {
        fn call(&self, module: &str, method: &str, args: Value) -> Result<Value> {
            Ok(json!({
                "module": module,
                "method": method,
                "args": args,
            }))
        }
    }

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
        for method in runtime_methods::runtime_method_entries() {
            if inspector_command(method.name()) != Some(InspectorCommand::Runtime(method)) {
                let name = method.name();
                bail!("runtime method `{name}` missing from inspector surface");
            }
        }
        Ok(())
    }

    #[test]
    fn surface_owns_zone_catalog_names() -> Result<()> {
        for method in zone_catalog::zone_catalog_command_names() {
            if !matches!(
                inspector_command(method),
                Some(InspectorCommand::ZoneCatalog(_))
            ) {
                bail!("Zone Catalog method `{method}` missing from inspector surface");
            }
        }
        Ok(())
    }

    #[test]
    fn surface_owns_zone_l2_names() -> Result<()> {
        for method in zone_l2::zone_l2_command_names() {
            if !matches!(inspector_command(method), Some(InspectorCommand::ZoneL2(_))) {
                bail!("Zone L2 method `{method}` missing from inspector surface");
            }
        }
        Ok(())
    }

    #[test]
    fn surface_names_are_unique() -> Result<()> {
        let mut names = HashSet::new();
        for method in operations::operation_bridge_command_names()
            .chain(runtime_methods::runtime_method_names())
            .chain(zone_catalog::zone_catalog_command_names())
            .chain(zone_l2::zone_l2_command_names())
            .chain(["callModule"])
        {
            if !names.insert(method) {
                bail!("duplicate inspector method `{method}`");
            }
        }
        Ok(())
    }

    #[test]
    fn surface_dispatches_call_module_special() -> Result<()> {
        let surface =
            InspectorCommandSurface::with_core_modules(FakeCoreModules).context("surface")?;

        let value =
            surface.call_inspector("callModule", json!(["module_a", "method_b", ["arg"]]))?;

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
    fn surface_dispatches_non_inspector_modules_to_core_adapter() -> Result<()> {
        let surface =
            InspectorCommandSurface::with_core_modules(FakeCoreModules).context("surface")?;

        let value = surface.call_module("other_module", "method_c", json!(["arg"]))?;

        if value
            != json!({
                "module": "other_module",
                "method": "method_c",
                "args": ["arg"],
            })
        {
            bail!("unexpected module dispatch value: {value}");
        }
        Ok(())
    }

    #[test]
    fn surface_reports_unknown_methods() -> Result<()> {
        let surface =
            InspectorCommandSurface::with_core_modules(FakeCoreModules).context("surface")?;

        let result = surface.call_inspector("missingMethod", json!([]));
        let Err(error) = result else {
            bail!("unknown method should fail");
        };
        if !error
            .to_string()
            .contains("unknown inspector method `missingMethod`")
        {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn zone_l2_reads_do_not_enter_runtime_operation_history() -> Result<()> {
        let surface =
            InspectorCommandSurface::with_core_modules(FakeCoreModules).context("surface")?;
        if surface.operations_for_test().len()? != 0 {
            bail!("fresh operation history is not empty");
        }

        let result = surface.call_inspector("zoneL2Programs", json!([]));
        if result.is_ok() {
            bail!("malformed Zone L2 request unexpectedly succeeded");
        }
        if surface.operations_for_test().len()? != 0 {
            bail!("Zone L2 read entered runtime operation history");
        }
        Ok(())
    }
}
