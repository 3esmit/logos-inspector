use anyhow::{Context as _, Result, bail};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind")]
pub enum SocialPayload {
    #[serde(rename = "comment")]
    Comment {
        version: u64,
        identity: Value,
        body: String,
        created_at: String,
        conversation_id: String,
    },
    #[serde(rename = "lez_account_idl")]
    LezAccountIdl {
        version: u64,
        identity: Value,
        account_id: String,
        program_id: String,
        idl_name: String,
        idl_json: String,
        created_at: String,
    },
}

pub fn parse_social_payload(
    raw_json: &str,
    expected_account_id: Option<&str>,
) -> Result<SocialPayload> {
    let value = serde_json::from_str::<Value>(raw_json).context("social payload is not JSON")?;
    parse_social_payload_value(&value, expected_account_id)
}

fn parse_social_payload_value(
    value: &Value,
    expected_account_id: Option<&str>,
) -> Result<SocialPayload> {
    let object = value
        .as_object()
        .context("social payload must be a JSON object")?;
    let kind = required_string(value, "kind")?;
    let version = object
        .get("version")
        .and_then(Value::as_u64)
        .context("social payload version is required")?;
    if version != 1 {
        bail!("social payload version is not supported");
    }
    let identity = object
        .get("identity")
        .filter(|value| value.as_object().is_some())
        .cloned()
        .context("social payload identity is required")?;

    match kind {
        "comment" => Ok(SocialPayload::Comment {
            version,
            identity,
            body: required_string(value, "body")?.to_owned(),
            created_at: required_string(value, "created_at")?.to_owned(),
            conversation_id: required_string(value, "conversation_id")?.to_owned(),
        }),
        "lez_account_idl" => {
            let account_id = required_string(value, "account_id")?.to_owned();
            if let Some(expected) = expected_account_id
                && !ids_match(&account_id, expected)
            {
                bail!("shared IDL account does not match requested account");
            }
            let idl_json = required_string(value, "idl_json")?.to_owned();
            let _idl_value: Value =
                serde_json::from_str(&idl_json).context("shared IDL JSON is not valid JSON")?;
            Ok(SocialPayload::LezAccountIdl {
                version,
                identity,
                account_id,
                program_id: required_string(value, "program_id")?.to_owned(),
                idl_name: required_string(value, "idl_name")?.to_owned(),
                idl_json,
                created_at: required_string(value, "created_at")?.to_owned(),
            })
        }
        _ => bail!("social payload kind is not supported"),
    }
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("social payload {key} is required"))
}

fn ids_match(left: &str, right: &str) -> bool {
    normalized_id_for_match(left) == normalized_id_for_match(right)
}

fn normalized_id_for_match(value: &str) -> String {
    let trimmed = value.trim().trim_start_matches("0x");
    if trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return trimmed.to_ascii_lowercase();
    }
    value.trim().to_owned()
}
