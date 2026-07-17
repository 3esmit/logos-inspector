use anyhow::{Context as _, Result, bail};
use serde_json::Value;

pub(crate) struct Args {
    values: Vec<Value>,
}

impl Args {
    pub(crate) fn new(value: Value) -> Result<Self> {
        let values = value
            .as_array()
            .context("bridge args must be a JSON array")?
            .clone();
        Ok(Self { values })
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Value> {
        self.values.iter()
    }

    pub(crate) fn value(&self, index: usize) -> Option<&Value> {
        self.values.get(index)
    }

    pub(crate) fn string(&self, index: usize, label: &str) -> Result<&str> {
        self.value(index)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .with_context(|| format!("{label} is required"))
    }

    pub(crate) fn optional_string(&self, index: usize) -> Option<&str> {
        self.value(index)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub(crate) fn optional_bool(&self, index: usize) -> bool {
        match self.value(index) {
            Some(Value::Bool(value)) => *value,
            Some(Value::String(value)) => matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            ),
            _ => false,
        }
    }

    pub(crate) fn canonical_decimal_u64(&self, index: usize, label: &str) -> Result<u64> {
        let value = self
            .value(index)
            .with_context(|| format!("{label} is required"))?;
        if let Some(value) = value.as_u64() {
            return Ok(value);
        }
        let Some(raw) = value.as_str() else {
            bail!("invalid {label}");
        };
        let canonical = raw == "0"
            || (!raw.is_empty()
                && !raw.starts_with('0')
                && raw.bytes().all(|byte| byte.is_ascii_digit()));
        if !canonical {
            bail!("invalid {label}");
        }
        raw.parse::<u64>()
            .with_context(|| format!("invalid {label}"))
    }

    pub(crate) fn json_or_empty_array(&self, index: usize) -> Result<Value> {
        let Some(value) = self.value(index) else {
            return Ok(Value::Array(vec![]));
        };
        match value {
            Value::String(raw) if raw.trim().is_empty() => Ok(Value::Array(vec![])),
            Value::String(raw) => {
                serde_json::from_str(raw).context("failed to parse JSON argument")
            }
            value => Ok(value.clone()),
        }
    }
}
