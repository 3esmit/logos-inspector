use std::collections::BTreeMap;

use anyhow::{Context as _, Result, bail};
use lee_core::program::ProgramId;
use serde_json::Value;

use crate::normalize_program_id_hex;

#[derive(Debug, Clone)]
pub(super) struct ParsedValue {
    pub(super) report_value: String,
    pub(super) dynamic: DynamicValue,
    pub(super) seed_bytes: Option<[u8; 32]>,
}

#[derive(Debug, Clone)]
pub(super) enum DynamicValue {
    Bool(bool),
    U8(u8),
    U32(u32),
    U64(u64),
    U128(u128),
    Str(String),
    Tuple(Vec<DynamicValue>),
    Seq(Vec<DynamicValue>),
    None,
    Some(Box<DynamicValue>),
}

impl serde::Serialize for DynamicValue {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Self::Bool(value) => serializer.serialize_bool(*value),
            Self::U8(value) => serializer.serialize_u8(*value),
            Self::U32(value) => serializer.serialize_u32(*value),
            Self::U64(value) => serializer.serialize_u64(*value),
            Self::U128(value) => serializer.serialize_u128(*value),
            Self::Str(value) => serializer.serialize_str(value),
            Self::Tuple(items) => {
                use serde::ser::SerializeTuple as _;
                let mut tuple = serializer.serialize_tuple(items.len())?;
                for item in items {
                    tuple.serialize_element(item)?;
                }
                tuple.end()
            }
            Self::Seq(items) => {
                use serde::ser::SerializeSeq as _;
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Self::None => serializer.serialize_none(),
            Self::Some(value) => serializer.serialize_some(value.as_ref()),
        }
    }
}

pub(super) struct InstructionData<'a> {
    pub(super) variant_index: u32,
    pub(super) fields: &'a [DynamicValue],
}

impl serde::Serialize for InstructionData<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeTupleVariant as _;

        let mut variant =
            serializer.serialize_tuple_variant("", self.variant_index, "", self.fields.len())?;
        for field in self.fields {
            variant.serialize_field(field)?;
        }
        variant.end()
    }
}

pub(super) fn parse_typed_value(raw: &str, ty: &Value) -> Result<ParsedValue> {
    if let Some(primitive) = ty.as_str() {
        return parse_primitive(raw, primitive);
    }
    if let Some(array) = ty.get("array") {
        return parse_array(raw, array);
    }
    if let Some(vec) = ty.get("vec") {
        return parse_vec(raw, vec);
    }
    if let Some(option) = ty.get("option") {
        if raw.trim().is_empty() || matches!(raw.trim(), "none" | "null") {
            return Ok(ParsedValue {
                report_value: "None".to_owned(),
                dynamic: DynamicValue::None,
                seed_bytes: None,
            });
        }
        let parsed = parse_typed_value(raw, option)?;
        return Ok(ParsedValue {
            report_value: format!("Some({})", parsed.report_value),
            dynamic: DynamicValue::Some(Box::new(parsed.dynamic)),
            seed_bytes: parsed.seed_bytes,
        });
    }
    if let Some(defined) = ty.get("defined").and_then(Value::as_str) {
        bail!("defined IDL arg type `{defined}` is not supported for direct interaction");
    }
    bail!("unsupported IDL arg type `{}`", ty);
}

fn parse_primitive(raw: &str, primitive: &str) -> Result<ParsedValue> {
    let raw = raw.trim();
    match primitive {
        "bool" => {
            let value = match raw {
                "true" | "1" | "yes" => true,
                "false" | "0" | "no" => false,
                _ => bail!("invalid bool `{raw}`"),
            };
            Ok(parsed_value(
                value.to_string(),
                DynamicValue::Bool(value),
                None,
            ))
        }
        "u8" => {
            let value = raw.parse::<u8>().context("invalid u8")?;
            Ok(parsed_value(
                value.to_string(),
                DynamicValue::U8(value),
                None,
            ))
        }
        "u32" => {
            let value = raw.parse::<u32>().context("invalid u32")?;
            Ok(parsed_value(
                value.to_string(),
                DynamicValue::U32(value),
                None,
            ))
        }
        "u64" => {
            let value = raw.parse::<u64>().context("invalid u64")?;
            let mut seed = [0_u8; 32];
            seed[24..32].copy_from_slice(&value.to_be_bytes());
            Ok(parsed_value(
                value.to_string(),
                DynamicValue::U64(value),
                Some(seed),
            ))
        }
        "u128" => {
            let value = raw.parse::<u128>().context("invalid u128")?;
            let mut seed = [0_u8; 32];
            seed[16..32].copy_from_slice(&value.to_be_bytes());
            Ok(parsed_value(
                value.to_string(),
                DynamicValue::U128(value),
                Some(seed),
            ))
        }
        "string" | "String" => {
            let mut seed = [0_u8; 32];
            let bytes = raw.as_bytes();
            let seed_bytes = if bytes.len() <= 32 {
                let Some(seed_prefix) = seed.get_mut(..bytes.len()) else {
                    bail!("string seed exceeds 32 bytes");
                };
                seed_prefix.copy_from_slice(bytes);
                Some(seed)
            } else {
                None
            };
            Ok(parsed_value(
                raw.to_owned(),
                DynamicValue::Str(raw.to_owned()),
                seed_bytes,
            ))
        }
        "program_id" => {
            let program_id_hex = normalize_program_id_hex(raw)?;
            let program_id = program_id_from_hex(&program_id_hex)?;
            Ok(parsed_value(
                program_id_hex,
                DynamicValue::Tuple(program_id.iter().copied().map(DynamicValue::U32).collect()),
                None,
            ))
        }
        other => bail!("unsupported primitive IDL arg type `{other}`"),
    }
}

fn parse_array(raw: &str, array: &Value) -> Result<ParsedValue> {
    let items = array.as_array().context("IDL array type is not an array")?;
    let elem = items.first().context("IDL array type missing element")?;
    let size = items
        .get(1)
        .and_then(Value::as_u64)
        .context("IDL array type missing size")? as usize;
    match elem.as_str() {
        Some("u8") => parse_u8_array(raw, size),
        Some("u32") => parse_u32_array(raw, size),
        _ => bail!("unsupported array IDL arg type `{}`", array),
    }
}

fn parse_u8_array(raw: &str, size: usize) -> Result<ParsedValue> {
    let raw = raw.trim();
    let bytes = if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        hex::decode(hex).context("invalid hex bytes")?
    } else if raw.len() == size * 2 && raw.chars().all(|ch| ch.is_ascii_hexdigit()) {
        hex::decode(raw).context("invalid hex bytes")?
    } else {
        let mut bytes = vec![0_u8; size];
        let raw_bytes = raw.as_bytes();
        if raw_bytes.len() > size {
            bail!("string is {} bytes, max {size}", raw_bytes.len());
        }
        let Some(bytes_prefix) = bytes.get_mut(..raw_bytes.len()) else {
            bail!("string is {} bytes, max {size}", raw_bytes.len());
        };
        bytes_prefix.copy_from_slice(raw_bytes);
        bytes
    };
    if bytes.len() != size {
        bail!("expected {size} bytes, got {}", bytes.len());
    }
    let seed_bytes = if size == 32 {
        let mut seed = [0_u8; 32];
        seed.copy_from_slice(&bytes);
        Some(seed)
    } else {
        None
    };
    Ok(parsed_value(
        format!("0x{}", hex::encode(&bytes)),
        DynamicValue::Tuple(bytes.into_iter().map(DynamicValue::U8).collect()),
        seed_bytes,
    ))
}

fn parse_u32_array(raw: &str, size: usize) -> Result<ParsedValue> {
    let parts = raw.split(',').map(str::trim).collect::<Vec<_>>();
    if parts.len() != size {
        bail!("expected {size} u32 values, got {}", parts.len());
    }
    let mut values = Vec::with_capacity(size);
    for part in parts {
        values.push(part.parse::<u32>().context("invalid u32 array item")?);
    }
    Ok(parsed_value(
        format!(
            "[{}]",
            values
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        DynamicValue::Tuple(values.into_iter().map(DynamicValue::U32).collect()),
        None,
    ))
}

fn parse_vec(raw: &str, elem: &Value) -> Result<ParsedValue> {
    match elem.as_str() {
        Some("u8") => {
            let values = if raw.trim().is_empty() {
                Vec::new()
            } else {
                raw.split(',')
                    .map(str::trim)
                    .map(|item| item.parse::<u8>().context("invalid u8 vector item"))
                    .collect::<Result<Vec<_>>>()?
            };
            Ok(parsed_value(
                format!("{} bytes", values.len()),
                DynamicValue::Seq(values.into_iter().map(DynamicValue::U8).collect()),
                None,
            ))
        }
        Some("u32") => {
            let values = if raw.trim().is_empty() {
                Vec::new()
            } else {
                raw.split(',')
                    .map(str::trim)
                    .map(|item| item.parse::<u32>().context("invalid u32 vector item"))
                    .collect::<Result<Vec<_>>>()?
            };
            Ok(parsed_value(
                format!("{} words", values.len()),
                DynamicValue::Seq(values.into_iter().map(DynamicValue::U32).collect()),
                None,
            ))
        }
        _ if elem.get("array").is_some() => {
            let array = elem.get("array").context("checked array type")?;
            let items = array.as_array().context("IDL array type is not an array")?;
            let array_elem = items.first().context("IDL array type missing element")?;
            let size = items
                .get(1)
                .and_then(Value::as_u64)
                .context("IDL array type missing size")? as usize;
            if array_elem.as_str() != Some("u8") {
                bail!("unsupported vector element type `{elem}`");
            }
            let values = if raw.trim().is_empty() {
                Vec::new()
            } else {
                raw.split(',')
                    .map(str::trim)
                    .map(|item| parse_u8_array(item, size).map(|parsed| parsed.dynamic))
                    .collect::<Result<Vec<_>>>()?
            };
            Ok(parsed_value(
                format!("{} items", values.len()),
                DynamicValue::Seq(values),
                None,
            ))
        }
        _ => bail!("unsupported vector IDL arg type `{elem}`"),
    }
}

fn parsed_value(
    report_value: String,
    dynamic: DynamicValue,
    seed_bytes: Option<[u8; 32]>,
) -> ParsedValue {
    ParsedValue {
        report_value,
        dynamic,
        seed_bytes,
    }
}

pub(super) fn type_label(ty: &Value) -> String {
    match ty {
        Value::String(value) => value.clone(),
        Value::Object(object) if object.contains_key("array") => {
            let Some(items) = object.get("array").and_then(Value::as_array) else {
                return ty.to_string();
            };
            let elem = items
                .first()
                .map(type_label)
                .unwrap_or_else(|| "?".to_owned());
            let size = items
                .get(1)
                .and_then(Value::as_u64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".to_owned());
            format!("[{elem}; {size}]")
        }
        Value::Object(object) if object.contains_key("vec") => {
            let elem = object
                .get("vec")
                .map(type_label)
                .unwrap_or_else(|| "?".to_owned());
            format!("Vec<{elem}>")
        }
        Value::Object(object) if object.contains_key("option") => {
            let elem = object
                .get("option")
                .map(type_label)
                .unwrap_or_else(|| "?".to_owned());
            format!("Option<{elem}>")
        }
        Value::Object(object) if object.contains_key("defined") => object
            .get("defined")
            .and_then(Value::as_str)
            .unwrap_or("defined")
            .to_owned(),
        _ => ty.to_string(),
    }
}

pub(super) fn named_value<'a>(values: &'a BTreeMap<String, String>, name: &str) -> Option<&'a str> {
    values
        .get(name)
        .or_else(|| values.get(&kebab_name(name)))
        .map(String::as_str)
}

pub(super) fn kebab_name(name: &str) -> String {
    name.replace('_', "-")
}

pub(super) fn program_id_from_hex(value: &str) -> Result<ProgramId> {
    let bytes = hex::decode(value).context("invalid program id hex")?;
    if bytes.len() != 32 {
        bail!("program id hex must be 32 bytes");
    }
    let mut program_id = [0_u32; 8];
    for (index, chunk) in bytes.chunks_exact(4).enumerate() {
        let word = u32::from_le_bytes(
            chunk
                .try_into()
                .map_err(|_| anyhow::anyhow!("program id chunk must be 4 bytes"))?,
        );
        if let Some(slot) = program_id.get_mut(index) {
            *slot = word;
        }
    }
    Ok(program_id)
}
