use anyhow::{Context as _, Result, bail};
use reqwest::Method;
use serde_json::{Value, json};

use crate::{
    source_routing::{
        DeliveryStoreQuery, delivery_rest_source, messaging_layer, require_mutating_diagnostics,
    },
    support::args::Args,
};

use super::RuntimeOperationRequest;
use super::spec::{OperationDefinition, OperationDomain, OperationMethod};

const MAX_DELIVERY_STORE_PAGE_SIZE: u64 = 100;

pub(super) const OPERATION_DEFINITIONS: &[OperationDefinition] = &[
    OperationDefinition::mutating(
        OperationMethod::DeliverySubscribe,
        "deliverySubscribe",
        OperationDomain::Delivery,
        "Delivery subscribe",
    ),
    OperationDefinition::mutating(
        OperationMethod::DeliveryUnsubscribe,
        "deliveryUnsubscribe",
        OperationDomain::Delivery,
        "Delivery unsubscribe",
    ),
    OperationDefinition::mutating(
        OperationMethod::DeliverySend,
        "deliverySend",
        OperationDomain::Delivery,
        "Delivery send",
    ),
    OperationDefinition::mutating(
        OperationMethod::DeliveryCreateNode,
        "deliveryCreateNode",
        OperationDomain::Delivery,
        "Delivery create node",
    ),
    OperationDefinition::mutating(
        OperationMethod::DeliveryStart,
        "deliveryStart",
        OperationDomain::Delivery,
        "Delivery start",
    ),
    OperationDefinition::mutating(
        OperationMethod::DeliveryStop,
        "deliveryStop",
        OperationDomain::Delivery,
        "Delivery stop",
    ),
    OperationDefinition::new(
        OperationMethod::DeliveryStoreQuery,
        "deliveryStoreQuery",
        OperationDomain::Delivery,
        "Delivery store query",
    ),
];

pub(super) async fn execute(request: &RuntimeOperationRequest) -> Result<Value> {
    match request.method() {
        OperationMethod::DeliverySubscribe => {
            execute_delivery_subscription(request, Method::POST, "subscribe").await
        }
        OperationMethod::DeliveryUnsubscribe => {
            execute_delivery_subscription(request, Method::DELETE, "unsubscribe").await
        }
        OperationMethod::DeliverySend => execute_delivery_send(request).await,
        OperationMethod::DeliveryCreateNode => {
            execute_delivery_module_action(request, "createNode").await
        }
        OperationMethod::DeliveryStart => execute_delivery_module_action(request, "start").await,
        OperationMethod::DeliveryStop => execute_delivery_module_action(request, "stop").await,
        OperationMethod::DeliveryStoreQuery => execute_delivery_store_query(request).await,
        _ => bail!("`{}` is not a Delivery operation", request.method_name()),
    }
}

pub(super) async fn execute_delivery_subscription(
    request: &RuntimeOperationRequest,
    method: Method,
    module_method: &'static str,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if let Some(module_args) = messaging_layer::message_args(&args, "delivery message action")? {
        return messaging_layer::module_call(module_method, module_args.values).await;
    }
    let source = delivery_rest_source(&args)?;
    require_mutating_diagnostics(&args, source.next_index, "delivery message action")?;
    let topic = args.string(source.next_index + 1, "content topic")?;
    messaging_layer::update_subscription(source.endpoint, topic, method == Method::POST)
        .await
        .with_context(|| format!("failed to update relay subscription for {topic}"))
}

pub(super) async fn execute_delivery_send(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if let Some(module_args) = messaging_layer::message_args(&args, "delivery message action")? {
        let topic = module_args
            .values
            .first()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let payload_len = module_args
            .values
            .get(1)
            .and_then(Value::as_str)
            .map(str::len)
            .unwrap_or(0);
        return messaging_layer::module_dispatch(
            "send",
            module_args.values,
            vec![("contentTopic", topic), ("bytes", payload_len.to_string())],
        )
        .await;
    }
    let source = delivery_rest_source(&args)?;
    require_mutating_diagnostics(&args, source.next_index, "delivery message action")?;
    let topic = args.string(source.next_index + 1, "content topic")?;
    let payload = args.string(source.next_index + 2, "message payload")?;
    messaging_layer::send(source.endpoint, topic, payload)
        .await
        .with_context(|| format!("failed to send relay message on {topic}"))
}

pub(super) async fn execute_delivery_module_action(
    request: &RuntimeOperationRequest,
    method: &'static str,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let call_args = messaging_layer::lifecycle_args(&args, "delivery node lifecycle action")?;
    messaging_layer::module_call(method, call_args).await
}

pub(super) async fn execute_delivery_store_query(
    request: &RuntimeOperationRequest,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = delivery_rest_source(&args)?;
    let peer_addr = args.optional_string(source.next_index + 1);
    let content_topics = args.optional_string(source.next_index + 2);
    let pubsub_topic = args.optional_string(source.next_index + 3);
    let cursor = args.optional_string(source.next_index + 4);
    let page_size = args
        .value(source.next_index + 5)
        .and_then(Value::as_u64)
        .unwrap_or(20)
        .clamp(1, MAX_DELIVERY_STORE_PAGE_SIZE);
    let ascending = args.optional_bool(source.next_index + 6);
    let include_data = args.optional_bool(source.next_index + 7);
    let (query, value) = messaging_layer::store_query(
        source.endpoint,
        DeliveryStoreQuery {
            peer_addr,
            content_topics,
            pubsub_topic,
            cursor,
            page_size,
            ascending,
            include_data,
        },
    )
    .await
    .context("failed to query Delivery Store")?;
    Ok(json!({
        "endpoint": source.endpoint,
        "includeData": include_data,
        "pageSize": page_size,
        "query": query,
        "value": value,
    }))
}
