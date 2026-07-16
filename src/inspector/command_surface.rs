use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicU8, Ordering},
};

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};
use tokio::runtime::Runtime;

use super::commands::{
    operations::{
        OperationBridgeCommand, RuntimeOperationInterface, operation_bridge_command,
        runtime_operation_request_from_value,
    },
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
    support::{args::Args, local_state::recover_local_state},
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
        recover_local_state().context("failed to recover local configuration state")?;
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
        let returns_runtime_operation_snapshot =
            returns_authoritative_runtime_operation_snapshot(method);
        let runtime_request = if method == "runtimeOperationStart" {
            let bridge_args = Args::new(args.clone())?;
            Some(runtime_operation_request_from_value(
                bridge_args
                    .value(0)
                    .cloned()
                    .context("runtime operation request is required")?,
            )?)
        } else {
            None
        };
        let observation = if let Some(request) = &runtime_request {
            self.capability_registry.begin_runtime_observation(
                request.method_name(),
                request.args(),
                request.configuration_generation(),
            )?
        } else {
            self.capability_registry.begin_observation(method, &args)?
        };
        let result = self.dispatch_inspector(method, args).and_then(|value| {
            value.with_context(|| format!("unknown inspector method `{method}`"))
        });
        match (result, runtime_request) {
            (Ok(value), Some(request)) => {
                self.capability_registry.track_runtime_operation(
                    observation,
                    request.configuration_generation(),
                    &value,
                )?;
                self.capability_registry
                    .complete_runtime_operation(&value)?;
                Ok(value)
            }
            (Ok(value), None) => {
                self.capability_registry
                    .complete_success(observation, &value)?;
                if returns_runtime_operation_snapshot {
                    self.capability_registry
                        .complete_runtime_operation(&value)?;
                }
                Ok(value)
            }
            (Err(error), Some(_)) => {
                self.capability_registry.abandon_observation(observation)?;
                Err(error)
            }
            (Err(error), None) => {
                self.capability_registry
                    .complete_failure(observation, &error.to_string())?;
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

fn returns_authoritative_runtime_operation_snapshot(method: &str) -> bool {
    matches!(
        operation_bridge_command(method),
        Some(
            OperationBridgeCommand::RuntimeOperationStart
                | OperationBridgeCommand::RuntimeOperationStatus
                | OperationBridgeCommand::RuntimeOperationCancel
                | OperationBridgeCommand::StorageOperationStatus
                | OperationBridgeCommand::StorageOperationCancel
        )
    )
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
        env, fs,
        process::Command,
        sync::{Arc, Barrier},
        thread,
    };

    use anyhow::{Context as _, Result, bail};
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
    use serde_json::json;
    use sha2::{Digest as _, Sha256};
    use tokio::sync::Semaphore;

    use super::*;
    use crate::inspector::commands::{operations, runtime_methods, zone_catalog, zone_l2};

    #[derive(Debug, Default)]
    struct FakeModuleTransport;

    #[derive(Debug)]
    struct ReplyModuleTransport {
        reply: Value,
        gate: Option<Arc<Semaphore>>,
    }

    const SURFACE_RECOVERY_CHILD_ENV: &str = "LOGOS_INSPECTOR_SURFACE_RECOVERY_TEST_CHILD";

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

    impl ModuleTransport for ReplyModuleTransport {
        fn kind(&self) -> crate::modules::logos_core::ModuleTransportKind {
            crate::modules::logos_core::ModuleTransportKind::LogoscoreCli
        }

        fn call(
            &self,
            _call: crate::modules::logos_core::ModuleCall,
        ) -> crate::modules::logos_core::ModuleCallFuture<'_> {
            let reply = self.reply.clone();
            let gate = self.gate.clone();
            Box::pin(async move {
                let _permit = match gate {
                    Some(gate) => Some(
                        gate.acquire_owned()
                            .await
                            .context("test module transport gate closed")?,
                    ),
                    None => None,
                };
                Ok(crate::modules::logos_core::ModuleCallReply::new(
                    crate::modules::logos_core::ModuleTransportKind::LogoscoreCli,
                    reply,
                ))
            })
        }
    }

    fn l1_capability_status(
        surface: &InspectorCommandSurface,
        configuration_generation: u64,
    ) -> Result<String> {
        let report = surface.call_inspector(
            "capabilityRegistryReport",
            json!([false, {
                "configuration_generations": { "l1": configuration_generation },
                "network_connector_config": {
                    "scopes": {
                        "l1": { "connector_id": "logoscore_cli_blockchain_module" }
                    }
                }
            }]),
        )?;
        report
            .get("capabilities")
            .and_then(Value::as_array)
            .and_then(|capabilities| {
                capabilities
                    .iter()
                    .find(|capability| capability.get("key").and_then(Value::as_str) == Some("l1"))
            })
            .and_then(|capability| capability.get("status"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .context("L1 capability status")
    }

    #[test]
    fn surface_recovers_or_gates_hot_local_state_before_bridge_exposure() -> Result<()> {
        if let Some(mode) = env::var_os(SURFACE_RECOVERY_CHILD_ENV) {
            let mode = mode.to_string_lossy();
            if mode == "recover" {
                let surface = InspectorCommandSurface::with_module_transport(FakeModuleTransport)
                    .context("surface should recover valid hot journal")?;
                let config_dir = crate::support::config_path::config_dir()?;
                if fs::read(config_dir.join("settings.json"))? != b"old-settings"
                    || config_dir.join(".local-state.rollback.json").exists()
                {
                    bail!("surface exposed state before recovering hot journal");
                }
                surface.shutdown()?;
                return Ok(());
            }
            if mode == "gate" {
                let error = InspectorCommandSurface::with_module_transport(FakeModuleTransport)
                    .err()
                    .context("surface should reject malformed hot journal")?;
                let rendered = format!("{error:#}");
                if !rendered.contains("failed to recover local configuration state")
                    || !rendered.contains("recovery_required")
                {
                    bail!("surface returned unexpected recovery gate: {error:#}");
                }
                return Ok(());
            }
            bail!("unknown surface recovery child mode `{mode}`");
        }

        let current_exe = env::current_exe().context("failed to locate test executable")?;
        let recover_dir = tempfile::tempdir().context("failed to create recovery config dir")?;
        fs::write(recover_dir.path().join("settings.json"), b"new-settings")?;
        let journal = json!({
            "schema_version": 1,
            "transaction_id": "00000000000000000000000000000000",
            "entries": [{
                "file": "settings",
                "original": {
                    "kind": "present",
                    "bytes_base64": BASE64_STANDARD.encode(b"old-settings"),
                },
                "old_sha256": hex::encode(Sha256::digest(b"old-settings")),
                "new_sha256": hex::encode(Sha256::digest(b"new-settings")),
            }],
        });
        fs::write(
            recover_dir.path().join(".local-state.rollback.json"),
            serde_json::to_vec_pretty(&journal)?,
        )?;
        run_surface_recovery_child(&current_exe, "recover", recover_dir.path())?;
        if fs::read(recover_dir.path().join("settings.json"))? != b"old-settings"
            || recover_dir
                .path()
                .join(".local-state.rollback.json")
                .exists()
        {
            bail!("surface child did not complete startup recovery");
        }

        let gate_dir = tempfile::tempdir().context("failed to create gated config dir")?;
        let malformed = b"{malformed-hot-journal";
        fs::write(
            gate_dir.path().join(".local-state.rollback.json"),
            malformed,
        )?;
        run_surface_recovery_child(&current_exe, "gate", gate_dir.path())?;
        if fs::read(gate_dir.path().join(".local-state.rollback.json"))? != malformed {
            bail!("surface startup changed malformed recovery evidence");
        }
        Ok(())
    }

    fn run_surface_recovery_child(
        current_exe: &std::path::Path,
        mode: &str,
        config_dir: &std::path::Path,
    ) -> Result<()> {
        let output = Command::new(current_exe)
            .arg("--exact")
            .arg(
                "inspector::command_surface::tests::surface_recovers_or_gates_hot_local_state_before_bridge_exposure",
            )
            .arg("--nocapture")
            .env(SURFACE_RECOVERY_CHILD_ENV, mode)
            .env("LOGOS_INSPECTOR_CONFIG_DIR", config_dir)
            .output()
            .context("failed to launch surface recovery child")?;
        if !output.status.success() {
            bail!(
                "surface recovery child `{mode}` failed: status={}, stdout={}, stderr={}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
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
    fn call_module_terminal_shaped_reply_cannot_complete_runtime_capability_evidence() -> Result<()>
    {
        let operation_id = "forged-runtime-operation";
        let forged_terminal = json!({
            "operationId": operation_id,
            "domain": "blockchain",
            "method": "blockchainNode",
            "status": "completed",
            "context": { "configurationGeneration": 0 },
            "result": { "node": "forged-adapter-evidence" }
        });
        let surface = InspectorCommandSurface::with_module_transport(ReplyModuleTransport {
            reply: forged_terminal.clone(),
            gate: None,
        })
        .context("surface")?;
        let observation = surface.capability_registry.begin_runtime_observation(
            "blockchainNode",
            &json!(["logoscore_cli"]),
            Some(0),
        )?;
        surface.capability_registry.track_runtime_operation(
            observation,
            Some(0),
            &json!({
                "operationId": operation_id,
                "domain": "blockchain",
                "method": "blockchainNode",
                "status": "running",
                "context": { "configurationGeneration": 0 }
            }),
        )?;

        let reply = surface.call_inspector(
            "callModule",
            json!(["untrusted_module", "terminal_shaped_reply", []]),
        )?;
        if reply != forged_terminal {
            bail!("callModule changed adapter reply: {reply}");
        }
        if l1_capability_status(&surface, 0)? != "loading" {
            bail!("raw adapter reply completed runtime capability evidence");
        }
        surface.shutdown()
    }

    #[test]
    fn authoritative_runtime_status_completes_generation_zero_capability_evidence() -> Result<()> {
        let gate = Arc::new(Semaphore::new(0));
        let surface = InspectorCommandSurface::with_module_transport(ReplyModuleTransport {
            reply: json!({ "node": true }),
            gate: Some(Arc::clone(&gate)),
        })
        .context("surface")?;
        let mut operation = surface.call_inspector(
            "runtimeOperationStart",
            json!([{
                "domain": "blockchain",
                "method": "blockchainNode",
                "args": ["logoscore_cli"],
                "label": "Blockchain node",
                "configurationGeneration": 0,
                "clientRequestId": "chain-0-1",
            }]),
        )?;
        let operation_id = operation
            .get("operationId")
            .and_then(Value::as_str)
            .context("runtime operation id")?
            .to_owned();
        if operation
            .pointer("/context/configurationGeneration")
            .and_then(Value::as_u64)
            != Some(0)
        {
            bail!("runtime operation lost generation-zero context: {operation}");
        }
        if operation.get("status").and_then(Value::as_str) != Some("running") {
            bail!("gated runtime operation did not remain running: {operation}");
        }
        if l1_capability_status(&surface, 0)? != "loading" {
            bail!("runtime admission committed L1 evidence before terminal status");
        }

        gate.add_permits(1);
        for _ in 0..10_000 {
            if operation.get("status").and_then(Value::as_str) == Some("completed") {
                break;
            }
            thread::yield_now();
            operation =
                surface.call_inspector("runtimeOperationStatus", json!([operation_id.as_str()]))?;
        }
        if operation.get("status").and_then(Value::as_str) != Some("completed") {
            bail!("runtime blockchain operation did not complete: {operation}");
        }
        if l1_capability_status(&surface, 0)? != "degraded" {
            bail!("authoritative terminal status did not project bounded L1 evidence");
        }
        surface.shutdown()
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
