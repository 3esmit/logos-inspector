#[derive(Debug, Clone)]
pub(super) struct CapabilityState {
    pub(super) status: &'static str,
    pub(super) unavailable_sub_capabilities: Vec<String>,
    pub(super) warnings: Vec<String>,
    pub(super) compact_errors: Vec<String>,
}

pub(super) fn available_state() -> CapabilityState {
    CapabilityState {
        status: "available",
        unavailable_sub_capabilities: Vec::new(),
        warnings: Vec::new(),
        compact_errors: Vec::new(),
    }
}

pub(super) fn loading_state(sub_capabilities: &[&str], detail: String) -> CapabilityState {
    CapabilityState {
        status: "loading",
        unavailable_sub_capabilities: all_sub_capabilities(sub_capabilities),
        warnings: Vec::new(),
        compact_errors: vec![detail],
    }
}

pub(super) fn state_from_unavailable(
    sub_capabilities: &[&str],
    unavailable_sub_capabilities: Vec<String>,
    warnings: Vec<String>,
    compact_errors: Vec<String>,
) -> CapabilityState {
    let status = if unavailable_sub_capabilities.is_empty() {
        "available"
    } else if unavailable_sub_capabilities.len() >= sub_capabilities.len() {
        "unavailable"
    } else {
        "degraded"
    };
    CapabilityState {
        status,
        unavailable_sub_capabilities,
        warnings,
        compact_errors,
    }
}

pub(super) fn input_required_state(sub_capabilities: &[&str], error: String) -> CapabilityState {
    CapabilityState {
        status: "input_required",
        unavailable_sub_capabilities: all_sub_capabilities(sub_capabilities),
        warnings: Vec::new(),
        compact_errors: vec![error],
    }
}

pub(super) fn unavailable_state(sub_capabilities: &[&str], error: String) -> CapabilityState {
    CapabilityState {
        status: "unavailable",
        unavailable_sub_capabilities: all_sub_capabilities(sub_capabilities),
        warnings: Vec::new(),
        compact_errors: vec![error],
    }
}

pub(super) fn merge_state_constraints(
    mut state: CapabilityState,
    unavailable: Vec<String>,
    warnings: Vec<String>,
    compact_errors: Vec<String>,
) -> CapabilityState {
    append_unique(&mut state.unavailable_sub_capabilities, unavailable);
    append_unique(&mut state.warnings, warnings);
    append_unique(&mut state.compact_errors, compact_errors);
    if state.status == "available" && !state.unavailable_sub_capabilities.is_empty() {
        state.status = "degraded";
    }
    state
}

pub(super) fn all_sub_capabilities(sub_capabilities: &[&str]) -> Vec<String> {
    sub_capabilities
        .iter()
        .map(|capability| (*capability).to_owned())
        .collect()
}

pub(super) fn capability_state_usable(state: &CapabilityState) -> bool {
    matches!(state.status, "available" | "degraded")
}

pub(super) fn state_marks_unavailable(state: &CapabilityState, capability: &str) -> bool {
    state
        .unavailable_sub_capabilities
        .iter()
        .any(|unavailable| unavailable == capability)
}

pub(super) fn remove_unavailable(target: &mut Vec<String>, key: &str) {
    target.retain(|value| value != key);
}

pub(super) fn append_unique(target: &mut Vec<String>, incoming: Vec<String>) {
    for value in incoming {
        if !value.is_empty() && !target.iter().any(|current| current == &value) {
            target.push(value);
        }
    }
}

pub(super) fn dedup_strings(values: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    append_unique(&mut result, values);
    result
}
