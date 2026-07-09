use serde_json::Value;

use super::{
    CapabilityRuntimeInputs,
    availability::{
        CapabilityState, all_sub_capabilities, dedup_strings, loading_state,
        state_from_unavailable, unavailable_state,
    },
    string_list,
};

pub(super) fn source_report_state(
    inputs: &CapabilityRuntimeInputs,
    scope: &str,
    label: &str,
    sub_capabilities: &[&str],
) -> CapabilityState {
    let Some(report) = inputs.source_report_for(scope) else {
        return loading_state(
            sub_capabilities,
            format!("{label} provider probe has not run"),
        );
    };
    let explicit_unavailable = string_list(report.get("unavailable_sub_capabilities"));
    let compact_error = source_report_error(report);
    let health = report.get("health").filter(|value| value.is_object());
    let ready = health
        .and_then(|value| value.get("ready"))
        .and_then(Value::as_bool);
    let reachable = health
        .and_then(|value| value.get("reachable"))
        .and_then(Value::as_bool);
    let status = health
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if ready == Some(true) || status == "healthy" || status == "ready" {
        return state_from_unavailable(
            sub_capabilities,
            explicit_unavailable,
            Vec::new(),
            Vec::new(),
        );
    }
    if reachable == Some(true) || status == "degraded" {
        let unavailable = if explicit_unavailable.is_empty() {
            all_sub_capabilities(sub_capabilities)
        } else {
            explicit_unavailable
        };
        return state_from_unavailable(
            sub_capabilities,
            unavailable,
            compact_error.clone(),
            compact_error,
        );
    }
    if health.is_some() || report_has_probe_failure(report) {
        return unavailable_state(
            sub_capabilities,
            compact_error
                .first()
                .cloned()
                .unwrap_or_else(|| format!("{label} provider probe failed")),
        );
    }
    loading_state(
        sub_capabilities,
        format!("{label} provider probe has no health result"),
    )
}

pub(super) fn report_has_probe_failure(report: &Value) -> bool {
    ["probe_facts", "probes"].iter().any(|field| {
        report
            .get(*field)
            .and_then(Value::as_array)
            .is_some_and(|rows| {
                rows.iter()
                    .any(|row| row.get("ok").and_then(Value::as_bool) == Some(false))
            })
    })
}

pub(super) fn report_has_runtime_evidence(value: &Value) -> bool {
    if !value.is_object() {
        return false;
    }
    if value.get("health").is_some_and(|health| health.is_object())
        || value
            .get("unavailable_sub_capabilities")
            .is_some_and(Value::is_array)
        || value.get("last_known").is_some_and(value_has_content)
        || report_has_probe_failure(value)
    {
        return true;
    }
    value.as_object().is_some_and(|object| {
        object
            .values()
            .any(|item| item.is_object() && report_has_runtime_evidence(item))
    })
}

pub(super) fn source_report_error(report: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    if let Some(health) = report.get("health").filter(|value| value.is_object()) {
        push_error_text(&mut errors, health.get("detail"));
        push_error_text(&mut errors, health.get("summary"));
    }
    for field in ["probe_facts", "probes"] {
        if let Some(rows) = report.get(field).and_then(Value::as_array) {
            for row in rows {
                if row.get("ok").and_then(Value::as_bool) == Some(false) {
                    push_error_text(&mut errors, row.get("error"));
                }
            }
        }
    }
    dedup_strings(errors)
}

pub(super) fn push_error_text(errors: &mut Vec<String>, value: Option<&Value>) {
    let Some(text) = value.and_then(Value::as_str).map(str::trim) else {
        return;
    };
    if !text.is_empty() {
        errors.push(text.to_owned());
    }
}

fn value_has_content(value: &Value) -> bool {
    match value {
        Value::String(text) => !text.trim().is_empty(),
        Value::Array(values) => values.iter().any(value_has_content),
        Value::Object(values) => values.values().any(value_has_content),
        _ => false,
    }
}
