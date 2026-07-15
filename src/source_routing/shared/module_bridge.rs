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
    let bridge_callback_id = reply.bridge_callback_id();
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
    let receipt = ModuleDispatchReceipt::new(result, &raw_value, identity_role);
    match bridge_callback_id {
        Some(bridge_callback_id) => receipt.with_bridge_callback(bridge_callback_id),
        None => receipt,
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use serde_json::json;

    use super::*;
    use crate::modules::logos_core::BridgeCallbackId;

    #[test]
    fn dispatch_receipt_preserves_only_explicit_session_role() -> Result<()> {
        let receipt = dispatch_result(
            "storage_module",
            "uploadUrl",
            ModuleCallReply::new(ModuleTransportKind::LogoscoreCli, json!("42")),
            &[("path", "/tmp/a".to_owned())],
            ModuleDispatchIdentityRole::Session,
        );

        let correlation = receipt
            .session_correlation()
            .ok_or_else(|| anyhow::anyhow!("session correlation was lost"))?;
        anyhow::ensure!(
            correlation.session_id().map(|id| id.as_str()) == Some("42"),
            "explicit session conversion lost scalar identity"
        );
        anyhow::ensure!(correlation.request_id().is_none());
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

        let correlation = receipt
            .request_correlation()
            .ok_or_else(|| anyhow::anyhow!("request correlation was lost"))?;
        anyhow::ensure!(correlation.session_id().is_none());
        anyhow::ensure!(correlation.request_id().map(|id| id.as_str()) == Some("request-7"));
        let acknowledgement = receipt.into_acknowledgement();
        anyhow::ensure!(
            acknowledgement.get("requestId") == Some(&json!("request-7"))
                && acknowledgement.get("sessionId").is_none(),
            "request receipt lost or conflated explicit identity"
        );
        Ok(())
    }

    #[test]
    fn basecamp_callback_identity_reaches_acknowledgement_and_request_correlation() -> Result<()> {
        let receipt = dispatch_result(
            "delivery_module",
            "send",
            ModuleCallReply::new(ModuleTransportKind::Module, json!("request-7"))
                .with_bridge_callback(BridgeCallbackId::new(17)),
            &[],
            ModuleDispatchIdentityRole::Request,
        );

        let correlation = receipt
            .request_correlation()
            .ok_or_else(|| anyhow::anyhow!("request correlation was lost"))?;
        anyhow::ensure!(
            correlation
                .bridge_callback_id()
                .map(BridgeCallbackId::value)
                == Some(17),
            "Basecamp callback identity was lost from request correlation"
        );
        let acknowledgement = receipt.into_acknowledgement();
        anyhow::ensure!(
            acknowledgement.get("bridgeCallbackId") == Some(&json!(17)),
            "Basecamp callback identity was lost from dispatch acknowledgement"
        );
        Ok(())
    }
}
