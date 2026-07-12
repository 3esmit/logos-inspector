use anyhow::{Context as _, Result, bail};
use serde::Serialize;
use serde_json::Value;

use super::{ZoneSocialScope, zone_topic_matches_scope};

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
        #[serde(skip_serializing_if = "Option::is_none")]
        scope: Option<ZoneSocialScope>,
    },
    #[serde(rename = "lez_account_idl")]
    LezAccountIdl {
        version: u64,
        identity: Value,
        account_id: String,
        program_id: String,
        idl_name: String,
        idl_json: String,
        idl_cid: String,
        storage: Option<Value>,
        created_at: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        scope: Option<ZoneSocialScope>,
    },
}

pub fn parse_social_payload(
    raw_json: &str,
    expected_account_id: Option<&str>,
) -> Result<SocialPayload> {
    let value = serde_json::from_str::<Value>(raw_json).context("social payload is not JSON")?;
    parse_social_payload_value(&value, expected_account_id, None)
}

pub(crate) fn parse_social_payload_value(
    value: &Value,
    expected_account_id: Option<&str>,
    expected_topic: Option<&str>,
) -> Result<SocialPayload> {
    let object = value
        .as_object()
        .context("social payload must be a JSON object")?;
    let kind = required_string(value, "kind")?;
    let version = object
        .get("version")
        .and_then(Value::as_u64)
        .context("social payload version is required")?;
    if version != 1 && version != 2 {
        bail!("social payload version is not supported");
    }
    let identity = object
        .get("identity")
        .filter(|value| value.as_object().is_some())
        .cloned()
        .context("social payload identity is required")?;

    match kind {
        "comment" => {
            let conversation_id = required_string(value, "conversation_id")?.to_owned();
            let scope = parse_scope(value, version)?;
            validate_topic_scope(
                version,
                expected_topic.unwrap_or(&conversation_id),
                scope.as_ref(),
            )?;
            Ok(SocialPayload::Comment {
                version,
                identity,
                body: required_string(value, "body")?.to_owned(),
                created_at: required_string(value, "created_at")?.to_owned(),
                conversation_id,
                scope,
            })
        }
        "lez_account_idl" => {
            let account_id = required_string(value, "account_id")?.to_owned();
            if let Some(expected) = expected_account_id
                && !ids_match(&account_id, expected)
            {
                bail!("shared IDL account does not match requested account");
            }
            let idl_json = optional_string(value, "idl_json")
                .unwrap_or_default()
                .to_owned();
            if !idl_json.is_empty() {
                bail!("shared IDL inline JSON is not supported; use storage CID");
            }
            let idl_cid = required_string(value, "idl_cid")?.to_owned();
            let storage = value
                .get("storage")
                .filter(|value| value.as_object().is_some())
                .cloned()
                .context("shared IDL storage metadata is required")?;
            let scope = parse_scope(value, version)?;
            if let Some(scope) = &scope
                && (scope.entity_kind != crate::inspection::l2::ZoneL2EntityKind::Account
                    || !ids_match(&account_id, &scope.canonical_entity_key))
            {
                bail!("shared IDL scope does not match account");
            }
            if let Some(topic) = expected_topic {
                validate_topic_scope(version, topic, scope.as_ref())?;
            }
            Ok(SocialPayload::LezAccountIdl {
                version,
                identity,
                account_id,
                program_id: required_string(value, "program_id")?.to_owned(),
                idl_name: required_string(value, "idl_name")?.to_owned(),
                idl_json,
                idl_cid,
                storage: Some(storage),
                created_at: required_string(value, "created_at")?.to_owned(),
                scope,
            })
        }
        _ => bail!("social payload kind is not supported"),
    }
}

pub(crate) fn parse_social_payload_for_topic(
    raw_json: &str,
    expected_account_id: Option<&str>,
    expected_topic: &str,
) -> Result<SocialPayload> {
    let value = serde_json::from_str::<Value>(raw_json).context("social payload is not JSON")?;
    parse_social_payload_value(&value, expected_account_id, Some(expected_topic))
}

fn parse_scope(value: &Value, version: u64) -> Result<Option<ZoneSocialScope>> {
    let scope = value
        .get("scope")
        .map(|scope| serde_json::from_value(scope.clone()))
        .transpose()
        .context("social payload scope is invalid")?
        .map(|scope: ZoneSocialScope| {
            scope
                .canonicalized()
                .context("social payload scope is not canonicalizable")
        })
        .transpose()?;
    if version == 2 && scope.is_none() {
        bail!("social payload scope is required");
    }
    if version == 1 && scope.is_some() {
        bail!("social payload version 1 cannot include Zone scope");
    }
    Ok(scope)
}

fn validate_topic_scope(version: u64, topic: &str, scope: Option<&ZoneSocialScope>) -> Result<()> {
    if topic.starts_with("/lez/") {
        if version != 2 {
            bail!("unqualified LEZ social payload is not supported");
        }
        let scope = scope.context("Zone social scope is required")?;
        if !zone_topic_matches_scope(topic, scope) {
            bail!("social topic does not match payload scope");
        }
    } else if version != 1 || scope.is_some() {
        bail!("Cryptarchia social payload must use version 1");
    }
    Ok(())
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("social payload {key} is required"))
}

fn optional_string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn ids_match(left: &str, right: &str) -> bool {
    if let (Ok(left), Ok(right)) = (
        crate::parse_account_id(left),
        crate::parse_account_id(right),
    ) {
        return left == right;
    }
    normalized_id_for_match(left) == normalized_id_for_match(right)
}

fn normalized_id_for_match(value: &str) -> String {
    let trimmed = value.trim().trim_start_matches("0x");
    if trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return trimmed.to_ascii_lowercase();
    }
    value.trim().to_owned()
}
