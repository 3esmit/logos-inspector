use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct ProbeField {
    pub ok: bool,
    pub value: Option<Value>,
    pub error: Option<String>,
}

impl ProbeField {
    pub(crate) fn ok(value: impl Serialize) -> Self {
        Self {
            ok: true,
            value: Some(serde_json::to_value(value).unwrap_or(Value::Null)),
            error: None,
        }
    }

    pub(crate) fn err(error: impl std::fmt::Display) -> Self {
        Self {
            ok: false,
            value: None,
            error: Some(error.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProbeReport {
    pub label: String,
    pub source: String,
    pub ok: bool,
    pub value: Option<Value>,
    pub error: Option<String>,
}

impl ProbeReport {
    pub fn ok(label: impl Into<String>, source: impl Into<String>, value: impl Serialize) -> Self {
        let label = label.into();
        let source = source.into();
        match serde_json::to_value(value) {
            Ok(value) => Self {
                label,
                source,
                ok: true,
                value: Some(value),
                error: None,
            },
            Err(error) => Self::err(label, source, error),
        }
    }

    pub fn err(
        label: impl Into<String>,
        source: impl Into<String>,
        error: impl std::fmt::Display,
    ) -> Self {
        Self {
            label: label.into(),
            source: source.into(),
            ok: false,
            value: None,
            error: Some(error.to_string()),
        }
    }

    pub fn from_result<T, E>(
        label: impl Into<String>,
        source: impl Into<String>,
        result: Result<T, E>,
    ) -> Self
    where
        T: Serialize,
        E: std::fmt::Display,
    {
        let label = label.into();
        let source = source.into();
        match result {
            Ok(value) => Self::ok(label, source, value),
            Err(error) => Self::err(label, source, error),
        }
    }
}
