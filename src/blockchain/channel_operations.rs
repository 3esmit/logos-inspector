use std::fmt;

use common::block::{Block as SequencerBlock, HashableBlockData};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChannelOperationDecodeError {
    Invalid(String),
    Overflow(String),
}

impl fmt::Display for ChannelOperationDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(detail) => write!(formatter, "invalid Channel operation: {detail}"),
            Self::Overflow(detail) => write!(formatter, "Channel operation overflow: {detail}"),
        }
    }
}

impl std::error::Error for ChannelOperationDecodeError {}

pub(crate) type ChannelOperationDecodeResult<T> = Result<T, ChannelOperationDecodeError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InscriptionClassification {
    SequencerBlock,
    Raw,
    Conflicting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DecodedChannelOperationKind {
    Configuration {
        keys: Vec<String>,
        withdraw_threshold: String,
    },
    Inscription {
        parent: String,
        signer: String,
        bytes: Vec<u8>,
        classification: InscriptionClassification,
    },
    Deposit,
    Withdraw,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecodedChannelOperation {
    pub channel_id: String,
    pub transaction_hash: String,
    pub operation_index: u32,
    pub opcode: u8,
    pub payload: Value,
    pub kind: DecodedChannelOperationKind,
}

impl DecodedChannelOperation {
    #[must_use]
    pub(crate) const fn operation_name(&self) -> &'static str {
        match &self.kind {
            DecodedChannelOperationKind::Configuration { .. } => "ChannelConfig",
            DecodedChannelOperationKind::Inscription { .. } => "ChannelInscribe",
            DecodedChannelOperationKind::Deposit => "ChannelDeposit",
            DecodedChannelOperationKind::Withdraw => "ChannelWithdraw",
        }
    }
}

pub(crate) fn decode_block_channel_operations(
    block: &Value,
) -> ChannelOperationDecodeResult<Vec<DecodedChannelOperation>> {
    let transactions = block
        .get("transactions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ChannelOperationDecodeError::Invalid("block transactions are not an array".to_owned())
        })?;
    let mut decoded = Vec::new();
    for transaction in transactions {
        let Some(mantle_tx) = transaction.get("mantle_tx") else {
            continue;
        };
        let Some(operations) = mantle_tx.get("ops").and_then(Value::as_array) else {
            continue;
        };
        for (index, operation) in operations.iter().enumerate() {
            let operation_index = u32::try_from(index).map_err(|_| {
                ChannelOperationDecodeError::Overflow(
                    "transaction operation index exceeds u32".to_owned(),
                )
            })?;
            let transaction_hash = mantle_tx.get("hash").and_then(Value::as_str);
            if let Some(operation) =
                decode_channel_operation(transaction_hash, operation_index, operation)?
            {
                decoded.push(operation);
            }
        }
    }
    Ok(decoded)
}

pub(crate) fn decode_channel_operation(
    transaction_hash: Option<&str>,
    operation_index: u32,
    operation: &Value,
) -> ChannelOperationDecodeResult<Option<DecodedChannelOperation>> {
    let Some(opcode) = operation.get("opcode").and_then(parse_opcode) else {
        return Ok(None);
    };
    if !matches!(opcode, 0x10..=0x13) {
        return Ok(None);
    }
    let opcode = u8::try_from(opcode).map_err(|_| {
        ChannelOperationDecodeError::Overflow("Channel opcode exceeds u8".to_owned())
    })?;
    let transaction_hash = transaction_hash
        .ok_or_else(|| {
            ChannelOperationDecodeError::Invalid(
                "Channel operation transaction hash is missing".to_owned(),
            )
        })
        .and_then(|value| canonical_hex(value, "transaction hash"))?;
    let payload = operation.get("payload").ok_or_else(|| {
        ChannelOperationDecodeError::Invalid(format!(
            "Channel opcode {opcode:#x} payload is missing"
        ))
    })?;
    let (channel_id, kind) = match opcode {
        0x10 => parse_configuration(payload)?,
        0x11 => parse_inscription(payload)?,
        0x12 => (
            required_channel_id(payload, &["channel_id"])?,
            DecodedChannelOperationKind::Deposit,
        ),
        0x13 => (
            required_channel_id(payload, &["channel_id"])?,
            DecodedChannelOperationKind::Withdraw,
        ),
        _ => {
            return Err(ChannelOperationDecodeError::Invalid(format!(
                "unsupported Channel opcode {opcode:#x}"
            )));
        }
    };
    Ok(Some(DecodedChannelOperation {
        channel_id,
        transaction_hash,
        operation_index,
        opcode,
        payload: payload.clone(),
        kind,
    }))
}

fn parse_configuration(
    payload: &Value,
) -> ChannelOperationDecodeResult<(String, DecodedChannelOperationKind)> {
    let channel_id = required_channel_id(payload, &["channel"])?;
    let keys = payload
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ChannelOperationDecodeError::Invalid("ChannelConfig keys are missing".to_owned())
        })?
        .iter()
        .map(|key| {
            key.as_str()
                .ok_or_else(|| {
                    ChannelOperationDecodeError::Invalid("ChannelConfig key is not text".to_owned())
                })
                .and_then(|key| canonical_local_text(key, "Channel key"))
        })
        .collect::<ChannelOperationDecodeResult<Vec<_>>>()?;
    if keys.is_empty() {
        return Err(ChannelOperationDecodeError::Invalid(
            "ChannelConfig keys are empty".to_owned(),
        ));
    }
    let withdraw_threshold = payload
        .get("withdraw_threshold")
        .and_then(integer_text)
        .ok_or_else(|| {
            ChannelOperationDecodeError::Invalid(
                "ChannelConfig withdraw_threshold is missing".to_owned(),
            )
        })?;
    Ok((
        channel_id,
        DecodedChannelOperationKind::Configuration {
            keys,
            withdraw_threshold,
        },
    ))
}

fn parse_inscription(
    payload: &Value,
) -> ChannelOperationDecodeResult<(String, DecodedChannelOperationKind)> {
    let channel_id = required_channel_id(payload, &["channel_id"])?;
    let parent = payload
        .get("parent")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ChannelOperationDecodeError::Invalid("ChannelInscribe parent is missing".to_owned())
        })
        .and_then(|value| canonical_hex(value, "ChannelInscribe parent"))?;
    let signer = payload
        .get("signer")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ChannelOperationDecodeError::Invalid("ChannelInscribe signer is missing".to_owned())
        })
        .and_then(|value| canonical_local_text(value, "ChannelInscribe signer"))?;
    let bytes = inscription_bytes(payload.get("inscription").ok_or_else(|| {
        ChannelOperationDecodeError::Invalid("ChannelInscribe inscription is missing".to_owned())
    })?)?;
    let classification = classify_inscription(&bytes)?;
    Ok((
        channel_id,
        DecodedChannelOperationKind::Inscription {
            parent,
            signer,
            bytes,
            classification,
        },
    ))
}

pub(crate) fn classify_inscription(
    bytes: &[u8],
) -> ChannelOperationDecodeResult<InscriptionClassification> {
    let Ok(block) = borsh::from_slice::<SequencerBlock>(bytes) else {
        return Ok(InscriptionClassification::Raw);
    };
    let hashable = HashableBlockData::from(block.clone());
    let encoded = borsh::to_vec(&hashable).map_err(|error| {
        ChannelOperationDecodeError::Invalid(format!(
            "failed to re-encode Sequencer block: {error}"
        ))
    })?;
    const PREFIX: &[u8; 32] = b"/LEE/v0.3/Message/Block/\x00\x00\x00\x00\x00\x00\x00\x00";
    let mut hasher = Sha256::new();
    hasher.update(PREFIX);
    hasher.update(encoded);
    let computed: [u8; 32] = hasher.finalize().into();
    Ok(if computed == block.header.hash.0 {
        InscriptionClassification::SequencerBlock
    } else {
        InscriptionClassification::Conflicting
    })
}

fn inscription_bytes(value: &Value) -> ChannelOperationDecodeResult<Vec<u8>> {
    match value {
        Value::String(value) => {
            let value = value
                .strip_prefix("0x")
                .or_else(|| value.strip_prefix("0X"))
                .unwrap_or(value);
            hex::decode(value).map_err(|error| {
                ChannelOperationDecodeError::Invalid(format!(
                    "ChannelInscribe inscription is not hexadecimal: {error}"
                ))
            })
        }
        Value::Array(values) => values
            .iter()
            .map(|value| {
                value
                    .as_u64()
                    .and_then(|value| u8::try_from(value).ok())
                    .ok_or_else(|| {
                        ChannelOperationDecodeError::Invalid(
                            "ChannelInscribe byte array contains a non-byte value".to_owned(),
                        )
                    })
            })
            .collect(),
        _ => Err(ChannelOperationDecodeError::Invalid(
            "ChannelInscribe inscription is not bytes".to_owned(),
        )),
    }
}

fn required_channel_id(value: &Value, keys: &[&str]) -> ChannelOperationDecodeResult<String> {
    let candidate = keys
        .iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .ok_or_else(|| ChannelOperationDecodeError::Invalid("Channel id is missing".to_owned()))?;
    canonical_hex(candidate, "Channel id")
}

fn canonical_hex(value: &str, label: &str) -> ChannelOperationDecodeResult<String> {
    let value = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ChannelOperationDecodeError::Invalid(format!(
            "{label} must be 32-byte hexadecimal text"
        )));
    }
    Ok(value.to_ascii_lowercase())
}

fn canonical_local_text(value: &str, label: &str) -> ChannelOperationDecodeResult<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(ChannelOperationDecodeError::Invalid(format!(
            "{label} is invalid"
        )));
    }
    Ok(value.to_owned())
}

fn parse_opcode(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        value.as_str().and_then(|value| {
            let value = value.trim();
            if let Some(value) = value
                .strip_prefix("0x")
                .or_else(|| value.strip_prefix("0X"))
            {
                u64::from_str_radix(value, 16).ok()
            } else {
                value.parse().ok()
            }
        })
    })
}

fn integer_text(value: &Value) -> Option<String> {
    match value {
        Value::Number(value) => Some(value.to_string()),
        Value::String(value) if !value.trim().is_empty() => Some(value.trim().to_owned()),
        Value::Null | Value::Bool(_) | Value::String(_) | Value::Array(_) | Value::Object(_) => {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Context as _, ensure};
    use serde_json::json;

    #[test]
    fn decodes_role_complete_channel_operations() -> anyhow::Result<()> {
        let channel = "a".repeat(64);
        let transaction = "b".repeat(64);
        let block = json!({
            "transactions": [{
                "mantle_tx": {
                    "hash": transaction,
                    "ops": [
                        {
                            "opcode": 16,
                            "payload": {
                                "channel": channel,
                                "keys": ["key-a"],
                                "withdraw_threshold": 1
                            }
                        },
                        {
                            "opcode": "0x12",
                            "payload": { "channel_id": "a".repeat(64) }
                        }
                    ]
                }
            }]
        });

        let decoded = decode_block_channel_operations(&block)?;
        let configuration = decoded.first().context("configuration is missing")?;
        let deposit = decoded.get(1).context("deposit is missing")?;

        ensure!(decoded.len() == 2, "unexpected operation count");
        ensure!(
            matches!(
                &configuration.kind,
                DecodedChannelOperationKind::Configuration { .. }
            ),
            "configuration kind was not retained"
        );
        ensure!(
            deposit.kind == DecodedChannelOperationKind::Deposit,
            "deposit kind was not retained"
        );
        ensure!(
            deposit.operation_index == 1,
            "operation index was not retained"
        );
        Ok(())
    }

    #[test]
    fn malformed_known_operation_fails_strict_decode() {
        let block = json!({
            "transactions": [{
                "mantle_tx": {
                    "hash": "b".repeat(64),
                    "ops": [{
                        "opcode": 16,
                        "payload": { "channel": "a".repeat(64) }
                    }]
                }
            }]
        });

        assert!(decode_block_channel_operations(&block).is_err());
    }
}
