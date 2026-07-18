use std::collections::BTreeMap;

use anyhow::{Context as _, Result, bail};
use lee::AccountId;
use lee_core::program::ProgramId;
use serde_json::Value;

use crate::{
    decode::idl_type::{fixed_array_type, idl_type_label},
    normalize_program_id_hex,
};

#[derive(Debug, Clone)]
pub(crate) struct ParsedValue {
    pub(crate) report_value: String,
    pub(crate) dynamic: DynamicValue,
    pub(crate) seed_bytes: Option<[u8; 32]>,
}

#[derive(Debug, Clone)]
pub(crate) enum DynamicValue {
    Bool(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
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
            Self::I8(value) => serializer.serialize_i8(*value),
            Self::I16(value) => serializer.serialize_i16(*value),
            Self::I32(value) => serializer.serialize_i32(*value),
            Self::I64(value) => serializer.serialize_i64(*value),
            Self::I128(value) => serializer.serialize_i128(*value),
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

pub(crate) struct InstructionData<'a> {
    pub(crate) variant_index: u32,
    pub(crate) fields: &'a [DynamicValue],
}

pub(crate) struct InstructionDecoded {
    pub(crate) value: String,
    pub(crate) consumed: usize,
    pub(crate) type_label: String,
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

pub(crate) fn parse_typed_value(raw: &str, ty: &Value) -> Result<ParsedValue> {
    if let Some(primitive) = ty.as_str() {
        return parse_primitive(raw, primitive);
    }
    if fixed_array_type(ty)?.is_some() {
        return parse_array(raw, ty);
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
        "i8" => {
            let value = raw.parse::<i8>().context("invalid i8")?;
            Ok(parsed_value(
                value.to_string(),
                DynamicValue::I8(value),
                None,
            ))
        }
        "i16" => {
            let value = raw.parse::<i16>().context("invalid i16")?;
            Ok(parsed_value(
                value.to_string(),
                DynamicValue::I16(value),
                None,
            ))
        }
        "i32" => {
            let value = raw.parse::<i32>().context("invalid i32")?;
            Ok(parsed_value(
                value.to_string(),
                DynamicValue::I32(value),
                None,
            ))
        }
        "i64" => {
            let value = raw.parse::<i64>().context("invalid i64")?;
            Ok(parsed_value(
                value.to_string(),
                DynamicValue::I64(value),
                None,
            ))
        }
        "i128" => {
            let value = raw.parse::<i128>().context("invalid i128")?;
            Ok(parsed_value(
                value.to_string(),
                DynamicValue::I128(value),
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
        "account_id" => {
            let account_id = parse_instruction_account_id(raw)?;
            let canonical = account_id.to_string();
            Ok(parsed_value(
                canonical.clone(),
                DynamicValue::Str(canonical),
                Some(*account_id.value()),
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

fn parse_instruction_account_id(raw: &str) -> Result<AccountId> {
    let raw = raw.trim();
    let raw = raw
        .strip_prefix("Private/")
        .or_else(|| raw.strip_prefix("private/"))
        .or_else(|| raw.strip_prefix("Public/"))
        .or_else(|| raw.strip_prefix("public/"))
        .unwrap_or(raw)
        .trim();
    crate::parse_account_id(raw)
}

fn parse_array(raw: &str, ty: &Value) -> Result<ParsedValue> {
    let (elem, size) = fixed_array_type(ty)?.context("IDL array type is not an array")?;
    match elem.as_str() {
        Some("u8") => parse_u8_array(raw, size),
        Some("u32") => parse_u32_array(raw, size),
        _ => bail!("unsupported array IDL arg type `{}`", ty),
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
    if let Some((array_elem, size)) = fixed_array_type(elem)? {
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
        return Ok(parsed_value(
            format!("{} items", values.len()),
            DynamicValue::Seq(values),
            None,
        ));
    }

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

pub(crate) fn type_label(ty: &Value) -> String {
    match ty {
        Value::String(value) => value.clone(),
        Value::Object(object) if object.contains_key("array") => {
            if let Ok(Some((elem, size))) = fixed_array_type(ty) {
                return format!("[{}; {size}]", type_label(elem));
            }
            ty.to_string()
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

pub(crate) fn named_value<'a>(values: &'a BTreeMap<String, String>, name: &str) -> Option<&'a str> {
    values
        .get(name)
        .or_else(|| values.get(&kebab_name(name)))
        .map(String::as_str)
}

pub(crate) fn kebab_name(name: &str) -> String {
    name.replace('_', "-")
}

pub(crate) fn program_id_from_hex(value: &str) -> Result<ProgramId> {
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

pub(crate) fn decode_instruction_type(
    ty: &Value,
    words: &[u32],
    offset: usize,
    depth: usize,
) -> Result<InstructionDecoded> {
    if depth > 32 {
        bail!("IDL nesting too deep");
    }

    let label = idl_type_label(ty);
    if let Some(primitive) = ty.as_str() {
        return decode_instruction_primitive(primitive, words, offset).map(|mut decoded| {
            decoded.type_label = label;
            decoded
        });
    }

    let object = ty
        .as_object()
        .with_context(|| format!("unsupported instruction type {label}"))?;

    if let Some(inner) = object.get("option") {
        let tag = *words
            .get(offset)
            .with_context(|| format!("missing option tag at word {offset}"))?;
        if tag == 0 {
            return Ok(InstructionDecoded {
                value: "None".to_owned(),
                consumed: 1,
                type_label: label,
            });
        }
        if tag != 1 {
            bail!("invalid option tag {tag}");
        }
        let decoded = decode_instruction_type(inner, words, offset + 1, depth + 1)?;
        return Ok(InstructionDecoded {
            value: format!("Some({})", decoded.value),
            consumed: decoded.consumed + 1,
            type_label: label,
        });
    }

    if let Some((inner, len)) = fixed_array_type(ty)? {
        let mut cursor = offset;
        let mut values = Vec::with_capacity(len);
        for _ in 0..len {
            let decoded = decode_instruction_type(inner, words, cursor, depth + 1)?;
            cursor += decoded.consumed;
            values.push(decoded.value);
        }
        return Ok(InstructionDecoded {
            value: format!("[{}]", values.join(", ")),
            consumed: cursor - offset,
            type_label: label,
        });
    }

    bail!("unsupported instruction type {label}")
}

fn decode_instruction_primitive(
    ty: &str,
    words: &[u32],
    offset: usize,
) -> Result<InstructionDecoded> {
    let (value, consumed) = match ty {
        "bool" => (word_at(words, offset)? != 0).to_string().into_pair(1),
        "u8" | "u16" | "u32" => (word_at(words, offset)? as u128).to_string().into_pair(1),
        "i8" => {
            let signed = word_at(words, offset)? as i32;
            let value = i8::try_from(signed)
                .map_err(|_| anyhow::anyhow!("i8 value {signed} is out of range"))?;
            value.to_string().into_pair(1)
        }
        "i16" => {
            let signed = word_at(words, offset)? as i32;
            let value = i16::try_from(signed)
                .map_err(|_| anyhow::anyhow!("i16 value {signed} is out of range"))?;
            value.to_string().into_pair(1)
        }
        "i32" => (word_at(words, offset)? as i32).to_string().into_pair(1),
        "u64" => read_words_unsigned(words, offset, 2)?
            .to_string()
            .into_pair(2),
        "i64" => read_words_signed(words, offset, 2)?
            .to_string()
            .into_pair(2),
        "u128" => read_words_unsigned(words, offset, 4)?
            .to_string()
            .into_pair(4),
        "i128" => read_words_signed(words, offset, 4)?
            .to_string()
            .into_pair(4),
        "account_id" => {
            let (value, consumed) = decode_risc0_string(words, offset)?;
            let account_id = value
                .parse::<AccountId>()
                .with_context(|| format!("invalid account_id wire string `{value}`"))?;
            account_id.to_string().into_pair(consumed)
        }
        "program_id" => hex::encode(words_to_le_bytes(words_range(words, offset, 8)?)).into_pair(8),
        "string" | "String" => decode_risc0_string(words, offset)?,
        other => bail!("unsupported primitive instruction type `{other}`"),
    };

    Ok(InstructionDecoded {
        value,
        consumed,
        type_label: ty.to_owned(),
    })
}

fn decode_risc0_string(words: &[u32], offset: usize) -> Result<(String, usize)> {
    let len = usize::try_from(word_at(words, offset)?)
        .context("string byte length does not fit usize")?;
    let word_len = len.div_ceil(4);
    let mut bytes = words_to_le_bytes(words_range(words, offset + 1, word_len)?);
    bytes.truncate(len);
    let value = String::from_utf8(bytes).context("string arg is not valid UTF-8")?;
    Ok((value, 1 + word_len))
}

trait IntoPair {
    fn into_pair(self, consumed: usize) -> (String, usize);
}

impl IntoPair for String {
    fn into_pair(self, consumed: usize) -> (String, usize) {
        (self, consumed)
    }
}

fn word_at(words: &[u32], offset: usize) -> Result<u32> {
    words
        .get(offset)
        .copied()
        .with_context(|| format!("missing word {offset}"))
}

fn words_range(words: &[u32], offset: usize, count: usize) -> Result<&[u32]> {
    if offset
        .checked_add(count)
        .is_some_and(|end| end <= words.len())
    {
        let end = offset + count;
        words.get(offset..end).with_context(|| {
            format!("unexpected end of instruction data at word {offset}, need {count} words")
        })
    } else {
        bail!("unexpected end of instruction data at word {offset}, need {count} words")
    }
}

fn read_words_unsigned(words: &[u32], offset: usize, count: usize) -> Result<u128> {
    let words = words_range(words, offset, count)?;
    if count > 4 {
        bail!("cannot decode instruction integer wider than 128 bits");
    }
    let mut value = 0_u128;
    for (index, word) in words.iter().copied().enumerate() {
        value |= u128::from(word) << (32 * index);
    }
    Ok(value)
}

fn read_words_signed(words: &[u32], offset: usize, count: usize) -> Result<i128> {
    let words = words_range(words, offset, count)?;
    if count > 4 {
        bail!("cannot decode instruction integer wider than 128 bits");
    }
    let high_word = words
        .last()
        .copied()
        .context("cannot decode zero-width signed integer")?;
    let fill = if high_word & 0x8000_0000 == 0 {
        0_u32
    } else {
        u32::MAX
    };
    let mut fixed = [fill; 4];
    fixed
        .get_mut(..count)
        .context("cannot decode instruction integer wider than 128 bits")?
        .copy_from_slice(words);
    let bytes = words_to_le_bytes(&fixed);
    let mut fixed_bytes = [0_u8; 16];
    fixed_bytes.copy_from_slice(&bytes);
    Ok(i128::from_le_bytes(fixed_bytes))
}

fn words_to_le_bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}
