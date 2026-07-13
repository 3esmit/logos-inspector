use anyhow::Result;
use serde_json::{Value, json};

use crate::source_routing::{ModuleDispatchIdentityRole, ModuleDispatchReceipt};

pub(crate) fn call_value(module: &str, method: &str, values: &[Value]) -> Result<Value> {
    let args = values.iter().map(module_arg_text).collect::<Vec<_>>();
    crate::source_routing::core::adapters::module::call_value(module, method, &args)
}

pub(crate) fn dispatch_result(
    module: &str,
    method: &str,
    value: Value,
    context: &[(&str, String)],
    identity_role: ModuleDispatchIdentityRole,
) -> ModuleDispatchReceipt {
    let raw_value = value.clone();
    let mut result = json!({
        "module": module,
        "method": method,
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

fn module_arg_text(value: &Value) -> String {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| value.to_string())
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
            json!("42"),
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
            json!("request-7"),
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
