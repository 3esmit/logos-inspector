use anyhow::{Context as _, Result, bail};
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use tokio::runtime::Runtime;

use super::dispatch_catalog::{DispatchContext, dispatch};
use crate::bridge_envelope::bridge_response_json;
#[cfg(test)]
use crate::source_routing::{
    self, CoreEndpointMode, DEFAULT_DELIVERY_REST_ENDPOINT, DEFAULT_STORAGE_REST_ENDPOINT,
    DeliveryStoreQuery, delivery_rest_source, delivery_store_query_url, storage_rest_source,
};
use crate::{inspector::operations::RuntimeOperationInterface, logoscore, source_routing::Args};

pub const INSPECTOR_MODULE: &str = "logos_inspector";
#[cfg(test)]
const BLOCKCHAIN_MODULE: &str = source_routing::BLOCKCHAIN_MODULE;
#[cfg(test)]
const INDEXER_MODULE: &str = source_routing::INDEXER_MODULE;

pub struct InspectorBridge {
    runtime: Runtime,
    runtime_operations: RuntimeOperationInterface,
}

impl InspectorBridge {
    pub fn new() -> Result<Self> {
        Ok(Self {
            runtime: Runtime::new().context("failed to create tokio runtime")?,
            runtime_operations: RuntimeOperationInterface::default(),
        })
    }

    pub fn call_module(&self, module: &str, method: &str, args: Value) -> Result<Value> {
        if module == INSPECTOR_MODULE {
            self.call_inspector(method, args)
        } else {
            self.call_logoscore_module(module, method, args)
        }
    }

    fn call_inspector(&self, method: &str, args: Value) -> Result<Value> {
        let call_core_module = |module: &str, method: &str, args: Value| {
            self.call_logoscore_module(module, method, args)
        };
        let context = DispatchContext {
            runtime: &self.runtime,
            operations: &self.runtime_operations,
            call_core_module: &call_core_module,
        };
        if let Some(value) = dispatch(&context, method, args)? {
            return Ok(value);
        }
        bail!("unknown inspector method `{method}`")
    }

    fn call_logoscore_module(&self, module: &str, method: &str, args: Value) -> Result<Value> {
        let args = Args::new(args)?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| value.to_string())
            })
            .collect::<Vec<_>>();
        to_value(logoscore::call(module, method, &args)?)
    }
}

pub fn call_module_response_json(
    bridge: &InspectorBridge,
    module: &str,
    method: &str,
    args_json: &str,
) -> String {
    let result = serde_json::from_str(args_json)
        .context("failed to parse bridge args")
        .and_then(|args| bridge.call_module(module, method, args));
    bridge_response_json(result)
}

pub fn call_inspector_response_json(
    bridge: &InspectorBridge,
    method: &str,
    args_json: &str,
) -> String {
    call_module_response_json(bridge, INSPECTOR_MODULE, method, args_json)
}

pub(crate) fn to_value(value: impl serde::Serialize) -> Result<Value> {
    serde_json::to_value(value).context("failed to serialize bridge response")
}

pub(crate) async fn blocking_value(
    label: &'static str,
    task: impl FnOnce() -> Result<Value> + Send + 'static,
) -> Result<Value> {
    tokio::task::spawn_blocking(task)
        .await
        .with_context(|| format!("{label} task failed"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_module_response_json_wraps_parse_errors() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let response = call_module_response_json(&bridge, INSPECTOR_MODULE, "sourcePolicy", "{");
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
    fn indexer_status_bridge_requires_endpoint_argument() -> Result<()> {
        let bridge = InspectorBridge::new()?;

        let result = bridge.call_module(INSPECTOR_MODULE, "indexerStatus", json!([]));

        let Err(error) = result else {
            bail!("expected missing indexer endpoint to fail");
        };
        if !error.to_string().contains("indexer endpoint is required") {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn blockchain_live_blocks_bridge_requires_slot_arguments() -> Result<()> {
        let bridge = InspectorBridge::new()?;

        let result = bridge.call_module(
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

        let value = bridge.call_module(INSPECTOR_MODULE, "sourcePolicy", json!([]))?;

        if value.get("version").and_then(Value::as_u64) != Some(2)
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
        let args = Args::new(json!(["module", "http://127.0.0.1:8779", 42]))?;
        let source = args.source_endpoint(0, "indexer endpoint")?;

        if source.mode != CoreEndpointMode::Module
            || source.endpoint != "http://127.0.0.1:8779"
            || source.next_index != 2
            || source.module != INDEXER_MODULE
        {
            bail!("unexpected source endpoint");
        }
        Ok(())
    }

    #[test]
    fn account_sources_accepts_mixed_source_shape() -> Result<()> {
        let args = Args::new(json!([
            "rpc",
            "https://testnet.lez.logos.co/",
            "module",
            "http://127.0.0.1:8779/",
            "account-1"
        ]))?;
        let sources = args.account_sources()?;

        if sources.execution_mode != CoreEndpointMode::Rpc
            || sources.sequencer_endpoint != "https://testnet.lez.logos.co/"
            || sources.indexer_mode != CoreEndpointMode::Module
            || sources.indexer_endpoint != "http://127.0.0.1:8779/"
            || sources.account != "account-1"
            || sources.next_index != 5
        {
            bail!("unexpected account sources");
        }
        Ok(())
    }

    #[test]
    fn storage_rest_source_rejects_module_mode_for_rest_actions() -> Result<()> {
        let args = Args::new(json!([
            "module",
            "http://127.0.0.1:8080/api/storage/v1",
            "cid"
        ]))?;
        let result = storage_rest_source(&args);

        let Err(error) = result else {
            bail!("expected module source to fail");
        };
        if !error.to_string().contains("require storage REST source") {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn delivery_rest_source_rejects_metrics_for_message_actions() -> Result<()> {
        let args = Args::new(json!(["metrics", "http://127.0.0.1:8008/metrics", "topic"]))?;
        let result = delivery_rest_source(&args);

        let Err(error) = result else {
            bail!("expected metrics source to fail");
        };
        if !error.to_string().contains("require delivery REST source") {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn delivery_store_query_url_defaults_to_hashes_only_and_caps_page_size() -> Result<()> {
        let url = delivery_store_query_url(
            "http://127.0.0.1:8645/",
            DeliveryStoreQuery {
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
        let url = delivery_store_query_url(
            "http://127.0.0.1:8645/",
            DeliveryStoreQuery {
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
        let result = bridge.call_module(
            INSPECTOR_MODULE,
            "deliverySend",
            json!([
                "rest",
                "http://127.0.0.1:8645",
                false,
                "/app/1/chat/proto",
                "hello"
            ]),
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
        let result = bridge.call_module(
            INSPECTOR_MODULE,
            "storageFetch",
            json!([
                "rest",
                "http://127.0.0.1:8080/api/storage/v1",
                false,
                "zDvtest"
            ]),
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
        let result = bridge.call_module(
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
        let result = bridge.call_module(
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
        let result = bridge.call_module(
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
        let result = bridge.call_module(
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
        let result = bridge.call_module(
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
        let result = bridge.call_module(
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
        let result = bridge.call_module(
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
    fn node_operation_cancel_marks_cancelable_operation() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let cancel_requested = bridge.runtime_operations.insert_test_running_operation(
            "existing",
            "storage",
            "storageDownloadToUrl",
            true,
        );

        let value =
            bridge.call_module(INSPECTOR_MODULE, "nodeOperationCancel", json!(["existing"]))?;

        if value.get("status").and_then(Value::as_str) != Some("canceling") {
            bail!("expected canceling status: {value}");
        }
        if !cancel_requested.load(std::sync::atomic::Ordering::Relaxed) {
            bail!("expected cancel flag to be set");
        }
        Ok(())
    }

    #[test]
    fn node_operation_start_accepts_storage_download_request() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let value = bridge.call_module(
            INSPECTOR_MODULE,
            "nodeOperationStart",
            json!([{
                "domain": "storage",
                "sourceMode": "rest",
                "endpoint": "http://127.0.0.1:8080/api/storage/v1",
                "method": "storageDownloadToUrl",
                "args": ["cid-b", "/tmp/b", false],
                "mutatingEnabled": true,
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
    fn node_operation_request_normalizes_storage_endpoint_first_args() -> Result<()> {
        let request = crate::inspector::operations::node_operation_request_from_value(json!({
            "domain": "storage",
            "sourceMode": "rest",
            "endpoint": "http://127.0.0.1:8080/api/storage/v1",
            "method": "storageDownloadManifest",
            "args": ["http://127.0.0.1:8080/api/storage/v1", "z-storage"]
        }))?;

        if request.args() != &json!(["rest", "http://127.0.0.1:8080/api/storage/v1", "z-storage"]) {
            bail!("unexpected normalized args: {}", request.args());
        }
        Ok(())
    }

    #[test]
    fn node_operation_request_normalizes_delivery_endpoint_first_args() -> Result<()> {
        let request = crate::inspector::operations::node_operation_request_from_value(json!({
            "domain": "delivery",
            "sourceMode": "rest",
            "endpoint": "http://127.0.0.1:8645",
            "method": "deliverySend",
            "mutatingEnabled": true,
            "args": ["http://127.0.0.1:8645", "/waku/2/default/proto", "hello"]
        }))?;

        if request.args()
            != &json!([
                "rest",
                "http://127.0.0.1:8645",
                true,
                "/waku/2/default/proto",
                "hello"
            ])
        {
            bail!("unexpected normalized args: {}", request.args());
        }
        Ok(())
    }

    #[test]
    fn node_operation_request_keeps_delivery_store_query_read_only_args() -> Result<()> {
        let request = crate::inspector::operations::node_operation_request_from_value(json!({
            "domain": "delivery",
            "sourceMode": "rest",
            "endpoint": "http://127.0.0.1:8645",
            "method": "deliveryStoreQuery",
            "args": ["peer-a", "/topic/1/a/proto", "/waku/2/default-waku/proto", "cursor-a", 10, true, true]
        }))?;

        if request.args()
            != &json!([
                "rest",
                "http://127.0.0.1:8645",
                "peer-a",
                "/topic/1/a/proto",
                "/waku/2/default-waku/proto",
                "cursor-a",
                10,
                true,
                true
            ])
        {
            bail!("unexpected normalized args: {}", request.args());
        }
        Ok(())
    }

    #[test]
    fn node_operation_start_rejects_second_storage_download() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        bridge.runtime_operations.insert_test_running_operation(
            "storage-download-existing",
            "storage",
            "storageDownloadToUrl",
            true,
        );

        let result = bridge.call_module(
            INSPECTOR_MODULE,
            "nodeOperationStart",
            json!([{
                "domain": "storage",
                "sourceMode": "rest",
                "endpoint": "http://127.0.0.1:8080/api/storage/v1",
                "method": "storageDownloadToUrl",
                "args": ["cid-c", "/tmp/c", false],
                "mutatingEnabled": true,
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
    fn node_operation_request_normalizes_module_delivery_send_args() -> Result<()> {
        let request = crate::inspector::operations::node_operation_request_from_value(json!({
            "domain": "delivery",
            "sourceMode": "module",
            "endpoint": "",
            "method": "deliverySend",
            "args": ["/waku/2/default/proto", "hello"],
            "mutatingEnabled": true,
            "label": "Send message"
        }))?;

        if request.args()
            != &json!([
                "module",
                DEFAULT_DELIVERY_REST_ENDPOINT,
                true,
                "/waku/2/default/proto",
                "hello"
            ])
        {
            bail!("unexpected normalized args: {}", request.args());
        }
        Ok(())
    }

    #[test]
    fn wallet_operation_record_is_removed_after_wait() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module(INSPECTOR_MODULE, "localWalletCreateAccount", json!([]));

        let Err(error) = result else {
            bail!("expected wallet operation to fail before execution");
        };
        if !error
            .to_string()
            .contains("wallet account creation requires explicit confirmation")
        {
            bail!("unexpected error: {error:#}");
        }
        let operations_len = bridge.runtime_operations.len()?;
        if operations_len != 0 {
            bail!("expected operation registry cleanup, found {operations_len}",);
        }
        Ok(())
    }
}
