use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};

use super::request::RuntimeOperationRequest;
use super::spec::{AffectedContextField, ContextPresence, OperationClass, RestartPolicy};
#[cfg(test)]
use super::{
    request::runtime_operation_context,
    spec::{AffectedContextKey, OperationMethod},
};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct RuntimeOperationPolicy {
    class: OperationClass,
    affected_inputs: Vec<AffectedInput>,
    restart_policy: RestartPolicy,
    confirmation_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AffectedInput {
    key: &'static str,
    value: String,
}

impl RuntimeOperationPolicy {
    pub(super) fn from_request(request: &RuntimeOperationRequest, context: &Value) -> Result<Self> {
        let definition = request.policy_definition();
        Ok(Self {
            class: definition.class(),
            affected_inputs: affected_inputs(
                request,
                context,
                definition.affected_context_fields(),
            )?,
            restart_policy: definition.restart_policy(),
            confirmation_required: definition.confirmation_required(),
        })
    }

    pub(super) fn as_value(&self) -> Value {
        json!({
            "operationClass": self.class.as_str(),
            "affectedInputs": self.affected_inputs.iter().map(AffectedInput::as_value).collect::<Vec<_>>(),
            "restartPolicy": self.restart_policy.as_str(),
            "confirmationRequired": self.confirmation_required,
            "provenance": ["runtime_operation_policy"],
        })
    }

    pub(super) fn safe_to_restart(&self) -> bool {
        self.restart_policy == RestartPolicy::SafeReadPolling
    }

    pub(super) fn class_name(&self) -> &'static str {
        self.class.as_str()
    }

    pub(super) fn restart_policy_name(&self) -> &'static str {
        self.restart_policy.as_str()
    }

    pub(super) fn affected_inputs_value(&self) -> Value {
        Value::Array(
            self.affected_inputs
                .iter()
                .map(AffectedInput::as_value)
                .collect(),
        )
    }
}

impl AffectedInput {
    fn as_value(&self) -> Value {
        json!({
            "key": self.key,
            "value": self.value,
        })
    }
}

fn affected_inputs(
    request: &RuntimeOperationRequest,
    context: &Value,
    affected_context_fields: &[AffectedContextField],
) -> Result<Vec<AffectedInput>> {
    let mut inputs = Vec::new();
    push_input(&mut inputs, "domain", request.domain_name());
    push_input(&mut inputs, "method", request.method_name());
    let context = context
        .as_object()
        .context("runtime operation context must be a JSON object")?;
    for &field in affected_context_fields {
        let key = field.key().as_str();
        let Some(value) = context.get(key) else {
            if field.presence() == ContextPresence::Optional {
                continue;
            }
            bail!("runtime operation context is missing required affected input `{key}`");
        };
        let value = value.as_str().with_context(|| {
            format!("runtime operation affected input `{key}` must be a string")
        })?;
        if value.trim().is_empty() {
            bail!("runtime operation affected input `{key}` must not be empty");
        }
        push_input(&mut inputs, key, value);
    }
    Ok(inputs)
}

fn push_input(inputs: &mut Vec<AffectedInput>, key: &'static str, value: &str) {
    let trimmed = value.trim();
    if !trimmed.is_empty() {
        inputs.push(AffectedInput {
            key,
            value: trimmed.to_owned(),
        });
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{Context as _, Result, bail};
    use serde_json::json;

    use super::*;
    use crate::inspector::commands::operations::runtime_operation_request_from_value;

    #[test]
    fn policy_marks_read_operations_as_safe_restartable() -> Result<()> {
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageManifests",
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://storage.local" }
            },
            "payload": {}
        }))?;

        let context = runtime_operation_context(&request)?;
        let policy = RuntimeOperationPolicy::from_request(&request, &context)?.as_value();

        if policy.get("operationClass").and_then(Value::as_str) != Some("read_poll")
            || policy.get("restartPolicy").and_then(Value::as_str) != Some("safe_read_polling")
            || policy.get("confirmationRequired").and_then(Value::as_bool) != Some(false)
        {
            bail!("unexpected read policy: {policy}");
        }
        Ok(())
    }

    #[test]
    fn policy_marks_mutating_storage_operations_manual_with_inputs() -> Result<()> {
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageUploadUrl",
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://storage.local" }
            },
            "mutating_enabled": true,
            "payload": { "path": "/tmp/file.bin" }
        }))?;

        let context = runtime_operation_context(&request)?;
        let policy = RuntimeOperationPolicy::from_request(&request, &context)?.as_value();

        if policy.get("operationClass").and_then(Value::as_str) != Some("mutating")
            || policy.get("restartPolicy").and_then(Value::as_str) != Some("manual_required")
            || policy.get("confirmationRequired").and_then(Value::as_bool) != Some(true)
        {
            bail!("unexpected mutating policy: {policy}");
        }
        let inputs = policy
            .get("affectedInputs")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if !inputs
            .iter()
            .any(|input| input.get("key").and_then(Value::as_str) == Some("path"))
        {
            bail!("storage upload policy should include path input: {policy}");
        }
        Ok(())
    }

    #[test]
    fn policy_marks_wallet_submission_operations_manual() -> Result<()> {
        let request = RuntimeOperationRequest::from_call(
            OperationMethod::LocalWalletInstructionSubmit,
            json!([
                {},
                {},
                {
                    "network_scope": {
                        "kind": "genesis_id",
                        "genesis_id": "11".repeat(32)
                    },
                    "channel_id": "22".repeat(32),
                    "source_id": "src_verified",
                    "source_config_revision": 7,
                    "context_revision": 9,
                    "request_revision": 11,
                    "endpoint": "https://sequencer.example.test/"
                },
                "confirm-idl-instruction"
            ]),
            "IDL instruction",
        )?;
        let context = runtime_operation_context(&request)?;
        let policy = RuntimeOperationPolicy::from_request(&request, &context)?.as_value();

        if policy.get("operationClass").and_then(Value::as_str) != Some("signing_submission")
            || policy.get("restartPolicy").and_then(Value::as_str) != Some("manual_required")
        {
            bail!("unexpected wallet submission policy: {policy}");
        }
        let inputs = policy
            .get("affectedInputs")
            .and_then(Value::as_array)
            .context("wallet submission affected inputs must be an array")?;
        for (key, value) in [
            ("source", "src_verified"),
            ("endpoint", "https://sequencer.example.test/"),
        ] {
            if !inputs.iter().any(|input| {
                input.get("key").and_then(Value::as_str) == Some(key)
                    && input.get("value").and_then(Value::as_str) == Some(value)
            }) {
                bail!("wallet submission policy omitted verified {key}: {policy}");
            }
        }
        Ok(())
    }

    #[test]
    fn optional_affected_context_absence_is_intentional() -> Result<()> {
        let request = runtime_operation_request_from_value(json!({
            "domain": "delivery",
            "method": "deliverySend",
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "topic": "/topic", "payload": "hello" }
        }))?;
        let context = runtime_operation_context(&request)?;
        let policy = RuntimeOperationPolicy::from_request(&request, &context)?.as_value();
        let inputs = policy
            .get("affectedInputs")
            .and_then(Value::as_array)
            .context("affectedInputs must be an array")?;

        if inputs
            .iter()
            .any(|input| input.get("key").and_then(Value::as_str) == Some("endpoint"))
            || !inputs
                .iter()
                .any(|input| input.get("key").and_then(Value::as_str) == Some("source"))
        {
            bail!("optional endpoint contract drifted: {policy}");
        }
        Ok(())
    }

    #[test]
    fn required_affected_context_absence_is_rejected_for_every_key() -> Result<()> {
        let request = RuntimeOperationRequest::from_call(
            OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        for key in [
            AffectedContextKey::Source,
            AffectedContextKey::Endpoint,
            AffectedContextKey::Cid,
            AffectedContextKey::Path,
            AffectedContextKey::Filename,
            AffectedContextKey::BackupCatalogId,
            AffectedContextKey::DownloadScope,
            AffectedContextKey::SlotRange,
            AffectedContextKey::BlockId,
            AffectedContextKey::TransactionId,
        ] {
            let result =
                affected_inputs(&request, &json!({}), &[AffectedContextField::required(key)]);

            let Err(error) = result else {
                bail!("missing required affected context should fail for {key:?}");
            };
            let expected = format!("missing required affected input `{}`", key.as_str());
            if !error.to_string().contains(&expected) {
                bail!("unexpected required-context error for {key:?}: {error:#}");
            }
        }
        Ok(())
    }

    #[test]
    fn present_optional_affected_context_must_be_valid() -> Result<()> {
        let request = RuntimeOperationRequest::from_call(
            OperationMethod::LocalWalletAccounts,
            json!(["default"]),
            "Wallet accounts",
        )?;
        let result = affected_inputs(
            &request,
            &json!({ "endpoint": 42 }),
            &[AffectedContextField::optional(AffectedContextKey::Endpoint)],
        );

        let Err(error) = result else {
            bail!("invalid optional affected context should fail");
        };
        if !error
            .to_string()
            .contains("affected input `endpoint` must be a string")
        {
            bail!("unexpected optional-context error: {error:#}");
        }
        Ok(())
    }
}
