use serde_json::Value;

pub(crate) fn enum_payload(value: &Value) -> (&str, &Value) {
    if let Some(object) = value.as_object()
        && object.len() == 1
        && let Some((kind, payload)) = object.iter().next()
    {
        return (kind, payload);
    }
    ("Unknown", value)
}

pub(crate) fn value_list_strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items.iter().map(value_to_string).collect(),
        Some(Value::String(value)) => split_list_string(value),
        Some(value) => vec![value_to_string(value)],
        None => Vec::new(),
    }
}

fn split_list_string(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Null => "null".to_owned(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}
