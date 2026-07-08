use anyhow::{Context as _, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use reqwest::Method;
use serde_json::{Value, json};

use crate::source_routing::{
    self, Args, DeliveryStoreQuery, delivery_rest_source, delivery_store_query_url,
    raw_http_json_url, require_mutating_diagnostics, rest_empty_request,
};

use super::{NodeOperationRequest, blocking_module_call, blocking_module_dispatch};

const MAX_DELIVERY_STORE_PAGE_SIZE: u64 = 100;

pub(super) async fn execute_delivery_subscription(
    request: &NodeOperationRequest,
    method: Method,
    module_method: &'static str,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if let Some(module_args) =
        source_routing::delivery_message_args(&args, "delivery message action")?
    {
        return blocking_module_call(
            "delivery module message action",
            source_routing::DELIVERY_MODULE,
            module_method,
            module_args.values,
        )
        .await;
    }
    let source = delivery_rest_source(&args)?;
    require_mutating_diagnostics(&args, source.next_index, "delivery message action")?;
    let topic = args.string(source.next_index + 1, "content topic")?;
    rest_empty_request(
        method.clone(),
        source.endpoint,
        "/relay/v1/auto/subscriptions",
        Some(json!([topic])),
    )
    .await
    .with_context(|| format!("failed to update relay subscription for {topic}"))?;
    Ok(json!({
        "subscribed": method == Method::POST,
        "contentTopic": topic,
        "endpoint": source.endpoint,
    }))
}

pub(super) async fn execute_delivery_send(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if let Some(module_args) =
        source_routing::delivery_message_args(&args, "delivery message action")?
    {
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
        return blocking_module_dispatch(
            "delivery module send",
            source_routing::DELIVERY_MODULE,
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
    let body = json!({
        "contentTopic": topic,
        "payload": BASE64_STANDARD.encode(payload.as_bytes()),
    });
    rest_empty_request(
        Method::POST,
        source.endpoint,
        "/relay/v1/auto/messages",
        Some(body),
    )
    .await
    .with_context(|| format!("failed to send relay message on {topic}"))?;
    Ok(json!({
        "sent": true,
        "contentTopic": topic,
        "bytes": payload.len(),
        "endpoint": source.endpoint,
    }))
}

pub(super) async fn execute_delivery_module_action(
    request: &NodeOperationRequest,
    method: &'static str,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let call_args =
        source_routing::delivery_lifecycle_args(&args, "delivery node lifecycle action")?;
    blocking_module_call(
        "delivery module node action",
        source_routing::DELIVERY_MODULE,
        method,
        call_args,
    )
    .await
}

pub(super) async fn execute_delivery_store_query(request: &NodeOperationRequest) -> Result<Value> {
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
    let query = delivery_store_query_url(
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
    )?;
    let value = raw_http_json_url(query.as_str())
        .await
        .context("failed to query Delivery Store")?;
    Ok(json!({
        "endpoint": source.endpoint,
        "includeData": include_data,
        "pageSize": page_size,
        "query": query.as_str(),
        "value": value,
    }))
}
