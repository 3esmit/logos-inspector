use anyhow::Result;
use serde_json::{Value, json};

use crate::{
    modules::logos_core::{
        ModuleCall, ModuleCallReply, ModuleTransportKind, SharedModuleTransport,
        dispatch_module_call,
    },
    source_routing::{ModuleDispatchIdentityRole, ModuleDispatchReceipt},
};

pub(crate) async fn call_value(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    module: &str,
    method: &str,
    values: Vec<Value>,
) -> Result<ModuleCallReply> {
    let call = ModuleCall::new(transport_kind, module, method, values)?;
    dispatch_module_call(transport.as_ref(), call).await
}

pub(crate) fn dispatch_result(
    module: &str,
    method: &str,
    reply: ModuleCallReply,
    context: &[(&str, String)],
    identity_role: ModuleDispatchIdentityRole,
) -> ModuleDispatchReceipt {
    let transport = reply.transport();
    let value = reply.into_value();
    let raw_value = value.clone();
    let mut result = json!({
        "module": module,
        "method": method,
        "adapter": transport,
        "dispatched": true,
        "value": value,
    });
    if let Some(object) = result.as_object_mut() {
        for (key, value) in context {
            if !value.trim().is_empty() {
                object.insert((*key).to_owned(), json!(value));
            }
        }
    }
    ModuleDispatchReceipt::new(result, &raw_value, identity_role)
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use serde_json::json;

    use super::*;

    #[test]
    fn dispatch_receipt_preserves_only_explicit_session_role() -> Result<()> {
        let receipt = dispatch_result(
            "storage_module",
            "uploadUrl",
            ModuleCallReply::new(ModuleTransportKind::LogoscoreCli, json!("42")),
            &[("path", "/tmp/a".to_owned())],
            ModuleDispatchIdentityRole::Session,
        );

        anyhow::ensure!(
            receipt.session_id().map(|id| id.as_str().to_owned()) == Some("42".to_owned()),
            "explicit session conversion lost scalar identity"
        );
        anyhow::ensure!(
            receipt.request_id().is_none(),
            "session-tagged receipt exposed request identity"
        );
        let acknowledgement = receipt.into_acknowledgement();
        anyhow::ensure!(
            acknowledgement
                == json!({
                    "module": "storage_module",
                    "method": "uploadUrl",
                    "adapter": "logoscore_cli",
                    "dispatched": true,
                    "value": "42",
                    "sessionId": "42",
                    "path": "/tmp/a"
                }),
            "dispatch acknowledgement drifted"
        );
        anyhow::ensure!(
            acknowledgement.get("requestId").is_none(),
            "session receipt inferred request identity"
        );
        Ok(())
    }

    #[test]
    fn dispatch_receipt_preserves_only_explicit_request_role() -> Result<()> {
        let receipt = dispatch_result(
            "delivery_module",
            "send",
            ModuleCallReply::new(ModuleTransportKind::LogoscoreCli, json!("request-7")),
            &[],
            ModuleDispatchIdentityRole::Request,
        );

        anyhow::ensure!(receipt.session_id().is_none());
        anyhow::ensure!(
            receipt.request_id().map(|id| id.as_str().to_owned()) == Some("request-7".to_owned())
        );
        let acknowledgement = receipt.into_acknowledgement();
        anyhow::ensure!(
            acknowledgement.get("requestId") == Some(&json!("request-7"))
                && acknowledgement.get("sessionId").is_none(),
            "request receipt lost or conflated explicit identity"
        );
        Ok(())
    }
}
