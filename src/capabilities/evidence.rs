use std::{collections::BTreeMap, sync::Mutex};

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use super::{
    CapabilityBuildMode, CapabilityRegistryReport, CapabilityRuntimeInputs,
    capability_registry_report_with_inputs,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SourceScope {
    L1,
    Storage,
    Delivery,
}

impl SourceScope {
    const fn key(self) -> &'static str {
        match self {
            Self::L1 => "l1",
            Self::Storage => "storage",
            Self::Delivery => "delivery",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ModuleScope {
    Blockchain,
    Storage,
    Delivery,
}

impl ModuleScope {
    const fn key(self) -> &'static str {
        match self {
            Self::Blockchain => "blockchain",
            Self::Storage => "storage",
            Self::Delivery => "delivery",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum LastKnownScope {
    Wallet,
    LocalNodes,
}

impl LastKnownScope {
    const fn key(self) -> &'static str {
        match self {
            Self::Wallet => "wallet",
            Self::LocalNodes => "local_nodes",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObservationTarget {
    L1Connectivity,
    Source(SourceScope),
    Module(ModuleScope),
    LastKnown(LastKnownScope),
}

#[derive(Debug, Clone, Default)]
pub(super) struct CapabilityEvidenceSnapshot {
    source_reports: BTreeMap<SourceScope, Value>,
    module_reports: BTreeMap<ModuleScope, Value>,
    last_known: BTreeMap<LastKnownScope, String>,
}

impl CapabilityEvidenceSnapshot {
    pub(super) fn source_report(&self, scope: &str) -> Option<&Value> {
        let scope = match scope {
            "l1" => SourceScope::L1,
            "storage" => SourceScope::Storage,
            "delivery" => SourceScope::Delivery,
            _ => return None,
        };
        self.source_reports.get(&scope)
    }

    pub(super) fn diagnostics_report(&self) -> Option<Value> {
        if self.source_reports.is_empty()
            && self.module_reports.is_empty()
            && self.last_known.is_empty()
        {
            return None;
        }
        let module_reports = self
            .module_reports
            .iter()
            .map(|(scope, report)| (scope.key().to_owned(), report.clone()))
            .collect::<serde_json::Map<_, _>>();
        let source_reports = self
            .source_reports
            .iter()
            .map(|(scope, report)| (scope.key().to_owned(), report.clone()))
            .collect::<serde_json::Map<_, _>>();
        let last_known = self
            .last_known
            .iter()
            .map(|(scope, detail)| (scope.key().to_owned(), Value::String(detail.clone())))
            .collect::<serde_json::Map<_, _>>();
        Some(json!({
            "module_reports": module_reports,
            "source_reports": source_reports,
            "last_known": last_known,
        }))
    }

    fn observe_success(&mut self, method: &str, value: &Value) {
        let Some(target) = observation_target(method) else {
            return;
        };
        match target {
            ObservationTarget::L1Connectivity => {
                self.source_reports.insert(
                    SourceScope::L1,
                    json!({
                        "health": {
                            "ready": true,
                            "reachable": true,
                            "status": "ready",
                            "detail": "L1 provider call succeeded",
                            "summary": "L1 provider call succeeded",
                        },
                        "probe_facts": [{
                            "key": "blockchain.connection",
                            "ok": true,
                            "value": value,
                            "error": "",
                        }],
                    }),
                );
            }
            ObservationTarget::Source(scope) => {
                self.source_reports.insert(scope, value.clone());
            }
            ObservationTarget::Module(scope) => {
                self.module_reports.insert(scope, value.clone());
            }
            ObservationTarget::LastKnown(scope) => {
                self.last_known.remove(&scope);
            }
        }
    }

    fn observe_failure(&mut self, method: &str, detail: &str) {
        let Some(target) = observation_target(method) else {
            return;
        };
        match target {
            ObservationTarget::L1Connectivity => {
                self.source_reports.insert(
                    SourceScope::L1,
                    failed_source_report("blockchain.connection", detail),
                );
            }
            ObservationTarget::Source(scope) => {
                self.source_reports
                    .insert(scope, failed_source_report(scope.key(), detail));
            }
            ObservationTarget::Module(scope) => {
                self.module_reports.insert(
                    scope,
                    json!({
                        "module_info": { "ok": false, "error": detail },
                        "health": {
                            "ready": false,
                            "reachable": false,
                            "status": "unavailable",
                            "detail": detail,
                        },
                    }),
                );
            }
            ObservationTarget::LastKnown(scope) => {
                self.last_known.insert(scope, detail.to_owned());
            }
        }
    }

    #[cfg(test)]
    pub(super) fn from_legacy_test_value(value: Option<&Value>) -> Self {
        let Some(value) = value else {
            return Self::default();
        };
        let mut evidence = Self::default();
        if let Some(reports) = value.get("source_reports").and_then(Value::as_object) {
            for (key, report) in reports {
                let scope = match key.as_str() {
                    "l1" | "blockchain" | "node" => Some(SourceScope::L1),
                    "storage" | "storage_source" => Some(SourceScope::Storage),
                    "delivery" | "delivery_source" | "messaging" | "messaging_source" => {
                        Some(SourceScope::Delivery)
                    }
                    _ => None,
                };
                if let Some(scope) = scope {
                    evidence.source_reports.insert(scope, report.clone());
                }
            }
        }
        if let Some(report) = value.get("diagnostics_reports").and_then(Value::as_object) {
            evidence.import_test_diagnostics(report);
        }
        evidence
    }

    #[cfg(test)]
    fn import_test_diagnostics(&mut self, report: &serde_json::Map<String, Value>) {
        if let Some(reports) = report.get("module_reports").and_then(Value::as_object) {
            for (key, value) in reports {
                let scope = match key.as_str() {
                    "blockchain" | "l1" => Some(ModuleScope::Blockchain),
                    "storage" => Some(ModuleScope::Storage),
                    "delivery" | "messaging" => Some(ModuleScope::Delivery),
                    _ => None,
                };
                if let Some(scope) = scope {
                    self.module_reports.insert(scope, value.clone());
                }
            }
        }
        if let Some(reports) = report.get("source_reports").and_then(Value::as_object) {
            for (key, value) in reports {
                let scope = match key.as_str() {
                    "l1" | "blockchain" | "node" => Some(SourceScope::L1),
                    "storage" | "storage_source" => Some(SourceScope::Storage),
                    "delivery" | "delivery_source" | "messaging" | "messaging_source" => {
                        Some(SourceScope::Delivery)
                    }
                    _ => None,
                };
                if let Some(scope) = scope {
                    self.source_reports.insert(scope, value.clone());
                }
            }
        }
        if let Some(last_known) = report.get("last_known").and_then(Value::as_object) {
            for (key, value) in last_known {
                let scope = match key.as_str() {
                    "wallet" => Some(LastKnownScope::Wallet),
                    "local_nodes" | "localNodes" => Some(LastKnownScope::LocalNodes),
                    _ => None,
                };
                if let (Some(scope), Some(detail)) = (scope, value.as_str())
                    && !detail.trim().is_empty()
                {
                    self.last_known.insert(scope, detail.trim().to_owned());
                }
            }
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct CapabilityRegistry {
    evidence: Mutex<CapabilityEvidenceSnapshot>,
}

impl CapabilityRegistry {
    pub(crate) fn observe_success(&self, method: &str, value: &Value) -> Result<()> {
        self.evidence
            .lock()
            .map_err(|_| anyhow!("capability evidence lock is poisoned"))?
            .observe_success(method, value);
        Ok(())
    }

    pub(crate) fn observe_failure(&self, method: &str, detail: &str) -> Result<()> {
        self.evidence
            .lock()
            .map_err(|_| anyhow!("capability evidence lock is poisoned"))?
            .observe_failure(method, detail);
        Ok(())
    }

    pub(crate) fn report(
        &self,
        build_mode: CapabilityBuildMode,
        value: Option<&Value>,
    ) -> Result<CapabilityRegistryReport> {
        let evidence = self
            .evidence
            .lock()
            .map_err(|_| anyhow!("capability evidence lock is poisoned"))?
            .clone();
        let inputs = CapabilityRuntimeInputs::from_value_with_evidence(value, evidence);
        Ok(capability_registry_report_with_inputs(build_mode, &inputs))
    }
}

fn failed_source_report(key: &str, detail: &str) -> Value {
    json!({
        "health": {
            "ready": false,
            "reachable": false,
            "status": "unavailable",
            "detail": detail,
            "summary": detail,
        },
        "probe_facts": [{ "key": key, "ok": false, "error": detail }],
    })
}

fn observation_target(method: &str) -> Option<ObservationTarget> {
    match method {
        "blockchainNode" => Some(ObservationTarget::L1Connectivity),
        "storageSourceReport" => Some(ObservationTarget::Source(SourceScope::Storage)),
        "deliverySourceReport" => Some(ObservationTarget::Source(SourceScope::Delivery)),
        "blockchainModuleReport" => Some(ObservationTarget::Module(ModuleScope::Blockchain)),
        "storageReport" => Some(ObservationTarget::Module(ModuleScope::Storage)),
        "deliveryReport" => Some(ObservationTarget::Module(ModuleScope::Delivery)),
        "localNodesStatus" | "localNodesAction" => {
            Some(ObservationTarget::LastKnown(LastKnownScope::LocalNodes))
        }
        "localWalletProfileStatus"
        | "localWalletAccounts"
        | "localWalletCreateAccount"
        | "localWalletSendTransaction"
        | "localWalletCommand"
        | "localWalletSyncPrivate"
        | "localWalletDeployProgram"
        | "localWalletInstructionSubmit" => {
            Some(ObservationTarget::LastKnown(LastKnownScope::Wallet))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{Context as _, Result, bail};

    use super::*;

    #[test]
    fn registry_observes_provider_results_without_qml_reports() -> Result<()> {
        let registry = CapabilityRegistry::default();
        registry.observe_success("blockchainNode", &json!({ "slot": 9 }))?;
        registry.observe_success(
            "storageSourceReport",
            &json!({ "health": { "ready": true, "status": "ready" } }),
        )?;

        let report = serde_json::to_value(registry.report(
            CapabilityBuildMode::Standalone,
            Some(&json!({
                "node_url": "http://127.0.0.1:8545",
                "storage_rest_url": "http://127.0.0.1:8081",
                "storage_mutating_diagnostics_enabled": true,
                "network_connector_config": {
                    "scopes": {
                        "l1": { "connector_id": "direct_l1_rpc" },
                        "storage": { "connector_id": "direct_storage_rest" }
                    }
                }
            })),
        )?)?;
        let capabilities = report
            .get("capabilities")
            .and_then(Value::as_array)
            .context("missing capabilities")?;
        for key in ["l1", "storage"] {
            let capability = capabilities
                .iter()
                .find(|entry| entry.get("key").and_then(Value::as_str) == Some(key))
                .with_context(|| format!("missing {key} capability"))?;
            if capability.get("status").and_then(Value::as_str) != Some("available") {
                bail!("{key} evidence was not applied: {capability}");
            }
        }
        Ok(())
    }

    #[test]
    fn registry_records_typed_failure_evidence() -> Result<()> {
        let registry = CapabilityRegistry::default();
        registry.observe_failure("blockchainNode", "connection refused")?;
        let report = registry.report(
            CapabilityBuildMode::Standalone,
            Some(&json!({ "node_url": "http://127.0.0.1:8545" })),
        )?;
        let l1 = report
            .capabilities
            .iter()
            .find(|capability| capability.key == "l1")
            .context("missing l1 capability")?;
        if l1.status != "unavailable"
            || !l1
                .compact_errors
                .iter()
                .any(|error| error.contains("connection refused"))
        {
            bail!("failure evidence was not projected: {l1:?}");
        }
        Ok(())
    }
}
