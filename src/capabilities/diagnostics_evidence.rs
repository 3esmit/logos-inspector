use serde_json::Value;

use super::{
    CapabilityRuntimeInputs,
    availability::{
        CapabilityState, all_sub_capabilities, append_unique, dedup_strings, loading_state,
        remove_unavailable, state_from_unavailable,
    },
    runtime_evidence, string_list,
};

pub(super) fn diagnostics_state(
    inputs: &CapabilityRuntimeInputs,
    sub_capabilities: &[&str],
) -> CapabilityState {
    let Some(report) = inputs.diagnostics_report() else {
        return loading_state(
            sub_capabilities,
            "Diagnostics runtime evidence has not loaded".to_owned(),
        );
    };
    let mut unavailable = all_sub_capabilities(sub_capabilities);
    let mut warnings = string_list(report.get("warnings"));
    let mut compact_errors = runtime_evidence::source_report_error(report);
    mark_diagnostics_runtime_evidence(report, &mut unavailable);
    append_unique(
        &mut unavailable,
        string_list(report.get("unavailable_sub_capabilities")),
    );
    append_diagnostics_report_constraints(
        report,
        &mut unavailable,
        &mut warnings,
        &mut compact_errors,
    );
    let mut state = state_from_unavailable(sub_capabilities, unavailable, warnings, compact_errors);
    if state.status == "unavailable" {
        state.status = "degraded";
    }
    state
}

fn mark_diagnostics_runtime_evidence(report: &Value, unavailable: &mut Vec<String>) {
    if report
        .get("module_reports")
        .and_then(Value::as_object)
        .is_some_and(|module_reports| {
            module_reports.values().any(|report| {
                report.is_object() && runtime_evidence::report_has_runtime_evidence(report)
            })
        })
    {
        for key in [
            "diagnostics.modules.status.read",
            "diagnostics.modules.info.read",
            "diagnostics.modules.metrics.read",
        ] {
            remove_unavailable(unavailable, key);
        }
    }

    if let Some(source_reports) = report.get("source_reports").and_then(Value::as_object) {
        for (key, source_report) in source_reports {
            if !source_report.is_object()
                || !runtime_evidence::report_has_runtime_evidence(source_report)
            {
                continue;
            }
            let Some(sub_capability) = diagnostics_source_sub_capability(key) else {
                continue;
            };
            remove_unavailable(unavailable, sub_capability);
            remove_unavailable(unavailable, "diagnostics.provider.probe");
        }
    }
}

fn append_diagnostics_report_constraints(
    report: &Value,
    unavailable: &mut Vec<String>,
    warnings: &mut Vec<String>,
    compact_errors: &mut Vec<String>,
) {
    if let Some(module_reports) = report.get("module_reports").and_then(Value::as_object) {
        for report in module_reports.values().filter(|value| value.is_object()) {
            if diagnostics_nested_report_unavailable(report) {
                append_unique(
                    unavailable,
                    vec![
                        "diagnostics.modules.status.read".to_owned(),
                        "diagnostics.modules.info.read".to_owned(),
                        "diagnostics.modules.metrics.read".to_owned(),
                    ],
                );
                append_unique(
                    warnings,
                    vec!["Module diagnostics report is unavailable".to_owned()],
                );
                append_unique(compact_errors, diagnostics_nested_report_errors(report));
            }
        }
    }

    if let Some(source_reports) = report.get("source_reports").and_then(Value::as_object) {
        for (key, source_report) in source_reports {
            if !source_report.is_object() || !diagnostics_nested_report_unavailable(source_report) {
                continue;
            }
            let Some(sub_capability) = diagnostics_source_sub_capability(key) else {
                continue;
            };
            append_unique(unavailable, vec![sub_capability.to_owned()]);
            append_unique(
                warnings,
                vec![format!(
                    "{} diagnostics report is unavailable",
                    diagnostics_source_label(key)
                )],
            );
            append_unique(
                compact_errors,
                diagnostics_nested_report_errors(source_report),
            );
        }
    }

    if let Some(last_known) = report.get("last_known").and_then(Value::as_object) {
        for (key, value) in last_known {
            let Some(detail) = value
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
            else {
                continue;
            };
            let Some(sub_capability) = diagnostics_last_known_sub_capability(key) else {
                continue;
            };
            append_unique(unavailable, vec![sub_capability.to_owned()]);
            append_unique(
                warnings,
                vec![format!(
                    "{} diagnostics are based on last-known error state",
                    diagnostics_source_label(key)
                )],
            );
            append_unique(compact_errors, vec![detail.to_owned()]);
        }
    }
}

fn diagnostics_nested_report_unavailable(report: &Value) -> bool {
    !string_list(report.get("unavailable_sub_capabilities")).is_empty()
        || runtime_evidence::report_has_probe_failure(report)
        || module_info_failed(report)
        || report_health_unavailable(report)
}

fn diagnostics_nested_report_errors(report: &Value) -> Vec<String> {
    let mut errors = runtime_evidence::source_report_error(report);
    if let Some(module_info) = report.get("module_info").filter(|value| value.is_object()) {
        runtime_evidence::push_error_text(&mut errors, module_info.get("error"));
    }
    dedup_strings(errors)
}

fn module_info_failed(report: &Value) -> bool {
    report
        .get("module_info")
        .filter(|value| value.is_object())
        .and_then(|module_info| module_info.get("ok"))
        .and_then(Value::as_bool)
        == Some(false)
}

fn report_health_unavailable(report: &Value) -> bool {
    let Some(health) = report.get("health").filter(|value| value.is_object()) else {
        return false;
    };
    if health.get("ready").and_then(Value::as_bool) == Some(false) {
        return true;
    }
    let status = health
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        status.as_str(),
        "degraded" | "error" | "failed" | "unavailable" | "unsupported"
    )
}

fn diagnostics_source_sub_capability(key: &str) -> Option<&'static str> {
    match key {
        "l1" | "blockchain" | "node" => Some("diagnostics.l1.read"),
        "lez.indexer" | "indexer" | "lez_indexer" => Some("diagnostics.lez.indexer.read"),
        "lez.sequencer" | "sequencer" | "execution" | "lez_sequencer" => {
            Some("diagnostics.lez.sequencer.read")
        }
        "storage" | "storage_source" => Some("diagnostics.storage.read"),
        "delivery" | "delivery_source" | "messaging" | "messaging_source" => {
            Some("diagnostics.delivery.read")
        }
        _ => None,
    }
}

fn diagnostics_last_known_sub_capability(key: &str) -> Option<&'static str> {
    match key {
        "wallet" => Some("diagnostics.wallet.read"),
        "local_nodes" | "localNodes" => Some("diagnostics.local_nodes.read"),
        _ => diagnostics_source_sub_capability(key),
    }
}

fn diagnostics_source_label(key: &str) -> &'static str {
    match key {
        "l1" | "blockchain" | "node" => "L1",
        "lez.indexer" | "indexer" | "lez_indexer" => "LEZ Indexer",
        "lez.sequencer" | "sequencer" | "execution" | "lez_sequencer" => "LEZ Sequencer",
        "storage" | "storage_source" => "Storage",
        "delivery" | "delivery_source" | "messaging" | "messaging_source" => "Delivery",
        "wallet" => "Wallet",
        "local_nodes" | "localNodes" => "Local Nodes",
        _ => "Provider",
    }
}
