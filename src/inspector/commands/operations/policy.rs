use serde_json::{Value, json};

use super::request::{RuntimeOperationRequest, runtime_operation_context};
use super::spec::OperationMethod;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OperationClass {
    Backup,
    Destructive,
    Lifecycle,
    Mutating,
    ReadPoll,
    SigningSubmission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RestartPolicy {
    ManualRequired,
    SafeReadPolling,
}

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
        let class = operation_class(request.method(), &request.domain);
        Self {
            class,
            affected_inputs: affected_inputs(request),
            restart_policy: restart_policy(class),
            confirmation_required: confirmation_required(class),
        }
    }

    #[cfg(test)]
    pub(super) fn from_method(domain: &str, method: &str) -> Self {
        let method = OperationMethod::from_str(method);
        let class = method
            .map(|method| operation_class(method, domain))
            .unwrap_or(OperationClass::ReadPoll);
        Self {
            class,
            affected_inputs: Vec::new(),
            restart_policy: restart_policy(class),
            confirmation_required: confirmation_required(class),
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

impl OperationClass {
    fn as_str(self) -> &'static str {
        match self {
            Self::Backup => "backup",
            Self::Destructive => "destructive",
            Self::Lifecycle => "lifecycle",
            Self::Mutating => "mutating",
            Self::ReadPoll => "read_poll",
            Self::SigningSubmission => "signing_submission",
        }
    }
}

impl RestartPolicy {
    fn as_str(self) -> &'static str {
        match self {
            Self::ManualRequired => "manual_required",
            Self::SafeReadPolling => "safe_read_polling",
        }
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

fn operation_class(method: OperationMethod, domain: &str) -> OperationClass {
    match method {
        OperationMethod::LocalWalletCreateAccount
        | OperationMethod::LocalWalletSendTransaction
        | OperationMethod::LocalWalletCommand
        | OperationMethod::LocalWalletDeployProgram
        | OperationMethod::LocalWalletInstructionSubmit
        | OperationMethod::LocalWalletSyncPrivate => OperationClass::SigningSubmission,
        OperationMethod::LocalNodesAction
        | OperationMethod::DeliveryCreateNode
        | OperationMethod::DeliveryStart
        | OperationMethod::DeliveryStop => OperationClass::Lifecycle,
        OperationMethod::StorageRemove => OperationClass::Destructive,
        OperationMethod::StorageFetch
        | OperationMethod::StorageUploadUrl
        | OperationMethod::StorageDownloadToUrl
        | OperationMethod::DeliverySubscribe
        | OperationMethod::DeliveryUnsubscribe
        | OperationMethod::DeliverySend => OperationClass::Mutating,
        _ if domain == "backup" => OperationClass::Backup,
        _ => OperationClass::ReadPoll,
    }
}

fn restart_policy(class: OperationClass) -> RestartPolicy {
    match class {
        OperationClass::ReadPoll => RestartPolicy::SafeReadPolling,
        OperationClass::Backup
        | OperationClass::Destructive
        | OperationClass::Lifecycle
        | OperationClass::Mutating
        | OperationClass::SigningSubmission => RestartPolicy::ManualRequired,
    }
}

fn confirmation_required(class: OperationClass) -> bool {
    !matches!(class, OperationClass::ReadPoll)
}

fn affected_inputs(request: &RuntimeOperationRequest) -> Vec<AffectedInput> {
    let mut inputs = Vec::new();
    push_input(&mut inputs, "domain", &request.domain);
    push_input(&mut inputs, "method", request.method_name());
    if let Value::Object(context) = runtime_operation_context(request) {
        for key in ["source", "endpoint", "cid", "path", "module"] {
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
    use anyhow::{Result, bail};
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
}
