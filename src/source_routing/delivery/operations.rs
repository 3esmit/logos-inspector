use anyhow::{Context as _, Result, bail};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::modules::logos_core::{ModuleTransportKind, SharedModuleTransport};
use crate::source_routing::{
    AdapterInitialization, DeliverySourceMode, ModuleDispatchIdentityRole, ModuleDispatchReceipt,
    ModuleEventCorrelationKind, ModuleTerminalEventContract, NodeOperationOutcome,
    NodeOperationRequest, ObservableOperationAcceptance,
};

use super::{layer::MESSAGING_SOURCE_MODES, transport};

const MAX_STORE_PAGE_SIZE: u64 = 100;
const DELIVERY_STORE_TIMEOUT_MS: i64 = 5_000;
const DELIVERY_STORE_REQUEST_ID_BYTES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeliveryOperation {
    Subscribe,
    Unsubscribe,
    Send,
    CreateNode,
    Start,
    Stop,
    StoreQuery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MessagingOperationAdapter {
    Module {
        transport: ModuleTransportKind,
        store_peer_addr: Option<String>,
    },
    Rest {
        endpoint: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DeliveryStoreQuery<'a> {
    pub(crate) peer_addr: Option<&'a str>,
    pub(crate) content_topics: Option<&'a str>,
    pub(crate) pubsub_topic: Option<&'a str>,
    pub(crate) cursor: Option<&'a str>,
    pub(crate) page_size: u64,
    pub(crate) ascending: bool,
    pub(crate) include_data: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DeliveryOperationRequest {
    plan: DeliveryOperationPlan,
    context: Map<String, Value>,
}

impl DeliveryOperationRequest {
    pub(crate) fn parse(
        request: &NodeOperationRequest,
        operation: DeliveryOperation,
    ) -> Result<Self> {
        let adapter = parse_adapter(request.adapter())?;
        let (plan, context) = operation_plan(request, adapter, operation)?;
        Ok(Self { plan, context })
    }

    #[must_use]
    pub(crate) fn context(&self) -> &Map<String, Value> {
        &self.context
    }
}

pub(crate) async fn execute_operation(
    request: DeliveryOperationRequest,
    module_transport: SharedModuleTransport,
) -> Result<NodeOperationOutcome> {
    execute_plan(request.plan, module_transport).await
}

#[cfg(test)]
pub(crate) fn store_query_url(endpoint: &str, query: DeliveryStoreQuery<'_>) -> Result<url::Url> {
    transport::store_query_url(endpoint, query)
}

#[derive(Debug, Clone, PartialEq)]
enum DeliveryOperationPlan {
    Module {
        transport: ModuleTransportKind,
        method: &'static str,
        args: Vec<Value>,
        context: Vec<(&'static str, String)>,
        dispatch: bool,
    },
    ModuleStoreQuery {
        transport: ModuleTransportKind,
        peer_addr: String,
        query: Value,
        page_size: u64,
        include_data: bool,
    },
    Rest(DeliveryRestOperation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DeliveryRestOperation {
    Subscription {
        endpoint: String,
        topic: String,
        subscribe: bool,
    },
    Send {
        endpoint: String,
        topic: String,
        payload: String,
    },
    StoreQuery {
        endpoint: String,
        peer_addr: Option<String>,
        content_topics: Option<String>,
        pubsub_topic: Option<String>,
        cursor: Option<String>,
        page_size: u64,
        ascending: bool,
        include_data: bool,
    },
}

#[derive(Debug, Deserialize)]
struct TopicPayload {
    topic: String,
}

#[derive(Debug, Deserialize)]
struct SendPayload {
    topic: String,
    payload: String,
}

#[derive(Debug, Default, Deserialize)]
struct EmptyPayload {}

#[derive(Debug, Deserialize)]
struct CreateNodePayload {
    config: String,
}

#[derive(Debug, Default, Deserialize)]
struct StoreQueryPayload {
    #[serde(default)]
    peer_addr: String,
    #[serde(default)]
    content_topics: String,
    #[serde(default)]
    pubsub_topic: String,
    #[serde(default)]
    cursor: String,
    #[serde(default = "default_page_size")]
    page_size: u64,
    #[serde(default)]
    ascending: bool,
    #[serde(default)]
    include_data: bool,
}

fn operation_plan(
    request: &NodeOperationRequest,
    adapter: MessagingOperationAdapter,
    operation: DeliveryOperation,
) -> Result<(DeliveryOperationPlan, Map<String, Value>)> {
    match operation {
        DeliveryOperation::Subscribe | DeliveryOperation::Unsubscribe => {
            let payload: TopicPayload = request.payload("delivery subscription")?;
            let topic = required_text(payload.topic, "content topic")?;
            let subscribe = operation == DeliveryOperation::Subscribe;
            match adapter {
                MessagingOperationAdapter::Module { transport, .. } => Ok((
                    DeliveryOperationPlan::Module {
                        transport,
                        method: if subscribe {
                            "subscribe"
                        } else {
                            "unsubscribe"
                        },
                        args: vec![json!(topic)],
                        context: Vec::new(),
                        dispatch: false,
                    },
                    context_map(&[("contentTopic", topic)]),
                )),
                MessagingOperationAdapter::Rest { endpoint } => {
                    let mut context = context_map(&[("contentTopic", topic.clone())]);
                    context.insert("endpoint".to_owned(), json!(endpoint));
                    Ok((
                        DeliveryOperationPlan::Rest(DeliveryRestOperation::Subscription {
                            endpoint,
                            topic,
                            subscribe,
                        }),
                        context,
                    ))
                }
            }
        }
        DeliveryOperation::Send => {
            let payload: SendPayload = request.payload("delivery send")?;
            let topic = required_text(payload.topic, "content topic")?;
            let message = required_text(payload.payload, "message payload")?;
            let bytes = message.len().to_string();
            match adapter {
                MessagingOperationAdapter::Module { transport, .. } => Ok((
                    DeliveryOperationPlan::Module {
                        transport,
                        method: "send",
                        args: vec![json!(topic), json!(message)],
                        context: vec![("contentTopic", topic.clone()), ("bytes", bytes.clone())],
                        dispatch: true,
                    },
                    context_map(&[("contentTopic", topic), ("bytes", bytes)]),
                )),
                MessagingOperationAdapter::Rest { endpoint } => {
                    let mut context =
                        context_map(&[("contentTopic", topic.clone()), ("bytes", bytes)]);
                    context.insert("endpoint".to_owned(), json!(endpoint));
                    Ok((
                        DeliveryOperationPlan::Rest(DeliveryRestOperation::Send {
                            endpoint,
                            topic,
                            payload: message,
                        }),
                        context,
                    ))
                }
            }
        }
        DeliveryOperation::CreateNode => {
            require_module_adapter(&adapter)?;
            let payload: CreateNodePayload = request.payload("delivery node creation")?;
            let config = required_text(payload.config, "node config")?;
            Ok((
                DeliveryOperationPlan::Module {
                    transport: module_transport_kind(&adapter)?,
                    method: "createNode",
                    args: vec![json!(config)],
                    context: Vec::new(),
                    dispatch: false,
                },
                Map::new(),
            ))
        }
        DeliveryOperation::Start | DeliveryOperation::Stop => {
            require_module_adapter(&adapter)?;
            let _payload: EmptyPayload = request.payload("delivery node lifecycle")?;
            Ok((
                DeliveryOperationPlan::Module {
                    transport: module_transport_kind(&adapter)?,
                    method: if operation == DeliveryOperation::Start {
                        "start"
                    } else {
                        "stop"
                    },
                    args: Vec::new(),
                    context: Vec::new(),
                    dispatch: true,
                },
                Map::new(),
            ))
        }
        DeliveryOperation::StoreQuery => {
            let payload: StoreQueryPayload = request.payload("Delivery Store query")?;
            let content_topics = optional_text(payload.content_topics);
            let mut context = Map::new();
            if let Some(topic) = &content_topics {
                context.insert("contentTopic".to_owned(), json!(topic));
            }
            let pubsub_topic = optional_text(payload.pubsub_topic);
            let cursor = optional_text(payload.cursor);
            let page_size = payload.page_size.clamp(1, MAX_STORE_PAGE_SIZE);
            match adapter {
                MessagingOperationAdapter::Module {
                    transport: ModuleTransportKind::LogoscoreCli,
                    store_peer_addr,
                } => {
                    let peer_addr = optional_text(payload.peer_addr)
                        .or(store_peer_addr)
                        .context("Store provider multiaddress is required for LogosCore CLI")?;
                    let query = delivery_store_query_request(
                        &new_delivery_store_request_id()?,
                        content_topics.as_deref(),
                        pubsub_topic.as_deref(),
                        cursor.as_deref(),
                        page_size,
                        payload.ascending,
                        payload.include_data,
                    );
                    context.insert("storePeer".to_owned(), json!(&peer_addr));
                    Ok((
                        DeliveryOperationPlan::ModuleStoreQuery {
                            transport: ModuleTransportKind::LogoscoreCli,
                            peer_addr,
                            query,
                            page_size,
                            include_data: payload.include_data,
                        },
                        context,
                    ))
                }
                MessagingOperationAdapter::Module { .. } => {
                    bail!("Delivery Store query requires LogosCore CLI Delivery source")
                }
                MessagingOperationAdapter::Rest { endpoint } => {
                    context.insert("endpoint".to_owned(), json!(endpoint));
                    Ok((
                        DeliveryOperationPlan::Rest(DeliveryRestOperation::StoreQuery {
                            endpoint,
                            peer_addr: optional_text(payload.peer_addr),
                            content_topics,
                            pubsub_topic,
                            cursor,
                            page_size,
                            ascending: payload.ascending,
                            include_data: payload.include_data,
                        }),
                        context,
                    ))
                }
            }
        }
    }
}

async fn execute_plan(
    plan: DeliveryOperationPlan,
    module_transport: SharedModuleTransport,
) -> Result<NodeOperationOutcome> {
    match plan {
        DeliveryOperationPlan::Module {
            transport,
            method,
            args,
            context,
            dispatch,
        } => {
            execute_module_plan(
                &module_transport,
                transport,
                method,
                args,
                context,
                dispatch,
            )
            .await
        }
        DeliveryOperationPlan::ModuleStoreQuery {
            transport,
            peer_addr,
            query,
            page_size,
            include_data,
        } => {
            let value = super::transport::module_call(
                &module_transport,
                transport,
                "storeQuery",
                vec![
                    json!(query.to_string()),
                    json!(&peer_addr),
                    json!(DELIVERY_STORE_TIMEOUT_MS),
                ],
            )
            .await
            .context("failed to query Delivery Store through LogosCore CLI")?;
            Ok(NodeOperationOutcome::Completed(json!({
                "storePeer": peer_addr,
                "includeData": include_data,
                "pageSize": page_size,
                "query": query,
                "value": value,
            })))
        }
        DeliveryOperationPlan::Rest(DeliveryRestOperation::Subscription {
            endpoint,
            topic,
            subscribe,
        }) => Ok(NodeOperationOutcome::Completed(
            transport::update_subscription(&endpoint, &topic, subscribe)
                .await
                .with_context(|| format!("failed to update relay subscription for {topic}"))?,
        )),
        DeliveryOperationPlan::Rest(DeliveryRestOperation::Send {
            endpoint,
            topic,
            payload,
        }) => Ok(NodeOperationOutcome::Completed(
            transport::send(&endpoint, &topic, &payload)
                .await
                .with_context(|| format!("failed to send relay message on {topic}"))?,
        )),
        DeliveryOperationPlan::Rest(DeliveryRestOperation::StoreQuery {
            endpoint,
            peer_addr,
            content_topics,
            pubsub_topic,
            cursor,
            page_size,
            ascending,
            include_data,
        }) => {
            let (query, value) = transport::store_query(
                &endpoint,
                DeliveryStoreQuery {
                    peer_addr: peer_addr.as_deref(),
                    content_topics: content_topics.as_deref(),
                    pubsub_topic: pubsub_topic.as_deref(),
                    cursor: cursor.as_deref(),
                    page_size,
                    ascending,
                    include_data,
                },
            )
            .await
            .context("failed to query Delivery Store")?;
            Ok(NodeOperationOutcome::Completed(json!({
                "endpoint": endpoint,
                "includeData": include_data,
                "pageSize": page_size,
                "query": query,
                "value": value,
            })))
        }
    }
}

async fn execute_module_plan(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    method: &'static str,
    args: Vec<Value>,
    context: Vec<(&'static str, String)>,
    dispatch: bool,
) -> Result<NodeOperationOutcome> {
    if !dispatch {
        return super::transport::module_call(transport, transport_kind, method, args)
            .await
            .map(NodeOperationOutcome::Completed);
    }

    let identity_role = if method == "send" {
        ModuleDispatchIdentityRole::Request
    } else {
        ModuleDispatchIdentityRole::None
    };
    let receipt = super::transport::module_dispatch(
        transport,
        transport_kind,
        method,
        args,
        &context,
        identity_role,
    )
    .await?;
    delivery_module_dispatch_outcome(method, receipt)
}

#[cfg(test)]
pub(crate) async fn execute_module_adapter_fixture(
    method: &'static str,
    dispatch: bool,
    value: Value,
) -> Result<NodeOperationOutcome> {
    let transport: SharedModuleTransport = std::sync::Arc::new(FakeDeliveryModuleTransport {
        value,
        last_call: std::sync::Mutex::new(None),
    });
    execute_module_plan(
        &transport,
        ModuleTransportKind::LogoscoreCli,
        method,
        Vec::new(),
        Vec::new(),
        dispatch,
    )
    .await
}

#[cfg(test)]
struct FakeDeliveryModuleTransport {
    value: Value,
    last_call: std::sync::Mutex<Option<crate::modules::logos_core::ModuleCall>>,
}

#[cfg(test)]
impl crate::modules::logos_core::ModuleTransport for FakeDeliveryModuleTransport {
    fn kind(&self) -> ModuleTransportKind {
        ModuleTransportKind::LogoscoreCli
    }

    fn call(
        &self,
        call: crate::modules::logos_core::ModuleCall,
    ) -> crate::modules::logos_core::ModuleCallFuture<'_> {
        Box::pin(async move {
            let mut last_call = self.last_call.lock().map_err(|error| {
                anyhow::anyhow!("Delivery module call recording lock failed: {error}")
            })?;
            *last_call = Some(call);
            Ok(crate::modules::logos_core::ModuleCallReply::new(
                ModuleTransportKind::LogoscoreCli,
                self.value.clone(),
            ))
        })
    }
}

fn delivery_module_dispatch_outcome(
    method: &str,
    receipt: ModuleDispatchReceipt,
) -> Result<NodeOperationOutcome> {
    let accepted = (method == "send")
        .then(|| receipt.request_correlation())
        .flatten()
        .map(|correlation| {
            (
                correlation,
                ModuleTerminalEventContract::new(
                    super::layer::module_id(),
                    Some("messagePropagated"),
                    "messageSent",
                    Some("messageError"),
                    ModuleEventCorrelationKind::Request,
                ),
            )
        });
    let acknowledgement = receipt.into_acknowledgement();
    match accepted {
        Some((correlation, terminal_event)) => Ok(NodeOperationOutcome::Accepted(Box::new(
            ObservableOperationAcceptance::new(acknowledgement, correlation, terminal_event),
        ))),
        None if method == "send" => bail!("delivery module `send` returned no request ID"),
        None => Ok(NodeOperationOutcome::Dispatched(acknowledgement)),
    }
}

fn parse_adapter(value: &Value) -> Result<MessagingOperationAdapter> {
    let initialization = AdapterInitialization::parse(value, MESSAGING_SOURCE_MODES, "rest")?;
    match DeliverySourceMode::from_token(initialization.source_mode()) {
        DeliverySourceMode::Module => Ok(MessagingOperationAdapter::Module {
            transport: ModuleTransportKind::Module,
            store_peer_addr: None,
        }),
        DeliverySourceMode::LogoscoreCli => Ok(MessagingOperationAdapter::Module {
            transport: ModuleTransportKind::LogoscoreCli,
            store_peer_addr: initialization
                .input("store_peer_addr")
                .map(ToOwned::to_owned),
        }),
        DeliverySourceMode::Rest => Ok(MessagingOperationAdapter::Rest {
            endpoint: initialization
                .input("rest_endpoint")
                .context("Waku REST URL is required")?
                .to_owned(),
        }),
        DeliverySourceMode::Metrics => {
            bail!("Delivery message actions require delivery REST or module source, not metrics")
        }
        DeliverySourceMode::NetworkMonitor => bail!(
            "Delivery message actions require delivery REST or module source, not network monitor"
        ),
        DeliverySourceMode::Unsupported => bail!(
            "delivery source mode `{}` is not supported",
            initialization.source_mode()
        ),
    }
}

fn require_module_adapter(adapter: &MessagingOperationAdapter) -> Result<()> {
    if matches!(adapter, MessagingOperationAdapter::Module { .. }) {
        return Ok(());
    }
    bail!("delivery node lifecycle actions require delivery module source")
}

fn module_transport_kind(adapter: &MessagingOperationAdapter) -> Result<ModuleTransportKind> {
    match adapter {
        MessagingOperationAdapter::Module { transport, .. } => Ok(*transport),
        MessagingOperationAdapter::Rest { .. } => {
            bail!("delivery node lifecycle actions require delivery module source")
        }
    }
}

fn required_text(value: String, label: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} is required")
    }
    Ok(value.to_owned())
}

fn optional_text(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn new_delivery_store_request_id() -> Result<String> {
    let mut random = [0_u8; DELIVERY_STORE_REQUEST_ID_BYTES];
    getrandom::fill(&mut random).context("failed to generate Delivery Store request ID")?;
    Ok(format!("delivery-store-{}", hex::encode(random)))
}

fn delivery_store_query_request(
    request_id: &str,
    content_topics: Option<&str>,
    pubsub_topic: Option<&str>,
    cursor: Option<&str>,
    page_size: u64,
    ascending: bool,
    include_data: bool,
) -> Value {
    let mut query = serde_json::Map::from_iter([
        ("requestId".to_owned(), json!(request_id)),
        ("includeData".to_owned(), json!(include_data)),
        ("paginationForward".to_owned(), json!(ascending)),
        ("paginationLimit".to_owned(), json!(page_size)),
    ]);
    if let Some(content_topics) = content_topics {
        query.insert("contentTopics".to_owned(), json!([content_topics]));
    }
    if let Some(pubsub_topic) = pubsub_topic {
        query.insert("pubsubTopic".to_owned(), json!(pubsub_topic));
    }
    if let Some(cursor) = cursor {
        query.insert("paginationCursor".to_owned(), json!(cursor));
    }
    Value::Object(query)
}

fn context_map(values: &[(&'static str, String)]) -> Map<String, Value> {
    values
        .iter()
        .map(|(key, value)| ((*key).to_owned(), json!(value)))
        .collect()
}

const fn default_page_size() -> u64 {
    20
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use serde_json::json;

    use super::*;

    fn request(value: Value) -> Result<NodeOperationRequest> {
        NodeOperationRequest::from_value(&value)
    }

    #[test]
    fn module_send_plan_enables_legacy_mutating_flag() -> Result<()> {
        let request = request(json!({
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": false,
            "payload": { "topic": "/topic", "payload": "hello" }
        }))?;

        anyhow::ensure!(
            request.mutating_enabled(),
            "legacy Delivery request was not enabled"
        );

        let request = DeliveryOperationRequest::parse(&request, DeliveryOperation::Send)?;

        let expected = DeliveryOperationPlan::Module {
            transport: ModuleTransportKind::Module,
            method: "send",
            args: vec![json!("/topic"), json!("hello")],
            context: vec![
                ("contentTopic", "/topic".to_owned()),
                ("bytes", "5".to_owned()),
            ],
            dispatch: true,
        };
        anyhow::ensure!(request.plan == expected, "unexpected Messaging send plan");
        Ok(())
    }

    #[test]
    fn logoscore_cli_send_plan_preserves_cli_transport() -> Result<()> {
        let request = request(json!({
            "adapter": { "source_mode": "logoscore_cli", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "topic": "/topic", "payload": "hello" }
        }))?;

        let request = DeliveryOperationRequest::parse(&request, DeliveryOperation::Send)?;

        anyhow::ensure!(
            matches!(
                request.plan,
                DeliveryOperationPlan::Module {
                    transport: ModuleTransportKind::LogoscoreCli,
                    ..
                }
            ),
            "Messaging LogosCore CLI plan lost transport identity"
        );
        Ok(())
    }

    #[test]
    fn module_send_uses_request_identity_only() -> Result<()> {
        let outcome = delivery_module_dispatch_outcome(
            "send",
            ModuleDispatchReceipt::new(
                json!({ "dispatched": true }),
                &json!("request-1"),
                ModuleDispatchIdentityRole::Request,
            )
            .with_bridge_callback(crate::source_routing::BridgeCallbackId::new(41)),
        )?;

        let NodeOperationOutcome::Accepted(acceptance) = outcome else {
            anyhow::bail!("module send was not accepted");
        };
        anyhow::ensure!(
            acceptance.correlation().request_id().map(|id| id.as_str()) == Some("request-1")
                && acceptance.correlation().session_id().is_none()
                && acceptance
                    .correlation()
                    .bridge_callback_id()
                    .map(crate::source_routing::BridgeCallbackId::value)
                    == Some(41)
                && acceptance.terminal_event().correlation()
                    == &ModuleEventCorrelationKind::Request,
            "Delivery request identity role drifted"
        );
        Ok(())
    }

    #[test]
    fn uncorrelated_delivery_lifecycle_is_dispatched() -> Result<()> {
        let outcome = delivery_module_dispatch_outcome(
            "start",
            ModuleDispatchReceipt::new(
                json!({ "dispatched": true }),
                &json!(true),
                ModuleDispatchIdentityRole::None,
            ),
        )?;

        anyhow::ensure!(matches!(outcome, NodeOperationOutcome::Dispatched(_)));
        Ok(())
    }

    #[test]
    fn observable_delivery_dispatch_rejects_missing_request_identity() -> Result<()> {
        let Err(error) = delivery_module_dispatch_outcome(
            "send",
            ModuleDispatchReceipt::new(
                json!({ "dispatched": true }),
                &Value::Null,
                ModuleDispatchIdentityRole::Request,
            ),
        ) else {
            anyhow::bail!("observable delivery dispatch accepted no correlation identity");
        };

        anyhow::ensure!(error.to_string() == "delivery module `send` returned no request ID");
        Ok(())
    }

    #[test]
    fn rest_store_plan_caps_page_size() -> Result<()> {
        let request = request(json!({
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://delivery" }
            },
            "payload": {
                "content_topics": "/topic",
                "page_size": 1000,
                "ascending": true,
                "include_data": false
            }
        }))?;

        let request = DeliveryOperationRequest::parse(&request, DeliveryOperation::StoreQuery)?;

        anyhow::ensure!(
            matches!(
                request.plan,
                DeliveryOperationPlan::Rest(DeliveryRestOperation::StoreQuery {
                    page_size: MAX_STORE_PAGE_SIZE,
                    ..
                })
            ),
            "Messaging Store page size was not capped"
        );
        Ok(())
    }

    #[test]
    fn cli_store_plan_uses_configured_provider_and_compatible_query_shape() -> Result<()> {
        let provider = "/dns4/provider.example/tcp/30303/p2p/peer";
        let request = request(json!({
            "adapter": {
                "source_mode": "logoscore_cli",
                "inputs": { "store_peer_addr": provider }
            },
            "payload": {
                "content_topics": "/topic",
                "pubsub_topic": "/waku/2/rs/16/32",
                "cursor": "cursor-1",
                "page_size": 1_000,
                "ascending": true,
                "include_data": true
            }
        }))?;

        let request = DeliveryOperationRequest::parse(&request, DeliveryOperation::StoreQuery)?;
        let DeliveryOperationPlan::ModuleStoreQuery {
            transport,
            peer_addr,
            query,
            page_size,
            include_data,
        } = &request.plan
        else {
            anyhow::bail!("LogosCore CLI Store request did not produce a module plan");
        };

        anyhow::ensure!(
            *transport == ModuleTransportKind::LogoscoreCli
                && peer_addr == provider
                && *page_size == MAX_STORE_PAGE_SIZE
                && *include_data
                && query
                    .get("requestId")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value.starts_with("delivery-store-"))
                && query.get("contentTopics") == Some(&json!(["/topic"]))
                && query.get("pubsubTopic") == Some(&json!("/waku/2/rs/16/32"))
                && query.get("paginationCursor") == Some(&json!("cursor-1"))
                && query.get("paginationForward") == Some(&json!(true))
                && query.get("paginationLimit") == Some(&json!(MAX_STORE_PAGE_SIZE)),
            "LogosCore CLI Store plan drifted: {query}"
        );
        anyhow::ensure!(
            request.context().get("storePeer") == Some(&json!(provider)),
            "LogosCore CLI Store plan lost provider context"
        );
        Ok(())
    }

    #[test]
    fn cli_store_plan_prefers_payload_provider_and_rejects_missing_provider() -> Result<()> {
        let payload_provider = "/dns4/payload.example/tcp/30303/p2p/peer";
        let operation_request = request(json!({
            "adapter": {
                "source_mode": "logoscore_cli",
                "inputs": { "store_peer_addr": "/dns4/configured.example/tcp/30303/p2p/peer" }
            },
            "payload": {
                "peer_addr": payload_provider,
                "content_topics": "/topic"
            }
        }))?;
        let parsed =
            DeliveryOperationRequest::parse(&operation_request, DeliveryOperation::StoreQuery)?;
        let DeliveryOperationPlan::ModuleStoreQuery { peer_addr, .. } = &parsed.plan else {
            anyhow::bail!("LogosCore CLI Store request did not produce a module plan");
        };
        anyhow::ensure!(
            peer_addr == payload_provider,
            "payload provider did not override config"
        );

        let missing_provider = request(json!({
            "adapter": { "source_mode": "logoscore_cli", "inputs": {} },
            "payload": { "content_topics": "/topic" }
        }))?;
        let Err(error) =
            DeliveryOperationRequest::parse(&missing_provider, DeliveryOperation::StoreQuery)
        else {
            anyhow::bail!("LogosCore CLI Store query accepted no provider");
        };
        anyhow::ensure!(
            error
                .to_string()
                .contains("Store provider multiaddress is required"),
            "unexpected missing provider error: {error:#}"
        );
        Ok(())
    }

    #[test]
    fn basecamp_store_plan_remains_unsupported() -> Result<()> {
        let request = request(json!({
            "adapter": { "source_mode": "module", "inputs": {} },
            "payload": { "content_topics": "/topic" }
        }))?;
        let Err(error) = DeliveryOperationRequest::parse(&request, DeliveryOperation::StoreQuery)
        else {
            anyhow::bail!("Basecamp Delivery unexpectedly accepted Store query");
        };
        anyhow::ensure!(
            error
                .to_string()
                .contains("requires LogosCore CLI Delivery source"),
            "unexpected Basecamp Store error: {error:#}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn cli_store_execution_uses_typed_module_arguments() -> Result<()> {
        let provider = "/dns4/provider.example/tcp/30303/p2p/peer";
        let query =
            delivery_store_query_request("request-1", Some("/topic"), None, None, 20, true, true);
        let transport = std::sync::Arc::new(FakeDeliveryModuleTransport {
            value: json!({ "requestId": "request-1", "statusCode": 200, "messages": [] }),
            last_call: std::sync::Mutex::new(None),
        });
        let outcome = execute_plan(
            DeliveryOperationPlan::ModuleStoreQuery {
                transport: ModuleTransportKind::LogoscoreCli,
                peer_addr: provider.to_owned(),
                query: query.clone(),
                page_size: 20,
                include_data: true,
            },
            transport.clone(),
        )
        .await?;

        let call = transport
            .last_call
            .lock()
            .map_err(|error| {
                anyhow::anyhow!("Delivery module call recording lock failed: {error}")
            })?
            .clone()
            .context("CLI Store query did not invoke Delivery module")?;
        anyhow::ensure!(
            call.transport() == ModuleTransportKind::LogoscoreCli
                && call.module() == "delivery_module"
                && call.method() == "storeQuery"
                && call.args()
                    == [
                        json!(query.to_string()),
                        json!(provider),
                        json!(DELIVERY_STORE_TIMEOUT_MS),
                    ],
            "CLI Store call arguments drifted: {:?}",
            call.args()
        );
        let NodeOperationOutcome::Completed(value) = outcome else {
            anyhow::bail!("CLI Store query did not complete synchronously");
        };
        anyhow::ensure!(
            value.get("storePeer") == Some(&json!(provider))
                && value.pointer("/value/statusCode") == Some(&json!(200)),
            "CLI Store response projection drifted: {value}"
        );
        Ok(())
    }

    #[test]
    fn lifecycle_plan_rejects_rest_adapter() -> Result<()> {
        let request = request(json!({
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://delivery" }
            },
            "mutating_enabled": true,
            "payload": {}
        }))?;

        let Err(error) = DeliveryOperationRequest::parse(&request, DeliveryOperation::Start) else {
            anyhow::bail!("REST Messaging lifecycle was accepted");
        };
        anyhow::ensure!(
            error.to_string().contains("require delivery module source"),
            "unexpected Messaging lifecycle error: {error:#}"
        );
        Ok(())
    }
}
