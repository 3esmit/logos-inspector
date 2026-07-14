use anyhow::{Context as _, Result};
use serde_json::Value;
#[cfg(test)]
use serde_json::json;

use super::command_surface::{INSPECTOR_MODULE, InspectorCommandSurface};
use crate::modules::logos_core::{
    ModuleTransport, SharedModuleTransport, UnavailableModuleTransport,
};
#[cfg(test)]
use crate::source_routing::{
    self, CoreEndpointMode, DEFAULT_DELIVERY_REST_ENDPOINT, DEFAULT_STORAGE_REST_ENDPOINT,
    messaging_layer,
};
use crate::support::bridge_envelope::{bridge_error_response_json, bridge_response_json};
#[cfg(test)]
const BLOCKCHAIN_MODULE: &str = source_routing::BLOCKCHAIN_MODULE;

pub struct InspectorBridge {
    surface: InspectorCommandSurface,
}

impl InspectorBridge {
    pub fn new() -> Result<Self> {
        Self::standalone()
    }

    pub fn standalone() -> Result<Self> {
        Ok(Self {
            surface: InspectorCommandSurface::new()?,
        })
    }

    /// Builds a bridge around one authoritative module transport.
    ///
    /// The transport future must complete without re-entering the thread that
    /// invokes this synchronous bridge. Host-backed Runtime Operation aliases
    /// are rejected before dispatch and require `runtimeOperationStart`.
    pub fn with_module_transport(module_transport: impl ModuleTransport + 'static) -> Result<Self> {
        Ok(Self {
            surface: InspectorCommandSurface::with_module_transport(module_transport)?,
        })
    }

    /// Builds a bridge around one shared authoritative module transport.
    ///
    /// The transport future must complete without re-entering the thread that
    /// invokes this synchronous bridge. Host-backed Runtime Operation aliases
    /// are rejected before dispatch and require `runtimeOperationStart`.
    pub fn with_shared_module_transport(module_transport: SharedModuleTransport) -> Result<Self> {
        Ok(Self {
            surface: InspectorCommandSurface::with_shared_module_transport(module_transport)?,
        })
    }

    pub fn basecamp_unavailable() -> Result<Self> {
        Self::with_module_transport(UnavailableModuleTransport::basecamp_protocol_gate())
    }

    pub fn call_module_json(&self, module: &str, method: &str, args_json: &str) -> String {
        let result = serde_json::from_str::<Value>(args_json)
            .context("failed to parse bridge args")
            .and_then(|args| self.call_module_value(module, method, args));
        bridge_response_json(result)
    }

    pub fn call_inspector_json(&self, method: &str, args_json: &str) -> String {
        self.call_module_json(INSPECTOR_MODULE, method, args_json)
    }

    /// Reports whether a host-backed bridge may run this inspector method
    /// synchronously without entering Tokio or its module transport.
    #[must_use]
    pub fn allows_host_synchronous_call(method: &str) -> bool {
        InspectorCommandSurface::allows_host_synchronous_call(method)
    }

    /// Ingests one typed host module event without routing it back through the
    /// host module transport.
    pub fn ingest_module_event(
        &self,
        module: &str,
        event: &str,
        args: Vec<Value>,
    ) -> Result<Value> {
        self.surface.ingest_module_event(module, event, args)
    }

    pub fn error_json(error: impl Into<String>) -> String {
        bridge_error_response_json(error)
    }

    fn call_module_value(&self, module: &str, method: &str, args: Value) -> Result<Value> {
        self.surface.call_module(module, method, args)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        io::{Read as _, Write as _},
        net::TcpListener,
        process::Command,
        sync::{Arc, Mutex},
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    use anyhow::bail;

    use super::*;
    use crate::inspector::commands::operations::{
        RuntimeOperationRequest, runtime_operation_request_from_value,
    };
    use crate::support::args::Args;

    const BACKUP_ALIAS_CHILD_ENV: &str = "LOGOS_INSPECTOR_BACKUP_ALIAS_TEST_CHILD";
    const BACKUP_ALIAS_ENDPOINT_ENV: &str = "LOGOS_INSPECTOR_BACKUP_ALIAS_TEST_ENDPOINT";
    const BACKUP_ALIAS_TEST_NAME: &str =
        "inspector::bridge::tests::standalone_legacy_backup_alias_downloads_without_applying";

    #[derive(Clone)]
    struct RecordingModuleTransport {
        calls: Arc<Mutex<Vec<crate::modules::logos_core::ModuleCall>>>,
        kind: crate::modules::logos_core::ModuleTransportKind,
    }

    impl crate::modules::logos_core::ModuleTransport for RecordingModuleTransport {
        fn kind(&self) -> crate::modules::logos_core::ModuleTransportKind {
            self.kind
        }

        fn call(
            &self,
            call: crate::modules::logos_core::ModuleCall,
        ) -> crate::modules::logos_core::ModuleCallFuture<'_> {
            if let Ok(mut calls) = self.calls.lock() {
                calls.push(call.clone());
            }
            let kind = self.kind;
            let value = match call.method() {
                "exists" => json!(true),
                _ => json!({ "handled": call.method() }),
            };
            Box::pin(async move {
                Ok(crate::modules::logos_core::ModuleCallReply::new(
                    kind, value,
                ))
            })
        }
    }

    fn storage_download_request(cid: &str, path: &str) -> Result<RuntimeOperationRequest> {
        runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageDownloadToUrl",
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://127.0.0.1:8080/api/storage/v1" }
            },
            "payload": { "cid": cid, "path": path, "local_only": false },
            "mutating_enabled": true,
            "label": "Storage download"
        }))
    }

    #[test]
    fn direct_runtime_method_calls_share_module_transport() -> Result<()> {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let bridge = InspectorBridge::with_module_transport(RecordingModuleTransport {
            calls: Arc::clone(&calls),
            kind: crate::modules::logos_core::ModuleTransportKind::Module,
        })?;

        let direct = bridge.call_module_value(
            BLOCKCHAIN_MODULE,
            "nodeInfo",
            json!([{ "includePeers": true }]),
        )?;
        let exists = bridge.call_module_value(
            INSPECTOR_MODULE,
            "storageExists",
            json!([{
                "adapter": { "source_mode": "module", "inputs": {} },
                "payload": { "cid": "cid-1" },
                "mutating_enabled": false
            }]),
        )?;
        anyhow::ensure!(direct == json!({ "handled": "nodeInfo" }));
        anyhow::ensure!(exists == json!(true));
        let calls = calls
            .lock()
            .map_err(|error| anyhow::anyhow!("recorded call lock failed: {error}"))?;
        let [direct_call, exists_call] = calls.as_slice() else {
            bail!("expected two calls through shared seam: {calls:?}");
        };
        anyhow::ensure!(
            direct_call.module() == BLOCKCHAIN_MODULE
                && direct_call.method() == "nodeInfo"
                && direct_call.args() == [json!({ "includePeers": true })]
        );
        anyhow::ensure!(
            exists_call.module() == "storage_module"
                && exists_call.method() == "exists"
                && exists_call.args() == [json!("cid-1")]
        );
        Ok(())
    }

    #[test]
    fn host_backed_direct_operation_requires_runtime_operation_start() -> Result<()> {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let bridge = InspectorBridge::with_module_transport(RecordingModuleTransport {
            calls: Arc::clone(&calls),
            kind: crate::modules::logos_core::ModuleTransportKind::Module,
        })?;

        let cases = [
            ("storageFetch", json!({ "cid": "cid-2" })),
            (
                "storageUploadPayload",
                json!({
                    "filename": "shared-idl.json",
                    "payload": { "kind": "shared-idl" },
                    "block_size": 65536
                }),
            ),
            (
                "storageUploadBackupCatalogEntry",
                json!({ "backup_catalog_id": "backup-2", "block_size": 65536 }),
            ),
            (
                "storageDownloadBackupCatalogEntry",
                json!({ "cid": "cid-backup", "local_only": false }),
            ),
        ];
        for (method, payload) in cases {
            let result = bridge.call_module_value(
                INSPECTOR_MODULE,
                method,
                json!([{
                    "adapter": { "source_mode": "module", "inputs": {} },
                    "payload": payload,
                    "mutating_enabled": true
                }]),
            );

            let Err(error) = result else {
                bail!("host-backed direct operation should fail before dispatch");
            };
            anyhow::ensure!(
                error.to_string().contains(&format!(
                    "host-backed operation `{method}` requires `runtimeOperationStart`"
                )),
                "unexpected direct host operation error: {error:#}"
            );
        }
        let legacy = bridge.call_module_value(
            INSPECTOR_MODULE,
            "storageRestoreSettings",
            json!([{
                "adapter": { "source_mode": "module", "inputs": {} },
                "payload": { "cid": "cid-legacy", "local_only": false },
                "mutating_enabled": false
            }]),
        );
        let Err(error) = legacy else {
            bail!("legacy host-backed backup restore should fail before dispatch");
        };
        anyhow::ensure!(
            error.to_string().contains(
                "host-backed operation `storageDownloadBackupCatalogEntry` requires `runtimeOperationStart`"
            ),
            "legacy alias did not resolve through canonical host gate: {error:#}"
        );
        let calls = calls
            .lock()
            .map_err(|error| anyhow::anyhow!("recorded call lock failed: {error}"))?;
        anyhow::ensure!(calls.is_empty(), "direct host operation reached transport");
        Ok(())
    }

    #[test]
    fn standalone_legacy_backup_alias_downloads_without_applying() -> Result<()> {
        if env::var_os(BACKUP_ALIAS_CHILD_ENV).is_some() {
            let endpoint = env::var(BACKUP_ALIAS_ENDPOINT_ENV)
                .context("backup alias child endpoint is missing")?;
            let bridge = InspectorBridge::standalone()?;
            let result = bridge.call_module_value(
                INSPECTOR_MODULE,
                "storageRestoreSettings",
                json!([{
                    "adapter": {
                        "source_mode": "rest",
                        "inputs": { "rest_endpoint": endpoint }
                    },
                    "payload": { "cid": "cid-legacy-rest", "local_only": false },
                    "mutating_enabled": false
                }]),
            )?;

            anyhow::ensure!(
                result.get("downloaded") == Some(&json!(true))
                    && result.get("restored") == Some(&json!(false))
                    && result.get("cid") == Some(&json!("cid-legacy-rest"))
                    && result.pointer("/catalog_entry/remote/cid")
                        == Some(&json!("cid-legacy-rest")),
                "legacy backup alias result drifted: {result:?}"
            );
            return Ok(());
        }

        let base_dir = unique_bridge_test_dir("backup-alias")?;
        fs::create_dir_all(&base_dir)?;
        let sentinels = [
            ("settings.json", br#"{"sentinel":"settings"}"#.as_slice()),
            ("idls.json", br#"{"sentinel":"idls"}"#.as_slice()),
            ("wallet.json", br#"{"sentinel":"wallet"}"#.as_slice()),
        ];
        for (name, bytes) in sentinels {
            fs::write(base_dir.join(name), bytes)?;
        }

        let payload = serde_json::to_vec(&json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "favorites": [] } }
        }))?;
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let server = thread::spawn(move || -> Result<Vec<u8>> {
            let deadline = Instant::now() + Duration::from_secs(10);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            bail!("backup alias test server timed out waiting for request");
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => return Err(error.into()),
                }
            };
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            let request = read_bridge_http_headers(&mut stream)?;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                payload.len()
            )?;
            stream.write_all(&payload)?;
            Ok(request)
        });

        let output = Command::new(env::current_exe()?)
            .arg("--exact")
            .arg(BACKUP_ALIAS_TEST_NAME)
            .arg("--nocapture")
            .env(BACKUP_ALIAS_CHILD_ENV, "1")
            .env(BACKUP_ALIAS_ENDPOINT_ENV, &endpoint)
            .env("LOGOS_INSPECTOR_CONFIG_DIR", &base_dir)
            .output()
            .context("failed to run isolated backup alias test")?;
        let request = server
            .join()
            .map_err(|_| anyhow::anyhow!("backup alias test server panicked"))??;

        anyhow::ensure!(
            output.status.success(),
            "isolated backup alias test failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let request = std::str::from_utf8(&request)?;
        anyhow::ensure!(
            request.starts_with("GET /data/cid-legacy-rest/network/stream HTTP/1.1\r\n"),
            "legacy alias used unexpected REST route: {request}"
        );
        for (name, expected) in sentinels {
            let actual = fs::read(base_dir.join(name))?;
            anyhow::ensure!(
                actual == expected,
                "legacy backup alias modified application state file `{name}`"
            );
        }
        let catalog: Value =
            serde_json::from_slice(&fs::read(base_dir.join("backup_catalog.json"))?)?;
        anyhow::ensure!(
            catalog
                .get("entries")
                .and_then(Value::as_array)
                .is_some_and(|entries| entries.len() == 1),
            "legacy backup alias did not record exactly one catalog entry: {catalog:?}"
        );
        fs::remove_dir_all(&base_dir)?;
        Ok(())
    }

    #[test]
    fn logoscore_cli_direct_operation_remains_blocking_compatible() -> Result<()> {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let bridge = InspectorBridge::with_module_transport(RecordingModuleTransport {
            calls: Arc::clone(&calls),
            kind: crate::modules::logos_core::ModuleTransportKind::LogoscoreCli,
        })?;

        let value = bridge.call_module_value(
            INSPECTOR_MODULE,
            "storageFetch",
            json!([{
                "adapter": { "source_mode": "logoscore_cli", "inputs": {} },
                "payload": { "cid": "cid-cli" },
                "mutating_enabled": true
            }]),
        )?;

        anyhow::ensure!(value == json!({ "handled": "fetch" }));
        let calls = calls
            .lock()
            .map_err(|error| anyhow::anyhow!("recorded call lock failed: {error}"))?;
        let [call] = calls.as_slice() else {
            bail!("expected one CLI call through shared seam: {calls:?}");
        };
        anyhow::ensure!(
            call.transport() == crate::modules::logos_core::ModuleTransportKind::LogoscoreCli
                && call.module() == "storage_module"
                && call.method() == "fetch"
                && call.args() == [json!("cid-cli")]
        );
        Ok(())
    }

    #[test]
    fn call_module_response_json_wraps_parse_errors() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let response = bridge.call_module_json(INSPECTOR_MODULE, "sourcePolicy", "{");
        let response: Value = serde_json::from_str(&response)?;

        if response.get("ok").and_then(Value::as_bool) != Some(false)
            || !response.get("value").is_some_and(Value::is_null)
            || response.get("text").and_then(Value::as_str) != Some("")
            || response
                .get("error")
                .and_then(Value::as_str)
                .is_none_or(|error| !error.contains("failed to parse bridge args"))
        {
            bail!("unexpected bridge parse error response: {response}");
        }
        Ok(())
    }

    #[test]
    fn blockchain_live_blocks_bridge_requires_slot_arguments() -> Result<()> {
        let bridge = InspectorBridge::new()?;

        let result = bridge.call_module_value(
            INSPECTOR_MODULE,
            "blockchainLiveBlocks",
            json!(["http://127.0.0.1:8080"]),
        );

        let Err(error) = result else {
            bail!("expected missing slot argument to fail");
        };
        if !error.to_string().contains("slot from is required") {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn source_policy_bridge_exposes_defaults_profiles_and_modes() -> Result<()> {
        let bridge = InspectorBridge::new()?;

        let value = bridge.call_module_value(INSPECTOR_MODULE, "sourcePolicy", json!([]))?;

        if value.get("version").and_then(Value::as_u64) != Some(4)
            || value.pointer("/defaults/sequencer_endpoint").is_some()
            || value.pointer("/defaults/indexer_endpoint").is_some()
            || value
                .pointer("/defaults/storage_rest_endpoint")
                .and_then(Value::as_str)
                != Some(DEFAULT_STORAGE_REST_ENDPOINT)
            || value
                .pointer("/defaults/delivery_rest_endpoint")
                .and_then(Value::as_str)
                != Some(DEFAULT_DELIVERY_REST_ENDPOINT)
        {
            bail!("unexpected source policy defaults: {value}");
        }

        let Some(profiles) = value.get("network_profiles").and_then(Value::as_array) else {
            bail!("source policy missing network profiles: {value}");
        };
        if !profiles
            .iter()
            .any(|profile| profile.get("id").and_then(Value::as_str) == Some("default"))
        {
            bail!("source policy missing default profile: {value}");
        }
        if profiles.iter().any(|profile| {
            profile.get("sequencer_endpoint").is_some() || profile.get("indexer_endpoint").is_some()
        }) {
            bail!("source policy exposes global L2 endpoints: {value}");
        }

        let Some(storage_modes) = value
            .pointer("/source_modes/storage")
            .and_then(Value::as_array)
        else {
            bail!("source policy missing storage modes: {value}");
        };
        let Some(module_mode) = storage_modes
            .iter()
            .find(|mode| mode.get("key").and_then(Value::as_str) == Some("module"))
        else {
            bail!("source policy missing storage module mode: {value}");
        };
        if module_mode
            .pointer("/adapter/target")
            .and_then(Value::as_str)
            != Some("module")
        {
            bail!("source policy missing storage adapter facts: {value}");
        }
        let Some(cli_mode) = storage_modes
            .iter()
            .find(|mode| mode.get("key").and_then(Value::as_str) == Some("logoscore_cli"))
        else {
            bail!("source policy missing LogosCore CLI storage mode: {value}");
        };
        if cli_mode
            .pointer("/adapter/connection_type")
            .and_then(Value::as_str)
            != Some("logoscore_cli")
        {
            bail!("source policy conflated LogosCore CLI with host module: {value}");
        }
        Ok(())
    }

    #[test]
    fn capability_registry_bridge_exposes_registry_report_shape() -> Result<()> {
        let bridge = InspectorBridge::new()?;

        let value = bridge.call_module_value(
            INSPECTOR_MODULE,
            "capabilityRegistryReport",
            json!([true, {}]),
        )?;

        if value.get("schema_version").and_then(Value::as_u64) != Some(1)
            || value.get("build_mode").and_then(Value::as_str) != Some("basecamp")
            || !value.get("capabilities").is_some_and(Value::is_array)
            || !value.get("provider_instances").is_some_and(Value::is_array)
        {
            bail!("unexpected capability registry report shape: {value}");
        }
        let Some(capabilities) = value.get("capabilities").and_then(Value::as_array) else {
            bail!("capabilities missing from registry report: {value}");
        };
        if !capabilities
            .iter()
            .any(|capability| capability.get("key").and_then(Value::as_str) == Some("storage"))
        {
            bail!("storage capability missing from registry report: {value}");
        }
        Ok(())
    }

    #[test]
    fn capability_registry_bridge_requires_runtime_inputs() -> Result<()> {
        let bridge = InspectorBridge::new()?;

        let result =
            bridge.call_module_value(INSPECTOR_MODULE, "capabilityRegistryReport", json!([true]));
        let Err(error) = result else {
            bail!("capability registry accepted missing runtime inputs");
        };
        if !error
            .to_string()
            .contains("capability runtime inputs are required")
        {
            bail!("unexpected capability registry error: {error}");
        }
        Ok(())
    }

    #[test]
    fn source_endpoint_accepts_existing_rpc_shape() -> Result<()> {
        let args = Args::new(json!(["http://127.0.0.1:8080", 1, 2]))?;
        let source = args.source_endpoint(0, "node endpoint")?;

        if source.mode != CoreEndpointMode::Rpc
            || source.endpoint != "http://127.0.0.1:8080"
            || source.next_index != 1
            || source.module != BLOCKCHAIN_MODULE
        {
            bail!("unexpected source endpoint");
        }
        Ok(())
    }

    #[test]
    fn source_endpoint_accepts_module_shape() -> Result<()> {
        let args = Args::new(json!(["module", 42]))?;
        let source = args.source_endpoint(0, "indexer endpoint")?;

        if source.mode != CoreEndpointMode::Module
            || !source.endpoint.is_empty()
            || source.next_index != 1
            || source.module != BLOCKCHAIN_MODULE
        {
            bail!("unexpected source endpoint");
        }
        Ok(())
    }

    #[test]
    fn delivery_store_query_url_defaults_to_hashes_only_and_caps_page_size() -> Result<()> {
        let url = messaging_layer::store_query_url(
            "http://127.0.0.1:8645/",
            messaging_layer::DeliveryStoreQuery {
                peer_addr: Some("/ip4/127.0.0.1/tcp/60001/p2p/peer-a"),
                content_topics: Some("/app/1/chat/proto"),
                pubsub_topic: None,
                cursor: None,
                page_size: 100,
                ascending: true,
                include_data: false,
            },
        )?;
        let text = url.as_str();

        if !text.contains("/store/v3/messages?") {
            bail!("unexpected store path: {text}");
        }
        if !text.contains("includeData=false") || !text.contains("pageSize=100") {
            bail!("unexpected safe query parameters: {text}");
        }
        if !text.contains("peerAddr=%2Fip4%2F127.0.0.1") {
            bail!("peer address was not url encoded: {text}");
        }
        Ok(())
    }

    #[test]
    fn delivery_store_query_url_supports_comment_cursor_and_payloads() -> Result<()> {
        let url = messaging_layer::store_query_url(
            "http://127.0.0.1:8645/",
            messaging_layer::DeliveryStoreQuery {
                peer_addr: None,
                content_topics: Some("/lez/account/account-1/comments"),
                pubsub_topic: None,
                cursor: Some("cursor-1"),
                page_size: 25,
                ascending: true,
                include_data: true,
            },
        )?;
        let text = url.as_str();

        if !text.contains("contentTopics=%2Flez%2Faccount%2Faccount-1%2Fcomments")
            || !text.contains("cursor=cursor-1")
            || !text.contains("pageSize=25")
            || !text.contains("includeData=true")
        {
            bail!("unexpected comment store query parameters: {text}");
        }
        Ok(())
    }

    #[test]
    fn delivery_mutations_require_mutating_diagnostics_flag() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module_value(
            INSPECTOR_MODULE,
            "deliverySend",
            json!([{
                "adapter": {
                    "source_mode": "rest",
                    "inputs": { "rest_endpoint": "http://127.0.0.1:8645" }
                },
                "mutating_enabled": false,
                "payload": { "topic": "/app/1/chat/proto", "payload": "hello" }
            }]),
        );

        let Err(error) = result else {
            bail!("expected disabled mutating diagnostics to fail");
        };
        if !error
            .to_string()
            .contains("requires mutating diagnostics to be enabled")
        {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn storage_mutations_require_mutating_diagnostics_flag() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module_value(
            INSPECTOR_MODULE,
            "storageFetch",
            json!([{
                "adapter": {
                    "source_mode": "rest",
                    "inputs": { "rest_endpoint": "http://127.0.0.1:8080/api/storage/v1" }
                },
                "mutating_enabled": false,
                "payload": { "cid": "zDvtest" }
            }]),
        );

        let Err(error) = result else {
            bail!("expected disabled mutating diagnostics to fail");
        };
        if !error
            .to_string()
            .contains("requires mutating diagnostics to be enabled")
        {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn local_wallet_deploy_program_requires_confirmation() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module_value(
            INSPECTOR_MODULE,
            "localWalletDeployProgram",
            json!([
                {
                    "wallet_binary": "wallet",
                    "wallet_home": ".",
                    "network_profile": "local"
                },
                "program.bin"
            ]),
        );

        let Err(error) = result else {
            bail!("expected missing deployment confirmation to fail");
        };
        if !error
            .to_string()
            .contains("program deployment requires explicit confirmation")
        {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn local_wallet_deploy_program_reaches_wallet_validation_after_confirmation() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module_value(
            INSPECTOR_MODULE,
            "localWalletDeployProgram",
            json!([
                {
                    "wallet_binary": "",
                    "wallet_home": ".",
                    "network_profile": "local"
                },
                "program.bin",
                "confirm-deploy-program"
            ]),
        );

        let Err(error) = result else {
            bail!("expected wallet validation to fail");
        };
        if !error
            .to_string()
            .contains("wallet binary is required to deploy program binary")
        {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn local_wallet_create_account_requires_confirmation() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module_value(
            INSPECTOR_MODULE,
            "localWalletCreateAccount",
            json!([
                {
                    "wallet_binary": "wallet",
                    "wallet_home": ".",
                    "network_profile": "local"
                },
                "public",
                ""
            ]),
        );

        let Err(error) = result else {
            bail!("expected missing create confirmation to fail");
        };
        if !error
            .to_string()
            .contains("wallet account creation requires explicit confirmation")
        {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn local_wallet_send_transaction_requires_confirmation() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module_value(
            INSPECTOR_MODULE,
            "localWalletSendTransaction",
            json!([
                {
                    "wallet_binary": "wallet",
                    "wallet_home": ".",
                    "network_profile": "local"
                },
                {
                    "from": "Public/source",
                    "to": "Public/recipient",
                    "amount": "1"
                }
            ]),
        );

        let Err(error) = result else {
            bail!("expected missing send confirmation to fail");
        };
        if !error
            .to_string()
            .contains("wallet transaction send requires explicit confirmation")
        {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn local_wallet_instruction_submit_requires_confirmation() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module_value(
            INSPECTOR_MODULE,
            "localWalletInstructionSubmit",
            json!([
                {
                    "wallet_home": ".",
                    "network_profile": "local"
                },
                {
                    "idl_json": "{}",
                    "program_id_hex": "00",
                    "instruction": "set"
                }
            ]),
        );

        let Err(error) = result else {
            bail!("expected missing IDL instruction confirmation to fail");
        };
        if !error
            .to_string()
            .contains("IDL instruction send requires explicit confirmation")
        {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn local_wallet_command_requires_confirmation() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module_value(
            INSPECTOR_MODULE,
            "localWalletCommand",
            json!([
                {
                    "wallet_binary": "wallet",
                    "wallet_home": ".",
                    "network_profile": "local"
                },
                ["account", "list"]
            ]),
        );

        let Err(error) = result else {
            bail!("expected missing wallet command confirmation to fail");
        };
        if !error
            .to_string()
            .contains("wallet command requires explicit confirmation")
        {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn local_nodes_action_requires_confirmation() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module_value(
            INSPECTOR_MODULE,
            "localNodesAction",
            json!(["local", { "action": "new_network", "network_id": "devnet-test" }]),
        );

        let Err(error) = result else {
            bail!("expected missing local node confirmation to fail");
        };
        if !error
            .to_string()
            .contains("local node action requires explicit confirmation")
        {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn runtime_operation_cancel_marks_cancelable_operation() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let cancel_requested = bridge
            .surface
            .operations_for_test()
            .insert_test_running_operation(
                "existing",
                storage_download_request("cid-existing", "/tmp/existing")?,
            )?;

        let value = bridge.call_module_value(
            INSPECTOR_MODULE,
            "nodeOperationCancel",
            json!(["existing"]),
        )?;

        if value.get("status").and_then(Value::as_str) != Some("canceling") {
            bail!("expected canceling status: {value}");
        }
        if !cancel_requested.load(std::sync::atomic::Ordering::Relaxed) {
            bail!("expected cancel flag to be set");
        }
        Ok(())
    }

    #[test]
    fn runtime_operation_start_accepts_storage_download_request() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let value = bridge.call_module_value(
            INSPECTOR_MODULE,
            "nodeOperationStart",
            json!([{
                "domain": "storage",
                "method": "storageDownloadToUrl",
                "adapter": {
                    "source_mode": "rest",
                    "inputs": { "rest_endpoint": "http://127.0.0.1:8080/api/storage/v1" }
                },
                "payload": { "cid": "cid-b", "path": "/tmp/b", "local_only": false },
                "mutating_enabled": true,
                "label": "Storage download"
            }]),
        )?;

        if value.get("domain").and_then(Value::as_str) != Some("storage")
            || value.get("method").and_then(Value::as_str) != Some("storageDownloadToUrl")
            || value.get("cancellable").and_then(Value::as_bool) != Some(true)
            || value.get("cid").and_then(Value::as_str) != Some("cid-b")
        {
            bail!("unexpected operation value: {value}");
        }
        Ok(())
    }

    #[test]
    fn runtime_operation_request_accepts_typed_storage_payload() -> Result<()> {
        let request =
            crate::inspector::commands::operations::runtime_operation_request_from_value(json!({
                "domain": "storage",
                "method": "storageDownloadManifest",
                "adapter": {
                    "source_mode": "rest",
                    "inputs": { "rest_endpoint": "http://127.0.0.1:8080/api/storage/v1" }
                },
                "payload": { "cid": "z-storage" }
            }))?;

        if request.method_name() != "storageDownloadManifest" || request.args() != &json!([]) {
            bail!("unexpected typed request");
        }
        Ok(())
    }

    #[test]
    fn runtime_operation_request_accepts_typed_delivery_payload() -> Result<()> {
        let request =
            crate::inspector::commands::operations::runtime_operation_request_from_value(json!({
                "domain": "delivery",
                "method": "deliverySend",
                "adapter": {
                    "source_mode": "rest",
                    "inputs": { "rest_endpoint": "http://127.0.0.1:8645" }
                },
                "mutating_enabled": true,
                "payload": { "topic": "/waku/2/default/proto", "payload": "hello" }
            }))?;

        if request.method_name() != "deliverySend" || request.args() != &json!([]) {
            bail!("unexpected typed request");
        }
        Ok(())
    }

    #[test]
    fn runtime_operation_request_accepts_typed_delivery_store_query() -> Result<()> {
        let request =
            crate::inspector::commands::operations::runtime_operation_request_from_value(json!({
                "domain": "delivery",
                "method": "deliveryStoreQuery",
                "adapter": {
                    "source_mode": "rest",
                    "inputs": { "rest_endpoint": "http://127.0.0.1:8645" }
                },
                "payload": {
                    "peer_addr": "peer-a",
                    "content_topics": "/topic/1/a/proto",
                    "pubsub_topic": "/waku/2/default-waku/proto",
                    "cursor": "cursor-a",
                    "page_size": 10,
                    "ascending": true,
                    "include_data": true
                }
            }))?;

        if request.method_name() != "deliveryStoreQuery" || request.args() != &json!([]) {
            bail!("unexpected typed request");
        }
        Ok(())
    }

    #[test]
    fn runtime_operation_start_rejects_second_storage_download() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        bridge
            .surface
            .operations_for_test()
            .insert_test_running_operation(
                "storage-download-existing",
                storage_download_request("cid-existing", "/tmp/existing")?,
            )?;

        let result = bridge.call_module_value(
            INSPECTOR_MODULE,
            "nodeOperationStart",
            json!([{
                "domain": "storage",
                "method": "storageDownloadToUrl",
                "adapter": {
                    "source_mode": "rest",
                    "inputs": { "rest_endpoint": "http://127.0.0.1:8080/api/storage/v1" }
                },
                "payload": { "cid": "cid-c", "path": "/tmp/c", "local_only": false },
                "mutating_enabled": true,
                "label": "Storage download"
            }]),
        );

        let Err(error) = result else {
            bail!("expected duplicate storage download to fail");
        };
        if !error.to_string().contains("storage download operation") {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn runtime_operation_request_accepts_module_delivery_adapter() -> Result<()> {
        let request =
            crate::inspector::commands::operations::runtime_operation_request_from_value(json!({
                "domain": "delivery",
                "method": "deliverySend",
                "adapter": { "source_mode": "module", "inputs": {} },
                "payload": { "topic": "/waku/2/default/proto", "payload": "hello" },
                "mutating_enabled": true,
                "label": "Send message"
            }))?;

        if request.method_name() != "deliverySend" || request.args() != &json!([]) {
            bail!("unexpected typed request");
        }
        Ok(())
    }

    #[test]
    fn wallet_operation_record_is_removed_after_wait() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result =
            bridge.call_module_value(INSPECTOR_MODULE, "localWalletCreateAccount", json!([]));

        let Err(error) = result else {
            bail!("expected wallet operation to fail before execution");
        };
        if !error
            .to_string()
            .contains("wallet account creation requires explicit confirmation")
        {
            bail!("unexpected error: {error:#}");
        }
        let operations_len = bridge.surface.operations_for_test().len()?;
        if operations_len != 0 {
            bail!("expected operation registry cleanup, found {operations_len}",);
        }
        Ok(())
    }

    fn read_bridge_http_headers(stream: &mut std::net::TcpStream) -> Result<Vec<u8>> {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let bytes = stream.read(&mut buffer)?;
            if bytes == 0 {
                bail!("bridge test HTTP headers were incomplete");
            }
            request.extend_from_slice(
                buffer
                    .get(..bytes)
                    .context("bridge test HTTP header chunk was invalid")?,
            );
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                return Ok(request);
            }
        }
    }

    fn unique_bridge_test_dir(label: &str) -> Result<std::path::PathBuf> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock is before Unix epoch")?
            .as_nanos();
        Ok(env::temp_dir().join(format!(
            "logos-inspector-bridge-{label}-{}-{nanos}",
            std::process::id()
        )))
    }
}
