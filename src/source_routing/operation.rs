use std::io;

use serde_json::{Map, Value};

use crate::modules::logos_core::BridgeCallbackId;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ModuleSessionId(String);

impl ModuleSessionId {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        nonempty_text(value).map(Self)
    }

    #[must_use]
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ModuleRequestId(String);

impl ModuleRequestId {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        nonempty_text(value).map(Self)
    }

    #[must_use]
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ModuleEventCorrelationKind {
    Session,
    Request,
}

impl ModuleEventCorrelationKind {
    #[must_use]
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Session => "module_session",
            Self::Request => "module_request",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ModuleCorrelation {
    bridge_callback_id: Option<BridgeCallbackId>,
    session_id: Option<ModuleSessionId>,
    request_id: Option<ModuleRequestId>,
}

impl ModuleCorrelation {
    #[must_use]
    pub(crate) fn with_bridge_callback(mut self, bridge_callback_id: BridgeCallbackId) -> Self {
        self.bridge_callback_id = Some(bridge_callback_id);
        self
    }

    #[must_use]
    pub(crate) fn with_session(session_id: ModuleSessionId) -> Self {
        Self {
            session_id: Some(session_id),
            ..Self::default()
        }
    }

    #[must_use]
    pub(crate) fn with_request(request_id: ModuleRequestId) -> Self {
        Self {
            request_id: Some(request_id),
            ..Self::default()
        }
    }

    #[must_use]
    pub(crate) fn bridge_callback_id(&self) -> Option<&BridgeCallbackId> {
        self.bridge_callback_id.as_ref()
    }

    #[must_use]
    pub(crate) fn session_id(&self) -> Option<&ModuleSessionId> {
        self.session_id.as_ref()
    }

    #[must_use]
    pub(crate) fn request_id(&self) -> Option<&ModuleRequestId> {
        self.request_id.as_ref()
    }

    #[must_use]
    pub(crate) fn identity_for(&self, kind: &ModuleEventCorrelationKind) -> Option<&str> {
        match kind {
            ModuleEventCorrelationKind::Session => self.session_id().map(ModuleSessionId::as_str),
            ModuleEventCorrelationKind::Request => self.request_id().map(ModuleRequestId::as_str),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModuleTerminalEventContract {
    module: String,
    progress_event: Option<String>,
    success_event: String,
    failure_event: Option<String>,
    correlation: ModuleEventCorrelationKind,
}

impl ModuleTerminalEventContract {
    #[must_use]
    pub(crate) fn new(
        module: &str,
        progress_event: Option<&str>,
        success_event: &str,
        failure_event: Option<&str>,
        correlation: ModuleEventCorrelationKind,
    ) -> Self {
        Self {
            module: module.to_owned(),
            progress_event: progress_event.map(ToOwned::to_owned),
            success_event: success_event.to_owned(),
            failure_event: failure_event.map(ToOwned::to_owned),
            correlation,
        }
    }

    #[must_use]
    pub(crate) fn module(&self) -> &str {
        &self.module
    }

    #[must_use]
    pub(crate) fn progress_event(&self) -> Option<&str> {
        self.progress_event.as_deref()
    }

    #[must_use]
    pub(crate) fn success_event(&self) -> &str {
        &self.success_event
    }

    #[must_use]
    pub(crate) fn failure_event(&self) -> Option<&str> {
        self.failure_event.as_deref()
    }

    #[must_use]
    pub(crate) fn correlation(&self) -> &ModuleEventCorrelationKind {
        &self.correlation
    }

    #[must_use]
    pub(crate) fn recognizes(&self, event: &ModuleEventEnvelope) -> bool {
        self.module == event.module_name
            && (self.progress_event() == Some(event.event_name())
                || self.success_event == event.event_name
                || self.failure_event() == Some(event.event_name()))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ModuleDispatchReceipt {
    acknowledgement: Value,
    identity: ModuleDispatchIdentity,
    bridge_callback_id: Option<BridgeCallbackId>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ModuleDispatchIdentityRole {
    None,
    Session,
    Request,
}

#[derive(Debug, Clone, PartialEq)]
enum ModuleDispatchIdentity {
    None,
    Session(Option<ModuleSessionId>),
    Request(Option<ModuleRequestId>),
}

impl ModuleDispatchReceipt {
    #[must_use]
    pub(crate) fn new(
        mut acknowledgement: Value,
        raw_value: &Value,
        identity_role: ModuleDispatchIdentityRole,
    ) -> Self {
        let identity = match identity_role {
            ModuleDispatchIdentityRole::None => ModuleDispatchIdentity::None,
            ModuleDispatchIdentityRole::Session => ModuleDispatchIdentity::Session(role_id(
                raw_value,
                &["sessionId", "session_id"],
                ModuleSessionId::parse,
            )),
            ModuleDispatchIdentityRole::Request => ModuleDispatchIdentity::Request(role_id(
                raw_value,
                &["requestId", "request_id"],
                ModuleRequestId::parse,
            )),
        };
        if let Value::Object(object) = &mut acknowledgement {
            match &identity {
                ModuleDispatchIdentity::Session(Some(session_id)) => {
                    object.insert(
                        "sessionId".to_owned(),
                        Value::String(session_id.as_str().to_owned()),
                    );
                }
                ModuleDispatchIdentity::Request(Some(request_id)) => {
                    object.insert(
                        "requestId".to_owned(),
                        Value::String(request_id.as_str().to_owned()),
                    );
                }
                ModuleDispatchIdentity::None
                | ModuleDispatchIdentity::Session(None)
                | ModuleDispatchIdentity::Request(None) => {}
            }
        }
        Self {
            acknowledgement,
            identity,
            bridge_callback_id: None,
        }
    }

    #[must_use]
    pub(crate) fn with_bridge_callback(mut self, bridge_callback_id: BridgeCallbackId) -> Self {
        if let Value::Object(object) = &mut self.acknowledgement {
            object.insert(
                "bridgeCallbackId".to_owned(),
                Value::from(bridge_callback_id.value()),
            );
        }
        self.bridge_callback_id = Some(bridge_callback_id);
        self
    }

    #[must_use]
    pub(crate) fn session_correlation(&self) -> Option<ModuleCorrelation> {
        let ModuleDispatchIdentity::Session(Some(session_id)) = &self.identity else {
            return None;
        };
        Some(self.attach_bridge_callback(ModuleCorrelation::with_session(session_id.clone())))
    }

    #[must_use]
    pub(crate) fn request_correlation(&self) -> Option<ModuleCorrelation> {
        let ModuleDispatchIdentity::Request(Some(request_id)) = &self.identity else {
            return None;
        };
        Some(self.attach_bridge_callback(ModuleCorrelation::with_request(request_id.clone())))
    }

    fn attach_bridge_callback(&self, correlation: ModuleCorrelation) -> ModuleCorrelation {
        match self.bridge_callback_id {
            Some(bridge_callback_id) => correlation.with_bridge_callback(bridge_callback_id),
            None => correlation,
        }
    }

    #[must_use]
    pub(crate) fn into_acknowledgement(self) -> Value {
        self.acknowledgement
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NodeOperationOutcome {
    Completed(Value),
    Accepted(Box<ObservableOperationAcceptance>),
    Dispatched(Value),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ObservableOperationAcceptance {
    acknowledgement: Value,
    correlation: ModuleCorrelation,
    terminal_event: ModuleTerminalEventContract,
}

impl ObservableOperationAcceptance {
    #[must_use]
    pub(crate) fn new(
        acknowledgement: Value,
        correlation: ModuleCorrelation,
        terminal_event: ModuleTerminalEventContract,
    ) -> Self {
        Self {
            acknowledgement,
            correlation,
            terminal_event,
        }
    }

    #[must_use]
    pub(crate) fn correlation(&self) -> &ModuleCorrelation {
        &self.correlation
    }

    #[must_use]
    pub(crate) fn terminal_event(&self) -> &ModuleTerminalEventContract {
        &self.terminal_event
    }

    #[must_use]
    pub(crate) fn into_parts(self) -> (Value, ModuleCorrelation, ModuleTerminalEventContract) {
        (self.acknowledgement, self.correlation, self.terminal_event)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ModuleEventEnvelope {
    module_name: String,
    event_name: String,
    args: Vec<Value>,
    first_payload: Value,
}

impl ModuleEventEnvelope {
    pub(crate) fn new(
        module_name: &str,
        event_name: &str,
        args: Vec<Value>,
    ) -> anyhow::Result<Self> {
        let module_name = nonempty_text(module_name)
            .ok_or_else(|| anyhow::anyhow!("runtime module event module name is required"))?;
        let event_name = nonempty_text(event_name)
            .ok_or_else(|| anyhow::anyhow!("runtime module event event name is required"))?;
        let first_payload = args.first().map_or(Value::Null, parsed_value);
        Ok(Self {
            module_name,
            event_name,
            args,
            first_payload,
        })
    }

    pub(crate) fn from_value(value: &Value) -> anyhow::Result<Self> {
        let object = value
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("runtime module event must be an object"))?;
        let module_name = required_object_text(object, "moduleName", "module name")?;
        let event_name = required_object_text(object, "eventName", "event name")?;
        let args = match object.get("args") {
            Some(Value::Array(values)) => values.clone(),
            Some(Value::Null) | None => Vec::new(),
            Some(value) => vec![value.clone()],
        };
        Self::new(&module_name, &event_name, args)
    }

    #[must_use]
    pub(crate) fn module_name(&self) -> &str {
        &self.module_name
    }

    #[must_use]
    pub(crate) fn event_name(&self) -> &str {
        &self.event_name
    }

    #[must_use]
    pub(crate) fn args(&self) -> &[Value] {
        &self.args
    }

    pub(crate) fn retained_serialized_bytes(&self) -> anyhow::Result<usize> {
        let mut counter = SerializedByteCounter::default();
        serde_json::to_writer(
            &mut counter,
            &(
                &self.module_name,
                &self.event_name,
                &self.args,
                &self.first_payload,
            ),
        )
        .map_err(|error| anyhow::anyhow!("failed to measure runtime module event: {error}"))?;
        Ok(counter.bytes)
    }

    #[must_use]
    pub(crate) fn result(&self) -> Value {
        match self.args.as_slice() {
            [] => Value::Null,
            [_] => self.first_payload.clone(),
            values => Value::Array(values.iter().map(parsed_value).collect()),
        }
    }

    #[must_use]
    pub(crate) fn correlation_value(&self, kind: &ModuleEventCorrelationKind) -> Option<String> {
        match kind {
            ModuleEventCorrelationKind::Session => self
                .object_payload()
                .and_then(|object| object_text(object, &["sessionId", "session_id"])),
            ModuleEventCorrelationKind::Request => self
                .object_payload()
                .and_then(|object| object_text(object, &["requestId", "request_id"]))
                .or_else(|| self.args.first().and_then(value_text)),
        }
    }

    #[must_use]
    pub(crate) fn failed(&self, contract: &ModuleTerminalEventContract) -> bool {
        if contract.failure_event() == Some(self.event_name()) {
            return true;
        }
        let Some(object) = self.object_payload() else {
            return false;
        };
        object.get("success").and_then(Value::as_bool) == Some(false)
            || object
                .get("error")
                .and_then(value_text)
                .is_some_and(|error| !error.is_empty())
    }

    #[must_use]
    pub(crate) fn error(&self) -> Option<String> {
        self.object_payload()
            .and_then(|object| object.get("error"))
            .and_then(value_text)
            .or_else(|| self.args.get(2).and_then(value_text))
            .filter(|error| !error.is_empty())
    }

    #[must_use]
    pub(crate) fn progress(&self) -> (Option<u64>, Option<u64>) {
        let Some(object) = self.object_payload() else {
            return (None, None);
        };
        (
            object
                .get("bytes")
                .or_else(|| object.get("byteCount"))
                .or_else(|| object.get("byte_count"))
                .and_then(Value::as_u64),
            object
                .get("totalBytes")
                .or_else(|| object.get("total_bytes"))
                .or_else(|| object.get("contentLength"))
                .or_else(|| object.get("content_length"))
                .and_then(Value::as_u64),
        )
    }

    fn object_payload(&self) -> Option<&Map<String, Value>> {
        self.first_payload.as_object()
    }
}

#[derive(Default)]
struct SerializedByteCounter {
    bytes: usize,
}

impl io::Write for SerializedByteCounter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(buffer.len())
            .ok_or_else(|| io::Error::other("serialized module event byte count overflow"))?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn required_object_text(
    object: &Map<String, Value>,
    key: &str,
    label: &str,
) -> anyhow::Result<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .and_then(nonempty_text)
        .ok_or_else(|| anyhow::anyhow!("runtime module event {label} is required"))
}

fn parsed_value(value: &Value) -> Value {
    let Value::String(text) = value else {
        return value.clone();
    };
    let text = text.trim();
    let structured = (text.starts_with('{') && text.ends_with('}'))
        || (text.starts_with('[') && text.ends_with(']'));
    if !structured {
        return value.clone();
    }
    serde_json::from_str(text).unwrap_or_else(|_| value.clone())
}

fn object_text(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(value_text))
}

fn value_text(value: &Value) -> Option<String> {
    value.as_str().and_then(nonempty_text)
}

fn role_id<T>(value: &Value, keys: &[&str], parse: impl Fn(&str) -> Option<T>) -> Option<T> {
    if let Some(text) = value.as_str() {
        return parse(text);
    }
    let object = value.as_object()?;
    keys.iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str).and_then(&parse))
}

fn nonempty_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};
    use serde_json::json;

    use super::*;

    #[test]
    fn storage_json_string_event_preserves_typed_session_correlation() -> Result<()> {
        let event = ModuleEventEnvelope::from_value(&json!({
            "moduleName": "storage_module",
            "eventName": "storageUploadProgress",
            "args": ["{\"sessionId\":\"session-1\",\"bytes\":8,\"totalBytes\":16}"]
        }))?;

        anyhow::ensure!(
            event.module_name == "storage_module"
                && event.event_name() == "storageUploadProgress"
                && event.correlation_value(&ModuleEventCorrelationKind::Session)
                    == Some("session-1".to_owned())
                && event.progress() == (Some(8), Some(16)),
            "storage module envelope lost typed correlation"
        );
        Ok(())
    }

    #[test]
    fn delivery_positional_event_preserves_request_and_error_roles() -> Result<()> {
        let event = ModuleEventEnvelope::from_value(&json!({
            "moduleName": "delivery_module",
            "eventName": "messageError",
            "args": ["request-1", "hash-1", "delivery failed"]
        }))?;

        anyhow::ensure!(
            event.correlation_value(&ModuleEventCorrelationKind::Request)
                == Some("request-1".to_owned())
                && event.error().as_deref() == Some("delivery failed")
                && event.result() == json!(["request-1", "hash-1", "delivery failed"]),
            "delivery module envelope conflated positional roles"
        );
        Ok(())
    }

    #[test]
    fn delivery_string_values_preserve_wire_types() -> Result<()> {
        let event = ModuleEventEnvelope::from_value(&json!({
            "moduleName": "delivery_module",
            "eventName": "messageSent",
            "args": ["42", "true", "null", "[1,2]"]
        }))?;

        anyhow::ensure!(
            event.result() == json!(["42", "true", "null", [1, 2]]),
            "module event scalar strings changed JSON types"
        );
        Ok(())
    }

    #[test]
    fn module_event_requires_stable_envelope_names() -> Result<()> {
        let Err(error) = ModuleEventEnvelope::from_value(&json!({
            "module": "storage_module",
            "event": "storageUploadDone"
        })) else {
            bail!("legacy untyped module event envelope was accepted");
        };

        anyhow::ensure!(error.to_string() == "runtime module event module name is required");
        Ok(())
    }
}
