use anyhow::Result;
use serde_json::Value;

use crate::source_routing::{NodeOperationOutcome, messaging_layer};

use super::RuntimeOperationRequest;
use super::spec::{
    AffectedContextField, AffectedContextKey, OperationClass, OperationCommand,
    OperationDefinition, OperationMethod,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DeliveryCommand {
    Subscribe,
    Unsubscribe,
    Send,
    CreateNode,
    Start,
    Stop,
    StoreQuery,
}

impl DeliveryCommand {
    pub(super) const fn method(self) -> OperationMethod {
        match self {
            Self::Subscribe => OperationMethod::DeliverySubscribe,
            Self::Unsubscribe => OperationMethod::DeliveryUnsubscribe,
            Self::Send => OperationMethod::DeliverySend,
            Self::CreateNode => OperationMethod::DeliveryCreateNode,
            Self::Start => OperationMethod::DeliveryStart,
            Self::Stop => OperationMethod::DeliveryStop,
            Self::StoreQuery => OperationMethod::DeliveryStoreQuery,
        }
    }

    const fn operation(self) -> messaging_layer::DeliveryOperation {
        match self {
            Self::Subscribe => messaging_layer::DeliveryOperation::Subscribe,
            Self::Unsubscribe => messaging_layer::DeliveryOperation::Unsubscribe,
            Self::Send => messaging_layer::DeliveryOperation::Send,
            Self::CreateNode => messaging_layer::DeliveryOperation::CreateNode,
            Self::Start => messaging_layer::DeliveryOperation::Start,
            Self::Stop => messaging_layer::DeliveryOperation::Stop,
            Self::StoreQuery => messaging_layer::DeliveryOperation::StoreQuery,
        }
    }
}

pub(super) const OPERATION_DEFINITIONS: &[OperationDefinition] = &[
    OperationDefinition::new(
        OperationCommand::Delivery(DeliveryCommand::Subscribe),
        "deliverySubscribe",
        "Delivery subscribe",
        OperationClass::Mutating,
    )
    .with_context_inputs(&[
        AffectedContextField::required(AffectedContextKey::Source),
        AffectedContextField::optional(AffectedContextKey::Endpoint),
    ]),
    OperationDefinition::new(
        OperationCommand::Delivery(DeliveryCommand::Unsubscribe),
        "deliveryUnsubscribe",
        "Delivery unsubscribe",
        OperationClass::Mutating,
    )
    .with_context_inputs(&[
        AffectedContextField::required(AffectedContextKey::Source),
        AffectedContextField::optional(AffectedContextKey::Endpoint),
    ]),
    OperationDefinition::new(
        OperationCommand::Delivery(DeliveryCommand::Send),
        "deliverySend",
        "Delivery send",
        OperationClass::Mutating,
    )
    .with_context_inputs(&[
        AffectedContextField::required(AffectedContextKey::Source),
        AffectedContextField::optional(AffectedContextKey::Endpoint),
    ]),
    OperationDefinition::new(
        OperationCommand::Delivery(DeliveryCommand::CreateNode),
        "deliveryCreateNode",
        "Delivery create node",
        OperationClass::Lifecycle,
    )
    .with_context_inputs(&[AffectedContextField::required(AffectedContextKey::Source)]),
    OperationDefinition::new(
        OperationCommand::Delivery(DeliveryCommand::Start),
        "deliveryStart",
        "Delivery start",
        OperationClass::Lifecycle,
    )
    .with_context_inputs(&[AffectedContextField::required(AffectedContextKey::Source)]),
    OperationDefinition::new(
        OperationCommand::Delivery(DeliveryCommand::Stop),
        "deliveryStop",
        "Delivery stop",
        OperationClass::Lifecycle,
    )
    .with_context_inputs(&[AffectedContextField::required(AffectedContextKey::Source)]),
    OperationDefinition::new(
        OperationCommand::Delivery(DeliveryCommand::StoreQuery),
        "deliveryStoreQuery",
        "Delivery store query",
        OperationClass::ReadPoll,
    )
    .with_context_inputs(&[
        AffectedContextField::required(AffectedContextKey::Source),
        AffectedContextField::required(AffectedContextKey::Endpoint),
    ]),
];

pub(super) async fn execute(
    command: DeliveryCommand,
    request: &RuntimeOperationRequest,
) -> Result<NodeOperationOutcome> {
    let request = messaging_layer::DeliveryOperationRequest::parse(
        request.node_request()?,
        command.operation(),
    )?;
    messaging_layer::execute_operation(request).await
}

pub(super) fn add_operation_context(
    command: DeliveryCommand,
    request: &RuntimeOperationRequest,
    context: &mut serde_json::Map<String, Value>,
) -> Result<()> {
    let operation_request = messaging_layer::DeliveryOperationRequest::parse(
        request.node_request()?,
        command.operation(),
    )?;
    context.extend(operation_request.context().clone());
    Ok(())
}

pub(super) fn validate(command: DeliveryCommand, request: &RuntimeOperationRequest) -> Result<()> {
    messaging_layer::DeliveryOperationRequest::parse(request.node_request()?, command.operation())
        .map(|_| ())
}
