use anyhow::{Context as _, Result, bail};
use serde_json::Value;

pub(crate) fn idl_type_label(ty: &Value) -> String {
    if let Some(value) = ty.as_str() {
        return value.to_owned();
    }
    if let Some(inner) = ty.get("option") {
        return format!("option<{}>", idl_type_label(inner));
    }
    if let Some(inner) = ty.get("vec") {
        return format!("vec<{}>", idl_type_label(inner));
    }
    if let Ok(Some((inner, len))) = fixed_array_type(ty) {
        return format!("array<{}, {len}>", idl_type_label(inner));
    }
    if let Some(name) = ty.get("defined").and_then(Value::as_str) {
        return name.to_owned();
    }
    ty.to_string()
}

pub(crate) fn fixed_array_type(ty: &Value) -> Result<Option<(&Value, usize)>> {
    let Some(array) = ty.get("array") else {
        return Ok(None);
    };
    match array {
        Value::Array(items) => {
            if items.len() != 2 {
                bail!("array type must be [element_type, len]");
            }
            let inner = items
                .first()
                .context("array type missing element type after length check")?;
            let len_value = items
                .get(1)
                .context("array type missing length after length check")?;
            Ok(Some((inner, array_len(len_value)?)))
        }
        Value::Object(array_object) => {
            let inner = array_object
                .get("type")
                .or_else(|| array_object.get("inner"))
                .or_else(|| array_object.get("element"))
                .context("array type object missing element type")?;
            let len_value = array_object
                .get("len")
                .or_else(|| array_object.get("length"))
                .context("array type object missing length")?;
            Ok(Some((inner, array_len(len_value)?)))
        }
        _ => bail!("array type must be [element_type, len] or object"),
    }
}

fn array_len(value: &Value) -> Result<usize> {
    let len = value
        .as_u64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
        .context("array length must be an unsigned integer")?;
    let len = usize::try_from(len).context("array length does not fit usize")?;
    if len > 100_000 {
        bail!("array length too large: {len}");
    }
    Ok(len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::ensure;
    use serde_json::json;

    #[test]
    fn fixed_array_type_accepts_tuple_and_object_shapes() -> Result<()> {
        let tuple_shape = json!({ "array": ["u8", 3] });
        let object_shape = json!({ "array": { "type": "u32", "len": "2" } });

        ensure!(
            fixed_array_type(&tuple_shape)?.map(|(_, len)| len) == Some(3),
            "tuple array length mismatch"
        );
        ensure!(
            fixed_array_type(&object_shape)?.map(|(_, len)| len) == Some(2),
            "object array length mismatch"
        );
        ensure!(
            idl_type_label(&object_shape) == "array<u32, 2>",
            "object array label mismatch"
        );
        Ok(())
    }
}
