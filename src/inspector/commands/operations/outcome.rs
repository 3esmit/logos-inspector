use serde_json::Value;

use crate::source_routing::{NodeOperationOutcome, ObservableOperationAcceptance};

#[derive(Debug, Clone, PartialEq)]
pub(super) enum RuntimeOperationOutcome {
    Completed(Value),
    Accepted(Box<ObservableOperationAcceptance>),
    Dispatched(Value),
}

impl From<NodeOperationOutcome> for RuntimeOperationOutcome {
    fn from(value: NodeOperationOutcome) -> Self {
        match value {
            NodeOperationOutcome::Completed(result) => Self::Completed(result),
            NodeOperationOutcome::Accepted(acceptance) => Self::Accepted(acceptance),
            NodeOperationOutcome::Dispatched(acknowledgement) => Self::Dispatched(acknowledgement),
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use serde_json::json;

    use crate::source_routing::messaging_layer;

    use super::*;

    #[tokio::test]
    async fn production_adapter_mapping_preserves_runtime_outcome_variants() -> Result<()> {
        let completed: RuntimeOperationOutcome = messaging_layer::execute_module_adapter_fixture(
            "subscribe",
            false,
            json!({ "subscribed": true }),
        )
        .await?
        .into();
        let accepted: RuntimeOperationOutcome =
            messaging_layer::execute_module_adapter_fixture("send", true, json!("request-1"))
                .await?
                .into();
        let dispatched: RuntimeOperationOutcome =
            messaging_layer::execute_module_adapter_fixture("start", true, json!(true))
                .await?
                .into();

        anyhow::ensure!(
            completed == RuntimeOperationOutcome::Completed(json!({ "subscribed": true })),
            "module call outcome collapsed before runtime reduction"
        );
        let RuntimeOperationOutcome::Accepted(acceptance) = accepted else {
            anyhow::bail!("observable module dispatch did not remain accepted");
        };
        anyhow::ensure!(
            acceptance
                .correlation()
                .request_id()
                .map(|request_id| request_id.as_str())
                == Some("request-1"),
            "accepted module dispatch lost request correlation"
        );
        let RuntimeOperationOutcome::Dispatched(acknowledgement) = dispatched else {
            anyhow::bail!("unobservable module dispatch did not remain dispatched");
        };
        anyhow::ensure!(
            acknowledgement
                == json!({
                    "module": "delivery_module",
                    "method": "start",
                    "dispatched": true,
                    "value": true,
                }),
            "unobservable module dispatch acknowledgement drifted"
        );
        Ok(())
    }
}
