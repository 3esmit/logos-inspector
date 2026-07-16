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
    Module(ModuleTransportKind),
    Rest { endpoint: String },
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
                MessagingOperationAdapter::Module(transport) => Ok((
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
                MessagingOperationAdapter::Module(transport) => Ok((
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
            let MessagingOperationAdapter::Rest { endpoint } = adapter else {
                bail!("Delivery Store query requires delivery REST source")
            };
            let payload: StoreQueryPayload = request.payload("Delivery Store query")?;
            let content_topics = optional_text(payload.content_topics);
            let mut context = Map::new();
            if let Some(topic) = &content_topics {
                context.insert("contentTopic".to_owned(), json!(topic));
            }
            context.insert("endpoint".to_owned(), json!(endpoint));
            Ok((
                DeliveryOperationPlan::Rest(DeliveryRestOperation::StoreQuery {
                    endpoint,
                    peer_addr: optional_text(payload.peer_addr),
                    content_topics,
                    pubsub_topic: optional_text(payload.pubsub_topic),
                    cursor: optional_text(payload.cursor),
                    page_size: payload.page_size.clamp(1, MAX_STORE_PAGE_SIZE),
                    ascending: payload.ascending,
                    include_data: payload.include_data,
                }),
                context,
            ))
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
    let transport: SharedModuleTransport =
        std::sync::Arc::new(FakeDeliveryModuleTransport { value });
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
}

#[cfg(test)]
impl crate::modules::logos_core::ModuleTransport for FakeDeliveryModuleTransport {
    fn kind(&self) -> ModuleTransportKind {
        ModuleTransportKind::LogoscoreCli
    }

    fn call(
        &self,
        _call: crate::modules::logos_core::ModuleCall,
    ) -> crate::modules::logos_core::ModuleCallFuture<'_> {
        Box::pin(async move {
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
        DeliverySourceMode::Module => Ok(MessagingOperationAdapter::Module(
            ModuleTransportKind::Module,
        )),
        DeliverySourceMode::LogoscoreCli => Ok(MessagingOperationAdapter::Module(
            ModuleTransportKind::LogoscoreCli,
        )),
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
    if matches!(adapter, MessagingOperationAdapter::Module(_)) {
        return Ok(());
    }
    bail!("delivery node lifecycle actions require delivery module source")
}

fn module_transport_kind(adapter: &MessagingOperationAdapter) -> Result<ModuleTransportKind> {
    match adapter {
        MessagingOperationAdapter::Module(transport) => Ok(*transport),
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
