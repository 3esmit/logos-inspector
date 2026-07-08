use anyhow::{Context as _, Result, bail};
use lee::AccountId;
use serde_json::{Map, Value, json};

use super::idl_type::{fixed_array_type, idl_type_label};

pub(crate) struct DecodedValue {
    pub(crate) value: Value,
    pub(crate) consumed: usize,
}

pub(crate) fn decode_borsh_shape(
    shape: &Value,
    bytes: &[u8],
    offset: usize,
    idl: &Value,
    depth: usize,
) -> Result<DecodedValue> {
    if depth > 32 {
        bail!("IDL nesting too deep");
    }

    match shape.get("kind").and_then(Value::as_str) {
        Some("struct") => decode_borsh_struct(shape, bytes, offset, idl, depth),
        Some("enum") => decode_borsh_enum(shape, bytes, offset, idl, depth),
        Some(kind) => bail!("unsupported IDL shape kind `{kind}`"),
        None => bail!("IDL shape missing kind"),
    }
}

fn decode_borsh_struct(
    shape: &Value,
    bytes: &[u8],
    offset: usize,
    idl: &Value,
    depth: usize,
) -> Result<DecodedValue> {
    let fields = shape
        .get("fields")
        .and_then(Value::as_array)
        .context("struct shape has no fields array")?;
    let mut cursor = offset;
    let mut object = Map::new();

    for (index, field) in fields.iter().enumerate() {
        let name = field
            .get("name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("field_{index}"));
        let ty = field
            .get("type")
            .with_context(|| format!("field `{name}` has no type"))?;
        let decoded = decode_borsh_type(ty, bytes, cursor, idl, depth + 1)
            .with_context(|| format!("failed to decode field `{name}`"))?;
        cursor += decoded.consumed;
        object.insert(name, decoded.value);
    }

    Ok(DecodedValue {
        value: Value::Object(object),
        consumed: cursor - offset,
    })
}

fn decode_borsh_enum(
    shape: &Value,
    bytes: &[u8],
    offset: usize,
    idl: &Value,
    depth: usize,
) -> Result<DecodedValue> {
    let variant_index = usize::from(byte_at(bytes, offset)?);
    let variants = shape
        .get("variants")
        .and_then(Value::as_array)
        .context("enum shape has no variants array")?;
    let variant = variants
        .get(variant_index)
        .with_context(|| format!("enum variant {variant_index} not present"))?;
    let variant_name = variant
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let mut cursor = offset + 1;
    let mut fields = Map::new();

    for (index, field) in variant
        .get("fields")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let name = field
            .get("name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("field_{index}"));
        let ty = field.get("type").unwrap_or(field);
        let decoded = decode_borsh_type(ty, bytes, cursor, idl, depth + 1)
            .with_context(|| format!("failed to decode variant field `{name}`"))?;
        cursor += decoded.consumed;
        fields.insert(name, decoded.value);
    }

    Ok(DecodedValue {
        value: json!({
            "variant": variant_name,
            "variant_index": variant_index,
            "fields": fields,
        }),
        consumed: cursor - offset,
    })
}

pub(crate) fn decode_borsh_type(
    ty: &Value,
    bytes: &[u8],
    offset: usize,
    idl: &Value,
    depth: usize,
) -> Result<DecodedValue> {
    if depth > 32 {
        bail!("IDL nesting too deep");
    }

    if let Some(primitive) = ty.as_str() {
        return decode_borsh_primitive(primitive, bytes, offset);
    }

    let object = ty
        .as_object()
        .with_context(|| format!("unsupported IDL type {}", idl_type_label(ty)))?;

    if let Some(inner) = object.get("option") {
        let tag = byte_at(bytes, offset)?;
        if tag == 0 {
            return Ok(DecodedValue {
                value: Value::Null,
                consumed: 1,
            });
        }
        if tag != 1 {
            bail!("invalid option tag {tag}");
        }
        let decoded = decode_borsh_type(inner, bytes, offset + 1, idl, depth + 1)?;
        return Ok(DecodedValue {
            value: decoded.value,
            consumed: decoded.consumed + 1,
        });
    }

    if let Some(inner) = object.get("vec") {
        let len = read_le_unsigned(bytes, offset, 4)?;
        let len = usize::try_from(len).context("vector length does not fit usize")?;
        if len > 100_000 {
            bail!("vector length too large: {len}");
        }
        let mut cursor = offset + 4;
        let mut values = Vec::with_capacity(len);
        for _ in 0..len {
            let decoded = decode_borsh_type(inner, bytes, cursor, idl, depth + 1)?;
            cursor += decoded.consumed;
            values.push(decoded.value);
        }
        return Ok(DecodedValue {
            value: Value::Array(values),
            consumed: cursor - offset,
        });
    }

    if let Some((inner, len)) = fixed_array_type(ty)? {
        let mut cursor = offset;
        let mut values = Vec::with_capacity(len);
        for _ in 0..len {
            let decoded = decode_borsh_type(inner, bytes, cursor, idl, depth + 1)?;
            cursor += decoded.consumed;
            values.push(decoded.value);
        }
        return Ok(DecodedValue {
            value: Value::Array(values),
            consumed: cursor - offset,
        });
    }

    if let Some(name) = object.get("defined").and_then(Value::as_str) {
        let shape = find_defined_shape(idl, name)
            .with_context(|| format!("defined IDL type `{name}` not found"))?;
        return decode_borsh_shape(shape, bytes, offset, idl, depth + 1);
    }

    if object.contains_key("kind") {
        return decode_borsh_shape(ty, bytes, offset, idl, depth + 1);
    }

    bail!("unsupported IDL type {}", idl_type_label(ty))
}

fn decode_borsh_primitive(ty: &str, bytes: &[u8], offset: usize) -> Result<DecodedValue> {
    let (value, consumed) = match ty {
        "bool" => (Value::Bool(byte_at(bytes, offset)? != 0), 1),
        "u8" => (Value::String(byte_at(bytes, offset)?.to_string()), 1),
        "i8" => {
            let value = i8::from_le_bytes([byte_at(bytes, offset)?]);
            (Value::String(value.to_string()), 1)
        }
        "u16" => (
            Value::String(read_le_unsigned(bytes, offset, 2)?.to_string()),
            2,
        ),
        "i16" => (
            Value::String(read_le_signed(bytes, offset, 2)?.to_string()),
            2,
        ),
        "u32" => (
            Value::String(read_le_unsigned(bytes, offset, 4)?.to_string()),
            4,
        ),
        "i32" => (
            Value::String(read_le_signed(bytes, offset, 4)?.to_string()),
            4,
        ),
        "u64" => (
            Value::String(read_le_unsigned(bytes, offset, 8)?.to_string()),
            8,
        ),
        "i64" => (
            Value::String(read_le_signed(bytes, offset, 8)?.to_string()),
            8,
        ),
        "u128" => (
            Value::String(read_le_unsigned(bytes, offset, 16)?.to_string()),
            16,
        ),
        "i128" => (
            Value::String(read_le_signed(bytes, offset, 16)?.to_string()),
            16,
        ),
        "account_id" => (
            Value::String(account_id_base58(bytes_range(bytes, offset, 32)?)),
            32,
        ),
        "program_id" => (
            Value::String(hex::encode(bytes_range(bytes, offset, 32)?)),
            32,
        ),
        "string" => {
            let len = read_le_unsigned(bytes, offset, 4)?;
            let len = usize::try_from(len).context("string length does not fit usize")?;
            let value = std::str::from_utf8(bytes_range(bytes, offset + 4, len)?)
                .context("string field is not valid UTF-8")?;
            (Value::String(value.to_owned()), 4 + len)
        }
        other => bail!("unsupported primitive IDL type `{other}`"),
    };

    Ok(DecodedValue { value, consumed })
}

fn find_defined_shape<'a>(idl: &'a Value, name: &str) -> Option<&'a Value> {
    idl.get("types")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|item| item.get("name").and_then(Value::as_str) == Some(name))
        .or_else(|| {
            idl.get("accounts")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .find(|item| item.get("name").and_then(Value::as_str) == Some(name))
                .and_then(|item| item.get("type"))
        })
}

pub(crate) fn parse_hex_bytes(value: &str) -> Result<Vec<u8>> {
    let mut hex = value.trim();
    if let Some(stripped) = hex.strip_prefix("0x").or_else(|| hex.strip_prefix("0X")) {
        hex = stripped;
    }
    let hex = hex
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    if hex.len() % 2 != 0 {
        bail!("hex string must have even length");
    }
    hex::decode(hex).context("invalid hex")
}

fn byte_at(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes
        .get(offset)
        .copied()
        .with_context(|| format!("unexpected end of data at byte {offset}, need 1 byte"))
}

fn bytes_range(bytes: &[u8], offset: usize, count: usize) -> Result<&[u8]> {
    if offset
        .checked_add(count)
        .is_some_and(|end| end <= bytes.len())
    {
        let end = offset + count;
        bytes
            .get(offset..end)
            .with_context(|| format!("unexpected end of data at byte {offset}, need {count} bytes"))
    } else {
        bail!("unexpected end of data at byte {offset}, need {count} bytes")
    }
}

fn read_le_unsigned(bytes: &[u8], offset: usize, count: usize) -> Result<u128> {
    let bytes = bytes_range(bytes, offset, count)?;
    if count > 16 {
        bail!("cannot decode unsigned integer wider than 128 bits");
    }
    let mut value = 0_u128;
    for (index, byte) in bytes.iter().copied().enumerate() {
        value |= u128::from(byte) << (8 * index);
    }
    Ok(value)
}

fn read_le_signed(bytes: &[u8], offset: usize, count: usize) -> Result<i128> {
    let bytes = bytes_range(bytes, offset, count)?;
    if count > 16 {
        bail!("cannot decode signed integer wider than 128 bits");
    }
    let high_byte = bytes
        .last()
        .copied()
        .context("cannot decode zero-width signed integer")?;
    let mut fixed = if high_byte & 0x80 == 0 {
        [0_u8; 16]
    } else {
        [0xff_u8; 16]
    };
    fixed
        .get_mut(..count)
        .context("cannot decode signed integer wider than 128 bits")?
        .copy_from_slice(bytes);
    Ok(i128::from_le_bytes(fixed))
}

fn account_id_base58(bytes: &[u8]) -> String {
    let mut fixed = [0_u8; 32];
    fixed.copy_from_slice(bytes);
    AccountId::new(fixed).to_string()
}
