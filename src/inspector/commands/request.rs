use anyhow::{Context as _, Result, bail};
use serde::de::DeserializeOwned;
use serde_json::Value;

pub(crate) fn decode_object_request<T>(args: &Value, command: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    let values = args
        .as_array()
        .context("bridge args must be a JSON array")?;
    if values.len() != 1 {
        bail!("{command} requires exactly one request object");
    }
    let value = values
        .first()
        .filter(|value| value.is_object())
        .with_context(|| format!("{command} request must be a JSON object"))?;
    serde_json::from_value(value.clone())
        .with_context(|| format!("failed to decode {command} request"))
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};
    use serde::Deserialize;
    use serde_json::json;

    use super::*;

    #[derive(Debug, PartialEq, Eq, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        revision: u64,
    }

    #[test]
    fn decodes_exactly_one_typed_object() -> Result<()> {
        let request: Request = decode_object_request(&json!([{ "revision": 4 }]), "typedCall")?;
        if request != (Request { revision: 4 }) {
            bail!("unexpected request: {request:?}");
        }
        Ok(())
    }

    #[test]
    fn rejects_positional_or_unknown_request_fields() -> Result<()> {
        for args in [
            json!([]),
            json!([4]),
            json!([{ "revision": 4 }, { "revision": 5 }]),
            json!([{ "revision": 4, "endpoint": "http://localhost" }]),
        ] {
            if decode_object_request::<Request>(&args, "typedCall").is_ok() {
                bail!("invalid typed request was accepted: {args}");
            }
        }
        Ok(())
    }
}
