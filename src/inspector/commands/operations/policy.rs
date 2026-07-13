use serde_json::{Value, json};

use super::request::{RuntimeOperationRequest, runtime_operation_context};
use super::spec::{OperationClass, RestartPolicy};
#[cfg(test)]
use super::spec::{OperationMethod, OperationPolicyDefinition};

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
    pub(super) fn from_request(request: &RuntimeOperationRequest) -> Self {
        let definition = request.policy_definition();
        Self {
            class: definition.class(),
            affected_inputs: affected_inputs(request, definition.affected_context_keys()),
            restart_policy: definition.restart_policy(),
            confirmation_required: definition.confirmation_required(),
        }
    }

    #[cfg(test)]
    pub(super) fn from_method(domain: &str, method: &str) -> Self {
        let definition = OperationMethod::from_str(method)
            .and_then(OperationMethod::definition)
            .map(|definition| definition.policy())
            .unwrap_or_else(|| OperationPolicyDefinition::new(OperationClass::ReadPoll));
        let mut affected_inputs = Vec::new();
        push_input(&mut affected_inputs, "domain", domain);
        push_input(&mut affected_inputs, "method", method);
        Self {
            class: definition.class(),
            affected_inputs,
            restart_policy: definition.restart_policy(),
            confirmation_required: definition.confirmation_required(),
        }
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
    affected_context_keys: &[&'static str],
) -> Vec<AffectedInput> {
    let mut inputs = Vec::new();
    push_input(&mut inputs, "domain", request.domain_name());
    push_input(&mut inputs, "method", request.method_name());
    if let Value::Object(context) = runtime_operation_context(request) {
        for &key in affected_context_keys {
            if let Some(value) = context.get(key).and_then(Value::as_str) {
                push_input(&mut inputs, key, value);
            }
        }
    }
    inputs
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

        let policy = RuntimeOperationPolicy::from_request(&request).as_value();

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

        let policy = RuntimeOperationPolicy::from_request(&request).as_value();

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
        let policy =
            RuntimeOperationPolicy::from_method("execution", "localWalletInstructionSubmit")
                .as_value();

        if policy.get("operationClass").and_then(Value::as_str) != Some("signing_submission")
            || policy.get("restartPolicy").and_then(Value::as_str) != Some("manual_required")
        {
            bail!("unexpected wallet submission policy: {policy}");
        }
        Ok(())
    }

    #[test]
    fn policy_projection_matches_every_operation_definition() -> Result<()> {
        for &method in OperationMethod::ALL {
            let definition = method
                .definition()
                .with_context(|| format!("definition missing for {method:?}"))?;
            let expected = definition.policy();

            let policy = RuntimeOperationPolicy::from_method(
                definition.domain().as_str(),
                definition.name(),
            )
            .as_value();

            if policy.get("operationClass").and_then(Value::as_str)
                != Some(expected.class().as_str())
                || policy.get("restartPolicy").and_then(Value::as_str)
                    != Some(expected.restart_policy().as_str())
                || policy.get("confirmationRequired").and_then(Value::as_bool)
                    != Some(expected.confirmation_required())
                || policy.get("provenance") != Some(&json!(["runtime_operation_policy"]))
            {
                bail!("policy projection drifted for {method:?}: {policy}");
            }
            let affected_inputs = policy
                .get("affectedInputs")
                .and_then(Value::as_array)
                .context("affectedInputs must be an array")?;
            if affected_inputs.first().and_then(|input| input.get("key")) != Some(&json!("domain"))
                || affected_inputs.get(1).and_then(|input| input.get("key"))
                    != Some(&json!("method"))
            {
                bail!("base affected inputs drifted for {method:?}: {policy}");
            }
        }

        Ok(())
    }
}
