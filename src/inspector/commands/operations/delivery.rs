use anyhow::{Result, bail};
use serde_json::Value;

use crate::source_routing::messaging_layer;

use super::RuntimeOperationRequest;
use super::spec::{OperationClass, OperationDefinition, OperationDomain, OperationMethod};

pub(super) const OPERATION_DEFINITIONS: &[OperationDefinition] = &[
    OperationDefinition::new(
        OperationMethod::DeliverySubscribe,
        "deliverySubscribe",
        OperationDomain::Delivery,
        "Delivery subscribe",
        OperationClass::Mutating,
    )
    .with_context_inputs(&["source", "endpoint"]),
    OperationDefinition::new(
        OperationMethod::DeliveryUnsubscribe,
        "deliveryUnsubscribe",
        OperationDomain::Delivery,
        "Delivery unsubscribe",
        OperationClass::Mutating,
    )
    .with_context_inputs(&["source", "endpoint"]),
    OperationDefinition::new(
        OperationMethod::DeliverySend,
        "deliverySend",
        OperationDomain::Delivery,
        "Delivery send",
        OperationClass::Mutating,
    )
    .with_context_inputs(&["source", "endpoint"]),
    OperationDefinition::new(
        OperationMethod::DeliveryCreateNode,
        "deliveryCreateNode",
        OperationDomain::Delivery,
        "Delivery create node",
        OperationClass::Lifecycle,
    )
    .with_context_inputs(&["source", "endpoint"]),
    OperationDefinition::new(
        OperationMethod::DeliveryStart,
        "deliveryStart",
        OperationDomain::Delivery,
        "Delivery start",
        OperationClass::Lifecycle,
    )
    .with_context_inputs(&["source", "endpoint"]),
    OperationDefinition::new(
        OperationMethod::DeliveryStop,
        "deliveryStop",
        OperationDomain::Delivery,
        "Delivery stop",
        OperationClass::Lifecycle,
    )
    .with_context_inputs(&["source", "endpoint"]),
    OperationDefinition::new(
        OperationMethod::DeliveryStoreQuery,
        "deliveryStoreQuery",
        OperationDomain::Delivery,
        "Delivery store query",
        OperationClass::ReadPoll,
    )
    .with_context_inputs(&["source", "endpoint"]),
];

pub(super) async fn execute(request: &RuntimeOperationRequest) -> Result<Value> {
    let operation = delivery_operation(request)?;
    let request =
        messaging_layer::DeliveryOperationRequest::parse(request.node_request()?, operation)?;
    messaging_layer::execute_operation(request).await
}

pub(super) fn add_operation_context(
    request: &RuntimeOperationRequest,
    context: &mut serde_json::Map<String, Value>,
) {
    let Ok(operation) = delivery_operation(request) else {
        return;
    };
    if let Ok(node_request) = request.node_request()
        && let Ok(operation_request) =
            messaging_layer::DeliveryOperationRequest::parse(node_request, operation)
    {
        context.extend(operation_request.context().clone());
    }
}

pub(super) fn validate(request: &RuntimeOperationRequest) -> Result<()> {
    let operation = delivery_operation(request)?;
    messaging_layer::DeliveryOperationRequest::parse(request.node_request()?, operation).map(|_| ())
}

fn delivery_operation(
    request: &RuntimeOperationRequest,
) -> Result<messaging_layer::DeliveryOperation> {
    let operation = match request.method() {
        OperationMethod::DeliverySubscribe => messaging_layer::DeliveryOperation::Subscribe,
        OperationMethod::DeliveryUnsubscribe => messaging_layer::DeliveryOperation::Unsubscribe,
        OperationMethod::DeliverySend => messaging_layer::DeliveryOperation::Send,
        OperationMethod::DeliveryCreateNode => messaging_layer::DeliveryOperation::CreateNode,
        OperationMethod::DeliveryStart => messaging_layer::DeliveryOperation::Start,
        OperationMethod::DeliveryStop => messaging_layer::DeliveryOperation::Stop,
        OperationMethod::DeliveryStoreQuery => messaging_layer::DeliveryOperation::StoreQuery,
        _ => bail!("`{}` is not a Delivery operation", request.method_name()),
    };
    Ok(operation)
}
