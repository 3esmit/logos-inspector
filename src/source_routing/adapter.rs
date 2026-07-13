use serde::Serialize;
use serde_json::Value;

use crate::modules::logos_core::{LogoscoreCliRuntime, LogoscoreCliTransport, ModuleTransport};

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

pub(crate) mod sealed {
    pub trait Sealed {}
}

pub(crate) trait AdapterLayer: sealed::Sealed {
    fn key(&self) -> &'static str;
    fn modes(&self) -> &'static [SourceModePolicy];
}

#[must_use]
pub(crate) fn adapter_for_connector(
    layers: &[&dyn AdapterLayer],
    connector_id: &str,
) -> Option<&'static SourceAdapterPolicy> {
    layers
        .iter()
        .filter(|layer| !layer.key().is_empty())
        .flat_map(|layer| layer.modes())
        .find(|mode| mode.adapter.connector_id == connector_id)
        .map(|mode| &mode.adapter)
}

pub(crate) fn ensure_managed_module(
    runtime: &LogoscoreCliRuntime,
    module: &str,
) -> anyhow::Result<()> {
    runtime.ensure_module_loaded(module)
}

pub(crate) fn call_managed_module(
    runtime: &LogoscoreCliRuntime,
    module: &str,
    method: &str,
    signature: &str,
    args: &[String],
) -> anyhow::Result<Value> {
    runtime.require_module_method(module, method, signature)?;
    LogoscoreCliTransport::for_runtime(runtime.clone()).call(module, method, args)
}

#[cfg(test)]
pub(crate) mod contract_tests {
    use std::{collections::BTreeSet, fs, path::Path};

    use super::{AdapterConnectionType, AdapterLayer, SourceAdapterPolicy};

    pub(crate) fn assert_layer_contract(layer: &dyn AdapterLayer) {
        let modes = layer.modes();
        assert!(
            modes.len() >= 2,
            "{} must expose a real adapter seam",
            layer.key()
        );

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
    fn node_transport_calls_are_encapsulated_by_layer_implementations() {
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
            || text.ends_with("source_routing/core/layer.rs")
            || text.ends_with("source_routing/storage/layer.rs")
            || text.ends_with("source_routing/delivery/layer.rs")
            || text.ends_with("source_routing/channel_sources/layer.rs")
            || text.contains("/blockchain/")
            || text.contains("/lez/")
            || text.contains("/modules/")
    }

    fn forbidden_transport_call(line: &str) -> bool {
        const PATTERNS: &[&str] = &[
            "crate::lez::sequencer_",
            "crate::lez::indexer_",
            "crate::lez::last_sequencer_block_id(",
            "crate::lez::account_transactions_by_account(",
            "crate::blockchain::blockchain_",
            "crate::blockchain::channels::channel_",
            "LogoscoreCliTransport::for_runtime(",
            "source_routing::shared::http::",
            "source_routing::shared::module_bridge::call_value(",
        ];
        PATTERNS.iter().any(|pattern| line.contains(pattern))
    }
}
