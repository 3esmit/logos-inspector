use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicU8, Ordering},
};

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};
use tokio::runtime::Runtime;

use super::commands::{
    operations::{OperationBridgeCommand, RuntimeOperationInterface, operation_bridge_command},
    runtime_methods::{self, RuntimeMethodEntry},
    zone_catalog::{ZoneCatalogCommand, ZoneCatalogCommandInterface, zone_catalog_command},
    zone_l2::{ZoneL2Command, ZoneL2CommandInterface, zone_l2_command},
};
use crate::{
    capabilities::{CapabilityBuildMode, CapabilityRegistry},
    inspection::catalog::DirectZoneCatalogWorker,
    modules::logos_core::{
        LogoscoreCliTransport, ModuleCall, ModuleTransport, SharedModuleTransport,
        dispatch_module_call,
    },
    support::args::Args,
};

pub(crate) const INSPECTOR_MODULE: &str = "logos_inspector";
const SURFACE_OPEN: u8 = 0;
const SURFACE_CLOSING: u8 = 1;
const SURFACE_CLOSED: u8 = 2;

#[derive(Clone)]
pub(crate) struct InspectorCommandSurfaceCloseHandle {
    lifecycle: Arc<SurfaceLifecycle>,
    operations: super::commands::operations::RuntimeOperationCloseHandle,
}

impl InspectorCommandSurfaceCloseHandle {
    pub(crate) fn begin_close(&self) -> Result<()> {
        self.lifecycle.begin_close();
        self.operations.begin_close()
    }
}

struct SurfaceLifecycle {
    phase: AtomicU8,
    shutdown: Mutex<SurfaceShutdownState>,
    shutdown_complete: Condvar,
}

#[derive(Default)]
struct SurfaceShutdownState {
    running: bool,
    result: Option<std::result::Result<(), String>>,
}

impl SurfaceLifecycle {
    fn new() -> Self {
        Self {
            phase: AtomicU8::new(SURFACE_OPEN),
            shutdown: Mutex::new(SurfaceShutdownState::default()),
            shutdown_complete: Condvar::new(),
        }
    }

    fn begin_close(&self) {
        let _previous = self.phase.compare_exchange(
            SURFACE_OPEN,
            SURFACE_CLOSING,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    fn claim_shutdown(&self) -> Result<bool> {
        let mut state = self
            .shutdown
            .lock()
            .map_err(|_| anyhow::anyhow!("inspector shutdown state is unavailable"))?;
        loop {
            if let Some(result) = &state.result {
                return result.clone().map(|()| false).map_err(anyhow::Error::msg);
            }
            if !state.running {
                state.running = true;
                return Ok(true);
            }
            state = self
                .shutdown_complete
                .wait(state)
                .map_err(|_| anyhow::anyhow!("inspector shutdown state is unavailable"))?;
        }
    }

    fn finish_shutdown(&self, result: &Result<()>) -> Result<()> {
        self.phase.store(SURFACE_CLOSED, Ordering::Release);
        let mut state = self
            .shutdown
            .lock()
            .map_err(|_| anyhow::anyhow!("inspector shutdown state is unavailable"))?;
        state.running = false;
        state.result = Some(match result {
            Ok(()) => Ok(()),
            Err(error) => Err(format!("{error:#}")),
        });
        self.shutdown_complete.notify_all();
        Ok(())
    }
}

pub(crate) struct InspectorCommandSurface {
    operations: RuntimeOperationInterface,
    module_transport: SharedModuleTransport,
    zone_catalog: Arc<ZoneCatalogCommandInterface>,
    zone_l2: ZoneL2CommandInterface,
    capability_registry: CapabilityRegistry,
    lifecycle: Arc<SurfaceLifecycle>,
    runtime: Runtime,
}

impl InspectorCommandSurface {
    pub(crate) fn new() -> Result<Self> {
        Self::with_module_transport(LogoscoreCliTransport::default())
    }

    pub(crate) fn with_module_transport(
        module_transport: impl ModuleTransport + 'static,
    ) -> Result<Self> {
        Self::with_shared_module_transport(Arc::new(module_transport))
    }

    pub(crate) fn with_shared_module_transport(
        module_transport: SharedModuleTransport,
    ) -> Result<Self> {
        let runtime = Runtime::new().context("failed to create tokio runtime")?;
        let module_transport_kind = module_transport.kind();
        let catalog_worker = Arc::new(DirectZoneCatalogWorker::for_config_dir()?);
        let zone_catalog = Arc::new(
            ZoneCatalogCommandInterface::with_worker_and_module_transport(
                &runtime,
                catalog_worker,
                Arc::clone(&module_transport),
                module_transport_kind,
            ),
        );
        let zone_l2 = ZoneL2CommandInterface::new(
            zone_catalog.clone(),
            Arc::clone(&module_transport),
            module_transport_kind,
        );
        Ok(Self {
            operations: RuntimeOperationInterface::new(Arc::clone(&module_transport)),
            module_transport,
            zone_catalog,
            zone_l2,
            capability_registry: CapabilityRegistry::default(),
            lifecycle: Arc::new(SurfaceLifecycle::new()),
            runtime,
        })
    }

    pub(crate) fn call_module(&self, module: &str, method: &str, args: Value) -> Result<Value> {
        self.ensure_open()?;
        if module == INSPECTOR_MODULE {
            self.call_inspector(method, args)
        } else {
            self.call_transport(module, method, args)
        }
    }

    pub(crate) fn call_inspector(&self, method: &str, args: Value) -> Result<Value> {
        self.ensure_open()?;
        let result = self.dispatch_inspector(method, args).and_then(|value| {
            value.with_context(|| format!("unknown inspector method `{method}`"))
        });
        match result {
            Ok(value) => {
                self.capability_registry.observe_success(method, &value)?;
                Ok(value)
            }
            Err(error) => {
                self.capability_registry
                    .observe_failure(method, &error.to_string())?;
                Err(error)
            }
        }
    }

    pub(crate) fn ingest_module_event(
        &self,
        module: &str,
        event: &str,
        args: Vec<Value>,
    ) -> Result<Value> {
        if self.lifecycle.phase.load(Ordering::Acquire) == SURFACE_CLOSED {
            bail!("inspector command surface is closed");
        }
        self.operations.ingest_module_event(module, event, args)
    }

    pub(crate) fn close_handle(&self) -> InspectorCommandSurfaceCloseHandle {
        InspectorCommandSurfaceCloseHandle {
            lifecycle: Arc::clone(&self.lifecycle),
            operations: self.operations.close_handle(),
        }
    }

    pub(crate) fn begin_close(&self) -> Result<()> {
        self.close_handle().begin_close()
    }

    pub(crate) fn shutdown(&self) -> Result<()> {
        if !self.lifecycle.claim_shutdown()? {
            return Ok(());
        }
        let begin_result = self.begin_close();
        let operations_result = self.operations.shutdown(&self.runtime);
        let zone_catalog_result = self.runtime.block_on(self.zone_catalog.shutdown());
        let result = begin_result.and(operations_result).and(zone_catalog_result);
        self.lifecycle.finish_shutdown(&result)?;
        result
    }

    pub(crate) fn allows_host_synchronous_call(method: &str) -> bool {
        match inspector_command(method) {
            Some(InspectorCommand::Runtime(entry)) => entry.allows_host_synchronous_call(),
            Some(InspectorCommand::CapabilityRegistry) => true,
            Some(
                InspectorCommand::Operation(_)
                | InspectorCommand::ZoneCatalog(_)
                | InspectorCommand::ZoneL2(_)
                | InspectorCommand::CallModule,
            )
            | None => false,
        }
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
            InspectorCommand::Runtime(method) => runtime_methods::handle(
                &self.runtime,
                method,
                args,
                Arc::clone(&self.module_transport),
            )
            .map(Some),
            InspectorCommand::ZoneCatalog(command) => self
                .zone_catalog
                .bridge_call(&self.runtime, command, &args)
                .map(Some),
            InspectorCommand::ZoneL2(command) => self
                .zone_l2
                .bridge_call(&self.runtime, command, &args)
                .map(Some),
            InspectorCommand::CapabilityRegistry => {
                let args = Args::new(args)?;
                let build_mode = CapabilityBuildMode::from_prefers_basecamp(args.optional_bool(0));
                let runtime_inputs = args
                    .value(1)
                    .filter(|value| value.is_object())
                    .context("capability runtime inputs are required")?;
                serde_json::to_value(
                    self.capability_registry
                        .report(build_mode, Some(runtime_inputs))?,
                )
                .context("failed to serialize capability registry report")
                .map(Some)
            }
            InspectorCommand::CallModule => {
                let args = Args::new(args)?;
                let module = args.string(0, "module name")?;
                let method = args.string(1, "method name")?;
                let call_args = args.value(2).cloned().unwrap_or_else(|| json!([]));
                self.call_transport(module, method, call_args).map(Some)
            }
        }
    }

    fn call_transport(&self, module: &str, method: &str, args: Value) -> Result<Value> {
        let args = Args::new(args)?.iter().cloned().collect::<Vec<_>>();
        let call = ModuleCall::new(self.module_transport.kind(), module, method, args)?;
        self.runtime
            .block_on(dispatch_module_call(self.module_transport.as_ref(), call))
            .map(|reply| reply.into_value())
    }

    fn ensure_open(&self) -> Result<()> {
        match self.lifecycle.phase.load(Ordering::Acquire) {
            SURFACE_OPEN => Ok(()),
            SURFACE_CLOSING => bail!("inspector command surface is shutting down"),
            _ => bail!("inspector command surface is closed"),
        }
    }

    #[cfg(test)]
    pub(crate) fn operations_for_test(&self) -> &RuntimeOperationInterface {
        &self.operations
    }
}

impl Drop for InspectorCommandSurface {
    fn drop(&mut self) {
        let _result = self.shutdown();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InspectorCommand {
    Operation(OperationBridgeCommand),
    Runtime(&'static RuntimeMethodEntry),
    ZoneCatalog(ZoneCatalogCommand),
    ZoneL2(ZoneL2Command),
    CapabilityRegistry,
    CallModule,
}

fn inspector_command(method: &str) -> Option<InspectorCommand> {
    if method == "capabilityRegistryReport" {
        return Some(InspectorCommand::CapabilityRegistry);
    }
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
    use std::{
        collections::HashSet,
        sync::{Arc, Barrier},
        thread,
    };

    use anyhow::{Context as _, Result, bail};
    use serde_json::json;

    use super::*;
    use crate::inspector::commands::{operations, runtime_methods, zone_catalog, zone_l2};

    #[derive(Debug, Default)]
    struct FakeModuleTransport;

    impl ModuleTransport for FakeModuleTransport {
        fn kind(&self) -> crate::modules::logos_core::ModuleTransportKind {
            crate::modules::logos_core::ModuleTransportKind::LogoscoreCli
        }

        fn call(
            &self,
            call: crate::modules::logos_core::ModuleCall,
        ) -> crate::modules::logos_core::ModuleCallFuture<'_> {
            Box::pin(async move {
                Ok(crate::modules::logos_core::ModuleCallReply::new(
                    crate::modules::logos_core::ModuleTransportKind::LogoscoreCli,
                    json!({
                        "module": call.module(),
                        "method": call.method(),
                        "args": call.args(),
                    }),
                ))
            })
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
            .chain(["capabilityRegistryReport", "callModule"])
        {
            if !names.insert(method) {
                bail!("duplicate inspector method `{method}`");
            }
        }
        Ok(())
    }

    #[test]
    fn surface_dispatches_call_module_special() -> Result<()> {
        let surface = InspectorCommandSurface::with_module_transport(FakeModuleTransport)
            .context("surface")?;

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
        let surface = InspectorCommandSurface::with_module_transport(FakeModuleTransport)
            .context("surface")?;

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
        let surface = InspectorCommandSurface::with_module_transport(FakeModuleTransport)
            .context("surface")?;

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
    fn closing_rejects_calls_but_drains_queued_module_events() -> Result<()> {
        let surface = InspectorCommandSurface::with_module_transport(FakeModuleTransport)
            .context("surface")?;
        let close_handle = surface.close_handle();

        close_handle.begin_close()?;

        let Err(call_error) = surface.call_inspector("sourcePolicy", json!([])) else {
            bail!("closing surface accepted a new call");
        };
        if !call_error.to_string().contains("shutting down") {
            bail!("unexpected closing error: {call_error:#}");
        }
        let ingress = surface.ingest_module_event(
            "storage_module",
            "storageUploadDone",
            vec![json!({"sessionId": "unmatched"})],
        )?;
        if ingress.get("disposition").and_then(Value::as_str) != Some("unknown") {
            bail!("closing surface did not drain module event: {ingress}");
        }

        surface.shutdown()?;
        let Err(ingress_error) =
            surface.ingest_module_event("storage_module", "storageUploadDone", vec![])
        else {
            bail!("closed surface accepted a module event");
        };
        if !ingress_error.to_string().contains("is closed") {
            bail!("unexpected closed ingress error: {ingress_error:#}");
        }
        surface.shutdown()?;
        Ok(())
    }

    #[test]
    fn concurrent_shutdown_callers_wait_for_one_surface_drain() -> Result<()> {
        let surface = Arc::new(
            InspectorCommandSurface::with_module_transport(FakeModuleTransport)
                .context("surface")?,
        );
        let barrier = Arc::new(Barrier::new(3));
        let mut callers = Vec::new();
        for _ in 0..2 {
            let surface = Arc::clone(&surface);
            let barrier = Arc::clone(&barrier);
            callers.push(thread::spawn(move || {
                barrier.wait();
                surface.shutdown()
            }));
        }
        barrier.wait();
        for caller in callers {
            caller
                .join()
                .map_err(|_| anyhow::anyhow!("shutdown caller panicked"))??;
        }
        let Err(error) = surface.call_inspector("sourcePolicy", json!([])) else {
            bail!("drained surface accepted a call");
        };
        if !error.to_string().contains("is closed") {
            bail!("unexpected post-shutdown error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn zone_l2_reads_do_not_enter_runtime_operation_history() -> Result<()> {
        let surface = InspectorCommandSurface::with_module_transport(FakeModuleTransport)
            .context("surface")?;
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

    #[test]
    fn host_synchronous_policy_excludes_async_required_commands() {
        assert!(InspectorCommandSurface::allows_host_synchronous_call(
            "sourcePolicy"
        ));
        assert!(InspectorCommandSurface::allows_host_synchronous_call(
            "decodeAccount"
        ));
        assert!(InspectorCommandSurface::allows_host_synchronous_call(
            "loadIdlState"
        ));
        assert!(InspectorCommandSurface::allows_host_synchronous_call(
            "capabilityRegistryReport"
        ));
        assert!(!InspectorCommandSurface::allows_host_synchronous_call(
            "rawRpc"
        ));
        assert!(!InspectorCommandSurface::allows_host_synchronous_call(
            "runtimeOperationStatus"
        ));
        assert!(!InspectorCommandSurface::allows_host_synchronous_call(
            "modules"
        ));
        assert!(!InspectorCommandSurface::allows_host_synchronous_call(
            "zoneCatalogStatus"
        ));
        assert!(!InspectorCommandSurface::allows_host_synchronous_call(
            "zoneL2Programs"
        ));
        assert!(!InspectorCommandSurface::allows_host_synchronous_call(
            "callModule"
        ));
        assert!(!InspectorCommandSurface::allows_host_synchronous_call(
            "missingMethod"
        ));
    }
}
