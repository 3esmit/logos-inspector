use std::collections::BTreeMap;

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterConnectionType {
    Module,
    Rpc,
    Rest,
    Metrics,
    NetworkMonitor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AdapterInputPolicy {
    pub key: &'static str,
    pub label: &'static str,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SourceAdapterPolicy {
    pub connector_id: &'static str,
    pub connection_type: AdapterConnectionType,
    pub target: &'static str,
    pub module_id: Option<&'static str>,
    pub inputs: &'static [AdapterInputPolicy],
    pub capabilities: &'static [&'static str],
    pub supports_cid_probe: bool,
    pub supports_mutating_diagnostics: bool,
    #[serde(skip)]
    pub capability_scopes: &'static [&'static str],
    #[serde(skip)]
    pub endpoint_role: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceModePolicy {
    pub key: &'static str,
    pub aliases: &'static [&'static str],
    pub effective: &'static str,
    pub label_key: &'static str,
    pub label: &'static str,
    pub source_label: &'static str,
    pub summary: &'static str,
    pub implemented: bool,
    pub adapter: SourceAdapterPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct AdapterInitialization {
    #[serde(default)]
    source_mode: String,
    #[serde(default)]
    inputs: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagedNodeAction {
    Initialize,
    Start,
    Stop,
    Destroy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagedModuleCallSpec {
    pub(crate) method: &'static str,
    pub(crate) signature: &'static str,
    pub(crate) args: Vec<String>,
}

impl ManagedModuleCallSpec {
    #[must_use]
    pub(crate) fn new(method: &'static str, signature: &'static str, args: Vec<String>) -> Self {
        Self {
            method,
            signature,
            args,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagedLifecycleOutcome {
    pub(crate) success: bool,
    pub(crate) detail: String,
}

impl AdapterInitialization {
    pub(crate) fn parse(
        value: &Value,
        modes: &'static [SourceModePolicy],
        default_mode: &str,
    ) -> Result<Self> {
        let mut initialization: Self = serde_json::from_value(value.clone())
            .context("adapter initialization must be an object")?;
        let requested_mode = if initialization.source_mode.trim().is_empty() {
            default_mode
        } else {
            initialization.source_mode.trim()
        };
        let mode = mode_for_token(modes, requested_mode)
            .with_context(|| format!("unsupported source mode `{requested_mode}`"))?;
        initialization.source_mode = mode.key.to_owned();
        initialization.normalize_inputs(mode)?;
        Ok(initialization)
    }

    #[must_use]
    pub(crate) fn source_mode(&self) -> &str {
        &self.source_mode
    }

    #[must_use]
    pub(crate) fn input(&self, key: &str) -> Option<&str> {
        self.inputs
            .get(key)
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    fn normalize_inputs(&mut self, mode: &SourceModePolicy) -> Result<()> {
        for key in self.inputs.keys() {
            if !mode.adapter.inputs.iter().any(|input| input.key == key) {
                bail!(
                    "adapter `{}` does not accept input `{key}`",
                    mode.adapter.connector_id
                );
            }
        }
        for input in mode.adapter.inputs {
            if input.required && self.input(input.key).is_none() {
                bail!("{} is required", input.label);
            }
        }
        self.inputs.retain(|_, value| !value.trim().is_empty());
        Ok(())
    }
}

#[must_use]
pub(crate) fn adapter_for_connector(
    mode_families: &[&'static [SourceModePolicy]],
    connector_id: &str,
) -> Option<&'static SourceAdapterPolicy> {
    mode_families
        .iter()
        .flat_map(|modes| modes.iter())
        .find(|mode| mode.adapter.connector_id == connector_id)
        .map(|mode| &mode.adapter)
}

fn mode_for_token(
    modes: &'static [SourceModePolicy],
    value: &str,
) -> Option<&'static SourceModePolicy> {
    let value = value.trim().to_ascii_lowercase();
    modes
        .iter()
        .find(|mode| mode.key == value || mode.aliases.contains(&value.as_str()))
}

#[cfg(test)]
pub(crate) mod contract_tests {
    use std::{collections::BTreeSet, fs, path::Path};

    use serde_json::{Value, json};

    use super::{
        AdapterConnectionType, AdapterInitialization, ManagedModuleCallSpec, ManagedNodeAction,
        SourceAdapterPolicy, SourceModePolicy,
    };

    const MANAGED_ACTIONS: &[ManagedNodeAction] = &[
        ManagedNodeAction::Initialize,
        ManagedNodeAction::Start,
        ManagedNodeAction::Stop,
        ManagedNodeAction::Destroy,
    ];

    pub(crate) fn assert_layer_contract(key: &str, modes: &'static [SourceModePolicy]) {
        assert!(modes.len() >= 2, "{} must expose a real adapter seam", key);

        let mut mode_keys = BTreeSet::new();
        let mut connector_ids = BTreeSet::new();
        for mode in modes {
            assert!(mode_keys.insert(mode.key), "duplicate mode `{}`", mode.key);
            assert!(
                connector_ids.insert(mode.adapter.connector_id),
                "duplicate connector `{}`",
                mode.adapter.connector_id
            );
            assert_adapter_contract(&mode.adapter);
            if mode.implemented {
                assert!(
                    !mode.adapter.capabilities.is_empty(),
                    "implemented adapter `{}` must declare capabilities",
                    mode.adapter.connector_id
                );
            }
            assert_initialization_contract(mode, modes);
        }
    }

    pub(crate) fn assert_managed_module_contract(
        key: &str,
        module_id: &str,
        supported_actions: &[ManagedNodeAction],
        call_spec: impl Fn(ManagedNodeAction, &str) -> Option<ManagedModuleCallSpec>,
    ) {
        assert!(!module_id.trim().is_empty(), "{key} module id is empty");
        for action in MANAGED_ACTIONS {
            let spec = call_spec(*action, "/tmp/node.json");
            assert_eq!(
                spec.is_some(),
                supported_actions.contains(action),
                "{key} action support mismatch for {action:?}"
            );
            if let Some(spec) = spec {
                assert_call_signature(key, &spec);
            }
        }
    }

    fn assert_call_signature(key: &str, spec: &ManagedModuleCallSpec) {
        assert!(!spec.method.is_empty(), "{key} method is empty");
        assert!(
            spec.signature.contains('('),
            "{key} signature has no opening parenthesis"
        );
        let Some((signature_method, parameters)) = spec.signature.split_once('(') else {
            return;
        };
        assert_eq!(
            signature_method, spec.method,
            "{key} method/signature drift"
        );
        assert!(
            parameters.ends_with(')'),
            "{key} signature has no closing parenthesis"
        );
        let Some(parameters) = parameters.strip_suffix(')') else {
            return;
        };
        let parameter_count = if parameters.is_empty() {
            0
        } else {
            parameters.split(',').count()
        };
        assert_eq!(
            parameter_count,
            spec.args.len(),
            "{key} call argument/signature arity drift"
        );
    }

    fn assert_initialization_contract(mode: &SourceModePolicy, modes: &'static [SourceModePolicy]) {
        let inputs = mode
            .adapter
            .inputs
            .iter()
            .map(|input| {
                (
                    input.key.to_owned(),
                    Value::String(format!("{}-value", input.key)),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let value = json!({
            "source_mode": mode.key,
            "inputs": inputs,
        });
        let result = AdapterInitialization::parse(&value, modes, mode.key);
        assert!(
            result.is_ok(),
            "adapter `{}` rejected valid inputs: {result:?}",
            mode.key
        );
        let Ok(initialization) = result else {
            return;
        };
        assert_eq!(initialization.source_mode(), mode.key);

        let unknown = json!({
            "source_mode": mode.key,
            "inputs": { "unknown_input": "value" },
        });
        assert!(AdapterInitialization::parse(&unknown, modes, mode.key).is_err());

        if mode.adapter.inputs.iter().any(|input| input.required) {
            let missing = json!({ "source_mode": mode.key, "inputs": {} });
            assert!(AdapterInitialization::parse(&missing, modes, mode.key).is_err());
        }
    }

    fn assert_adapter_contract(adapter: &SourceAdapterPolicy) {
        let keys: Vec<&str> = adapter.inputs.iter().map(|input| input.key).collect();
        let required: Vec<&str> = adapter
            .inputs
            .iter()
            .filter(|input| input.required)
            .map(|input| input.key)
            .collect();
        let unique: BTreeSet<&str> = keys.iter().copied().collect();
        assert_eq!(unique.len(), keys.len(), "duplicate adapter input key");

        match adapter.connection_type {
            AdapterConnectionType::Module => {
                assert!(keys.is_empty(), "module adapters take no user input");
                assert_eq!(adapter.target, "module");
                assert!(adapter.module_id.is_some(), "module id is layer-owned");
            }
            AdapterConnectionType::Rpc => {
                assert_eq!(keys, ["rpc_endpoint"]);
                assert_eq!(required, ["rpc_endpoint"]);
                assert_eq!(adapter.target, "rpc_endpoint");
                assert!(adapter.module_id.is_none());
            }
            AdapterConnectionType::Rest | AdapterConnectionType::NetworkMonitor => {
                assert_eq!(keys.first().copied(), Some("rest_endpoint"));
                assert_eq!(required, ["rest_endpoint"]);
                assert!(keys == ["rest_endpoint"] || keys == ["rest_endpoint", "metrics_endpoint"]);
                assert_eq!(adapter.target, "rest_endpoint");
                assert!(adapter.module_id.is_none());
            }
            AdapterConnectionType::Metrics => {
                assert_eq!(keys, ["metrics_endpoint"]);
                assert_eq!(required, ["metrics_endpoint"]);
                assert_eq!(adapter.target, "metrics_endpoint");
                assert!(adapter.module_id.is_none());
            }
        }
    }

    #[test]
    fn supplemental_source_scan_finds_no_node_transport_bypasses() {
        let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut violations = Vec::new();
        inspect_source_tree(&source_root, &mut violations);

        assert!(
            violations.is_empty(),
            "node transport calls bypass adapter layers:\n{}",
            violations.join("\n")
        );
    }

    fn inspect_source_tree(path: &Path, violations: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(path) else {
            violations.push(format!("cannot read {}", path.display()));
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                inspect_source_tree(&path, violations);
                continue;
            }
            if path.extension().and_then(|extension| extension.to_str()) != Some("rs")
                || allowed_layer_path(&path)
            {
                continue;
            }
            let Ok(source) = fs::read_to_string(&path) else {
                violations.push(format!("cannot read {}", path.display()));
                continue;
            };
            for (index, line) in source.lines().enumerate() {
                if forbidden_transport_call(line) {
                    violations.push(format!("{}:{}: {}", path.display(), index + 1, line.trim()));
                }
            }
        }
    }

    fn allowed_layer_path(path: &Path) -> bool {
        let text = path.to_string_lossy();
        text.ends_with("source_routing/adapter.rs")
            || text.ends_with("modules/logos_core.rs")
            || text.ends_with("source_routing/core/layer.rs")
            || text.ends_with("source_routing/storage/layer.rs")
            || text.ends_with("source_routing/delivery/layer.rs")
            || text.ends_with("source_routing/channel_sources/layer.rs")
    }

    fn forbidden_transport_call(line: &str) -> bool {
        const PATTERNS: &[&str] = &[
            "crate::lez::sequencer_",
            "crate::lez::indexer_",
            "crate::lez::last_sequencer_block_id(",
            "crate::lez::account_transactions_by_account(",
            "crate::blockchain::blockchain_",
            "crate::blockchain::channels::channel_",
            "LogoscoreCliTransport::from_runtime(",
            ".call_checked(",
            "runtime.ensure_module_loaded(",
            "source_routing::shared::http::",
            "source_routing::shared::module_bridge::call_value(",
        ];
        PATTERNS.iter().any(|pattern| line.contains(pattern))
    }
}
